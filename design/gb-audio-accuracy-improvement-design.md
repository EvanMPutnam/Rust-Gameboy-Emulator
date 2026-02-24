# DMG Audio Accuracy Improvement Design

This document defines a focused plan to move emulator audio quality from "mostly correct" to high-confidence DMG-accurate behavior, while preserving determinism and platform separation.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-4-5.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-9-10.md`

## 1. Goals

1. Improve audible accuracy of DMG audio output in `gb-core` and `gb-desktop`.
2. Eliminate avoidable distortion sources (clipping, bad DAC modeling, unstable queue behavior).
3. Preserve determinism for fixed ROM/input scripts.
4. Keep `gb-core` platform-agnostic and independent from host audio timing.
5. Land changes in small, test-backed slices.

## 2. Non-Goals

1. No Game Boy Color audio behavior in this plan.
2. No unsafe optimization work.
3. No host-clock-driven logic in core emulation.
4. No large frontend refactor unrelated to audio accuracy.

## 3. Problem Decomposition

Audio defects can come from two separate layers and must be diagnosed independently:

1. `gb-core` APU behavior accuracy.
2. `gb-desktop` transport/resampling/output behavior.

The plan intentionally treats these as separate tracks, then validates end-to-end.

## 4. Known High-Risk Accuracy Gaps

These are the highest-probability sources of "80% right" audio.

1. Mixer and DAC model mismatch:
- Current output is heavily gain-scaled and hard-clamped.
- Per-channel output centering is simplified.
- This can produce clipping distortion even when timing is correct.

2. Frame sequencer timing source simplification:
- Hardware clocks APU sequencing from divider edges.
- Simplified periodic stepping can miss edge behaviors around DIV changes.

3. Trigger and length/envelope/sweep edge rules:
- Trigger-time side effects are often where compatibility and timbre drift occur.
- Length enable and sweep overflow edge cases are commonly mis-modeled.

4. Wave channel details:
- Wave DAC disable behavior, volume code behavior, and active wave RAM semantics are easy to get subtly wrong.

5. Frontend resampler quality:
- Current hold-style resampling can add audible roughness when device rate differs from 48 kHz.
- Queue underflow/overflow policy can create crackle or pumping.

## 5. Design Principles

1. Accuracy before polish: fix modeled hardware behavior before adding audio effects.
2. Deterministic core always: host callback timing never changes emulated state.
3. Measured rollout: every behavior change must include targeted tests and before/after evidence.
4. Explicit diagnostics: all high-overhead audio diagnostics stay opt-in.

## 6. Work Plan

## 6.1 Phase A: Baseline and Instrumentation

Add clear observability before large behavior changes.

1. Add opt-in APU diagnostics in core:
- Per-channel peak/RMS.
- Clipping sample count and clipping ppm.
- Frame sequencer step events.
- Queue-independent sample hash checkpoints.

2. Extend desktop diagnostics:
- Queue underflow/overflow rates.
- Callback rate and requested sample rate deltas.
- Resampler mode and ratio.

3. Baseline capture commands:
- `cargo run -p gb-headless --release -- --rom "/Users/evanputnam/Workplace/gb-emulator/roms/Tetris (World) (Rev A).gb" --frames 600`
- `cargo run -p gb-desktop --release -- --rom "/Users/evanputnam/Workplace/gb-emulator/roms/Tetris (World) (Rev A).gb" --audio-diagnostics`

## 6.2 Phase B: Core APU Output Model Accuracy

Prioritize distortion-causing modeling gaps.

1. Rework channel-to-mixer path to DMG-like DAC domain:
- Normalize channel outputs to a consistent DAC amplitude model.
- Remove ad-hoc gain that causes frequent clamp.
- Keep deterministic fixed-point math.

2. Add DMG-style output conditioning:
- Implement a deterministic high-pass stage to reduce DC offset behavior mismatch.
- Keep filter state in save states and determinism tests.

3. Validate with:
- Unit tests for peak headroom and no pathological clipping on known scripts.
- Deterministic audio hash tests on scripted register writes.

## 6.3 Phase C: APU Timing and Register Edge Behavior

Tighten behavior that affects timbre and envelope shape.

1. Drive frame sequencer from divider-edge semantics.
2. Correct trigger-time rules for all channels.
3. Reconcile length enable extra-clock behavior and envelope/sweep corner cases.
4. Tighten NR52 power-on/off register masking and channel disable effects.
5. Tighten wave channel active read/write behavior.

Each behavior change requires:
- One focused unit/regression test.
- One integration script showing deterministic sample stream.

## 6.4 Phase D: Desktop Output Fidelity

Fix non-core distortion sources without touching emulator logic.

1. Replace hold-style resampling with deterministic linear interpolation.
2. Keep queue bounded but reduce audible artifacts:
- Prefer smooth continuity over abrupt zero-fill when feasible.
- Keep underflow/overflow counters for diagnostics.
3. Maintain channel mapping correctness for >2 output channels.
4. Keep frontend timing and buffering policy separate from core progression.

## 6.5 Phase E: Compatibility and Acceptance Gating

1. Add audio-focused ROM coverage in compat manifests (including APU-focused suites).
2. Track pass/fail and audio hash deltas per ROM in compatibility reporting.
3. Require no determinism regressions and no CI quality-gate regressions.

## 7. File-Level Change Plan

Core:

- `crates/gb-core/src/apu/mod.rs`
- `crates/gb-core/src/apu/frame_sequencer.rs` (new if split out)
- `crates/gb-core/src/apu/mixer.rs` (new if split out)
- `crates/gb-core/src/state/snapshot.rs` (filter/diagnostic state compatibility as needed)
- `crates/gb-core/tests/apu_channels.rs`
- `crates/gb-core/tests/apu_determinism.rs`
- `crates/gb-core/tests/regression/` (new targeted audio regressions)

Frontend:

- `crates/gb-desktop/src/audio.rs`
- `crates/gb-desktop/src/app.rs`
- `crates/gb-desktop/tests/` (new desktop audio pipeline tests if needed)

Compatibility:

- `testdata/compat/manifests/smoke.toml`
- `testdata/compat/manifests/core.toml`
- `testdata/compat/manifests/full.toml`
- `crates/gb-headless/src/compat/runner.rs`

## 8. Test Strategy

1. Unit tests:
- Channel trigger/length/envelope/sweep rules.
- NR52 power semantics.
- Wave channel edge behaviors.

2. Determinism tests:
- Scripted writes produce exact repeated sample vectors.
- Same ROM/input produces identical audio hash across repeated runs.

3. Regression tests:
- One test per discovered mismatch.
- Keep failures minimal and focused (small scripts/fixtures).

4. Desktop pipeline tests:
- Resampler conversion correctness for common rate pairs.
- Queue policy behavior under stress.
- No panics and stable callback handling.

## 9. Acceptance Criteria

All must be true:

1. Audio quality is audibly improved on target commercial ROMs (no persistent heavy clipping/distortion).
2. Determinism holds for repeated fixed runs (exact hash match run-to-run).
3. APU-focused compatibility tests pass at required thresholds.
4. `cargo fmt`, strict `clippy`, and full `cargo test` remain green.
5. No core behavior depends on host audio callback timing.

## 10. Rollout and Risk Control

1. Land phases A-E as small PR slices.
2. Keep behavior changes isolated from performance-only changes.
3. If a change improves sound but regresses determinism, reject and redesign.
4. If frontend change improves smoothness but hides core mismatch, keep both fixes explicit.

## 11. Required Validation Commands Per Slice

1. `cargo fmt`
2. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic`
3. `cargo test --all-targets --all-features`
4. `cargo run -p gb-headless --release -- --rom "/Users/evanputnam/Workplace/gb-emulator/roms/Tetris (World) (Rev A).gb" --frames 600`

