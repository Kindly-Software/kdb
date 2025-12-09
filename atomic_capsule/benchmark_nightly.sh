#!/bin/bash
# Simplified B32-compliant nightly compilation benchmark
set -e

echo "=== Nightly Compilation Benchmark ==="
echo "Hardware: Intel Ultra 7 155H"
echo "Date: $(date)"
echo ""

# Helper function
time_build() {
    cargo clean -q 2>/dev/null
    /usr/bin/time -f "%E" cargo +nightly build -q --features "$1" 2>&1 | grep -E "^[0-9]"
}

echo "Baseline (no features):"
baseline=$(time_build "")
echo "  ${baseline}"
echo ""

echo "const-hashing:"
const_time=$(time_build "const-hashing")
echo "  ${const_time}"
echo ""

echo "simd-hashing:"
simd_time=$(time_build "simd-hashing")
echo "  ${simd_time}"
echo ""

echo "nightly-all:"
all_time=$(time_build "nightly-all")
echo "  ${all_time}"
echo ""

echo "=== SUMMARY ==="
echo "Baseline:      ${baseline}"
echo "const-hashing: ${const_time}"
echo "simd-hashing:  ${simd_time}"
echo "nightly-all:   ${all_time}"
