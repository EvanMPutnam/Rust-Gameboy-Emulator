# Game Boy Desktop Frontend Low-Level Design (View/Audio/Input)

This document defines the low-level design required to move `gb-desktop` from scaffold status to a usable desktop frontend that can load and run `.gb` files with visible video and audible audio.

Related docs:

- `/Users/evanputnam/Workplace/gb-emulator/design/rust-standards.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-high-level-design.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/gb-low-level-design-phases-9-10.md`
- `/Users/evanputnam/Workplace/gb-emulator/design/agent-handoff.md`

## 1. Objectives

1. Run `.gb` ROMs via `gb-desktop` using a real OS window.
2. Present deterministic core frames (`160x144`) with stable aspect policy.
3. Play audio from core sample output through host audio device.
4. Map host key events to `GbButton` without mutating core determinism.
5. Preserve all existing core/headless behavior and quality gates.
6. Support Windows, macOS, and Linux from one frontend implementation.

## 2. Non-Goals

- No Game Boy Color support in this phase.
- No cross-machine link cable support.
- No full debugger UI in this phase (hooks already exist in `gb-core`).
- No hot-path optimization work beyond what is needed for stable playback.

## 3. Engineering Constraints

- Production code must not use `unwrap()`/`expect()`.
- All fallible desktop operations return typed errors.
- Frontend pacing must not alter logical emulator determinism.
- Rendering/audio jitter may affect latency only, never emulation state transitions.
- `gb-core` remains platform-agnostic and free of window/audio backend dependencies.

## 4. Proposed Crate and File Layout

`crates/gb-desktop/src/`

- `main.rs`
  - CLI parsing and launch path.
- `app.rs`
  - High-level orchestration around `GameBoy`.
- `platform.rs` (new)
  - `winit` integration and window lifecycle/events.
- `video.rs`
  - DMG frame conversion and renderer submission path.
- `audio.rs`
  - output queue and backend callback integration.
- `input.rs`
  - host event to `GbButton` mapping.
- `runloop.rs`
  - host-time accumulation -> cycle budgets.
- `error.rs` (new)
  - typed desktop error enum.

## 5. Dependency Plan

Recommended backend stack:

- Window/event loop: `winit`
- Pixel presentation: `pixels` (or `wgpu` directly)
- Audio output: `cpal`

Selection criteria:

- Minimal API complexity.
- Stable cross-platform support on Windows/macOS/Linux.
- Deterministic-friendly architecture (frontend buffers, core isolated).

Platform policy:

- Prefer a single backend path for all three major desktop platforms.
- Use backend defaults first; add platform-specific branches only when unavoidable.
- Keep platform conditionals localized to `platform.rs` and backend glue.

## 6. Runtime Architecture

Per event-loop tick:

1. Poll host events.
2. Map key events to `GbButton` and call `GameBoy::set_button`.
3. Compute cycle budget from elapsed host time accumulator.
4. Execute core via `run_cycles`.
5. Drain newest frame(s); present latest frame.
6. Drain audio samples; enqueue for audio callback consumption.
7. Sleep/yield only for host pacing and CPU usage control.

Determinism rule:

- Core always advances by explicit cycle counts from `runloop`.
- Frontend render frequency and callback timing do not mutate core state.

## 7. Detailed Subsystem Design

### 7.1 CLI Contract

Required:

- `--rom <path>`

Optional:

- `--boot-rom <path>`
- `--save-in <path>`
- `--save-out <path>`
- `--max-cycles <n>` (debug/testing)
- `--mute` (audio off for diagnostics)

Error behavior:

- Missing/invalid args return non-zero exit with actionable error text.
- Invalid ROM payload surfaces typed load/parse errors.

### 7.2 Video Pipeline

Input:

- `GameBoy::take_frame()` returns `[u8; 160*144]` shade indices `0..=3`.

Conversion:

- Map indices to fixed RGB palette table.
- Write to renderer framebuffer format (`RGBA8` or backend-native equivalent).

