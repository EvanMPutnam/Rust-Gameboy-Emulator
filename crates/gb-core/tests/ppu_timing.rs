use gb_core::bus::Memory;
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;
use gb_core::interrupts::Interrupt;
use gb_core::ppu::{LcdMode, TileDataAddressing};

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn ppu_mode_transitions_and_ly_progress() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");

    gb.run_cycles(80).expect("run mode2");
    assert_eq!(gb.ppu.mode(), LcdMode::Transfer);

    gb.run_cycles(172).expect("run mode3");
    assert_eq!(gb.ppu.mode(), LcdMode::HBlank);

    gb.run_cycles(204).expect("run mode0");
    assert_eq!(gb.ppu.mode(), LcdMode::OamScan);
    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 1);
}

#[test]
fn enters_vblank_and_requests_interrupt() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");

    gb.run_cycles(456 * 144).expect("run to vblank");

    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 144);
    assert_eq!(gb.ppu.mode(), LcdMode::VBlank);
    assert_ne!(
        gb.bus.read8(0xFF0F).expect("read if") & Interrupt::VBlank.bit(),
        0
    );
}

#[test]
fn stat_lyc_interrupt_requests_on_match() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.bus.write8(0xFF41, 0x40).expect("enable lyc irq");
    gb.bus.write8(0xFF45, 0x01).expect("set lyc");

    gb.run_cycles(456).expect("advance one line");

    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 1);
    assert_ne!(
        gb.bus.read8(0xFF0F).expect("read if") & Interrupt::LcdStat.bit(),
        0
    );
}

#[test]
fn bus_vram_oam_access_is_restricted_by_mode() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.bus.write8(0x8000, 0x12).expect("seed vram");
    gb.bus.write8(0xFE00, 0x12).expect("seed oam");

    gb.run_cycles(80).expect("enter mode3");
    assert_eq!(gb.ppu.mode(), LcdMode::Transfer);

    gb.bus.write8(0x8000, 0x34).expect("blocked vram write");
    gb.bus.write8(0xFE00, 0x34).expect("blocked oam write");
    assert_eq!(gb.bus.read8(0x8000).expect("blocked vram read"), 0xFF);
    assert_eq!(gb.bus.read8(0xFE00).expect("blocked oam read"), 0xFF);

    gb.run_cycles(172).expect("enter hblank");
    gb.bus.write8(0x8000, 0x56).expect("unblocked vram write");
    assert_eq!(gb.bus.read8(0x8000).expect("unblocked vram read"), 0x56);
}

#[test]
fn lcd_disable_resets_mode_and_unblocks_vram_oam() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.run_cycles(80).expect("enter mode3");
    assert_eq!(gb.ppu.mode(), LcdMode::Transfer);

    gb.bus.write8(0xFF40, 0x11).expect("disable lcd");
    gb.run_cycles(4).expect("advance state");

    assert_eq!(gb.ppu.mode(), LcdMode::HBlank);
    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 0);
    assert_eq!(gb.bus.read8(0xFF41).expect("read stat") & 0x03, 0);

    gb.bus
        .write8(0x8000, 0xAA)
        .expect("vram write while lcd off");
    gb.bus
        .write8(0xFE00, 0x55)
        .expect("oam write while lcd off");
    assert_eq!(gb.bus.read8(0x8000).expect("vram read while lcd off"), 0xAA);
    assert_eq!(gb.bus.read8(0xFE00).expect("oam read while lcd off"), 0x55);
}

#[test]
fn writing_ly_register_resets_to_zero() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.run_cycles(456).expect("advance one line");
    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 1);

    gb.bus.write8(0xFF44, 0x99).expect("write ly");
    assert_eq!(gb.bus.read8(0xFF44).expect("read ly"), 0);
}

#[test]
fn writing_stat_preserves_read_only_mode_bits() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.run_cycles(80).expect("enter mode3");
    let mode_bits_before = gb.bus.read8(0xFF41).expect("read stat") & 0x07;

    gb.bus.write8(0xFF41, 0x00).expect("write stat");
    let mode_bits_after = gb.bus.read8(0xFF41).expect("read stat") & 0x07;

    assert_eq!(mode_bits_after, mode_bits_before);
}

#[test]
fn ppu_diagnostics_capture_scanline_metadata() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.set_ppu_diagnostics_enabled(true);
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd");
    gb.run_cycles(456 * 144).expect("run frame");

    let diagnostics = gb
        .take_ppu_frame_diagnostics()
        .expect("frame diagnostics should exist");
    assert_eq!(diagnostics.scanlines.len(), 144);

    let first_line = diagnostics.scanlines[0];
    assert_eq!(first_line.ly, 0);
    assert_eq!(first_line.mode, LcdMode::Transfer);
    assert_eq!(first_line.bg_tile_map_base, 0x9800);
    assert_eq!(first_line.window_tile_map_base, 0x9800);
    assert_eq!(
        first_line.tile_data_addressing,
        TileDataAddressing::Unsigned8000
    );
}
