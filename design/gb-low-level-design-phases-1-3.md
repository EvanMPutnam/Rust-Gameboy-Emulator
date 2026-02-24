# Original Game Boy Low-Level Design (Phases 1-3)

This document expands phases 1-3 from the high-level plan into implementation-ready low-level design.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-high-level-design.md`

## 1. Global Engineering Constraints (Applies to Phases 1-3)

- Production code must not use `unwrap()`/`expect()`.
- All fallible operations return typed `Result` errors.
- Core remains deterministic and side-effect free except explicit state mutation.
- Every component and task requires comprehensive tests (unit + edge + integration + regression as applicable).

## 2. Shared Types and Conventions

## 2.1 Type Aliases and Newtypes

```rust
pub type Cycles = u32;
pub type Address = u16;
pub type Byte = u8;
pub type Word = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickResult {
    Continue,
    FrameComplete,
}
```

## 2.2 Core Errors

```rust
#[derive(thiserror::Error, Debug)]
pub enum GbError {
    #[error("CPU error: {0}")]
    Cpu(#[from] CpuError),
    #[error("Bus error: {0}")]
    Bus(#[from] BusError),
    #[error("Cartridge error: {0}")]
    Cartridge(#[from] CartridgeError),
    #[error("Clock error: {0}")]
    Clock(#[from] ClockError),
}
```

## 2.3 Memory Access Interface

```rust
pub trait Memory {
    fn read8(&self, addr: Address) -> Result<Byte, BusError>;
    fn write8(&mut self, addr: Address, val: Byte) -> Result<(), BusError>;

    fn read16(&self, addr: Address) -> Result<Word, BusError>;
    fn write16(&mut self, addr: Address, val: Word) -> Result<(), BusError>;
}
```

- `read16/write16` must follow LR35902 little-endian behavior.
- Unaligned access is legal on GB and performed as two byte accesses.

## 2.4 Cycle Accounting Rule

- CPU instruction execution returns consumed machine cycles converted to T-cycles (or store one canonical unit and convert at subsystem boundaries).
- For phases 1-3, use one canonical `Cycles` unit consistently across CPU, timers, and scheduler.

## 3. Phase 1 Low-Level Design: CPU Bring-Up

## 3.1 Objectives

1. Implement register file and flags.
2. Implement instruction decode/dispatch scaffolding.
3. Implement core instruction groups: arithmetic, loads/stores, jumps/calls/returns.
4. Produce a testable step pipeline with deterministic state updates.

## 3.2 File-Level Implementation Plan

- `crates/gb-core/src/cpu/registers.rs`
- `crates/gb-core/src/cpu/decode.rs`
- `crates/gb-core/src/cpu/execute.rs`
- `crates/gb-core/src/cpu/core.rs`
- `crates/gb-core/src/cpu/mod.rs`
- `crates/gb-core/tests/cpu_opcodes.rs`
- `crates/gb-core/tests/cpu_interrupts.rs` (scaffold only in phase 1)

## 3.3 Register Model

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub a: u8,
    pub f: Flags,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub sp: u16,
    pub pc: u16,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u8 {
        const Z = 0b1000_0000;
        const N = 0b0100_0000;
        const H = 0b0010_0000;
        const C = 0b0001_0000;
    }
}
```

Rules:

- Lower nibble of `F` always reads as zero.
- Helper methods expose `af/bc/de/hl` get/set with masking for `F`.

## 3.4 CPU Core State

```rust
pub struct Cpu {
    regs: Registers,
    ime: bool,
    ime_scheduled: bool,
    halted: bool,
    stopped: bool,
}
```

- `ime_scheduled` supports EI delayed enable behavior.
- `halted` indicates fetch/execute stall until interrupt wake conditions.

## 3.5 Decode and Dispatch

Design:

- Static lookup table for 256 base opcodes.
- Static lookup table for 256 CB-prefixed opcodes.
- Each entry includes:
  - mnemonic (for debug/tests)
  - length (bytes)
  - base cycles
  - execute fn pointer

```rust
pub struct Op {
    pub code: u8,
    pub len: u8,
    pub cycles: Cycles,
    pub exec: fn(&mut Cpu, &mut dyn Memory) -> Result<ExecOutcome, CpuError>,
}

pub enum ExecOutcome {
    Next,
    Jumped,
    Halted,
}
```

PC behavior:

- Default PC increments by opcode length before/after execution per chosen consistent convention.
- Control-flow ops (`JP/JR/CALL/RET/RST`) must explicitly set PC and return `Jumped`.

## 3.6 Instruction Groups in Phase 1

Required coverage:

- 8-bit loads: register/register, immediate/register, memory via `HL`.
- 16-bit loads: `LD rr,d16`, `LD SP,HL`, stack push/pop.
- Arithmetic/logic: `ADD/ADC/SUB/SBC/AND/OR/XOR/CP`, `INC/DEC`.
- Control flow: `JP/JR`, conditional branches, `CALL/RET`, `RST`.
- Misc core ops: `NOP`, `DI`, `EI`, `CCF`, `SCF`, `CPL`, `DAA`.
- CB subset baseline: rotate/shift/bit/set/res on registers and `(HL)`.

Deferred to later phases if needed:

- Exact STOP low-power semantics.
- Full HALT edge-case finalization (initial scaffold required now).
- These deferred CPU behaviors are mandatory completion items and must be closed in phase 10 with explicit regression tests.

## 3.7 Step Pipeline

```rust
impl Cpu {
    pub fn step(&mut self, mem: &mut dyn Memory) -> Result<Cycles, CpuError> {
        // 1) resolve delayed IME update
        // 2) halt handling
        // 3) fetch opcode
        // 4) decode
        // 5) execute
        // 6) update pc/cycles
        // 7) return cycles consumed
    }
}
```

Constraints:

- No hidden global state.
- Every step path returns deterministic cycle count.

## 3.8 Phase 1 Test Matrix (Comprehensive)

`cpu_opcodes.rs` minimum suites:

- Register-pair getters/setters (`AF` low nibble masking).
- Flag correctness for representative arithmetic boundaries.
- Branch taken/not-taken cycle and PC behavior.
- Stack push/pop and call/ret correctness.
- CB opcode bit ops on registers and `(HL)`.
- `EI` delayed IME enable behavior.
- Initial HALT wake behavior fixture.
- Track CPU deferred-edge fixtures (HALT bug and STOP policy) as required phase-10 regression tests.

Test style:

- Table-driven cases for opcodes.
- Small deterministic memory mock implementing `Memory`.
- Regression test per bug fix discovered during ROM validation.

## 3.9 Phase 1 AI Task Slices

1. Implement `Registers` and flag helpers + tests.
2. Implement base opcode table scaffolding + unknown opcode typed error.
3. Implement load/store instructions + tests.
4. Implement arithmetic/logic instructions + tests.
5. Implement control-flow instructions + tests.
6. Implement CB baseline + tests.
7. Implement `Cpu::step` integration tests.

Each slice must include comprehensive tests before merge.

## 4. Phase 2 Low-Level Design: Memory and Cartridge Core

## 4.1 Objectives

1. Implement memory map and bus dispatch.
2. Implement cartridge loading/header parsing.
3. Implement `ROM ONLY` and `MBC1` mapper behavior.
4. Integrate mapper-backed ROM/RAM through bus.

## 4.2 File-Level Implementation Plan

- `crates/gb-core/src/bus/mod.rs`
- `crates/gb-core/src/bus/memory_map.rs`
- `crates/gb-core/src/memory/mod.rs`
- `crates/gb-core/src/memory/wram.rs`
- `crates/gb-core/src/memory/hram.rs`
- `crates/gb-core/src/memory/vram.rs`
- `crates/gb-core/src/memory/oam.rs`
- `crates/gb-core/src/memory/io.rs`
- `crates/gb-core/src/cartridge/header.rs`
- `crates/gb-core/src/cartridge/mod.rs`
- `crates/gb-core/src/cartridge/mbc/rom_only.rs`
- `crates/gb-core/src/cartridge/mbc/mbc1.rs`
- `crates/gb-core/tests/memory_map.rs`
- `crates/gb-core/tests/mbc.rs`

## 4.3 Bus Structure

```rust
pub struct Bus {
    pub cartridge: Cartridge,
    pub vram: [u8; 0x2000],
    pub wram: [u8; 0x2000],
    pub oam: [u8; 0xA0],
    pub hram: [u8; 0x7F],
    pub io: IoRegs,
    pub ie: u8,
}
```

Routing rules (phase 2):

- `0x0000..=0x7FFF` -> cartridge ROM via mapper.
- `0x8000..=0x9FFF` -> VRAM.
- `0xA000..=0xBFFF` -> cartridge external RAM via mapper.
- `0xC000..=0xDFFF` -> WRAM.
- `0xE000..=0xFDFF` -> echo of WRAM.
- `0xFE00..=0xFE9F` -> OAM.
- `0xFEA0..=0xFEFF` -> unusable, read fixed value/write ignored (document chosen behavior).
- `0xFF00..=0xFF7F` -> IO registers (minimal stubs in phase 2).
- `0xFF80..=0xFFFE` -> HRAM.
- `0xFFFF` -> IE.

## 4.4 Cartridge Abstractions

```rust
pub struct CartridgeHeader {
    pub title: String,
    pub cartridge_type: u8,
    pub rom_size_code: u8,
    pub ram_size_code: u8,
    pub cgb_flag: u8,
}

pub trait Mapper {
    fn read_rom(&self, addr: u16) -> Result<u8, CartridgeError>;
    fn write_rom(&mut self, addr: u16, val: u8) -> Result<(), CartridgeError>;
    fn read_ram(&self, addr: u16) -> Result<u8, CartridgeError>;
    fn write_ram(&mut self, addr: u16, val: u8) -> Result<(), CartridgeError>;
}
```

`Cartridge` holds raw ROM, optional RAM, header, and mapper implementation.

## 4.5 ROM ONLY Mapper Behavior

- `0x0000..=0x7FFF`: direct ROM read.
- ROM writes ignored.
- RAM absent unless explicitly supported by header/type.

## 4.6 MBC1 Behavior (Phase 2 Target)

Required behavior:

- RAM enable register (`0x0000..=0x1FFF`, lower nibble `0x0A`).
- ROM bank low bits (`0x2000..=0x3FFF`).
- Bank upper bits / RAM bank select (`0x4000..=0x5FFF`).
- Banking mode select (`0x6000..=0x7FFF`): ROM mode vs RAM mode.
- Bank 0 mapping rules and prohibited bank translation (e.g., 0->1 for switchable bank).

Explicitly document chosen edge behavior for unsupported header combinations.

## 4.7 Header Parsing

Parse and validate:

- Entry/header region needed for metadata.
- Cartridge type.
- ROM/RAM size codes.
- Header checksum (optional warn/error policy should be explicit).

API:

```rust
impl Cartridge {
    pub fn from_rom_bytes(bytes: Vec<u8>) -> Result<Self, CartridgeError>;
}
```

## 4.8 Phase 2 Test Matrix (Comprehensive)

`memory_map.rs` minimum suites:

- Region routing for each primary memory range.
- Echo RAM mirror read/write consistency.
- Unusable range behavior invariants.
- IE and HRAM direct behavior.

`mbc.rs` minimum suites:

- Header parse success/failure cases.
- ROM ONLY fixed bank reads.
- MBC1 bank switch across low/high bits.
- MBC1 RAM enable gating and RAM bank mode behavior.
- Edge regression cases (bank numbers mapping to forbidden values).

Integration tests:

- CPU fetches instructions from mapped ROM through bus.
- Writes to mapper control registers affect subsequent fetches.

## 4.9 Phase 2 AI Task Slices

1. Implement basic bus + static regions + tests.
2. Implement cartridge header parser + tests.
3. Implement ROM ONLY mapper + tests.
4. Implement MBC1 mapper + tests.
5. Integrate cartridge mapper into bus + integration tests.
6. Add regression tests for discovered mapping bugs.

Each slice must include comprehensive tests before merge.

## 5. Phase 3 Low-Level Design: Timing and Interrupts

## 5.1 Objectives

1. Implement scheduler/clock for deterministic progression.
2. Implement interrupt request/mask/service behavior.
3. Implement timers (`DIV`, `TIMA`, `TMA`, `TAC`) with correct overflow/interrupt behavior.
4. Couple CPU stepping to timing/interrupt flow.

## 5.2 File-Level Implementation Plan

- `crates/gb-core/src/scheduler/mod.rs`
- `crates/gb-core/src/scheduler/clock.rs`
- `crates/gb-core/src/scheduler/event.rs`
- `crates/gb-core/src/interrupts/mod.rs`
- `crates/gb-core/src/timers/mod.rs`
- `crates/gb-core/src/gameboy.rs`
- `crates/gb-core/tests/timers.rs`
- `crates/gb-core/tests/cpu_interrupts.rs`

## 5.3 Global Clock Model

```rust
pub struct Clock {
    now: Cycles,
}

impl Clock {
    pub fn tick(&mut self, delta: Cycles) { self.now = self.now.wrapping_add(delta); }
    pub fn now(&self) -> Cycles { self.now }
}
```

- `GameBoy::step()` executes one CPU step, then advances timers and interrupt sampling by returned cycles.
- All subsystems consume the same cycle delta to preserve determinism.

## 5.4 Interrupt Controller

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    VBlank = 0,
    LcdStat = 1,
    Timer = 2,
    Serial = 3,
    Joypad = 4,
}

pub struct InterruptController {
    pub ie: u8,
    pub iflags: u8,
}
```

API:

```rust
impl InterruptController {
    pub fn request(&mut self, interrupt: Interrupt);
    pub fn pending(&self) -> Option<Interrupt>;
    pub fn acknowledge(&mut self, interrupt: Interrupt);
}
```

Rules:

- Priority order: VBlank > LCD STAT > Timer > Serial > Joypad.
- `pending()` masks `iflags` with `ie`.
- CPU services interrupt only when IME is enabled (except HALT wake behavior).

## 5.5 Timer Model

Registers:

- `DIV` (`FF04`): upper byte of internal divider counter.
- `TIMA` (`FF05`)
- `TMA` (`FF06`)
- `TAC` (`FF07`)

Internal fields:

```rust
pub struct Timers {
    div_counter: u16,
    tima: u8,
    tma: u8,
    tac: u8,
    overflow_pending: bool,
}
```

Behavior requirements for phase 3:

- Increment divider with cycle progression.
- Select TIMA input clock via TAC bits.
- TIMA overflow triggers reload from TMA and requests Timer interrupt per modeled delay policy.
- Writes to DIV reset divider state according to chosen hardware model (document exact behavior).

## 5.6 CPU Interrupt Service Flow

At instruction boundary:

1. Resolve delayed IME change (`EI`).
2. Query `pending()` interrupt.
3. If IME and pending:
   - clear IME
   - clear HALT
   - push PC to stack
   - jump to vector (`0x40`, `0x48`, `0x50`, `0x58`, `0x60`)
   - return interrupt service cycle cost
4. Otherwise execute next opcode or HALT behavior.

## 5.7 GameBoy Integration Loop (Phase 3)

```rust
impl GameBoy {
    pub fn step(&mut self) -> Result<Cycles, GbError> {
        let cycles = self.cpu.step(&mut self.bus)?;
        self.clock.tick(cycles);
        self.timers.tick(cycles, &mut self.interrupts);
        self.sync_interrupt_registers();
        Ok(cycles)
    }
}
```

- `sync_interrupt_registers()` keeps memory-mapped IE/IF coherent between bus and controller.

## 5.8 Phase 3 Test Matrix (Comprehensive)

`timers.rs` minimum suites:

- DIV increments and reset behavior.
- TAC frequency selection correctness.
- TIMA overflow -> TMA reload behavior.
- Timer interrupt request timing behavior.

`cpu_interrupts.rs` minimum suites:

- Interrupt priority ordering.
- IME on/off service gating.
- `EI` delayed behavior with pending interrupt.
- HALT wake/service behavior baseline.
- Stack push and vector jump correctness during service.

Integration suites:

- `GameBoy::step()` advances CPU/clock/timer coherently.
- Timer-generated IF bit becomes serviceable by CPU.
- Deterministic state hash across repeated identical step sequences.

## 5.9 Phase 3 AI Task Slices

1. Implement `Clock` + scheduler scaffolding + tests.
2. Implement `InterruptController` + priority/masking tests.
3. Implement timer core + register behavior tests.
4. Integrate timer interrupt requests into controller + tests.
5. Integrate CPU interrupt servicing end-to-end + tests.
6. Add deterministic integration tests for `GameBoy::step`.

Each slice must include comprehensive tests before merge.

## 6. Cross-Phase Definition of Done

For every task in phases 1-3:

1. Implementation is complete and follows rust standards.
2. Comprehensive tests are included for normal, edge, and failure behavior.
3. `cargo fmt --check` passes.
4. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic` passes.
5. `cargo test --all-targets --all-features` passes.
6. Any bug fix includes a regression test in the same change.

## 7. Suggested Execution Order

1. Phase 1 slices 1-7.
2. Phase 2 slices 1-6.
3. Phase 3 slices 1-6.

Do not start phase 4 until all phase 3 tests are green and deterministic checks are stable.
