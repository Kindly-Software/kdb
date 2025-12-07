#!/bin/bash
# Cloudflare WAF Configuration for kdb-mcp Production Hardening
# UCE34 Framework: Q34 Audit Compliance, T8 Network Security
#
# Prerequisites:
#   - CLOUDFLARE_API_KEY environment variable set
#   - CLOUDFLARE_EMAIL environment variable set
#   - CLOUDFLARE_ZONE_ID environment variable set
#
# Usage: ./cloudflare_waf.sh [apply|dry-run|status]

set -euo pipefail

# =============================================================================
# Configuration
# =============================================================================
CLOUDFLARE_API_BASE="https://api.cloudflare.com/client/v4"
ZONE_ID="${CLOUDFLARE_ZONE_ID:-}"
API_KEY="${CLOUDFLARE_API_KEY:-}"
API_EMAIL="${CLOUDFLARE_EMAIL:-samuel@kindly.software}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# =============================================================================
# Validation Functions
# =============================================================================
validate_credentials() {
    if [[ -z "$ZONE_ID" ]]; then
        echo -e "${RED}ERROR: CLOUDFLARE_ZONE_ID not set${NC}"
        exit 1
    fi
    if [[ -z "$API_KEY" ]]; then
        echo -e "${RED}ERROR: CLOUDFLARE_API_KEY not set${NC}"
        exit 1
    fi
}

# =============================================================================
# WAF Rule Definitions (UCE34 Q34 Audit Compliance)
# =============================================================================

# Rule 1: Block high-risk countries from debug API
create_country_block_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "(ip.geoip.country in {\"CN\" \"RU\" \"KP\" \"IR\"}) and (http.request.uri.path contains \"/debug\" or http.request.uri.path contains \"/api\")",
        "paused": false,
        "description": "Block high-risk countries from debug API endpoints"
    },
    "action": "block",
    "priority": 1,
    "description": "UCE34-WAF-001: High-risk country block for debug API"
}
EOF
}

# Rule 2: Rate limit aggressive clients (100 req/10s = 600/min)
create_rate_limit_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "http.request.uri.path contains \"/api\"",
        "paused": false,
        "description": "Rate limit API endpoints"
    },
    "action": "challenge",
    "priority": 10,
    "description": "UCE34-WAF-002: API rate limit challenge"
}
EOF
}

# Rule 3: Block known malicious user agents
create_ua_block_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "(http.user_agent contains \"sqlmap\") or (http.user_agent contains \"nikto\") or (http.user_agent contains \"nmap\") or (http.user_agent contains \"masscan\") or (http.user_agent contains \"zgrab\")",
        "paused": false,
        "description": "Block malicious scanner user agents"
    },
    "action": "block",
    "priority": 2,
    "description": "UCE34-WAF-003: Malicious scanner block"
}
EOF
}

# Rule 4: Challenge suspicious patterns (path traversal, SQL injection)
create_attack_challenge_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "(http.request.uri.query contains \"../\") or (http.request.uri.query contains \"union select\") or (http.request.uri.query contains \"<script\") or (http.request.uri contains \";/bin/\") or (http.request.uri contains \"|/bin/\")",
        "paused": false,
        "description": "Challenge potential attack patterns"
    },
    "action": "managed_challenge",
    "priority": 3,
    "description": "UCE34-WAF-004: Attack pattern challenge"
}
EOF
}

# Rule 5: Protect authentication endpoints
create_auth_protection_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "(http.request.uri.path contains \"/auth\") or (http.request.uri.path contains \"/login\") or (http.request.uri.path contains \"/token\")",
        "paused": false,
        "description": "Extra protection for auth endpoints"
    },
    "action": "managed_challenge",
    "priority": 5,
    "description": "UCE34-WAF-005: Auth endpoint protection"
}
EOF
}

# Rule 6: Block empty/suspicious referrers on sensitive endpoints
create_referrer_check_rule() {
    cat <<'EOF'
{
    "filter": {
        "expression": "(http.referer eq \"\") and (http.request.uri.path contains \"/admin\" or http.request.uri.path contains \"/internal\")",
        "paused": false,
        "description": "Block direct access to admin/internal endpoints"
    },
    "action": "block",
    "priority": 6,
    "description": "UCE34-WAF-006: Direct admin access block"
}
EOF
}

# =============================================================================
# Rate Limiting Configuration
# =============================================================================

create_rate_limit_config() {
    cat <<'EOF'
{
    "match": {
        "request": {
            "url_pattern": "*kdb.kindly.software/api/*",
            "schemes": ["HTTPS"],
            "methods": ["GET", "POST", "PUT", "DELETE"]
        }
    },
    "threshold": 100,
    "period": 10,
    "action": {
        "mode": "challenge",
        "timeout": 3600
    },
    "description": "UCE34-RL-001: Global API rate limit (100 req/10s)"
}
EOF
}

