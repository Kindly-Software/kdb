#!/bin/bash
# =============================================================================
# Kindly Services Security Testing Script
# =============================================================================
#
# Version: 1.0.0
# Framework: UCE34/COCA/T28/B32/ASSUM
#
# Usage:
#   ./scripts/security_test.sh              # Run all tests
#   ./scripts/security_test.sh --local      # Test local server only
#   ./scripts/security_test.sh --remote     # Test remote server only
#   ./scripts/security_test.sh --quick      # Quick smoke test
#   ./scripts/security_test.sh --full       # Full security audit
#
# Requirements:
#   - curl (for HTTP tests)
#   - ssh access to kindly-hub (for remote tests)
#   - jq (optional, for JSON parsing)
#
# =============================================================================

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test targets
LOCAL_URL="http://localhost:8082"
REMOTE_HOST="kindly-hub"
REMOTE_URL="https://kindly.software"

# Counters
TESTS_PASSED=0
TESTS_FAILED=0
TESTS_SKIPPED=0

# =============================================================================
# Helper Functions
# =============================================================================

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((TESTS_PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((TESTS_FAILED++))
}

log_skip() {
    echo -e "${YELLOW}[SKIP]${NC} $1"
    ((TESTS_SKIPPED++))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_section() {
    echo ""
    echo -e "${BLUE}=== $1 ===${NC}"
    echo ""
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check if URL is reachable
url_reachable() {
    curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$1" 2>/dev/null || echo "000"
}

# =============================================================================
# Test Functions: Security Headers
# =============================================================================

test_security_headers() {
    local url="$1"
    local name="$2"

    log_section "Security Headers ($name)"

    # Fetch headers
    local headers
    headers=$(curl -sI --max-time 10 "$url" 2>/dev/null) || {
        log_fail "Failed to fetch headers from $url"
        return 1
    }

    # Test HSTS
    if echo "$headers" | grep -qi "Strict-Transport-Security"; then
        local hsts_value
        hsts_value=$(echo "$headers" | grep -i "Strict-Transport-Security" | head -1)
        if echo "$hsts_value" | grep -q "max-age="; then
            log_pass "HSTS header present: $hsts_value"
        else
            log_fail "HSTS header missing max-age"
        fi
    else
        log_fail "HSTS header missing"
    fi

    # Test X-Frame-Options
    if echo "$headers" | grep -qi "X-Frame-Options"; then
        log_pass "X-Frame-Options present"
    else
        log_fail "X-Frame-Options missing"
    fi

    # Test X-Content-Type-Options
    if echo "$headers" | grep -qi "X-Content-Type-Options.*nosniff"; then
        log_pass "X-Content-Type-Options: nosniff present"
    else
        log_fail "X-Content-Type-Options missing or not nosniff"
    fi

    # Test X-XSS-Protection
    if echo "$headers" | grep -qi "X-XSS-Protection"; then
        log_pass "X-XSS-Protection present"
    else
        log_warn "X-XSS-Protection missing (deprecated but recommended)"
    fi

    # Test Referrer-Policy
    if echo "$headers" | grep -qi "Referrer-Policy"; then
        log_pass "Referrer-Policy present"
    else
        log_fail "Referrer-Policy missing"
    fi

    # Test COOP
    if echo "$headers" | grep -qi "Cross-Origin-Opener-Policy"; then
        log_pass "Cross-Origin-Opener-Policy present"
    else
        log_warn "Cross-Origin-Opener-Policy missing (recommended)"
    fi

    # Test CORP
    if echo "$headers" | grep -qi "Cross-Origin-Resource-Policy"; then
        log_pass "Cross-Origin-Resource-Policy present"
    else
        log_warn "Cross-Origin-Resource-Policy missing (recommended)"
    fi

    # Test Server header (should not reveal version)
    local server_header
    server_header=$(echo "$headers" | grep -i "^Server:" | head -1)
    if [ -n "$server_header" ]; then
        if echo "$server_header" | grep -qE "(nginx|apache|IIS)/[0-9]"; then
            log_warn "Server header reveals version: $server_header"
        else
            log_pass "Server header present (no version leaked)"
        fi
    else
        log_pass "Server header hidden"
    fi
}

# =============================================================================
# Test Functions: Rate Limiting
# =============================================================================

test_rate_limiting() {
    local url="$1"
    local name="$2"

    log_section "Rate Limiting ($name)"

    log_info "Sending 600 requests to trigger rate limiting..."

    local count_200=0
    local count_429=0
    local count_other=0

    for i in $(seq 1 600); do
        local status
        status=$(curl -s -o /dev/null -w "%{http_code}" --max-time 2 "$url" 2>/dev/null) || status="000"

        case $status in
            200) ((count_200++)) ;;
            429) ((count_429++)) ;;
            *) ((count_other++)) ;;
        esac

        # Progress indicator
        if ((i % 100 == 0)); then
            log_info "Progress: $i/600 (200: $count_200, 429: $count_429)"
        fi
    done

    echo ""
    log_info "Results: 200=$count_200, 429=$count_429, other=$count_other"

    if ((count_429 > 0)); then
        log_pass "Rate limiting triggered ($count_429 requests denied)"
    else
        log_fail "Rate limiting NOT triggered (0 requests denied)"
    fi

    # Test Retry-After header
    local retry_response
    retry_response=$(curl -sI --max-time 2 "$url" 2>/dev/null) || true
    if echo "$retry_response" | grep -qi "Retry-After"; then
        log_pass "Retry-After header present in rate limit response"
    else
        log_warn "Retry-After header not found (may need more requests)"
    fi
}

