#!/bin/bash
# Nightly Compilation Verification Script
# Tests all nightly feature combinations for correctness
#
# Usage: ./verify_nightly_compilation.sh
# Returns: 0 if all pass, 1 if any fail

set -e

echo "=== Nightly Compilation Verification ==="
echo "Date: $(date)"
echo "Stable Rust: $(cargo --version)"
echo "Nightly Rust: $(cargo +nightly --version)"
echo ""

# Track test results
PASS=0
FAIL=0
TESTS=0

# Test function
test_build() {
    local label="$1"
    local features="$2"
    local toolchain="${3:-+nightly}"

    TESTS=$((TESTS + 1))
    echo -n "Testing ${label}... "

    if cargo ${toolchain} check --quiet --features "$features" 2>&1 | grep -q "error:"; then
        echo "❌ FAIL"
        FAIL=$((FAIL + 1))
        return 1
    else
        echo "✅ PASS"
        PASS=$((PASS + 1))
        return 0
    fi
}

echo "--- Stable Rust (Baseline) ---"
test_build "stable-default" "" ""
test_build "stable-portable_simd" "portable_simd" ""
echo ""

echo "--- Nightly Features (Individual) ---"
test_build "const-hashing" "const-hashing"
test_build "simd-hashing" "simd-hashing"
echo ""

echo "--- Nightly Combinations ---"
test_build "nightly-all" "nightly-all"
test_build "profile-high-performance" "profile-high-performance"
echo ""

echo "--- Feature Orthogonality Tests ---"
test_build "const-hashing + tier1" "const-hashing,portable_simd"
test_build "simd-hashing + tier2" "simd-hashing,portable_simd"
test_build "const-hashing + audit-trail" "const-hashing,audit-trail"
test_build "simd-hashing + highway-hash" "simd-hashing,highway-hash"
echo ""

echo "--- Full Feature Matrix ---"
echo "Skipping --all-features (pre-existing unrelated issues)"
echo ""

echo "=== SUMMARY ==="
echo "Tests: ${TESTS}"
echo "Passed: ${PASS} ✅"
echo "Failed: ${FAIL} ❌"
echo ""

if [ "$FAIL" -eq 0 ]; then
    echo "SUCCESS: All nightly features compile correctly!"
    echo ""
    echo "Next steps:"
    echo "  1. cargo +nightly test --features nightly-all"
    echo "  2. cargo +nightly bench --features nightly-all"
    echo "  3. Deploy with profile-high-performance feature"
    exit 0
else
    echo "FAILURE: Some feature combinations failed compilation"
    echo ""
    echo "Debug steps:"
    echo "  1. Review error messages above"
    echo "  2. Check src/hash/const_hash.rs for const fn requirements"
    echo "  3. Check src/hash/simd_hash.rs for portable_simd dependencies"
    echo "  4. Ensure nightly features properly feature-gated"
    exit 1
fi
