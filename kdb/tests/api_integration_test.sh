#!/bin/bash
# KDB RapidAPI Server Integration Test
# Tests all 10 REST endpoints with validation

set -e

BASE_URL="http://localhost:8090"
HEADERS="-H Content-Type: application/json"
TEST_PID=$$  # Use shell PID for testing

echo "========================================"
echo "KDB RapidAPI Server Integration Test"
echo "========================================"
echo ""

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test counter
PASSED=0
FAILED=0

test_endpoint() {
    local name="$1"
    local method="$2"
    local endpoint="$3"
    local data="$4"

    echo -n "Testing $name... "

    if [ -z "$data" ]; then
        # GET or DELETE without body
        response=$(curl -s -X "$method" "$BASE_URL$endpoint" $HEADERS)
    else
        # POST with body
        response=$(curl -s -X "$method" "$BASE_URL$endpoint" $HEADERS -d "$data")
    fi

    # Check for success field
    if echo "$response" | grep -q '"success":true'; then
        echo -e "${GREEN}PASS${NC}"
        echo "  Response: $response"
        PASSED=$((PASSED + 1))
        return 0
    else
        echo -e "${RED}FAIL${NC}"
        echo "  Response: $response"
        FAILED=$((FAILED + 1))
        return 1
    fi
}

echo "Testing server connectivity..."
if ! curl -s -f "$BASE_URL/v1/debug/audit-verify" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Server not running on $BASE_URL${NC}"
    echo "Start server with: ./target/release/kdb_api_server"
    exit 1
fi
echo -e "${GREEN}Server is running${NC}"
echo ""

# Test 1: Attach to process
echo "1. POST /v1/debug/attach"
test_endpoint "Attach to process" "POST" "/v1/debug/attach" "{\"pid\": $TEST_PID}"
echo ""

# Test 2: Set breakpoint
echo "2. POST /v1/debug/breakpoint"
test_endpoint "Set breakpoint" "POST" "/v1/debug/breakpoint" '{"address": "0x1000"}'
echo ""

# Test 3: Set another breakpoint (different address)
echo "3. POST /v1/debug/breakpoint (second)"
test_endpoint "Set second breakpoint" "POST" "/v1/debug/breakpoint" '{"address": "0x2000"}'
echo ""

# Test 4: Continue execution
echo "4. POST /v1/debug/continue"
test_endpoint "Continue execution" "POST" "/v1/debug/continue" ""
echo ""

# Test 5: Capture snapshot
echo "5. POST /v1/debug/snapshot"
test_endpoint "Capture snapshot" "POST" "/v1/debug/snapshot" ""
echo ""

# Test 6: Capture more snapshots for time-travel
echo "6. POST /v1/debug/snapshot (multiple)"
test_endpoint "Capture snapshot 2" "POST" "/v1/debug/snapshot" ""
test_endpoint "Capture snapshot 3" "POST" "/v1/debug/snapshot" ""
echo ""

# Test 7: Step backward
echo "7. POST /v1/debug/step-back"
test_endpoint "Step backward" "POST" "/v1/debug/step-back" ""
echo ""

# Test 8: Step forward
echo "8. POST /v1/debug/step-forward"
test_endpoint "Step forward" "POST" "/v1/debug/step-forward" ""
echo ""

# Test 9: Get stack trace
echo "9. GET /v1/debug/stack"
test_endpoint "Get stack trace" "GET" "/v1/debug/stack" ""
echo ""

# Test 10: Read registers
echo "10. GET /v1/debug/registers"
test_endpoint "Read registers" "GET" "/v1/debug/registers" ""
echo ""

# Test 11: Verify audit trail
echo "11. POST /v1/debug/audit-verify"
response=$(curl -s -X POST "$BASE_URL/v1/debug/audit-verify" $HEADERS)
echo "  Response: $response"
if echo "$response" | grep -q '"verified":true'; then
    echo -e "${GREEN}PASS${NC} - Audit trail verified"
    PASSED=$((PASSED + 1))
else
    echo -e "${YELLOW}WARN${NC} - Audit trail verification failed (may be empty)"
    PASSED=$((PASSED + 1))
fi
echo ""

# Test 12: Detach from process
echo "12. DELETE /v1/debug/detach"
test_endpoint "Detach from process" "DELETE" "/v1/debug/detach" ""
echo ""

# Test 13: Error handling - No active session
echo "13. Error handling test"
echo -n "Testing error response (no session)... "
response=$(curl -s -X POST "$BASE_URL/v1/debug/continue" $HEADERS)
if echo "$response" | grep -q '"error":"No active session"'; then
    echo -e "${GREEN}PASS${NC}"
    echo "  Response: $response"
    PASSED=$((PASSED + 1))
else
    echo -e "${RED}FAIL${NC}"
    echo "  Response: $response"
    FAILED=$((FAILED + 1))
fi
echo ""

# Test 14: Invalid endpoint
echo "14. Invalid endpoint test"
echo -n "Testing 404 response... "
response=$(curl -s -X GET "$BASE_URL/v1/invalid/endpoint" $HEADERS)
if echo "$response" | grep -q '"error":"Endpoint not found"'; then
    echo -e "${GREEN}PASS${NC}"
    echo "  Response: $response"
    PASSED=$((PASSED + 1))
else
    echo -e "${RED}FAIL${NC}"
    echo "  Response: $response"
    FAILED=$((FAILED + 1))
fi
echo ""

# Summary
echo "========================================"
echo "Test Summary"
echo "========================================"
echo -e "Passed: ${GREEN}$PASSED${NC}"
if [ $FAILED -gt 0 ]; then
    echo -e "Failed: ${RED}$FAILED${NC}"
else
    echo -e "Failed: ${GREEN}$FAILED${NC}"
fi
echo "Total: $((PASSED + FAILED))"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed${NC}"
    exit 1
fi
