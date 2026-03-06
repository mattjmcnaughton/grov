# Product Backlog

Feature ideas and their status. Items marked "Needs PRD: Yes" require a PRD before implementation begins.

| # | Feature | Description | Needs PRD | Status |
|---|---------|-------------|-----------|--------|
| 1 | Expanded service support | Support services beyond MinIO and Postgres (e.g., Redis, DynamoDB, Elasticsearch) | No | Idea |
| 2 | Native multi-platform service installation | Support installing backing services (Postgres, MinIO, etc.) natively across multiple platforms (macOS, Linux, Windows) via the native backend | Yes | Idea |
| 3 | Compose file compatibility | Allow grov to read a `docker-compose.yml` and run `grov up` as a drop-in replacement for `docker compose up`, with per-worktree isolation | Yes | Idea |
