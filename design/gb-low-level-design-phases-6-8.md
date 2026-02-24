# Original Game Boy Low-Level Design (Phases 6-8)

This document expands phases 6-8 from the high-level plan into implementation-ready low-level design.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-high-level-design.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-1-3.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-4-5.md`

## 1. Global Engineering Constraints (Applies to Phases 6-8)

- Production code must not use `unwrap()`/`expect()`.
- All fallible operations return typed `Result` errors.
- Determinism remains mandatory across repeated runs with same ROM and inputs.
- Every component and task requires comprehensive tests (unit + edge + integration + regression as applicable).

## 2. Shared Integration Conventions

## 2.1 IO Ownership and Routing

- `Bus` remains the single owner of memory-mapped IO state.
- DMA, joypad, serial, boot ROM latch, and cartridge mapper controls are accessed through explicit module interfaces.
- No subsystem may mutate another subsystem state except via owned APIs.

## 2.2 Save and Persistent Data Boundary

- `gb-core` exposes save payload bytes through typed interfaces.
- Frontend or headless runner is responsible for file IO and storage paths.
- Save persistence behavior must be deterministic and testable without OS filesystem dependencies.

## 2.3 Deterministic Runner Contract

- Headless runner executes fixed step/input scripts and can emit machine-readable summaries.
- Compatibility validation compares expected pass/fail outcomes and optional frame/audio hashes.

## 3. Phase 6 Low-Level Design: DMA, Joypad, Serial

## 3.1 Objectives

1. Implement OAM DMA transfer behavior.
2. Implement joypad input matrix and interrupt behavior.
3. Implement serial transfer baseline behavior.
4. Add subsystem interaction tests with CPU/interrupts/bus.

## 3.2 File-Level Implementation Plan

- `crates/gb-core/src/dma/mod.rs`
- `crates/gb-core/src/joypad/mod.rs`
- `crates/gb-core/src/serial/mod.rs`
- `crates/gb-core/src/memory/io.rs` (extend mapped register handling)
- `crates/gb-core/src/bus/mod.rs` (DMA and IO interaction wiring)
- `crates/gb-core/tests/dma.rs`
- `crates/gb-core/tests/joypad.rs` (new)
- `crates/gb-core/tests/serial.rs` (new)
- `crates/gb-core/tests/integration_io.rs` (new)

## 3.3 OAM DMA Design

Registers:

- `FF46` DMA source high byte.

Behavior:

- Writing `N` to `FF46` triggers copy from `N << 8` to `OAM` (`0xFE00..=0xFE9F`).
- Transfer length is exactly 160 bytes.
- Transfer consumes modeled cycles and applies CPU memory access restrictions during active DMA window.

State:

```rust
pub struct OamDma {
    active: bool,
    source_base: u16,
    index: u16,
    cycles_remaining: Cycles,
}
```

API:

```rust
impl OamDma {
    pub fn start(&mut self, source_high: u8) -> Result<(), DmaError>;
    pub fn tick(&mut self, cycles: Cycles, bus: &mut Bus) -> Result<(), DmaError>;
    pub fn is_active(&self) -> bool;
}
```

Policy for phase 6:

- Use deterministic byte-copy model consistent with selected cycle granularity.
- Document exact CPU read/write restrictions while DMA is active and enforce in bus access paths.

## 3.4 Joypad Design

Register:

- `P1/JOYP` (`FF00`) with select lines and active-low button bits.

Input groups:

- D-pad: Right, Left, Up, Down.
- Buttons: A, B, Select, Start.

State:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbButton {
    Right,
    Left,
    Up,
    Down,
    A,
    B,
    Select,
    Start,
}

pub struct Joypad {
    select_dpad: bool,
    select_buttons: bool,
    pressed_mask: u8,
}
```

API:

```rust
impl Joypad {
    pub fn press(&mut self, button: GbButton, interrupts: &mut InterruptController);
    pub fn release(&mut self, button: GbButton);
    pub fn read_p1(&self) -> u8;
    pub fn write_p1(&mut self, value: u8);
}
```

Interrupt behavior:

- Request joypad interrupt on high-to-low transition in selected input lines.
- Keep transition policy explicit and tested.

## 3.5 Serial Baseline Design

Registers:

- `SB` (`FF01`) serial transfer data.
- `SC` (`FF02`) control/start bits.

Phase-6 baseline:

