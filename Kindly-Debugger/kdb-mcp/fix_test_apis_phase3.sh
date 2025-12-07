#!/bin/bash
# Fix Remaining API Mismatches - Phase 3
# Handle Result<(), u64> boolean conversions

set -e

echo "Fixing Phase 3 API mismatches (Result boolean conversions)..."

cd /home/samuel/Primitives/atomic_mcp_server

# ============================================================================
# Fix 10: assert!(!denied, ...) where denied is Result<(), u64>
# ============================================================================
echo "Fix 10: assert!(!result) → assert!(result.is_err())"

# Pattern: assert!(!variable, ...) where variable is a Result
# This is tricky because we need context to know if it's a Result
# Manual fix recommended for assert! statements

# Fix specific known cases:
find tests -name "*.rs" -type f -exec sed -i \
    's/assert!(!denied,/assert!(denied.is_err(),/g' {} +

find tests -name "*.rs" -type f -exec sed -i \
    's/assert!(!rate_limit,/assert!(rate_limit.is_err(),/g' {} +

# ============================================================================
# Fix 11: if denied { ... } where denied is Result<(), u64>
# ============================================================================
echo "Fix 11: if result { ... } → if result.is_err() { ... }"

# This requires manual review - can't safely automate without context

# ============================================================================
# Fix 12: Fix unused mut in common.rs
# ============================================================================
echo "Fix 12: Remove unused 'mut' qualifiers"
sed -i 's/mut f: F/f: F/g' tests/common.rs
sed -i 's/pub fn set_test_feature_flag(flag_name: &str, enabled: bool)/pub fn set_test_feature_flag(_flag_name: \&str, _enabled: bool)/g' tests/common.rs

echo ""
echo "✅ Phase 3 fixes applied!"
echo ""
echo "Remaining manual fixes needed:"
echo "  1. Review all Result<(), u64> usage in boolean contexts"
echo "  2. Change 'if result {...}' to 'if result.is_ok() {...}'"
echo "  3. Change '!result' to 'result.is_err()'"
echo ""
echo "Run compilation again to check progress:"
echo "  cargo test --no-run --all-features 2>&1 | grep 'error\[E' | wc -l"
