#!/bin/bash
# Full test suite (P0-P2: comprehensive testing)
# Execution time: <5 min
# Run on CI for main branch pre-release validation

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "========================================================================"
echo "  kindly_dedup Full Test Suite (P0-P2 Comprehensive)"
echo "========================================================================"
echo
echo "Project: $PROJECT_DIR"
echo "Time: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Hostname: $(hostname)"
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# Flags
VERBOSE=${VERBOSE:-false}
STOP_ON_FAILURE=${STOP_ON_FAILURE:-false}

run_test() {
    local test_name="$1"
    local test_command="$2"
    local optional="${3:-false}"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    printf "[%3d] %-50s " "$TOTAL_TESTS" "$test_name"

    if eval "$test_command" > /tmp/test_output.log 2>&1; then
        echo -e "${GREEN}✓${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        if [ "$optional" = "true" ]; then
            echo -e "${YELLOW}⊙${NC} (optional)"
            SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
        else
            echo -e "${RED}✗${NC}"
            FAILED_TESTS=$((FAILED_TESTS + 1))
            if [ "$VERBOSE" = "true" ]; then
                echo "       Error output:"
                tail -30 /tmp/test_output.log | sed 's/^/         /'
            fi
            if [ "$STOP_ON_FAILURE" = "true" ]; then
                exit 1
            fi
        fi
    fi
}

# Change to project directory
cd "$PROJECT_DIR"

# ==============================================================================
# Phase 1: Compilation and Style Checks
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 1: Compilation & Style Checks${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "Build library (release)" "cargo build --lib --release 2>&1"
run_test "Build binary (release)" "cargo build --bin kindly_dedup --release 2>&1"
run_test "Check formatting" "cargo fmt --all -- --check 2>&1"
run_test "Clippy: Warnings as errors" "cargo clippy --lib --release -- -D warnings 2>&1"
run_test "Clippy: Bin target" "cargo clippy --bin kindly_dedup --release -- -D warnings 2>&1"

# ==============================================================================
# Phase 2: Library Tests (P0-P2)
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 2: Library Tests (Unit/Property/Integration)${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# P0 Tests (Critical)
run_test "P0: Unit tests" "cargo test --lib p0_unit --release 2>&1"
run_test "P0: Property tests" "cargo test --lib p0_property --release 2>&1"
run_test "P0: Integration tests" "cargo test --test p0_integration --release 2>&1"
run_test "P0: Production tests" "cargo test --test p0_production --release 2>&1"

# P5 Tests (Collections)
run_test "P5: Unit tests" "cargo test --lib p5_unit --release 2>&1"
run_test "P5: Property tests" "cargo test --lib p5_property --release 2>&1"
run_test "P5: Integration tests" "cargo test --test p5_integration --release 2>&1"
run_test "P5: Production tests" "cargo test --test p5_production --release 2>&1"

# ==============================================================================
# Phase 3: Feature-Specific Tests
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 3: Feature Tests (SIMD, Bloom, Crypto)${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "SIMD: Integration tests" "cargo test --test simd_integration --release 2>&1"
run_test "SIMD: Property tests" "cargo test --test simd_property --release 2>&1"
run_test "Bloom: Unit tests" "cargo test --lib phase6_2_bloom --release 2>&1"
run_test "Bloom: Sharding tests" "cargo test --test bloom_shard --release 2>&1"
run_test "Crypto: License tests" "cargo test --test crypto_license --release 2>&1"
run_test "Audit: Trail tests" "cargo test --test audit_trail --release 2>&1"

# ==============================================================================
# Phase 4: Format and Pipeline Tests
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 4: Format & Pipeline Tests${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "Streaming Pipeline: Robustness" "cargo test --test streaming_pipeline_robustness --release 2>&1"
run_test "T5 Streaming: Comprehensive" "cargo test --test t5_comprehensive --release 2>&1"
run_test "Format Error Handling" "cargo test --test format_error_handling --release 2>&1"
run_test "Custom Data: Segfault regression" "cargo test --test custom_data_segfault --release 2>&1"

# ==============================================================================
# Phase 5: Protection and Safety Tests
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 5: Protection & Safety Tests${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "Obfuscation: Integration" "cargo test --test obfuscation_integration --release 2>&1" "true"
run_test "Encrypted State: Tests" "cargo test --test encrypted_state --release 2>&1"
run_test "Build Hardening: Tests" "cargo test --test build_hardening --release 2>&1"

# ==============================================================================
# Phase 6: Optional Advanced Tests
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 6: Optional Advanced Tests${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "Persistent Dedup: Tests" "cargo test --test persistent_dedup --release 2>&1" "true"
run_test "Disk-backed LSH: Integration" "cargo test --test disk_backed_lsh --release 2>&1" "true"
run_test "Bounded DocumentId: Integration" "cargo test --test bounded_docid_integration --release 2>&1" "true"
run_test "Bounded DocumentId: Stress" "cargo test --test bounded_docid_production_stress --release 2>&1" "true"
run_test "Hash Chain: Verification" "cargo test --test hash_chain_verification --release 2>&1" "true"

# ==============================================================================
# Phase 7: Binary Smoke Tests
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Phase 7: Binary Smoke Tests${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

run_test "Binary help" "./target/release/kindly_dedup --help 2>&1"
run_test "Binary version" "./target/release/kindly_dedup --version 2>&1"

# ==============================================================================
# Summary
# ==============================================================================
echo
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BLUE}Full Test Suite Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo
printf "Total:        %d\n" "$TOTAL_TESTS"
printf "%-13s ${GREEN}%d${NC}\n" "Passed:" "$PASSED_TESTS"
printf "%-13s ${RED}%d${NC}\n" "Failed:" "$FAILED_TESTS"
printf "%-13s ${YELLOW}%d${NC}\n" "Skipped:" "$SKIPPED_TESTS"
echo
echo "Duration: $(date '+%H:%M:%S')"
echo

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✓ FULL SUITE PASSED${NC}"
    echo "Status: READY FOR RELEASE"
    echo
    exit 0
else
    echo -e "${RED}✗ $FAILED_TESTS TEST(S) FAILED${NC}"
    echo "Status: FIX FAILURES BEFORE RELEASE"
    echo
    exit 1
fi
