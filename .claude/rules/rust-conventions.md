---
paths:
  - "**/*.rs"
---

# Rust Conventions

## Error Handling

- Use `thiserror` for all library error types. Define errors in the module that owns them.
- `anyhow` is only used at the top level (`main.rs`) for ad-hoc conversions. Library code never returns `anyhow::Result`.
- Error hierarchy: `GrovError` wraps `BackendError`, `StorageError`, and `HealthCheckError` via `#[from]`.
- Each error variant has a human-readable `#[error("...")]` message.

## Async

- Runtime: tokio with `features = ["full"]`.
- The `Backend` trait uses RPITIT (return-position impl trait in traits) — no `async_trait` macro. Methods return `impl Future<Output = ...> + Send`.
- Async tests use `#[tokio::test]`.

## Import Order

1. `std::` standard library
2. External crates (alphabetical)
3. `crate::` internal modules

Separate each group with a blank line.

## Naming

- `snake_case` for functions and variables
- `CamelCase` for types and traits
- `UPPER_CASE` for constants
- `is_*` prefix for boolean predicates

## Formatting & Linting

- `cargo fmt` — run before committing
- `cargo clippy -- -D warnings` — all warnings are errors
- Both are checked by `just gate`

## Patterns

- **Builder pattern for test mocks:** Mock structs use `with_*` methods to configure behavior (e.g., `MockBackend::new().with_is_running(true)`).
- **Atomic state writes:** Use tmp file + rename pattern for crash safety (see `StateManager::save_state`).
- **File locking:** Use `fs2::FileExt` for cross-process state coordination.
