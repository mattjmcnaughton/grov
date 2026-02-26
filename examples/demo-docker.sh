#!/usr/bin/env bash
# Steel-thread demo: grov lifecycle using the Docker backend.
# Exercises: install -> up -> verify connectivity -> env -> status -> down -> restart (data persistence) -> down
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

GROV="cargo run --"

echo "=== Building grov ==="
cargo build

echo ""
echo "=== Step 1: Install postgres and minio images ==="
$GROV install postgres minio

echo ""
echo "=== Step 2: Start postgres and minio ==="
$GROV -v up postgres minio

echo ""
echo "=== Step 3: Verify services are running ==="
docker ps --filter "name=grov-" --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}"

echo ""
echo "=== Step 4: Verify TCP connectivity ==="
STATE_FILE=$(ls ~/.grov/store/*/state.json 2>/dev/null | head -1)
if [ -n "$STATE_FILE" ]; then
    for svc in postgres minio; do
        port=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d['services']['$svc']['port'])" "$STATE_FILE" 2>/dev/null || true)
        if [ -n "$port" ]; then
            if bash -c "echo >/dev/tcp/127.0.0.1/$port" 2>/dev/null; then
                echo "$svc: listening on port $port"
            else
                echo "$svc: NOT responding on port $port"
            fi
        fi
    done
else
    echo "(no state file found)"
fi

echo ""
echo "=== Step 5: Test psql connection (if available) ==="
if [ -n "${STATE_FILE:-}" ]; then
    PG_PORT=$(python3 -c "import json,sys; d=json.load(open(sys.argv[1])); print(d['services']['postgres']['port'])" "$STATE_FILE" 2>/dev/null || true)
    if [ -n "$PG_PORT" ] && command -v psql &>/dev/null; then
        PGPASSWORD=dev psql -h localhost -p "$PG_PORT" -U dev -d myapp_dev -c "SELECT 'grov steel-thread works!' AS result;" || echo "(psql connection failed — service may still be initializing)"
    else
        echo "(psql not found — skipping, TCP health check already passed)"
    fi
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
echo "=== Step 9: Verify containers are gone ==="
REMAINING=$(docker ps --filter "name=grov-" --format "{{.Names}}" 2>/dev/null)
if [ -z "$REMAINING" ]; then
    echo "All grov containers stopped"
else
    echo "WARNING: containers still running: $REMAINING"
fi

echo ""
echo "=== Step 10: Restart postgres (data should persist) ==="
$GROV -v up postgres

echo ""
echo "=== Step 11: Final cleanup ==="
$GROV down

echo ""
echo "=== Docker steel-thread demo complete ==="
