use gb_core::bus::{Bus, Memory};
use gb_core::cartridge::Cartridge;
use gb_core::dma::OamDma;

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn dma_copies_exactly_160_bytes_to_oam() {
    let cartridge = Cartridge::from_rom_bytes(test_rom()).expect("valid test rom");
    let mut bus = Bus::new(cartridge);
    let mut dma = OamDma::new();

    for offset in 0u16..160 {
        let value = u8::try_from(offset).expect("offset is in u8 range");
        bus.write8(0xC000 + offset, value).expect("seed source");
    }

    dma.start(0xC0, &mut bus).expect("start dma");
    dma.tick(640, &mut bus).expect("finish dma");

    for offset in 0u16..160 {
        let value = u8::try_from(offset).expect("offset is in u8 range");
        assert_eq!(bus.oam[usize::from(offset)], value);
    }
}

#[test]
fn dma_blocks_cpu_access_to_data_regions_while_active() {
    let cartridge = Cartridge::from_rom_bytes(test_rom()).expect("valid test rom");
    let mut bus = Bus::new(cartridge);
    let mut dma = OamDma::new();

    bus.write8(0xC000, 0x12).expect("seed wram");
    dma.start(0xC0, &mut bus).expect("start dma");

    assert_eq!(bus.read8(0xC000).expect("blocked read"), 0xFF);
    bus.write8(0xC000, 0x34).expect("blocked write ignored");
    assert_eq!(bus.read_dma_source(0xC000).expect("raw read"), 0x12);

    assert_eq!(bus.read8(0x0000).expect("blocked rom read"), 0xFF);
    assert_eq!(bus.read8(0xFF40).expect("blocked io read"), 0xFF);

    bus.write8(0xFF40, 0x91).expect("blocked io write ignored");
    assert_eq!(bus.read_dma_source(0xFF40).expect("io raw read"), 0x00);

    bus.write8(0xFF80, 0x5A).expect("hram write allowed");
    assert_eq!(bus.read8(0xFF80).expect("hram read allowed"), 0x5A);
}

#[test]
fn dma_completion_clears_active_state_and_unblocks_cpu_access() {
    let cartridge = Cartridge::from_rom_bytes(test_rom()).expect("valid test rom");
    let mut bus = Bus::new(cartridge);
    let mut dma = OamDma::new();

    dma.start(0xC0, &mut bus).expect("start dma");
    assert!(dma.is_active());

    dma.tick(640, &mut bus).expect("finish dma");
    assert!(!dma.is_active());

    bus.write8(0xC000, 0x55).expect("unblocked write");
    assert_eq!(bus.read8(0xC000).expect("unblocked read"), 0x55);
}
