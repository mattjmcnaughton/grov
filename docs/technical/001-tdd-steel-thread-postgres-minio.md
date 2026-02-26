# TDD: Grov Steel Thread - Launch Postgres and MinIO

## Meta

| Field | Value |
|-------|-------|
| Author | mattjmcnaughton |
| Status | Draft |
| PRD Reference | docs/product/001-prd-steel-thread-postgres-minio.md |
| Date | 2026-02-06 |
| Reviewers | TBD |

## Overview

### Technical Summary

Implement grov's core architecture as a Rust async CLI using a layered module design (CLI, orchestration, backend, storage) with trait-based service and backend abstractions. The steel thread validates end-to-end integration by launching Postgres and MinIO with grove-isolated ports, data directories, and environment variables, using both Docker (via bollard) and native process backends.

### Background

Grov is a greenfield project with no existing implementation -- `src/main.rs` contains only a placeholder `Hello, world!` and `Cargo.toml` declares zero dependencies. The README describes a full-featured service orchestrator, but no code exists. This steel thread establishes every foundational pattern: module structure, error handling, async runtime, service abstraction, backend trait, state persistence, and CLI command routing. Every decision here becomes the convention for all future grov development.

### Goals

- Establish the crate structure (library + binary with layered modules) that all future code follows
- Define a `ServiceDefinition` trait/struct and `Backend` trait that make adding new services (Redis, Elasticsearch, DynamoDB) trivial
- Implement grove identification, port allocation, data directory isolation, and environment variable templating
- Deliver working `grov install`, `grov up`, `grov down`, `grov env`, and `grov status` commands for Postgres and MinIO
- Validate Docker backend (macOS + Linux) and native backend (Linux only)
- Prove that two groves can run the same service simultaneously without conflict

### Non-Goals

- `grov.toml` parsing or any configuration file support (all service definitions are hardcoded)
- Multiple profiles
- Commands beyond install/up/down/env/status (no init, exec, destroy, reset, ps, port, doctor, config)
- HTTP API server
- `--json` output flag or `--quiet` flag
- Services beyond Postgres and MinIO
- Deterministic port allocation
- Custom credentials or database names
- Extension installation for Postgres
- Container networking or linking

### CLI Contracts

#### Exit Codes

| Code | Meaning | Example |
|------|---------|---------|
| 0 | Success | `grov up` completed, `grov status` ran, `grov down` stopped services |
| 1 | Runtime error | Docker unavailable, health check timeout, backend start failure |
| 2 | Usage error | Unknown command, invalid arguments, unknown service name |

Exit codes are part of the CLI's public contract. Scripts and tool integrations (justfiles, CI, IDE plugins) depend on these values. They must not change without a major version bump.

#### Output Format Contract

`grov env` outputs bare `KEY=VALUE` pairs to stdout, one per line, with no quoting, no `export` prefix, and no comments. This format is compatible with:
- `eval $(grov env)` in bash/zsh
- dotenv file consumers (write to `.env` via `grov env > .env`)
- Line-by-line parsing in scripts

Example output:
```
DATABASE_URL=postgresql://dev:dev@localhost:54321/myapp_dev
PGHOST=localhost
PGPORT=54321
PGUSER=dev
PGPASSWORD=dev
PGDATABASE=myapp_dev
MINIO_ENDPOINT=http://localhost:9001
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
```

All other commands (`up`, `down`, `status`, `install`) output human-readable text to stdout. Errors go to stderr. This separation ensures `eval $(grov env)` never captures error messages. The `--json` flag is out of scope for the steel thread, but the stdout/stderr discipline established here makes it a straightforward addition later.

#### Logging and Diagnostics

Structured logging via `tracing` + `tracing-subscriber`. Log output goes exclusively to stderr so it never contaminates stdout command output (critical for `eval $(grov env)`).

- **Default verbosity**: WARN level. Users see errors and warnings only.
- **`-v` flag**: INFO level. Shows operational progress ("starting postgres on port 54321", "health check passed").
- **`-vv` flag**: DEBUG level. Shows internal details ("binding to port 0", "container abc123 created", "TCP connect attempt 3").
- **`RUST_LOG` env var**: Overrides the `-v` flag for fine-grained control (e.g., `RUST_LOG=grov::backend::docker=trace`).
- **No log files**: All output to stderr. Users redirect if they want files.

The `tracing` crate is chosen over `log` + `env_logger` because it provides structured spans that are valuable for debugging async operations (e.g., correlating log lines from concurrent health checks in future multi-service concurrent startup).

### Concurrency and State Safety

#### File Locking

When two terminals run `grov up` for the same grove simultaneously (or `grov up` in one and `grov down` in another), both processes read and write `state.json`. Without coordination, the second writer overwrites the first's changes.

**Strategy**: Advisory file locking via the `fs2` crate on `~/.grov/store/<grove>/state.lock`.

```rust
impl StateManager {
    pub fn with_lock<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&mut GroveState) -> Result<T, StorageError>,
    {
        let lock_file = File::create(self.store_path.join("state.lock"))?;
        lock_file.lock_exclusive()?;  // blocks until lock acquired
        let mut state = self.load_state_unlocked()?;
        let result = f(&mut state)?;
        self.save_state_unlocked(&state)?;
        lock_file.unlock()?;
        Ok(result)
    }
}
```

- The lock is held for the duration of a state read-modify-write cycle, not for the entire `grov up` operation. This means the lock is held briefly (microseconds for JSON read/write), not for the 30+ seconds of service startup.
- The orchestration layer acquires the lock, reads state, updates the in-memory representation, writes state, and releases the lock at each state transition (after each service starts/stops).
- If a grov process crashes while holding the lock, the OS releases the advisory lock automatically.
- The `fs2` crate provides cross-platform file locking (flock on Unix, LockFileEx on Windows).

#### Signal Handling

When the user sends SIGINT (Ctrl+C) or SIGTERM during `grov up`:

- **Behavior**: Any services already started remain running. State is saved with whatever services successfully started. The interrupted operation exits with code 130 (128 + SIGINT signal number), following Unix convention.
- **Rationale**: Leaving services running matches Docker Compose behavior and avoids cleanup complexity. The user can run `grov down` to stop services or `grov up` again to continue starting the remaining ones.
- **Implementation**: Use `tokio::signal::ctrl_c()` to detect SIGINT. When received during the service startup loop, break out of the loop, save current state, and exit. No rollback of already-started services.
- **State consistency**: Because state is saved after each individual service starts (not batched at the end), a SIGINT between service startups leaves state accurate -- it reflects exactly which services are running.

## System Context

### Current Architecture

There is no current architecture. The project is an empty Rust binary crate with edition 2024. This TDD defines the initial architecture from scratch.

### Affected Components

| Component | Current Role | Impact of Change |
|-----------|-------------|-----------------|
| src/main.rs | Placeholder `Hello, world!` | Becomes thin async entry point: parse CLI args, call into library, handle exit code |
| src/lib.rs | Does not exist | New: crate root declaring `cli`, `orchestration`, `backend`, `storage` modules |
| Cargo.toml | Empty dependencies | Adds clap, tokio, bollard, serde, serde_json, thiserror, anyhow, sha2, directories, which, tracing, tracing-subscriber, fs2 |

