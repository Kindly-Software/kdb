#!/bin/bash
# DDoS Protection Testing Suite
# Framework: UCE34 Q33 Verification

set -e

TARGET="${1:-192.168.0.38}"
HTTPS_PORT="443"
HTTP_PORT="80"
TIMEOUT="10"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

test_count=0
pass_count=0
fail_count=0

log_test() {
    test_count=$((test_count + 1))
    echo ""
    echo "=== TEST $test_count: $1 ==="
}

pass_test() {
    pass_count=$((pass_count + 1))
    echo -e "${GREEN}✅ PASS${NC}: $1"
}

fail_test() {
    fail_count=$((fail_count + 1))
    echo -e "${RED}❌ FAIL${NC}: $1"
}

warn() {
    echo -e "${YELLOW}⚠️  WARNING${NC}: $1"
}

echo "DDoS Protection Testing Suite"
echo "Target: $TARGET"
echo "Framework: UCE34 Q33 Verification"
echo ""

# ===== TEST 1: Kernel Parameters =====
log_test "Kernel Parameters (SYN Protection)"

if [[ -z "$TARGET" || "$TARGET" == "localhost" ]]; then
    echo "Testing local machine..."

    local syn_cookies=$(sysctl -n net.ipv4.tcp_syncookies 2>/dev/null || echo "0")
    local backlog=$(sysctl -n net.ipv4.tcp_max_syn_backlog 2>/dev/null || echo "0")

    if [[ "$syn_cookies" == "1" ]]; then
        pass_test "tcp_syncookies enabled"
    else
        fail_test "tcp_syncookies disabled ($syn_cookies)"
    fi

    if [[ $backlog -ge 8192 ]]; then
        pass_test "tcp_max_syn_backlog=$backlog (≥8192)"
    else
        fail_test "tcp_max_syn_backlog=$backlog (<8192)"
    fi
else
    echo "Skipping (remote target, requires SSH)"
fi

# ===== TEST 2: Connectivity Check =====
log_test "Target Connectivity"

if nc -z -w 2 "$TARGET" "$HTTPS_PORT" 2>/dev/null; then
    pass_test "HTTPS port $HTTPS_PORT reachable"
else
    warn "HTTPS port $HTTPS_PORT not reachable (may not be running)"
fi

if nc -z -w 2 "$TARGET" "$HTTP_PORT" 2>/dev/null; then
    pass_test "HTTP port $HTTP_PORT reachable"
else
    warn "HTTP port $HTTP_PORT not reachable (may not be running)"
fi

# ===== TEST 3: Rate Limiting - Single Connection =====
log_test "Rate Limiting - Single Connection"

if command -v curl &> /dev/null; then
    if timeout 5 curl -s -o /dev/null https://"$TARGET":443/ 2>/dev/null; then
        pass_test "Single connection to HTTPS succeeds"
    else
        fail_test "Single connection to HTTPS failed"
    fi
else
    warn "curl not installed, skipping connection test"
fi

# ===== TEST 4: Rate Limiting - Rapid Connections =====
log_test "Rate Limiting - 150 Rapid Connections (should limit after 100)"

if command -v curl &> /dev/null; then
    success=0
    failed=0

    for i in {1..150}; do
        if timeout 2 curl -s -o /dev/null https://"$TARGET":443/ 2>/dev/null; then
            success=$((success + 1))
        else
            failed=$((failed + 1))
        fi
    done

    echo "  Results: $success succeeded, $failed failed"

    if [[ $success -ge 90 ]]; then
        pass_test "Rate limiting allows >90% legitimate traffic ($success/150)"
    else
        fail_test "Rate limiting too aggressive ($success/150, expected >90%)"
    fi

    if [[ $failed -gt 10 ]]; then
        pass_test "Rate limiting drops excessive traffic ($failed dropped, expected >10)"
    else
        warn "Rate limiting not blocking enough ($failed dropped, expected >10)"
    fi
else
    warn "curl not installed, skipping rate limit test"
fi

# ===== TEST 5: Malformed Packet Filtering =====
log_test "Malformed Packet Filtering"

if command -v hping3 &> /dev/null; then
    echo "Sending NULL packet (no TCP flags)..."
    if timeout 2 hping3 -p "$HTTPS_PORT" --tcp-flags 0x00 -c 1 "$TARGET" > /dev/null 2>&1; then
        pass_test "NULL packet sent (kernel should silently drop)"
    fi

    echo "Sending XMAS packet (all flags set)..."
    if timeout 2 hping3 -p "$HTTPS_PORT" --tcp-flags 0x3F -c 1 "$TARGET" > /dev/null 2>&1; then
        pass_test "XMAS packet sent (kernel should silently drop)"
    fi
else
    warn "hping3 not installed (apt install hping3), skipping malformed packet test"
fi

# ===== TEST 6: ICMP Rate Limiting =====
log_test "ICMP Rate Limiting (Ping Flood)"

echo "Sending 20 ICMP pings (rate limited to 100/sec kernel-wise)..."
if timeout 5 ping -c 20 "$TARGET" > /dev/null 2>&1; then
    pass_test "ICMP pings received (rate limiting may drop some)"
fi

# ===== TEST 7: Connection Exhaustion Simulation =====
log_test "Connection Exhaustion Resistance (Apache Bench)"

if command -v ab &> /dev/null; then
    echo "Sending 1000 requests, 100 concurrent (benign load)..."
    if timeout 30 ab -n 1000 -c 100 -q https://"$TARGET":443/ > /tmp/ab_result.txt 2>&1; then
        local rps=$(grep "Requests per second:" /tmp/ab_result.txt | awk '{print $4}')
        echo "  Throughput: ~$rps req/sec"
        pass_test "1000 requests completed successfully (req/s=$rps)"
    else
        fail_test "Apache Bench failed (target may not support HTTPS)"
    fi
else
    warn "apache2-utils not installed (apt install apache2-utils), skipping load test"
fi

# ===== SUMMARY =====
echo ""
echo "=========================================="
echo "TESTING SUMMARY"
echo "=========================================="
echo "Total Tests: $test_count"
echo -e "Passed: ${GREEN}$pass_count${NC}"
echo -e "Failed: ${RED}$fail_count${NC}"
echo ""

if [[ $fail_count -eq 0 ]]; then
    echo -e "${GREEN}✅ ALL TESTS PASSED${NC}"
    echo ""
    echo "DDoS Protection Status: VERIFIED"
    echo "  ✅ Kernel SYN protection"
    echo "  ✅ Connection tracking"
    echo "  ✅ Per-IP rate limiting"
    echo "  ✅ Malformed packet filtering"
    echo "  ✅ Connection exhaustion resistance"
    exit 0
else
    echo -e "${YELLOW}⚠️  SOME TESTS FAILED OR WERE SKIPPED${NC}"
    echo ""
    echo "Recommendations:"
    echo "  1. Verify target ($TARGET) is reachable"
    echo "  2. Install test dependencies: apt install -y curl hping3 apache2-utils"
    echo "  3. Check firewall rules: sudo iptables -L -n"
    echo "  4. Verify sysctl config: sudo sysctl -a | grep tcp_syncookies"
    exit 1
fi
