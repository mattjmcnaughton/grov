# Execution Diary: Grov Steel Thread - Postgres and MinIO

Started: 2026-02-21

Plan source: docs/technical/001-tickets-steel-thread-postgres-minio.md
TDD reference: docs/technical/001-tdd-steel-thread-postgres-minio.md

## Execution Protocol

- Commits: One per ticket
- Check-ins: Pause after every ticket
- Gate (three-tier):
  - `just gate` -- fmt + clippy + unit tests
  - `just gate-expensive` -- integration/e2e tests
  - `just gate-external` -- tests requiring external components (Docker, filesystem, network)
- Before justfile exists (T-001, T-002): use raw cargo commands

## Wave Schedule

- Wave 1: T-001 (Crate structure)
- Wave 2: T-002, T-003, T-006, T-007, T-009, T-010, T-011, T-012
- Wave 3: T-008, T-013, T-015, T-016 (T-004, T-005, T-025 deferred to Wave 8)
- Wave 4: T-014, T-022
- Wave 5: T-017
- Wave 6: T-018, T-019, T-020, T-023, T-024
- Wave 7: T-021, T-026, T-027
- Wave 8 (cleanup): T-004, T-005, T-025

---

## T-001: Set up crate structure with module directories
**Status**: Completed
**What was done**:
- Updated Cargo.toml with all dependencies (clap, tokio, bollard, serde, serde_json, thiserror, anyhow, sha2, directories, which, tracing, tracing-subscriber, fs2) and dev-dependencies (tempfile, assert_cmd, predicates, tokio-test)
- Added `[features]` section with `integration-tests` feature
- Created src/lib.rs declaring pub mod cli, orchestration, backend, storage
- Created all module directories with mod.rs files pre-populated with pub mod declarations
- Created src/cli/mod.rs (pub mod commands), src/cli/commands/mod.rs
- Created src/orchestration/mod.rs (pub mod env_template, grove, port, service)
- Created src/backend/mod.rs (pub mod docker, health, native)
- Created src/storage/mod.rs (pub mod state)
- Created empty implementation files: grove.rs, port.rs, env_template.rs, service.rs, docker.rs, native.rs, health.rs, state.rs
- Created tests/common/mod.rs
- Updated src/main.rs with placeholder

**Rationale**:
- Pre-populated all pub mod declarations in mod.rs files to avoid file conflicts in later tickets (per TDD recommendation)
- Used Rust edition 2024 as specified in TDD
- All dependencies declared upfront so later tickets focus on implementation

**Issues/Deviations**:
- None

---

## T-002: Configure tracing with stderr output and verbosity flags
**Status**: Completed
**What was done**:
- Rewrote src/main.rs with clap Parser struct (Cli) with -v/--verbose flag using ArgAction::Count
- Implemented init_tracing() that maps verbosity: 0->warn, 1->info, 2+->debug
- RUST_LOG env var overrides flag-based level via EnvFilter::try_from_default_env()
- All tracing output directed to stderr via .with_writer(std::io::stderr)

**Rationale**:
- Stderr output is critical for `eval $(grov env)` correctness
- EnvFilter provides RUST_LOG support with fallback to flag level
- Minimal clap struct with just verbosity; full CLI parsing deferred to T-016

**Issues/Deviations**:
- None

---

## T-003: Set up justfile with gate and gate-expensive targets
**Status**: Completed
**What was done**:
- Created justfile with fmt, build, test, gate, gate-expensive, and gate-external targets
- gate: cargo fmt --check + cargo clippy -- -D warnings + cargo test --lib
- gate-expensive: gate + cargo test --test '*' --features integration-tests
- gate-external: gate-expensive (placeholder for Docker/network-dependent tests)

**Rationale**:
- Three-tier gate per user request: fast (gate), slow (gate-expensive), external (gate-external)
- gate-expensive depends on gate; gate-external depends on gate-expensive (cascading)

**Issues/Deviations**:
- Added gate-external as a third tier beyond the ticket spec per user request

---

## T-009: Implement ServiceDefinition and builtin service registry (refactored to trait-based)
**Status**: Completed
**What was done**:
- Refactored from plain data struct to trait-based design per user feedback
- Created Service trait in src/orchestration/services/mod.rs with default impls for native methods
- Created src/orchestration/services/postgres.rs with Postgres struct implementing Service
- Created src/orchestration/services/minio.rs with Minio struct implementing Service
- Kept src/orchestration/service.rs for shared types (ResolvedService, NativeInitStep)
- builtin_services() returns Vec<Box<dyn Service>>
- 12 unit tests across three files: registry (1), postgres (5), minio (5), plus existing state tests

