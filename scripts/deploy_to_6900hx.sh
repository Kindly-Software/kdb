#!/bin/bash
# Deploy atomic_capsule HTTP server to 6900HX production server
#
# Purpose: Upload compiled binary to 6900HX, set permissions, restart service
# Target: 6900HX (192.168.0.38, AMD Ryzen 9 6900HX, 64GB DDR5, Ubuntu Server 24.04)
# Deployment: systemd service (atomic-http-server)
#
# Requirements: SSH access to 6900HX, sudo privileges on remote
# Time: ~30-60 seconds (network-dependent)

set -e  # Exit on any error

# ============================================================================
# Configuration
# ============================================================================

PROJECT_DIR="/home/samuel/Primitives/atomic_capsule"
BINARY_NAME="atomic_http_server"
BUILD_MODE="release"

# 6900HX Server Configuration
REMOTE_USER="samuel"
REMOTE_HOST="192.168.0.38"
REMOTE_SERVER="${REMOTE_USER}@${REMOTE_HOST}"
REMOTE_BINARY_DIR="/home/samuel/Primitives/atomic_capsule/target/$BUILD_MODE"
REMOTE_SERVICE_NAME="atomic-http-server"

# Local binary path
LOCAL_BINARY="${PROJECT_DIR}/target/${BUILD_MODE}/${BINARY_NAME}"

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ============================================================================
# Helper Functions
# ============================================================================

log_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

log_success() {
    echo -e "${GREEN}✅${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}⚠${NC}  $1"
}

log_error() {
    echo -e "${RED}❌${NC} $1"
}

log_server() {
    echo -e "${CYAN}→${NC}  $1"
}

check_ssh() {
    if ! ssh -o ConnectTimeout=5 "$REMOTE_SERVER" "echo" &>/dev/null; then
        log_error "SSH connection failed to $REMOTE_SERVER"
        echo "Troubleshooting:"
        echo "  1. Check network: ping $REMOTE_HOST"
        echo "  2. Check SSH: ssh -v $REMOTE_SERVER"
        echo "  3. Check WiFi: Connected to TP-Link_E1C8?"
        exit 1
    fi
}

# ============================================================================
# Pre-flight Checks
# ============================================================================

log_info "Running pre-flight checks..."

# Verify local binary exists
if [ ! -f "$LOCAL_BINARY" ]; then
    log_error "Binary not found: $LOCAL_BINARY"
    log_warn "Run ./build_http_server.sh first"
    exit 1
fi

SIZE=$(du -h "$LOCAL_BINARY" | cut -f1)
log_success "Local binary ready: $LOCAL_BINARY ($SIZE)"

# Check SSH connectivity
log_info "Testing SSH connection to $REMOTE_SERVER..."
check_ssh
log_success "SSH connection successful"

# Get remote info
log_info "Checking remote system..."
REMOTE_OS=$(ssh "$REMOTE_SERVER" "uname -s")
REMOTE_KERNEL=$(ssh "$REMOTE_SERVER" "uname -r" | cut -d. -f1-3)
REMOTE_USER_CHECK=$(ssh "$REMOTE_SERVER" "whoami")

log_server "OS: $REMOTE_OS"
log_server "Kernel: $REMOTE_KERNEL"
log_server "User: $REMOTE_USER_CHECK"

# ============================================================================
# Create Remote Directory
# ============================================================================

log_info "Ensuring remote directory exists..."
ssh "$REMOTE_SERVER" "mkdir -p '$REMOTE_BINARY_DIR'" || {
    log_error "Failed to create remote directory"
    exit 1
}

log_success "Remote directory ready: $REMOTE_BINARY_DIR"

# ============================================================================
# Upload Binary
# ============================================================================

log_info "Uploading binary to $REMOTE_SERVER..."
log_server "Source: $LOCAL_BINARY ($SIZE)"
log_server "Target: $REMOTE_SERVER:$REMOTE_BINARY_DIR/$BINARY_NAME"

scp -p "$LOCAL_BINARY" "$REMOTE_SERVER:$REMOTE_BINARY_DIR/$BINARY_NAME" || {
    log_error "SCP upload failed"
    exit 1
}

log_success "Binary uploaded"

# ============================================================================
# Remote Verification
# ============================================================================

log_info "Verifying remote binary..."

# Check file exists
ssh "$REMOTE_SERVER" "test -f '$REMOTE_BINARY_DIR/$BINARY_NAME'" || {
    log_error "Remote verification failed: binary not found after upload"
    exit 1
}

# Get remote size
REMOTE_SIZE=$(ssh "$REMOTE_SERVER" "du -h '$REMOTE_BINARY_DIR/$BINARY_NAME' | cut -f1")
log_success "Remote binary verified: $REMOTE_SIZE"

# Set executable permissions
log_info "Setting executable permissions..."
ssh "$REMOTE_SERVER" "chmod +x '$REMOTE_BINARY_DIR/$BINARY_NAME'" || {
    log_error "Failed to set executable permissions"
    exit 1
}

log_success "Permissions set (755)"

# ============================================================================
# Systemd Service Management
# ============================================================================

log_info "Checking systemd service status..."

SERVICE_STATUS=$(ssh "$REMOTE_SERVER" "systemctl is-active $REMOTE_SERVICE_NAME 2>/dev/null" || echo "inactive")

