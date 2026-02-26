# Development Tickets: Grov Steel Thread - Launch Postgres and MinIO

Generated from: docs/technical/001-tdd-steel-thread-postgres-minio.md
PRD reference: docs/product/001-prd-steel-thread-postgres-minio.md
Date: 2026-02-07

## Implementation Phases

The TDD defines 6 phases (Phase 0 through Phase 5). Tickets are organized by phase with dependencies mapped across phases.

---

## Phase 0: Project Scaffolding and Quality Gates

### T-001: Set up crate structure with module directories

**Type**: Technical Task

**Summary**: Create the foundational Rust crate structure with all module directories and empty mod.rs files, plus add all dependencies to Cargo.toml.

**Motivation**:
- Establishes the module hierarchy that all subsequent tickets build on
- All crate dependencies declared upfront so later tickets focus on implementation, not project config

**Scope**:
- Update Cargo.toml with all dependencies: clap (derive), tokio (full), bollard, serde + serde_json, thiserror, anyhow, sha2, directories, which, tracing, tracing-subscriber, fs2
- Add dev-dependencies: tempfile, assert_cmd, predicates, tokio-test
- Add `[features]` section with `integration-tests` feature
- Create src/lib.rs declaring pub mod cli, orchestration, backend, storage
- Create module directories: src/cli/, src/cli/commands/, src/orchestration/, src/backend/, src/storage/
- Create empty mod.rs in each module directory
- Create tests/ directory with tests/common/mod.rs
- Update src/main.rs to call into lib (placeholder)

**Acceptance Criteria**:

**Deliverable 1: Crate compiles**
- Given all module directories and mod.rs files exist
- When `cargo build` is run
- Then the build succeeds with zero errors

**Deliverable 2: Dependencies resolve**
- Given all dependencies are declared in Cargo.toml
- When `cargo check` is run
- Then all crate dependencies resolve and compile

**Deliverable 3: Module structure matches TDD**
- Given the file tree is inspected
- When compared to the TDD Architecture section
- Then every module directory and file listed in the TDD exists

**Out of Scope**:
- Any actual logic implementation
- Tracing initialization (T-002)
- CI/CD setup (T-004)

**Technical Notes**:
- Use Rust edition 2024 (requires Rust 1.85+)
- Use clap with the `derive` feature, tokio with the `full` feature
- Reference TDD "High-Level Design" section for exact directory structure

**Dependencies**: None (first ticket)

**Estimated Effort**: S

---

### T-002: Configure tracing with stderr output and verbosity flags

**Type**: Technical Task

**Summary**: Initialize `tracing-subscriber` in main.rs with stderr output, support for `-v`/`-vv` flags, and `RUST_LOG` environment variable override.

**Motivation**:
- Logging infrastructure must exist before any other implementation so all modules can emit diagnostics from the start
- Stderr output is critical for `eval $(grov env)` correctness -- logs must never contaminate stdout

**Scope**:
- Initialize `tracing_subscriber::fmt()` with `.with_writer(std::io::stderr)` in main.rs
- Default verbosity: WARN level
- `-v` flag: INFO level
- `-vv` flag: DEBUG level
- `RUST_LOG` env var overrides `-v` flag
- Stub CLI parsing (minimal clap struct with verbosity flag) to wire verbosity to tracing

**Acceptance Criteria**:

**Deliverable 1: Tracing outputs to stderr**
- Given tracing is initialized
- When a tracing event is emitted
- Then the log output appears on stderr, not stdout

**Deliverable 2: Verbosity levels**
- Given the binary is invoked with `-v`
- When a `tracing::info!` event is emitted
- Then the event is visible in stderr output

- Given the binary is invoked without `-v`
- When a `tracing::info!` event is emitted
- Then the event is suppressed (only WARN+ visible)

**Deliverable 3: RUST_LOG override**
- Given `RUST_LOG=debug` is set
- When the binary is invoked without `-v`
- Then DEBUG-level events are visible (env var overrides default)

**Out of Scope**:
- Full CLI command parsing (T-013)
- Structured spans for async operations (future enhancement)

**Technical Notes**:
- Use `tracing_subscriber::EnvFilter` for `RUST_LOG` support with fallback to flag-based level
- Reference TDD "Logging and Diagnostics" section

**Dependencies**: T-001

**Estimated Effort**: S

---

### T-003: Set up justfile with gate and gate-expensive targets

**Type**: Technical Task

**Summary**: Create a justfile with `gate` (fast pre-commit checks) and `gate-expensive` (full test suite) targets matching the TDD quality gates specification.

**Motivation**:
- Standardized quality gates ensure consistent code quality from the first commit
- `just gate` is the pre-commit check; `just gate-expensive` is the pre-push check

