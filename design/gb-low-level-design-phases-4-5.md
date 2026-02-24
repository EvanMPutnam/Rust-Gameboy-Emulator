# Original Game Boy Low-Level Design (Phases 4-5)

This document expands phases 4-5 from the high-level plan into implementation-ready low-level design.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-high-level-design.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-1-3.md`

## 1. Global Engineering Constraints (Applies to Phases 4-5)

- Production code must not use `unwrap()`/`expect()`.
- All fallible operations return typed `Result` errors.
- Timing progression must be deterministic for identical ROM + input streams.
- Every component and task requires comprehensive tests (unit + edge + integration + regression as applicable).

## 2. Shared Timing and Data Conventions

## 2.1 Cycle Unit

- Continue using canonical `Cycles` from earlier phases.
- CPU, PPU, and APU consume identical cycle units from `GameBoy::step()`.
- PPU/APU must never read wall-clock time directly.

## 2.2 Frame and Audio Output Interfaces

```rust
pub const GB_SCREEN_WIDTH: usize = 160;
pub const GB_SCREEN_HEIGHT: usize = 144;

pub type Pixel = u8; // 0..=3 DMG grayscale index

pub trait VideoSink {
    fn submit_frame(&mut self, frame: &[Pixel; GB_SCREEN_WIDTH * GB_SCREEN_HEIGHT]);
}

pub trait AudioSink {
    fn submit_samples(&mut self, interleaved_stereo: &[i16]);
}
```

- Core remains backend-agnostic: sinks are implemented by headless/frontend crates.
- Pixel format remains logical grayscale in core; frontend handles final palette conversion.

## 2.3 Memory/Bus Access Policy for PPU/APU

- PPU and APU register reads/writes are memory mapped through bus/IO register facade.
- Access restrictions (VRAM/OAM during LCD modes) are implemented inside PPU-aware bus logic.
- Phase 4 introduces strict VRAM/OAM access timing behavior.

## 3. Phase 4 Low-Level Design: PPU Core

## 3.1 Objectives

1. Implement LCD mode timing state machine.
2. Implement background and window rendering.
3. Implement sprite rendering and priority.
4. Implement STAT and VBlank interrupt generation.
5. Add deterministic frame output and golden-frame tests.

## 3.2 File-Level Implementation Plan

- `crates/gb-core/src/ppu/mod.rs`
- `crates/gb-core/src/ppu/timing.rs`
- `crates/gb-core/src/ppu/registers.rs`
- `crates/gb-core/src/ppu/fetcher.rs`
- `crates/gb-core/src/ppu/renderer.rs`
- `crates/gb-core/src/ppu/pixel_fifo.rs` (new)
- `crates/gb-core/src/ppu/oam_scan.rs` (new)
- `crates/gb-core/tests/ppu_timing.rs`
- `crates/gb-core/tests/ppu_golden.rs`

## 3.3 PPU Registers and State

Memory-mapped registers:

- `LCDC` (`FF40`)
- `STAT` (`FF41`)
- `SCY` (`FF42`), `SCX` (`FF43`)
- `LY` (`FF44`), `LYC` (`FF45`)
- `DMA` (`FF46`) (DMA execution owned by DMA module; trigger integration here)
- `BGP` (`FF47`), `OBP0` (`FF48`), `OBP1` (`FF49`)
- `WY` (`FF4A`), `WX` (`FF4B`)

Core state:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LcdMode {
    HBlank = 0,
    VBlank = 1,
    OamScan = 2,
    Transfer = 3,
}

pub struct Ppu {
    regs: PpuRegs,
    mode: LcdMode,
    mode_cycles: Cycles,
    frame: [u8; 160 * 144],
    line_sprite_cache: [Sprite; 10],
    line_sprite_count: usize,
    frame_ready: bool,
}
```

Rules:

- `LY` increments on each scanline boundary.
- `LY=0..143` visible lines; `LY=144..153` VBlank.
- `STAT` mode bits and `LYC==LY` flag must be kept coherent each transition.

## 3.4 Timing State Machine

Per visible line target model:

- Mode 2 (OAM scan): 80 cycles
- Mode 3 (transfer): variable model, start with fixed baseline for correctness (documented) and refine later
- Mode 0 (HBlank): remaining cycles to line budget
- Full line budget: 456 cycles
- Frame budget: 154 lines

Initial phase-4 compromise (explicit):

