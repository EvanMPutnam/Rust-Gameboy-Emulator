use gb_core::bus::{Bus, Memory};
use gb_core::cartridge::Cartridge;
use gb_core::cpu::Cpu;
use gb_core::interrupts::{Interrupt, InterruptController};

fn rom_with_program(program: &[u8]) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100..0x0100 + program.len()].copy_from_slice(program);
    rom
}

fn setup(program: &[u8]) -> (Cpu, Bus, InterruptController) {
    let cart = Cartridge::from_rom_bytes(rom_with_program(program)).expect("valid test rom");
    (Cpu::new(), Bus::new(cart), InterruptController::new())
}

#[test]
fn services_interrupt_when_ime_enabled() {
    let (mut cpu, mut bus, mut interrupts) = setup(&[0x00]);
    cpu.registers_mut().pc = 0x1234;
    cpu.registers_mut().sp = 0xFFFE;
    cpu.set_ime(true);

    interrupts.ie = Interrupt::Timer.bit();
    interrupts.iflags = Interrupt::Timer.bit();

    let cycles = cpu
        .step(&mut bus, &mut interrupts)
        .expect("interrupt should service");

    assert_eq!(cycles, 20);
    assert_eq!(cpu.registers().pc, 0x0050);
    assert_eq!(cpu.registers().sp, 0xFFFC);
    assert_eq!(bus.read8(0xFFFC).expect("stack lo"), 0x34);
    assert_eq!(bus.read8(0xFFFD).expect("stack hi"), 0x12);
    assert_eq!(interrupts.iflags & Interrupt::Timer.bit(), 0);
    assert!(!cpu.ime());
}

#[test]
fn uses_priority_order_for_pending_interrupts() {
    let mut interrupts = InterruptController::new();
    interrupts.ie = Interrupt::Timer.bit() | Interrupt::VBlank.bit();
    interrupts.iflags = Interrupt::Timer.bit() | Interrupt::VBlank.bit();

    assert_eq!(interrupts.pending(), Some(Interrupt::VBlank));
}

#[test]
fn halt_wakes_when_interrupt_pending_even_if_ime_disabled() {
    let (mut cpu, mut bus, mut interrupts) = setup(&[0x76, 0x00]);
    cpu.set_ime(false);

    cpu.step(&mut bus, &mut interrupts)
        .expect("halt should run");
    assert!(cpu.is_halted());

    interrupts.ie = Interrupt::Timer.bit();
    interrupts.iflags = Interrupt::Timer.bit();

    cpu.step(&mut bus, &mut interrupts)
        .expect("cpu should wake and execute nop");
    assert!(!cpu.is_halted());
}
