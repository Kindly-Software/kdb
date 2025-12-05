#!/bin/bash
# kdb-mcp Security Penetration Tests
# Version: 1.0.0
# Date: 2025-12-04
#
# Usage: ./scripts/security_penetration_tests.sh [test_name]
# Examples:
#   ./scripts/security_penetration_tests.sh all
#   ./scripts/security_penetration_tests.sh auth_bypass
#   ./scripts/security_penetration_tests.sh pid_allowlist

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$PROJECT_DIR/security_test_results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$RESULTS_DIR/penetration_test_report_$TIMESTAMP.md"

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Create results directory
mkdir -p "$RESULTS_DIR"

# Initialize report
init_report() {
    cat > "$REPORT_FILE" << EOF
# kdb-mcp Security Penetration Test Report

**Date**: $(date '+%Y-%m-%d %H:%M:%S')
**Tester**: Automated Script
**Version**: 1.0.0

---

## Executive Summary

| Metric | Value |
|--------|-------|
| Total Tests | $TOTAL_TESTS |
| Passed | $PASSED_TESTS |
| Failed | $FAILED_TESTS |
| Pass Rate | TBD |

---

## Test Results

EOF
}

# Record test result
record_result() {
    local test_id="$1"
    local test_name="$2"
    local result="$3"  # PASS or FAIL
    local details="$4"

    TOTAL_TESTS=$((TOTAL_TESTS + 1))

    if [[ "$result" == "PASS" ]]; then
        PASSED_TESTS=$((PASSED_TESTS + 1))
        echo -e "${GREEN}[PASS]${NC} $test_id: $test_name"
        echo "### $test_id: $test_name - PASS" >> "$REPORT_FILE"
    else
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo -e "${RED}[FAIL]${NC} $test_id: $test_name"
        echo "### $test_id: $test_name - **FAIL**" >> "$REPORT_FILE"
    fi

    echo "" >> "$REPORT_FILE"
    echo "**Details**: $details" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"
}

# ============================================================================
# TEST CATEGORY 1: Authentication Bypass Tests
# ============================================================================

test_auth_bypass() {
    echo ""
    echo -e "${YELLOW}=== Authentication Bypass Tests ===${NC}"
    echo ""

    # SEC-001: JWT signature forgery (via unit tests)
    echo "SEC-001: Testing JWT signature validation..."
    if cargo test --lib --features "std,json-rpc" test_validate_key 2>/dev/null | grep -q "ok"; then
        record_result "SEC-001" "JWT signature forgery prevention" "PASS" "License validator correctly rejects invalid signatures"
    else
        record_result "SEC-001" "JWT signature forgery prevention" "FAIL" "License validation test failed"
    fi

    # SEC-002: Expired license rejection
    echo "SEC-002: Testing expired license rejection..."
    if cargo test --lib --features "std,json-rpc" test_expired_license 2>/dev/null | grep -q "ok"; then
        record_result "SEC-002" "Expired license rejection" "PASS" "Expired licenses correctly rejected"
    else
        record_result "SEC-002" "Expired license rejection" "FAIL" "Expired license test failed"
    fi

    # SEC-003: Rate limiter enforcement
    echo "SEC-003: Testing rate limiter enforcement..."
    if cargo test --lib --features "std,json-rpc" rate_limiter 2>/dev/null | grep -q "test result: ok"; then
        record_result "SEC-003" "Rate limiter enforcement" "PASS" "Rate limiting correctly enforced"
    else
        record_result "SEC-003" "Rate limiter enforcement" "FAIL" "Rate limiter tests failed"
    fi

    # SEC-004: Quota tracking
    echo "SEC-004: Testing quota tracking..."
    if cargo test --lib --features "std,json-rpc" quota_tracker 2>/dev/null | grep -q "test result: ok"; then
        record_result "SEC-004" "Quota tracking" "PASS" "Quota limits correctly enforced"
    else
        record_result "SEC-004" "Quota tracking" "FAIL" "Quota tracking tests failed"
    fi
}

# ============================================================================
# TEST CATEGORY 2: PID Allowlist Tests
# ============================================================================

