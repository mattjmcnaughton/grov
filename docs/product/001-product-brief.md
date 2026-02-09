# Product Brief: grov

## Executive Summary

Modern development workflows, particularly agentic engineering workflows, increasingly rely on git worktrees for parallel feature development, but existing service orchestration tools assume a single-worktree model with hardcoded ports and shared state. This creates constant friction: port conflicts, data pollution between experiments, and manual environment management. grov solves this by providing per-worktree service isolation with automatic port allocation, isolated data directories, and environment variable discovery. The tool serves both human developers via CLI and programmatic consumers (AI agents, IDE plugins, CI systems) via JSON output and HTTP API, positioning it as infrastructure for the emerging AI-assisted development ecosystem.

## Problem Statement

### The Problem

Developers working with multiple git worktrees cannot effectively use standard service orchestration tools. Docker Compose and similar tools hardcode ports (e.g., postgres:5432), meaning only one worktree can run services at a time. Developers must manually stop services, change ports, manage conflicting data directories, or accept constant conflicts.

Secondary problem: developers working in ephemeral environments (cloud sandboxes, Codespaces, remote development containers) often lack Docker entirely, yet still need backing services like Postgres and Redis.

### Impact

- **Port conflicts**: Developers waste time debugging "port already in use" errors and manually coordinating service lifecycles across worktrees
- **Data pollution**: Shared database directories mean one experiment can corrupt another, requiring manual cleanup and database resets
- **Context switching cost**: Every worktree switch requires manual environment reconfiguration (updating .env files, restarting services)
- **Blocked parallel work**: Developers cannot truly work on multiple features simultaneously when services conflict

This affects any developer using worktrees with backing services -- a pattern increasingly common in teams practicing trunk-based development or managing multiple long-lived feature branches.

### Current Solutions

| Solution | Limitation |
|----------|------------|
| Docker Compose per worktree | Requires manual port management, duplicate configs, no automatic env discovery |
| Single shared services | Data conflicts, can only develop one feature at a time |
| Cloud-hosted dev databases | Latency, cost, requires network connectivity |
| Manual port offset conventions | Error-prone, doesn't scale, no isolation guarantee |
| Devcontainers | Heavy, slow startup, overkill for service isolation |

## Opportunity Assessment

### Market Size

The addressable market includes:
- **Primary**: Professional developers using git worktrees with local backing services
- **Secondary**: AI coding agents and IDE integrations requiring programmatic service control
- **Tertiary**: Teams building cloud development environments needing lightweight service orchestration

The git worktree pattern has grown significantly as trunk-based development and parallel feature work become standard practice in high-velocity teams.

### Trends

1. **Worktree adoption**: Git worktrees are becoming the preferred method for parallel development, replacing long-lived branches
2. **AI-assisted development**: Coding agents (Claude Code, Cursor, GitHub Copilot Workspace) need programmatic access to development infrastructure
3. **Remote/cloud development**: Codespaces, Gitpod, and similar tools create demand for lightweight, Docker-optional service management
4. **Developer experience focus**: Teams invest heavily in reducing friction in local development workflows

### Competitive Landscape

| Alternative | Differentiation from grov |
|-------------|---------------------------|
| Docker Compose | No worktree awareness, hardcoded ports, no env discovery |
| Tilt | Kubernetes-focused, heavy for local dev, no worktree isolation |
| devenv/nix | Complex setup, learning curve, different problem scope |
| mise/asdf | Runtime version management, not service orchestration |
| Custom scripts | Fragile, per-project, no standardization |

grov differentiates through: (1) first-class worktree isolation, (2) dual interface for humans and machines, (3) zero-config port allocation, (4) optional Docker-free operation.

## Solution Vision

### Proposed Solution

grov is a service orchestrator purpose-built for worktree-based development. It manages backing services (Postgres, Redis, MinIO, DynamoDB) with automatic per-worktree isolation: unique ports, isolated data directories, and automatic environment variable generation.

### Unique Value Proposition

**"One command to isolated services for any worktree."**

grov eliminates the mental overhead of service management across parallel development contexts. Developers run `grov up` and get correctly configured, isolated services without thinking about ports, data directories, or environment variables.

### Key Capabilities

1. **Worktree-aware isolation**: Each worktree maps to a "grove" with its own ports, data directories, and configuration
2. **Automatic port allocation**: Dynamic or deterministic port assignment eliminates conflicts
3. **Environment discovery**: `grov env` generates correct connection strings for the current worktree
4. **Dual interface**: CLI for humans, JSON output and HTTP API for programmatic consumers
5. **Backend flexibility**: Docker by default, native processes as fallback for Docker-free environments

## Ideal User Profile

### Profile Summary

Senior backend developer at a 20-100 person software company, working on a monorepo or multi-service application. Uses git worktrees to work on 2-4 features simultaneously. Maintains a local development environment with Postgres, Redis, and occasionally other services. Values automation and tooling but has limited patience for complex setup.

### Key Characteristics

- Uses git worktrees (not just branches) for parallel development
- Runs 2-5 backing services locally (database, cache, object storage)
- Comfortable with CLI tools, expects Unix-philosophy composability
- May also use AI coding assistants that need programmatic service access
- Works across multiple projects with similar service needs

### User Journey

1. Clone repo, create worktree for feature work
2. Run `grov init` once to define service requirements
3. Run `grov up` to start isolated services for this worktree
4. Run `eval $(grov env)` or `grov env --format dotenv > .env` to configure application
5. Develop feature with fully isolated backing services
6. Switch to another worktree; services continue running independently
7. Run `grov down` when done; data persists for later

### Pain Points Addressed

- "I can't run services for two worktrees at once"
- "I have to remember which ports I assigned to which worktree"
- "My test database keeps getting polluted by other experiments"
- "I have to manually update .env files every time I switch context"
- "My AI agent can't programmatically check if services are running"

## Success Metrics

### North Star Metric

**Active groves per user per week**: Number of distinct worktree environments actively using grov services. Target: 3+ active groves indicates successful multi-worktree workflow adoption.

### Supporting Metrics

| Metric | Target | Rationale |
|--------|--------|-----------|
| Time to first successful `grov up` | < 2 minutes | Measures onboarding friction |
| Service start time | < 10 seconds | Measures operational speed |
| Port conflict incidents | 0 | Measures core value delivery |
| Retention at 30 days | > 60% | Measures sustained value |
| Programmatic API usage | > 20% of users | Measures ecosystem adoption |

### Tracking Approach

- CLI telemetry (opt-in) for usage patterns
- GitHub stars/issues as community engagement proxy

## Risks and Assumptions

### Key Assumptions

1. Developers using worktrees have recurring service conflict pain (validated via user interviews)
2. The CLI + programmatic API dual interface serves both human and machine consumers effectively
3. Docker is available for most users; native fallback covers the minority without it
4. Users will accept a new tool if it demonstrably reduces friction (low switching cost)
