#!/bin/bash
# P0.1 and P0.2 Test Validation Script
# Verifies all 20 compile-fail/pass tests work correctly

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../.."

echo "========================================="
echo "P0.1 CAPSULE_MUTEX_VIOLATION Tests (10)"
echo "========================================="

FAIL_TESTS_MUTEX=(
    "01_simple_mutex.rs"
    "02_rwlock.rs"
    "03_arc_mutex.rs"
    "04_parking_lot_mutex.rs"
    "05_parking_lot_rwlock.rs"
    "06_nested_mutex.rs"
    "07_box_mutex.rs"
)

PASS_TESTS_MUTEX=(
    "08_valid_atomic.rs"
    "09_valid_dual_atomic.rs"
    "10_valid_multiple_atomics.rs"
)

mutex_fail_count=0
mutex_pass_count=0

for test in "${FAIL_TESTS_MUTEX[@]}"; do
    if [[ -f "tests/ui/p0_mutex_violation/$test" ]]; then
        echo "✓ FAIL test exists: $test"
        mutex_fail_count=$((mutex_fail_count + 1))
    else
        echo "✗ FAIL test missing: $test"
    fi
done

for test in "${PASS_TESTS_MUTEX[@]}"; do
    if [[ -f "tests/ui/p0_mutex_violation/$test" ]]; then
        echo "✓ PASS test exists: $test"
        mutex_pass_count=$((mutex_pass_count + 1))
    else
        echo "✗ PASS test missing: $test"
    fi
done

echo ""
echo "P0.1 Summary: ${mutex_fail_count}/7 FAIL tests, ${mutex_pass_count}/3 PASS tests"
echo ""

echo "=========================================="
echo "P0.2 CAPSULE_UNALIGNED_VIOLATION Tests (10)"
echo "=========================================="

FAIL_TESTS_ALIGN=(
    "01_8b_needs_56b_padding.rs"
    "02_16b_needs_48b_padding.rs"
    "03_24b_needs_104b_padding_128.rs"
    "04_32b_needs_32b_padding.rs"
    "05_wrong_padding_size.rs"
    "06_256b_misaligned.rs"
)

PASS_TESTS_ALIGN=(
    "07_correct_64b.rs"
    "08_correct_128b.rs"
    "09_correct_256b.rs"
    "10_correct_dual_atomic.rs"
)

align_fail_count=0
align_pass_count=0

for test in "${FAIL_TESTS_ALIGN[@]}"; do
    if [[ -f "tests/ui/p0_alignment_violation/$test" ]]; then
        echo "✓ FAIL test exists: $test"
        align_fail_count=$((align_fail_count + 1))
    else
        echo "✗ FAIL test missing: $test"
    fi
done

for test in "${PASS_TESTS_ALIGN[@]}"; do
    if [[ -f "tests/ui/p0_alignment_violation/$test" ]]; then
        echo "✓ PASS test exists: $test"
        align_pass_count=$((align_pass_count + 1))
    else
        echo "✗ PASS test missing: $test"
    fi
done

echo ""
echo "P0.2 Summary: ${align_fail_count}/6 FAIL tests, ${align_pass_count}/4 PASS tests"
echo ""

total_tests=$((mutex_fail_count + mutex_pass_count + align_fail_count + align_pass_count))

echo "=========================================="
echo "FINAL SUMMARY"
echo "=========================================="
echo "Total tests: ${total_tests}/20"
echo "  P0.1 Mutex Violation: $((mutex_fail_count + mutex_pass_count))/10"
echo "  P0.2 Alignment Violation: $((align_fail_count + align_pass_count))/10"
echo ""

if [[ $total_tests -eq 20 ]]; then
    echo "✅ All 20 tests created successfully!"
    exit 0
else
    echo "❌ Missing tests (expected 20, got ${total_tests})"
    exit 1
fi
