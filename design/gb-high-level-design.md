# Original Game Boy (DMG) Emulator High-Level Design

This document defines the high-level architecture for a Rust emulator targeting the original Game Boy (DMG-01), with the goal of running commercial GB games at high compatibility and deterministic behavior.

## 1. Goals

- Emulate original Game Boy (DMG) hardware accurately enough to run real cartridges.
- Prioritize correctness and determinism before optimization.
- Keep core emulation independent from platform UI/audio/input backends.
- Split work into small, reviewable tasks that AI can complete safely.
- Build comprehensive tests for every component from day one.

## 2. Scope and Non-Goals

In scope:

- LR35902 CPU emulation (GB instruction set).
- Full DMG memory map and bus behavior.
- PPU pipeline and LCD timing (modes 0-3 + VBlank).
- APU channels and mixer behavior.
- Timers, interrupts, joypad input, serial interface.
- OAM DMA transfer behavior.
- Cartridge support with mapper families needed by GB titles.
- Save persistence and optional RTC for compatible cartridges.
- Deterministic scheduler and test harness.

Out of scope for first stable release:

- Game Boy Color-specific hardware behavior.
- Link cable networking across machines (local stub acceptable first).
- Cycle-perfect emulation of every undocumented edge on day one.

## 3. Hardware Components

### 3.1 CPU (`cpu`)

Responsibilities:

- Implement LR35902 instruction execution.
- Manage registers (`A, F, B, C, D, E, H, L, SP, PC`) and flag rules.
- Implement interrupt master enable semantics and `HALT`/`STOP` behavior.
- Handle interrupt dispatch and return (`RETI`).

Key interface:

- `step(&mut self, bus: &mut Bus, scheduler: &mut Scheduler) -> Result<Cycles, CpuError>`

Critical behaviors:

- Correct flag math on arithmetic/bit operations.
- `HALT` behavior including known edge behavior.
- Accurate interrupt timing and IME interactions.

Test requirements:

- Opcode-level unit tests (including CB-prefixed instructions).
- Flag transition tests.
- Interrupt/IME/HALT behavior tests.
- Regression tests for documented CPU edge cases.

### 3.2 Memory Bus and Map (`bus`, `memory`)

Responsibilities:

- Route reads/writes to correct address space region.
- Enforce access behavior and mirroring.
- Provide a single consistent memory API for CPU, DMA, PPU, and APU.

DMG memory regions (high-level):

- `0x0000-0x3FFF`: ROM bank 0
- `0x4000-0x7FFF`: Switchable ROM bank
- `0x8000-0x9FFF`: VRAM
- `0xA000-0xBFFF`: External RAM (cartridge)
- `0xC000-0xDFFF`: WRAM
- `0xE000-0xFDFF`: Echo RAM
- `0xFE00-0xFE9F`: OAM
- `0xFEA0-0xFEFF`: Unusable
- `0xFF00-0xFF7F`: IO registers
- `0xFF80-0xFFFE`: HRAM
- `0xFFFF`: IE register

Test requirements:

- Address dispatch tests.
- Read/write width and masking tests.
- Echo RAM and unusable region behavior tests.
- IO register access behavior tests.

### 3.3 Cartridge and MBC (`cartridge`)

Responsibilities:

- Parse ROM header and cartridge metadata.
- Implement no-MBC and common mappers:
  - `ROM ONLY`
  - `MBC1`
  - `MBC2`
  - `MBC3` (including RTC where applicable)
  - `MBC5`
- Persist battery-backed RAM/RTC state.

Test requirements:

- Header parsing and capability detection tests.
- Bank-switching logic tests per mapper.
- RAM enable/disable behavior tests.
- Save persistence round-trip tests.
- RTC register/latch tests for MBC3.

### 3.4 PPU / LCD (`ppu`)

Responsibilities:

- Emulate scanline timing and LCD modes:
  - Mode 2: OAM scan
  - Mode 3: pixel transfer
  - Mode 0: HBlank
  - Mode 1: VBlank
- Render background, window, and sprites with priority rules.
- Update `LY`, `LYC`, STAT interrupts, and VBlank interrupt.

Test requirements:

- Mode transition timing tests.
- Register behavior tests (`LCDC`, `STAT`, `SCX`, `SCY`, `WX`, `WY`, palettes).
- Sprite priority/overlap tests.
- Golden-frame tests for representative scenes.

### 3.5 APU / Sound (`apu`)

Responsibilities:

- Emulate DMG audio channels:
  - Square 1 (sweep)
  - Square 2
  - Wave channel
  - Noise channel
- Implement frame sequencer timing.
- Mix output samples for frontend consumption.