- Internal-clock transfer model only.
- Transfer completes after modeled bit-time budget.
- On completion, set received byte using deterministic stub policy and request serial interrupt.
- This baseline must be upgraded in phase 10 to a fully specified supported-serial model with regression coverage.

State:

```rust
pub struct Serial {
    sb: u8,
    sc: u8,
    transfer_active: bool,
    cycles_until_complete: Cycles,
}
```

API:

```rust
impl Serial {
    pub fn read_sb(&self) -> u8;
    pub fn write_sb(&mut self, v: u8);
    pub fn read_sc(&self) -> u8;
    pub fn write_sc(&mut self, v: u8);
    pub fn tick(&mut self, cycles: Cycles, interrupts: &mut InterruptController);
}
```

## 3.6 Phase 6 Integration Points

- `GameBoy::step()` order includes:
  1. CPU step
  2. timers/interrupt updates
  3. DMA tick
  4. serial tick
  5. ppu/apu ticks (already integrated in earlier phases)
- Bus IO reads/writes map `FF00`, `FF01`, `FF02`, `FF46` to modules.
- DMA restrictions are observable by CPU memory access during active transfer.

## 3.7 Phase 6 Test Matrix (Comprehensive)

`dma.rs` minimum suites:

- DMA trigger writes start transfer with correct source page.
- Exactly 160 bytes copied to OAM with correct ordering.
- CPU memory restrictions during active DMA.
- DMA completion clears active state.

`joypad.rs` minimum suites:

- P1 select line behavior for dpad vs buttons.
- Active-low bit mapping for presses/releases.
- Joypad interrupt transition behavior.

`serial.rs` minimum suites:

- SC start bit begins transfer only for supported mode.
- Transfer completion timing and interrupt request.
- Deterministic received-byte policy.

Integration suites:

- Concurrent DMA plus CPU stepping remains deterministic.
- Joypad and serial interrupts are serviced in correct priority order relative to other pending interrupts.
- IO reads/writes reflect current subsystem states.

## 3.8 Phase 6 AI Task Slices

1. Implement joypad register behavior and button state API + tests.
2. Implement serial baseline transfer state machine + tests.
3. Implement OAM DMA state machine + bus restrictions + tests.
4. Integrate IO mapping for `FF00/FF01/FF02/FF46` + tests.
5. Add cross-subsystem integration tests for DMA/joypad/serial interrupts.

Each slice must include comprehensive tests before merge.

## 4. Phase 7 Low-Level Design: Advanced Cartridge Support

## 4.1 Objectives

1. Add `MBC2`, `MBC3`, and `MBC5` mapper support.
2. Add battery-backed save persistence interfaces.
3. Add `MBC3` RTC support.
4. Add mapper and persistence regression tests.

## 4.2 File-Level Implementation Plan

- `crates/gb-core/src/cartridge/mod.rs` (extend mapper selection)
- `crates/gb-core/src/cartridge/header.rs` (extend validation metadata)
- `crates/gb-core/src/cartridge/mbc/mbc2.rs`
- `crates/gb-core/src/cartridge/mbc/mbc3.rs`
- `crates/gb-core/src/cartridge/mbc/mbc5.rs`
- `crates/gb-core/src/cartridge/rtc.rs`
- `crates/gb-core/src/cartridge/save.rs` (new persistence abstraction)
- `crates/gb-core/tests/mbc.rs` (extend)
- `crates/gb-core/tests/cartridge_persistence.rs` (new)
- `crates/gb-core/tests/rtc.rs` (new)

## 4.3 Mapper Trait Extensions

Existing mapper interfaces should support persistence hooks:

```rust
pub trait Mapper {
    fn read_rom(&self, addr: u16) -> Result<u8, CartridgeError>;
    fn write_rom(&mut self, addr: u16, val: u8) -> Result<(), CartridgeError>;
    fn read_ram(&self, addr: u16) -> Result<u8, CartridgeError>;
    fn write_ram(&mut self, addr: u16, val: u8) -> Result<(), CartridgeError>;

    fn save_ram(&self) -> Option<Vec<u8>>;
    fn load_ram(&mut self, data: &[u8]) -> Result<(), CartridgeError>;

    fn save_rtc(&self) -> Option<RtcState>;
    fn load_rtc(&mut self, state: &RtcState) -> Result<(), CartridgeError>;
}
```

## 4.4 MBC2 Behavior

