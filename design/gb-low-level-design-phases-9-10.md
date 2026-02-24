# Original Game Boy Low-Level Design (Phases 9-10)

This document expands phases 9-10 from the high-level plan into implementation-ready low-level design.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-high-level-design.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-1-3.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-4-5.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-6-8.md`

## 1. Global Engineering Constraints (Applies to Phases 9-10)

- Production code must not use `unwrap()`/`expect()`.
- All fallible operations return typed `Result` errors.
- Frontend and tooling must not break core determinism.
- Every component and task requires comprehensive tests (unit + edge + integration + regression as applicable).

## 2. Shared Conventions for Finalization

## 2.1 Core/Frontend Boundary

- `gb-core` remains platform-agnostic and deterministic.
- `gb-desktop` and `gb-headless` are adapters around `gb-core`.
- Any timing, rendering, and input policy in frontends must be explicit and testable.

## 2.2 Debug Instrumentation Policy

- Debug hooks must be optional and disabled by default for release runs.
- Enabling debug tools must not change emulation outcomes.
- Hook overhead should be measurable and documented.

## 2.3 Save-State Stability Policy

- Save states must include a format version and compatibility checks.
- Loading mismatched versions returns typed errors, never panics.
- Round-trip state behavior must be deterministic.

## 2.4 Compatibility Data Policy

- Compatibility results are tracked by manifest and machine-readable reports.
- Every fixed compatibility bug adds a reproducible regression test.
- Performance optimizations are only accepted with before/after measurement.

## 3. Phase 9 Low-Level Design: Frontend and Tooling

## 3.1 Objectives

1. Add desktop frontend for video/audio/input.
2. Add debugging tools (trace/breakpoints/watchpoints).
3. Add save-state support with versioning.

## 3.2 File-Level Implementation Plan

- `crates/gb-desktop/src/main.rs`
- `crates/gb-desktop/src/app.rs` (new)
- `crates/gb-desktop/src/video.rs` (new)
- `crates/gb-desktop/src/audio.rs` (new)
- `crates/gb-desktop/src/input.rs` (new)
- `crates/gb-desktop/src/runloop.rs` (new)
- `crates/gb-desktop/src/debug_ui.rs` (new, optional)
- `crates/gb-core/src/debug/mod.rs`
- `crates/gb-core/src/debug/trace.rs`
- `crates/gb-core/src/debug/breakpoint.rs` (new)
- `crates/gb-core/src/debug/watchpoint.rs` (new)
- `crates/gb-core/src/state/mod.rs`
- `crates/gb-core/src/state/snapshot.rs`
- `crates/gb-core/src/state/version.rs` (new)
- `crates/gb-core/tests/state_snapshot.rs` (new)
- `crates/gb-core/tests/debug_hooks.rs` (new)
- `crates/gb-core/tests/integration_frontend_contract.rs` (new)

## 3.3 Desktop Frontend Architecture

Runtime components:

- Emulator core instance (`GameBoy`).
- Video output path (160x144 texture scale to window).
- Audio output path (pull/push buffer based on backend).
- Input mapper (keyboard/controller -> `GbButton`).
- Main run loop with cycle/frame pacing.

Reference flow:

1. Poll input events and update button states.
2. Execute core for target cycle budget.
3. Pull completed frame(s) and present latest.
4. Drain audio samples and queue to backend.
5. Apply pacing strategy to avoid runaway CPU usage.

## 3.4 Run Loop and Timing Strategy

- Use fixed emulation step accumulation tied to GB clock rate.
- Decouple render frequency from emulation stepping (frame skip optional).
- Audio buffer fullness can provide minor pacing correction, but never mutate emulation logic.

Core policy:

- Emulation progression derives from accumulated virtual cycles.
- Host jitter affects only presentation latency, not logical core outputs.

## 3.5 Input Mapping Design

Features:

- Configurable key bindings for all GB buttons.
- Press/release events forwarded immediately to core.
- Optional turbo bindings implemented as explicit repeated press/release script, not hidden state mutation.

API sketch:

```rust
pub struct InputMap {
    // host keycode -> GbButton
}

impl InputMap {
    pub fn map_event(&self, event: HostInputEvent) -> Option<(GbButton, bool)>;
}
```

## 3.6 Debug Tooling Design

Capabilities:

- Instruction trace stream (PC/opcode/register snapshot).
- Execution breakpoints:
  - PC breakpoint
  - Opcode breakpoint
- Memory watchpoints:
  - address read
  - address write

Hook interface:

```rust
pub enum DebugEvent {
    CpuStep { pc: u16, opcode: u8 },
    MemRead { addr: u16, value: u8 },
    MemWrite { addr: u16, value: u8 },
    InterruptServiced { vector: u16 },
}