### External Dependencies

- **Docker daemon**: Required for Docker backend. Accessed via bollard crate (Rust Docker API client). No minimum version constraint for the steel thread, but targets Docker Engine API v1.40+.
- **PostgreSQL native binaries** (`initdb`, `pg_ctl`): Required for native backend on Linux. Discovered via PATH lookup using `which` crate. Version 14+ expected.
- **MinIO native binary** (`minio`): Required for native backend on Linux. Discovered via PATH lookup. No version constraint.
- **Operating system**: Docker backend targets macOS and Linux. Native backend targets Linux only.

### Rust Crate Dependencies

| Crate | Purpose | Category |
|-------|---------|----------|
| clap (derive) | CLI argument parsing | CLI |
| tokio (full) | Async runtime, process spawning, signal handling | Runtime |
| bollard | Docker Engine API client | Backend |
| serde + serde_json | State serialization/deserialization | Storage |
| thiserror | Typed library error definitions | Error handling |
| anyhow | Ergonomic error propagation in CLI layer | Error handling |
| sha2 | SHA-256 for grove ID hashing | Orchestration |
| directories | Cross-platform home directory resolution | Storage |
| which | Native binary PATH lookup | Backend |
| tracing | Structured logging facade with spans | Observability |
| tracing-subscriber | Stderr log output, RUST_LOG filtering | Observability |
| fs2 | Cross-platform advisory file locking | Storage |

Dev dependencies: `tempfile`, `assert_cmd`, `predicates`, `tokio-test`.

## Architecture

### High-Level Design

The system follows a four-layer architecture where each layer has a single responsibility and communicates through well-defined Rust types:

```
src/
  main.rs              # Entry point: parse CLI, call lib, exit code
  lib.rs               # pub mod cli, orchestration, backend, storage

  cli/
    mod.rs             # Cli struct (Parser), Commands enum, dispatch(), global flags (-v)
    commands/
      mod.rs           # Re-exports command handlers
      install.rs       # grov install: resolve services, call backend.install()
      up.rs            # grov up: resolve grove, allocate ports, start services, health check
      down.rs          # grov down: load state, stop services, update state
      env.rs           # grov env: load state, template env vars, print KEY=VALUE to stdout
      status.rs        # grov status: load state, verify liveness, print table

  orchestration/
    mod.rs             # Orchestrator struct, public methods: install/up/down/env/status
    grove.rs           # resolve() -> GroveId: SHA-256 hash of cwd, truncate to 16 hex chars
    port.rs            # allocate() -> u16: bind to port 0, record OS-assigned port, close
    data_dir.rs        # ensure_data_dir() -> PathBuf: create ~/.grov/store/<grove>/data/<svc>/
    env_template.rs    # render(): substitute {port}, {username}, etc. in template strings
    service.rs         # ServiceDefinition struct, builtin_services() -> Vec<ServiceDefinition>

  backend/
    mod.rs             # Backend trait (async: install/start/stop/is_running), ServiceHandle enum
    docker.rs          # DockerBackend: bollard client, container CRUD, port/volume binding
    native.rs          # NativeBackend: which lookup, process spawn, PID tracking, signal send
    health.rs          # wait_until_healthy(): TCP connect poll loop with timeout

  storage/
    mod.rs             # StateManager: new/load_state/save_state/data_dir/ensure_data_dir/with_lock
    state.rs           # GroveState, ServiceState, ServiceHandleState: serde types + JSON I/O

tests/
    docker_backend.rs  # Integration tests: Docker container lifecycle (feature-gated)
    native_backend.rs  # Integration tests: native process lifecycle (feature-gated, Linux only)
    health_check.rs    # Integration tests: TCP health check against real listeners
    e2e_docker.rs      # End-to-end: full grov lifecycle via Docker backend (feature-gated)
    e2e_native.rs      # End-to-end: full grov lifecycle via native backend (feature-gated)
    common/
      mod.rs           # Shared test fixtures: temp directories, test grove setup, cleanup helpers
```

**Control flow**: `main.rs` initializes tracing, parses CLI args via clap, constructs the `Orchestrator` with the appropriate `Backend` and `StateManager`, and calls `cli::dispatch()`. Each command handler calls `Orchestrator` methods which coordinate across the orchestration, backend, and storage layers. All errors propagate up as `anyhow::Result` to `main.rs` which maps them to exit codes.

**Key design principle**: The orchestration layer never knows whether a service is running in Docker or as a native process. The `Backend` trait abstracts this entirely. Similarly, the orchestration layer never knows the specifics of Postgres vs MinIO -- it works with `ServiceDefinition` values that describe any service uniformly.

**Dependency flow** (compile-time enforced by module visibility):
```
main.rs -> cli -> orchestration -> backend
                                -> storage
```
The `cli` layer depends on `orchestration`. The `orchestration` layer depends on `backend` and `storage`. The `backend` and `storage` layers are independent of each other and of `cli`. No circular dependencies. No layer skipping (CLI never calls backend directly).

### Component Design

#### CLI Layer (`src/cli/`)

- **Responsibility**: Parse command-line arguments using clap derive macros, dispatch to command handlers, format human-readable output for the terminal, and map errors to exit codes.
- **Interface**: `Cli` struct with `#[derive(Parser)]` containing a `Commands` enum and global flags (`-v`/`-vv` for verbosity). Each command variant holds its arguments. `main.rs` calls `Cli::parse()`, initializes tracing at the requested verbosity, then calls `cli::dispatch(cli, orchestrator).await`.
- **Key Decisions**:
  - Use clap's derive API (not builder) for compile-time validation and less boilerplate.
  - Command handlers receive parsed arguments and an `Orchestrator` instance (injected from main). They return `anyhow::Result<()>`.
  - Command output to stdout (`println!`), diagnostics and errors to stderr (`eprintln!` and `tracing`). This separation is critical for `eval $(grov env)` correctness.
  - Exit code mapping: `main.rs` catches the `anyhow::Error`, downcasts to `GrovError`, and maps: validation/usage errors -> 2, runtime/backend errors -> 1, success -> 0, SIGINT -> 130. See "CLI Contracts" section.
  - Tracing initialization: `tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(...)` using RUST_LOG env var with fallback to `-v` flag level. See "Logging and Diagnostics" section.

#### Orchestration Layer (`src/orchestration/`)

- **Responsibility**: Core business logic. Resolves grove identity, manages port allocation, creates data directories, templates environment variables, and coordinates service lifecycle by delegating to backends.
- **Interface**: `Orchestrator` struct that holds a `Backend` (trait object), the grove's `StorePath`, and the service registry. Public methods: `install()`, `up()`, `down()`, `env()`, `status()`.
- **Key Decisions**:
  - `Orchestrator` is the single entry point from CLI into business logic. It owns the `Backend` and `StateManager`.
  - Service definitions are loaded from a hardcoded registry function `fn builtin_services() -> Vec<ServiceDefinition>`. Future TOML parsing replaces this function without changing the `Orchestrator` interface.
  - The `Orchestrator` handles idempotency: `up` checks state before starting, `down` checks state before stopping.

