# PRD: Grov Steel Thread - Launch Postgres and MinIO

## Overview

### Summary
Implement the core grov workflow: a developer runs `grov up postgres minio` in a worktree, services start with isolated ports and data directories, `grov env` outputs connection details, and the developer connects to both services.

### Background
Grov is a greenfield Rust CLI project with comprehensive documentation but no implementation. Before building the full feature set, we need to validate that the core architectural concepts work together: grove identification, port allocation, data isolation, environment templating, and service lifecycle management across both Docker and native backends.

### Objective
Validate end-to-end integration of all system layers: CLI argument parsing, grove (worktree) hashing, port allocation, data directory isolation, environment variable templating, container/process lifecycle management, and health checking. Prove that the architectural vision in the README is implementable.

### Alignment
This steel thread establishes the foundation for all future grov development. Every subsequent feature (profiles, TOML parsing, additional services, HTTP API) builds on these validated integration points.

## Target Users

### Primary Persona
A developer working in a git worktree who needs postgres and minio running with isolated ports and data, without conflicting with other worktrees.

### Simplified User Story
As a developer working in a git worktree, I want to start postgres and minio with a single command so that I can develop against isolated backing services without port conflicts or data pollution.

## Steel Thread Scope

### The Single Path
1. Developer runs `grov install postgres minio` to ensure runtimes are available
2. Developer runs `grov up postgres minio` in their worktree
3. Grov identifies the grove (hashes worktree path), allocates ports, creates data directories
4. Grov starts postgres and minio containers/processes with isolated configuration
5. Grov waits for services to be healthy
6. Developer runs `grov env` to get connection environment variables
7. Developer connects to postgres and minio using the provided connection details
8. Developer runs `grov status` to verify services are running
9. Developer runs `grov down` to stop services (data preserved)

### In Scope (Minimal Features Only)
- **`grov install postgres minio`**: Install Docker images or native binaries for postgres and minio
- **`grov up postgres minio`**: Start both services with grove-isolated ports and data directories
- **`grov down [services...]`**: Stop services gracefully (preserve data)
- **`grov env`**: Output environment variables for service connections (dotenv format)
- **`grov status`**: Show running services, ports, and health status for current grove
- **Hardcoded service definitions**: Postgres 16 and MinIO with sensible defaults (no TOML parsing)
- **Docker backend**: Pull and run containers with mounted volumes
- **Native backend**: Start postgres via `initdb`/`pg_ctl` and minio binary (Linux only)
- **Grove identification**: Hash worktree path to create isolated environment
- **Dynamic port allocation**: Find available ports, avoid conflicts
- **Data directory isolation**: Store data in `~/.grov/store/<grove-hash>/data/<service>/`
- **Environment variable templating**: Substitute `{port}`, `{username}`, `{password}`, etc.
- **Health checking**: Wait for services to accept connections before returning

### Explicitly Out of Scope
- `grov.toml` parsing (hardcoded service definitions only)
- Multiple profiles (single default profile)
- `grov init` (no manifest creation)
- `grov exec` (no command execution against services)
- `grov doctor` (no setup validation)
- `grov reset`, `grov destroy` (no data cleanup commands)
- `grov ps` (no cross-grove service listing)
- `grov port` (use `grov env` instead)
- `grov config` (no config validation/display)
- `--json` output flag (human-readable only)
- `--quiet` flag
- HTTP API server
- Redis, DynamoDB, or other services
- Deterministic port allocation strategy
- `--clean` flag on `grov down`
- Custom usernames/passwords (use hardcoded defaults)
- Custom database names (use hardcoded defaults)
- Extension installation for postgres

## End-to-End Flow

### System Architecture
```
+------------------+     +-------------------+     +------------------+
|   CLI Layer      | --> |  Orchestration    | --> |  Backend Layer   |
|                  |     |     Layer         |     |                  |
| - Arg parsing    |     | - Grove ID        |     | - Docker client  |
| - Command router |     | - Port allocation |     | - Native process |
| - Output format  |     | - Data dirs       |     | - Health checks  |
|                  |     | - Env templating  |     |                  |
+------------------+     +-------------------+     +------------------+
                                  |
                                  v
                         +------------------+
                         |  Storage Layer   |
                         |                  |
                         | ~/.grov/store/   |
                         | <grove>/data/    |
                         +------------------+
```

### Flow Steps
1. **CLI receives command**: Parse `grov up postgres minio`, validate service names
2. **Identify grove**: Hash current worktree path to get grove identifier
3. **Resolve service definitions**: Load hardcoded postgres and minio configurations
4. **Allocate ports**: Find two available ports for postgres and minio
5. **Create data directories**: Ensure `~/.grov/store/<grove>/data/postgres/` and `minio/` exist
6. **Select backend**: Check if Docker is available, fall back to native if not (or respect explicit choice)
7. **Start services**: Launch containers or native processes with correct configuration
8. **Health check**: Poll services until they accept connections or timeout
9. **Record state**: Persist running service metadata (ports, PIDs, container IDs)
10. **Return success**: Output service status to terminal