**Rationale**:
- Trait-based design makes adding new services maximally easy: one file, one impl, one registry line
- Default impls on native_binary/native_args/native_init mean Docker-only services skip those
- Each service isolated in its own file for clean separation

**Issues/Deviations**:
- None

---

## T-011: Implement environment variable template rendering
**Status**: Completed
**What was done**:
- Added minijinja dependency to Cargo.toml
- Implemented render() in src/orchestration/env_template.rs using minijinja with strict undefined behavior
- Updated postgres and minio env_template values from {key} to {{ key }} (Jinja2 syntax)
- Updated postgres test assertions to match new syntax
- 6 unit tests: substitution, missing key error, no placeholders, multiple occurrences, empty, literal braces

**Rationale**:
- minijinja chosen over hand-rolled parser to avoid escaping complexity
- Strict undefined behavior ensures missing keys produce errors rather than silent empty strings

**Issues/Deviations**:
- Deviated from TDD's {key} syntax to {{ key }} (Jinja2) per user request to use minijinja

---

## T-007: Implement state types and JSON serialization
**Status**: Completed
**What was done**:
- Implemented GroveState, ServiceState, ServiceHandleState in src/storage/state.rs
- Derive Serialize, Deserialize, PartialEq, Clone, Debug on all types
- #[serde(default)] on services field for forward compatibility
- GroveState::new() constructor for empty state
- 6 unit tests: roundtrip, forward compat, empty state, missing fields default, Docker/Native handle serialization

**Rationale**:
- PartialEq derived for test assertions
- Forward compatibility via serde defaults and ignoring unknown fields

**Issues/Deviations**:
- None

---

## T-006: Implement grove identification (SHA-256 path hashing)
**Status**: Completed
**What was done**:
- Implemented resolve() and resolve_path() in src/orchestration/grove.rs
- SHA-256 hash of absolute path, truncated to 16 hex chars (first 8 bytes)
- Relative paths resolved to absolute before hashing
- 5 unit tests: determinism, isolation, length, lowercase hex, similar-path differentiation

**Rationale**:
- Used manual hex formatting instead of adding hex crate dependency (not in Cargo.toml)
- Exposed resolve_path() as public for testing with arbitrary paths

**Issues/Deviations**:
- None

---

## T-010: Implement port allocation (bind-to-0 strategy)
**Status**: Completed
**What was done**:
- Implemented allocate() in src/orchestration/port.rs using TcpListener::bind("127.0.0.1:0")
- Reads OS-assigned port, drops listener, returns port
- 3 unit tests: valid range, uniqueness, port release

**Rationale**:
- Minimal implementation per TDD spec
- Port released immediately after allocation (known TOCTOU race documented in TDD)

**Issues/Deviations**:
- None (note: this entry was retroactively added during session resume; commit existed but diary entry was missing)

---

## T-012: Implement error type hierarchy
**Status**: Completed
**What was done**:
- Defined GrovError enum in src/lib.rs with variants: Backend, Storage, HealthCheck, UnknownService, AlreadyRunning
- Defined BackendError enum in src/backend/mod.rs with variants: DockerUnavailable, PortUnavailable, StartFailed, StopFailed, Docker, BinaryNotFound
- Defined StorageError enum in src/storage/mod.rs with variants: Io (from std::io::Error), Serialization (from serde_json::Error), Lock
- Defined HealthCheckError enum in src/backend/health.rs with Timeout variant (custom Display impl for Duration formatting)
- Implemented From conversions: BackendError -> GrovError, StorageError -> GrovError, HealthCheckError -> GrovError, std::io::Error -> StorageError, serde_json::Error -> StorageError
- All error messages match TDD specification
- 15 new unit tests across three files: lib.rs (7), backend/mod.rs (6), storage/mod.rs (2)

**Rationale**:
- Used thiserror for all error types except HealthCheckError (needed custom Display for Duration::as_secs())
- Kept error types close to their modules per TDD guidance
- GrovError in lib.rs serves as the top-level error type for CLI exit code mapping

**Issues/Deviations**:
- HealthCheckError uses manual Display impl instead of thiserror because the elapsed field is Duration and the message needs to display seconds as an integer

---