#### Service Abstraction (`src/orchestration/service.rs`)

- **Responsibility**: Define what a service is, independent of how it runs. This is the primary extension point for adding new services.
- **Interface**:

```rust
pub struct ServiceDefinition {
    /// Unique service identifier (e.g., "postgres", "minio")
    pub name: String,
    /// Docker image to use (e.g., "postgres:16-alpine")
    pub docker_image: String,
    /// Command to run inside the Docker container (if override needed)
    pub docker_cmd: Option<Vec<String>>,
    /// Environment variables to set on the container/process
    pub process_env: HashMap<String, String>,
    /// Native binary name to look up in PATH (e.g., "pg_ctl")
    pub native_binary: Option<String>,
    /// Function to produce native backend start arguments given resolved config
    pub native_args_fn: Option<fn(&ResolvedService) -> Vec<String>>,
    /// Function to produce native backend init arguments (e.g., initdb for postgres)
    pub native_init_fn: Option<fn(&ResolvedService) -> Option<NativeInitStep>>,
    /// Port the service listens on inside the container (used for Docker port mapping)
    pub default_port: u16,
    /// Volume mount: maps data directory to this path inside the container
    pub docker_data_mount: String,
    /// Environment variable templates for the user (keys are env var names, values
    /// contain {port}, {username}, {password}, etc. placeholders)
    pub env_template: HashMap<String, String>,
    /// Default credentials and config values for template substitution
    pub defaults: HashMap<String, String>,
}

pub struct ResolvedService {
    pub definition: ServiceDefinition,
    pub grove_id: String,
    pub allocated_port: u16,
    pub data_dir: PathBuf,
    pub resolved_env: HashMap<String, String>,
}
```

- **Key Decisions**:
  - `ServiceDefinition` is a plain data struct, not a trait. Adding a new service means adding a new function that returns a `ServiceDefinition` value. No new types, no new trait impls -- just data.
  - The `native_args_fn` and `native_init_fn` fields use function pointers rather than trait objects to keep the struct simple and `Clone`-friendly. Each service provides its own logic for how to construct native process arguments.
  - Template substitution (`{port}`, `{username}`, etc.) is handled by the env_template module using the `defaults` map plus runtime values (allocated port, data dir).
  - This design means adding Elasticsearch, Redis, or any new service requires writing one function (~30 lines) that returns a `ServiceDefinition`. No other code changes needed unless the service has truly novel lifecycle requirements.

#### Backend Layer (`src/backend/`)

- **Responsibility**: Start, stop, and inspect actual service processes. Abstracted behind a trait so Docker and native implementations are interchangeable.
- **Interface**:

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    /// Install runtime dependencies for the service (pull image, check binary exists)
    async fn install(&self, service: &ServiceDefinition) -> Result<(), BackendError>;

    /// Start a service, returning an identifier (container ID or PID)
    async fn start(&self, service: &ResolvedService) -> Result<ServiceHandle, BackendError>;

    /// Stop a running service
    async fn stop(&self, handle: &ServiceHandle) -> Result<(), BackendError>;

    /// Check if a service is currently running
    async fn is_running(&self, handle: &ServiceHandle) -> Result<bool, BackendError>;

    /// Return the backend type name (for display)
    fn backend_type(&self) -> &'static str;
}

pub enum ServiceHandle {
    Docker { container_id: String },
    Native { pid: u32 },
}
```

- **Key Decisions**:
  - `Backend` is a trait with async methods (via `async_trait` macro or Rust native async trait if edition 2024 supports it). The `Orchestrator` holds a `Box<dyn Backend>`.
  - `ServiceHandle` is an enum rather than an opaque string so each backend can store exactly what it needs for stop/inspect operations.
  - The backend does NOT handle health checking. Health checking is a separate concern in `backend::health` that works the same regardless of backend type (TCP connect to localhost:port).
  - Backend selection: The `Orchestrator` constructs the appropriate backend. For the steel thread, Docker is the default. Native is selected if Docker is unavailable or if a future `--native` flag is passed.

#### Docker Backend (`src/backend/docker.rs`)

- **Responsibility**: Manage service lifecycle via Docker containers using the bollard crate.
- **Interface**: Implements `Backend` trait.
- **Key Decisions**:
  - Connect to Docker via bollard's default connection (Unix socket on Linux/macOS). No custom connection configuration for the steel thread.
  - Container naming convention: `grov-{grove_id_short}-{service_name}` (e.g., `grov-a1b2c3d4-postgres`). The grove ID is truncated to 8 characters for readability.
  - Port mapping: Bind allocated host port to the service's `default_port` inside the container, on 127.0.0.1 only.
  - Volume mounting: Mount `{data_dir}` to `{docker_data_mount}` inside the container.
  - On `install`: Pull the Docker image if not already present locally.
  - On `start`: Create and start a container. If a container with the same name already exists and is running, return its ID (idempotency). If it exists but is stopped, remove it and create a new one.
  - On `stop`: Stop the container with a 10-second timeout, then remove it. Data persists because the volume is a bind mount to the host filesystem.

#### Native Backend (`src/backend/native.rs`)

- **Responsibility**: Manage service lifecycle via native OS processes.
- **Interface**: Implements `Backend` trait.
- **Key Decisions**:
  - Use `which` crate to locate binaries in PATH. `install` verifies the binary exists and returns an error with installation instructions if not found.
  - On `start`: Run the init step if needed (e.g., `initdb` for Postgres when the data directory is empty), then spawn the service process via `tokio::process::Command`. Store the PID in the `ServiceHandle`.
  - On `stop`: Send SIGTERM to the process. If it does not exit within 10 seconds, send SIGKILL.
  - Postgres native specifics: `initdb -D {data_dir}` for initialization, `pg_ctl -D {data_dir} -l {data_dir}/logfile -o "-p {port} -k /tmp" start` for starting, `pg_ctl -D {data_dir} stop -m fast` for stopping.
  - MinIO native specifics: `minio server {data_dir} --address :{port}` for starting. No init step needed. Kill process on stop.
  - PID file: Write PID to `{data_dir}/../{service_name}.pid` for recovery after grov process exits. On `is_running`, check if the PID is still alive.

#### Health Check (`src/backend/health.rs`)

- **Responsibility**: Determine when a service is ready to accept connections.
- **Interface**:

```rust
pub async fn wait_until_healthy(
    host: &str,
    port: u16,
    timeout: Duration,
    interval: Duration,
) -> Result<(), HealthCheckError>;
```

- **Key Decisions**:
  - TCP connect only: Attempt `TcpStream::connect` to `127.0.0.1:{port}`. If the connection succeeds, the service is considered healthy.
  - Polling interval: 250ms between attempts.
  - Timeout: 60 seconds (from PRD NFR). Returns `HealthCheckError::Timeout` if exceeded.
  - This is deliberately simple. Protocol-specific checks (pg_isready, HTTP GET) can be added as an enhancement later. TCP connect is sufficient for the steel thread because both Postgres and MinIO accept TCP connections only when ready.

#### Storage Layer (`src/storage/`)

- **Responsibility**: Persist grove state to disk so that `grov status`, `grov env`, and `grov down` can operate after `grov up` has exited.
- **Interface**:

```rust
pub struct StateManager {
    store_path: PathBuf,  // ~/.grov/store/<grove_id>/
}