test_pid_allowlist() {
    echo ""
    echo -e "${YELLOW}=== PID Allowlist Enforcement Tests ===${NC}"
    echo ""

    # PID-001: Validate PID 1 rejection
    echo "PID-001: Testing PID 1 (init) rejection..."
    if cargo test --lib --features "std,json-rpc" test_validate_init_pid 2>/dev/null | grep -q "ok"; then
        record_result "PID-001" "PID 1 (init) rejection" "PASS" "Init process correctly blocked from debugging"
    else
        record_result "PID-001" "PID 1 (init) rejection" "FAIL" "PID 1 validation test failed"
    fi

    # PID-002: Negative PID validation
    echo "PID-002: Testing negative PID rejection..."
    if cargo test --lib --features "std,json-rpc" test_validate_negative_pid 2>/dev/null | grep -q "ok"; then
        record_result "PID-002" "Negative PID rejection" "PASS" "Negative PIDs correctly rejected"
    else
        record_result "PID-002" "Negative PID rejection" "FAIL" "Negative PID validation test failed"
    fi

    # PID-003: Zero PID validation
    echo "PID-003: Testing PID 0 rejection..."
    if cargo test --lib --features "std,json-rpc" test_validate_zero_pid 2>/dev/null | grep -q "ok"; then
        record_result "PID-003" "Zero PID rejection" "PASS" "PID 0 correctly rejected"
    else
        record_result "PID-003" "Zero PID rejection" "FAIL" "Zero PID validation test failed"
    fi

    # PID-004: Non-existent PID validation
    echo "PID-004: Testing non-existent PID handling..."
    if cargo test --lib --features "std,json-rpc" test_validate_nonexistent_pid 2>/dev/null | grep -q "ok"; then
        record_result "PID-004" "Non-existent PID handling" "PASS" "Non-existent PIDs correctly handled"
    else
        record_result "PID-004" "Non-existent PID handling" "FAIL" "Non-existent PID test failed"
    fi

    # PID-005: Self PID validation (should work)
    echo "PID-005: Testing self PID validation..."
    if cargo test --lib --features "std,json-rpc" test_validate_self_pid 2>/dev/null | grep -q "ok"; then
        record_result "PID-005" "Self PID validation" "PASS" "Self PID correctly validated"
    else
        record_result "PID-005" "Self PID validation" "FAIL" "Self PID validation test failed"
    fi
}

# ============================================================================
# TEST CATEGORY 3: Rate Limiting Tests
# ============================================================================

test_rate_limiting() {
    echo ""
    echo -e "${YELLOW}=== Rate Limiting Tests ===${NC}"
    echo ""

    # RATE-001: Rate limit allow
    echo "RATE-001: Testing rate limit allow..."
    if cargo test --lib --features "std,json-rpc" test_rate_limit_allow 2>/dev/null | grep -q "ok"; then
        record_result "RATE-001" "Rate limit allow" "PASS" "Normal requests correctly allowed"
    else
        record_result "RATE-001" "Rate limit allow" "FAIL" "Rate limit allow test failed"
    fi

    # RATE-002: Rate limit deny
    echo "RATE-002: Testing rate limit deny..."
    if cargo test --lib --features "std,json-rpc" test_rate_limit_deny 2>/dev/null | grep -q "ok"; then
        record_result "RATE-002" "Rate limit deny" "PASS" "Excess requests correctly denied"
    else
        record_result "RATE-002" "Rate limit deny" "FAIL" "Rate limit deny test failed"
    fi

    # RATE-003: Rate limiter alignment (memory safety)
    echo "RATE-003: Testing rate limiter memory alignment..."
    if cargo test --lib --features "std,json-rpc" test_rate_limiter_alignment 2>/dev/null | grep -q "ok"; then
        record_result "RATE-003" "Rate limiter alignment" "PASS" "Cache-aligned for optimal performance"
    else
        record_result "RATE-003" "Rate limiter alignment" "FAIL" "Rate limiter alignment test failed"
    fi

    # RATE-004: Rate limiter size (memory efficiency)
    echo "RATE-004: Testing rate limiter size..."
    if cargo test --lib --features "std,json-rpc" test_rate_limiter_size 2>/dev/null | grep -q "ok"; then
        record_result "RATE-004" "Rate limiter size" "PASS" "Memory-efficient capsule size"
    else
        record_result "RATE-004" "Rate limiter size" "FAIL" "Rate limiter size test failed"
    fi
}

# ============================================================================
# TEST CATEGORY 4: Audit Trail Tests
# ============================================================================

