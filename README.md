# grov — Dev Workspace Services

A service orchestrator for local development. Manages backing services (Postgres, MinIO) with per-worktree isolation — unique ports, isolated data, automatic environment variable discovery.

```
~/.grov/
└── store/
    ├── a1b2c3d4/        ← grove for worktree-main (hash of path)
    │   └── data/
    │       ├── postgres/
    │       └── minio/
    ├── e5f6g7h8/        ← grove for worktree-feature-a
    │   └── data/
    └── ...
```

Each worktree maps to a **grove** — an isolated environment identified by a hash of the worktree path. All service data lives in `~/.grov/store/<grove-hash>/`.

---

## Install

```bash
brew install mattjmcnaughton/tap/grov
```

---

## Quick Start

```bash
grov install postgres minio    # pull Docker images
grov up postgres minio         # start services for this worktree
eval $(grov env)               # load connection env vars
grov status                    # verify services are running
grov down                      # stop when done
```

---

## CLI Reference

### `grov install <services...>`

Pull Docker images or verify native binaries for the listed services.

```bash
grov install postgres minio
```

### `grov up <services...>`

Start services for the current worktree. Allocates ports, creates data directories, waits for health checks.

```bash
grov up postgres minio         # start specific services
```

Idempotent — running `grov up` when a service is already running skips it.

### `grov down [services...]`

Stop services. Data is preserved. Stops all services if no names are given.

```bash
grov down                      # stop all
grov down postgres             # stop one
```

### `grov env`

Print environment variables for running services as `KEY=VALUE` lines.

```bash
eval $(grov env)               # load into current shell
```

### `grov clean [--all] [--orphans] [--dry-run]`

Remove grove data directories from `~/.grov/store/`. By default, removes data for the current worktree.

```bash
grov clean                     # remove current grove's data
grov clean --orphans           # remove data for worktrees that no longer exist
grov clean --all               # remove all grove data
grov clean --dry-run           # preview what would be removed
```

### `grov status`

Show service status for the current grove.

```
$ grov status
SERVICE   BACKEND  STATUS   PORT
postgres  docker   running  54321
minio     docker   running  9001
```

Prints `No running services.` when nothing is running.

### Global flags

- `-v` / `-vv` — increase log verbosity (info / debug). Logs go to stderr.

---

## Supported Services

| Service | Docker | Native (Linux) |
|---|---|---|
| **PostgreSQL** 16 | yes | yes |
| **MinIO** | yes | yes |

Service definitions are hardcoded with sensible defaults (no config file yet).

---

## Backends

**Docker** (default) — pulls and runs containers via the Docker daemon. Works on macOS and Linux. Supports DOCKER_HOST, DOCKER_CONTEXT, and Colima.

**Native** — runs postgres and minio as local processes. Linux only. Select with:

```bash
GROV_BACKEND=native grov up postgres
```

---

## Documentation

- [Supported Services](docs/product/supported-services.md)
- [Product Backlog](docs/product/backlog.md)

---

## Development

### Running locally

```bash
just run up postgres minio      # start services (Docker backend)
just run down                   # stop all services
just run-native up postgres     # native backend inside Linux container
```

### Demos

Full lifecycle demos (install, up, verify, down, restart):

```bash
bash examples/demo-docker.sh    # Docker backend on your machine
bash examples/demo-native.sh    # Native backend inside Linux container
```

### Quality gates

```bash
just gate              # fmt + clippy + unit tests
just gate-expensive    # gate + integration tests
just test-native       # native backend tests in Linux container
```

---

## Notes

- **No root required** — all commands run without sudo. Exception: `grov install` with the native backend on Linux may need elevated permissions.
- **Localhost-only binding** — all services bind to `127.0.0.1`.
- **Data persistence** — `grov down` preserves data directories. Restarting with `grov up` reuses the same data path.