impl StateManager {
    pub fn new(grove_id: &str) -> Result<Self, StorageError>;
    pub fn load_state(&self) -> Result<GroveState, StorageError>;
    pub fn save_state(&self, state: &GroveState) -> Result<(), StorageError>;
    pub fn with_lock<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: FnOnce(&mut GroveState) -> Result<T, StorageError>;
    pub fn data_dir(&self, service_name: &str) -> PathBuf;
    pub fn ensure_data_dir(&self, service_name: &str) -> Result<PathBuf, StorageError>;
}
```

- **Key Decisions**:
  - State file location: `~/.grov/store/<grove_id>/state.json`.
  - Use the `directories` crate to resolve the home directory cross-platform.
  - Atomic writes: Write to a temporary file then rename, to prevent corruption if grov crashes mid-write.
  - File locking: `with_lock()` acquires an exclusive advisory lock on `state.lock` via the `fs2` crate before reading/modifying/writing state. The lock is held only for the JSON I/O (microseconds), not for service startup. See "Concurrency and State Safety" section for details.
  - Stale state detection: When `load_state` is called, the orchestration layer should verify each service handle is still alive (via `backend.is_running()`). If a service died externally, update state accordingly.

### Data Model

#### State Schema

```rust
#[derive(Serialize, Deserialize)]
pub struct GroveState {
    pub grove_id: String,
    pub services: HashMap<String, ServiceState>,
}

#[derive(Serialize, Deserialize)]
pub struct ServiceState {
    pub service_name: String,
    pub port: u16,
    pub handle: ServiceHandleState,
    pub backend_type: String,  // "docker" or "native"
    pub started_at: String,    // ISO 8601 timestamp
}

#[derive(Serialize, Deserialize)]
pub enum ServiceHandleState {
    Docker { container_id: String },
    Native { pid: u32 },
}
```

Example `state.json`:

```json
{
  "grove_id": "a1b2c3d4e5f6g7h8",
  "services": {
    "postgres": {
      "service_name": "postgres",
      "port": 54321,
      "handle": { "Docker": { "container_id": "abc123def456" } },
      "backend_type": "docker",
      "started_at": "2026-02-06T10:30:00Z"
    },
    "minio": {
      "service_name": "minio",
      "port": 9001,
      "handle": { "Docker": { "container_id": "def456ghi789" } },
      "backend_type": "docker",
      "started_at": "2026-02-06T10:30:01Z"
    }
  }
}
```

#### Schema Changes

| Table/Collection | Change Type | Description |
|-----------------|-------------|-------------|
| `~/.grov/store/<grove>/state.json` | Add (new file) | Grove state tracking running services |
| `~/.grov/store/<grove>/data/<service>/` | Add (new directories) | Per-service data directories |

#### Migration Strategy

- No migration needed -- this is a greenfield project with no existing state files.
- Forward compatibility: `serde_json` deserialization with `#[serde(default)]` on optional fields ensures new fields can be added to `GroveState` without breaking existing state files.
- Rollback: Delete `~/.grov/` directory to remove all state. No data migration path is needed for the steel thread.

## Sequence Flows

### Primary Operation: `grov up postgres minio`

1. **main.rs** initializes tracing subscriber (level from `-v` flags), registers SIGINT handler via `tokio::signal::ctrl_c()`
2. **CLI** parses `["up", "postgres", "minio"]`, extracts service names, validates against service registry (exit 2 if unknown)
3. **CLI** constructs `Orchestrator` with selected `Backend` and `StateManager`
4. **Orchestrator.up()** calls `grove::resolve()` which hashes the current working directory path using SHA-256 and truncates to 16 hex characters
5. **Orchestrator** calls `StateManager::load_state()` to check for already-running services, verifying each via `backend.is_running()`
6. For each requested service not already running (checked against SIGINT flag between iterations):
   a. **Orchestrator** looks up `ServiceDefinition` from the hardcoded registry
   b. **Orchestrator** calls `port::allocate()` which binds to port 0, records the OS-assigned port, and closes the listener
   c. **Orchestrator** calls `StateManager::ensure_data_dir(service_name)` to create `~/.grov/store/<grove>/data/<service>/`
   d. **Orchestrator** constructs `ResolvedService` with allocated port, data dir, and templated env vars
   e. **Orchestrator** calls `backend.start(&resolved_service)` which returns a `ServiceHandle`
   f. **Orchestrator** calls `health::wait_until_healthy("127.0.0.1", port, 60s, 250ms)`
   g. **Orchestrator** calls `StateManager::with_lock()` to record `ServiceState` and persist immediately (state is consistent after each service, not batched)
7. **CLI** prints status table showing service names, ports, and "healthy" status
8. If SIGINT received during step 6, the loop breaks. Already-started services remain running with state saved. CLI exits with code 130.

### Secondary Operation: `grov env`

1. **CLI** parses `["env"]`
2. **Orchestrator.env()** loads `GroveState` from state file
3. For each service in state, **Orchestrator** looks up `ServiceDefinition`, constructs `ResolvedService` using the persisted port and data dir, then evaluates `env_template` with the resolved values
4. **CLI** prints each key=value pair in dotenv format to stdout

### Secondary Operation: `grov status`

1. **CLI** parses `["status"]`
2. **Orchestrator.status()** loads `GroveState` from state file
3. For each service in state, **Orchestrator** calls `backend.is_running(&handle)` to verify liveness
4. If a service is no longer running (external kill), update state to reflect this
5. **CLI** prints status table: SERVICE, BACKEND, STATUS, PORT

### Secondary Operation: `grov down`

1. **CLI** parses `["down"]` (optionally with service names)
2. **Orchestrator.down()** loads `GroveState`
3. For each service to stop: **Orchestrator** calls `backend.stop(&handle)`
4. **Orchestrator** removes the service from `GroveState.services` and saves state
5. Data directories are NOT deleted

### Secondary Operation: `grov install postgres minio`

1. **CLI** parses `["install", "postgres", "minio"]`
2. **Orchestrator.install()** looks up each `ServiceDefinition` from registry
3. For each service: **Orchestrator** calls `backend.install(&definition)`
4. Docker backend: calls `bollard::Docker::create_image()` to pull the image
5. Native backend: calls `which::which(native_binary)` to verify the binary exists; returns error with install instructions if not found
6. **CLI** prints success/failure for each service

### Error Flow: Docker daemon not running

1. **Orchestrator** attempts to construct the Docker backend by calling `bollard::Docker::connect_with_local_defaults()`
2. Connection fails with a bollard connection error
3. **Orchestrator** maps this to `BackendError::DockerUnavailable` with message "Docker daemon is not running. Start Docker and try again."
4. If native backend is available (Linux), **Orchestrator** could fall back. For the steel thread, it returns the error to the CLI.
5. **CLI** prints the error message and exits with code 1

