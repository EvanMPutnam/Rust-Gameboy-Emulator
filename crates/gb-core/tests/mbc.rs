use gb_core::cartridge::{Cartridge, header::CartridgeHeader};

fn make_rom(rom_banks: usize, cart_type: u8, rom_code: u8, ext_ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0u8; rom_banks * 0x4000];
    rom[0x0147] = cart_type;
    rom[0x0148] = rom_code;
    rom[0x0149] = ext_ram_code;
    rom
}

#[test]
fn parses_header_metadata() {
    let mut rom = make_rom(2, 0x00, 0x00, 0x00);
    rom[0x0134..0x0138].copy_from_slice(b"TEST");

    let header = CartridgeHeader::parse(&rom).expect("header parse");
    assert_eq!(header.title, "TEST");
    assert_eq!(header.cartridge_type, 0x00);
}

#[test]
fn rom_only_reads_from_fixed_rom() {
    let mut rom = make_rom(2, 0x00, 0x00, 0x00);
    rom[0x1234] = 0xAB;

    let cart = Cartridge::from_rom_bytes(rom).expect("cart parse");
    assert_eq!(cart.read_rom(0x1234).expect("rom read"), 0xAB);
}

#[test]
fn mbc1_switches_rom_banks() {
    let mut rom = make_rom(4, 0x01, 0x01, 0x03);
    rom[0x4000] = 0x11;
    rom[0x8000] = 0x22;
    rom[0xC000] = 0x33;

    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");

    assert_eq!(cart.read_rom(0x4000).expect("default bank read"), 0x11);

    cart.write_rom(0x2000, 0x02).expect("switch bank");
    assert_eq!(cart.read_rom(0x4000).expect("bank2 read"), 0x22);

    cart.write_rom(0x2000, 0x03).expect("switch bank");
    assert_eq!(cart.read_rom(0x4000).expect("bank3 read"), 0x33);
}

#[test]
fn mbc1_requires_ram_enable_for_writes() {
    let rom = make_rom(4, 0x03, 0x01, 0x03);
    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");

    cart.write_ram(0xA000, 0x55)
        .expect("disabled write ignored");
    assert_eq!(cart.read_ram(0xA000).expect("disabled read"), 0xFF);

    cart.write_rom(0x0000, 0x0A).expect("enable ram");
    cart.write_ram(0xA000, 0x55).expect("enabled ram write");
    assert_eq!(cart.read_ram(0xA000).expect("enabled ram read"), 0x55);
}

#[test]
fn mbc2_uses_4bit_ram_and_address_gated_register_writes() {
    let mut rom = make_rom(16, 0x06, 0x03, 0x00);
    rom[0x4000] = 0x11;
    rom[0x8000] = 0x22;
    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");

    cart.write_rom(0x2100, 0x02).expect("select bank 2");
    assert_eq!(cart.read_rom(0x4000).expect("banked read"), 0x22);

    cart.write_rom(0x0000, 0x0A).expect("enable ram");
    cart.write_ram(0xA000, 0xAB).expect("write nibble");
    assert_eq!(cart.read_ram(0xA000).expect("read nibble"), 0xFB);
}

#[test]
fn mbc3_supports_rom_ram_banks_and_rtc_register_selection() {
    let mut rom = make_rom(8, 0x10, 0x02, 0x03);
    rom[0x4000] = 0x11;
    rom[0x8000] = 0x22;
    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");

    cart.write_rom(0x2000, 0x02).expect("select rom bank 2");
    assert_eq!(cart.read_rom(0x4000).expect("banked read"), 0x22);

    cart.write_rom(0x0000, 0x0A).expect("enable ram");
    cart.write_rom(0x4000, 0x01).expect("select ram bank 1");
    cart.write_ram(0xA000, 0x66).expect("write banked ram");
    assert_eq!(cart.read_ram(0xA000).expect("read banked ram"), 0x66);

    cart.write_rom(0x4000, 0x08).expect("select rtc seconds");
    cart.write_ram(0xA000, 0x3A).expect("write rtc seconds");
    cart.write_rom(0x6000, 0x00).expect("latch arm");
    cart.write_rom(0x6000, 0x01).expect("latch trigger");
    assert_eq!(cart.read_ram(0xA000).expect("latched rtc seconds"), 0x3A);
}

#[test]
fn mbc5_supports_9bit_rom_banks_and_ram_banks() {
    let mut rom = make_rom(260, 0x1B, 0x07, 0x03);
    rom[0x4000] = 0x01;
    let marker_index = 257 * 0x4000;
    rom[marker_index] = 0xBE;
    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");

    cart.write_rom(0x2000, 0x01).expect("set bank low");
    cart.write_rom(0x3000, 0x01).expect("set bank high");
    assert_eq!(cart.read_rom(0x4000).expect("read bank 257"), 0xBE);

    cart.write_rom(0x0000, 0x0A).expect("enable ram");
    cart.write_rom(0x4000, 0x01).expect("select ram bank 1");
    cart.write_ram(0xA000, 0x5A).expect("write banked ram");
    assert_eq!(cart.read_ram(0xA000).expect("read banked ram"), 0x5A);
}

#[test]
fn mbc1_mode1_maps_bank0_to_low_region_when_upper_bits_are_zero() {
    let mut rom = make_rom(4, 0x01, 0x01, 0x00);
    rom[0x0000] = 0xAA;
    rom[0x4000] = 0xBB;

    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");
    cart.write_rom(0x6000, 0x01).expect("select mode 1");

    assert_eq!(cart.read_rom(0x0000).expect("low region read"), 0xAA);
}

#[test]
fn mbc2_register_select_uses_address_bit_8_across_whole_range() {
    let mut rom = make_rom(4, 0x05, 0x01, 0x00);
    rom[0x8000] = 0x22;

    let mut cart = Cartridge::from_rom_bytes(rom).expect("cart parse");
    cart.write_rom(0x0100, 0x02)
        .expect("select bank via 0x0100");
    assert_eq!(cart.read_rom(0x4000).expect("bank 2 read"), 0x22);

    cart.write_rom(0x2000, 0x0A).expect("enable ram via 0x2000");
    cart.write_ram(0xA000, 0x05).expect("ram write");
    assert_eq!(cart.read_ram(0xA000).expect("ram read"), 0xF5);
}

#[test]
fn mbc5_powers_on_with_bank1_in_switchable_region() {
    let mut rom = make_rom(4, 0x19, 0x01, 0x00);
    rom[0x4000] = 0x11;

    let cart = Cartridge::from_rom_bytes(rom).expect("cart parse");
    assert_eq!(cart.read_rom(0x4000).expect("default bank read"), 0x11);
}
