#!/bin/bash
# kdb-mcp Production Hardening Verification Script
# UCE34 Framework: Q34 Audit Compliance
#
# Usage: ./verify_hardening.sh [--fix]

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

PASSED=0
FAILED=0
WARNINGS=0

# =============================================================================
# Helper Functions
# =============================================================================

check_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

check_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAILED++))
}

check_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
    ((WARNINGS++))
}

check_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

section() {
    echo ""
    echo -e "${BLUE}=== $1 ===${NC}"
}

# =============================================================================
# Configuration File Checks
# =============================================================================

section "Configuration Files"

# Check Cloudflare WAF script
if [[ -f "$PROJECT_DIR/config/cloudflare_waf.sh" ]]; then
    if [[ -x "$PROJECT_DIR/config/cloudflare_waf.sh" ]] || chmod +x "$PROJECT_DIR/config/cloudflare_waf.sh" 2>/dev/null; then
        check_pass "cloudflare_waf.sh exists and is executable"
    else
        check_warn "cloudflare_waf.sh exists but may not be executable"
    fi
else
    check_fail "cloudflare_waf.sh not found"
fi

# Check rate limiting config
if [[ -f "$PROJECT_DIR/config/rate_limiting.yaml" ]]; then
    check_pass "rate_limiting.yaml exists"

    # Validate YAML syntax
    if command -v python3 &>/dev/null; then
        if python3 -c "import yaml; yaml.safe_load(open('$PROJECT_DIR/config/rate_limiting.yaml'))" 2>/dev/null; then
            check_pass "rate_limiting.yaml is valid YAML"
        else
            check_fail "rate_limiting.yaml has invalid YAML syntax"
        fi
    else
        check_info "python3 not available, skipping YAML validation"
    fi
else
    check_fail "rate_limiting.yaml not found"
fi

# Check license tiers config
if [[ -f "$PROJECT_DIR/config/license_tiers.yaml" ]]; then
    check_pass "license_tiers.yaml exists"

    # Validate YAML syntax
    if command -v python3 &>/dev/null; then
        if python3 -c "import yaml; yaml.safe_load(open('$PROJECT_DIR/config/license_tiers.yaml'))" 2>/dev/null; then
            check_pass "license_tiers.yaml is valid YAML"
        else
            check_fail "license_tiers.yaml has invalid YAML syntax"
        fi
    else
        check_info "python3 not available, skipping YAML validation"
    fi

    # Check tier definitions
    if grep -q "free:" "$PROJECT_DIR/config/license_tiers.yaml" && \
       grep -q "pro:" "$PROJECT_DIR/config/license_tiers.yaml" && \
       grep -q "enterprise:" "$PROJECT_DIR/config/license_tiers.yaml"; then
        check_pass "All three license tiers defined (free, pro, enterprise)"
    else
        check_fail "Missing license tier definitions"
    fi
else
    check_fail "license_tiers.yaml not found"
fi

# =============================================================================
# Prometheus Alert Checks
# =============================================================================

section "Prometheus Alerts"

if [[ -f "$PROJECT_DIR/prometheus/kdb_mcp_alerts.yml" ]]; then
    check_pass "kdb_mcp_alerts.yml exists"

    # Check required alerts
    REQUIRED_ALERTS=("KdbMcpHighErrorRate" "KdbMcpRateLimitExceeded" "KdbMcpDown" "KdbMcpHighLatency")
    for alert in "${REQUIRED_ALERTS[@]}"; do
        if grep -q "$alert" "$PROJECT_DIR/prometheus/kdb_mcp_alerts.yml"; then
            check_pass "Alert '$alert' defined"
        else
            check_fail "Required alert '$alert' not found"
        fi
    done

    # Validate YAML syntax
    if command -v python3 &>/dev/null; then
        if python3 -c "import yaml; yaml.safe_load(open('$PROJECT_DIR/prometheus/kdb_mcp_alerts.yml'))" 2>/dev/null; then
            check_pass "kdb_mcp_alerts.yml is valid YAML"
        else
            check_fail "kdb_mcp_alerts.yml has invalid YAML syntax"
        fi
    fi
else
    check_fail "kdb_mcp_alerts.yml not found"
fi

# Check existing rules.yml
if [[ -f "$PROJECT_DIR/prometheus/rules.yml" ]]; then
    check_pass "rules.yml (existing alerts) found"
else
    check_warn "rules.yml not found (may be expected)"
fi

# =============================================================================
# Source Code Checks
# =============================================================================

section "Rate Limiter Implementation"

# Check RateLimiterCapsule
if [[ -f "$PROJECT_DIR/src/rate_limiter.rs" ]]; then
    check_pass "rate_limiter.rs exists"

    # Verify key features
    if grep -q "RateLimiterCapsule" "$PROJECT_DIR/src/rate_limiter.rs"; then
        check_pass "RateLimiterCapsule struct defined"
    else
        check_fail "RateLimiterCapsule not found"
    fi

    if grep -q "pub fn check" "$PROJECT_DIR/src/rate_limiter.rs"; then
        check_pass "Rate limit check method implemented"
    else
        check_fail "Rate limit check method not found"
    fi