### Error Flow: Port allocation race condition

1. **Orchestrator** calls `port::allocate()` which binds to port 0 and gets port 54321
2. Port 54321 is released when the listener is dropped
3. Between release and Docker container binding, another process takes port 54321
4. `backend.start()` fails because Docker cannot bind to the port
5. **Orchestrator** does NOT retry for the steel thread -- returns `BackendError::PortUnavailable` to the CLI
6. User re-runs `grov up` which allocates a different port

### Error Flow: Health check timeout

1. `backend.start()` succeeds -- container is created and running
2. `health::wait_until_healthy()` polls TCP connect every 250ms
3. After 60 seconds, no connection succeeds
4. Returns `HealthCheckError::Timeout { service: "postgres", port: 54321, elapsed: 60s }`
5. **Orchestrator** stops the service (cleanup), removes from state, and returns the error
6. **CLI** prints "postgres failed to become healthy within 60 seconds" and exits with code 1

## Error Handling and Resilience

### Error Categories

| Category | Example | Handling Strategy | User Impact |
|----------|---------|-------------------|-------------|
| Validation | Unknown service name "postgre" | Return immediately with list of available services | Clear error message, no side effects |
| Configuration | Docker not running | Detect during backend construction, fail fast with actionable message | "Docker daemon is not running. Start Docker and try again." |
| Transient | Port allocated but taken before bind | Fail the current operation. User retries and gets a different port. | "Port 54321 is unavailable. Run `grov up` again to allocate a new port." |
| Infrastructure | Disk full when creating data dir | Fail with OS error context | "Failed to create data directory: No space left on device" |
| Timeout | Health check exceeds 60s | Stop the service, clean up state, report timeout | "postgres failed to become healthy within 60 seconds" |
| Stale state | state.json references dead container | Detect on status/env, update state, report accurate status | Status shows "stopped" instead of stale "running" |

### Error Type Hierarchy

```rust
// Library errors use thiserror for typed errors
#[derive(Debug, thiserror::Error)]
pub enum GrovError {
    #[error("backend error: {0}")]
    Backend(#[from] BackendError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("health check failed: {0}")]
    HealthCheck(#[from] HealthCheckError),
    #[error("unknown service: {name}")]
    UnknownService { name: String },
    #[error("service {name} is already running on port {port}")]
    AlreadyRunning { name: String, port: u16 },
}

#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error("Docker daemon is not running. Start Docker and try again.")]
    DockerUnavailable,
    #[error("port {port} is unavailable")]
    PortUnavailable { port: u16 },
    #[error("failed to start {service}: {reason}")]
    StartFailed { service: String, reason: String },
    #[error("failed to stop {service}: {reason}")]
    StopFailed { service: String, reason: String },
    #[error("Docker error: {0}")]
    Docker(String),
    #[error("native binary not found: {binary}. Install it and ensure it is in PATH.")]
    BinaryNotFound { binary: String },
}

// CLI layer uses anyhow for ergonomic propagation
// main.rs: fn main() -> anyhow::Result<()>
```

### Retry Strategy

- **No automatic retries in the steel thread.** Port allocation races and transient Docker errors are surfaced to the user who can retry manually. This is a deliberate simplicity choice -- automatic retries add complexity around idempotency and partial state that is not warranted for the steel thread.
- **Health check polling** is the one exception: it retries TCP connections every 250ms for up to 60 seconds. This is not a retry of a failed operation but a poll-until-ready pattern.

### Failure Modes

- **Docker daemon unavailable**: `grov up` fails immediately with an actionable message. No services started, no state modified.
- **One service starts, second fails**: The first service remains running and is recorded in state (saved immediately after each service). The error for the second service is reported. User can fix the issue and re-run `grov up` which will skip the already-running service (idempotency) and retry the failed one.
- **SIGINT during `grov up`**: Already-started services remain running. State is accurate because it is saved after each individual service start. Exit code 130 (128 + SIGINT). User can `grov down` or re-run `grov up` to start remaining services.
- **Concurrent `grov up` for same grove**: File locking via `fs2` on `state.lock` prevents lost updates. The second process blocks briefly during state write, then proceeds. Both processes may allocate ports independently, but state will reflect both services correctly.
- **Grov process crashes mid-operation**: State file may be stale. On next `grov status` or `grov up`, the orchestrator verifies each service handle against the actual backend. Docker containers with grov naming convention can be discovered; native process PIDs can be checked. State is corrected.
- **Container killed externally** (`docker kill`): `grov status` detects the container is gone via `backend.is_running()`, updates state to remove it. `grov up` will start a new instance.

## Security Considerations

### Authentication and Authorization

- Services bind to 127.0.0.1 only -- no network exposure beyond the local machine. This is enforced by Docker port binding configuration (`HostIp: "127.0.0.1"`) and native process flags (`-k /tmp` for Postgres, `--address :port` with no external interface for MinIO).
- Hardcoded default credentials (Postgres: dev/dev, MinIO: minioadmin/minioadmin) are acceptable for a local development tool. These are not production credentials.
- No authentication on grov itself -- it runs as the current user with the current user's permissions.

### Data Protection

- All data is stored in the user's home directory (`~/.grov/`). File permissions follow the user's umask.
- No encryption at rest -- this is local development data. The security boundary is the user's file system permissions.
- No PII handling -- grov stores service configuration and connection metadata only.

### Attack Surface and Mitigations

| Threat | Risk | Mitigation |
|--------|------|------------|
| Service exposed to network | M | Bind all services to 127.0.0.1 only. Docker: set HostIp in port binding. Native: use localhost-only flags. |
| Malicious Docker image | L | Use only official images from Docker Hub (postgres:16-alpine, minio/minio:latest). Future: allow pinning image digests. |
| State file tampering | L | State file contains port numbers and container IDs, not credentials. Tampering causes grov to lose track of services, not a security breach. |
| Path traversal in grove ID | L | Grove ID is a SHA-256 hex digest -- contains only [0-9a-f]. No path traversal possible. |

## Performance

### Targets

| Metric | Baseline | Target | Measurement Method |
|--------|----------|--------|-------------------|
| CLI startup (binary load to first useful work) | N/A (greenfield) | < 50ms | Wall clock from exec to first tracing span. Measured by running `grov --help` under `time`. |
| `grov up` (cold start, Docker) | N/A | < 30 seconds (including health check) | Wall clock time from command start to prompt return |
| `grov up` (warm start, images cached) | N/A | < 15 seconds | Wall clock time |
| `grov env` | N/A | < 100ms | Wall clock time (reads state.json only, no Docker calls) |
| `grov status` | N/A | < 2 seconds | Wall clock time (one is_running check per service) |
| `grov down` | N/A | < 15 seconds | Wall clock time (Docker stop timeout is 10s) |
| Health check polling | N/A | 250ms interval, 60s max | Timer within wait_until_healthy |

### Scalability