# =============================================================================
# Test Functions: Path Security
# =============================================================================

test_path_security() {
    local url="$1"
    local name="$2"

    log_section "Path Security ($name)"

    # Test path traversal attempts
    local traversal_paths=(
        "/../../etc/passwd"
        "/../../../etc/passwd"
        "/..%2f..%2fetc/passwd"
        "//etc/passwd"
        "/./../../etc/passwd"
        "/assets/../../etc/passwd"
    )

    for path in "${traversal_paths[@]}"; do
        local status
        local body
        status=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "${url}${path}" 2>/dev/null) || status="000"
        body=$(curl -s --max-time 5 "${url}${path}" 2>/dev/null) || body=""

        # Should not return 200 with /etc/passwd content
        if [[ "$status" == "200" ]] && echo "$body" | grep -q "root:"; then
            log_fail "Path traversal VULNERABLE: $path (exposed /etc/passwd)"
        elif [[ "$status" == "403" ]] || [[ "$status" == "400" ]]; then
            log_pass "Path traversal blocked: $path (status $status)"
        elif [[ "$status" == "200" ]]; then
            log_pass "Path traversal handled (SPA fallback): $path"
        else
            log_warn "Path traversal unknown response: $path (status $status)"
        fi
    done

    # Test XSS in path
    local xss_path="/%3Cscript%3Ealert(1)%3C/script%3E"
    local xss_body
    xss_body=$(curl -s --max-time 5 "${url}${xss_path}" 2>/dev/null) || xss_body=""

    if echo "$xss_body" | grep -q "<script>alert"; then
        log_fail "XSS reflected in response body"
    else
        log_pass "XSS not reflected in response"
    fi
}

# =============================================================================
# Test Functions: Infrastructure (Remote Only)
# =============================================================================

test_infrastructure() {
    local host="$1"

    log_section "Infrastructure Security ($host)"

    # Check if SSH is available
    if ! ssh -o BatchMode=yes -o ConnectTimeout=5 "samuel@$host" "echo test" >/dev/null 2>&1; then
        log_skip "Cannot SSH to $host (skipping infrastructure tests)"
        return
    fi

    # Test UFW
    log_info "Checking UFW firewall..."
    local ufw_status
    ufw_status=$(ssh "samuel@$host" "sudo ufw status 2>/dev/null" 2>/dev/null) || ufw_status=""

    if echo "$ufw_status" | grep -q "Status: active"; then
        log_pass "UFW firewall active"

        # Check for expected ports
        if echo "$ufw_status" | grep -q "22/tcp.*ALLOW"; then
            log_pass "SSH port (22) allowed"
        else
            log_warn "SSH port (22) not explicitly allowed"
        fi

        if echo "$ufw_status" | grep -q "443/tcp.*ALLOW"; then
            log_pass "HTTPS port (443) allowed"
        else
            log_warn "HTTPS port (443) not explicitly allowed"
        fi
    else
        log_fail "UFW firewall NOT active"
    fi

    # Test fail2ban
    log_info "Checking fail2ban..."
    local fail2ban_status
    fail2ban_status=$(ssh "samuel@$host" "sudo systemctl is-active fail2ban 2>/dev/null" 2>/dev/null) || fail2ban_status=""

    if [[ "$fail2ban_status" == "active" ]]; then
        log_pass "fail2ban active"

        # Check SSH jail
        local sshd_jail
        sshd_jail=$(ssh "samuel@$host" "sudo fail2ban-client status sshd 2>/dev/null" 2>/dev/null) || sshd_jail=""
        if [ -n "$sshd_jail" ]; then
            log_pass "fail2ban SSH jail configured"
        else
            log_warn "fail2ban SSH jail not found"
        fi
    else
        log_fail "fail2ban NOT active"
    fi

    # Test SSH hardening
    log_info "Checking SSH hardening..."
    local sshd_config
    sshd_config=$(ssh "samuel@$host" "sudo sshd -T 2>/dev/null" 2>/dev/null) || sshd_config=""

    if echo "$sshd_config" | grep -q "permitrootlogin no"; then
        log_pass "SSH root login disabled"
    else
        log_fail "SSH root login NOT disabled"
    fi

    if echo "$sshd_config" | grep -q "passwordauthentication no"; then
        log_pass "SSH password authentication disabled"
    else
        log_warn "SSH password authentication enabled"
    fi
}

