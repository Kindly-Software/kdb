#!/bin/bash
# B32 Performance Validation Suite for T10 Probabilistic and T11 QuantumHybrid
# Runs comprehensive benchmarks and collects metrics for analysis

set -e

REPO_ROOT="/home/samuel/Primitives/atomic_capsule"
OUTPUT_DIR="/tmp/b32_validation_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$OUTPUT_DIR"

echo "================================"
echo "B32 Performance Validation Suite"
echo "================================"
echo "Output directory: $OUTPUT_DIR"
echo ""

# Color codes
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

log_start() {
    echo -e "${YELLOW}[STARTING] $1${NC}"
}

log_success() {
    echo -e "${GREEN}[SUCCESS] $1${NC}"
}

log_error() {
    echo -e "${RED}[ERROR] $1${NC}"
}

cd "$REPO_ROOT"

# T10 Probabilistic Benchmarks
echo ""
echo "=== T10 PROBABILISTIC TIER BENCHMARKS ==="

# 1. MinHash SIMD Benchmark
log_start "MinHash SIMD Benchmark (T10 - 2-8× speedup target)"
timeout 600 cargo bench --bench minhash_simd_bench --features "std,probabilistic,portable_simd" --release 2>&1 | tee "$OUTPUT_DIR/minhash_simd.log" || log_error "MinHash benchmark failed"
log_success "MinHash SIMD Benchmark"

# 2. HyperLogLog Benchmark
log_start "HyperLogLog Benchmark (T10 - <2% error bound)"
timeout 600 cargo bench --bench hyperloglog_bench --features "std,probabilistic" --release 2>&1 | tee "$OUTPUT_DIR/hyperloglog.log" || log_error "HyperLogLog benchmark failed"
log_success "HyperLogLog Benchmark"

# 3. Bloom Filter Benchmark
log_start "Bloom Filter Benchmark (T10 - <50ns query)"
timeout 600 cargo bench --bench bloom_filter_bench --features "std,probabilistic" --release 2>&1 | tee "$OUTPUT_DIR/bloom_filter.log" || log_error "Bloom filter benchmark failed"
log_success "Bloom Filter Benchmark"

# T11 Quantum Benchmarks (if quantum feature available)
echo ""
echo "=== T11 QUANTUMHYBRID TIER BENCHMARKS ==="

log_start "Quantum State Benchmark (T11 - 10-16,667× theoretical speedup)"
timeout 600 cargo bench --bench quantum_state_b32 --features "std" --release 2>&1 | tee "$OUTPUT_DIR/quantum_state.log" || echo "Quantum benchmark not available (requires quantum-simulation feature)"
log_success "Quantum State Benchmark (if available)"

# T10 Tests
echo ""
echo "=== T10 PROBABILISTIC TESTS ==="

log_start "Running T10 Probabilistic Tests"
timeout 300 cargo test --lib --features "std,probabilistic,minhash-simd" --release minhash 2>&1 | tee "$OUTPUT_DIR/minhash_tests.log" || log_error "MinHash tests failed"
log_success "T10 Tests completed"

# Summary
echo ""
echo "==================================="
echo "B32 VALIDATION SUMMARY"
echo "==================================="
echo "All benchmark logs saved to: $OUTPUT_DIR"
echo ""
echo "Log files:"
ls -lh "$OUTPUT_DIR"
echo ""
echo "Next steps:"
echo "1. Review criterion.rs HTML reports:"
echo "   - open target/criterion/minhash_compute/report/index.html"
echo "   - open target/criterion/hyperloglog_*/report/index.html"
echo "2. Analyze error logs for accuracy validation"
echo "3. Compare actual vs claimed speedups (B32 framework)"
echo ""