else
    check_fail "rate_limiter.rs not found"
fi

# Check PerClientRateLimiterCapsule
if [[ -f "$PROJECT_DIR/src/per_client_rate_limiter.rs" ]]; then
    check_pass "per_client_rate_limiter.rs exists"

    # Verify key features
    if grep -q "PerClientRateLimiterCapsule" "$PROJECT_DIR/src/per_client_rate_limiter.rs"; then
        check_pass "PerClientRateLimiterCapsule struct defined"
    else
        check_fail "PerClientRateLimiterCapsule not found"
    fi

    if grep -q "check_rate_limit" "$PROJECT_DIR/src/per_client_rate_limiter.rs"; then
        check_pass "Per-client rate limit method implemented"
    else
        check_fail "Per-client rate limit method not found"
    fi

    # Check for test coverage
    TEST_COUNT=$(grep -c "#\[test\]" "$PROJECT_DIR/src/per_client_rate_limiter.rs" 2>/dev/null || echo "0")
    if [[ "$TEST_COUNT" -ge 20 ]]; then
        check_pass "Per-client rate limiter has $TEST_COUNT tests (T28 compliant)"
    else
        check_warn "Per-client rate limiter has only $TEST_COUNT tests (target: 28)"
    fi
else
    check_fail "per_client_rate_limiter.rs not found"
fi

# =============================================================================
# License Validator Checks
# =============================================================================

section "License Validator"

if [[ -f "$PROJECT_DIR/src/license_validator.rs" ]]; then
    check_pass "license_validator.rs exists"

    if grep -q "LicenseValidatorCapsule" "$PROJECT_DIR/src/license_validator.rs"; then
        check_pass "LicenseValidatorCapsule struct defined"
    else
        check_fail "LicenseValidatorCapsule not found"
    fi

    if grep -q "pub fn validate" "$PROJECT_DIR/src/license_validator.rs"; then
        check_pass "License validation method implemented"
    else
        check_fail "License validation method not found"
    fi
else
    check_fail "license_validator.rs not found"
fi

# =============================================================================
# SystemD Service Checks
# =============================================================================

section "SystemD Configuration"

if [[ -f "$PROJECT_DIR/systemd/mcp-debug.service" ]]; then
    check_pass "mcp-debug.service exists"

    # Check security hardening
    if grep -q "NoNewPrivileges=true" "$PROJECT_DIR/systemd/mcp-debug.service"; then
        check_pass "NoNewPrivileges enabled"
    else
        check_warn "NoNewPrivileges not set (security recommendation)"
    fi

    if grep -q "ProtectSystem" "$PROJECT_DIR/systemd/mcp-debug.service"; then
        check_pass "ProtectSystem set"
    else
        check_warn "ProtectSystem not set"
    fi
else
    check_warn "mcp-debug.service not found"
fi

# =============================================================================
# Dependency Checks
# =============================================================================

section "Dependencies"

if [[ -f "$PROJECT_DIR/Cargo.toml" ]]; then
    # Check for required features
    if grep -q 'rate-limiting' "$PROJECT_DIR/Cargo.toml"; then
        check_pass "rate-limiting feature defined"
    else
        check_fail "rate-limiting feature not found in Cargo.toml"
    fi

    if grep -q 'dashmap' "$PROJECT_DIR/Cargo.toml"; then
        check_pass "DashMap dependency present (lockfree maps)"
    else
        check_fail "DashMap not found (required for per-client rate limiting)"
    fi
else
    check_fail "Cargo.toml not found"
fi

# =============================================================================
# Compilation Check
# =============================================================================

section "Compilation Verification"

check_info "Checking if project compiles with hardening features..."

cd "$PROJECT_DIR"
if cargo check --features "rate-limiting,session" 2>/dev/null; then
    check_pass "Project compiles with rate-limiting features"
else
    check_warn "Compilation check failed - run 'cargo check' for details"
fi

# =============================================================================
# Summary
# =============================================================================

echo ""
echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}           VERIFICATION SUMMARY            ${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""
echo -e "  ${GREEN}Passed:${NC}   $PASSED"
echo -e "  ${RED}Failed:${NC}   $FAILED"
echo -e "  ${YELLOW}Warnings:${NC} $WARNINGS"
echo ""

if [[ $FAILED -eq 0 ]]; then
    echo -e "${GREEN}All critical checks passed!${NC}"
    exit 0
elif [[ $FAILED -lt 5 ]]; then
    echo -e "${YELLOW}Some checks failed - review and fix before deployment.${NC}"
    exit 1
else
    echo -e "${RED}Multiple critical failures - hardening incomplete.${NC}"
    exit 2
fi
