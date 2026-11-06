use gb_core::bus::{Memory, TimerRegs};
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;
use gb_core::interrupts::{Interrupt, InterruptController};
use gb_core::timers::Timers;

#[test]
fn div_increments_with_cycles() {
    let mut timers = Timers::new();
    let mut interrupts = InterruptController::new();

    timers.tick(256, &mut interrupts);
    let regs = timers.export_regs();
    assert_eq!(regs.div, 1);
}

#[test]
fn tima_overflow_reloads_tma_and_requests_interrupt() {
    let mut timers = Timers::new();
    let mut interrupts = InterruptController::new();
    timers.apply_bus_regs(TimerRegs {
        div: 0,
        tima: 0xFF,
        tma: 0xAC,
        tac: 0x05,
    });

    timers.tick(16, &mut interrupts);
    let regs = timers.export_regs();

    assert_eq!(regs.tima, 0xAC);
    assert_ne!(interrupts.iflags & Interrupt::Timer.bit(), 0);
}

#[test]
fn tac_frequency_controls_increment_rate() {
    let mut timers = Timers::new();
    let mut interrupts = InterruptController::new();
    timers.apply_bus_regs(TimerRegs {
        div: 0,
        tima: 0,
        tma: 0,
        tac: 0x05,
    });

    timers.tick(32, &mut interrupts);
    let regs = timers.export_regs();

    assert_eq!(regs.tima, 2);
}

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn gameboy_step_syncs_timer_and_interrupt_registers() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("gb init");

    gb.bus.write8(0xFF07, 0x05).expect("write tac");
    gb.bus.write8(0xFF05, 0xFF).expect("write tima");
    gb.bus.write8(0xFF06, 0x77).expect("write tma");
    gb.bus
        .write8(0xFFFF, Interrupt::Timer.bit())
        .expect("enable timer interrupt");

    gb.run_cycles(16).expect("run cycles");

    assert_eq!(gb.bus.read8(0xFF05).expect("read tima"), 0x77);
    assert_ne!(
        gb.bus.read8(0xFF0F).expect("read if") & Interrupt::Timer.bit(),
        0
    );
}

#[test]
fn cpu_writes_to_timer_registers_take_effect() {
    // LD A,$05; LDH (TAC),A; LD A,$AB; LDH (TMA),A; NOP...
    let mut rom = test_rom();
    rom[0x0100..0x0108].copy_from_slice(&[0x3E, 0x05, 0xE0, 0x07, 0x3E, 0xAB, 0xE0, 0x06]);
    let mut gb = GameBoy::new(GbConfig::default(), rom).expect("gb init");

    gb.run_cycles(64).expect("run cycles");

    assert_eq!(gb.bus.read8(0xFF07).expect("read tac") & 0x07, 0x05);
    assert_eq!(gb.bus.read8(0xFF06).expect("read tma"), 0xAB);
    assert_ne!(gb.bus.read8(0xFF05).expect("read tima"), 0);
}
