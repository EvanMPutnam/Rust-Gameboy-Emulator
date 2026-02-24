use gb_core::bus::Memory;
use gb_core::config::GbConfig;
use gb_core::gameboy::GameBoy;

fn test_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom[0x0100] = 0x00;
    rom
}

#[test]
fn produces_deterministic_background_frame() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x91).expect("enable lcd bg");
    gb.bus.write8(0xFF47, 0xE4).expect("set bg palette");

    for row in 0..8 {
        gb.bus
            .write8(0x8000 + row * 2, 0xFF)
            .expect("tile lo write");
        gb.bus
            .write8(0x8000 + row * 2 + 1, 0x00)
            .expect("tile hi write");
    }

    gb.run_cycles(456 * 144).expect("run frame");
    let frame = gb.take_frame().expect("frame should be ready");

    assert_eq!(frame[0], 1);
    assert_eq!(frame[159], 1);
    assert_eq!(frame[160], 1);
}

#[test]
fn window_and_sprite_affect_final_pixels() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus
        .write8(0xFF40, 0xF3)
        .expect("lcd on bg+window+obj enabled");
    gb.bus.write8(0xFF47, 0xE4).expect("bg palette");
    gb.bus.write8(0xFF48, 0xE4).expect("obj palette");
    gb.bus.write8(0xFF4A, 0x00).expect("wy");
    gb.bus.write8(0xFF4B, 0x07).expect("wx starts at x0");

    for row in 0..8 {
        gb.bus
            .write8(0x8000 + row * 2, 0xFF)
            .expect("tile0 lo write");
        gb.bus
            .write8(0x8000 + row * 2 + 1, 0x00)
            .expect("tile0 hi write");
    }

    for row in 0..8 {
        gb.bus
            .write8(0x8010 + row * 2, 0xFF)
            .expect("tile1 lo write");
        gb.bus
            .write8(0x8010 + row * 2 + 1, 0xFF)
            .expect("tile1 hi write");
    }

    gb.bus.write8(0x9C00, 0x01).expect("window map tile1");

    for row in 0..8 {
        gb.bus
            .write8(0x8020 + row * 2, 0x00)
            .expect("sprite tile lo");
        gb.bus
            .write8(0x8020 + row * 2 + 1, 0xFF)
            .expect("sprite tile hi");
    }

    gb.bus.write8(0xFE00, 16).expect("sprite y");
    gb.bus.write8(0xFE01, 8).expect("sprite x");
    gb.bus.write8(0xFE02, 0x02).expect("sprite tile");
    gb.bus.write8(0xFE03, 0x00).expect("sprite attr");

    gb.run_cycles(456 * 144).expect("run frame");
    let frame = gb.take_frame().expect("frame should be ready");

    assert_eq!(frame[0], 2);
}

#[test]
fn window_respects_wx_offset_when_less_than_seven() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus
        .write8(0xFF40, 0xF1)
        .expect("lcd on bg+window enabled");
    gb.bus.write8(0xFF47, 0xE4).expect("bg palette");
    gb.bus.write8(0xFF4A, 0x00).expect("wy");
    gb.bus.write8(0xFF4B, 0x00).expect("wx");

    for row in 0..8 {
        gb.bus
            .write8(0x8010 + row * 2, 0x01)
            .expect("window tile lo");
        gb.bus
            .write8(0x8010 + row * 2 + 1, 0x00)
            .expect("window tile hi");
    }

    gb.bus.write8(0x9C00, 0x01).expect("window tile id");

    gb.run_cycles(456 * 144).expect("run frame");
    let frame = gb.take_frame().expect("frame should be ready");

    assert_eq!(frame[0], 1);
}

#[test]
fn sprite_priority_uses_raw_bg_color_not_palette_mapped_shade() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus.write8(0xFF40, 0x93).expect("lcd on bg+obj enabled");
    gb.bus
        .write8(0xFF47, 0x03)
        .expect("bg palette maps color0 to shade3");
    gb.bus.write8(0xFF48, 0xE4).expect("obj palette");

    for row in 0..8 {
        gb.bus
            .write8(0x8010 + row * 2, 0x00)
            .expect("sprite tile lo");
        gb.bus
            .write8(0x8010 + row * 2 + 1, 0x80)
            .expect("sprite tile hi");
    }

    gb.bus.write8(0xFE00, 16).expect("sprite y");
    gb.bus.write8(0xFE01, 8).expect("sprite x");
    gb.bus.write8(0xFE02, 0x01).expect("sprite tile");
    gb.bus.write8(0xFE03, 0x80).expect("sprite priority attr");

    gb.run_cycles(456 * 144).expect("run frame");
    let frame = gb.take_frame().expect("frame should be ready");

    assert_eq!(frame[0], 2);
}

#[test]
fn window_line_counter_only_advances_on_lines_where_window_is_drawn() {
    let mut gb = GameBoy::new(GbConfig::default(), test_rom()).expect("init gb");
    gb.bus
        .write8(0xFF40, 0xF1)
        .expect("lcd on bg+window enabled");
    gb.bus.write8(0xFF47, 0xE4).expect("bg palette");
    gb.bus.write8(0xFF4A, 0x00).expect("wy");
    gb.bus.write8(0xFF4B, 0x07).expect("wx");

    gb.bus.write8(0x9C00, 0x01).expect("window map tile1");

    gb.bus.write8(0x8010, 0xFF).expect("row0 lo");
    gb.bus.write8(0x8011, 0x00).expect("row0 hi");
    gb.bus.write8(0x8012, 0x00).expect("row1 lo");
    gb.bus.write8(0x8013, 0xFF).expect("row1 hi");
    gb.bus.write8(0x8014, 0xFF).expect("row2 lo");
    gb.bus.write8(0x8015, 0xFF).expect("row2 hi");

    gb.run_cycles(456).expect("run line 0");

    gb.bus
        .write8(0xFF40, 0xD1)
        .expect("disable window for line 1");
    gb.run_cycles(456).expect("run line 1");

    gb.bus
        .write8(0xFF40, 0xF1)
        .expect("re-enable window for line 2");
    gb.run_cycles(456 * 142).expect("finish visible frame");

    let frame = gb.take_frame().expect("frame should be ready");
    let line2_x0 = frame[2 * 160];
    assert_eq!(line2_x0, 2);
}
