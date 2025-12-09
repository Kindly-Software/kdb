#!/bin/bash
# Validation Script for KV Cache Compression Tests
# Run this after implementing the capsules

set -e

echo "=================================================="
echo "KV Cache Compression Tests - Validation Suite"
echo "=================================================="
echo ""

echo "Step 1: Add feature flags to Cargo.toml"
echo "----------------------------------------"
echo "Add the following to [features] section:"
echo ""
echo "  inference-kv-cache = [\"std\", \"nightly\"]"
echo ""
read -p "Press Enter when feature flag is added..."

echo ""
echo "Step 2: Verify test files compile"
echo "-----------------------------------"
cargo check --tests --features inference-kv-cache

echo ""
echo "Step 3: Run unit tests (Q1-Q7)"
echo "--------------------------------"
cargo test --test kv_cache_compression_tests --features inference-kv-cache unit_tests

echo ""
echo "Step 4: Run property tests (Q8-Q14) - requires proptest"
echo "---------------------------------------------------------"
echo "Add to Cargo.toml [dev-dependencies]:"
echo "  proptest = \"1.0\""
echo ""
read -p "Press Enter when proptest is added..."
cargo test --test kv_cache_compression_tests --features inference-kv-cache,proptest property_tests

echo ""
echo "Step 5: Run integration tests"
echo "-------------------------------"
cargo test --test kv_cache_compression_tests --features inference-kv-cache integration_tests

echo ""
echo "Step 6: Run GPU decompression tests"
echo "-------------------------------------"
cargo test --test kv_cache_gpu_decompression_tests --features inference-kv-cache

echo ""
echo "Step 7: Run performance smoke tests"
echo "-------------------------------------"
cargo test --test kv_cache_compression_tests --features inference-kv-cache perf_tests -- --nocapture

echo ""
echo "Step 8: Run ASSUM safety verification"
echo "---------------------------------------"
cargo test --test kv_cache_compression_tests --features inference-kv-cache assum_tests
cargo test --test kv_cache_gpu_decompression_tests --features inference-kv-cache assum_tests

echo ""
echo "Step 9: REMOTE EXECUTION (Mandatory for T28)"
echo "----------------------------------------------"
echo "Per CLAUDE.md § remote-execution-mandate, run on kindly-hub:"
echo ""
echo "  ssh samuel@kindly-hub \"cd ~/Primitives/atomic_capsule && cargo test --features inference-kv-cache\""
echo ""
read -p "Press Enter to execute remotely..."

ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && cargo test --test kv_cache_compression_tests --features inference-kv-cache"
ssh samuel@kindly-hub "cd ~/Primitives/atomic_capsule && cargo test --test kv_cache_gpu_decompression_tests --features inference-kv-cache"

echo ""
echo "=================================================="
echo "✅ ALL TESTS PASSED!"
echo "=================================================="
echo ""
echo "Test Coverage:"
echo "  - Unit Tests (Q1-Q7): 25 tests"
echo "  - Property Tests (Q8-Q14): 8 tests"
echo "  - Integration Tests: 5 tests"
echo "  - Performance Tests: 5 tests"
echo "  - ASSUM Safety: 6 tests"
echo "  - Total: 61 tests"
echo ""
echo "Next Steps:"
echo "  1. Run B32 benchmarks: cargo bench --bench kv_cache_compression_bench"
echo "  2. Implement Q15-Q21 integration tests (FlashAttention)"
echo "  3. Implement Q22-Q28 production tests (LongBench)"
echo "  4. Implement Q29-Q35 determinism tests (Loom)"
echo ""
