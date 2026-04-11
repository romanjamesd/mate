# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the application and library code. Core areas are split by concern: `src/chess/` for board and move logic, `src/cli/` for command parsing and UX, `src/network/` for peer connections, `src/messages/` for wire and chess message types, `src/crypto/` for identity handling, and `src/storage/` for SQLite-backed persistence. The binary entry point is `src/main.rs`; shared exports live in `src/lib.rs`. Test code is under `tests/`, organized as `unit/`, `integration/`, `security/`, `performance/`, plus shared helpers in `tests/common/`. Utility scripts live in `scripts/`.

## Build, Test, and Development Commands
Run commands from the repository root:

- `cargo build` builds the crate and CLI binary.
- `cargo test` runs the full local test suite.
- `make check` runs formatting and Clippy checks for quick validation.
- `make ci` runs the same `fmt`, `clippy`, and CI-style test flow used in automation.
- `make test-ci-safe` runs tests single-threaded for debugging race-sensitive failures.

Use `cargo test timeout` or similar filters when iterating on a focused area.

## Coding Style & Naming Conventions
Use standard Rust formatting with 4-space indentation and keep code `cargo fmt` clean. Treat Clippy warnings as errors; `make clippy` matches CI settings. Follow existing Rust naming: modules and functions in `snake_case`, types and traits in `CamelCase`, constants in `SCREAMING_SNAKE_CASE`. Keep modules narrowly scoped by feature area and prefer explicit imports over wildcard imports.

## Testing Guidelines
Add tests beside the relevant category under `tests/`, not inline in production modules unless there is a strong reason. Name test files by behavior, such as `connection_recovery.rs`, and use descriptive `test_*` function names. Cover both happy paths and failure cases, especially around networking, storage, and signed message handling. Run `cargo test -- --nocapture` when debugging and use `make test-ci-safe` before merging changes that touch shared state or timing-sensitive code.

## Commit & Pull Request Guidelines
Recent history favors short, imperative, lowercase subjects such as `fixes clippy errors` and `adjusts perf expectation for binary vs json in CI`. Keep commits focused and easy to scan. PRs should explain the behavioral change, call out test coverage, and link the relevant issue when one exists. Include terminal output or logs when changing CLI behavior or CI/debugging flows; screenshots are only useful for rendered output changes.
