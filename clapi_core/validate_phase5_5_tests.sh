#!/bin/bash
# Phase 5.5 Collections Migration - Test Validation Script
# Runs all T28 tests and reports results
# Framework: T28 Testing Framework (Q1-Q28)
# Status: Production-ready

set -e

echo "======================================================================"
echo "Phase 5.5 Collections Migration - T28 Test Validation"
echo "======================================================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

TOTAL_TESTS=0
TOTAL_PASSED=0
TOTAL_FAILED=0

# Function to run test suite and capture results
run_test_suite() {
    local test_name=$1
    local test_file=$2
    local extra_args=$3

    echo "----------------------------------------------------------------------"
    echo "Running: $test_name"
    echo "----------------------------------------------------------------------"

    # Run tests and capture output
    if cargo test --test "$test_file" $extra_args 2>&1 | tee /tmp/test_output.txt; then
        # Extract test counts from output
        local passed=$(grep -oP '\d+(?= passed)' /tmp/test_output.txt | tail -1)
        local failed=$(grep -oP '\d+(?= failed)' /tmp/test_output.txt | tail -1)

        # Default to 0 if not found
        passed=${passed:-0}
        failed=${failed:-0}

        local total=$((passed + failed))

        TOTAL_TESTS=$((TOTAL_TESTS + total))
        TOTAL_PASSED=$((TOTAL_PASSED + passed))
        TOTAL_FAILED=$((TOTAL_FAILED + failed))

        if [ "$failed" -eq 0 ]; then
            echo -e "${GREEN}✅ $test_name: $passed/$total pass (100%)${NC}"
        else
            echo -e "${RED}❌ $test_name: $passed/$total pass ($failed failures)${NC}"
        fi
    else
        echo -e "${RED}❌ $test_name: FAILED TO RUN${NC}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
    fi

    echo ""
}

# T1: Unit Tests (140 tests expected)
run_test_suite "T1: Unit Tests" "phase5_5_unit_tests" ""

# T2: Property Tests (60 tests expected)
run_test_suite "T2: Property Tests" "phase5_5_property_tests" ""

# T3: Integration Tests (40 tests expected)
run_test_suite "T3: Integration Tests" "phase5_5_integration_tests" ""

# T4: Stress Tests (24 tests expected)
echo "----------------------------------------------------------------------"
echo "Running: T4: Stress Tests (this may take 5+ minutes)"
echo "----------------------------------------------------------------------"
echo -e "${YELLOW}⚠️  Note: Stress tests are compute-intensive${NC}"
run_test_suite "T4: Stress Tests" "phase5_5_stress_tests" "-- --ignored"

# Summary
echo "======================================================================"
echo "SUMMARY"
echo "======================================================================"
echo ""
echo "Total Tests: $TOTAL_TESTS"
echo "Passed: $TOTAL_PASSED"
echo "Failed: $TOTAL_FAILED"
echo ""

if [ "$TOTAL_FAILED" -eq 0 ]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED (100%)${NC}"
    echo ""
    echo "Phase 5.5 T28 test suite: VALIDATION COMPLETE"
    echo "Status: Ready for migration"
    exit 0
else
    echo -e "${RED}❌ $TOTAL_FAILED TESTS FAILED${NC}"
    echo ""
    echo "Phase 5.5 T28 test suite: VALIDATION FAILED"
    echo "Action required: Fix failing tests before migration"
    exit 1
fi
