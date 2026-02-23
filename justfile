# Format code
fmt:
    cargo fmt

# Build the project
build:
    cargo build

# Run unit tests
test:
    cargo test --lib

# Fast pre-commit gate: fmt + clippy + unit tests
gate:
    cargo fmt --check
    cargo clippy -- -D warnings
    cargo test --lib

# Expensive gate: gate + integration/e2e tests (no external deps)
gate-expensive: gate
    cargo test --test '*' --features integration-tests

# External gate: tests requiring external components (Docker, filesystem, network)
gate-external: gate-expensive
