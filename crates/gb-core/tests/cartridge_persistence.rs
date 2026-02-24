use gb_core::cartridge::Cartridge;
use gb_core::cartridge::rtc::RtcState;
use gb_core::cartridge::save::CartridgeSave;
use gb_core::error::CartridgeError;

fn make_rom(rom_banks: usize, cart_type: u8, rom_code: u8, ext_ram_code: u8) -> Vec<u8> {
    let mut rom = vec![0u8; rom_banks * 0x4000];
    rom[0x0147] = cart_type;
    rom[0x0148] = rom_code;
    rom[0x0149] = ext_ram_code;
    rom
}

#[test]
fn ram_only_export_import_round_trip() {
    let mut cart = Cartridge::from_rom_bytes(make_rom(4, 0x03, 0x01, 0x03)).expect("cart parse");
    cart.write_rom(0x0000, 0x0A).expect("enable ram");
    cart.write_ram(0xA123, 0x77).expect("write ram");

    let save = cart.export_save();
    let mut restored =
        Cartridge::from_rom_bytes(make_rom(4, 0x03, 0x01, 0x03)).expect("cart parse");
    restored.import_save(&save).expect("import save");
    restored.write_rom(0x0000, 0x0A).expect("enable ram");

    assert_eq!(restored.read_ram(0xA123).expect("read restored"), 0x77);
}

#[test]
fn rtc_enabled_export_import_round_trip() {
    let mut cart = Cartridge::from_rom_bytes(make_rom(8, 0x10, 0x02, 0x03)).expect("cart parse");
    cart.write_rom(0x0000, 0x0A).expect("enable ram/rtc");
    cart.write_rom(0x4000, 0x08).expect("select rtc seconds");
    cart.write_ram(0xA000, 0x1D).expect("set seconds");

    let save = cart.export_save();
    assert!(save.rtc.is_some());

    let mut restored =
        Cartridge::from_rom_bytes(make_rom(8, 0x10, 0x02, 0x03)).expect("cart parse");
    restored.import_save(&save).expect("import save");
    restored.write_rom(0x0000, 0x0A).expect("enable ram/rtc");
    restored
        .write_rom(0x4000, 0x08)
        .expect("select rtc seconds");
    restored.write_rom(0x6000, 0x00).expect("latch arm");
    restored.write_rom(0x6000, 0x01).expect("latch trigger");

    assert_eq!(restored.read_ram(0xA000).expect("read seconds"), 0x1D);
}

#[test]
fn import_rejects_size_mismatched_ram_payloads() {
    let mut cart = Cartridge::from_rom_bytes(make_rom(4, 0x03, 0x01, 0x03)).expect("cart parse");
    let save = CartridgeSave {
        ram: Some(vec![0; 4]),
        rtc: None,
    };

    let err = cart
        .import_save(&save)
        .expect_err("size mismatch should fail");
    assert!(matches!(
        err,
        CartridgeError::SaveRamSizeMismatch {
            expected: _,
            actual: 4
        }
    ));
}

#[test]
fn import_rejects_missing_rtc_state_for_rtc_mapper() {
    let mut cart = Cartridge::from_rom_bytes(make_rom(8, 0x10, 0x02, 0x03)).expect("cart parse");
    let save = CartridgeSave {
        ram: Some(vec![0; 0x8000]),
        rtc: None,
    };

    let err = cart
        .import_save(&save)
        .expect_err("missing rtc should fail");
    assert!(matches!(err, CartridgeError::MissingRtcState));
}

#[test]
fn import_accepts_explicit_rtc_payload() {
    let mut cart = Cartridge::from_rom_bytes(make_rom(8, 0x10, 0x02, 0x03)).expect("cart parse");
    let save = CartridgeSave {
        ram: Some(vec![0; 0x8000]),
        rtc: Some(RtcState {
            seconds: 42,
            minutes: 0,
            hours: 0,
            day_counter: 0,
            halt: false,
            day_carry: false,
        }),
    };

    cart.import_save(&save).expect("import should succeed");
    cart.write_rom(0x0000, 0x0A).expect("enable ram/rtc");
    cart.write_rom(0x4000, 0x08).expect("select rtc seconds");
    cart.write_rom(0x6000, 0x00).expect("latch arm");
    cart.write_rom(0x6000, 0x01).expect("latch trigger");
    assert_eq!(cart.read_ram(0xA000).expect("read seconds"), 42);
}
