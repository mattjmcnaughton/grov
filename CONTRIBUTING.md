# Contributing to grov

## Prerequisites

- **Rust 1.85+** (edition 2024)
- **Docker** (required for integration tests and the default Docker backend)
- **[just](https://github.com/casey/just)** — command runner

## Setup

```bash
git clone https://github.com/mattjmcnaughton/grov.git
cd grov
cargo build
```

## Development Loop

```bash
# edit code...
just gate          # fmt + clippy + unit tests — run before every push
git push
```

For the full test suite including integration tests:

```bash
just gate-expensive    # gate + integration tests + native backend tests
```

## Testing

| Command | What it runs |
|---------|-------------|
| `just test` | Unit tests (`cargo test --lib`) |
| `just gate` | Format check + Clippy + unit tests |
| `just gate-expensive` | Full gate + integration + native backend tests |
| `just test-native` | Native backend tests in a Linux container |

## Commit Messages

This project uses [conventional commits](https://www.conventionalcommits.org/) with semantic-release:

- `feat:` — new feature
- `fix:` — bug fix
- `chore:` — maintenance (deps, CI, docs)
- `test:` — test-only changes
- `refactor:` — code restructuring without behavior change

## Project Structure

```
src/
├── main.rs                    # Entry point, backend wiring, CLI dispatch
├── lib.rs                     # GrovError, module exports
├── cli/                       # clap CLI definition
├── orchestration/             # Orchestrator, service lifecycle
│   ├── services/              # Service trait, Postgres, MinIO
│   ├── grove.rs               # Grove ID resolution
│   ├── port.rs                # Port allocation
│   └── env_template.rs        # MiniJinja env var rendering
├── backend/                   # Backend trait, Docker, Native, health checks
└── storage/                   # StateManager, GroveState persistence
tests/
├── common/mod.rs              # TestGrove fixture
├── docker_backend.rs          # Docker integration tests
├── native_backend.rs          # Native backend integration tests
└── e2e_docker.rs              # End-to-end Docker lifecycle tests
```