- The steel thread targets 2 services per grove. The architecture supports more (10+) without design changes since services are started sequentially and state is a simple HashMap.
- The main scaling bottleneck for many services would be sequential startup. Future optimization: start services concurrently with `tokio::join!` or `futures::future::join_all`. The async architecture supports this without structural changes.
- Grove count is unbounded. Each grove has its own state.json and data directories. No cross-grove coordination is needed.

### Optimization Approach

- **No premature optimization for the steel thread.** Sequential service startup is acceptable for 2 services. Concurrent startup is deferred to a future enhancement.
- **Image caching**: Docker images are pulled once and reused. `grov install` is separated from `grov up` to allow pre-pulling images. `grov up` does NOT pull images -- it fails if the image is not present.
- **State file reads**: `grov env` and `grov status` only read state.json and do not contact Docker unless verifying liveness. This keeps read-only commands fast.

## Testing Strategy

### Test Layout and Organization

```
src/
  cli/mod.rs             # #[cfg(test)] mod tests { ... }  -- inline unit tests
  orchestration/grove.rs # #[cfg(test)] mod tests { ... }  -- inline unit tests
  orchestration/port.rs  # #[cfg(test)] mod tests { ... }  -- inline unit tests
  ...                    # (each module has inline #[cfg(test)] unit tests)

tests/
  common/
    mod.rs               # Shared test helpers: temp dir setup, grove fixtures, cleanup
  docker_backend.rs      # Docker backend integration tests (feature-gated)
  native_backend.rs      # Native backend integration tests (feature-gated, Linux)
  health_check.rs        # Health check integration tests (feature-gated)
  e2e_docker.rs          # Full lifecycle end-to-end via Docker (feature-gated)
  e2e_native.rs          # Full lifecycle end-to-end via native (feature-gated, Linux)

Cargo.toml:
  [features]
  default = []
  integration-tests = []  # gates tests requiring Docker or native binaries
```

**Convention**: Unit tests live inline in `#[cfg(test)]` modules within each source file. Integration and end-to-end tests live in `tests/`. All tests requiring Docker or native binaries are gated behind the `integration-tests` Cargo feature so `cargo test` (without features) always passes on any machine.

### Unit Tests (inline, no external deps)

All unit tests run without Docker, network, or filesystem side effects (use temp dirs where needed).

- **grove.rs**:
  - Hash determinism: same path always produces same grove ID
  - Hash isolation: different paths produce different IDs (including paths that differ by one character)
  - Truncation: output is exactly 16 hex characters
  - Character set: output contains only `[0-9a-f]`
  - Absolute path handling: relative paths are resolved before hashing

- **port.rs**:
  - Allocated port is in valid range (1024-65535)
  - Two sequential allocations return different ports
  - Allocation does not hold the port after returning (port is released)

- **env_template.rs**:
  - All placeholders substituted: `{port}`, `{username}`, `{password}`, `{database}` produce correct output
  - Missing placeholder key: returns error (not silent empty string)
  - No placeholders: template passes through unchanged
  - Nested or malformed braces: handled gracefully (literal `{` if not a valid key)
  - Template with multiple occurrences of same placeholder

- **service.rs**:
  - `builtin_services()` returns exactly 2 services (postgres, minio)
  - Postgres definition has correct docker_image, default_port (5432), env_template keys
  - MinIO definition has correct docker_image, default_port (9000), env_template keys
  - `ResolvedService` construction populates `resolved_env` correctly from templates + defaults + runtime values

- **state.rs**:
  - Serialization/deserialization roundtrip: write then read produces identical `GroveState`
  - Deserialization with missing optional fields: `#[serde(default)]` fills defaults
  - Deserialization of future-schema state: unknown fields are ignored (forward compat)
  - Atomic write: write to temp + rename (verified by checking file exists after write)
  - Empty state: `GroveState` with no services serializes/deserializes correctly

- **cli/mod.rs**:
  - Clap parsing: `["up", "postgres", "minio"]` produces correct command variant with service list
  - Clap parsing: `["down"]` with no services produces correct variant
  - Clap parsing: unknown command produces usage error
  - Clap parsing: `-v` and `-vv` flags parsed correctly
  - Exit code mapping: `GrovError::UnknownService` maps to exit code 2, `BackendError` maps to 1

### Integration Tests (feature-gated, require external deps)

All integration tests are gated: `#[cfg(feature = "integration-tests")]`. They require Docker (for Docker tests) or native binaries (for native tests, Linux only).

- **docker_backend.rs** (requires Docker daemon):
  - Pull `alpine:latest` (small image), verify it exists locally via bollard list
  - Create and start a container with port binding (127.0.0.1:allocated_port -> 80), verify running via bollard inspect
  - Stop and remove a container, verify it no longer exists
  - Idempotent start: start same-named container twice, verify only one exists
  - Container naming: verify name matches `grov-{grove_id_short}-{service_name}` convention
  - Volume mount: create temp dir, mount into container, write file from container, verify file exists on host

- **native_backend.rs** (requires pg_ctl or minio, Linux CI only):
  - `which` finds `pg_ctl` when Postgres is installed, returns `BinaryNotFound` when not
  - Postgres: `initdb` initializes a data directory, `pg_ctl start` launches process, PID file written, `pg_ctl stop` terminates process
  - MinIO: process spawns and binds to specified port, PID recorded, SIGTERM stops process
  - PID file: PID written to expected path, `is_running` returns true for live process, false after stop

- **health_check.rs**:
  - Bind a TCP listener on a known port, verify `wait_until_healthy` returns Ok within 1 second
  - No listener on a port, verify `wait_until_healthy` returns `HealthCheckError::Timeout` after specified timeout (use short timeout for test speed, e.g., 2 seconds)
  - Listener starts after a delay (500ms), verify `wait_until_healthy` succeeds (tests the polling loop)

- **state persistence** (no feature gate, uses temp dirs):
  - Write state to temp dir, read it back, verify equality
  - `with_lock`: two threads attempt concurrent state updates, verify both writes are reflected (no lost update)
  - Stale lock: simulate process crash (drop lock file handle), verify lock is released and next acquisition succeeds

### End-to-End Tests (feature-gated, full binary integration)

These tests exercise the full binary by invoking the compiled `grov` binary via `std::process::Command` or by calling library entry points with real backends.

- **e2e_docker.rs** (requires Docker):
  - **Full lifecycle**: `install postgres minio` -> `up postgres minio` -> verify services healthy via TCP connect -> `env` produces valid `DATABASE_URL` and `MINIO_ENDPOINT` -> `status` shows both running and healthy -> `down` -> `status` shows no running services -> data directories still exist on disk
  - **Cross-grove isolation**: Run `up postgres` from two different temp directories, verify both instances run simultaneously on different ports with different data directories, both independently stoppable via `down`
  - **Idempotency**: Run `up postgres` twice, verify only one container exists, second invocation returns success with no side effects
  - **Data persistence**: `up postgres` -> connect and create a table -> `down` -> `up postgres` again -> verify table still exists
  - **Exit codes**: verify `up` with unknown service exits 2, `up` with Docker stopped exits 1, successful `up` exits 0
  - **Env output format**: verify `env` output is parseable as KEY=VALUE, one per line, no export prefix, no comments

