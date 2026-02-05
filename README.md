# grov — Dev Workspace Services

A service orchestrator for local development. Manages backing services (Postgres, Redis, MinIO, DynamoDB) with per-worktree isolation — unique ports, isolated data, automatic environment variable discovery.

Available as a **CLI** for humans and a **programmatic API** for tools, scripts, and AI coding agents.

```
repo/
├── worktree-main/
├── worktree-feature-a/
├── worktree-feature-b/
└── grov.toml            ← shared service definitions

~/.grov/
├── config.toml          ← global grov configuration (optional)
└── store/
    ├── a1b2c3d4/        ← grove for worktree-main (hash of path)
    │   └── data/
    │       ├── postgres/
    │       └── redis/
    ├── e5f6g7h8/        ← grove for worktree-feature-a
    │   └── data/
    └── ...
```

Each worktree maps to a **grove** — an isolated environment identified by a hash of the worktree path. All service data lives in `~/.grov/store/<grove-hash>/`.

---

## Install

```bash
brew install grov
# or
cargo install grov
```

---

## Quick Start

```bash
grov init                    # create grov.toml interactively
grov up                      # start services for this worktree
eval $(grov env)             # load connection env vars
grov down                    # stop when done
```

---

## Two Interfaces

### 1. CLI — for humans

The primary interface. All functionality is available here.

```bash
grov up
grov status
grov env --format dotenv > .env
grov exec postgres -- psql -c "SELECT 1"
grov down
```

### 2. Programmatic API — for tools, scripts, CI, IDE plugins, AI agents

Every CLI command supports `--json` for machine-readable output. For long-lived integrations (IDE plugins, dashboards), an optional HTTP API server exposes the same functionality over REST.

```bash
# JSON output from CLI (no server needed)
grov status --json
grov env --format json
grov ps --json

# Or start the HTTP API for richer integrations
grov api start --port 9100
curl http://localhost:9100/v1/groves/a1b2c3d4/status
curl -X POST http://localhost:9100/v1/groves/a1b2c3d4/up
```

The CLI and API are designed to be easily consumable by AI coding agents — structured JSON output, predictable exit codes, and `--quiet` mode for suppressing noise.

---

## CLI Reference

### Core Lifecycle

#### `grov init`

Interactive manifest creation. Writes `grov.toml`.

```bash
$ grov init
? Which services does your project need?
  [x] postgres
  [x] redis
  [ ] minio
  [x] dynamodb
? Default backend? docker
Wrote grov.toml
```

#### `grov up [services...] [--profile <p>] [--quiet]`

Start services for the current worktree. Allocates ports and creates data directories.

```bash
grov up                      # start everything
grov up postgres redis       # start specific services
grov up --profile test       # start with test profile
grov up --quiet              # suppress output (for scripting)
```

Idempotent — running `grov up` when services are already running is a no-op.

#### `grov down [services...] [--clean]`

Stop services. Data is preserved unless `--clean` is passed.

```bash
grov down                    # stop all
grov down postgres           # stop one
grov down --clean            # stop and delete data
```

#### `grov env [--profile <p>] [--format <f>]`

Print environment variables for service discovery.

```bash
eval $(grov env)                     # shell eval (auto-detects shell)
grov env --format dotenv > .env      # write .env file
grov env --format json               # machine-readable
grov env --format fish               # fish shell
grov env --format powershell         # PowerShell
grov env --profile test              # test profile env vars
```

#### `grov status [--json]`

Service status for the current worktree.

```
$ grov status
Grove: a1b2c3d4 (feature-auth)

SERVICE     BACKEND   STATUS    PORT    DATA DIR
postgres    docker    running   54320   ~/.grov/store/a1b2c3d4/data/postgres
redis       docker    running   63790   ~/.grov/store/a1b2c3d4/data/redis
dynamodb    docker    running   48001   ~/.grov/store/a1b2c3d4/data/dynamodb
minio       docker    stopped   —       ~/.grov/store/a1b2c3d4/data/minio
```

#### `grov ps [--resources] [--json]`

Services across all groves.

```
$ grov ps --resources
GROVE      WORKTREE           SERVICES   PORTS USED   DISK     MEMORY
a1b2c3d4   main               4/4        54320-54323  120MB    ~400MB
e5f6g7h8   feature-auth       4/4        54324-54327  85MB     ~400MB
i9j0k1l2   feature-payments   2/4        54328-54329  40MB     ~200MB
```

#### `grov destroy [--all] [--grove <hash>]`

Full teardown: stop services, remove containers, delete data.

```bash
grov destroy                     # current grove
grov destroy --all               # every grove
grov destroy --grove a1b2c3d4    # specific grove
```

### Data Management

#### `grov reset [services...]`

Wipe data and restart services from scratch.

```bash
grov reset                   # reset everything
grov reset postgres          # reset one service
```

### Utilities

#### `grov exec <service> [-- cmd...]`

Run commands against a service with connection details injected.

```bash
grov exec postgres                       # open psql shell
grov exec postgres -- psql -f init.sql   # run a SQL file
grov exec redis                          # open redis-cli
```

#### `grov port <service>`

Print allocated port for scripting.

```bash
psql -h localhost -p $(grov port postgres) mydb
```

#### `grov install [services...] [--docker] [--all]`

Install service runtimes. On Linux, `--native` is also available.

```bash
grov install --all               # install everything in grov.toml
grov install postgres --native   # force native install (Linux only)
```

#### `grov doctor`

Validate setup, check runtimes, find problems.