# =============================================================================
# Test Functions: TLS/SSL
# =============================================================================

test_tls() {
    local url="$1"
    local name="$2"

    log_section "TLS Security ($name)"

    # Only test HTTPS URLs
    if [[ ! "$url" =~ ^https:// ]]; then
        log_skip "TLS tests require HTTPS URL"
        return
    fi

    # Extract hostname
    local hostname
    hostname=$(echo "$url" | sed -E 's|https://([^/]+).*|\1|')

    # Test TLS connection
    if command_exists openssl; then
        local tls_info
        tls_info=$(echo | openssl s_client -connect "$hostname:443" -servername "$hostname" 2>/dev/null | head -20) || tls_info=""

        if echo "$tls_info" | grep -q "TLSv1.3"; then
            log_pass "TLS 1.3 supported"
        elif echo "$tls_info" | grep -q "TLSv1.2"; then
            log_pass "TLS 1.2 supported"
        else
            log_warn "Unknown TLS version"
        fi

        # Check certificate validity
        local cert_info
        cert_info=$(echo | openssl s_client -connect "$hostname:443" -servername "$hostname" 2>/dev/null | openssl x509 -noout -dates 2>/dev/null) || cert_info=""

        if [ -n "$cert_info" ]; then
            log_pass "TLS certificate valid"
        else
            log_fail "TLS certificate issue"
        fi
    else
        log_skip "openssl not installed"
    fi
}

# =============================================================================
# Test Functions: Quick Smoke Test
# =============================================================================

quick_smoke_test() {
    local url="$1"

    log_section "Quick Smoke Test"

    # Test basic connectivity
    local status
    status=$(url_reachable "$url")

    if [[ "$status" == "200" ]]; then
        log_pass "Server reachable (status 200)"
    elif [[ "$status" == "000" ]]; then
        log_fail "Server unreachable"
        return 1
    else
        log_warn "Server returned status $status"
    fi

    # Test one security header
    local headers
    headers=$(curl -sI --max-time 5 "$url" 2>/dev/null) || headers=""

    if echo "$headers" | grep -qi "Strict-Transport-Security\|X-Frame-Options"; then
        log_pass "Security headers present"
    else
        log_warn "Security headers may be missing"
    fi

    # Test path traversal
    local traversal_status
    traversal_status=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "${url}/../../etc/passwd" 2>/dev/null) || traversal_status="000"

    if [[ "$traversal_status" == "403" ]] || [[ "$traversal_status" == "400" ]]; then
        log_pass "Path traversal protection active"
    elif [[ "$traversal_status" == "200" ]]; then
        local body
        body=$(curl -s --max-time 5 "${url}/../../etc/passwd" 2>/dev/null) || body=""
        if echo "$body" | grep -q "root:"; then
            log_fail "Path traversal VULNERABLE"
        else
            log_pass "Path traversal handled (SPA fallback)"
        fi
    else
        log_warn "Path traversal status: $traversal_status"
    fi
}

# =============================================================================
# Test Summary
# =============================================================================

print_summary() {
    log_section "Test Summary"

    echo ""
    echo -e "  ${GREEN}Passed:${NC}  $TESTS_PASSED"
    echo -e "  ${RED}Failed:${NC}  $TESTS_FAILED"
    echo -e "  ${YELLOW}Skipped:${NC} $TESTS_SKIPPED"
    echo ""

    local total=$((TESTS_PASSED + TESTS_FAILED))
    if ((total > 0)); then
        local pass_rate=$((TESTS_PASSED * 100 / total))
        echo "  Pass Rate: ${pass_rate}%"
    fi

    echo ""

    if ((TESTS_FAILED > 0)); then
        echo -e "${RED}SECURITY ISSUES DETECTED${NC}"
        return 1
    else
        echo -e "${GREEN}ALL TESTS PASSED${NC}"
        return 0
    fi
}

# =============================================================================
# Main
# =============================================================================

main() {
    local mode="${1:-all}"

    echo ""
    echo "=============================================="
    echo "  Kindly Services Security Testing"
    echo "  $(date)"
    echo "=============================================="
    echo ""

    case "$mode" in
        --local)
            log_info "Testing local server only..."

            if [[ "$(url_reachable "$LOCAL_URL")" != "000" ]]; then
                test_security_headers "$LOCAL_URL" "Local"
                test_path_security "$LOCAL_URL" "Local"
                # Skip rate limiting (takes too long for local test)
            else
                log_fail "Local server not reachable at $LOCAL_URL"
            fi
            ;;

        --remote)
            log_info "Testing remote server only..."

            if [[ "$(url_reachable "$REMOTE_URL")" != "000" ]]; then
                test_security_headers "$REMOTE_URL" "Remote"
                test_path_security "$REMOTE_URL" "Remote"
                test_tls "$REMOTE_URL" "Remote"
                test_infrastructure "$REMOTE_HOST"
            else
                log_fail "Remote server not reachable at $REMOTE_URL"
            fi
            ;;

        --quick)
            log_info "Running quick smoke test..."

            if [[ "$(url_reachable "$LOCAL_URL")" != "000" ]]; then
                quick_smoke_test "$LOCAL_URL"
            elif [[ "$(url_reachable "$REMOTE_URL")" != "000" ]]; then
                quick_smoke_test "$REMOTE_URL"
            else
                log_fail "No server reachable"
            fi
            ;;

        --full)
            log_info "Running full security audit..."

            # Local tests
            if [[ "$(url_reachable "$LOCAL_URL")" != "000" ]]; then
                test_security_headers "$LOCAL_URL" "Local"
                test_path_security "$LOCAL_URL" "Local"
                test_rate_limiting "$LOCAL_URL" "Local"
            else
                log_warn "Local server not reachable, skipping local tests"
            fi

            # Remote tests
            if [[ "$(url_reachable "$REMOTE_URL")" != "000" ]]; then
                test_security_headers "$REMOTE_URL" "Remote"
                test_path_security "$REMOTE_URL" "Remote"
                test_tls "$REMOTE_URL" "Remote"
                test_infrastructure "$REMOTE_HOST"
            else
                log_warn "Remote server not reachable, skipping remote tests"
            fi
            ;;

        --help|-h)
            echo "Usage: $0 [--local|--remote|--quick|--full]"
            echo ""
            echo "Options:"
            echo "  --local   Test local server only (localhost:8082)"
            echo "  --remote  Test remote server only (kindly.software)"
            echo "  --quick   Quick smoke test (fastest)"
            echo "  --full    Full security audit (slowest, includes rate limiting)"
            echo "  --help    Show this help message"
            echo ""
            echo "Default: Test local if available, otherwise remote"
            exit 0
            ;;

        *)
            log_info "Running default tests..."

            # Try local first
            if [[ "$(url_reachable "$LOCAL_URL")" != "000" ]]; then
                test_security_headers "$LOCAL_URL" "Local"
                test_path_security "$LOCAL_URL" "Local"
            else
                log_info "Local server not running, trying remote..."
            fi

            # Then remote
            if [[ "$(url_reachable "$REMOTE_URL")" != "000" ]]; then
                test_security_headers "$REMOTE_URL" "Remote"
                test_path_security "$REMOTE_URL" "Remote"
                test_tls "$REMOTE_URL" "Remote"
            else
                log_warn "Remote server not reachable"
            fi

            # Infrastructure always needs SSH
            test_infrastructure "$REMOTE_HOST"
            ;;
    esac

    print_summary
}

# Run main
main "$@"
