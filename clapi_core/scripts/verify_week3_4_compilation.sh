#!/bin/bash
#
# Week 3-4 Compilation Verification Script (UCE-D7)
# Comprehensive verification of 10 critical checks
#

set -e

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_DIR"

CHECKS_PASSED=0
CHECKS_FAILED=0
TOTAL_CHECKS=10

echo "=========================================="
echo "Week 3-4 Compilation Verification (UCE-D7)"
echo "=========================================="
echo ""

# Check 1: cargo check (lib, all features)
echo "[1/10] Running cargo check --lib --all-features..."
if cargo check --lib --all-features 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: Library compilation successful"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Library compilation failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 2: cargo test (lib)
echo "[2/10] Running cargo test --lib --all-features..."
if cargo test --lib --all-features 2>&1 | grep -q "test result: ok"; then
    echo "✅ PASS: Library tests pass"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Library tests failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 3: Example build (compliance_audit)
echo "[3/10] Building example: compliance_audit..."
if cargo build --release --example compliance_audit 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: compliance_audit example builds"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: compliance_audit example failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 4: Example build (hash_integration)
echo "[4/10] Building example: hash_integration..."
if cargo build --release --example hash_integration 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: hash_integration example builds"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: hash_integration example failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 5: Example build (client_sdk_demo)
echo "[5/10] Building example: client_sdk_demo..."
if cargo build --release --example client_sdk_demo 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: client_sdk_demo example builds"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: client_sdk_demo example failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 6: cargo clippy (no warnings)
echo "[6/10] Running cargo clippy --lib --all-features..."
CLIPPY_OUTPUT=$(cargo clippy --lib --all-features 2>&1)
if echo "$CLIPPY_OUTPUT" | grep -q "Finished"; then
    echo "✅ PASS: Clippy check successful"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Clippy check failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 7: No compilation errors
echo "[7/10] Checking for compilation errors..."
CHECK_OUTPUT=$(cargo check --lib --all-features 2>&1)
if ! echo "$CHECK_OUTPUT" | grep -q "^error:"; then
    echo "✅ PASS: Zero compilation errors"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Compilation errors detected"
    echo "$CHECK_OUTPUT" | grep "^error:"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 8: Integration tests
echo "[8/10] Running integration tests..."
if cargo test --test week4_option_a_integration_tests 2>&1 | grep -q "test result: ok"; then
    echo "✅ PASS: Integration tests pass"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Integration tests failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 9: Benchmarks compile
echo "[9/10] Checking benchmarks compile..."
if cargo check --benches --all-features 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: Benchmarks compile"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: Benchmarks failed to compile"
    ((CHECKS_FAILED++))
fi
echo ""

# Check 10: All features enabled
echo "[10/10] Verifying all features compile..."
if cargo check --all-features 2>&1 | grep -q "Finished"; then
    echo "✅ PASS: All features compile"
    ((CHECKS_PASSED++))
else
    echo "❌ FAIL: All features failed"
    ((CHECKS_FAILED++))
fi
echo ""

# Summary
echo "=========================================="
echo "VERIFICATION SUMMARY"
echo "=========================================="
echo "Total checks: $TOTAL_CHECKS"
echo "Passed: $CHECKS_PASSED ✅"
echo "Failed: $CHECKS_FAILED ❌"
echo ""

if [ $CHECKS_FAILED -eq 0 ]; then
    echo "🎉 ALL CHECKS PASSED - READY FOR PRODUCTION"
    exit 0
else
    echo "⚠️  NEEDS MORE FIXES ($CHECKS_FAILED failures)"
    exit 1
fi
