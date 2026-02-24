use gb_core::bus::Memory;
use gb_core::{GameBoy, GbConfig};

fn rom_with_header() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

#[test]
fn ff50_zero_does_not_disable_boot_rom_mapping() {
    let mut boot_rom = [0u8; 256];
    boot_rom[0x00] = 0xAA;

    let mut rom = rom_with_header();
    rom[0x00] = 0x55;

    let config = GbConfig {
        force_post_boot_state: false,
        use_boot_rom: true,
        boot_rom: Some(boot_rom),
    };
    let mut gb = GameBoy::new(config, rom).expect("init gb");

    gb.bus.write8(0xFF50, 0x00).expect("write ff50 zero");
    assert_eq!(gb.bus.read8(0x0000).expect("read boot rom"), 0xAA);
}
