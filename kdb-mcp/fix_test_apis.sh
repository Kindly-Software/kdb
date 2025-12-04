#!/bin/bash
# Fix API Mismatches in All Test Files
# Applies systematic fixes based on actual API signatures

set -e

echo "Fixing API mismatches in atomic_mcp_server tests..."

# Navigate to atomic_mcp_server directory
cd /home/samuel/Primitives/atomic_mcp_server

# ============================================================================
# Fix 1: license.validate(key) → license.validate_key(key)
# ============================================================================
echo "Fix 1: license.validate(key) → license.validate_key(key)"
find tests -name "*.rs" -type f -exec sed -i \
    's/\blicense\.validate(\([^)]*\))/license.validate_key(\1)/g' {} +

# ============================================================================
# Fix 2: json_rpc.parse(x) → json_rpc.parse_request(x)
# ============================================================================
echo "Fix 2: json_rpc.parse() → json_rpc.parse_request()"
find tests -name "*.rs" -type f -exec sed -i \
    's/\bjson_rpc\.parse(/json_rpc.parse_request(/g' {} +

# ============================================================================
# Fix 3: quota.increment() → quota.check_and_increment(bytes)
# ============================================================================
echo "Fix 3: quota.increment() → quota.check_and_increment(1)"
find tests -name "*.rs" -type f -exec sed -i \
    's/\bquota\.increment()/quota.check_and_increment(1)/g' {} +

# ============================================================================
# Fix 4: quota.current() → quota.get_stats().current_requests
# ============================================================================
echo "Fix 4: quota.current() → quota.get_stats().current_requests"
find tests -name "*.rs" -type f -exec sed -i \
    's/\bquota\.current()/quota.get_stats().current_requests/g' {} +

# ============================================================================
# Fix 5: rate_limiter.check() result - Result<(), u64> not bool
# ============================================================================
echo "Fix 5: !rate_limiter.check(x) → rate_limiter.check(x).is_err()"
find tests -name "*.rs" -type f -exec sed -i \
    's/!rate_limiter\.check(\([^)]*\))/rate_limiter.check(\1).is_err()/g' {} +

echo "Fix 5b: rate_limiter.check(x) in boolean context → rate_limiter.check(x).is_ok()"
# This requires more careful replacement - we'll do specific fixes in affected files

# ============================================================================
# Fix 6: tools.lookup() returns Option<ToolHandle>, not Result
# ============================================================================
echo "Fix 6: tools.lookup() returns Option, not Result"
# Manual fix needed - check individual files

echo ""
echo "✅ Automated fixes applied!"
echo ""
echo "Manual fixes still needed:"
echo "  1. Review rate_limiter.check() usage in boolean contexts"
echo "  2. Review tools.lookup() error handling (Option vs Result)"
echo "  3. Verify quota.check_and_increment() byte amounts"
echo ""
echo "Run: cargo test --no-run --all-features 2>&1 | grep 'error\[E'"
echo "to see remaining compilation errors."