- Use deterministic fixed mode-3 duration baseline to unblock rendering correctness.
- Keep transition API extensible to switch to dot-accurate fetch timing in compatibility hardening.
- Dot-accurate mode-3 timing is required before final completion and must be closed in phase 10 with compatibility regressions.

API:

```rust
impl Ppu {
    pub fn tick(
        &mut self,
        cycles: Cycles,
        vram: &[u8; 0x2000],
        oam: &[u8; 0xA0],
        interrupts: &mut InterruptController,
    ) -> Result<(), PpuError>;
}
```

## 3.5 Rendering Pipeline

Background/window path:

- Resolve tile map base from `LCDC`.
- Resolve tile data area and signed/unsigned tile addressing mode.
- Apply `SCX/SCY` for BG and `WX/WY` for window enable region.
- Produce 2bpp color index for each pixel.

Sprite path:

- During mode 2, collect max 10 sprites for current `LY`.
- Respect sprite size mode (8x8, 8x16).
- Respect X/Y flip and palette selection.
- Resolve priority rules against BG color index.

Palette mapping:

- Apply `BGP/OBP0/OBP1` mapping from logical color index to final DMG shade index (0..3).
- Store final shade index in frame buffer.

## 3.6 Bus Interaction Rules

Required behavior in phase 4:

- OAM inaccessible to CPU during modes 2 and 3.
- VRAM inaccessible to CPU during mode 3.
- Inaccessible reads return chosen constant/open-bus behavior consistently (document exact value).
- Writes during blocked windows are ignored or handled per selected policy, consistently documented.

## 3.7 Interrupt Generation

VBlank interrupt:

- Request VBlank interrupt on transition to `LY=144` (mode 1 start).

STAT interrupt sources:

- Mode 0 interrupt enable bit.
- Mode 1 interrupt enable bit.
- Mode 2 interrupt enable bit.
- LYC=LY coincidence interrupt enable bit.

Edge behavior policy:

- Trigger only on source transition from inactive to active.
- Keep this policy explicit in tests to avoid duplicate interrupts.

## 3.8 Integration Points

- `GameBoy::step()` after CPU/timer updates calls `ppu.tick(cycles, ...)`.
- PPU exposes `take_frame_ready()` returning optional completed frame.
- Headless/frontend consumes completed frame buffers without mutating core timing.

## 3.9 Phase 4 Test Matrix (Comprehensive)

`ppu_timing.rs` minimum suites:

- Mode transition schedule per line.
- LY progression across full frame.
- LYC coincidence flag and interrupt behavior.
- VBlank start timing and interrupt request.
- VRAM/OAM access restrictions by mode.

`ppu_golden.rs` minimum suites:

- BG-only render cases with scroll offsets.
- Window enable/position behavior.
- Sprite priority and transparency behavior.
- Palette mapping behavior.
- Golden frame hash or pixel array equality for representative scenes.

Integration tests:

- CPU writes to LCD registers change rendered output predictably.
- PPU interrupts surface through interrupt controller and CPU servicing.
- Deterministic frame output across repeated runs.

## 3.10 Phase 4 AI Task Slices

1. Implement PPU registers/state + mode transition scaffold + tests.
2. Implement LY/LYC/STAT/VBlank interrupt logic + tests.
3. Implement BG renderer + tests.
4. Implement window renderer + tests.
5. Implement sprite pipeline (OAM scan + blending/priority) + tests.
6. Implement bus access restrictions for VRAM/OAM by mode + tests.
7. Add golden-frame integration suites.

Each slice must include comprehensive tests before merge.

## 4. Phase 5 Low-Level Design: APU Core

## 4.1 Objectives

1. Implement frame sequencer and per-channel state machines.
2. Implement channel sample generation.
3. Implement mixer and sample output API.
4. Add deterministic audio tests.

## 4.2 File-Level Implementation Plan

- `crates/gb-core/src/apu/mod.rs`
- `crates/gb-core/src/apu/frame_sequencer.rs`
- `crates/gb-core/src/apu/square.rs`
- `crates/gb-core/src/apu/wave.rs`
- `crates/gb-core/src/apu/noise.rs`
- `crates/gb-core/src/apu/mixer.rs`
- `crates/gb-core/src/apu/registers.rs` (new)
- `crates/gb-core/tests/apu_channels.rs`
- `crates/gb-core/tests/apu_determinism.rs` (new)

## 4.3 DMG APU Channels and Registers

Channel 1 (square + sweep):

- `NR10`..`NR14`

Channel 2 (square):

- `NR21`..`NR24`

Channel 3 (wave):

- `NR30`..`NR34`, wave RAM `FF30..FF3F`

