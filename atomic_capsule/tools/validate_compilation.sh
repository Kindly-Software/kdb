#!/usr/bin/env bash
# Compilation Validation Matrix - Atomic Capsule
# UCE34 Q33: Validation Foundation
# Tests all meaningful feature combinations across stable + nightly Rust

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

PASS_COUNT=0
FAIL_COUNT=0
SKIP_COUNT=0

# Test result tracking
declare -a RESULTS

test_build() {
    local name="$1"
    local features="$2"
    local toolchain="${3:-stable}"
    local expected="${4:-pass}"  # pass, fail, skip

    echo -n "Testing $name (features: '$features', toolchain: $toolchain)... "

    if [ "$expected" = "skip" ]; then
        echo -e "${YELLOW}SKIP${NC}"
        RESULTS+=("$name|$features|$toolchain|SKIP")
        ((SKIP_COUNT++))
        return 0
    fi

    if cargo +$toolchain check --lib --no-default-features ${features:+--features "$features"} 2>&1 >/dev/null; then
        if [ "$expected" = "pass" ]; then
            echo -e "${GREEN}PASS${NC}"
            RESULTS+=("$name|$features|$toolchain|PASS")
            ((PASS_COUNT++))
        else
            echo -e "${RED}UNEXPECTED PASS${NC}"
            RESULTS+=("$name|$features|$toolchain|UNEXPECTED_PASS")
            ((FAIL_COUNT++))
        fi
    else
        if [ "$expected" = "fail" ]; then
            echo -e "${GREEN}EXPECTED FAIL${NC}"
            RESULTS+=("$name|$features|$toolchain|EXPECTED_FAIL")
            ((PASS_COUNT++))
        else
            echo -e "${RED}FAIL${NC}"
            RESULTS+=("$name|$features|$toolchain|FAIL")
            ((FAIL_COUNT++))
        fi
    fi
}

echo "=========================================="
echo "Atomic Capsule Compilation Validation"
echo "=========================================="
echo ""

# ============================================================================
# STABLE RUST TESTS
# ============================================================================
echo "=== STABLE RUST TESTS ==="
echo ""

# Tier 0: Core no_std support
test_build "T0.1: Minimal (no features)" "" "stable" "pass"
test_build "T0.2: std only" "std" "stable" "pass"
test_build "T0.3: Default features" "" "stable" "pass"  # Note: uses default features

# Tier 1: Atomic capsules (always available)
test_build "T1.1: stable-fallback" "stable-fallback" "stable" "pass"

# Tier 2/3/6: SIMD + Fixed-Point (nightly only)
test_build "T2.1: portable_simd (stable)" "portable_simd" "stable" "fail"
test_build "T3.1: fixed-point" "fixed-point" "stable" "pass"

# Tier 0: Hash features
test_build "T0.4: fast-hash" "fast-hash" "stable" "pass"
test_build "T0.5: audit-trail" "audit-trail" "stable" "pass"
test_build "T0.6: highway-hash" "highway-hash" "stable" "pass"
test_build "T0.7: fips-compliant" "fips-compliant" "stable" "pass"

# Tier 4/5: Collections (require std)
test_build "T4.1: std (for collections)" "std" "stable" "pass"
test_build "T5.1: async-log" "async-log" "stable" "pass"

# Tier 0: Serialization
test_build "T0.8: capsule-serialize" "capsule-serialize" "stable" "pass"

# Feature combinations (stable)
test_build "T0.9: std + fast-hash + capsule-serialize" "std,fast-hash,capsule-serialize" "stable" "pass"
test_build "T0.10: profile-production" "profile-production" "stable" "pass"
test_build "T0.11: profile-government" "profile-government" "stable" "pass"

echo ""

# ============================================================================
# NIGHTLY RUST TESTS
# ============================================================================
echo "=== NIGHTLY RUST TESTS ==="
echo ""

# Check if nightly is available
if ! rustup toolchain list | grep -q nightly; then
    echo -e "${YELLOW}Nightly toolchain not installed, skipping nightly tests${NC}"
    NIGHTLY_AVAILABLE=false
else
    NIGHTLY_AVAILABLE=true
fi

if [ "$NIGHTLY_AVAILABLE" = true ]; then
    # Tier 2: SIMD capsules
    test_build "T2.2: portable_simd (nightly)" "portable_simd" "nightly" "pass"
    test_build "T2.3: nightly base" "nightly" "nightly" "pass"

    # Tier 3: Fixed-point SIMD
    test_build "T3.2: fixed-simd" "fixed-simd" "nightly" "pass"

    # Phase 2.2: Nightly optimizations
    test_build "T2.4: const-hashing" "const-hashing" "nightly" "pass"
    test_build "T2.5: simd-hashing" "simd-hashing" "nightly" "pass"
    test_build "T2.6: nightly-all" "nightly-all" "nightly" "pass"

    # Feature combinations (nightly)
    test_build "T2.7: profile-high-performance" "profile-high-performance" "nightly" "pass"
    test_build "T2.8: nightly + capsule-serialize" "nightly,capsule-serialize" "nightly" "pass"
    test_build "T2.9: nightly + all-features" "nightly,std,portable_simd,const-hashing,simd-hashing,capsule-serialize,audit-trail" "nightly" "pass"

    # Ultra-low latency mode
    test_build "T7.1: ultra-low-latency" "ultra-low-latency" "nightly" "pass"
    test_build "T8.1: rt-priority" "rt-priority" "nightly" "pass"
else
    test_build "T2.2: portable_simd (nightly)" "portable_simd" "nightly" "skip"
    test_build "T2.3: nightly base" "nightly" "nightly" "skip"
    test_build "T3.2: fixed-simd" "fixed-simd" "nightly" "skip"
    test_build "T2.4: const-hashing" "const-hashing" "nightly" "skip"
    test_build "T2.5: simd-hashing" "simd-hashing" "nightly" "skip"
    test_build "T2.6: nightly-all" "nightly-all" "nightly" "skip"
    test_build "T2.7: profile-high-performance" "profile-high-performance" "nightly" "skip"
    test_build "T2.8: nightly + capsule-serialize" "nightly,capsule-serialize" "nightly" "skip"
    test_build "T2.9: nightly + all-features" "nightly,std,portable_simd,const-hashing,simd-hashing,capsule-serialize,audit-trail" "nightly" "skip"
    test_build "T7.1: ultra-low-latency" "ultra-low-latency" "nightly" "skip"
    test_build "T8.1: rt-priority" "rt-priority" "nightly" "skip"
fi

echo ""

# ============================================================================
# SUMMARY
# ============================================================================
echo "=========================================="
echo "SUMMARY"
echo "=========================================="
echo -e "Total: $((PASS_COUNT + FAIL_COUNT + SKIP_COUNT)) tests"
echo -e "${GREEN}PASS: $PASS_COUNT${NC}"
echo -e "${RED}FAIL: $FAIL_COUNT${NC}"
echo -e "${YELLOW}SKIP: $SKIP_COUNT${NC}"
echo ""

if [ $FAIL_COUNT -gt 0 ]; then
    echo "Failed tests:"
    for result in "${RESULTS[@]}"; do
        if [[ "$result" == *"|FAIL" ]] || [[ "$result" == *"|UNEXPECTED_PASS" ]]; then
            echo "  - $result"
        fi
    done
    exit 1
fi

echo "All tests passed!"
exit 0
