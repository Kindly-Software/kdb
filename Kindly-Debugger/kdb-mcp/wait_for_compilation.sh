#!/bin/bash
# Wait for compilation to succeed, then run tests

PROJECT_DIR="/home/samuel/Primitives/atomic_mcp_server"
cd "$PROJECT_DIR"

echo "========================================="
echo "Waiting for Compilation Success"
echo "Started: $(date)"
echo "========================================="
echo ""

MAX_ATTEMPTS=60
ATTEMPT=0
SLEEP_INTERVAL=10

while [ $ATTEMPT -lt $MAX_ATTEMPTS ]; do
    ((ATTEMPT++))

    echo "[$ATTEMPT/$MAX_ATTEMPTS] Checking compilation at $(date +%H:%M:%S)..."

    ERROR_COUNT=$(cargo test --all-features --no-run 2>&1 | grep -c "^error\[")

    echo "  Errors: $ERROR_COUNT"

    if [ $ERROR_COUNT -eq 0 ]; then
        echo ""
        echo "========================================="
        echo "✓ COMPILATION SUCCESS!"
        echo "Time: $(date)"
        echo "========================================="
        echo ""

        # Optionally run tests immediately
        if [ "$1" = "--run-tests" ]; then
            echo "Starting test execution..."
            ./run_tests.sh
        fi

        exit 0
    fi

    if [ $ATTEMPT -lt $MAX_ATTEMPTS ]; then
        sleep $SLEEP_INTERVAL
    fi
done

echo ""
echo "========================================="
echo "✗ TIMEOUT: Compilation still failing after $((MAX_ATTEMPTS * SLEEP_INTERVAL)) seconds"
echo "Final error count: $ERROR_COUNT"
echo "========================================="
exit 1
