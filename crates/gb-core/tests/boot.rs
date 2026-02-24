use gb_core::bus::Memory;
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;

fn rom_with_header() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

#[test]
fn boot_rom_overlay_is_active_when_configured() {
    let mut boot_rom = [0u8; 256];
    boot_rom[0x00] = 0xAB;

    let mut rom = rom_with_header();
    rom[0x00] = 0x11;

    let config = GbConfig {
        force_post_boot_state: false,
        use_boot_rom: true,
        boot_rom: Some(boot_rom),
    };
    let gb = GameBoy::new(config, rom).expect("init gb");

    assert_eq!(gb.bus.read8(0x0000).expect("boot read"), 0xAB);
    assert!(gb.bus.is_boot_rom_enabled());
}

#[test]
fn ff50_unmaps_boot_rom_permanently_until_reset() {
    let mut boot_rom = [0u8; 256];
    boot_rom[0x00] = 0xAB;

    let mut rom = rom_with_header();
    rom[0x00] = 0x11;

    let config = GbConfig {
        force_post_boot_state: false,
        use_boot_rom: true,
        boot_rom: Some(boot_rom),
    };
    let mut gb = GameBoy::new(config, rom).expect("init gb");

    gb.bus.write8(0xFF50, 0x01).expect("disable boot rom");
    assert_eq!(gb.bus.read8(0x0000).expect("rom read"), 0x11);

    gb.bus
        .write8(0xFF50, 0x00)
        .expect("writes cannot re-enable");
    assert_eq!(gb.bus.read8(0x0000).expect("rom read"), 0x11);
    assert!(!gb.bus.is_boot_rom_enabled());
}

#[test]
fn force_post_boot_state_applies_when_boot_rom_is_disabled() {
    let config = GbConfig {
        force_post_boot_state: true,
        use_boot_rom: false,
        boot_rom: None,
    };
    let gb = GameBoy::new(config, rom_with_header()).expect("init gb");

    assert_eq!(gb.cpu.registers().pc, 0x0100);
    assert_eq!(gb.cpu.registers().sp, 0xFFFE);
    assert_eq!(gb.cpu.registers().a, 0x01);
}