if [ "$SERVICE_STATUS" = "active" ]; then
    log_warn "Service is currently active. Stopping before update..."
    ssh "$REMOTE_SERVER" "sudo systemctl stop $REMOTE_SERVICE_NAME" || {
        log_warn "Failed to stop service (may be OK if it's user-managed)"
    }
    sleep 2
fi

# Create/Update systemd service unit file (if needed)
log_info "Checking systemd unit file..."

UNIT_FILE="/etc/systemd/system/$REMOTE_SERVICE_NAME.service"

UNIT_EXISTS=$(ssh "$REMOTE_SERVER" "sudo test -f '$UNIT_FILE' && echo 'yes' || echo 'no'")

if [ "$UNIT_EXISTS" = "no" ]; then
    log_warn "Systemd unit file not found. Creating..."

    # Create unit file content
    UNIT_CONTENT="[Unit]
Description=Atomic Capsule HTTP Server
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$REMOTE_USER
WorkingDirectory=/home/$REMOTE_USER/Primitives
ExecStart=$REMOTE_BINARY_DIR/$BINARY_NAME
Restart=on-failure
RestartSec=5

# Security hardening
NoNewPrivileges=true
PrivateTmp=true

# Resource limits
LimitNOFILE=65535
LimitNPROC=32768

# Logging
StandardOutput=journal
StandardError=journal
SyslogIdentifier=atomic-http

[Install]
WantedBy=multi-user.target
"

    # Write unit file
    ssh "$REMOTE_SERVER" "cat > /tmp/atomic-http-server.service << 'EOF'
$UNIT_CONTENT
EOF
" || {
        log_error "Failed to create temporary unit file"
        exit 1
    }

    # Copy to system location with sudo
    ssh "$REMOTE_SERVER" "sudo cp /tmp/atomic-http-server.service '$UNIT_FILE'" || {
        log_error "Failed to install systemd unit file"
        exit 1
    }

    # Reload systemd
    ssh "$REMOTE_SERVER" "sudo systemctl daemon-reload" || {
        log_error "Failed to reload systemd"
        exit 1
    }

    log_success "Systemd unit file created"
else
    log_success "Systemd unit file already exists"
fi

# ============================================================================
# Start Service
# ============================================================================

log_info "Starting service..."
ssh "$REMOTE_SERVER" "sudo systemctl restart $REMOTE_SERVICE_NAME" || {
    log_error "Failed to start service"
    exit 1
}

sleep 2

# Check service status
SERVICE_STATUS=$(ssh "$REMOTE_SERVER" "systemctl is-active $REMOTE_SERVICE_NAME 2>/dev/null" || echo "unknown")

if [ "$SERVICE_STATUS" = "active" ]; then
    log_success "Service started successfully"
else
    log_warn "Service status: $SERVICE_STATUS"
    log_info "Checking systemd logs for errors..."
    ssh "$REMOTE_SERVER" "sudo journalctl -u $REMOTE_SERVICE_NAME -n 20 --no-pager"
fi

# ============================================================================
# Health Check
# ============================================================================

log_info "Performing health check..."

# Wait for service to be ready
sleep 2

# Get PID
PID=$(ssh "$REMOTE_SERVER" "systemctl show -p MainPID --value $REMOTE_SERVICE_NAME")

if [ -n "$PID" ] && [ "$PID" != "0" ]; then
    log_success "Service running with PID $PID"

    # Check listening ports
    log_info "Checking listening ports..."
    PORTS=$(ssh "$REMOTE_SERVER" "sudo lsof -i -P -n | grep $PID" 2>/dev/null || echo "No open ports yet")
    log_server "$PORTS"
else
    log_warn "Could not determine service PID"
    log_info "Check manually: ssh $REMOTE_SERVER 'systemctl status $REMOTE_SERVICE_NAME'"
fi

# ============================================================================
# Deployment Summary
# ============================================================================

echo ""
log_success "Deployment complete!"
echo ""
log_info "Deployment summary:"
log_server "Server: $REMOTE_SERVER"
log_server "Binary: $REMOTE_BINARY_DIR/$BINARY_NAME ($REMOTE_SIZE)"
log_server "Service: $REMOTE_SERVICE_NAME"
log_server "Status: $(ssh "$REMOTE_SERVER" "systemctl is-active $REMOTE_SERVICE_NAME 2>/dev/null" || echo "unknown")"
echo ""

log_info "Next steps:"
log_server "Check status: ssh $REMOTE_SERVER 'systemctl status $REMOTE_SERVICE_NAME'"
log_server "View logs: ssh $REMOTE_SERVER 'sudo journalctl -u $REMOTE_SERVICE_NAME -f'"
log_server "Test HTTP: curl http://$REMOTE_HOST:8080/health"
log_server "Stop service: ssh $REMOTE_SERVER 'sudo systemctl stop $REMOTE_SERVICE_NAME'"
echo ""

log_info "Useful commands:"
log_server "Restart: ssh $REMOTE_SERVER 'sudo systemctl restart $REMOTE_SERVICE_NAME'"
log_server "Logs (50 lines): ssh $REMOTE_SERVER 'sudo journalctl -u $REMOTE_SERVICE_NAME -n 50'"
log_server "Logs (follow): ssh $REMOTE_SERVER 'sudo journalctl -u $REMOTE_SERVICE_NAME -f'"
log_server "Resource usage: ssh $REMOTE_SERVER 'systemctl show -p MemoryCurrent --value $REMOTE_SERVICE_NAME'"
echo ""