Required behavior:

- Built-in 512x4-bit RAM semantics.
- RAM enable write uses address bit conditions.
- ROM bank switch via designated control addresses.
- Upper nibble behavior for RAM reads/writes follows documented policy.

## 4.5 MBC3 Behavior

Required behavior:

- ROM bank selection.
- RAM bank selection and RTC register selection.
- Latch clock command sequence.
- RTC registers: seconds, minutes, hours, day low, day high/control.
- Halt/carry/day overflow behaviors at modeled granularity.

## 4.6 MBC5 Behavior

Required behavior:

- 9-bit ROM bank selection (low 8 + high 1).
- RAM bank selection for supported variants.
- RAM enable gate behavior.

## 4.7 Save Persistence Design

Core abstraction:

```rust
pub struct CartridgeSave {
    pub ram: Option<Vec<u8>>,
    pub rtc: Option<RtcState>,
}

impl Cartridge {
    pub fn export_save(&self) -> CartridgeSave;
    pub fn import_save(&mut self, save: &CartridgeSave) -> Result<(), CartridgeError>;
}
```

- `gb-core` does not write files.
- Frontends/headless runner serialize `CartridgeSave` externally.

## 4.8 RTC Clock Source Policy

Phase-7 deterministic policy:

- RTC advances by explicit provided elapsed seconds, not direct system clock reads.
- Integration layer may provide wall-clock delta, but tests inject fixed deltas.

API:

```rust
impl Cartridge {
    pub fn tick_rtc(&mut self, elapsed_seconds: u64) -> Result<(), CartridgeError>;
}
```

## 4.9 Phase 7 Test Matrix (Comprehensive)

`mbc.rs` minimum suites:

- MBC2 bank switching and 4-bit RAM behavior.
- MBC3 ROM/RAM bank switching and RTC register selection.
- MBC5 9-bit ROM bank addressing and RAM bank switching.
- RAM enable gates across all supported mapper types.

`cartridge_persistence.rs` minimum suites:

- Export/import round-trip for RAM-only cartridges.
- Export/import round-trip for RTC-enabled cartridges.
- Reject malformed or size-mismatched save payloads with typed errors.

`rtc.rs` minimum suites:

- Latch behavior correctness.
- Halt/resume and overflow carry behavior.
- Deterministic progression with injected elapsed time.

Integration suites:

- CPU-visible ROM/RAM data changes correctly after mapper writes.
- Save data survives emulator restart through import/export path.

## 4.10 Phase 7 AI Task Slices

1. Implement persistence abstractions and cartridge import/export hooks + tests.
2. Implement MBC2 + tests.
3. Implement MBC3 core banking + tests.
4. Implement MBC3 RTC + latch/halt logic + tests.
5. Implement MBC5 + tests.
6. Add cross-mapper regression and persistence integration tests.

Each slice must include comprehensive tests before merge.

## 5. Phase 8 Low-Level Design: Boot ROM and Integration

## 5.1 Objectives

1. Add optional boot ROM execution flow.
2. Integrate all subsystems through `GameBoy` facade.
3. Add headless ROM runner for automated validation.
4. Validate with compatibility ROM suites.

## 5.2 File-Level Implementation Plan

- `crates/gb-core/src/boot/mod.rs`
- `crates/gb-core/src/gameboy.rs` (full subsystem ownership and step loop finalization)
- `crates/gb-core/src/config.rs` (boot ROM, compatibility options)
- `crates/gb-core/src/bus/memory_map.rs` (boot ROM overlay behavior)
- `crates/gb-core/tests/boot.rs` (new)
- `crates/gb-core/tests/integration_roms.rs` (extend)
- `crates/gb-headless/src/main.rs`
- `crates/gb-headless/src/runner.rs` (new)
- `crates/gb-headless/src/report.rs` (new)

## 5.3 Boot ROM Mapping Design

Behavior:

- When enabled, addresses `0x0000..=0x00FF` are served from boot ROM until unmap event.
- Write to boot control register (typically `FF50`) disables boot ROM mapping permanently until reset.
- If boot ROM is disabled by config, emulator initializes to documented post-boot register state.

State:

```rust
pub struct BootRom {
    enabled: bool,
    data: [u8; 256],
}

pub struct BootConfig {
    pub use_boot_rom: bool,
    pub force_post_boot_state: bool,
}
```