## T-008: Implement StateManager with atomic writes and file locking
**Status**: Completed
**What was done**:
- Implemented StateManager in src/storage/mod.rs with new(), load_state(), save_state(), with_lock(), data_dir(), ensure_data_dir(), store_path()
- Atomic writes via temp file + rename pattern
- Exclusive advisory locking via fs2 on state.lock
- Home directory resolution via directories::BaseDirs
- Extracted constants: GROV_DIR, STORE_DIR, DATA_DIR, STATE_FILE, STATE_TMP_FILE, STATE_LOCK_FILE
- with_path() test-only constructor for temp dir isolation
- 7 unit tests: empty state load, save/load roundtrip, atomic write file existence, with_lock RMW, concurrent lock (4 threads), data_dir path, ensure_data_dir creation

**Rationale**:
- Used tempfile::tempdir() for all tests to avoid touching ~/.grov/
- Lock held only during JSON I/O (microseconds), not during service startup
- Constants extracted per user feedback

**Issues/Deviations**:
- None

---

## T-013: Implement Backend trait and ServiceHandle enum
**Status**: Completed
**What was done**:
- Defined Backend trait in src/backend/mod.rs with async methods: install, start, stop, is_running, backend_type
- Used native Rust async trait support (RPITIT) instead of async_trait macro
- Defined ServiceHandle enum with Docker { container_id } and Native { pid } variants
- Backend trait requires Send + Sync, all async methods return Send futures

**Rationale**:
- Rust 2024 edition supports return-position impl Trait in traits natively, avoiding async_trait dependency
- start() takes both &dyn Service and &ResolvedService so backends have access to service definition and resolved config

**Issues/Deviations**:
- None

---

## T-015: Implement health check (TCP connect polling)
**Status**: Completed
**What was done**:
- Implemented wait_until_healthy() in src/backend/health.rs
- Polls TcpStream::connect at configurable intervals (default 250ms) with configurable timeout (default 60s)
- Uses tokio::time::timeout wrapping an async poll loop
- Extracted DEFAULT_INTERVAL and DEFAULT_TIMEOUT constants
- 3 unit tests: healthy service detected, timeout on unhealthy, delayed readiness (listener starts after 300ms)

**Rationale**:
- Optional parameters with defaults for flexibility in tests vs production
- tokio::time::timeout cleanly handles the overall deadline

**Issues/Deviations**:
- None

---

## T-016: Implement CLI argument parsing with clap derive
**Status**: Completed
**What was done**:
- Moved Cli struct from main.rs to src/cli/mod.rs
- Added Commands enum with Install, Up, Down, Env, Status variants
- Up and Install require at least one service name
- Down services is Option<Vec<String>> -- None means stop all
- Global -v/-vv verbosity flags preserved
- main.rs reduced to thin entry point: parse, init tracing, debug log
- 9 unit tests: up/install/down/env/status parsing, verbosity flags, unknown command, up requires services

**Rationale**:
- Cli in library crate enables unit testing via parse_from/try_parse_from
- main.rs kept minimal per TDD architecture guidance

**Issues/Deviations**:
- None

---

## T-014: Implement Docker backend (DockerBackend)
**Status**: Completed
**What was done**:
- Implemented DockerBackend in src/backend/docker.rs with bollard crate
- DockerBackend::new() connects to Docker, pings to verify reachability
- create_client() checks DOCKER_HOST env var for non-standard socket paths (e.g., Colima), falls back to /var/run/docker.sock
- install(): checks if image exists locally via inspect_image, pulls via create_image stream if not present
- start(): inspects existing container by name, returns ID if running (idempotent), removes if stopped, creates new container with port binding (127.0.0.1 only), volume bind mount, and environment variables
- Container naming: grov-{grove_id first 8 chars}-{service_name}
- stop(): stops with 10s timeout, removes container, handles 304 (already stopped) and 404 (gone) gracefully
- is_running(): inspects container by ID, returns false for 404
- backend_type(): returns "docker"
- Added futures-util dependency for stream consumption (create_image returns a stream)
- 3 unit tests: container name with 8-char prefix, short grove ID, backend type
- 6 integration tests (feature-gated): install pulls image, container lifecycle, idempotent start, port binding accessible, data dir preserved after stop, stop nonexistent container