- **e2e_native.rs** (requires native binaries, Linux only):
  - **Full lifecycle**: same as Docker lifecycle but using native backend
  - **Postgres init**: verify `initdb` runs on first `up`, does not re-run on subsequent `up`

### Test Fixtures and Helpers (`tests/common/mod.rs`)

- `TestGrove`: Creates a unique temp directory per test, returns a path that produces a deterministic grove ID. Cleans up on drop (removes temp dir, stops any containers with matching grove prefix).
- `assert_port_open(port)`: TCP connect to localhost:port, assert success.
- `assert_port_closed(port)`: TCP connect to localhost:port, assert failure.
- `wait_for_cleanup(grove_id)`: After `down`, poll until containers with grove prefix are removed (handles async Docker cleanup).

## Quality Gates and CI Pipeline

### Local Quality Gates

Two tiers of local checks, designed to be invoked via justfile or shell alias:

**`just gate`** (pre-commit, must be fast -- target < 15 seconds):
```
cargo fmt --check
cargo clippy -- -D warnings
cargo test --lib
```

- `cargo fmt --check`: Verify all code is formatted per `rustfmt` defaults. No custom rustfmt.toml for the steel thread -- use Rust standard formatting.
- `cargo clippy -- -D warnings`: All clippy warnings treated as errors. Catches common Rust pitfalls, unused imports, redundant clones, etc.
- `cargo test --lib`: Run unit tests only (inline `#[cfg(test)]` modules in `src/`). No integration tests, no Docker required. These tests have no external dependencies and should complete in under 5 seconds.

**`just gate-expensive`** (pre-push or manual, may take minutes):
```
cargo fmt --check
cargo clippy -- -D warnings
cargo test --lib
cargo test --test '*' --features integration-tests
```

- Runs everything in `gate` plus all integration and end-to-end tests from `tests/`.
- Requires Docker to be running. Native backend tests only run on Linux.
- Expected duration: 1-3 minutes (dominated by Docker container startup and health check waits).
- Run this before pushing to verify the full test suite passes locally.

**Pre-commit hook**: The project will include a git pre-commit hook (via a shell script at `.githooks/pre-commit` with `core.hooksPath` configured in the repo) that runs `just gate`. This ensures no commit breaks formatting, linting, or unit tests. Developers can bypass with `--no-verify` for WIP commits.

### CI Pipeline (GitHub Actions)

CI runs on every push to any branch and every pull request to `main`. The pipeline is defined in `.github/workflows/ci.yml`.

**Matrix strategy**:

| Runner | Target | Tests |
|--------|--------|-------|
| `ubuntu-latest` | `x86_64-unknown-linux-gnu` | Unit + integration (Docker + native) + E2E |
| `ubuntu-24.04-arm` | `aarch64-unknown-linux-gnu` | Unit + integration (Docker) + E2E |
| `macos-latest` | `aarch64-apple-darwin` | Unit + integration (Docker) + E2E |
| `macos-13` | `x86_64-apple-darwin` | Unit only (no Docker in macOS CI runners) |

**CI steps** (per matrix entry):

1. **Checkout** and cache Rust toolchain + Cargo registry
2. **`cargo fmt --check`** -- fail fast on formatting issues
3. **`cargo clippy -- -D warnings`** -- fail on lint warnings
4. **`cargo test --lib`** -- unit tests (all runners)
5. **`cargo test --test '*' --features integration-tests`** -- integration + E2E tests (runners with Docker only)
6. **`cargo build --release`** -- verify release build compiles (catches release-only issues like LTO errors)

**Docker in CI**: Ubuntu and ARM runners have Docker pre-installed. macOS GitHub Actions runners (macos-latest, Apple Silicon) have Docker Desktop available. The `macos-13` (Intel) runner does not reliably have Docker, so only unit tests run there.

**Native backend tests in CI**: Only run on `ubuntu-latest` where `postgresql` and `minio` packages can be installed via `apt-get`. Other runners skip native tests.

**CI artifacts**: The release build step produces binaries but does not publish them. Release publishing is out of scope for the steel thread.

**Branch protection**: `main` branch requires CI to pass before merge. No direct pushes to `main`.

### Cargo Configuration

```toml
# Cargo.toml additions for testing

[features]
default = []
integration-tests = []

[dev-dependencies]
tempfile = "3"       # temp directories for test isolation
assert_cmd = "2"     # E2E: invoke compiled binary, assert exit code and output
predicates = "3"     # E2E: assert stdout/stderr content
tokio-test = "0.4"   # async test utilities
```

The `integration-tests` feature gates all tests in `tests/` that require Docker or native binaries. `cargo test` without `--features integration-tests` runs only unit tests and is safe on any machine.

## Migration and Rollout

### Implementation Phases

**Phase 0: Project Scaffolding and Quality Gates**
- Set up crate structure: Cargo.toml with all dependencies, src/lib.rs, src/main.rs, module directories (empty mod.rs files)
- Configure tracing: initialize `tracing-subscriber` in main.rs with stderr output, `-v`/`-vv` flag support
- Set up justfile with `gate` (fmt + clippy + unit tests) and `gate-expensive` (+ integration tests) targets
- Set up `.githooks/pre-commit` running `just gate`, configure `core.hooksPath`
- Set up `.github/workflows/ci.yml` with matrix builds (Linux x86/ARM, macOS x86/ARM)
- Verify `cargo build`, `cargo test --lib`, `cargo fmt --check`, `cargo clippy` all pass on empty crate
- Deliverable: empty crate that compiles, lints, and passes CI

**Phase 1: Foundation**
- Implement `grove.rs` (SHA-256 hashing of cwd path, truncate to 16 hex)
- Implement `storage/state.rs` (GroveState, ServiceState, ServiceHandleState serde types)
- Implement `storage/mod.rs` (StateManager: load/save with atomic writes, `with_lock` via fs2, data_dir/ensure_data_dir)
- Implement `orchestration/service.rs` (ServiceDefinition struct, builtin_services() returning Postgres and MinIO)
- Implement `orchestration/port.rs` (bind-to-0 allocation)
- Implement `orchestration/data_dir.rs` (create data directories under ~/.grov/store/)
- Implement `orchestration/env_template.rs` (placeholder substitution: {port}, {username}, etc.)
- Unit tests for all above (grove hashing, port allocation, template rendering, state roundtrip, file locking)
- Deliverable: all orchestration primitives working with comprehensive unit test coverage

**Phase 2: Docker Backend**
- Implement `backend/mod.rs` (Backend trait with async methods, ServiceHandle enum, BackendError)
- Implement `backend/docker.rs` (DockerBackend: bollard connect, install/start/stop/is_running, container naming, port binding, volume mounting)
- Implement `backend/health.rs` (wait_until_healthy: TCP connect polling, 250ms interval, configurable timeout)
- Integration tests: container lifecycle, port binding, volume mount, idempotent start, health check pass/timeout
- Deliverable: Docker backend fully functional and tested

