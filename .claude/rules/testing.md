---
paths:
  - "tests/**/*.rs"
  - "src/**/*test*"
---

# Testing Conventions

## Unit Tests

- Placed in `#[cfg(test)] mod tests` at the bottom of each source file.
- Import the parent module with `use super::*`.
- Run with `just test` or `cargo test --lib`.

## Integration Tests

- Located in the `tests/` directory.
- Feature-gated with `#[cfg(feature = "integration-tests")]`.
- Run with `just gate-expensive` (includes `cargo test --test '*' --features integration-tests`).

## Test Fixture: TestGrove

Defined in `tests/common/mod.rs`. Provides:

- `TestGrove::new()` — creates a temp directory, computes a grove ID from the canonicalized path.
- `grove.cmd()` — returns an `assert_cmd::Command` for the `grov` binary with cwd set to the temp dir.
- `TestGrove::parse_env_output(stdout)` — parses `KEY=VALUE` lines into a HashMap.
- `grove.store_path()` — path to `~/.grov/store/{grove_id}/`.
- **Automatic cleanup on drop:** stops and removes Docker containers matching the grove prefix, deletes the state directory.

## CLI Testing

- Use `assert_cmd::Command` to invoke the `grov` binary.
- Use the `predicates` crate for stdout/stderr assertions.
- Example: `grove.cmd().args(["up", "postgres"]).assert().success();`

## Async Tests

- Use `#[tokio::test]` for async test functions.

## Mock Pattern

- Define a `MockBackend` struct implementing `Backend` in the test module.
- Use `Arc<Mutex<Vec<...>>>` fields to record calls (installed, started, stopped).
- Configure with builder methods: `with_is_running(bool)`, `with_listener()`.
- The mock bind a TCP listener to pass health checks when `with_listener()` is used.

## Commands

| Command | What it runs |
|---------|-------------|
| `just test` | Unit tests (`cargo test --lib`) |
| `just gate` | fmt + clippy + unit tests |
| `just gate-expensive` | gate + integration tests + native backend tests |
| `just test-native` | Native backend tests in a Linux container |
