#!/bin/bash
# Quick status check for atomic_capsule HTTP server on 6900HX
#
# Usage: ./check_http_server_status.sh
# Output: Service status, resource usage, recent logs

set -e

# Configuration
REMOTE_USER="samuel"
REMOTE_HOST="192.168.0.38"
REMOTE_SERVER="${REMOTE_USER}@${REMOTE_HOST}"
SERVICE_NAME="atomic-http-server"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

log_header() {
    echo ""
    echo -e "${BLUE}═════════════════════════════════════════${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}═════════════════════════════════════════${NC}"
}

log_section() {
    echo -e "${CYAN}▶ $1${NC}"
}

log_success() {
    echo -e "${GREEN}✓${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}⚠${NC} $1"
}

log_error() {
    echo -e "${RED}✗${NC} $1"
}

# Check SSH connectivity
if ! ssh -o ConnectTimeout=5 "$REMOTE_SERVER" "echo" &>/dev/null; then
    log_error "SSH connection failed to $REMOTE_SERVER"
    exit 1
fi

log_header "Atomic HTTP Server Status"

# ============================================================================
# Service Status
# ============================================================================

log_section "Service Status"

SERVICE_STATUS=$(ssh "$REMOTE_SERVER" "systemctl is-active $SERVICE_NAME 2>/dev/null" || echo "unknown")

case "$SERVICE_STATUS" in
    active)
        log_success "Status: ACTIVE"
        ;;
    inactive)
        log_warn "Status: INACTIVE"
        ;;
    failed)
        log_error "Status: FAILED"
        ;;
    *)
        log_warn "Status: $SERVICE_STATUS"
        ;;
esac

# Get full status
echo ""
ssh "$REMOTE_SERVER" "systemctl status $SERVICE_NAME --no-pager" 2>/dev/null | head -15 || true

# ============================================================================
# Process Information
# ============================================================================

log_section "Process Information"

PID=$(ssh "$REMOTE_SERVER" "systemctl show -p MainPID --value $SERVICE_NAME")

if [ -n "$PID" ] && [ "$PID" != "0" ]; then
    log_success "PID: $PID"

    # Memory
    MEM=$(ssh "$REMOTE_SERVER" "systemctl show -p MemoryCurrent --value $SERVICE_NAME")
    if [ -n "$MEM" ] && [ "$MEM" != "0" ]; then
        MEM_MB=$((MEM / 1024 / 1024))
        log_success "Memory: ${MEM_MB}MB"
    fi

    # CPU Usage
    CPU=$(ssh "$REMOTE_SERVER" "systemctl show -p CPUUsageNSec --value $SERVICE_NAME")
    if [ -n "$CPU" ] && [ "$CPU" != "0" ]; then
        CPU_SEC=$((CPU / 1000000000))
        log_success "CPU time: ${CPU_SEC}s"
    fi

    # Uptime
    UPTIME=$(ssh "$REMOTE_SERVER" "systemctl show -p ActiveEnterTimestamp --value $SERVICE_NAME")
    if [ -n "$UPTIME" ]; then
        log_success "Started: $UPTIME"
    fi
else
    log_warn "Service not running (PID: $PID)"
fi

# ============================================================================
# Listening Ports
# ============================================================================

log_section "Network Ports"

PORTS=$(ssh "$REMOTE_SERVER" "sudo lsof -i -P -n 2>/dev/null | grep $SERVICE_NAME" | head -10 || echo "No ports")

if [ "$PORTS" != "No ports" ]; then
    echo "$PORTS"
else
    log_warn "Service not listening (may not be fully initialized)"
fi

# ============================================================================
# Recent Logs
# ============================================================================

log_section "Recent Logs (Last 20 lines)"

ssh "$REMOTE_SERVER" "sudo journalctl -u $SERVICE_NAME -n 20 --no-pager" 2>/dev/null || {
    log_warn "Unable to retrieve logs"
}

# ============================================================================
# Error Check
# ============================================================================

log_section "Error Check"

ERROR_COUNT=$(ssh "$REMOTE_SERVER" "sudo journalctl -u $SERVICE_NAME --priority err -n 100 2>/dev/null | wc -l" || echo "0")

if [ "$ERROR_COUNT" -gt 0 ]; then
    log_warn "Found $ERROR_COUNT error(s) in recent logs"
    ssh "$REMOTE_SERVER" "sudo journalctl -u $SERVICE_NAME --priority err -n 5 --no-pager" 2>/dev/null || true
else
    log_success "No recent errors"
fi

# ============================================================================
# Summary
# ============================================================================

echo ""
log_header "Summary"

STATUS_EMOJI="❌"
if [ "$SERVICE_STATUS" = "active" ]; then
    STATUS_EMOJI="✅"
fi

echo -e "Status: $STATUS_EMOJI $SERVICE_STATUS"
echo -e "Server: $REMOTE_SERVER"
echo -e "Service: $SERVICE_NAME"
echo ""

if [ "$SERVICE_STATUS" != "active" ]; then
    log_warn "Service is not running!"
    echo ""
    log_section "Recovery Steps:"
    echo "1. Check logs: ssh $REMOTE_SERVER 'sudo journalctl -u $SERVICE_NAME -n 100'"
    echo "2. Restart: ssh $REMOTE_SERVER 'sudo systemctl restart $SERVICE_NAME'"
    echo "3. Check binary: ssh $REMOTE_SERVER 'file /home/samuel/Primitives/atomic_capsule/target/release/atomic_http_server'"
fi

echo ""