```
$ grov doctor
Checking grov.toml...             ✓ valid
Checking Docker...               ✓ Docker 24.0.7
Checking postgres runtime...     ✓ postgres:16-alpine
Checking port conflicts...       ✓ none
```

#### `grov config validate|show`

```bash
grov config validate                     # check grov.toml for errors
grov config show --profile test          # show fully resolved config
```

---

## Programmatic API

### CLI JSON Mode

Every command that produces output supports `--json` for machine-readable results. This is sufficient for most scripting, CI, and automation needs — no server required.

```bash
# Status as JSON
grov status --json | jq '.services.postgres.port'

# Env vars as JSON
grov env --format json | jq -r '.env.DATABASE_URL'

# Process list as JSON
grov ps --json | jq '.groves[] | select(.services.postgres.status == "running")'

# Exit codes for scripting
grov status --check postgres  # exits 0 if running, 1 if not
```

### HTTP API (optional)

For long-lived integrations — IDE plugins, dashboards — `grov` can run an HTTP API server.

```bash
grov api start --port 9100 [--daemon] [--token <secret>]
grov api stop
```

Binds to `127.0.0.1` only. All endpoints under `/v1`, JSON request/response bodies.

#### Endpoints

All grove-specific endpoints are scoped by grove hash in the path:

```
# Lifecycle
POST   /v1/groves/{grove}/up        Start services
POST   /v1/groves/{grove}/down      Stop services
POST   /v1/groves/{grove}/reset     Reset data
POST   /v1/groves/{grove}/destroy   Full teardown

# Status & Environment
GET    /v1/groves/{grove}/status    Service status
GET    /v1/groves/{grove}/env       Environment variables
GET    /v1/groves/{grove}/config    Resolved configuration

# Execution
POST   /v1/groves/{grove}/exec/{service}  Run command against service

# Global
GET    /v1/groves                   List all groves
GET    /v1/health                   API server health
GET    /v1/doctor                   Run diagnostics
```

---

## Configuration: `grov.toml`

Lives in the repo root. Shared across worktrees.

```toml
[workspace]
port_strategy = "dynamic"       # "dynamic" | "deterministic"
default_profile = "dev"

# -------------------------------------------------------------------
# Services
# -------------------------------------------------------------------

[services.postgres]
version = "16"
backend = "docker"
image = "postgres:16-alpine"

[services.postgres.config]
databases = ["myapp_dev"]
extensions = ["uuid-ossp", "pgcrypto"]
username = "dev"
password = "dev"

[services.postgres.env]
DATABASE_URL = "postgresql://{username}:{password}@localhost:{port}/{databases[0]}"
PGHOST = "localhost"
PGPORT = "{port}"
PGUSER = "{username}"
PGPASSWORD = "{password}"
PGDATABASE = "{databases[0]}"


[services.redis]
version = "7"
backend = "docker"
image = "redis:7-alpine"

[services.redis.env]
REDIS_URL = "redis://localhost:{port}/0"


[services.minio]
version = "latest"
backend = "docker"
image = "minio/minio:latest"

[services.minio.config]
root_user = "minioadmin"
root_password = "minioadmin"
buckets = ["uploads", "exports"]

[services.minio.env]
MINIO_ENDPOINT = "http://localhost:{port}"
AWS_ACCESS_KEY_ID = "{root_user}"
AWS_SECRET_ACCESS_KEY = "{root_password}"
AWS_S3_ENDPOINT = "http://localhost:{port}"
AWS_S3_FORCE_PATH_STYLE = "true"


[services.dynamodb]
version = "latest"
backend = "docker"
image = "amazon/dynamodb-local:latest"

[services.dynamodb.env]
DYNAMODB_ENDPOINT = "http://localhost:{port}"
AWS_ACCESS_KEY_ID = "local"
AWS_SECRET_ACCESS_KEY = "local"
AWS_DEFAULT_REGION = "us-east-1"

# -------------------------------------------------------------------
# Profiles
# -------------------------------------------------------------------

[profiles.test.postgres.config]
databases = ["myapp_test"]
```

---

## Composing with `just`

`grov` manages services. Your task runner manages the developer workflow.

```just
# justfile

init:
    uv sync
    grov install --all
    grov up
    grov env --format dotenv > .env
    @echo "Ready. Run: just dev"

dev:
    grov up --quiet
    eval $(grov env) && uvicorn app.main:app --reload

test:
    grov up --quiet --profile test
    eval $(grov env --profile test) && pytest

reset:
    grov reset
    grov env --format dotenv > .env

db:
    grov exec postgres

doctor:
    grov doctor
    uv run python -c "import app; print('ok')"

clean:
    grov destroy
    rm -rf .venv .env
```

---

## Supported Services

| Service | Docker | Native (Linux only) | Notes |
|---|---|---|---|
| **PostgreSQL** | ✓ | ✓ | `initdb` for isolated clusters |
| **Redis** | ✓ | ✓ | `--dir` for data isolation |
| **MinIO** | ✓ | ✓ | Positional arg for data dir |
| **DynamoDB Local** | ✓ | ✓ | `-dbPath` + `-sharedDb`, needs JRE |

Adding new services is composable — any Docker image or binary that accepts a port flag can be configured.

---

## Notes

- **No root required** — `grov up`, `grov down`, and all other commands never need sudo. The only exception is `grov install --native` on Linux, which may need elevated permissions for system package managers.
- **Localhost-only binding** — all services bind to `127.0.0.1`. No accidental network exposure.
