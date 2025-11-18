#!/bin/bash
#
# Measure Hybrid LSH Memory Usage
#
# Purpose: Validate Phase 4 memory scaling (<10 GB @ 1M docs)
# Uses /usr/bin/time -v to measure peak resident set size (RSS)
#
# Usage: bash tests/measure_hybrid_lsh_memory.sh
#

set -e

echo "=========================================="
echo "Hybrid LSH Memory Measurement (Phase 4)"
echo "=========================================="
echo ""

# Build release binary
echo "[1/4] Building release test binary..."
cargo build --release --test hybrid_lsh_integration 2>&1 | tail -5
echo ""

# Get binary path
BINARY="./target/release/deps/hybrid_lsh_integration-$(cargo metadata --format-version 1 | jq -r '.workspace_members[0]' | cut -d' ' -f1 | sed 's/-/_/g')"
if [ ! -f "$BINARY" ]; then
    # Fallback: try to find the binary
    BINARY=$(find ./target/release/deps -name "hybrid_lsh_integration-*" -type f -executable | head -1)
fi

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Could not find test binary at $BINARY"
    echo "Available binaries in target/release/deps:"
    ls -la ./target/release/deps/ | grep hybrid_lsh_integration
    exit 1
fi

echo "[2/4] Found binary: $BINARY"
echo ""

# Run memory-bounded test
echo "[3/4] Running memory-bounded test (1M documents simulation)..."
echo ""

# Capture timing output
/usr/bin/time -v "$BINARY" --test test_hybrid_lsh_memory_bounded --nocapture 2>&1 | tee /tmp/hybrid_memory_test.log

echo ""
echo "[4/4] Extracting memory metrics..."
echo ""

# Extract key metrics
if [ -f /tmp/hybrid_memory_test.log ]; then
    PEAK_RSS=$(grep "Maximum resident set size" /tmp/hybrid_memory_test.log | awk '{print $NF}')
    PEAK_RSS_KB=$(echo "$PEAK_RSS" | head -1)

    if [ -n "$PEAK_RSS_KB" ]; then
        PEAK_RSS_GB=$(echo "scale=2; $PEAK_RSS_KB / 1024 / 1024" | bc)
        echo "Peak resident set size: ${PEAK_RSS_KB} KB (${PEAK_RSS_GB} GB)"
        echo ""

        # Check against target (<10 GB)
        TARGET_GB=10
        if (( $(echo "$PEAK_RSS_GB < $TARGET_GB" | bc -l) )); then
            echo "✅ PASS: Memory usage ($PEAK_RSS_GB GB) < target ($TARGET_GB GB)"
            STATUS=0
        else
            echo "❌ FAIL: Memory usage ($PEAK_RSS_GB GB) >= target ($TARGET_GB GB)"
            STATUS=1
        fi
    else
        echo "ERROR: Could not parse peak RSS from test output"
        STATUS=1
    fi
else
    echo "ERROR: Test output not found"
    STATUS=1
fi

echo ""
echo "=========================================="
echo "Memory Measurement Complete"
echo "=========================================="
echo ""

# Print additional system info
echo "System Information:"
echo "  OS: $(uname -s)"
echo "  Kernel: $(uname -r)"
echo "  Memory: $(free -h | grep "^Mem:" | awk '{print $2}')"
echo "  CPU: $(nproc) cores @ $(lscpu | grep "^CPU max" | awk '{print $NF}')"
echo ""

exit $STATUS
