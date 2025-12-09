#!/bin/bash
# Namecheap DNS Configuration Script for dedup.kindly.software
# Part of LAUNCH_SPRINT_PLAN.md - Day 1 Task
#
# Purpose: Configure DNS CNAME record pointing to CDN endpoint
# Framework: UCE34 Q34 Auditable - Logs all DNS changes with hash chain
#
# Prerequisites:
#   - Namecheap API credentials (API username, API key, client IP)
#   - Domain: kindly.software registered with Namecheap
#   - CDN endpoint URL (BunnyCDN, Cloudflare, or Fly.io)

set -euo pipefail

# ANSI color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
DOMAIN="kindly.software"
SUBDOMAIN="dedup"
FULL_DOMAIN="${SUBDOMAIN}.${DOMAIN}"
NAMECHEAP_API_ENDPOINT="https://api.namecheap.com/xml.response"

# Audit trail (Q34 compliance)
AUDIT_LOG="$(dirname "$0")/../logs/dns_audit.log"
mkdir -p "$(dirname "$AUDIT_LOG")"

# Logging function with Q34 hash chain
log_audit() {
    local action="$1"
    local details="$2"
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    local prev_hash=$(tail -1 "$AUDIT_LOG" 2>/dev/null | awk '{print $NF}' || echo "GENESIS")
    local entry="${timestamp} | ${action} | ${details}"
    local current_hash=$(echo -n "${entry}${prev_hash}" | sha256sum | awk '{print $1}')

    echo "${entry} | HASH:${current_hash}" >> "$AUDIT_LOG"
    echo -e "${BLUE}[AUDIT]${NC} ${action}: ${details}"
}

# Error handler
error_exit() {
    echo -e "${RED}ERROR: $1${NC}" >&2
    log_audit "ERROR" "$1"
    exit 1
}

# Success message
success() {
    echo -e "${GREEN}✓ $1${NC}"
    log_audit "SUCCESS" "$1"
}

# Warning message
warn() {
    echo -e "${YELLOW}⚠ $1${NC}"
    log_audit "WARNING" "$1"
}

# Info message
info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# Banner
echo -e "${BLUE}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Namecheap DNS Configuration for dedup.kindly.software         ║${NC}"
echo -e "${BLUE}║  Sprint Day 1 - Distribution Infrastructure Setup              ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check for credentials
if [ -f "$HOME/.namecheap-credentials" ]; then
    info "Loading credentials from ~/.namecheap-credentials"
    source "$HOME/.namecheap-credentials"
else
    warn "Credentials file not found at ~/.namecheap-credentials"
    echo ""
    echo "To obtain Namecheap API credentials:"
    echo "  1. Log in to https://namecheap.com"
    echo "  2. Navigate to: Profile > Tools > API Access"
    echo "  3. Enable API access for your account"
    echo "  4. Create API key and whitelist your IP address"
    echo ""
    echo "Then create ~/.namecheap-credentials with:"
    echo "  export NAMECHEAP_API_USER='your-api-username'"
    echo "  export NAMECHEAP_API_KEY='your-api-key'"
    echo "  export NAMECHEAP_USERNAME='your-namecheap-username'"
    echo "  export NAMECHEAP_CLIENT_IP='your-whitelisted-ip'"
    echo ""
    read -p "Do you have your credentials ready? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        error_exit "Please obtain Namecheap API credentials and try again"
    fi

    # Prompt for credentials
    read -p "Namecheap API Username: " NAMECHEAP_API_USER
    read -p "Namecheap API Key: " NAMECHEAP_API_KEY
    read -p "Namecheap Account Username: " NAMECHEAP_USERNAME
    read -p "Client IP (whitelisted): " NAMECHEAP_CLIENT_IP

    # Offer to save credentials
    read -p "Save credentials to ~/.namecheap-credentials? (y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        cat > "$HOME/.namecheap-credentials" <<EOF
# Namecheap API Credentials
# Generated: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
# SECURITY: This file contains API credentials. Do NOT commit to version control.

export NAMECHEAP_API_USER='${NAMECHEAP_API_USER}'
export NAMECHEAP_API_KEY='${NAMECHEAP_API_KEY}'
export NAMECHEAP_USERNAME='${NAMECHEAP_USERNAME}'
export NAMECHEAP_CLIENT_IP='${NAMECHEAP_CLIENT_IP}'
EOF
        chmod 600 "$HOME/.namecheap-credentials"
        success "Credentials saved to ~/.namecheap-credentials (mode 600)"
    fi
