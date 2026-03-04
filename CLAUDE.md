# grov

A service orchestrator for local development. Manages backing services (Postgres, MinIO) with per-worktree isolation — unique ports, isolated data, automatic environment variable discovery.

## Tech Stack

- **Language:** Rust 2024 edition
- **Async runtime:** tokio
- **CLI framework:** clap (derive)
- **Docker client:** bollard
- **Error handling:** thiserror (library errors), anyhow (top-level only in main.rs)
- **Templating:** minijinja (env var templates)
- **Serialization:** serde + serde_json

## Build & Test

```bash
just gate              # fmt + clippy + unit tests (fast, run before every push)
just gate-expensive    # gate + integration tests + native backend tests
just test              # unit tests only (cargo test --lib)
just test-native       # native backend tests in Linux container
```

Individual commands:

```bash
cargo fmt --check      # formatting
cargo clippy -- -D warnings  # lints
cargo build            # build
```

## Module Map

| Module | Path | Purpose |
|--------|------|---------|
| **cli** | `src/cli/` | clap CLI definition, command parsing |
| **orchestration** | `src/orchestration/` | Orchestrator, service lifecycle, port allocation, grove ID resolution |
| **backend** | `src/backend/` | Backend trait, DockerBackend, NativeBackend, health checks |
| **storage** | `src/storage/` | StateManager — persists grove state to `~/.grov/store/<grove_id>/state.json` |
| **services** | `src/orchestration/services/` | Service trait, Postgres + MinIO service definitions |

## Architecture

```
CLI (clap) → Orchestrator → Backend (Docker | Native)
                          → StateManager (storage)
```

- `main.rs` wires the backend (Docker or Native via `GROV_BACKEND` env var), creates the Orchestrator, and dispatches commands.
- Each worktree maps to a **grove** — identified by a SHA-256 hash of the worktree path.
- State is persisted as JSON with atomic writes (tmp + rename) and file locking (`fs2`).

## Key Constraints

- **stdout is for user output only.** Only `grov env` and `grov status` write to stdout. All logs and errors go to stderr.
- **Localhost-only binding.** All services bind to `127.0.0.1`.
- **No root required.** All commands run without sudo (exception: `grov install` with native backend on Linux).

## Error Hierarchy

```
GrovError
├── Backend(BackendError)     — Docker/native failures
├── Storage(StorageError)     — I/O, serialization, locking
├── HealthCheck(HealthCheckError) — service health timeouts
├── UnknownService            — invalid service name
├── AlreadyRunning            — service already started
├── Internal                  — catch-all
└── Interrupted               — SIGINT/Ctrl+C
```

## Further Reading

- `.claude/rules/` — code conventions (Rust, testing, architecture)
- `docs/technical/` — TDD and tickets
- `docs/product/` — PRD, product brief, backlog