pub trait DebugSink {
    fn on_event(&mut self, event: &DebugEvent);
}
```

Rules:

- Hooks are invoked from deterministic points in core execution.
- If debug is disabled, fast path should avoid allocation and string formatting.

## 3.7 Save-State Format Design

Snapshot content must include:

- CPU state (registers, flags, IME/HALT state).
- Bus and memory regions (WRAM/HRAM/VRAM/OAM/IO/IE/IF).
- PPU/APU internal timing state.
- Timers, DMA, joypad, serial state.
- Cartridge mapper state, RAM, and RTC state.
- Boot ROM enable state and scheduler clock.

Serialization container:

```rust
pub struct SnapshotHeader {
    pub magic: [u8; 4],   // e.g. GBST
    pub version: u16,
    pub flags: u16,
    pub payload_len: u32,
    pub checksum: u32,
}

pub struct Snapshot {
    pub header: SnapshotHeader,
    pub payload: Vec<u8>,
}
```

Versioning policy:

- Start at `v1` for first stable snapshot format.
- Backward compatibility policy explicit per version.
- Unsupported versions return `StateError::UnsupportedVersion`.

## 3.8 Phase 9 Integration Points

- `gb-desktop` consumes:
  - `GameBoy::step/run_cycles`
  - `GameBoy::set_button`
  - `GameBoy::take_frame`
  - `GameBoy::take_audio_samples`
  - `GameBoy::export_save/import_save`
- Debug UI and headless tooling share the same `debug` hook interfaces.
- Save state can be triggered from desktop hotkeys and headless CLI.

## 3.9 Phase 9 Test Matrix (Comprehensive)

`state_snapshot.rs` minimum suites:

- Snapshot serialize/deserialize round-trip.
- Invalid header, checksum mismatch, and unsupported version errors.
- Restore determinism: state hash equality after save/load resume.

`debug_hooks.rs` minimum suites:

- Breakpoint stops execution at expected PC.
- Watchpoints fire only on configured addresses/access type.
- Debug disabled path emits no events and does not alter emulation results.

`integration_frontend_contract.rs` minimum suites:

- Input events cause expected joypad register behavior.
- Frame/audio extraction APIs are safe and deterministic over long runs.
- Save import/export through facade preserves cartridge-backed RAM.

`gb-desktop` smoke suites:

- Event loop startup/shutdown without panic.
- Audio queue underflow/overflow handling policy behavior.
- Window resize path preserves aspect ratio policy and stability.

## 3.10 Phase 9 AI Task Slices

1. Implement desktop run loop scaffold + input/video/audio adapters + smoke tests.
2. Implement debug event model and sink interface + unit tests.
3. Implement breakpoints/watchpoints in core hooks + tests.
4. Implement snapshot header/version/checksum format + tests.
5. Implement full emulator snapshot capture/restore + determinism tests.
6. Add frontend integration tests for input/frame/audio/save workflows.

Each slice must include comprehensive tests before merge.

## 4. Phase 10 Low-Level Design: Compatibility Hardening

## 4.1 Objectives

1. Run broad ROM matrix and track failures by subsystem.
2. Fix highest-impact compatibility bugs.
3. Add regression tests for every fixed issue.
4. Profile hot paths and optimize without behavior changes.

## 4.2 File-Level Implementation Plan

- `crates/gb-headless/src/compat/mod.rs` (new)
- `crates/gb-headless/src/compat/manifest.rs` (new)
- `crates/gb-headless/src/compat/runner.rs` (new)
- `crates/gb-headless/src/compat/report.rs` (new)
- `crates/gb-headless/src/main.rs` (extend CLI)
- `crates/gb-core/tests/regression/` (new directory)
- `crates/gb-core/tests/integration_roms.rs` (extend)
- `testdata/compat/manifests/smoke.toml` (new)
- `testdata/compat/manifests/core.toml` (new)
- `testdata/compat/manifests/full.toml` (new)
- `design/compatibility-matrix.md` (new)

## 4.3 Compatibility Harness Design

Manifest-driven execution:

```toml
[[rom]]
id = "cpu_instrs"
path = "testdata/roms/blargg/cpu_instrs.gb"
max_cycles = 80000000
expected = "pass"
tags = ["cpu", "timing"]
```

Runner behavior:

- Load manifest entries.
- Execute each ROM with fixed seed/config/input scripts.
- Capture outcome and optional diagnostics:
  - exit status
  - elapsed emulated cycles
  - frame hash/audio hash
  - failure category
- Emit machine-readable JSON report.

Failure categories:

- `cpu`
- `timing`
- `ppu`
- `apu`
- `memory_mapper`
- `io`
- `unknown`

## 4.4 Triage and Regression Workflow

For each failing ROM:

1. Reproduce with minimal deterministic script.
2. Localize failure subsystem.
3. Implement fix in narrow scope.
4. Add targeted regression test in `gb-core/tests/regression`.
5. Re-run affected matrix tiers.

Regression artifact policy:

- Prefer smallest legal ROM fixture or synthetic state-machine test.
- If ROM redistribution is restricted, use hash-identified local-only references and synthetic unit/integration equivalents.

## 4.5 ROM Matrix Tiers

Tier definitions:

- `smoke`: fast subset; run on every PR.
- `core`: broader compatibility set; run on merge/nightly.
- `full`: largest set; run scheduled/nightly or release candidates.

Gating policy:

- PRs must not regress `smoke` tier.
- Merge branch must maintain or improve `core` pass rate.
- Releases require reviewed `full` tier baseline.

## 4.5.1 Mandatory Deferred-Item Closure

Phase 10 must explicitly close deferred/baseline behaviors introduced in earlier phases, with regression tests:

- CPU: HALT bug family, STOP behavior policy, interrupt boundary edge timing.
- PPU: dot-accurate mode-3 timing and STAT/LYC edge behavior under contention.
- Timers/DMA: edge timing quirks validated against compatibility ROM outcomes.
- APU: power/reset and channel timing quirks that affect commercial titles.
- Serial: fully specified supported-mode behavior replacing earlier stub baseline.
- Cartridge: mapper or special-cartridge gaps exposed by compatibility matrix results.

## 4.6 Performance Hardening Design

Profiling workflow:

1. Record baseline metrics (cycles/sec, frame time, audio buffer stability).
2. Profile top hotspots using stable repeatable workloads.
3. Apply optimization in smallest possible change.
4. Re-run determinism + compatibility + performance checks.

Optimization constraints:

- No behavior changes without explicit compatibility/test updates.
- Prefer algorithm/data-layout improvements before unsafe micro-optimizations.
- Any `unsafe` optimization requires documented `// SAFETY:` invariants and dedicated tests.