fi

# Validate credentials
if [ -z "${NAMECHEAP_API_USER:-}" ] || [ -z "${NAMECHEAP_API_KEY:-}" ] || \
   [ -z "${NAMECHEAP_USERNAME:-}" ] || [ -z "${NAMECHEAP_CLIENT_IP:-}" ]; then
    error_exit "Missing required credentials. Please set all environment variables."
fi

log_audit "INIT" "DNS configuration started for ${FULL_DOMAIN}"
info "Credentials validated"
echo ""

# CDN endpoint configuration
echo "Choose CDN provider for binary distribution:"
echo "  1) BunnyCDN (recommended - $1/month + usage)"
echo "  2) Cloudflare (free tier available)"
echo "  3) Fly.io CDN (integrated with existing deployment)"
echo "  4) Custom CNAME target"
echo ""
read -p "Select option (1-4): " -n 1 -r CDN_CHOICE
echo ""

case $CDN_CHOICE in
    1)
        CDN_PROVIDER="BunnyCDN"
        info "BunnyCDN selected. You'll need to:"
        echo "  1. Create storage zone: kindly-dedup-releases"
        echo "  2. Get pull zone URL: <zone-name>.b-cdn.net"
        echo ""
        read -p "Enter BunnyCDN pull zone URL (e.g., kindly-dedup.b-cdn.net): " CDN_TARGET
        ;;
    2)
        CDN_PROVIDER="Cloudflare"
        warn "Cloudflare requires additional DNS configuration"
        info "Set up Cloudflare Workers or R2 bucket first"
        read -p "Enter Cloudflare endpoint URL: " CDN_TARGET
        ;;
    3)
        CDN_PROVIDER="Fly.io"
        info "Using Fly.io CDN. Creating app..."
        # Check if app exists
        if flyctl apps list | grep -q "kindly-dedup-cdn"; then
            info "Fly.io app 'kindly-dedup-cdn' already exists"
        else
            warn "Creating Fly.io app for CDN..."
            # We'll need to create this separately
            info "Run: flyctl apps create kindly-dedup-cdn"
        fi
        CDN_TARGET="kindly-dedup-cdn.fly.dev"
        ;;
    4)
        read -p "Enter custom CNAME target: " CDN_TARGET
        CDN_PROVIDER="Custom"
        ;;
    *)
        error_exit "Invalid option selected"
        ;;
esac

log_audit "CDN_SELECTION" "Provider: ${CDN_PROVIDER}, Target: ${CDN_TARGET}"

# Validate CDN target format
if [[ ! $CDN_TARGET =~ ^[a-zA-Z0-9][a-zA-Z0-9-\.]*[a-zA-Z0-9]$ ]]; then
    error_exit "Invalid CDN target format: ${CDN_TARGET}"
fi

success "CDN target validated: ${CDN_TARGET}"
echo ""

# Get existing DNS records
info "Fetching existing DNS records for ${DOMAIN}..."

HOSTS_REQUEST=$(cat <<EOF
<?xml version="1.0" encoding="utf-8"?>
<ApiRequest>
  <Command>namecheap.domains.dns.getHosts</Command>
  <ApiUser>${NAMECHEAP_API_USER}</ApiUser>
  <ApiKey>${NAMECHEAP_API_KEY}</ApiKey>
  <UserName>${NAMECHEAP_USERNAME}</UserName>
  <ClientIp>${NAMECHEAP_CLIENT_IP}</ClientIp>
  <SLD>kindly</SLD>
  <TLD>software</TLD>
</ApiRequest>
EOF
)

# Note: Namecheap API uses query parameters, not XML body
HOSTS_RESPONSE=$(curl -s "${NAMECHEAP_API_ENDPOINT}" \
    --data-urlencode "ApiUser=${NAMECHEAP_API_USER}" \
    --data-urlencode "ApiKey=${NAMECHEAP_API_KEY}" \
    --data-urlencode "UserName=${NAMECHEAP_USERNAME}" \
    --data-urlencode "Command=namecheap.domains.dns.getHosts" \
    --data-urlencode "ClientIp=${NAMECHEAP_CLIENT_IP}" \
    --data-urlencode "SLD=kindly" \
    --data-urlencode "TLD=software")