Test requirements:

- Channel register behavior tests.
- Length counter/envelope/sweep tests.
- Frame sequencer timing tests.
- Deterministic sample output tests with fixed input patterns.

### 3.6 Timers (`timers`)

Responsibilities:

- Emulate `DIV`, `TIMA`, `TMA`, `TAC` behavior.
- Generate timer interrupts on overflow.
- Reproduce edge timing behavior used by real games.

Test requirements:

- Frequency selection tests.
- Overflow/reload timing tests.
- Interrupt generation tests.

### 3.7 Interrupt Controller (`interrupts`)

Responsibilities:

- Manage `IE` and `IF` bits.
- Prioritize and dispatch interrupts in hardware order.

Interrupt lines:

- VBlank, LCD STAT, Timer, Serial, Joypad.

Test requirements:

- Priority and masking tests.
- Request/acknowledge behavior tests.
- IME interaction tests with CPU.

### 3.8 DMA (`dma`)

Responsibilities:

- OAM DMA transfer from source page to OAM (`FF46`).
- Apply CPU access restrictions during transfer where required.

Test requirements:

- Transfer length and destination tests.
- Timing/restriction behavior tests.

### 3.9 Joypad (`joypad`)

Responsibilities:

- Implement input matrix selection via `P1/JOYP`.
- Raise joypad interrupt when appropriate.

Test requirements:

- Button press/release mapping tests.
- Selection line and interrupt behavior tests.

### 3.10 Serial (`serial`)

Responsibilities:

- Implement serial control/data registers for single-player compatibility.
- Support internal clock transfer model; external/link behavior can be stubbed initially.

Test requirements:

- Register semantics tests.
- Transfer completion interrupt tests.

### 3.11 Boot ROM (`boot`)

Responsibilities:

- Optional boot ROM execution support.
- Correct boot ROM unmap behavior.

Test requirements:

- Boot ROM mapping/unmapping tests.
- Startup state validation tests.

### 3.12 Scheduler / Clock (`scheduler`)

Responsibilities:

- Central cycle progression source.
- Synchronize CPU, PPU, APU, timers, DMA, and interrupts.

Test requirements:

- Deterministic ordering tests.
- Long-run state hash consistency tests.

### 3.13 State and Debug (`state`, `debug`)

Responsibilities:

- Save/load complete emulator snapshots.
- Expose trace hooks, breakpoints, and memory watch support.

Test requirements:

- Snapshot round-trip tests.
- Restore determinism tests.
- Debug hook non-interference tests.

## 4. Proposed High-Level File Structure

```text
/Users/evanputnam/Workplace/gb-emulator/
  Cargo.toml
  design/
    rust-standards.md
    gb-high-level-design.md
  crates/
    gb-core/
      Cargo.toml
      src/
        lib.rs
        config.rs
        error.rs
        gameboy.rs
        scheduler/
          mod.rs
          clock.rs
          event.rs
        cpu/
          mod.rs
          core.rs
          decode.rs
          execute.rs
          registers.rs
        bus/
          mod.rs
          memory_map.rs
        memory/
          mod.rs
          wram.rs
          hram.rs
          vram.rs
          oam.rs
          io.rs
        cartridge/
          mod.rs
          header.rs
          mbc/
            mod.rs
            rom_only.rs
            mbc1.rs
            mbc2.rs
            mbc3.rs
            mbc5.rs
          rtc.rs
        ppu/
          mod.rs
          timing.rs
          fetcher.rs
          renderer.rs
          registers.rs
        apu/
          mod.rs
          frame_sequencer.rs
          square.rs
          wave.rs
          noise.rs
          mixer.rs
        timers/
          mod.rs
        interrupts/
          mod.rs
        dma/
          mod.rs
        joypad/
          mod.rs
        serial/
          mod.rs
        boot/
          mod.rs
        state/
          mod.rs
          snapshot.rs
        debug/
          mod.rs
          trace.rs
      tests/
        cpu_opcodes.rs
        cpu_interrupts.rs
        memory_map.rs
        mbc.rs
        ppu_timing.rs
        ppu_golden.rs
        apu_channels.rs
        timers.rs
        dma.rs
        integration_roms.rs
    gb-headless/
      Cargo.toml
      src/main.rs
    gb-desktop/
      Cargo.toml
      src/main.rs
  testdata/
    bootrom/
    roms/
    golden/
      frames/
      audio/
```

Notes:

- Keep `gb-core` pure and deterministic.
- Frontends should never own emulation logic.
- Test fixtures should be organized by subsystem and source.

## 5. AI-Friendly Task Decomposition