test_audit_trail() {
    echo ""
    echo -e "${YELLOW}=== Audit Trail Integrity Tests ===${NC}"
    echo ""

    # AUDIT-001: JSON-RPC request parsing
    echo "AUDIT-001: Testing JSON-RPC request parsing..."
    if cargo test --lib --features "std,json-rpc" test_parse_request 2>/dev/null | grep -q "ok"; then
        record_result "AUDIT-001" "JSON-RPC request parsing" "PASS" "Requests correctly parsed for audit"
    else
        record_result "AUDIT-001" "JSON-RPC request parsing" "FAIL" "JSON-RPC parsing test failed"
    fi

    # AUDIT-002: Response formatting
    echo "AUDIT-002: Testing response formatting..."
    if cargo test --lib --features "std,json-rpc" test_format_response 2>/dev/null | grep -q "ok"; then
        record_result "AUDIT-002" "Response formatting" "PASS" "Responses correctly formatted for audit"
    else
        record_result "AUDIT-002" "Response formatting" "FAIL" "Response formatting test failed"
    fi

    # AUDIT-003: Session ID generation
    echo "AUDIT-003: Testing session ID generation..."
    if cargo test --lib --features "std,json-rpc" test_session_id 2>/dev/null | grep -q "ok"; then
        record_result "AUDIT-003" "Session ID generation" "PASS" "Session IDs correctly generated for audit trail"
    else
        record_result "AUDIT-003" "Session ID generation" "FAIL" "Session ID test failed"
    fi
}

# ============================================================================
# TEST CATEGORY 5: Time-Travel Consistency Tests
# ============================================================================

test_time_travel() {
    echo ""
    echo -e "${YELLOW}=== Time-Travel Debugging Consistency Tests ===${NC}"
    echo ""

    # TTD-001: Monotonic request IDs
    echo "TTD-001: Testing monotonic request IDs..."
    if cargo test --lib --features "std,json-rpc" test_monotonic_request_ids 2>/dev/null | grep -q "ok"; then
        record_result "TTD-001" "Monotonic request IDs" "PASS" "Request IDs are strictly monotonic"
    else
        record_result "TTD-001" "Monotonic request IDs" "FAIL" "Monotonic request ID test failed"
    fi

    # TTD-002: Deterministic context alignment
    echo "TTD-002: Testing deterministic context alignment..."
    if cargo test --lib --features "std,json-rpc" test_deterministic_context_alignment 2>/dev/null | grep -q "ok"; then
        record_result "TTD-002" "Deterministic context alignment" "PASS" "Context correctly cache-aligned"
    else
        record_result "TTD-002" "Deterministic context alignment" "FAIL" "Deterministic context test failed"
    fi

    # TTD-003: Time advancement
    echo "TTD-003: Testing time advancement..."
    if cargo test --lib --features "std,json-rpc" test_time_advancement 2>/dev/null | grep -q "ok"; then
        record_result "TTD-003" "Time advancement" "PASS" "Time correctly advances for snapshots"
    else
        record_result "TTD-003" "Time advancement" "FAIL" "Time advancement test failed"
    fi

    # TTD-004: Reset functionality
    echo "TTD-004: Testing reset functionality..."
    if cargo test --lib --features "std,json-rpc" test_reset 2>/dev/null | grep -q "ok"; then
        record_result "TTD-004" "Reset functionality" "PASS" "State correctly resets for new sessions"
    else
        record_result "TTD-004" "Reset functionality" "FAIL" "Reset functionality test failed"
    fi
}

# ============================================================================
# TEST CATEGORY 6: Capsule Memory Safety Tests
# ============================================================================

test_capsule_safety() {
    echo ""
    echo -e "${YELLOW}=== Capsule Memory Safety Tests ===${NC}"
    echo ""

    # CAPSULE-001: Server alignment
    echo "CAPSULE-001: Testing server alignment..."
    if cargo test --lib --features "std,json-rpc" test_server_alignment 2>/dev/null | grep -q "ok"; then
        record_result "CAPSULE-001" "Server alignment" "PASS" "Server capsule correctly aligned (64B)"
    else
        record_result "CAPSULE-001" "Server alignment" "FAIL" "Server alignment test failed"
    fi

    # CAPSULE-002: Server size
    echo "CAPSULE-002: Testing server size..."
    if cargo test --lib --features "std,json-rpc" test_server_size 2>/dev/null | grep -q "ok"; then
        record_result "CAPSULE-002" "Server size" "PASS" "Server capsule size within limits"
    else
        record_result "CAPSULE-002" "Server size" "FAIL" "Server size test failed"
    fi

    # CAPSULE-003: JSON-RPC capsule alignment
    echo "CAPSULE-003: Testing JSON-RPC capsule alignment..."
    if cargo test --lib --features "std,json-rpc" test_json_rpc_capsule_alignment 2>/dev/null | grep -q "ok"; then
        record_result "CAPSULE-003" "JSON-RPC capsule alignment" "PASS" "JSON-RPC capsule correctly aligned"
    else
        record_result "CAPSULE-003" "JSON-RPC capsule alignment" "FAIL" "JSON-RPC alignment test failed"
    fi

    # CAPSULE-004: Tool registry alignment
    echo "CAPSULE-004: Testing tool registry alignment..."
    if cargo test --lib --features "std,json-rpc" test_registry_alignment 2>/dev/null | grep -q "ok"; then
        record_result "CAPSULE-004" "Tool registry alignment" "PASS" "Tool registry correctly aligned"
    else
        record_result "CAPSULE-004" "Tool registry alignment" "FAIL" "Tool registry alignment test failed"
    fi
}

