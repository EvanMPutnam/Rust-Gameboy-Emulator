use gb_core::bus::{Bus, Memory};
use gb_core::cartridge::Cartridge;

fn build_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0150] = 0xAA;
    rom[0x4000] = 0xBB;
    rom
}

#[test]
fn routes_memory_regions_correctly() {
    let cart = Cartridge::from_rom_bytes(build_rom()).expect("valid rom");
    let mut bus = Bus::new(cart);

    assert_eq!(bus.read8(0x0150).expect("rom read"), 0xAA);
    assert_eq!(bus.read8(0x4000).expect("rom bank read"), 0xBB);

    bus.write8(0xC123, 0x42).expect("wram write");
    assert_eq!(bus.read8(0xC123).expect("wram read"), 0x42);

    bus.write8(0x8001, 0x55).expect("vram write");
    assert_eq!(bus.read8(0x8001).expect("vram read"), 0x55);

    bus.write8(0xFE00, 0x66).expect("oam write");
    assert_eq!(bus.read8(0xFE00).expect("oam read"), 0x66);

    bus.write8(0xFF80, 0x77).expect("hram write");
    assert_eq!(bus.read8(0xFF80).expect("hram read"), 0x77);
}

#[test]
fn echo_ram_mirrors_wram() {
    let cart = Cartridge::from_rom_bytes(build_rom()).expect("valid rom");
    let mut bus = Bus::new(cart);

    bus.write8(0xC000, 0x12).expect("wram write");
    assert_eq!(bus.read8(0xE000).expect("echo read"), 0x12);

    bus.write8(0xE100, 0x34).expect("echo write");
    assert_eq!(bus.read8(0xC100).expect("wram read"), 0x34);
}

#[test]
fn unusable_range_reads_ff_and_ignores_writes() {
    let cart = Cartridge::from_rom_bytes(build_rom()).expect("valid rom");
    let mut bus = Bus::new(cart);

    bus.write8(0xFEA0, 0x99).expect("unusable write is ignored");
    assert_eq!(bus.read8(0xFEA0).expect("unusable read"), 0xFF);
}

#[test]
fn ie_and_if_registers_round_trip() {
    let cart = Cartridge::from_rom_bytes(build_rom()).expect("valid rom");
    let mut bus = Bus::new(cart);

    bus.write8(0xFFFF, 0x1F).expect("ie write");
    bus.write8(0xFF0F, 0xFF).expect("if write");

    assert_eq!(bus.read8(0xFFFF).expect("ie read"), 0x1F);
    assert_eq!(bus.read8(0xFF0F).expect("if read"), 0xFF);
}
