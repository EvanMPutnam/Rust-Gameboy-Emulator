use gb_core::bus::{Bus, Memory};
use gb_core::cartridge::Cartridge;

fn test_bus() -> Bus {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;

    let cartridge = Cartridge::from_rom_bytes(rom).expect("cartridge should load");
    Bus::new(cartridge)
}

#[test]
fn same_value_apu_register_writes_are_still_reported() {
    let mut bus = test_bus();

    bus.write8(0xFF12, 0xF3).expect("first write should pass");
    let first = bus.take_apu_write_mask();
    assert_ne!(first & (1u128 << 0x12), 0);

    bus.write8(0xFF12, 0xF3)
        .expect("same-value write should pass");
    let second = bus.take_apu_write_mask();
    assert_ne!(second & (1u128 << 0x12), 0);
}

#[test]
fn non_apu_writes_do_not_set_apu_write_mask() {
    let mut bus = test_bus();

    bus.write8(0xFF40, 0x91)
        .expect("non-apu register write should pass");
    let write_mask = bus.take_apu_write_mask();
    assert_eq!(write_mask & (1u128 << 0x40), 0);
    assert_eq!(write_mask & (1u128 << 0x10), 0);
}