create_auth_rate_limit_config() {
    cat <<'EOF'
{
    "match": {
        "request": {
            "url_pattern": "*kdb.kindly.software/api/auth/*",
            "schemes": ["HTTPS"],
            "methods": ["POST"]
        }
    },
    "threshold": 10,
    "period": 60,
    "action": {
        "mode": "challenge",
        "timeout": 3600
    },
    "description": "UCE34-RL-002: Auth endpoint rate limit (10 req/min)"
}
EOF
}

# =============================================================================
# API Functions
# =============================================================================

cloudflare_api() {
    local method="$1"
    local endpoint="$2"
    local data="${3:-}"

    local curl_args=(
        -s
        -X "$method"
        -H "X-Auth-Email: $API_EMAIL"
        -H "X-Auth-Key: $API_KEY"
        -H "Content-Type: application/json"
    )

    if [[ -n "$data" ]]; then
        curl_args+=(-d "$data")
    fi

    curl "${curl_args[@]}" "${CLOUDFLARE_API_BASE}${endpoint}"
}

apply_firewall_rule() {
    local rule_json="$1"
    local rule_name="$2"

    echo -e "${YELLOW}Applying rule: $rule_name${NC}"

    local response
    response=$(cloudflare_api "POST" "/zones/${ZONE_ID}/firewall/rules" "$rule_json")

    if echo "$response" | grep -q '"success":true'; then
        echo -e "${GREEN}  SUCCESS: $rule_name applied${NC}"
        return 0
    else
        echo -e "${RED}  FAILED: $rule_name - $(echo "$response" | jq -r '.errors[0].message // "Unknown error"')${NC}"
        return 1
    fi
}

list_existing_rules() {
    echo -e "${YELLOW}Fetching existing WAF rules...${NC}"
    local response
    response=$(cloudflare_api "GET" "/zones/${ZONE_ID}/firewall/rules")

    if echo "$response" | grep -q '"success":true'; then
        echo "$response" | jq -r '.result[] | "  - \(.description // "No description") (\(.action))"'
    else
        echo -e "${RED}Failed to fetch rules${NC}"
    fi
}

# =============================================================================
# Main Execution
# =============================================================================

case "${1:-help}" in
    apply)
        validate_credentials
        echo -e "${GREEN}=== Applying Cloudflare WAF Rules for kdb-mcp ===${NC}"
        echo ""

        apply_firewall_rule "$(create_country_block_rule)" "Country Block (CN/RU/KP/IR)"
        apply_firewall_rule "$(create_ua_block_rule)" "Malicious UA Block"
        apply_firewall_rule "$(create_attack_challenge_rule)" "Attack Pattern Challenge"
        apply_firewall_rule "$(create_auth_protection_rule)" "Auth Endpoint Protection"
        apply_firewall_rule "$(create_referrer_check_rule)" "Direct Admin Block"

        echo ""
        echo -e "${GREEN}=== WAF Rules Applied ===${NC}"
        ;;

    dry-run)
        echo -e "${YELLOW}=== Dry Run: WAF Rules Preview ===${NC}"
        echo ""
        echo "Rule 1: Country Block"
        create_country_block_rule | jq .
        echo ""
        echo "Rule 2: Malicious UA Block"
        create_ua_block_rule | jq .
        echo ""
        echo "Rule 3: Attack Pattern Challenge"
        create_attack_challenge_rule | jq .
        echo ""
        echo "Rule 4: Auth Endpoint Protection"
        create_auth_protection_rule | jq .
        echo ""
        echo "Rule 5: Direct Admin Block"
        create_referrer_check_rule | jq .
        ;;

    status)
        validate_credentials
        echo -e "${GREEN}=== Current Cloudflare WAF Rules ===${NC}"
        list_existing_rules
        ;;

    help|*)
        echo "Cloudflare WAF Configuration for kdb-mcp"
        echo ""
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  apply    - Apply WAF rules to Cloudflare zone"
        echo "  dry-run  - Preview rules without applying"
        echo "  status   - Show existing WAF rules"
        echo "  help     - Show this help message"
        echo ""
        echo "Required Environment Variables:"
        echo "  CLOUDFLARE_ZONE_ID  - Your Cloudflare zone ID"
        echo "  CLOUDFLARE_API_KEY  - Your Cloudflare API key"
        echo "  CLOUDFLARE_EMAIL    - Your Cloudflare account email"
        ;;
esac