**Rationale**:
- DOCKER_HOST support enables Colima, remote Docker, and other non-default socket locations
- Image existence check in install() avoids unnecessary network calls
- Idempotent start matches TDD spec: running container reused, stopped container replaced
- Port bindings restricted to 127.0.0.1 for security (no network exposure)
- Graceful 304/404 handling in stop() makes it safe to call on already-stopped or removed containers
- Docker context discovery added to create_client(): resolves socket path via DOCKER_CONTEXT env var or currentContext in ~/.docker/config.json, then scans ~/.docker/contexts/meta/*/meta.json for matching context. This mirrors the Docker CLI's own resolution order and fixes connectivity on Colima setups where DOCKER_HOST is not set.

**Issues/Deviations**:
- Added futures-util to Cargo.toml (not in original T-001 dependency list) -- needed for bollard stream consumption
- Added DOCKER_HOST env var support to create_client() -- bollard's connect_with_local_defaults() only tries /var/run/docker.sock, which fails with Colima
- Added Docker context discovery as priority 2-3 between DOCKER_HOST and local defaults -- enables zero-config Colima support without requiring DOCKER_HOST to be set

---

## T-017: Implement Orchestrator struct and core command logic (up, down, install)
**Status**: Completed
**What was done**:
- Added chrono = "0.4" to Cargo.toml for ISO 8601 timestamps
- Implemented Orchestrator<B: Backend> generic struct in src/orchestration/mod.rs with backend, state_manager, and services fields
- Added bidirectional From impls between ServiceHandle (runtime) and ServiceHandleState (serializable)
- Helper functions: build_template_values() merges service defaults with allocated port; render_service_env() renders env_template entries through minijinja
- install(): validates service names, delegates to backend.install() sequentially
- up(): validates all names upfront (fail fast), checks existing state (skips if running, cleans stale state if dead), allocates port, ensures data dir, renders env templates, starts service, runs health check (stops on failure), saves state under lock with chrono timestamp
- down(): None stops all services from state, Some(names) validates against registry; loads handle from state, calls backend.stop(), removes from state under lock; skips services not in state (idempotent)
- find_service() looks up by name in builtin_services registry, returns UnknownService on miss
- Updated src/main.rs: switched to #[tokio::main] async fn main() -> ExitCode, extracted async fn run() that resolves grove ID, creates StateManager, DockerBackend, and Orchestrator, dispatches Install/Up/Down to orchestrator methods, Env/Status are placeholders
- Changed StateManager::with_path from private to pub(crate) for cross-module test access
- Configurable MockBackend with is_running_returns flag and bind_listener option (binds real TCP listener on allocated port so health check passes)
- seed_service_state() test helper for pre-populating state
- 17 unit tests total: find_service (2), install (2), up (4: happy path, skip already running, stale state cleanup, fail-fast validation), down (4: unknown name error, skip not in state, stop and remove state, stop all with None), conversions (2), helpers (2)

**Rationale**:
- Orchestrator is generic over B: Backend because Backend trait uses RPITIT which is not dyn-compatible
- Port allocation errors mapped to BackendError::StartFailed since no dedicated port error variant exists
- up() skips already-running services with continue (not error) so multi-service commands proceed
- Grove ID read once before the loop to avoid redundant load_state() calls
- State saved per-service under lock (not batched) for crash safety
- MockBackend with_listener() enables unit testing the up() happy path by providing a real TCP endpoint for the health check

**Issues/Deviations**:
- Initial implementation returned AlreadyRunning error from up() instead of skipping; fixed during self-review to use continue per plan spec
- Initial implementation had redundant load_state() call inside loop for grove_id; fixed to read once before loop
- Eliminated unnecessary clones in state-save closure by moving values instead

---

## T-018: Implement env and status command handlers
**Status**: Completed
**What was done**:
- Added EnvEntry, ServiceStatus, and ServiceRunState types in src/orchestration/mod.rs before the Orchestrator struct
- ServiceRunState has Display impl producing "running"/"stopped"
- env() method: loads persisted state, iterates services sorted by name, renders env templates via existing render_service_env() using persisted ports, sorts keys within each service, returns Vec<EnvEntry>
- status() method: loads persisted state, iterates services sorted by name, calls backend.is_running() for each handle, returns Vec<ServiceStatus> using backend_type from persisted state (not current backend)
- Updated dispatch() in src/main.rs: Env prints KEY=VALUE lines, Status prints "No running services." or a dynamically-width-formatted table (SERVICE, BACKEND, STATUS, PORT columns)
- Added .dockerignore to exclude target/, .git/, .claude/, .agentic-wip/, docs/, examples/, tests/, README.md, justfile from Docker build context
- 9 new unit tests: env empty/single/multi-sorted/unknown-error, status empty/running/stopped/multi-sorted, ServiceRunState display

**Rationale**:
- env() does not call is_running() per plan (stale state detection deferred to T-023)
- status() uses svc_state.backend_type from persisted state to reflect actual backend used at start time
- Template render errors mapped to BackendError::StartFailed (pragmatic; templates are code-defined so errors shouldn't occur)
- .dockerignore added because Dockerfile.test-native only COPYs Cargo.toml and Cargo.lock; target/ alone can be gigabytes

**Issues/Deviations**:
- None

---