# ============================================================================
# Generate Final Report
# ============================================================================

finalize_report() {
    local pass_rate=0
    if [[ $TOTAL_TESTS -gt 0 ]]; then
        pass_rate=$((PASSED_TESTS * 100 / TOTAL_TESTS))
    fi

    # Update summary in report
    sed -i "s/Total Tests | .*/Total Tests | $TOTAL_TESTS |/" "$REPORT_FILE"
    sed -i "s/Passed | .*/Passed | $PASSED_TESTS |/" "$REPORT_FILE"
    sed -i "s/Failed | .*/Failed | $FAILED_TESTS |/" "$REPORT_FILE"
    sed -i "s/Pass Rate | TBD/Pass Rate | $pass_rate% |/" "$REPORT_FILE"

    # Add conclusion
    cat >> "$REPORT_FILE" << EOF

---

## Conclusion

**Overall Result**: $(if [[ $FAILED_TESTS -eq 0 ]]; then echo "PASS"; else echo "FAIL - $FAILED_TESTS tests failed"; fi)

### Go/No-Go Recommendation

$(if [[ $FAILED_TESTS -eq 0 ]]; then
    echo "**Recommendation: GO** - All security tests passed."
else
    echo "**Recommendation: NO-GO** - $FAILED_TESTS security tests failed. Address issues before deployment."
fi)

### Next Steps

$(if [[ $FAILED_TESTS -eq 0 ]]; then
    echo "1. Proceed to load testing"
    echo "2. Schedule external security audit"
    echo "3. Prepare for public beta"
else
    echo "1. Review failed tests in detail"
    echo "2. Implement fixes for security issues"
    echo "3. Re-run penetration tests"
fi)

---

**Report Generated**: $(date '+%Y-%m-%d %H:%M:%S')
**Script Version**: 1.0.0
EOF

    echo ""
    echo "=============================================="
    echo "SECURITY PENETRATION TEST SUMMARY"
    echo "=============================================="
    echo ""
    echo "Total Tests:  $TOTAL_TESTS"
    echo -e "Passed:       ${GREEN}$PASSED_TESTS${NC}"
    echo -e "Failed:       ${RED}$FAILED_TESTS${NC}"
    echo "Pass Rate:    $pass_rate%"
    echo ""
    echo "Full report saved to: $REPORT_FILE"
    echo ""

    if [[ $FAILED_TESTS -eq 0 ]]; then
        echo -e "${GREEN}=== ALL SECURITY TESTS PASSED ===${NC}"
        exit 0
    else
        echo -e "${RED}=== SECURITY TESTS FAILED ===${NC}"
        exit 1
    fi
}

# ============================================================================
# Main Entry Point
# ============================================================================

main() {
    local test_category="${1:-all}"

    echo "=============================================="
    echo "kdb-mcp Security Penetration Tests"
    echo "=============================================="
    echo ""
    echo "Date: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "Project: $PROJECT_DIR"
    echo "Test Category: $test_category"
    echo ""

    init_report

    case "$test_category" in
        auth_bypass)
            test_auth_bypass
            ;;
        pid_allowlist)
            test_pid_allowlist
            ;;
        rate_limiting)
            test_rate_limiting
            ;;
        audit_trail)
            test_audit_trail
            ;;
        time_travel)
            test_time_travel
            ;;
        capsule_safety)
            test_capsule_safety
            ;;
        all)
            test_auth_bypass
            test_pid_allowlist
            test_rate_limiting
            test_audit_trail
            test_time_travel
            test_capsule_safety
            ;;
        *)
            echo "Unknown test category: $test_category"
            echo "Available: auth_bypass, pid_allowlist, rate_limiting, audit_trail, time_travel, capsule_safety, all"
            exit 1
            ;;
    esac

    finalize_report
}

main "$@"