**Scope**:
- Create `justfile` in repo root
- `gate` target: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test --lib`
- `gate-expensive` target: everything in `gate` plus `cargo test --test '*' --features integration-tests`
- Add `fmt` target: `cargo fmt`
- Add `build` target: `cargo build`
- Add `test` target: `cargo test --lib`

**Acceptance Criteria**:

**Deliverable 1: Gate target passes**
- Given the crate compiles and is properly formatted
- When `just gate` is run
- Then fmt check, clippy, and unit tests all pass

**Deliverable 2: Gate-expensive target structure**
- Given the justfile exists
- When `just gate-expensive` is run
- Then it runs all gate checks plus integration tests with the integration-tests feature

**Out of Scope**:
- Git hooks setup (T-005)
- CI pipeline (T-004)

**Technical Notes**:
- Gate target should complete in under 15 seconds on a clean build
- Reference TDD "Local Quality Gates" section

**Dependencies**: T-001

**Estimated Effort**: XS

---

### T-004: Set up GitHub Actions CI pipeline

**Type**: Technical Task

**Summary**: Create `.github/workflows/ci.yml` with matrix builds across Linux (x86/ARM), macOS (x86/ARM) for fmt, clippy, unit tests, integration tests, and release build verification.

**Motivation**:
- CI must pass before any merge to main
- Cross-platform validation is essential since grov targets both macOS and Linux

**Scope**:
- Create `.github/workflows/ci.yml`
- Matrix: ubuntu-latest (x86), ubuntu-24.04-arm (ARM), macos-latest (ARM), macos-13 (x86)
- Steps: checkout, cache Rust toolchain + Cargo registry, fmt check, clippy, unit tests, integration tests (where Docker available), release build
- Native backend tests only on ubuntu-latest (install postgresql and minio via apt-get)
- Branch protection note in PR description

**Acceptance Criteria**:

**Deliverable 1: CI runs on push**
- Given a commit is pushed to any branch
- When GitHub Actions triggers
- Then the CI workflow runs across the matrix

**Deliverable 2: Matrix coverage**
- Given the CI workflow is defined
- When the matrix is inspected
- Then it includes ubuntu-latest, ubuntu-24.04-arm, macos-latest, and macos-13

**Deliverable 3: Integration test gating**
- Given the ubuntu-latest runner has Docker
- When CI runs
- Then integration tests execute with `--features integration-tests`

- Given the macos-13 runner lacks Docker
- When CI runs
- Then only unit tests execute (no integration tests)

**Out of Scope**:
- Release publishing
- Artifact upload
- Branch protection rule configuration (manual setup)

**Technical Notes**:
- Reference TDD "CI Pipeline" section for exact matrix and steps
- Cache `~/.cargo/registry` and `target/` for build speed
- Install postgresql and minio packages on ubuntu-latest for native backend tests

**Dependencies**: T-001, T-003

**Estimated Effort**: M

---

### T-005: Set up git pre-commit hook running gate checks

**Type**: Technical Task

**Summary**: Create `.githooks/pre-commit` script that runs `just gate` and configure the repo to use `.githooks/` as the hooks path.

**Motivation**:
- Prevents commits that break formatting, linting, or unit tests
- Developers can bypass with `--no-verify` for WIP commits

**Scope**:
- Create `.githooks/pre-commit` shell script that runs `just gate`
- Document `git config core.hooksPath .githooks` setup in README or CONTRIBUTING

**Acceptance Criteria**:

**Deliverable 1: Hook runs on commit**
- Given `core.hooksPath` is configured to `.githooks`
- When a developer runs `git commit`
- Then `just gate` executes before the commit is created

**Deliverable 2: Hook blocks bad commits**
- Given code has a clippy warning
- When a developer runs `git commit`
- Then the commit is rejected with clippy error output

**Out of Scope**:
- Pre-push hooks
- Husky or other hook management tools

**Technical Notes**:
- The hook script should be executable (`chmod +x`)
- Reference TDD "Local Quality Gates" section

**Dependencies**: T-003

**Estimated Effort**: XS

---

## Phase 1: Foundation

### T-006: Implement grove identification (SHA-256 path hashing)

**Type**: Technical Task

**Summary**: Implement `grove::resolve()` in `src/orchestration/grove.rs` that hashes the current working directory path using SHA-256 and truncates to 16 hex characters to produce a stable grove identifier.

**Motivation**:
- Grove identity is the primary isolation mechanism -- every other component (ports, data dirs, state) is keyed by grove ID
- Must be deterministic: same path always produces same grove ID

**Scope**:
- Implement `pub fn resolve() -> Result<String>` that gets cwd, hashes with SHA-256, truncates to 16 hex chars
- Resolve relative paths to absolute before hashing
- Use the `sha2` crate

**Acceptance Criteria**:

**Deliverable 1: Deterministic hashing**
- Given a directory path "/home/user/project"
- When `resolve()` is called twice from the same directory
- Then both calls return the same 16-character hex string

**Deliverable 2: Isolation**
- Given two different directory paths
- When `resolve()` is called from each
- Then the returned grove IDs are different

**Deliverable 3: Format**
- Given any directory path
- When `resolve()` is called
- Then the result is exactly 16 characters long and contains only `[0-9a-f]`

**Deliverable 4: Absolute path handling**
- Given a relative path
- When `resolve()` is called
- Then the path is resolved to absolute before hashing

**Testing Requirements**:
- Unit tests for determinism, isolation, truncation length, character set, absolute path resolution

**Out of Scope**:
- Symlink resolution
- Custom grove naming

**Technical Notes**:
- Reference TDD `orchestration/grove.rs` section
- Use `std::env::current_dir()` to get cwd

**Dependencies**: T-001

**Estimated Effort**: S

---

### T-007: Implement state types and JSON serialization

**Type**: Technical Task

**Summary**: Implement `GroveState`, `ServiceState`, and `ServiceHandleState` serde types in `src/storage/state.rs` with JSON serialization/deserialization.

**Motivation**:
- State persistence is required for `grov status`, `grov env`, and `grov down` to work after `grov up` exits
- The state schema is the contract between all commands

**Scope**:
- Define `GroveState` struct with `grove_id: String` and `services: HashMap<String, ServiceState>`
- Define `ServiceState` struct with `service_name`, `port`, `handle`, `backend_type`, `started_at` fields
- Define `ServiceHandleState` enum with `Docker { container_id }` and `Native { pid }` variants
- Derive `Serialize`, `Deserialize` on all types
- Use `#[serde(default)]` on optional fields for forward compatibility

**Acceptance Criteria**:

**Deliverable 1: Serialization roundtrip**
- Given a `GroveState` with two services
- When serialized to JSON and deserialized back
- Then the resulting struct is identical to the original

**Deliverable 2: Forward compatibility**
- Given a JSON state file with unknown fields
- When deserialized
- Then unknown fields are ignored and known fields parse correctly

**Deliverable 3: Empty state**
- Given a `GroveState` with no services
- When serialized and deserialized
- Then the roundtrip succeeds with an empty services map

**Testing Requirements**:
- Unit tests for roundtrip, forward compatibility, empty state, missing optional fields with defaults

**Out of Scope**:
- State file I/O (T-008)
- File locking (T-008)

**Technical Notes**:
- Reference TDD "Data Model" section for exact field definitions and example JSON
- Use `serde_json` for serialization

**Dependencies**: T-001

**Estimated Effort**: S

---

### T-008: Implement StateManager with atomic writes and file locking

**Type**: Technical Task

**Summary**: Implement `StateManager` in `src/storage/mod.rs` with `new()`, `load_state()`, `save_state()`, `with_lock()`, `data_dir()`, and `ensure_data_dir()` methods.

**Motivation**:
- State must persist between grov invocations and survive crashes without corruption
- File locking prevents concurrent grov processes from losing state updates

