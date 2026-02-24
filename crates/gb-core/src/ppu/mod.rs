#![allow(clippy::cast_possible_truncation)] // Pixel and tile math is bounded by GB screen/tile dimensions.
#![allow(clippy::cast_possible_wrap)] // Signed pixel offsets are intentional for sprite/window arithmetic.
#![allow(clippy::cast_sign_loss)] // Conversions occur after explicit range checks in rendering logic.
#![allow(clippy::needless_range_loop)] // Index-based loops map directly to hardware pixel coordinates.
#![allow(clippy::large_stack_arrays)] // Framebuffer is fixed-size DMG state and intentionally contiguous.

use crate::Cycles;
use crate::bus::Bus;
use crate::error::PpuError;
use crate::interrupts::{Interrupt, InterruptController};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;
const VISIBLE_LINES: u8 = 144;
const TOTAL_LINES: u8 = 154;
const CYCLES_PER_LINE: Cycles = 456;
const MODE2_CYCLES: Cycles = 80;
const MODE3_CYCLES: Cycles = 172;
const MODE0_CYCLES: Cycles = CYCLES_PER_LINE - MODE2_CYCLES - MODE3_CYCLES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LcdMode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Transfer = 3,
}

impl LcdMode {
    #[must_use]
    pub fn as_bits(self) -> u8 {
        match self {
            Self::HBlank => 0,
            Self::VBlank => 1,
            Self::OamScan => 2,
            Self::Transfer => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileDataAddressing {
    Unsigned8000,
    Signed8800,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpriteCompositionStats {
    pub sprites_on_line: u8,
    pub opaque_pixels_drawn: u16,
    pub priority_skips: u16,
    pub transparent_skips: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PpuScanlineDiagnostics {
    pub ly: u8,
    pub mode: LcdMode,
    pub lyc_match: bool,
    pub window_line_counter: u8,
    pub bg_tile_map_base: u16,
    pub window_tile_map_base: u16,
    pub tile_data_addressing: TileDataAddressing,
    pub sprite_stats: SpriteCompositionStats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PpuFrameDiagnostics {
    pub frame_number: u64,
    pub scanlines: Vec<PpuScanlineDiagnostics>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
struct SpriteCandidate {
    x: u8,
    y: u8,
    tile: u8,
    attr: u8,
    oam_index: u8,
}

#[derive(Default)]
struct PpuDiagnosticsState {
    enabled: bool,
    frame_number: u64,
    scanlines: Vec<PpuScanlineDiagnostics>,
    last_frame: Option<PpuFrameDiagnostics>,
}

impl PpuDiagnosticsState {
    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.scanlines.clear();
            self.last_frame = None;
        }
    }

    fn begin_frame(&mut self) {
        if self.enabled {
            self.scanlines.clear();
        }
    }

    fn record_scanline(&mut self, entry: PpuScanlineDiagnostics) {
        if self.enabled && self.scanlines.len() < SCREEN_HEIGHT {
            self.scanlines.push(entry);
        }
    }

    fn finish_frame(&mut self) {
        if !self.enabled {
            return;
        }

        self.last_frame = Some(PpuFrameDiagnostics {
            frame_number: self.frame_number,
            scanlines: self.scanlines.clone(),
        });
        self.frame_number = self.frame_number.wrapping_add(1);
        self.scanlines.clear();
    }

    fn take_last_frame(&mut self) -> Option<PpuFrameDiagnostics> {
        self.last_frame.take()
    }
}

#[derive(Serialize, Deserialize)]
pub struct Ppu {
    mode: LcdMode,
    mode_cycles: Cycles,
    #[serde(with = "BigArray")]
    frame: [u8; SCREEN_WIDTH * SCREEN_HEIGHT],
    frame_ready: bool,
    stat_irq_line: bool,
    window_line_counter: u8,
    lcd_enabled: bool,
    #[serde(skip)]
    diagnostics: PpuDiagnosticsState,
}

impl Default for Ppu {
    fn default() -> Self {
        Self {
            mode: LcdMode::OamScan,
            mode_cycles: 0,
            frame: [0; SCREEN_WIDTH * SCREEN_HEIGHT],
            frame_ready: false,
            stat_irq_line: false,
            window_line_counter: 0,
            lcd_enabled: false,
            diagnostics: PpuDiagnosticsState::default(),
        }
    }
}

impl Ppu {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn mode(&self) -> LcdMode {
        self.mode
    }

    pub fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics.set_enabled(enabled);
        if enabled {
            self.diagnostics.begin_frame();
        }
    }

    #[must_use]
    pub fn take_frame_diagnostics(&mut self) -> Option<PpuFrameDiagnostics> {
        self.diagnostics.take_last_frame()
    }

    pub fn tick(
        &mut self,
        cycles: Cycles,
        bus: &mut Bus,
        interrupts: &mut InterruptController,
    ) -> Result<(), PpuError> {
        let lcd_enabled_now = (bus.io[0x40] & 0x80) != 0;
        if !lcd_enabled_now {
            self.handle_lcd_disabled(bus, interrupts);
            return Ok(());
        }
        if !self.lcd_enabled {
            self.handle_lcd_enabled(bus, interrupts);
        }
        self.lcd_enabled = true;

        let mut remaining = cycles;
        while remaining > 0 {
            let boundary = match self.mode {
                LcdMode::OamScan => MODE2_CYCLES,
                LcdMode::Transfer => MODE3_CYCLES,
                LcdMode::HBlank => MODE0_CYCLES,
                LcdMode::VBlank => CYCLES_PER_LINE,
            };

            let until_boundary = boundary.saturating_sub(self.mode_cycles);
            let step = until_boundary.min(remaining);
            self.mode_cycles = self.mode_cycles.wrapping_add(step);
            remaining -= step;

            self.update_stat(bus, interrupts);

            if self.mode_cycles >= boundary {
                self.mode_cycles = 0;
                self.advance_mode(bus, interrupts);
            }
        }

        bus.set_ppu_access(
            self.mode == LcdMode::Transfer,
            matches!(self.mode, LcdMode::Transfer | LcdMode::OamScan),
        );

        Ok(())
    }

    #[must_use]
    pub fn take_frame(&mut self) -> Option<[u8; SCREEN_WIDTH * SCREEN_HEIGHT]> {
        if !self.frame_ready {
            return None;
        }

        self.frame_ready = false;
        Some(self.frame)
    }

    fn advance_mode(&mut self, bus: &mut Bus, interrupts: &mut InterruptController) {
        match self.mode {
            LcdMode::OamScan => {
                self.mode = LcdMode::Transfer;
            }
            LcdMode::Transfer => {
                self.render_scanline(bus);
                self.mode = LcdMode::HBlank;
            }
            LcdMode::HBlank => {
                let ly = bus.io[0x44].wrapping_add(1);
                bus.io[0x44] = ly;
                if ly >= VISIBLE_LINES {
                    self.mode = LcdMode::VBlank;
                    interrupts.request(Interrupt::VBlank);
                    self.frame_ready = true;
                    self.diagnostics.finish_frame();
                } else {
                    self.mode = LcdMode::OamScan;
                }
            }
            LcdMode::VBlank => {
                let next_ly = bus.io[0x44].wrapping_add(1);
                if next_ly >= TOTAL_LINES {
                    bus.io[0x44] = 0;
                    self.mode = LcdMode::OamScan;
                    self.window_line_counter = 0;
                    self.diagnostics.begin_frame();
                } else {
                    bus.io[0x44] = next_ly;
                }
            }
        }

        self.update_stat(bus, interrupts);
    }

    fn update_stat(&mut self, bus: &mut Bus, interrupts: &mut InterruptController) {
        let ly = bus.io[0x44];
        let lyc = bus.io[0x45];

        let mut stat = bus.io[0x41] & 0xF8;
        stat |= self.mode.as_bits();

        let coincidence = ly == lyc;
        if coincidence {
            stat |= 0x04;
        }

        let source_mode0 = self.lcd_enabled && self.mode == LcdMode::HBlank && (stat & 0x08) != 0;
        let source_mode1 = self.lcd_enabled && self.mode == LcdMode::VBlank && (stat & 0x10) != 0;
        let source_mode2 = self.lcd_enabled && self.mode == LcdMode::OamScan && (stat & 0x20) != 0;
        let source_lyc = self.lcd_enabled && coincidence && (stat & 0x40) != 0;
        let active = source_mode0 || source_mode1 || source_mode2 || source_lyc;

        if active && !self.stat_irq_line {
            interrupts.request(Interrupt::LcdStat);
        }

        self.stat_irq_line = active;
        bus.io[0x41] = stat;
    }

    fn render_scanline(&mut self, bus: &Bus) {
        let ly = usize::from(bus.io[0x44]);
        if ly >= SCREEN_HEIGHT {
            return;
        }

        let lcdc = bus.io[0x40];
        if (lcdc & 0x80) == 0 {
            let start = ly * SCREEN_WIDTH;
            let end = start + SCREEN_WIDTH;
            self.frame[start..end].fill(0);
            self.window_line_counter = 0;
            return;
        }

        let bg_tile_map_base = bg_tile_map_base(lcdc);
        let window_tile_map_base = window_tile_map_base(lcdc);
        let tile_data_addressing = tile_data_addressing_mode(lcdc);
        let window_line_counter = self.window_line_counter;

        let mut bg_color_ids = [0u8; SCREEN_WIDTH];
        for x in 0..SCREEN_WIDTH {
            bg_color_ids[x] = Self::bg_color_for_pixel(x as u8, ly as u8, bus);
        }

        if (lcdc & 0x20) != 0 && (lcdc & 0x01) != 0 && Self::window_visible_on_line(ly as u8, bus) {
            Self::apply_window_line(self.window_line_counter, bus, &mut bg_color_ids);
            self.window_line_counter = self.window_line_counter.wrapping_add(1);
        }

        let mut final_line = [0u8; SCREEN_WIDTH];
        for x in 0..SCREEN_WIDTH {
            final_line[x] = palette_map(bus.io[0x47], bg_color_ids[x]);
        }

        let sprite_stats = if (lcdc & 0x02) != 0 {
            Self::apply_sprite_line(ly as u8, bus, &bg_color_ids, &mut final_line)
        } else {
            SpriteCompositionStats::default()
        };

        let start = ly * SCREEN_WIDTH;
        let end = start + SCREEN_WIDTH;
        self.frame[start..end].copy_from_slice(&final_line);

        self.diagnostics.record_scanline(PpuScanlineDiagnostics {
            ly: ly as u8,
            mode: self.mode,
            lyc_match: bus.io[0x44] == bus.io[0x45],
            window_line_counter,
            bg_tile_map_base,
            window_tile_map_base,
            tile_data_addressing,
            sprite_stats,
        });
    }

    fn bg_color_for_pixel(x: u8, ly: u8, bus: &Bus) -> u8 {
        let lcdc = bus.io[0x40];
        if (lcdc & 0x01) == 0 {
            return 0;
        }

        let scx = bus.io[0x43];
        let scy = bus.io[0x42];
        let bg_x = x.wrapping_add(scx);
        let bg_y = ly.wrapping_add(scy);
        Self::tilemap_color(lcdc, bg_x, bg_y, false, bus)
    }

    fn window_visible_on_line(ly: u8, bus: &Bus) -> bool {
        let wx = bus.io[0x4B];
        let wy = bus.io[0x4A];
        ly >= wy && wx < 167
    }

    fn apply_window_line(window_line: u8, bus: &Bus, line: &mut [u8; SCREEN_WIDTH]) {
        let wx = bus.io[0x4B];
        if wx >= 167 {
            return;
        }
        let window_left = i16::from(wx) - 7;
        let lcdc = bus.io[0x40];

        for (x, pixel) in line.iter_mut().enumerate() {
            let x_i16 = x as i16;
            if x_i16 < window_left {
                continue;
            }

            let win_x = (x_i16 - window_left) as u8;
            *pixel = Self::tilemap_color(lcdc, win_x, window_line, true, bus);
        }
    }

    fn tilemap_color(lcdc: u8, x: u8, y: u8, window: bool, bus: &Bus) -> u8 {
        let map_base = if window {
            window_tile_map_base(lcdc)
        } else {
            bg_tile_map_base(lcdc)
        };

        let tile_y = usize::from(y / 8);
        let tile_x = usize::from(x / 8);
        let map_index = tile_y * 32 + tile_x;
        let tile_id = bus.vram[(map_base - 0x8000 + map_index as u16) as usize];

        let tile_addr = tile_data_address(tile_data_addressing_mode(lcdc), tile_id);

        let row = usize::from(y % 8);
        let lo = bus.vram[(tile_addr - 0x8000 + (row as u16 * 2)) as usize];
        let hi = bus.vram[(tile_addr - 0x8000 + (row as u16 * 2 + 1)) as usize];
        let bit = 7 - (x % 8);

        ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1)
    }

    fn apply_sprite_line(
        ly: u8,
        bus: &Bus,
        bg_color_ids: &[u8; SCREEN_WIDTH],
        out_line: &mut [u8; SCREEN_WIDTH],
    ) -> SpriteCompositionStats {
        let lcdc = bus.io[0x40];
        let sprite_height: u8 = if (lcdc & 0x04) != 0 { 16 } else { 8 };
        let sprites = collect_sprites_for_line(ly, sprite_height, bus);
        let mut stats = SpriteCompositionStats {
            sprites_on_line: u8::try_from(sprites.len()).unwrap_or(u8::MAX),
            ..SpriteCompositionStats::default()
        };

        for x in 0..SCREEN_WIDTH {
            for sprite in &sprites {
                let sprite_x = sprite.x.wrapping_sub(8);
                let sprite_y = sprite.y.wrapping_sub(16);

                let sx = x as i16 - i16::from(sprite_x);
                if !(0..8).contains(&sx) {
                    continue;
                }

                let sy = i16::from(ly) - i16::from(sprite_y);
                if sy < 0 || sy >= i16::from(sprite_height) {
                    continue;
                }

                let x_in_sprite = if (sprite.attr & 0x20) != 0 {
                    7 - sx as u8
                } else {
                    sx as u8
                };

                let mut y_in_sprite = sy as u8;
                if (sprite.attr & 0x40) != 0 {
                    y_in_sprite = sprite_height - 1 - y_in_sprite;
                }

                let tile = if sprite_height == 16 {
                    sprite.tile & 0xFE
                } else {
                    sprite.tile
                };

                let tile_row = if sprite_height == 16 && y_in_sprite >= 8 {
                    y_in_sprite - 8
                } else {
                    y_in_sprite
                };
                let tile_index = if sprite_height == 16 && y_in_sprite >= 8 {
                    tile.wrapping_add(1)
                } else {
                    tile
                };

                let base = 0x8000u16.wrapping_add(u16::from(tile_index) * 16);
                let lo = bus.vram[(base - 0x8000 + u16::from(tile_row) * 2) as usize];
                let hi = bus.vram[(base - 0x8000 + u16::from(tile_row) * 2 + 1) as usize];
                let bit = 7 - x_in_sprite;
                let color = ((hi >> bit) & 1) << 1 | ((lo >> bit) & 1);
                if color == 0 {
                    stats.transparent_skips = stats.transparent_skips.wrapping_add(1);
                    continue;
                }

                if (sprite.attr & 0x80) != 0 && bg_color_ids[x] != 0 {
                    stats.priority_skips = stats.priority_skips.wrapping_add(1);
                    continue;
                }

                let palette = if (sprite.attr & 0x10) != 0 {
                    bus.io[0x49]
                } else {
                    bus.io[0x48]
                };
                out_line[x] = palette_map(palette, color);
                stats.opaque_pixels_drawn = stats.opaque_pixels_drawn.wrapping_add(1);
                break;
            }
        }

        stats
    }

    fn handle_lcd_disabled(&mut self, bus: &mut Bus, interrupts: &mut InterruptController) {
        if self.lcd_enabled {
            self.frame.fill(0);
            self.frame_ready = true;
        }

        self.lcd_enabled = false;
        self.mode = LcdMode::HBlank;
        self.mode_cycles = 0;
        self.window_line_counter = 0;
        self.stat_irq_line = false;
        bus.io[0x44] = 0;
        self.diagnostics.begin_frame();

        self.update_stat(bus, interrupts);
        bus.set_ppu_access(false, false);
    }

    fn handle_lcd_enabled(&mut self, bus: &mut Bus, interrupts: &mut InterruptController) {
        self.mode = LcdMode::OamScan;
        self.mode_cycles = 0;
        self.window_line_counter = 0;
        self.stat_irq_line = false;
        bus.io[0x44] = 0;
        self.diagnostics.begin_frame();
        self.update_stat(bus, interrupts);
    }
}

fn collect_sprites_for_line(ly: u8, height: u8, bus: &Bus) -> Vec<SpriteCandidate> {
    let mut sprites = Vec::with_capacity(10);

    for index in 0u8..40 {
        let base = usize::from(index) * 4;
        let y = bus.oam[base];
        let x = bus.oam[base + 1];
        let tile = bus.oam[base + 2];
        let attr = bus.oam[base + 3];

        let sprite_y = y.wrapping_sub(16);
        let line = ly.wrapping_sub(sprite_y);
        if line < height {
            sprites.push(SpriteCandidate {
                x,
                y,
                tile,
                attr,
                oam_index: index,
            });
            if sprites.len() == 10 {
                break;
            }
        }
    }

    sprites.sort_by_key(|sprite| (sprite.x, sprite.oam_index));
    sprites
}

fn palette_map(palette: u8, color: u8) -> u8 {
    (palette >> (color * 2)) & 0x03
}

fn bg_tile_map_base(lcdc: u8) -> u16 {
    if (lcdc & 0x08) != 0 { 0x9C00 } else { 0x9800 }
}

fn window_tile_map_base(lcdc: u8) -> u16 {
    if (lcdc & 0x40) != 0 { 0x9C00 } else { 0x9800 }
}

fn tile_data_addressing_mode(lcdc: u8) -> TileDataAddressing {
    if (lcdc & 0x10) != 0 {
        TileDataAddressing::Unsigned8000
    } else {
        TileDataAddressing::Signed8800
    }
}

fn tile_data_address(addressing: TileDataAddressing, tile_id: u8) -> u16 {
    match addressing {
        TileDataAddressing::Unsigned8000 => 0x8000u16.wrapping_add(u16::from(tile_id) * 16),
        TileDataAddressing::Signed8800 => {
            let signed = i32::from(i8::from_ne_bytes([tile_id]));
            (0x9000i32 + signed * 16) as u16
        }
    }
}
