use gb_core::bus::Memory;
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;
use gb_core::joypad::GbButton;

fn test_rom(cart_type: u8, rom_code: u8, ext_ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = cart_type;
    rom[0x0148] = rom_code;
    rom[0x0149] = ext_ram_code;
    rom[0x0100] = 0x00;
    rom
}

fn state_hash(gb: &GameBoy) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let bytes = [
        gb.cpu.registers().a,
        gb.cpu.registers().b,
        gb.cpu.registers().c,
        gb.cpu.registers().d,
        gb.cpu.registers().e,
        gb.cpu.registers().h,
        gb.cpu.registers().l,
        gb.bus.read_if(),
        gb.bus.read_ie(),
    ];

    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0001_0000_01B3);
    }

    hash ^ u64::from(gb.cpu.registers().pc) ^ (u64::from(gb.clock.now()) << 16)
}

#[test]
fn deterministic_cycle_run_produces_stable_hash() {
    let mut gb_a = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00, 0x00)).expect("init gb");
    let mut gb_b = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00, 0x00)).expect("init gb");

    gb_a.run_cycles(50_000).expect("run a");
    gb_b.run_cycles(50_000).expect("run b");

    assert_eq!(state_hash(&gb_a), state_hash(&gb_b));
}

#[test]
fn scripted_input_updates_joypad_state_deterministically() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x00, 0x00, 0x00)).expect("init gb");
    gb.bus.write8(0xFF00, 0x20).expect("select dpad");

    gb.run_cycles(256).expect("pre-input run");
    gb.set_button(GbButton::Up, true).expect("press up");
    gb.run_cycles(256).expect("post-press run");
    gb.set_button(GbButton::Up, false).expect("release up");

    assert_ne!(gb.bus.read8(0xFF00).expect("read p1") & 0x04, 0);
}

#[test]
fn facade_save_export_import_preserves_mapper_ram() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom(0x03, 0x00, 0x03)).expect("init gb");
    gb.bus.write8(0x0000, 0x0A).expect("enable ram");
    gb.bus.write8(0xA010, 0x5E).expect("write ext ram");

    let save = gb.export_save().expect("export save");
    let mut restored =
        GameBoy::new(GbConfig::default(), test_rom(0x03, 0x00, 0x03)).expect("init gb");
    restored.import_save(&save).expect("import save");
    restored.bus.write8(0x0000, 0x0A).expect("enable ram");

    assert_eq!(restored.bus.read8(0xA010).expect("read ext ram"), 0x5E);
}