## 4.7 Compatibility Metrics and Reporting

Required report fields:

- Commit hash/build id.
- Manifest tier.
- Total/pass/fail/timeout counts.
- Per-ROM result with subsystem category.
- Delta versus previous baseline.

`compatibility-matrix.md` should summarize:

- Current pass rates by tier.
- Known blockers by subsystem.
- Recent regressions and fix status.

## 4.8 Phase 10 Test Matrix (Comprehensive)

`compat` harness tests:

- Manifest parser validation and error handling.
- Deterministic runner output for fixed ROM/script.
- Correct JSON report schema and values.

Regression suite tests:

- One regression test per fixed bug.
- No regressions for previously resolved issues.
- Subsystem-focused tests for failure classes.

Performance validation tests:

- Bench or perf-smoke checks with threshold alerts (non-blocking initially, gating later).
- Ensure optimized paths produce identical deterministic hashes to baseline runs.

Integration suites:

- Smoke/core tiers executable in CI pipeline.
- Failure categorization remains stable and actionable.

## 4.9 Phase 10 AI Task Slices

1. Implement compatibility manifest parser and typed model + tests.
2. Implement manifest runner and JSON report generator + tests.
3. Define smoke/core/full manifests and integrate CLI selection + tests.
4. Add regression test harness structure and first seeded regressions.
5. Implement failure categorization and compatibility summary generation.
6. Add profiling/benchmark scripts and deterministic equivalence checks.
7. Iterate bug-fix loops with mandatory regression tests per issue.
8. Close all deferred phase-1..8 behaviors listed in section 4.5.1 before release readiness sign-off.

Each slice must include comprehensive tests before merge.

## 5. Cross-Phase Definition of Done

For every task in phases 9-10:

1. Implementation is complete and follows rust standards.
2. Comprehensive tests are included for normal, edge, and failure behavior.
3. `cargo fmt --check` passes.
4. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic` passes.
5. `cargo test --all-targets --all-features` passes.
6. Any bug fix includes a regression test in the same change.

## 6. Suggested Execution Order

1. Phase 9 slices 1-6.
2. Phase 10 slices 1-7.

Project readiness checkpoint: do not declare release readiness until smoke/core/full compatibility baselines are documented and stable.