## 5.4 GameBoy Facade Finalization

`GameBoy` owns and advances:

- CPU
- Bus and memory regions
- PPU
- APU
- Timers
- Interrupt controller
- DMA
- Joypad
- Serial
- Cartridge (including RTC tick path)
- Boot ROM state

Core API:

```rust
impl GameBoy {
    pub fn new(config: GbConfig, rom: Vec<u8>) -> Result<Self, GbError>;
    pub fn step(&mut self) -> Result<Cycles, GbError>;
    pub fn run_cycles(&mut self, budget: Cycles) -> Result<Cycles, GbError>;
    pub fn set_button(&mut self, button: GbButton, pressed: bool) -> Result<(), GbError>;
    pub fn take_frame(&mut self) -> Option<[u8; 160 * 144]>;
    pub fn take_audio_samples(&mut self) -> Vec<i16>;
    pub fn export_save(&self) -> Result<CartridgeSave, GbError>;
    pub fn import_save(&mut self, save: &CartridgeSave) -> Result<(), GbError>;
}
```

Step ordering policy:

- Keep the subsystem update order explicit and stable in code and docs.
- Any future ordering changes require compatibility and regression test updates.

## 5.5 Headless Runner Design

Runner responsibilities:

- Load ROM and optional boot ROM.
- Load optional save payload.
- Execute for configured step/frame budget.
- Inject scripted inputs at deterministic cycle/frame markers.
- Emit structured report for CI.

CLI contract (initial):

- Inputs:
  - `--rom <path>`
  - `--boot-rom <path>` (optional)
  - `--cycles <n>` or `--frames <n>`
  - `--input-script <path>` (optional)
  - `--save-in <path>` / `--save-out <path>` (optional)
- Outputs:
  - exit code (success/failure)
  - summary report (JSON)
  - optional frame/audio hash metrics

## 5.6 Compatibility Validation Framework

`integration_roms.rs` and headless suites should support:

- Test ROM manifests with expected outcomes.
- Per-ROM timeout budgets.
- Result categories: pass, fail, timeout, unsupported-feature.
- Automatic regression capture for newly fixed issues.

Data format suggestion:

```toml
[[rom]]
name = "instr_timing"
path = "testdata/roms/blargg/instr_timing.gb"
max_cycles = 50000000
expected = "pass"
```

## 5.7 Phase 8 Test Matrix (Comprehensive)

`boot.rs` minimum suites:

- Boot ROM overlay active at reset when configured.
- `FF50` unmap behavior and permanence until reset.
- Post-boot forced state path correctness when boot ROM disabled.

`integration_roms.rs` minimum suites:

- GameBoy facade runs deterministic cycles with stable hashes.
- Input scripts trigger expected joypad behavior.
- Save export/import works end-to-end with cartridge mappers.

`gb-headless` tests:

- CLI argument validation and error paths.
- Report generation format correctness.
- Deterministic result for identical ROM/input scripts.

Compatibility suites:

- Add initial known ROM set and expected results.
- Ensure failures emit actionable diagnostics.

## 5.8 Phase 8 AI Task Slices

1. Implement boot ROM module and bus overlay/unmap behavior + tests.
2. Finalize `GameBoy` facade ownership and stable step ordering + tests.
3. Implement save import/export integration at facade level + tests.
4. Implement headless runner core (ROM load, run loop, report output) + tests.
5. Add compatibility manifest loader and ROM execution harness + tests.
6. Add deterministic integration regressions for boot, runner, and save workflows.

Each slice must include comprehensive tests before merge.

## 6. Cross-Phase Definition of Done

For every task in phases 6-8:

1. Implementation is complete and follows rust standards.
2. Comprehensive tests are included for normal, edge, and failure behavior.
3. `cargo fmt --check` passes.
4. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic` passes.
5. `cargo test --all-targets --all-features` passes.
6. Any bug fix includes a regression test in the same change.

## 7. Suggested Execution Order

1. Phase 6 slices 1-5.
2. Phase 7 slices 1-6.
3. Phase 8 slices 1-6.

Do not begin phase 9 until compatibility harness and deterministic integration baselines are stable.

Before final release, phase 10 must close phase-6/7/8 deferred items through compatibility-driven regressions:

- Serial behavior parity for all supported modes.
- Remaining mapper/special-cartridge gaps found in commercial title testing.
- Boot flow and post-boot initialization edge behavior.
