#!/usr/bin/env bash
# Steel-thread demo: grov native backend lifecycle.
# Builds the Linux test container and runs the full lifecycle inside it.
# Exercises: install -> up -> env -> status -> down -> restart (data persistence) -> down
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

IMAGE="grov-test-native"

echo "=== Building test container ==="
docker build -f Dockerfile.test-native -t "$IMAGE" .

echo ""
echo "=== Running native steel-thread demo inside container ==="
docker run --rm -e GROV_BACKEND=native -v "${REPO_ROOT}:/app" -w /app "$IMAGE" \
    bash -c '
set -euo pipefail
GROV="cargo run --"

echo "=== Step 1: Building grov ==="
cargo build 2>&1

echo ""
echo "=== Step 2: Verify native binaries are available ==="
$GROV install postgres minio

echo ""
echo "=== Step 3: Start postgres and minio (native) ==="
$GROV -v up postgres minio

echo ""
echo "=== Step 4: Verify processes are running ==="
echo "--- Processes ---"
ps aux | grep -E "(postgres|minio)" | grep -v grep || true
echo ""
echo "--- State file ---"
cat ~/.grov/store/*/state.json 2>/dev/null | python3 -m json.tool 2>/dev/null || cat ~/.grov/store/*/state.json 2>/dev/null || echo "(no state file found)"

echo ""
echo "=== Step 5: Verify TCP connectivity ==="
# Extract ports from state file
STATE_FILE=$(ls ~/.grov/store/*/state.json 2>/dev/null | head -1)
if [ -n "$STATE_FILE" ]; then
    for svc in postgres minio; do
        port=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d[\"services\"][\"$svc\"][\"port\"])" "$STATE_FILE" 2>/dev/null || true)
        if [ -n "$port" ]; then
            if bash -c "echo >/dev/tcp/127.0.0.1/$port" 2>/dev/null; then
                echo "$svc: listening on port $port ✓"
            else
                echo "$svc: NOT responding on port $port ✗"
            fi
        fi
    done
fi

echo ""
echo "=== Step 6: Print environment variables ==="
$GROV env || true

echo ""
echo "=== Step 7: Show service status ==="
$GROV status || true

echo ""
echo "=== Step 8: Stop all services ==="
$GROV down

echo ""
echo "=== Step 9: Verify processes are gone ==="
if ps aux | grep -E "(postgres|minio)" | grep -v grep > /dev/null 2>&1; then
    echo "WARNING: processes still running"
    ps aux | grep -E "(postgres|minio)" | grep -v grep
else
    echo "All service processes stopped ✓"
fi

echo ""
echo "=== Step 7: Restart postgres (data should persist) ==="
$GROV -v up postgres

echo ""
echo "=== Step 8: Final cleanup ==="
$GROV down

echo ""
echo "=== Native steel-thread demo complete ==="
'