**Scope**:
- `StateManager::new(grove_id)` creates `~/.grov/store/<grove_id>/` directory if needed
- `load_state()` reads and deserializes `state.json` (returns empty state if file doesn't exist)
- `save_state()` writes to temp file then renames (atomic write)
- `with_lock()` acquires exclusive advisory lock on `state.lock` via `fs2`, executes closure, releases lock
- `data_dir(service_name)` returns `PathBuf` for `~/.grov/store/<grove_id>/data/<service>/`
- `ensure_data_dir(service_name)` creates the data directory if it doesn't exist
- Use `directories` crate for cross-platform home directory resolution

**Acceptance Criteria**:

**Deliverable 1: Atomic writes**
- Given a state file exists
- When `save_state()` is called
- Then the state is written to a temp file first, then renamed to `state.json`
- And if the process crashes during write, the previous state.json is not corrupted

**Deliverable 2: File locking**
- Given two threads attempt concurrent state updates via `with_lock()`
- When both execute read-modify-write cycles
- Then both writes are reflected in the final state (no lost update)

**Deliverable 3: Data directory creation**
- Given a grove ID and service name
- When `ensure_data_dir("postgres")` is called
- Then `~/.grov/store/<grove_id>/data/postgres/` exists

**Deliverable 4: Missing state file**
- Given no state.json exists yet
- When `load_state()` is called
- Then an empty `GroveState` is returned with the grove ID and no services

**Testing Requirements**:
- Unit/integration tests for atomic writes (verify file exists after write), file locking (concurrent threads), data dir creation (tempdir-based), missing state file handling

**Out of Scope**:
- Stale state detection (T-023)
- Schema migration

**Technical Notes**:
- Reference TDD "Storage Layer" and "Concurrency and State Safety" sections
- Lock is held only for JSON I/O (microseconds), not for service startup
- Use `fs2::FileExt::lock_exclusive()` and `unlock()`
- Use `directories::BaseDirs` for home directory

**Dependencies**: T-007

**Estimated Effort**: M

---

### T-009: Implement ServiceDefinition and builtin service registry

**Type**: Technical Task

**Summary**: Implement `ServiceDefinition` and `ResolvedService` structs in `src/orchestration/service.rs`, plus `builtin_services()` function returning Postgres and MinIO definitions.

**Motivation**:
- `ServiceDefinition` is the primary abstraction for what a service is, independent of how it runs
- This is the extension point that makes adding new services trivial in the future

**Scope**:
- Define `ServiceDefinition` struct with all fields from the TDD (name, docker_image, docker_cmd, process_env, native_binary, native_args_fn, native_init_fn, default_port, docker_data_mount, env_template, defaults)
- Define `NativeInitStep` struct for init commands (e.g., initdb)
- Define `ResolvedService` struct with definition, grove_id, allocated_port, data_dir, resolved_env
- Implement `builtin_services()` returning a Vec with Postgres 16 and MinIO definitions
- Postgres: image `postgres:16-alpine`, default_port 5432, env vars (POSTGRES_USER=dev, POSTGRES_PASSWORD=dev, POSTGRES_DB=myapp_dev), env_template with DATABASE_URL, PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE, native binary `pg_ctl`, initdb init step
- MinIO: image `minio/minio:latest`, default_port 9000, env_template with MINIO_ENDPOINT, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, native binary `minio`

**Acceptance Criteria**:

**Deliverable 1: Registry returns two services**
- Given `builtin_services()` is called
- When the result is inspected
- Then exactly 2 services are returned: "postgres" and "minio"

**Deliverable 2: Postgres definition correctness**
- Given the postgres `ServiceDefinition`
- When its fields are inspected
- Then docker_image is "postgres:16-alpine", default_port is 5432, and env_template contains DATABASE_URL, PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE keys

**Deliverable 3: MinIO definition correctness**
- Given the minio `ServiceDefinition`
- When its fields are inspected
- Then docker_image is "minio/minio:latest", default_port is 9000, and env_template contains MINIO_ENDPOINT, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY keys

**Testing Requirements**:
- Unit tests for registry size, field correctness for each service

**Out of Scope**:
- TOML-based service definitions
- Services beyond Postgres and MinIO

**Technical Notes**:
- Reference TDD "Service Abstraction" section for exact struct definition
- `native_args_fn` and `native_init_fn` use function pointers `fn(&ResolvedService) -> ...`
- MinIO docker_cmd should include `server` and the data directory path

**Dependencies**: T-001

**Estimated Effort**: M

---

### T-010: Implement port allocation (bind-to-0 strategy)

**Type**: Technical Task

**Summary**: Implement `port::allocate()` in `src/orchestration/port.rs` that binds to port 0, records the OS-assigned port, and returns it after closing the listener.

**Motivation**:
- Dynamic port allocation avoids conflicts between groves and with other local services
- The bind-to-0 strategy leverages the OS to find available ports

**Scope**:
- Implement `pub fn allocate() -> Result<u16>` that binds a TCP listener to `127.0.0.1:0`, reads the assigned port, drops the listener, and returns the port
- Port will be in the ephemeral range (typically 1024-65535)

**Acceptance Criteria**:

**Deliverable 1: Valid port range**
- Given `allocate()` is called
- When the result is inspected
- Then the port is in the range 1024-65535

**Deliverable 2: Unique allocation**
- Given `allocate()` is called twice in sequence
- When both results are compared
- Then they return different ports

**Deliverable 3: Port released**
- Given `allocate()` returns port N
- When another process attempts to bind to port N
- Then the bind succeeds (port was released)

**Testing Requirements**:
- Unit tests for valid range, uniqueness of sequential allocations, port release

**Out of Scope**:
- Deterministic port allocation
- Port reservation or persistence

**Technical Notes**:
- Reference TDD `orchestration/port.rs` section
- Use `std::net::TcpListener::bind("127.0.0.1:0")`
- There is a TOCTOU race between port release and service bind -- this is a known limitation documented in the TDD

**Dependencies**: T-001

**Estimated Effort**: XS

---

### T-011: Implement environment variable template rendering

**Type**: Technical Task

**Summary**: Implement `env_template::render()` in `src/orchestration/env_template.rs` that substitutes `{port}`, `{username}`, `{password}`, `{database}`, and other placeholders in template strings.

**Motivation**:
- Environment variable templating is how `grov env` produces correct connection strings
- Templates are defined in `ServiceDefinition.env_template` and resolved at runtime with allocated ports and defaults

**Scope**:
- Implement a function that takes a template string and a `HashMap<String, String>` of values, returning the substituted string
- Placeholders use `{key}` syntax
- Return an error if a placeholder key is not found in the values map
- Handle literal braces gracefully (non-placeholder `{` passes through)
- Handle multiple occurrences of the same placeholder

**Acceptance Criteria**:

**Deliverable 1: Placeholder substitution**
- Given template `"postgresql://{username}:{password}@localhost:{port}/{database}"`
- When rendered with values {port: "54321", username: "dev", password: "dev", database: "myapp_dev"}
- Then the result is `"postgresql://dev:dev@localhost:54321/myapp_dev"`

**Deliverable 2: Missing key error**
- Given template `"http://localhost:{port}"`
- When rendered with an empty values map
- Then an error is returned indicating the `port` key is missing

**Deliverable 3: No placeholders**
- Given template `"minioadmin"`
- When rendered
- Then the template passes through unchanged

**Deliverable 4: Multiple occurrences**
- Given template `"{host}:{port} and {host}:{port}"`
- When rendered with values {host: "localhost", port: "9000"}
- Then the result is `"localhost:9000 and localhost:9000"`

**Testing Requirements**:
- Unit tests for all scenarios: substitution, missing key, no placeholders, multiple occurrences, malformed braces

**Out of Scope**:
- Nested templates
- Default values for missing keys
- Escaped braces syntax

**Technical Notes**:
- Reference TDD `orchestration/env_template.rs` section
- Keep implementation simple -- regex or manual string scanning, not a full template engine

**Dependencies**: T-001

**Estimated Effort**: S

---

### T-012: Implement error type hierarchy (GrovError, BackendError, StorageError, HealthCheckError)

**Type**: Technical Task

**Summary**: Define the complete error type hierarchy using `thiserror` for typed library errors, matching the TDD error specification.

**Motivation**:
- Consistent error types are needed before implementing any logic that returns errors
- Exit code mapping in main.rs depends on being able to downcast to specific error variants

**Scope**:
- Define `GrovError` enum in a shared location (src/lib.rs or src/error.rs) with variants: Backend, Storage, HealthCheck, UnknownService, AlreadyRunning
- Define `BackendError` enum in src/backend/mod.rs with variants: DockerUnavailable, PortUnavailable, StartFailed, StopFailed, Docker, BinaryNotFound
- Define `StorageError` enum in src/storage/mod.rs with variants for I/O errors, serialization errors, lock errors
- Define `HealthCheckError` enum in src/backend/health.rs with Timeout variant
- Implement `From` conversions between error types
- All error messages match the TDD specification (e.g., "Docker daemon is not running. Start Docker and try again.")

**Acceptance Criteria**:

**Deliverable 1: Error types compile and convert**
- Given a `BackendError::DockerUnavailable`
- When converted to `GrovError` via `From`
- Then the resulting `GrovError::Backend` variant wraps the original error

**Deliverable 2: Error messages match spec**
- Given a `BackendError::DockerUnavailable`
- When displayed
- Then the message is "Docker daemon is not running. Start Docker and try again."

**Testing Requirements**:
- Unit tests for error conversion and display messages

**Out of Scope**:
- Exit code mapping (T-015)
- Error recovery or retry logic

**Technical Notes**:
- Reference TDD "Error Type Hierarchy" and "Error Categories" sections
- Use `#[derive(Debug, thiserror::Error)]` on all error types

**Dependencies**: T-001

**Estimated Effort**: S

---

## Phase 2: Docker Backend

### T-013: Implement Backend trait and ServiceHandle enum

**Type**: Technical Task

**Summary**: Define the `Backend` async trait in `src/backend/mod.rs` with `install()`, `start()`, `stop()`, `is_running()`, and `backend_type()` methods, plus the `ServiceHandle` enum.

**Motivation**:
- The Backend trait is the abstraction that enables Docker and native backends to be interchangeable
- Must be defined before either backend can be implemented

**Scope**:
- Define `Backend` trait with async methods: `install(&self, service: &ServiceDefinition)`, `start(&self, service: &ResolvedService) -> ServiceHandle`, `stop(&self, handle: &ServiceHandle)`, `is_running(&self, handle: &ServiceHandle) -> bool`, `backend_type(&self) -> &'static str`
- Define `ServiceHandle` enum with `Docker { container_id: String }` and `Native { pid: u32 }` variants
- Implement `Serialize`/`Deserialize` on `ServiceHandle` for state persistence compatibility

**Acceptance Criteria**:

**Deliverable 1: Trait compiles**
- Given the Backend trait is defined
- When a struct implements all required methods
- Then the implementation compiles without errors

**Deliverable 2: Trait is object-safe**
- Given the Backend trait
- When used as `Box<dyn Backend>`
- Then the code compiles (trait is object-safe for dynamic dispatch)

**Out of Scope**:
- Docker backend implementation (T-014)
- Native backend implementation (T-019)

**Technical Notes**:
- Reference TDD "Backend Layer" section
- Use `async_trait` macro or native async trait (Rust 2024 edition may support it)
- All methods return `Result` with `BackendError`

**Dependencies**: T-009, T-012

**Estimated Effort**: S

---

### T-014: Implement Docker backend (DockerBackend)

**Type**: Technical Task

**Summary**: Implement `DockerBackend` in `src/backend/docker.rs` using the bollard crate to manage Docker container lifecycle: image pull, container create/start/stop/remove, port binding, and volume mounting.

**Motivation**:
- Docker is the primary backend for grov on both macOS and Linux
- This is the most critical backend implementation for the steel thread

**Scope**:
- `DockerBackend::new()` connects to Docker via `bollard::Docker::connect_with_local_defaults()`
- `install()`: Pull the Docker image if not present locally via `create_image()`
- `start()`: Create and start container with:
  - Name: `grov-{grove_id_short (8 chars)}-{service_name}`
  - Port mapping: `127.0.0.1:{allocated_port} -> {default_port}`
  - Volume: `{data_dir}:{docker_data_mount}`
  - Environment variables from `process_env`
  - Idempotency: if container with same name exists and is running, return its ID; if stopped, remove and recreate
- `stop()`: Stop container with 10-second timeout, then remove it
- `is_running()`: Inspect container by ID, return true if running
- `backend_type()`: Return `"docker"`

**Acceptance Criteria**:

**Deliverable 1: Image pull**
- Given Docker is running and an image is not present locally
- When `install()` is called with a service definition
- Then the image is pulled and available locally

**Deliverable 2: Container creation with port binding**
- Given a `ResolvedService` with allocated_port 54321 and default_port 5432
- When `start()` is called
- Then a container is created binding 127.0.0.1:54321 to container port 5432

**Deliverable 3: Container naming convention**
- Given grove_id "a1b2c3d4e5f6g7h8" and service_name "postgres"
- When a container is created
- Then its name is "grov-a1b2c3d4-postgres"

**Deliverable 4: Idempotent start**
- Given a container "grov-a1b2c3d4-postgres" is already running
- When `start()` is called for the same service
- Then the existing container's ID is returned without creating a duplicate

**Deliverable 5: Stop and remove**
- Given a running container
- When `stop()` is called
- Then the container is stopped (10s timeout) and removed
- And the bind-mounted data directory still exists on the host

**Deliverable 6: Docker unavailable**
- Given Docker daemon is not running
- When `DockerBackend::new()` is called
- Then `BackendError::DockerUnavailable` is returned

**Testing Requirements**:
- Integration tests (feature-gated `integration-tests`): container lifecycle, port binding, volume mount, idempotent start, container naming
- Use a small image like `alpine:latest` for tests to minimize pull time

**Out of Scope**:
- Container networking or linking
- Docker Compose compatibility
- Custom Docker socket paths

**Technical Notes**:
- Reference TDD "Docker Backend" section
- Use bollard's `ContainerConfig`, `HostConfig` with `PortBindings` and `Binds`
- Handle the case where a stopped container exists with the same name: remove it before creating new

**Dependencies**: T-013

**Estimated Effort**: L

---

### T-015: Implement health check (TCP connect polling)

**Type**: Technical Task

**Summary**: Implement `wait_until_healthy()` in `src/backend/health.rs` that polls TCP connections to `127.0.0.1:{port}` at 250ms intervals with a configurable timeout.

**Motivation**:
- Commands must only return after services are ready to accept connections
- TCP connect is sufficient for both Postgres and MinIO in the steel thread

**Scope**:
- Implement `pub async fn wait_until_healthy(host: &str, port: u16, timeout: Duration, interval: Duration) -> Result<(), HealthCheckError>`
- Attempt `TcpStream::connect` to `{host}:{port}` every `interval`
- If connection succeeds, return `Ok(())`
- If `timeout` is exceeded, return `HealthCheckError::Timeout`
- Default interval: 250ms, default timeout: 60s

**Acceptance Criteria**:

**Deliverable 1: Healthy service detected**
- Given a TCP listener is bound on a port
- When `wait_until_healthy` is called for that port
- Then it returns `Ok(())` within the first few poll attempts

**Deliverable 2: Timeout on unhealthy**
- Given no listener exists on a port
- When `wait_until_healthy` is called with a 2-second timeout
- Then `HealthCheckError::Timeout` is returned after approximately 2 seconds

**Deliverable 3: Delayed readiness**
- Given a TCP listener starts 500ms after the health check begins
- When `wait_until_healthy` is called
- Then it returns `Ok(())` after the listener starts (within polling interval tolerance)

**Testing Requirements**:
- Integration tests: bind listener then check, no listener with short timeout, delayed listener start

**Out of Scope**:
- Protocol-specific checks (pg_isready, HTTP GET)
- Health check retries at the orchestration level

**Technical Notes**:
- Reference TDD "Health Check" section
- Use `tokio::net::TcpStream::connect` for async TCP connect
- Use `tokio::time::sleep` for polling interval
- Use `tokio::time::timeout` for overall timeout

**Dependencies**: T-012

**Estimated Effort**: S

---

## Phase 3: CLI and Orchestration Wiring

### T-016: Implement CLI argument parsing with clap derive

**Type**: Technical Task

**Summary**: Implement the full `Cli` struct with clap derive macros, `Commands` enum with all 5 command variants (install, up, down, env, status), and the `dispatch()` function.

**Motivation**:
- CLI is the user-facing entry point; all commands must be parseable before handlers can be wired
- Clap derive provides compile-time validation and generates help text automatically

**Scope**:
- Define `Cli` struct with `#[derive(Parser)]`, global `-v`/`-vv` verbosity flags
- Define `Commands` enum with variants: Install { services: Vec<String> }, Up { services: Vec<String> }, Down { services: Option<Vec<String>> }, Env, Status
- Implement `dispatch(cli: Cli, orchestrator: Orchestrator) -> Result<()>` that matches on command and calls orchestrator methods
- Wire into main.rs: parse CLI, construct Orchestrator, dispatch

**Acceptance Criteria**:

**Deliverable 1: Command parsing**
- Given input `["up", "postgres", "minio"]`
- When parsed via `Cli::parse_from()`
- Then the result is `Commands::Up` with services `["postgres", "minio"]`

**Deliverable 2: Down with no services**
- Given input `["down"]`
- When parsed
- Then the result is `Commands::Down` with no services specified

**Deliverable 3: Unknown command**
- Given input `["foobar"]`
- When parsed
- Then clap returns a usage error

**Deliverable 4: Verbosity flags**
- Given input `["-vv", "status"]`
- When parsed
- Then verbosity level is 2 (DEBUG)

**Testing Requirements**:
- Unit tests for all command variants, verbosity flags, unknown commands

**Out of Scope**:
- Command handler implementations (T-017, T-018)
- `--json` or `--quiet` flags

**Technical Notes**:
- Reference TDD "CLI Layer" section
- Use clap's derive API, not builder API
- `Down` services is `Option<Vec<String>>` -- if None, stop all services

**Dependencies**: T-002, T-012

**Estimated Effort**: S

---

### T-017: Implement Orchestrator struct and core command logic (up, down, install)

**Type**: Technical Task

**Summary**: Implement the `Orchestrator` struct in `src/orchestration/mod.rs` that coordinates StateManager, Backend, and the service registry. Implement `install()`, `up()`, and `down()` methods.

**Motivation**:
- The Orchestrator is the central coordinator that ties all layers together
- `up`, `down`, and `install` are the primary lifecycle commands

**Scope**:
- `Orchestrator` struct holding `Box<dyn Backend>`, `StateManager`, and service registry
- `install(service_names)`: Look up definitions, call `backend.install()` for each
- `up(service_names)`: For each service:
  1. Resolve grove ID
  2. Load state, check if already running (skip if so)
  3. Allocate port
  4. Ensure data directory
  5. Construct ResolvedService
  6. Call `backend.start()`
  7. Call `health::wait_until_healthy()`
  8. Save state via `with_lock()` after each service (not batched)
- `down(service_names)`: Load state, call `backend.stop()` for each, remove from state, save
- Validate service names against registry, return `GrovError::UnknownService` for unknown names
- Idempotency: `up` skips already-running services, `down` skips already-stopped services

**Acceptance Criteria**:

**Deliverable 1: Up starts services and records state**
- Given no services are running
- When `orchestrator.up(["postgres"])` is called
- Then the backend starts postgres, health check passes, and state.json contains the service entry

**Deliverable 2: Up is idempotent**
- Given postgres is already running
- When `orchestrator.up(["postgres"])` is called
- Then no duplicate service is started and the command succeeds

**Deliverable 3: Down stops services and updates state**
- Given postgres is running
- When `orchestrator.down(["postgres"])` is called
- Then the backend stops postgres and state.json no longer contains the service entry
- And the data directory is preserved

**Deliverable 4: Unknown service validation**
- Given service name "postgre" (typo)
- When `orchestrator.up(["postgre"])` is called
- Then `GrovError::UnknownService` is returned

**Deliverable 5: Per-service state save**
- Given two services are requested in `up`
- When the first service starts successfully but the second fails
- Then state.json contains the first service (saved immediately after it started)

**Testing Requirements**:
- Integration tests with Docker backend (feature-gated)
- Unit tests for service name validation

**Out of Scope**:
- Signal handling (T-020)
- Stale state detection (T-023)
- Env and status commands (T-018)

**Technical Notes**:
- Reference TDD "Orchestration Layer" and "Primary Operation: grov up" sequence flow
- State is saved after each individual service start, not batched at the end
- On health check timeout: stop the service, remove from state, return error

**Dependencies**: T-006, T-008, T-009, T-010, T-011, T-013, T-014, T-015

**Estimated Effort**: L

---

### T-018: Implement env and status command handlers

**Type**: Technical Task

**Summary**: Implement `orchestrator.env()` and `orchestrator.status()` methods plus their CLI output formatting.

**Motivation**:
- `grov env` is the primary way developers get connection strings for their services
- `grov status` is the diagnostic command for checking service health

**Scope**:
- `env()`: Load state, look up ServiceDefinition for each running service, construct ResolvedService with persisted port, render env_template, return key-value pairs
- CLI handler: Print each key=value pair to stdout (bare format, no quoting, no `export` prefix, no comments)
- `status()`: Load state, call `backend.is_running()` for each service, return status table data
- CLI handler: Print table with columns SERVICE, BACKEND, STATUS, PORT

**Acceptance Criteria**:

**Deliverable 1: Env output format**
- Given postgres and minio are running
- When `grov env` is executed
- Then stdout contains bare `KEY=VALUE` lines including DATABASE_URL, PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE, MINIO_ENDPOINT, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY

**Deliverable 2: Env output is eval-safe**
- Given `grov env` output
- When piped to `eval`
- Then no errors, comments, or non-KEY=VALUE content is present on stdout

**Deliverable 3: Status shows running services**
- Given postgres is running on port 54321 via Docker backend
- When `grov status` is executed
- Then output includes a row showing postgres, docker, healthy/running, 54321

**Deliverable 4: Status with no services**
- Given no services are running
- When `grov status` is executed
- Then output indicates no running services

**Testing Requirements**:
- Integration tests: env output parsing, status output format
- E2E tests: full lifecycle with env and status verification

**Out of Scope**:
- `--json` output format
- Stale state correction in status (T-023)

**Technical Notes**:
- Reference TDD "Secondary Operation: grov env" and "Secondary Operation: grov status" sequence flows
- Reference TDD "Output Format Contract" for exact env output format
- All output to stdout, errors/logs to stderr

**Dependencies**: T-017

**Estimated Effort**: M

---

### T-019: Implement exit code mapping in main.rs

**Type**: Technical Task

**Summary**: Implement error-to-exit-code mapping in main.rs: success -> 0, runtime/backend errors -> 1, usage/validation errors -> 2, SIGINT -> 130.

**Motivation**:
- Exit codes are part of the CLI public contract -- scripts and justfiles depend on them
- Must match the TDD specification exactly

**Scope**:
- In main.rs, catch `anyhow::Error` from dispatch
- Downcast to `GrovError` or clap errors
- Map: `GrovError::UnknownService` -> exit 2, `BackendError::*` -> exit 1, `StorageError::*` -> exit 1, clap usage errors -> exit 2, success -> exit 0
- Print error message to stderr before exiting

**Acceptance Criteria**:

**Deliverable 1: Success exit code**
- Given a command succeeds
- When the process exits
- Then exit code is 0

**Deliverable 2: Runtime error exit code**
- Given Docker is not running and `grov up postgres` is executed
- When the process exits
- Then exit code is 1

**Deliverable 3: Usage error exit code**
- Given an unknown service name "foobar"
- When `grov up foobar` is executed
- Then exit code is 2

**Testing Requirements**:
- E2E tests using `assert_cmd` to verify exit codes for success, runtime error, usage error

**Out of Scope**:
- SIGINT exit code 130 (T-020)

**Technical Notes**:
- Reference TDD "Exit Codes" table and "CLI Layer" section
- Use `std::process::exit()` after printing the error

**Dependencies**: T-016, T-017

**Estimated Effort**: S

---

### T-020: Implement signal handling (SIGINT/Ctrl+C)

**Type**: Technical Task

**Summary**: Handle SIGINT (Ctrl+C) during `grov up` by breaking the service startup loop, saving current state, and exiting with code 130.

**Motivation**:
- Users need a safe way to interrupt long-running `grov up` operations
- Already-started services must remain running with accurate state

**Scope**:
- Register `tokio::signal::ctrl_c()` handler in main.rs
- During the `up` service startup loop, check the SIGINT flag between service iterations
- If SIGINT received: break the loop, save current state (already-started services recorded), exit with code 130
- Already-started services remain running

**Acceptance Criteria**:

**Deliverable 1: SIGINT breaks startup loop**
- Given `grov up postgres minio` is running and postgres has started
- When SIGINT is sent before minio starts
- Then the process exits with code 130
- And postgres remains running
- And state.json contains postgres but not minio

**Deliverable 2: State accuracy after SIGINT**
- Given SIGINT was received during `grov up`
- When `grov status` is run afterward
- Then only the services that successfully started are shown

**Out of Scope**:
- SIGTERM handling
- Rollback of already-started services
- Graceful shutdown timeout

**Technical Notes**:
- Reference TDD "Signal Handling" section
- Use `tokio::signal::ctrl_c()` as an async future
- Check a shared flag (e.g., `AtomicBool`) between service startup iterations
- Exit code 130 = 128 + SIGINT signal number (Unix convention)

**Dependencies**: T-017

**Estimated Effort**: M

---

### T-021: Implement end-to-end Docker lifecycle tests

**Type**: Technical Task

**Summary**: Write comprehensive end-to-end tests that exercise the full grov binary lifecycle via Docker: install, up, env, status, down, cross-grove isolation, idempotency, data persistence, exit codes, and env output format.

**Motivation**:
- E2E tests validate that all layers integrate correctly when used through the compiled binary
- These are the definitive tests that prove the steel thread works

**Scope**:
- Create `tests/e2e_docker.rs` (feature-gated `integration-tests`)
- Create `tests/common/mod.rs` with `TestGrove` fixture (temp dir, cleanup on drop)
- Tests:
  - Full lifecycle: install -> up -> health verify -> env -> status -> down -> status shows empty -> data dirs exist
  - Cross-grove isolation: up postgres from two temp dirs, both run simultaneously on different ports, independently stoppable
  - Idempotency: up postgres twice, only one container exists
  - Data persistence: up postgres -> create table -> down -> up postgres -> table still exists
  - Exit codes: unknown service -> 2, Docker stopped -> 1, success -> 0
  - Env output format: parseable KEY=VALUE, no export prefix, no comments

**Acceptance Criteria**:

**Deliverable 1: Full lifecycle passes**
- Given Docker is running
- When the full lifecycle test executes
- Then all assertions pass: services start, health checks succeed, env outputs valid connection strings, status shows running, down stops everything, data dirs preserved

**Deliverable 2: Cross-grove isolation**
- Given two test groves in different directories
- When both run `up postgres`
- Then both instances are running simultaneously on different ports with different data directories

**Testing Requirements**:
- All tests feature-gated behind `integration-tests`
- Use `assert_cmd` for binary invocation, `predicates` for output assertions
- Clean up containers on test failure (TestGrove drop implementation)

**Out of Scope**:
- Native backend E2E tests (T-022)
- Performance benchmarks

**Technical Notes**:
- Reference TDD "End-to-End Tests" section
- TestGrove should clean up containers with matching grove prefix on drop
- Use short health check timeouts in tests where possible

**Dependencies**: T-017, T-018, T-019

**Estimated Effort**: L

---

## Phase 4: Native Backend

### T-022: Implement native backend (NativeBackend)

**Type**: Technical Task

**Summary**: Implement `NativeBackend` in `src/backend/native.rs` using `which` for binary discovery, `tokio::process::Command` for process management, and PID tracking for lifecycle control. Includes Postgres-specific initdb/pg_ctl logic and MinIO process management.

**Motivation**:
- Native backend provides an alternative to Docker for Linux users who prefer native installations
- Validates the Backend trait abstraction works with a fundamentally different implementation

**Scope**:
- `NativeBackend::new()` (no special initialization needed)
- `install()`: Use `which::which()` to verify binary exists in PATH; return `BinaryNotFound` error with install instructions if not found
- `start()`:
  - Run init step if defined and data dir is empty (e.g., `initdb -D {data_dir}` for Postgres)
  - Spawn service process via `tokio::process::Command`
  - Write PID to `{data_dir}/../{service_name}.pid`
  - Return `ServiceHandle::Native { pid }`
- `stop()`: Send SIGTERM; if not exited within 10 seconds, send SIGKILL
- `is_running()`: Check if PID is alive (kill with signal 0)
- Postgres specifics: `initdb -D {data_dir}`, `pg_ctl -D {data_dir} -l {data_dir}/logfile -o "-p {port} -k /tmp" start`, `pg_ctl -D {data_dir} stop -m fast`
- MinIO specifics: `minio server {data_dir} --address :{port}`, kill process on stop

**Acceptance Criteria**:

**Deliverable 1: Binary discovery**
- Given `pg_ctl` is in PATH
- When `install()` is called for postgres
- Then it succeeds

- Given `pg_ctl` is not in PATH
- When `install()` is called for postgres
- Then `BackendError::BinaryNotFound` is returned with an install instruction message

**Deliverable 2: Postgres lifecycle**
- Given postgres native binaries are available
- When `start()` is called for postgres with an empty data directory
- Then `initdb` runs, `pg_ctl start` launches postgres, a PID file is written, and the service accepts connections

**Deliverable 3: MinIO lifecycle**
- Given minio binary is available
- When `start()` is called for minio
- Then `minio server` launches, a PID file is written, and the service accepts connections

**Deliverable 4: Stop with SIGTERM/SIGKILL**
- Given a native service is running
- When `stop()` is called
- Then SIGTERM is sent; if process doesn't exit in 10 seconds, SIGKILL is sent

**Deliverable 5: is_running accuracy**
- Given a service was started and PID recorded
- When `is_running()` is called
- Then it returns true while the process is alive and false after it exits

**Testing Requirements**:
- Integration tests (feature-gated, Linux only): binary discovery, Postgres lifecycle (initdb + start + stop), MinIO lifecycle, PID file I/O
- E2E test for native backend full lifecycle

**Out of Scope**:
- macOS native backend support
- Automatic backend fallback from Docker to native

**Technical Notes**:
- Reference TDD "Native Backend" section
- Linux only for the steel thread
- Use `nix` crate or raw `libc::kill` for signal sending and PID liveness check
- `native_args_fn` and `native_init_fn` from ServiceDefinition provide the command arguments

**Dependencies**: T-013

**Estimated Effort**: L

---

## Phase 5: Hardening

### T-023: Implement stale state detection and correction

**Type**: Technical Task

**Summary**: When loading state for `status`, `env`, or `up`, verify each service handle is still alive via `backend.is_running()`. If a service died externally, update state to reflect the actual status.

**Motivation**:
- Containers can be killed externally (`docker kill`), processes can crash -- state.json becomes stale
- Users must see accurate status, and `grov up` must correctly detect that a service needs restarting

**Scope**:
- On `status()`: For each service in state, call `is_running()`. If false, mark as stopped/remove from state.
- On `env()`: Same check before rendering env vars. Only output env vars for actually running services.
- On `up()`: Same check before skipping "already running" services. If state says running but `is_running()` returns false, remove stale entry and proceed to start.

**Acceptance Criteria**:

**Deliverable 1: Status detects dead service**
- Given postgres was running but the container was killed externally
- When `grov status` is run
- Then status shows postgres as stopped (not stale "running")
- And state.json is updated to remove the dead service

**Deliverable 2: Up restarts dead service**
- Given state.json says postgres is running but the container is dead
- When `grov up postgres` is run
- Then postgres is started fresh (not skipped as "already running")

**Deliverable 3: Env skips dead services**
- Given state.json says postgres is running but the container is dead
- When `grov env` is run
- Then no postgres env vars are output

**Testing Requirements**:
- Integration test: start service, externally kill container, verify status/up/env detect the stale state

**Out of Scope**:
- Automatic service restart
- Orphaned container discovery (beyond state.json entries)

**Technical Notes**:
- Reference TDD "Stale state detection" in Storage Layer section and "Failure Modes" section

**Dependencies**: T-017, T-018

**Estimated Effort**: M

---

### T-024: Implement user-friendly error messages for common failures

**Type**: Technical Task

**Summary**: Ensure all common failure scenarios produce clear, actionable error messages as specified in the TDD error handling table.

**Motivation**:
- Good error messages are the difference between a frustrating tool and a helpful one
- The TDD specifies exact error messages for each failure category

**Scope**:
- Docker not running: "Docker daemon is not running. Start Docker and try again."
- Port unavailable: "Port {port} is unavailable. Run `grov up` again to allocate a new port."
- Unknown service: "Unknown service: 'postgre'. Available services: minio, postgres"
- Health check timeout: "{service} failed to become healthy within 60 seconds"
- Native binary not found: "{binary} not found. Install it and ensure it is in PATH."
- Disk full: "Failed to create data directory: {os_error}"
- List all available services when an unknown service name is provided

**Acceptance Criteria**:

**Deliverable 1: Docker unavailable message**
- Given Docker is not running
- When `grov up postgres` is run
- Then stderr shows "Docker daemon is not running. Start Docker and try again."
- And exit code is 1

**Deliverable 2: Unknown service lists available services**
- Given the user runs `grov up postgre`
- When the service name is validated
- Then the error message lists all available services

**Deliverable 3: Health check timeout message**
- Given a service fails to become healthy
- When the timeout is reached
- Then stderr shows "postgres failed to become healthy within 60 seconds"

**Testing Requirements**:
- Unit tests for unknown service error message with available services list
- E2E tests for error message output on common failures

**Out of Scope**:
- Structured error codes beyond exit codes
- Error telemetry or reporting

**Technical Notes**:
- Reference TDD "Error Categories" table for exact messages
- List available services from `builtin_services()` in the error message

**Dependencies**: T-017, T-012

**Estimated Effort**: M

---

### T-025: Implement concurrent state access safety test

**Type**: Technical Task

**Summary**: Write an integration test verifying that two processes running `grov up` simultaneously for the same grove do not lose state updates, validated by the file locking mechanism.

**Motivation**:
- The file locking implementation in T-008 needs to be verified under realistic concurrent conditions
- This is a hardening test that validates the concurrency guarantees promised by the TDD

**Scope**:
- Write an integration test that:
  1. Spawns two async tasks or threads simulating concurrent `grov up` for the same grove
  2. Both attempt to update state.json via `with_lock()`
  3. Verify the final state contains both services (no lost updates)
- Test stale lock recovery: simulate process crash by dropping lock file handle, verify subsequent lock acquisition succeeds

**Acceptance Criteria**:

**Deliverable 1: No lost updates**
- Given two concurrent state writers targeting the same grove
- When both complete their write operations
- Then the final state.json contains entries from both writers

**Deliverable 2: Lock recovery**
- Given a process acquired a lock and then crashed (handle dropped)
- When another process attempts to acquire the lock
- Then the lock is acquired successfully (OS releases advisory lock on process exit)

**Testing Requirements**:
- Integration test with concurrent threads and shared state file

**Out of Scope**:
- Cross-process testing (test uses threads within one process as proxy)
- Distributed locking

**Technical Notes**:
- Reference TDD "Concurrency and State Safety" section
- Use `std::thread::spawn` or `tokio::spawn` for concurrent tasks in test

**Dependencies**: T-008

**Estimated Effort**: S

---

### T-026: Implement cross-grove isolation end-to-end test

**Type**: Technical Task

**Summary**: Write an E2E test proving two groves can run the same service simultaneously on different ports with different data directories, and each can be stopped independently.

**Motivation**:
- Cross-grove isolation is a core architectural proof point from the PRD
- Must be validated end-to-end through the compiled binary

**Scope**:
- Create two temp directories (two different groves)
- Run `grov up postgres` from each directory
- Verify both instances are running on different ports
- Verify data directories are separate (`~/.grov/store/<grove_a>/data/postgres/` vs `~/.grov/store/<grove_b>/data/postgres/`)
- Stop one grove's postgres, verify the other is still running
- Stop the second grove's postgres

**Acceptance Criteria**:

**Deliverable 1: Simultaneous operation**
- Given two groves in different directories
- When both run `grov up postgres`
- Then both postgres instances are running simultaneously on different ports

**Deliverable 2: Data isolation**
- Given two groves running postgres
- When their data directories are compared
- Then they are in separate grove-specific paths

**Deliverable 3: Independent lifecycle**
- Given two groves running postgres
- When one grove runs `grov down`
- Then only that grove's postgres stops; the other remains running

**Testing Requirements**:
- Feature-gated E2E test requiring Docker

**Out of Scope**:
- Cross-grove service discovery
- Shared service instances

**Technical Notes**:
- Reference PRD "Cross-Worktree Isolation Criteria" acceptance criteria
- TestGrove fixture handles temp dir creation and cleanup

**Dependencies**: T-021

**Estimated Effort**: M

---

### T-027: Implement data persistence end-to-end test

**Type**: Technical Task

**Summary**: Write an E2E test proving that service data persists across stop/start cycles: start postgres, create a table, stop, start again, verify the table still exists.

**Motivation**:
- Data persistence across restarts is a fundamental requirement -- developers must not lose their database state

**Scope**:
- Start postgres via `grov up postgres`
- Connect to postgres using the port from `grov env` and create a table with a row
- Run `grov down`
- Run `grov up postgres` again (same grove, same data directory)
- Connect to postgres and verify the table and row still exist

**Acceptance Criteria**:

**Deliverable 1: Data survives restart**
- Given postgres was started, a table was created, and `grov down` was run
- When `grov up postgres` is run again from the same directory
- Then the previously created table and its data are present

**Testing Requirements**:
- Feature-gated E2E test requiring Docker
- May need a postgres client library or use `psql` via subprocess for verification

**Out of Scope**:
- MinIO data persistence test (similar pattern, deferred)
- Backup/restore functionality

**Technical Notes**:
- Data persists because the Docker volume is a bind mount to `~/.grov/store/<grove>/data/postgres/`
- May use `tokio-postgres` or shell out to `psql` for DB assertions

**Dependencies**: T-021

**Estimated Effort**: M

---

## Spikes

### T-028: Spike: Determine open question behaviors

**Type**: Spike

**Summary**: Resolve the TDD's open questions to finalize behaviors before or during implementation.

**Goal**: Make decisions on the 6 open questions listed in the TDD so implementation tickets have clear requirements.

**Questions to Answer**:
1. Should `grov up` with no service arguments start all hardcoded services or return an error? (TDD leans: require explicit names)
2. Should Docker backend auto-detect native fallback or require explicit choice? (TDD leans: fail with error)
3. Should `grov down` without arguments stop all services or require explicit names? (TDD leans: stop all)
4. How should `grov status` indicate a crashed service? (TDD leans: show as "stopped" with note)
5. Confirm Rust edition 2024 / Rust 1.85+ is acceptable
6. Should native async traits be used if Rust 2024 supports them, or use `async_trait` macro?

**Acceptance Criteria**:

**Research Complete**:
- Given all 6 questions are evaluated
- When decisions are made
- Then each decision is documented with rationale and affected ticket IDs are updated

**Deliverables**:
- Decision record (can be appended to TDD or captured in a separate document)
- Updated ticket descriptions if decisions change behavior

**Scope**:
- Evaluate TDD lean positions
- Check Rust 2024 edition async trait support
- Make decisions

**Out of Scope**:
- Implementing the decisions (covered by existing tickets)

**Time Box**: 1 day

**Dependencies**: None

**Estimated Effort**: S

---

## Dependency Map

```
Phase 0: Scaffolding
  T-001 (Crate structure)
    |
    +-- T-002 (Tracing) --------+
    +-- T-003 (Justfile) -------+-- T-005 (Git hooks)
    +-- T-006 (Grove ID)        |
    +-- T-009 (ServiceDef)      +-- T-004 (CI)
    +-- T-010 (Port alloc)
    +-- T-011 (Env template)
    +-- T-012 (Error types)

Phase 1: Foundation
  T-007 (State types) -- T-008 (StateManager)

Phase 2: Docker Backend
  T-009 + T-012 -- T-013 (Backend trait)
  T-013 ---------- T-014 (Docker backend)
  T-012 ---------- T-015 (Health check)

Phase 3: CLI + Orchestration
  T-002 + T-012 -- T-016 (CLI parsing)
  T-006 + T-008 + T-009 + T-010 + T-011 + T-013 + T-014 + T-015 -- T-017 (Orchestrator)
  T-017 ---------- T-018 (Env + Status)
  T-016 + T-017 -- T-019 (Exit codes)
  T-017 ---------- T-020 (Signal handling)
  T-017 + T-018 + T-019 -- T-021 (E2E tests)

Phase 4: Native Backend
  T-013 ---------- T-022 (Native backend)

Phase 5: Hardening
  T-017 + T-018 -- T-023 (Stale state)
  T-017 + T-012 -- T-024 (Error messages)
  T-008 ---------- T-025 (Concurrent state test)
  T-021 ---------- T-026 (Cross-grove test)
  T-021 ---------- T-027 (Data persistence test)

Spike (any time):
  T-028 (Open questions)
```

## Summary

- **Total tickets**: 28
- **Technical Tasks**: 27
- **Spikes**: 1
- **User Stories**: 0 (all work is infrastructure/backend; user-facing value validated via E2E tests)

**By phase**:
- Phase 0 (Scaffolding): 5 tickets (T-001 through T-005)
- Phase 1 (Foundation): 7 tickets (T-006 through T-012)
- Phase 2 (Docker Backend): 3 tickets (T-013 through T-015)
- Phase 3 (CLI + Orchestration): 6 tickets (T-016 through T-021)
- Phase 4 (Native Backend): 1 ticket (T-022)
- Phase 5 (Hardening): 5 tickets (T-023 through T-027)
- Spikes: 1 ticket (T-028)

**Estimated total effort**: M-L range (roughly 25-40 days of focused work across all tickets)

---

## Agent Parallelism and Coordination Guide

This section defines how an orchestrator agent should schedule and coordinate work across multiple coding agents executing tickets concurrently.

### Execution Model

A single **orchestrator agent** assigns tickets to **coding agents**, manages merge ordering, and resolves physical conflicts (e.g., two agents needing to edit the same file). Coding agents work autonomously within their assigned ticket scope and signal completion back to the orchestrator.

### Wave Schedule

Tickets are grouped into waves. All tickets within a wave are logically independent and can execute in parallel. A wave starts when all of its blocking dependencies from prior waves have merged.

```
Wave 1 [sequential — must complete before anything else]:
  T-001  Crate structure                                [XS]

Wave 2 [up to 8 parallel agents]:
  T-002  Tracing/stderr output and verbosity flags      [S]
  T-003  Justfile with gate targets                     [XS]
  T-006  Grove ID (SHA-256 path hashing)                [S]
  T-007  State types and JSON serialization             [S]
  T-009  ServiceDefinition and builtin registry         [M]
  T-010  Port allocation (bind-to-0)                    [XS]
  T-011  Env variable template rendering                [S]
  T-012  Error type hierarchy                           [S]

Wave 3 [up to 4 parallel agents]:
  T-008  StateManager (atomic writes, file locking)     [M]  ← after T-007
  T-013  Backend trait and ServiceHandle enum            [S]  ← after T-009, T-012
  T-015  Health check (TCP connect polling)             [S]  ← after T-012
  T-016  CLI argument parsing (clap derive)             [S]  ← after T-002, T-012

Wave 4 [2 parallel agents]:
  T-014  Docker backend (DockerBackend)                 [L]  ← after T-013
  T-022  Native backend (NativeBackend)                 [L]  ← after T-013

Wave 5 [serialization point — single agent recommended]:
  T-017  Orchestrator struct + core command logic       [L]  ← after T-006, T-008, T-009,
                                                               T-010, T-011, T-013, T-014, T-015

Wave 6 [up to 5 parallel agents]:
  T-018  Env and status command handlers                [M]  ← after T-017
  T-019  Exit code mapping in main.rs                   [S]  ← after T-016, T-017
  T-020  Signal handling (SIGINT/Ctrl+C)                [M]  ← after T-017
  T-023  Stale state detection and correction           [M]  ← after T-017, T-018
  T-024  User-friendly error messages                   [M]  ← after T-017, T-012

Wave 7 [up to 3 parallel agents]:
  T-021  End-to-end Docker lifecycle tests              [L]  ← after T-017, T-018, T-019
  T-026  Cross-grove isolation E2E test                 [M]  ← after T-021
  T-027  Data persistence E2E test                      [M]  ← after T-021

Wave 8 [cleanup — up to 3 parallel agents]:
  T-004  GitHub Actions CI pipeline                     [M]  ← after T-003
  T-005  Git pre-commit hook                            [XS] ← after T-003
  T-025  Concurrent state access safety test            [S]  ← after T-008
```

**Critical path**: T-001 → T-009 → T-013 → T-014 → T-017 → T-018 → T-021

T-028 (Spike: open questions) can execute at any time and should ideally complete before Wave 3 so decisions feed into backend and CLI implementations.

### Recommended Agent Count

- **Waves 2-3**: 3-4 concurrent agents. While 8 tickets are logically independent, more than 4 agents increases merge coordination overhead without proportional throughput gain.
- **Wave 4**: 2 agents (one per backend). These are large, self-contained tickets with no file overlap.
- **Wave 5**: 1 agent. T-017 is the integration bottleneck. It wires all prior layers together and benefits from a single agent with full context of the codebase.
- **Waves 6-7**: 2-3 agents.

### Physical Conflict Mitigation

Logical independence (no dependency edge) does not guarantee physical independence (no shared files). The orchestrator must manage file-level conflicts.

**High-conflict files after T-001**:
- `Cargo.toml` — T-001 declares all dependencies upfront. No subsequent ticket should need to edit this file. If a coding agent discovers a missing dependency, it must request the orchestrator to add it as a coordinated edit.
- `src/lib.rs` — T-001 creates all `pub mod` declarations. Subsequent tickets implement modules but should not need to modify lib.rs.
- Module `mod.rs` files — T-001 creates empty mod.rs files. Coding agents add `pub mod` declarations for their specific submodules. The orchestrator should assign at most one agent per module directory in any given wave.

**File ownership per wave (Wave 2 example)**:

| Agent | Ticket | Owned files |
|---|---|---|
| A | T-002 | `src/main.rs` (tracing init) |
| B | T-003 | `justfile` |
| C | T-006 | `src/orchestration/grove.rs`, `src/orchestration/mod.rs` |
| D | T-007 | `src/storage/state.rs`, `src/storage/mod.rs` |
| E | T-009 | `src/orchestration/service.rs`, `src/orchestration/mod.rs` |
| F | T-010 | `src/orchestration/port.rs`, `src/orchestration/mod.rs` |
| G | T-011 | `src/orchestration/env_template.rs`, `src/orchestration/mod.rs` |
| H | T-012 | `src/backend/mod.rs`, `src/backend/health.rs` (error types) |

**Conflict**: Agents C, E, F, and G all need to add `pub mod` lines to `src/orchestration/mod.rs`. The orchestrator has two options:
1. **Serialize mod.rs edits**: Let agents implement their `.rs` files in parallel, then have the orchestrator (or a single agent) add all `pub mod` declarations to `mod.rs` in one pass after all agents complete.
2. **Pre-populate mod.rs in T-001**: Extend T-001 to declare all submodule `pub mod` lines (with empty files), so no agent needs to edit mod.rs at all.

Option 2 is strongly preferred. The orchestrator should ensure T-001 creates all `pub mod` declarations, not just empty files.

### Merge Protocol

1. Coding agent completes work on a branch and signals the orchestrator.
2. Orchestrator verifies `just gate` passes on the agent's branch.
3. Orchestrator merges to the integration branch (main or a shared dev branch).
4. If merge conflicts arise, the orchestrator resolves them (these should be limited to mod.rs or similar declaration files if file ownership is respected).
5. Orchestrator runs `just gate` on the merged result before proceeding.
6. Next-wave tickets are unblocked only after their dependencies have merged and gate passes.

### Team Streams (for human + agent mixed workflows)

If distributing across teams rather than individual agents:

| Stream | Tickets | Notes |
|---|---|---|
| Infra/Scaffolding | T-001, T-002, T-003, T-004, T-005 | Project config, no domain logic. Good candidate for full agent automation. |
| Core Abstractions | T-006, T-007, T-008, T-009, T-010, T-011, T-012 | Independent modules with clear interfaces. Highest parallelism potential. |
| Backends | T-013, T-014, T-015, T-022 | Docker and native behind the trait. T-014 and T-022 can run simultaneously after T-013. |
| CLI + Orchestration | T-016, T-017, T-018, T-019, T-020 | Wiring layer. T-017 is the integration bottleneck — assign to the most capable agent or human. |
| Hardening + E2E | T-021, T-023, T-024, T-025, T-026, T-027 | Test-heavy, runs last. T-025 can be pulled earlier (only needs T-008). |
