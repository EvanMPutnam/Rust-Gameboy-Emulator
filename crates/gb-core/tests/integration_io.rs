use gb_core::bus::Memory;
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;
use gb_core::interrupts::Interrupt;
use gb_core::joypad::GbButton;

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn io_reads_and_writes_reflect_subsystem_state() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF00, 0x20).expect("select dpad");
    gb.set_button(GbButton::Right, true).expect("press right");
    assert_eq!(gb.bus.read8(0xFF00).expect("read p1") & 0x01, 0);

    gb.bus.write8(0xFF01, 0xA5).expect("write sb");
    gb.bus.write8(0xFF02, 0x81).expect("start serial");
    gb.run_cycles(4_096).expect("run serial transfer");
    assert_eq!(gb.bus.read8(0xFF01).expect("read sb"), 0xFF);
}

#[test]
fn serial_and_joypad_interrupts_follow_priority_order() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.interrupts.ie = Interrupt::Serial.bit() | Interrupt::Joypad.bit();

    gb.bus.write8(0xFF00, 0x20).expect("select dpad");
    gb.set_button(GbButton::Left, true).expect("press left");
    gb.bus.write8(0xFF02, 0x81).expect("start serial");
    gb.run_cycles(4_096).expect("complete serial transfer");

    assert_eq!(gb.interrupts.pending(), Some(Interrupt::Serial));
}

#[test]
fn concurrent_dma_and_cpu_stepping_is_deterministic() {
    let mut gb_a = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    let mut gb_b = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");

    for offset in 0u16..160 {
        let value = u8::try_from(offset).expect("offset is in u8 range");
        gb_a.bus
            .write8(0xC000 + offset, value)
            .expect("seed source a");
        gb_b.bus
            .write8(0xC000 + offset, value)
            .expect("seed source b");
    }

    gb_a.bus.write8(0xFF46, 0xC0).expect("start dma a");
    gb_b.bus.write8(0xFF46, 0xC0).expect("start dma b");

    gb_a.run_cycles(1_024).expect("run a");
    gb_b.run_cycles(1_024).expect("run b");

    assert_eq!(gb_a.bus.oam, gb_b.bus.oam);
    assert_eq!(gb_a.cpu.registers().pc, gb_b.cpu.registers().pc);
}