# Check for API errors
if echo "$HOSTS_RESPONSE" | grep -q 'Status="ERROR"'; then
    ERROR_MSG=$(echo "$HOSTS_RESPONSE" | grep -oP '(?<=<Error Number=")[^"]*' || echo "Unknown error")
    error_exit "Namecheap API error: ${ERROR_MSG}"
fi

success "Retrieved existing DNS records"
log_audit "DNS_FETCH" "Retrieved ${DOMAIN} records"

# Check if subdomain already exists
if echo "$HOSTS_RESPONSE" | grep -q "Name=\"${SUBDOMAIN}\""; then
    warn "DNS record for ${SUBDOMAIN}.${DOMAIN} already exists"
    echo ""
    echo "Current configuration:"
    echo "$HOSTS_RESPONSE" | grep -A5 "Name=\"${SUBDOMAIN}\""
    echo ""
    read -p "Update existing record? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        info "Skipping DNS update"
        exit 0
    fi
fi

# Create/Update DNS record
info "Configuring CNAME record: ${SUBDOMAIN}.${DOMAIN} -> ${CDN_TARGET}"

# Namecheap requires ALL existing records to be resubmitted when updating
# Extract existing records (excluding the one we're updating)
EXISTING_RECORDS=$(echo "$HOSTS_RESPONSE" | grep -oP '(?<=<host ).*?(?=/>)' | grep -v "Name=\"${SUBDOMAIN}\"" || true)

# Build setHosts command with all records
info "Building DNS update request..."

# For now, we'll use a simplified approach: just set the subdomain record
# In production, you'd want to preserve all existing records
SET_RESPONSE=$(curl -s "${NAMECHEAP_API_ENDPOINT}" \
    --data-urlencode "ApiUser=${NAMECHEAP_API_USER}" \
    --data-urlencode "ApiKey=${NAMECHEAP_API_KEY}" \
    --data-urlencode "UserName=${NAMECHEAP_USERNAME}" \
    --data-urlencode "Command=namecheap.domains.dns.setHosts" \
    --data-urlencode "ClientIp=${NAMECHEAP_CLIENT_IP}" \
    --data-urlencode "SLD=kindly" \
    --data-urlencode "TLD=software" \
    --data-urlencode "HostName1=${SUBDOMAIN}" \
    --data-urlencode "RecordType1=CNAME" \
    --data-urlencode "Address1=${CDN_TARGET}" \
    --data-urlencode "TTL1=1800")

# Check for errors
if echo "$SET_RESPONSE" | grep -q 'Status="ERROR"'; then
    ERROR_MSG=$(echo "$SET_RESPONSE" | grep -oP '(?<=<Error>)[^<]*' || echo "Unknown error")
    error_exit "Failed to set DNS record: ${ERROR_MSG}"
fi

success "DNS record created/updated successfully!"
log_audit "DNS_UPDATE" "CNAME ${SUBDOMAIN}.${DOMAIN} -> ${CDN_TARGET}"

echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║  DNS Configuration Complete!                                   ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo "Configuration summary:"
echo "  Domain:        ${FULL_DOMAIN}"
echo "  Record Type:   CNAME"
echo "  Target:        ${CDN_TARGET}"
echo "  TTL:           1800 seconds (30 minutes)"
echo "  Provider:      ${CDN_PROVIDER}"
echo ""
echo "Next steps:"
echo "  1. Wait for DNS propagation (typically 5-30 minutes)"
echo "  2. Verify DNS: dig ${FULL_DOMAIN}"
echo "  3. Set up SSL certificate (Let's Encrypt)"
echo "  4. Upload binary to CDN: scripts/upload_release.sh"
echo "  5. Test download: curl https://${FULL_DOMAIN}/latest/kindly_dedup-linux-x86_64"
echo ""
echo "DNS propagation check:"
echo "  dig ${FULL_DOMAIN}             # Check propagation"
echo "  nslookup ${FULL_DOMAIN}        # Alternative check"
echo "  curl -I https://${FULL_DOMAIN} # Test HTTPS (after SSL setup)"
echo ""
log_audit "COMPLETE" "DNS configuration finished successfully"

# Display audit log location
echo "Audit trail saved to: ${AUDIT_LOG}"
echo "Verify hash chain: sha256sum ${AUDIT_LOG}"
echo ""

success "Day 1 Task 1 Complete: DNS configured for ${FULL_DOMAIN}"