Channel 4 (noise):

- `NR41`..`NR44`

Global control:

- `NR50` (master volume)
- `NR51` (channel panning/routing)
- `NR52` (master enable + channel status)

Core state:

```rust
pub struct Apu {
    enabled: bool,
    frame_seq: FrameSequencer,
    ch1: SquareChannel,
    ch2: SquareChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,
    mixer: Mixer,
    sample_clock_accum: Cycles,
    sample_buffer: Vec<i16>,
}
```

## 4.4 Frame Sequencer Model

- 8-step frame sequencer at 512 Hz.
- Sequencer clocks:
  - Length counters.
  - Channel 1 sweep.
  - Volume envelopes.

API:

```rust
impl FrameSequencer {
    pub fn tick(&mut self, cycles: Cycles) -> FrameEvents;
}

pub struct FrameEvents {
    pub clock_length: bool,
    pub clock_sweep: bool,
    pub clock_envelope: bool,
}
```

## 4.5 Channel Generation Model

Square channels:

- Duty cycle pattern selection.
- Period timer from frequency registers.
- Envelope volume progression.
- Channel 1 frequency sweep unit.

Wave channel:

- Wave RAM nibble sampling.
- Output level shifts.
- DAC enable behavior.

Noise channel:

- LFSR-based pseudo-random generator.
- Width mode and clock divider behavior.

Trigger behavior:

- Writes with trigger bit set reinitialize channel state per register rules.
- Disabled DAC forces channel off.

## 4.6 Mixer and Sample Output

Mixing policy for phase 5:

- Mix four channel outputs into left/right using `NR51` routing.
- Apply `NR50` per-side volume scaling.
- Clamp and convert to `i16` sample format.

Sampling policy:

- Choose fixed output rate (example: 48_000 Hz).
- Use cycle accumulator to emit deterministic samples independent of host timing jitter.
- Keep interpolation strategy simple first (nearest/sample-hold), with later upgrade path documented.
- Phase 10 must validate remaining DMG APU edge behaviors (power/reset semantics and timing quirks) using deterministic regression fixtures.

API:

```rust
impl Apu {
    pub fn tick(&mut self, cycles: Cycles) -> Result<(), ApuError>;
    pub fn take_samples(&mut self) -> Vec<i16>;
}
```

## 4.7 Register Semantics and Power Control

- `NR52` master disable clears channel states and most writable audio registers per modeled policy.
- Reads from unused bits return documented masked values.
- Wave RAM access behavior during channel playback should be explicitly modeled and tested.

## 4.8 Integration Points

- `GameBoy::step()` invokes `apu.tick(cycles)` after CPU step.
- Audio samples are drained by headless/frontend loop via `take_samples()`.
- Timer/interrupt/PPU progression must remain unaffected by audio drain frequency.

## 4.9 Phase 5 Test Matrix (Comprehensive)

`apu_channels.rs` minimum suites:

- Register read/write semantics per channel.
- Trigger behavior and channel enable/disable transitions.
- Envelope, length counter, and sweep progression tests.
- Wave channel sample extraction and level shift tests.
- Noise LFSR progression tests.

`apu_determinism.rs` minimum suites:

- Fixed register-write script -> exact sample stream match.
- Same ROM/input run produces identical audio hash.
- NR52 enable/disable transitions maintain deterministic reset behavior.

Integration tests:

- APU sample generation under normal CPU stepping.
- Panning/volume (`NR50/NR51`) reflected in left/right output.
- No panic/error under long-run audio generation.

## 4.10 Phase 5 AI Task Slices

1. Implement APU register block + masked read/write behavior + tests.
2. Implement frame sequencer + length/envelope/sweep clocks + tests.
3. Implement square channels + tests.
4. Implement wave channel + tests.
5. Implement noise channel + tests.
6. Implement mixer + sample scheduler + tests.
7. Add deterministic integration/audio hash suites.

Each slice must include comprehensive tests before merge.

## 5. Cross-Phase Definition of Done

For every task in phases 4-5:

1. Implementation is complete and follows rust standards.
2. Comprehensive tests are included for normal, edge, and failure behavior.
3. `cargo fmt --check` passes.
4. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic` passes.
5. `cargo test --all-targets --all-features` passes.
6. Any bug fix includes a regression test in the same change.

## 6. Suggested Execution Order

1. Phase 4 slices 1-7.
2. Phase 5 slices 1-7.

Do not begin phase 6 until frame and audio determinism baselines are stable.
