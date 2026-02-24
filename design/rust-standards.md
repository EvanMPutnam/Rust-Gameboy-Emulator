# Rust Engineering Standards

This document defines project-wide Rust standards for the Game Boy emulator. These rules apply to production code unless explicitly marked otherwise.

## 1) Core Principles

- Prefer correctness, explicitness, and maintainability over cleverness.
- Keep modules focused and composable.
- Design for deterministic emulator behavior first; optimize only after profiling.
- Treat warnings as failures in CI.

## 2) Error Handling and Panics

- Do not use `unwrap()` or `expect()` in production code.
- `unwrap()`/`expect()` are allowed in tests and test-only helpers.
- Prefer `Result<T, E>` for fallible operations and propagate errors with `?`.
- Prefer `Option<T>` when absence is expected and not exceptional.
- Convert `Option` to `Result` with explicit context where failure is meaningful.
- Define domain-specific error enums (using `thiserror` is recommended).
- Keep panic paths out of runtime emulator flow.
- Use assertions for invariants that indicate programmer bugs, not user/runtime error handling.

## 3) API and Type Design

- Make invalid states unrepresentable when practical.
- Use newtypes for strongly-typed addresses, cycle counts, and register-like values where it improves correctness.
- Keep public APIs minimal; default to private visibility.
- Avoid boolean argument traps; prefer enums/struct config parameters.
- Prefer borrowed inputs (`&T`, `&[u8]`, `&str`) unless ownership is required.
- Use `From`/`TryFrom` and `Into` for safe, explicit conversions.
- Use `#[must_use]` on values where ignoring results is likely a bug.

## 4) Ownership, Borrowing, and Memory

- Minimize cloning in hot paths.
- Prefer stack allocation and fixed-size arrays for emulation state when appropriate.
- Use iterators idiomatically, but do not sacrifice clarity in cycle-accurate logic.
- Avoid shared mutable state unless clearly justified.
- Use interior mutability (`RefCell`, `Cell`, `Mutex`, etc.) only with clear rationale.

## 5) Module and Project Structure

- Organize by emulator domains (CPU, PPU, memory bus, cartridge, timer, input, APU).
- Keep files reasonably small; split large modules by responsibility.
- Keep parsing/IO boundaries separated from emulation core.
- Keep pure logic easy to test independently from platform concerns.

## 6) Style and Readability

- Follow `rustfmt` defaults; do not manually format against tooling.
- Use descriptive names over abbreviations (except domain-standard terms like `pc`, `sp`, `ppu`).
- Prefer explicit types when inference hurts readability.
- Add comments only when intent is not obvious from code.
- Document public APIs with `///` docs, including error behavior when relevant.

## 7) Lints and Static Analysis

- Enable strict Clippy in CI:
  - `-D warnings`
  - `-D clippy::all`
  - `-D clippy::pedantic` (allow narrowly with justification)
- For every `#[allow(...)]`, include a short reason.
- Use `cargo fmt --check`, `cargo clippy`, and `cargo test` as required checks.

## 8) Testing Standards

- Follow a testing pyramid:
  - Unit tests for instruction behavior and state transitions.
  - Integration tests for subsystem interactions.
  - ROM-based verification tests for compatibility.
- Tests must be deterministic and reproducible.
- Use table-driven tests for opcode coverage where practical.
- Add regression tests for every bug fix.
- `unwrap()`/`expect()` are acceptable in tests where failure should abort the test immediately.

## 9) Performance and Determinism

- Preserve cycle correctness and ordering semantics.
- Benchmark before optimizing (`criterion` recommended for microbenchmarks).
- Prefer straightforward optimizations first (data layout, branching, allocations).
- Avoid premature unsafe optimizations.

## 10) Unsafe Code Policy

- `unsafe` is disallowed unless there is a measured, justified need.
- Any `unsafe` block requires:
  - A clear `// SAFETY:` comment documenting invariants.
  - Tests covering boundary behavior.
  - Code review focused on memory and aliasing guarantees.

## 11) Dependency Management

- Keep dependencies minimal.
- Prefer mature, well-maintained crates with clear maintenance history.
- Avoid adding crates for small utilities that are trivial in std.
- Pin and review dependency updates regularly.

## 12) Logging and Diagnostics

- Use structured logging/tracing at subsystem boundaries, not per-cycle hot loops by default.
- Keep debug instrumentation easy to enable/disable via feature flags.
- Ensure diagnostics do not alter emulation behavior.

## 13) CI Gate (Required)

All PRs must pass:

1. `cargo fmt --check`
2. `cargo clippy --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic`
3. `cargo test --all-targets --all-features`

## 14) Code Review Checklist

- No production `unwrap()`/`expect()`.
- Errors are explicit and propagated with context.
- Public APIs are minimal and documented.
- Behavior-changing fixes include regression tests.
- No unjustified `allow` attributes.
- No unjustified `unsafe`.
- Emulator determinism preserved.

## 15) Project-Specific Non-Negotiables

- Zero unnecessary panics in runtime emulator paths.
- Prefer `Result`/`Option` return types over sentinel values.
- Keep hardware behavior explicit, even if verbose.
- Prioritize correctness and compatibility before optimization.
