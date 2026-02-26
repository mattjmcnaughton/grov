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

# Build the Docker image for native integration tests
build-test-native:
    docker build -f Dockerfile.test-native -t grov-test-native .

# Run native backend integration tests in a Linux container
test-native: build-test-native
    docker run --rm -v {{justfile_directory()}}:/app -w /app grov-test-native \
        cargo test --test native_backend --features integration-tests

# Expensive gate: gate + integration/e2e tests
gate-expensive: gate
    cargo test --test '*' --features integration-tests
    just test-native

# External gate: tests requiring external components (Docker, filesystem, network)
gate-external: gate-expensive

# Run grov locally (currently uses Docker backend; args passed through)
run *ARGS: build
    cargo run -- {{ARGS}}

# Run grov with the native backend inside the Linux test container (args passed through)
run-native *ARGS: build-test-native
    docker run --rm -e GROV_BACKEND=native -v {{justfile_directory()}}:/app -w /app grov-test-native \
        cargo run -- {{ARGS}}
