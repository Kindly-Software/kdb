#!/bin/bash
# Run full chaos test suite on kindly-hub (requires root)
# UCE34 Framework: T28 Q22-Q28 Production tier testing
# Location: Run on kindly-hub (192.168.0.38) for consistent hardware
set -euo pipefail

echo "=== Kindly-Debugger Full Chaos Test Suite ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "Host: $(hostname)"
echo "User: $(whoami)"
echo ""

# Verify we're on the right host
if [[ "$(hostname)" != "kindly-hub" ]]; then
    echo "WARNING: Not running on kindly-hub. For consistent results, run via:"
    echo "  ssh samuel@kindly-hub '~/Primitives/Kindly-Debugger/scripts/run_full_chaos_suite.sh'"
    echo ""
fi

# Navigate to kdb directory
cd /home/samuel/Primitives/Kindly-Debugger/kdb

# P0: Data Integrity (must pass)
echo "=== P0: Data Integrity Tests (non-privileged) ==="
echo "Running chaos tests that don't require root..."
cargo test --release --features chaos-testing --test chaos_tests -- --test-threads=1
P0_EXIT=$?
echo ""

# P0 with privileged tests (requires root)
echo "=== P0: Privileged Tests (requires sudo) ==="
if [[ $EUID -eq 0 ]]; then
    echo "Running as root..."
    cargo test --release --features chaos-testing --test chaos_tests -- --test-threads=1 --ignored
    P1_EXIT=$?
else
    echo "Running privileged tests with sudo..."
    sudo -E $(which cargo) test --release --features chaos-testing --test chaos_tests -- --test-threads=1 --ignored
    P1_EXIT=$?
fi
echo ""

# Summary
echo "=========================================="
echo "=== Chaos Test Suite Results ==="
echo "=========================================="
echo "P0 (non-privileged): $([ $P0_EXIT -eq 0 ] && echo 'PASS' || echo 'FAIL')"
echo "P0 (privileged):     $([ $P1_EXIT -eq 0 ] && echo 'PASS' || echo 'FAIL')"
echo ""

# Exit with P0 status (privileged tests are informational)
if [[ $P0_EXIT -ne 0 ]]; then
    echo "CRITICAL: P0 tests failed - data integrity at risk"
    exit 1
fi

if [[ $P1_EXIT -ne 0 ]]; then
    echo "WARNING: Privileged tests failed - some chaos scenarios not covered"
    echo "This is expected if running without CAP_SYS_PTRACE capability"
fi

echo "Chaos test suite completed successfully"
exit 0
