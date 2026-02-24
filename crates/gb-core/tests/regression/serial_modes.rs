use gb_core::bus::Memory;
use gb_core::{GameBoy, GbConfig};

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn external_clock_mode_does_not_start_internal_transfer() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");

    gb.bus.write8(0xFF01, 0x42).expect("write sb");
    gb.bus
        .write8(0xFF02, 0x80)
        .expect("start without internal clock");
    gb.run_cycles(8_192).expect("run cycles");

    assert_eq!(gb.bus.read8(0xFF01).expect("read sb"), 0x42);
    assert_eq!(gb.bus.read8(0xFF02).expect("read sc") & 0x80, 0x80);
}
