# Architecture

## Layer Overview

```
CLI (src/cli/) → Orchestrator (src/orchestration/) → Backend (src/backend/)
                                                   → StateManager (src/storage/)
```

- **CLI** parses arguments with clap and delegates to `main.rs::dispatch()`.
- **Orchestrator** coordinates the service lifecycle: install, up, down, env, status. It owns the backend, state manager, and service registry.
- **Backend** trait abstracts Docker vs Native execution. Each backend implements install, start, stop, and is_running.
- **Storage** handles state persistence — JSON files under `~/.grov/store/<grove_id>/`.

## Adding a New Service

1. Create a new module in `src/orchestration/services/` (e.g., `redis.rs`).
2. Define a struct implementing the `Service` trait: `name()`, `docker_image()`, `default_port()`, `env_template()`, `defaults()`, etc.
3. Add the service to `builtin_services()` in `src/orchestration/services/mod.rs`.
4. Add integration test coverage in `tests/`.

## Adding a New Backend

1. Create a new module in `src/backend/` (e.g., `podman.rs`).
2. Implement the `Backend` trait: `install()`, `start()`, `stop()`, `is_running()`, `backend_type()`.
3. Add a `ServiceHandle` variant if needed (currently: `Docker { container_id }`, `Native { pid }`).
4. Wire the backend selection in `main.rs::run()` (keyed off `GROV_BACKEND` env var).
5. Add matching `ServiceHandleState` variant in `src/storage/state.rs` and `From` impls in `src/orchestration/mod.rs`.

## State Management

- `StateManager` manages `~/.grov/store/<grove_id>/state.json`.
- **Atomic writes:** write to `state.json.tmp`, then rename to `state.json`.
- **File locking:** `state.lock` via `fs2::FileExt` for cross-process safety.
- `with_lock(|state| ...)` provides a read-modify-write transaction.

## Stdout Contract

Only `grov env` and `grov status` write to stdout — their output is meant to be machine-readable (e.g., `eval $(grov env)`). All other output (logs, errors, diagnostics) goes to stderr via `tracing` or `eprintln!`.