### Integration Points
| Layer | Component | Responsibility |
|-------|-----------|----------------|
| CLI | Argument parser | Parse commands, services, flags |
| CLI | Command router | Dispatch to correct handler |
| Orchestration | Grove resolver | Hash worktree path, manage grove identity |
| Orchestration | Port allocator | Find available ports, track allocations |
| Orchestration | Data manager | Create/manage isolated data directories |
| Orchestration | Env templater | Substitute variables in environment strings |
| Backend | Docker client | Pull images, run containers, mount volumes |
| Backend | Native runner | Execute initdb, pg_ctl, minio binary |
| Backend | Health checker | TCP connect, protocol-specific readiness |
| Storage | File system | Grove directories, service data, state files |

## Requirements

### Functional Requirements (Minimal)
| ID | Requirement | Integration Layer |
|----|-------------|-------------------|
| F1 | `grov install postgres minio` pulls Docker images or downloads native binaries | CLI -> Backend |
| F2 | `grov up postgres minio` starts both services | CLI -> Orchestration -> Backend |
| F3 | Each worktree gets unique grove identifier from path hash | Orchestration |
| F4 | Services receive dynamically allocated non-conflicting ports | Orchestration |
| F5 | Service data persists in `~/.grov/store/<grove>/data/<service>/` | Orchestration -> Storage |
| F6 | Services bind to 127.0.0.1 only | Backend |
| F7 | `grov up` waits for services to be healthy before returning | Backend |
| F8 | `grov env` outputs DATABASE_URL, PGHOST, PGPORT, PGUSER, PGPASSWORD, PGDATABASE for postgres | CLI -> Orchestration |
| F9 | `grov env` outputs MINIO_ENDPOINT, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY for minio | CLI -> Orchestration |
| F10 | `grov status` shows service name, port, health status | CLI -> Orchestration |
| F11 | `grov down` stops services gracefully, preserves data | CLI -> Backend |
| F12 | `grov up` is idempotent (running twice doesn't create duplicate services) | Orchestration |
| F13 | Docker backend works on macOS and Linux | Backend |
| F14 | Native backend works on Linux | Backend |

### Non-Functional Requirements
- **Performance**: Service startup within 30 seconds acceptable; health check timeout of 60 seconds
- **Security**: Services bind to localhost only; no authentication beyond service defaults
- **Reliability**: Happy path focus; basic error messages for common failures (Docker not running, port unavailable)

## Technical Validation Goals

### Architectural Proof Points
- **Grove isolation works**: Two worktrees can run postgres simultaneously without conflict
- **Dynamic port allocation is reliable**: Ports are correctly allocated and don't collide
- **Data directory structure is sound**: Services find and use their data directories correctly
- **Environment templating is correct**: Generated connection strings actually work
- **Health checking is reliable**: Commands return only when services are ready
- **Dual backend architecture works**: Same orchestration layer drives both Docker and native

### Integration Validation
- **CLI to Orchestration**: Commands correctly parsed and routed
- **Orchestration to Backend**: Service configs correctly translated to Docker/native commands
- **Backend to Storage**: Data volumes correctly mounted and persisted
- **State persistence**: Service state survives process restarts (can query status after `grov up`)

### Technical Decisions to Make
- **State storage format**: JSON file vs SQLite for tracking running services
- **Docker client**: Bollard crate vs shelling out to `docker` CLI
- **Port allocation strategy**: Random available port vs deterministic hash-based
- **Health check protocol**: TCP connect vs protocol-specific (pg_isready, mc alias)

## Acceptance Criteria

### End-to-End Success Criteria
Given grov is installed and Docker is running
When developer runs `grov up postgres minio` in a worktree
Then both services start with isolated ports and data, `grov env` outputs valid connection strings, and the developer can connect to both postgres and minio

### CLI Integration Criteria

**grov install:**
```
Given Docker is available
When user runs `grov install postgres minio`
Then postgres:16-alpine and minio/minio images are pulled (or native binaries downloaded)
```

**grov up:**
```
Given no services are running for the current grove
When user runs `grov up postgres minio`
Then postgres starts on an available port
And minio starts on a different available port
And data directories are created at ~/.grov/store/<grove>/data/
And command returns only after both services are healthy
```

```
Given postgres and minio are already running for the current grove
When user runs `grov up postgres minio`
Then command succeeds without starting duplicate services (idempotent)
```

**grov env:**
```
Given postgres and minio are running for the current grove
When user runs `grov env`
Then output includes DATABASE_URL with correct port
And output includes PGHOST=localhost, PGPORT=<allocated-port>
And output includes MINIO_ENDPOINT=http://localhost:<allocated-port>
And output includes AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY
```

**grov status:**
```
Given postgres and minio are running for the current grove
When user runs `grov status`
Then output shows postgres with port and "healthy" status
And output shows minio with port and "healthy" status
```

**grov down:**
```
Given postgres and minio are running for the current grove
When user runs `grov down`
Then both services stop gracefully
And data directories are preserved
And `grov status` shows no running services
```

### Cross-Worktree Isolation Criteria
```
Given two terminal sessions in different worktrees (worktree-a and worktree-b)
When user runs `grov up postgres` in worktree-a
And user runs `grov up postgres` in worktree-b
Then both postgres instances run simultaneously on different ports
And each instance has isolated data in ~/.grov/store/<grove-a>/data/postgres/ and ~/.grov/store/<grove-b>/data/postgres/
```

### Backend-Specific Criteria

**Docker backend:**
```
Given Docker daemon is running
When user runs `grov up postgres minio`
Then containers are created with names including grove identifier
And volumes are mounted from ~/.grov/store/<grove>/data/
And containers are removed on `grov down` (but volumes preserved)
```

**Native backend (Linux):**
```
Given native postgres and minio are installed
When user runs `grov up postgres minio` with native backend
Then postgres cluster is initialized in data directory via initdb
And postgres starts via pg_ctl with custom port
And minio starts with data directory as positional argument
```

## Technical Considerations

### Architecture Decisions
- **Rust with async**: Use tokio for async I/O, enables concurrent service management
- **Bollard for Docker**: Native Rust Docker client, avoids shelling out
- **State as JSON**: Simple JSON files in `~/.grov/store/<grove>/state.json` for MVP
- **Clap for CLI**: Standard Rust CLI argument parsing

### Technology Stack
| Layer | Technology |
|-------|------------|
| CLI | Rust + clap |
| Async runtime | tokio |
| Docker client | bollard |
| JSON serialization | serde + serde_json |
| File system | std::fs + directories crate |
| Hashing | sha256 for grove IDs |

### Data Flow
1. CLI parses command and service names
2. Grove resolver hashes `pwd` to get grove ID
3. State manager loads `~/.grov/store/<grove>/state.json`
4. Port allocator checks state for existing allocations or finds new ports
5. Backend manager starts services with allocated ports and data paths
6. Health checker polls until services ready
7. State manager persists updated state
8. CLI outputs result

### Dependencies
- Docker daemon (for Docker backend)
- Native postgres installation (for native backend)
- Native minio binary (for native backend)

### Known Limitations
- Hardcoded service configurations (no customization)
- Single profile only
- No graceful handling of Docker daemon not running
- No cleanup of orphaned containers on crash
- No support for custom networks or container linking

## Implementation Plan

### Development Approach
Build bottom-up, validating each layer before moving up:
1. Storage layer (directories, state files)
2. Orchestration layer (grove, ports, env templating)
3. Backend layer (Docker first, then native)
4. CLI layer (commands and output)
5. Integration testing across layers

### Key Milestones
1. **M1 - Foundation**: Project structure, grove identification, data directories
2. **M2 - Docker backend**: Docker client integration, postgres container lifecycle
3. **M3 - Second service**: Add minio, validate multi-service orchestration
4. **M4 - Environment and status**: `grov env` and `grov status` commands
5. **M5 - Native backend**: Linux native postgres and minio support
6. **M6 - Polish**: Health checking, idempotency, error handling

### Testing Strategy
- Unit tests for grove hashing, port allocation, env templating
- Integration tests with Docker (requires Docker in CI)
- Manual testing of cross-worktree isolation
- End-to-end test script that runs the full user flow

## Launch Plan

### Deployment Strategy
- Publish to crates.io as `grov` once steel thread complete
- Provide pre-built binaries via GitHub releases
- Document installation in README

### Rollout
- Internal dogfooding first (use grov in grov development)
- Share with small group of multi-worktree power users
- Gather feedback before expanding feature set

### Success Metrics
- Steel thread complete when acceptance criteria pass
- Validates that two worktrees can run isolated postgres instances simultaneously
- Developer can connect to services using `grov env` output without manual configuration

## Next Steps After Steel Thread

### Features to Add
- `grov.toml` parsing for custom service configuration
- Multiple profiles support
- `grov init` for interactive manifest creation
- `grov exec` for running commands against services
- `grov doctor` for setup validation
- `--json` output for programmatic consumers
- Redis and DynamoDB service support
- HTTP API for IDE integration

### Optimizations Needed
- Parallel service startup
- Connection pooling for health checks
- Faster container startup with health check tuning

### Edge Cases to Handle
- Docker daemon not running (helpful error message)
- Port already in use (retry with different port)
- Data directory permissions issues
- Container name conflicts
- Graceful handling of interrupted `grov up`
- Cleanup of orphaned containers

## Open Questions
- Should `grov up` with no arguments start all services or show help?
- Should native backend be auto-detected or require explicit flag?
- What timeout is appropriate for health checks before failing?
- Should state.json include timestamps for debugging?
- How should `grov status` indicate a service that crashed after startup?
