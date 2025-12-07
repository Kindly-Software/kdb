#!/bin/bash
# Test Execution Script for atomic_mcp_server
# Runs all test categories and generates comprehensive report

set -e

PROJECT_DIR="/home/samuel/Primitives/atomic_mcp_server"
REPORT_DIR="$PROJECT_DIR/test_reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

mkdir -p "$REPORT_DIR"

cd "$PROJECT_DIR"

echo "========================================="
echo "Test Execution - atomic_mcp_server"
echo "Timestamp: $(date)"
echo "========================================="
echo ""

# Function to run tests and capture output
run_test_category() {
    local category=$1
    local command=$2
    local output_file="$REPORT_DIR/${category}_${TIMESTAMP}.txt"

    echo "Running $category tests..."
    echo "Command: $command"
    echo "Output: $output_file"
    echo ""

    eval "$command" 2>&1 | tee "$output_file"

    local exit_code=${PIPESTATUS[0]}
    if [ $exit_code -eq 0 ]; then
        echo "✓ $category tests: PASSED"
    else
        echo "✗ $category tests: FAILED (exit code: $exit_code)"
    fi
    echo ""

    return $exit_code
}

# Track results
declare -A results
total_passed=0
total_failed=0

# Q1-Q7: Unit Tests
if run_test_category "unit" "cargo test --lib --all-features"; then
    results[unit]="PASS"
    ((total_passed++))
else
    results[unit]="FAIL"
    ((total_failed++))
fi

# Q8-Q14: Property Tests
if run_test_category "property" "cargo test proptest --all-features"; then
    results[property]="PASS"
    ((total_passed++))
else
    results[property]="FAIL"
    ((total_failed++))
fi

# Q15-Q21: Integration Tests (individual files)
integration_tests=(
    "component_integration"
    "failure_modes"
    "state_management"
    "concurrent_integration"
    "security_integration"
    "performance_integration"
    "configuration"
)

for test in "${integration_tests[@]}"; do
    if run_test_category "integration_${test}" "cargo test --test $test --all-features"; then
        results[integration_${test}]="PASS"
        ((total_passed++))
    else
        results[integration_${test}]="FAIL"
        ((total_failed++))
    fi
done

# Q22-Q28: Production Tests
if run_test_category "production" "cargo test --test production_tests --all-features"; then
    results[production]="PASS"
    ((total_passed++))
else
    results[production]="FAIL"
    ((total_failed++))
fi

# Security critical tests
if run_test_category "security_critical" "cargo test --test security_critical --all-features"; then
    results[security_critical]="PASS"
    ((total_passed++))
else
    results[security_critical]="FAIL"
    ((total_failed++))
fi

# Generate summary report
echo "========================================="
echo "Test Execution Summary"
echo "========================================="
echo "Total Test Suites: $((total_passed + total_failed))"
echo "Passed: $total_passed"
echo "Failed: $total_failed"
echo ""
echo "Results by Category:"
for category in "${!results[@]}"; do
    echo "  $category: ${results[$category]}"
done
echo ""

# Generate detailed report file
SUMMARY_FILE="$REPORT_DIR/summary_${TIMESTAMP}.md"
cat > "$SUMMARY_FILE" <<EOF
# Test Execution Summary - atomic_mcp_server

**Timestamp**: $(date)
**Total Test Suites**: $((total_passed + total_failed))
**Passed**: $total_passed
**Failed**: $total_failed
**Pass Rate**: $(echo "scale=2; $total_passed * 100 / ($total_passed + $total_failed)" | bc)%

## Results by Category

| Category | Status |
|----------|--------|
EOF

for category in "${!results[@]}"; do
    echo "| $category | ${results[$category]} |" >> "$SUMMARY_FILE"
done

cat >> "$SUMMARY_FILE" <<EOF

## Framework Compliance (T28)

- **Q1-Q7 (Unit Tests)**: $([ "${results[unit]}" = "PASS" ] && echo "✓ Complete" || echo "✗ Incomplete")
- **Q8-Q14 (Property Tests)**: $([ "${results[property]}" = "PASS" ] && echo "✓ Complete" || echo "✗ Incomplete")
- **Q15-Q21 (Integration Tests)**: $([ "${results[integration_component_integration]}" = "PASS" ] && echo "✓ Complete" || echo "✗ Incomplete")
- **Q22-Q28 (Production Tests)**: $([ "${results[production]}" = "PASS" ] && echo "✓ Complete" || echo "✗ Incomplete")

## Test Output Files

EOF

for file in "$REPORT_DIR"/*_${TIMESTAMP}.txt; do
    echo "- $(basename "$file")" >> "$SUMMARY_FILE"
done

echo ""
echo "Summary report: $SUMMARY_FILE"
echo ""

# Exit with failure if any tests failed
if [ $total_failed -gt 0 ]; then
    echo "OVERALL STATUS: FAILED ($total_failed test suites failed)"
    exit 1
else
    echo "OVERALL STATUS: SUCCESS (all test suites passed)"
    exit 0
fi
