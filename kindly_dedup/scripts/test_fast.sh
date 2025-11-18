#!/bin/bash
# Fast test suite (P0: smoke + functional)
# Execution time: <60s
# Run on every commit for rapid feedback

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "========================================================================"
echo "  kindly_dedup Fast Test Suite (P0 Critical Tests)"
echo "========================================================================"
echo
echo "Project: $PROJECT_DIR"
echo "Time: $(date '+%Y-%m-%d %H:%M:%S')"
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

run_test() {
    local test_name="$1"
    local test_command="$2"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -n "[${TOTAL_TESTS}] Running $test_name... "

    if eval "$test_command" > /tmp/test_output.log 2>&1; then
        echo -e "${GREEN}✓ PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}✗ FAILED${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo "       Error output:"
        tail -20 /tmp/test_output.log | sed 's/^/         /'
    fi
}

# Change to project directory
cd "$PROJECT_DIR"

# 1. Compilation check
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Phase 1: Compilation Checks"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

run_test "Build library" "cargo build --lib --release 2>&1"
run_test "Check formatting" "cargo fmt --all -- --check 2>&1"

echo

# 2. Unit tests
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Phase 2: Unit Tests"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

run_test "P0 unit tests" "cargo test --lib p0_unit --release 2>&1"
run_test "Bloom filter unit tests" "cargo test --lib bloom --release 2>&1"

echo

# 3. Integration tests
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Phase 3: Integration Tests"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

run_test "P0 integration tests" "cargo test --test p0_integration --release 2>&1"

echo

# 4. Smoke tests
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Phase 4: Library Smoke Tests"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo

run_test "Library doc tests" "cargo test --doc --release 2>&1"

echo

# 5. Summary
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Fast Test Suite Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo
echo "Total Tests:  $TOTAL_TESTS"
echo -e "Passed:       ${GREEN}$PASSED_TESTS${NC}"
if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "Failed:       ${GREEN}0${NC}"
    echo
    echo -e "${GREEN}✓ ALL TESTS PASSED${NC}"
    echo "Status: READY FOR COMMIT"
    echo
    exit 0
else
    echo -e "Failed:       ${RED}$FAILED_TESTS${NC}"
    echo
    echo -e "${RED}✗ SOME TESTS FAILED${NC}"
    echo "Status: FIX ISSUES BEFORE COMMIT"
    echo
    exit 1
fi