**Phase 3: CLI and Orchestration Wiring**
- Implement `cli/mod.rs` (Cli struct with Parser derive, Commands enum, global flags: -v/-vv, dispatch function)
- Implement `orchestration/mod.rs` (Orchestrator struct wiring StateManager + Backend + service registry)
- Implement command handlers: install.rs, up.rs (with per-service state save), down.rs, env.rs (KEY=VALUE output), status.rs
- Wire main.rs: init tracing, parse CLI, construct Orchestrator with DockerBackend, dispatch, map errors to exit codes (0/1/2)
- Implement signal handling: tokio::signal::ctrl_c() breaks startup loop, saves current state, exits 130
- End-to-end tests: full lifecycle, exit codes, env output format, idempotency
- Deliverable: working `grov` binary with Docker backend, all 5 commands functional

**Phase 4: Native Backend**
- Implement `backend/native.rs` (NativeBackend: which lookup, process spawn via tokio::process::Command, PID tracking, PID file I/O, SIGTERM/SIGKILL on stop)
- Postgres native: initdb when data dir empty, pg_ctl start with custom port and unix socket, pg_ctl stop
- MinIO native: minio server with data dir and address flag, process kill on stop
- Integration tests for native backend (Linux CI only)
- End-to-end test for native lifecycle
- Deliverable: native backend working on Linux, both backends selectable

**Phase 5: Hardening**
- Idempotency enforcement in Orchestrator (grov up when already running returns success)
- Stale state detection (verify handles via is_running on status/env/up, update state if dead)
- Concurrent state access test (two processes running grov up simultaneously for same grove)
- Error messages for common failure modes (Docker not running, port taken, binary not found, unknown service with available services list)
- Cross-grove isolation end-to-end test (two groves, same service, simultaneous)
- Deliverable: production-quality steel thread

Each phase is independently testable and produces a concrete deliverable. Phase 0 establishes quality infrastructure. Phases 1-3 deliver a working Docker-only grov. Phase 4 adds native support. Phase 5 hardens the experience. CI gates must pass at the end of every phase.

### Feature Flags

- No feature flags for the steel thread. The binary ships with both backends compiled in. Backend selection is automatic (Docker preferred, native as fallback on Linux) or can be influenced by a future CLI flag.
- Cargo features could be used to make the native backend optional at compile time (`--features native`), but this is not needed for the steel thread.

### Rollback Plan

- **Phase 1-2**: No deployable artifact yet. Rollback is "revert commits."
- **Phase 3**: First usable binary. Rollback: users uninstall the binary. No persistent state exists until first `grov up`.
- **Phase 4-5**: Incremental additions. Rollback: any phase can be reverted independently since native backend and polish are additive.
- **State rollback**: Delete `~/.grov/` to remove all state and data. `docker ps -a | grep grov- | xargs docker rm -f` to clean up orphaned containers.

### Monitoring

- No monitoring for a CLI tool. Success/failure is observed by the user at the terminal.
- `grov status` serves as the diagnostic command -- it verifies service liveness and reports accurate state.
- Future: `grov doctor` will provide comprehensive diagnostics.

## Alternatives Considered

### Alternative 1: Shell out to Docker CLI instead of bollard

- **Description**: Use `tokio::process::Command` to run `docker run`, `docker stop`, `docker ps`, etc. Parse stdout/stderr for results.
- **Pros**: No bollard dependency. Simpler initial implementation. Works with any Docker-compatible CLI (podman via alias).
- **Cons**: Requires parsing unstructured text output which is fragile across Docker versions. Error handling is imprecise (exit codes only, no structured errors). Cannot easily inspect container state without parsing `docker inspect` JSON. Subprocess spawning adds latency. No compile-time type safety for Docker API operations.
- **Why Rejected**: bollard provides a typed, async-native Rust API that matches grov's async architecture. The upfront investment in using bollard pays off in reliable error handling and simpler code for container lifecycle management. Podman compatibility can be addressed later via bollard's configurable connection or a separate backend implementation.

### Alternative 2: SQLite for state storage instead of JSON files

- **Description**: Use SQLite (via rusqlite crate) to store grove state, service records, and port allocations in a single database file at `~/.grov/grov.db`.
- **Pros**: Structured queries for cross-grove operations (e.g., `grov ps` listing all groves). ACID transactions prevent partial state writes. Schema migrations with versioned DDL. Concurrent access is handled by SQLite's locking.
- **Cons**: Adds a significant dependency (rusqlite + SQLite C library, or a pure-Rust SQLite). Over-engineered for the steel thread's needs (single-grove, 2 services). Complicates the build (C dependency or large Rust compilation). Schema migrations are a maintenance burden for a format that may change frequently during early development.
- **Why Rejected**: JSON files are sufficient for the steel thread's scope (one state file per grove, < 1KB per file). Atomic writes (temp file + rename) prevent corruption. The cross-grove query need (`grov ps`) is out of scope. If SQLite becomes necessary for future features, migration from JSON is straightforward since the data is simple and small. Starting with the simpler option preserves the ability to switch later without premature complexity.

### Alternative 3: Process supervisor (systemd/launchd) instead of direct process management for native backend

- **Description**: Register services as user-level systemd units (Linux) or launchd plists (macOS) instead of managing processes directly.
- **Pros**: Automatic restart on crash. Log management via journald/syslog. Clean process lifecycle management by the OS. Survives grov process exit naturally.
- **Cons**: Platform-specific implementation (systemd on Linux, launchd on macOS, neither on other platforms). Requires generating and managing unit files. Adds complexity to install/uninstall lifecycle. User may not have systemd user session enabled. Debugging requires knowledge of systemd/launchd.
- **Why Rejected**: Direct process management via PID tracking is simpler, portable, and sufficient for the steel thread. The native backend is Linux-only for now, and Postgres already provides `pg_ctl` for process management. Adding a process supervisor abstraction is warranted when grov needs services to survive terminal closure or auto-restart on crash -- both are out of scope for the steel thread.

## Open Questions

- Should `grov up` with no service arguments start all hardcoded services, or return an error requiring explicit service names? (Leans toward: require explicit names for steel thread, since there is no grov.toml to define a default set.)
- Should the Docker backend auto-detect whether to use the native backend as fallback when Docker is unavailable, or require the user to explicitly choose? (Leans toward: fail with an actionable error for steel thread; automatic fallback adds implicit behavior that may confuse users.)
- Should `grov down` without arguments stop all services in the grove, or require explicit service names? (Leans toward: stop all, matching the mental model of "shut everything down.")
- Should state.json include timestamps for service start time? (Decision: yes, included in the data model above as `started_at`. Useful for debugging without adding complexity.)
- How should `grov status` indicate a service that was running but crashed? (Leans toward: show as "stopped" with a note, since we detect this via `is_running()` check.)
- What is the minimum Rust edition/version to target? Cargo.toml says edition 2024 which requires Rust 1.85+. Confirm this is acceptable.

## References

- PRD: docs/product/001-prd-steel-thread-postgres-minio.md
- Product Brief: docs/product/001-product-brief.md
- README (full vision): README.md
- bollard crate: https://crates.io/crates/bollard
- clap derive: https://docs.rs/clap/latest/clap/_derive/
