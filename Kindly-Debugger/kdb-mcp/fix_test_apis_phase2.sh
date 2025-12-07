#!/bin/bash
# Fix Remaining API Mismatches - Phase 2
# Handles handle_request() signature and type imports

set -e

echo "Fixing Phase 2 API mismatches..."

cd /home/samuel/Primitives/atomic_mcp_server

# ============================================================================
# Fix 7: handle_request(request, debugger) → handle_request(request, None, None, debugger)
# ============================================================================
echo "Fix 7: handle_request() signature (add api_key=None, client_ip=None)"

# Pattern: handle_request(&request, debugger) or handle_request(request, debugger)
# Replace with: handle_request(&request, None, None, debugger)
find tests -name "*.rs" -type f -exec sed -i \
    's/handle_request(\([^,]*\), *debugger)/handle_request(\1, None, None, debugger)/g' {} +

# ============================================================================
# Fix 8: Add missing imports to common.rs
# ============================================================================
echo "Fix 8: Add missing type exports to common.rs"

# Add ClientId and ClientTokenBucket to imports
# This is already in common.rs from our previous fix, but needs proper exports

# ============================================================================
# Fix 9: LicenseValidatorCapsule::new([0u8; 32]) → LicenseValidatorCapsule::new()
# ============================================================================
echo "Fix 9: LicenseValidatorCapsule::new() takes no arguments"
find . -name "*.rs" -type f -exec sed -i \
    's/LicenseValidatorCapsule::new(\[0u8; 32\])/LicenseValidatorCapsule::new()/g' {} +

# Also fix any other argument patterns
find . -name "*.rs" -type f -exec sed -i \
    's/LicenseValidatorCapsule::new(\([^)]\+\))/LicenseValidatorCapsule::new()/g' {} +

echo ""
echo "✅ Phase 2 fixes applied!"
echo ""
echo "Remaining manual fixes:"
echo "  1. Add type exports to lib.rs for ClientId, ClientTokenBucket"
echo "  2. Fix AuditLogCapsule.head access (use public API)"
echo "  3. Add missing type exports (AuthError, PolicyDecision, etc.)"
echo "  4. Update examples with correct API usage"