Presentation:

- Maintain integer-scaled 10:9 output policy.
- On resize, letterbox/pillarbox to preserve aspect.
- Preserve stable behavior under repeated resize events.

### 7.3 Audio Pipeline

Input:

- `GameBoy::take_audio_samples()` returns interleaved stereo `i16`.

Queue policy:

- Push samples into bounded queue shared with device callback.
- Overflow policy: drop oldest samples, count drops for diagnostics.
- Underflow policy: output silence and count underflows.

Output:

- `cpal` callback drains queue into device format.
- Conversion paths are explicit and tested.

### 7.4 Input Pipeline

Mapping:

- Arrow keys -> D-pad.
- `Z/X` -> `A/B`.
- `Enter/Backspace` -> `Start/Select`.

Behavior:

- Key-down => pressed `true`; key-up => pressed `false`.
- Ignore unsupported keys without side effects.

### 7.5 Run-Loop and Pacing

Clock model:

- Use `Instant` deltas to accumulate cycle units.
- Convert elapsed time to cycle budget at DMG rate (`4_194_304 Hz`).
- Cap per-iteration budget to prevent runaway after stalls.

Jitter handling:

- Keep remainder accumulator for fractional cycles.
- Never alter core behavior based on wall clock except cycle budget amount.

### 7.6 Save/Load Integration (Frontend UX)

Hotkeys:

- `F5` -> snapshot export to disk.
- `F9` -> snapshot import from latest/default path.

Requirements:

- Failed save/load never panics.
- Show/return clear typed error context.

## 8. Error Model

Add `DesktopError` enum with categories:

- CLI/argument validation.
- ROM/boot/save file I/O.
- Core startup or runtime failures.
- Window/render backend init/runtime failures.
- Audio backend init/runtime failures.
- Snapshot save/load failures.

All top-level entry points return `Result<_, DesktopError>`.

## 9. Testing Matrix

### 9.1 Unit Tests

- Input mapping correctness.
- Run-loop budget accumulation and capping.
- Video aspect/resize policy.
- Audio queue overflow/underflow behavior.

### 9.2 Integration/Smoke Tests

- Desktop startup/shutdown with backend abstraction.
- `--rom` path load and first-frame presentation path.
- Input press/release integration with `GameBoy`.
- Audio callback drain behavior under sustained run.
- Snapshot hotkey save/load round-trip.

### 9.3 Cross-Platform Validation

- Add CI jobs for:
  - Windows latest
  - macOS latest
  - Ubuntu latest
- Each platform runs:
  - `cargo fmt --check`
  - strict `clippy`
  - `cargo test --all-targets --all-features`
- At least one desktop smoke path must run on each platform (headless fallback is insufficient for completion criteria).

### 9.4 Regression Tests

- Resize stress sequence keeps stable aspect + no panic.
- Audio queue policy remains deterministic.
- Frontend integration does not regress core deterministic tests.

## 10. Implementation Slices

1. Add desktop error type + CLI arg parsing for ROM path and options.
2. Integrate `winit` window lifecycle and event polling.
3. Integrate video backend and DMG palette conversion.
4. Integrate input event mapping through existing `InputMap`.
5. Integrate audio device callback and bounded queue.
6. Wire run-loop pacing and frame/audio drains end-to-end.
7. Add snapshot hotkey load/save.
8. Add/expand smoke and integration tests.

Each slice must land with tests and no quality-gate regressions.

## 11. Exit Criteria

All must be true:

1. `cargo run -p gb-desktop -- --rom /absolute/path/to/test.gb` opens a window and renders live frames.
2. Keyboard controls affect game state predictably.
3. Audio output plays without panic and with explicit queue diagnostics.
4. `cargo fmt`, strict `clippy`, and full `cargo test` pass.
5. Existing `gb-headless` compatibility harness and tests remain green.
6. Frontend build and smoke behavior are verified on Windows/macOS/Linux.