Each task is intentionally narrow so an AI agent can implement, test, and review in one pass.

### Phase 0: Repository Foundation

1. Convert repo to workspace layout with `gb-core`, `gb-headless`, `gb-desktop`.
2. Set lint/test commands and baseline CI checks.
3. Add shared error/config/logging scaffolding.

### Phase 1: CPU Bring-Up

1. Implement register model and flags.
2. Implement base instruction decode/dispatch tables.
3. Implement core arithmetic/load/jump instruction groups.
4. Add opcode and flag conformance tests.

### Phase 2: Memory and Cartridge Core

1. Implement memory map skeleton and bus dispatch.
2. Implement `ROM ONLY` and `MBC1`.
3. Add ROM loader and header parser.
4. Add mapper and memory routing tests.

### Phase 3: Timing and Interrupts

1. Implement scheduler/clock.
2. Implement interrupts (`IE`, `IF`, CPU servicing).
3. Implement timers (`DIV/TIMA/TMA/TAC`).
4. Add timing and interrupt interaction tests.

### Phase 4: PPU Core

1. Implement LCD mode timing state machine.
2. Implement background/window rendering.
3. Implement sprite rendering and priority.
4. Implement STAT/VBlank interrupt generation.
5. Add timing tests and golden-frame tests.

### Phase 5: APU Core

1. Implement frame sequencer and channel state.
2. Implement channel output generation.
3. Implement mixer output API.
4. Add deterministic audio tests.

### Phase 6: DMA, Joypad, Serial

1. Implement OAM DMA behavior.
2. Implement joypad register/input matrix.
3. Implement serial transfer baseline behavior.
4. Add subsystem interaction tests.

### Phase 7: Advanced Cartridge Support

1. Add `MBC2`, `MBC3`, `MBC5`.
2. Add battery save persistence.
3. Add `MBC3` RTC support.
4. Add persistence and mapper regression tests.

### Phase 8: Boot ROM and Integration

1. Add optional boot ROM flow.
2. Integrate all subsystems through `GameBoy` facade.
3. Add headless ROM runner.
4. Validate with compatibility ROM suites.

### Phase 9: Frontend and Tooling

1. Add desktop frontend for video/audio/input.
2. Add debugging tools (trace/breakpoints/watchpoints).
3. Add save-state support and versioned snapshots.

### Phase 10: Compatibility Hardening

1. Run broad ROM matrix and track failures by subsystem.
2. Fix highest-impact compatibility bugs.
3. Add regression tests for every fixed issue.
4. Profile and optimize hot paths without behavior changes.
5. Close deferred baseline behaviors from earlier phases (CPU, PPU, APU, DMA, timer, serial edge behavior).
6. Expand mapper/special-cartridge support as required by failing commercial titles in the compatibility matrix.

## 6. Comprehensive Testing Requirement (Mandatory)

Every component must be designed and implemented with comprehensive tests.

Minimum required for each component:

- Unit tests for core logic and register semantics.
- Edge-case tests for hardware quirks and boundary behavior.
- Integration tests for cross-component interactions.
- Regression tests for every bug fix.

Definition of done for any task/component:

1. Implementation complete.
2. Tests cover normal, edge, and failure paths.
3. Lints and formatting pass.
4. No behavioral regressions in existing suite.

## 7. Milestone Exit Criteria

A milestone is complete only if:

1. CI checks pass (`fmt`, `clippy`, `test`).
2. Determinism checks pass for affected systems.
3. Compatibility baseline improves or stays stable.
4. New behavior is documented in design and code docs.

## 8. Complete Emulator Validation Gate

The project is considered a complete DMG emulator only when all of the following are true:

1. The phase 10 compatibility matrix passes target thresholds for `smoke`, `core`, and `full` tiers.
2. Deferred behaviors from earlier phases are closed with tests:
   - CPU edge cases (`HALT` bug family, interrupt timing edge interactions, STOP behavior policy).
   - PPU timing fidelity (dot-accurate mode 3 transfer model and STAT/LYC edge behavior).
   - Timer and DMA edge timing behavior validated on compatibility ROMs.
   - APU power/timing quirks and channel edge behavior validated by deterministic fixtures.
   - Serial behavior is fully specified for supported modes and tested deterministically.
3. Commercial-title failures are either fixed or explicitly documented as unsupported with technical rationale.
4. All fixes include regression tests and are reflected in compatibility reporting.

## 9. Immediate Next Steps

1. Implement workspace structure and `gb-core` skeleton.
2. Add CPU register model + instruction table scaffolding.
3. Add bus/memory map with cartridge `ROM ONLY` and `MBC1`.
4. Establish first deterministic ROM test harness.
