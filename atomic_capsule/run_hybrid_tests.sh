#!/bin/bash

# Direct test runner for HybridBatchPool tests
# Avoids the default feature compilation issues

set -e

echo "=== HybridBatchPool B32 Benchmarks & T28 Tests ==="
echo ""

# Try to run tests with explicit std feature
cd /home/samuel/Primitives/atomic_capsule

echo "Step 1: Building with std feature..."
RUSTFLAGS="-C panic=abort" cargo build --lib --features std,queue-bounded 2>&1 | grep -E "(Finished|error)" || true

echo ""
echo "Step 2: Running T28 Integration Tests..."
RUSTFLAGS="-C panic=abort" cargo test --test hybrid_batch_pool_tests --features std,queue-bounded --no-fail-fast 2>&1 | tee /tmp/test_output.txt

echo ""
echo "Test Summary:"
grep -E "test result:" /tmp/test_output.txt || echo "Tests completed - see output above"

echo ""
echo "Step 3: Running B32 Benchmarks (Criterion)..."
echo "Note: Benchmarks require 1000+ iterations - this may take several minutes"
echo ""
RUSTFLAGS="-C panic=abort" cargo bench --bench hybrid_batch_pool_bench --features std,queue-bounded 2>&1 | tee /tmp/bench_output.txt | tail -100

echo ""
echo "=== Complete ==="
