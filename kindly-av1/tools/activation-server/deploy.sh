#!/bin/bash
# kindly-av1 Activation Server Deployment Script
# [TRADE SECRET] - Deploy to kindly-hub only
#
# Usage: ./deploy.sh [command]
# Commands:
#   build     - Build release binary locally
#   deploy    - Deploy to kindly-hub
#   install   - Install systemd service on kindly-hub
#   start     - Start the service
#   stop      - Stop the service
#   status    - Check service status
#   logs      - View service logs
#   all       - Build, deploy, install, and start

set -euo pipefail

# Configuration
REMOTE_HOST="kindly-hub"
REMOTE_USER="samuel"
REMOTE_DIR="/home/samuel/kindly-av1-activation"
LOCAL_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$LOCAL_DIR/../.." && pwd)"
BINARY_NAME="activation-server"
SERVICE_NAME="kindly-av1-activation"

# Colors
PURPLE='\033[38;2;155;89;182m'
GOLD='\033[38;2;241;196;15m'
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m'

log_info() { echo -e "${PURPLE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_gold() { echo -e "${GOLD}[KINDLY-AV1]${NC} $1"; }

cmd_build() {
    log_info "Building release binary on $REMOTE_HOST (avoids CPU arch issues)..."

    # Build on remote to avoid CPU architecture mismatches
    ssh "$REMOTE_USER@$REMOTE_HOST" "
        source ~/.cargo/env
        cd ~/Primitives/kindly-av1/tools/activation-server
        cargo build --release 2>&1 | tail -5
    "

    local size=$(ssh "$REMOTE_USER@$REMOTE_HOST" "ls -lh ~/Primitives/kindly-av1/tools/activation-server/target/release/$BINARY_NAME | awk '{print \$5}'")
    log_success "Built $BINARY_NAME ($size) on $REMOTE_HOST"
}

cmd_deploy() {
    log_info "Deploying to $REMOTE_HOST..."

    # Create remote directories
    ssh "$REMOTE_USER@$REMOTE_HOST" "mkdir -p $REMOTE_DIR/{bin,keys}"

    # Copy binary from remote build location (avoids CPU arch issues)
    ssh "$REMOTE_USER@$REMOTE_HOST" "cp ~/Primitives/kindly-av1/tools/activation-server/target/release/$BINARY_NAME $REMOTE_DIR/bin/"
    log_success "Deployed binary to $REMOTE_DIR/bin/"

    # Copy signing key (if exists locally)
    local key_file="$PROJECT_ROOT/keys/signing_key.bin"
    if [[ -f "$key_file" ]]; then
        scp "$key_file" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_DIR/keys/"
        log_success "Copied signing key"
    else
        log_info "No local signing key found - ensure it exists on remote"
    fi

    # Copy public key (for reference)
    local pub_key="$PROJECT_ROOT/keys/public_key.bin"
    if [[ -f "$pub_key" ]]; then
        scp "$pub_key" "$REMOTE_USER@$REMOTE_HOST:$REMOTE_DIR/keys/"
    fi

    # Create .env template if it doesn't exist
    ssh "$REMOTE_USER@$REMOTE_HOST" "cat > $REMOTE_DIR/.env.template << 'EOF'
# kindly-av1 Activation Server Environment
# Copy to .env and fill in values

# HTTP port (default: 8080)
PORT=8080

# Ed25519 signing key (BASE64 encoded, or use ED25519_SIGNING_KEY_PATH)
# ED25519_SIGNING_KEY=<base64-encoded-32-byte-key>
ED25519_SIGNING_KEY_PATH=$REMOTE_DIR/keys/signing_key.bin

# Gumroad product ID
GUMROAD_PRODUCT_ID=KINDLY_AV1_PLACEHOLDER

# Gumroad webhook secret (optional, for HMAC verification)
# GUMROAD_WEBHOOK_SECRET=<your-webhook-secret>
EOF"
    log_success "Created .env.template on remote"
}

cmd_install() {
    log_info "Installing systemd service..."

    # Copy service file
    scp "$LOCAL_DIR/systemd/$SERVICE_NAME.service" \
        "$REMOTE_USER@$REMOTE_HOST:/tmp/"

    # Install service (requires sudo)
    ssh "$REMOTE_USER@$REMOTE_HOST" "
        sudo cp /tmp/$SERVICE_NAME.service /etc/systemd/system/
        sudo systemctl daemon-reload
        rm /tmp/$SERVICE_NAME.service
    "
    log_success "Installed $SERVICE_NAME.service"

    # Create .env from template if it doesn't exist
    ssh "$REMOTE_USER@$REMOTE_HOST" "
        if [[ ! -f $REMOTE_DIR/.env ]]; then
            cp $REMOTE_DIR/.env.template $REMOTE_DIR/.env
            echo 'Created .env from template - please edit with real values'
        fi
    "
}

cmd_start() {
    log_info "Starting $SERVICE_NAME..."
    ssh "$REMOTE_USER@$REMOTE_HOST" "sudo systemctl start $SERVICE_NAME"
    sleep 1
    cmd_status
}

cmd_stop() {
    log_info "Stopping $SERVICE_NAME..."
    ssh "$REMOTE_USER@$REMOTE_HOST" "sudo systemctl stop $SERVICE_NAME"
    log_success "Service stopped"
}

cmd_status() {
    log_info "Service status:"
    ssh "$REMOTE_USER@$REMOTE_HOST" "
        systemctl is-active $SERVICE_NAME && echo 'Status: RUNNING' || echo 'Status: STOPPED'
        systemctl is-enabled $SERVICE_NAME 2>/dev/null && echo 'Enabled: YES' || echo 'Enabled: NO'
    " || true

    # Check if service is running and show health
    if ssh "$REMOTE_USER@$REMOTE_HOST" "systemctl is-active --quiet $SERVICE_NAME"; then
        log_info "Health check:"
        ssh "$REMOTE_USER@$REMOTE_HOST" "curl -s http://localhost:8080/health" || true
        echo
    fi
}

cmd_logs() {
    log_info "Service logs (last 50 lines, follow mode):"
    ssh "$REMOTE_USER@$REMOTE_HOST" "sudo journalctl -u $SERVICE_NAME -n 50 -f"
}

cmd_enable() {
    log_info "Enabling $SERVICE_NAME to start on boot..."
    ssh "$REMOTE_USER@$REMOTE_HOST" "sudo systemctl enable $SERVICE_NAME"
    log_success "Service enabled"
}

cmd_all() {
    log_gold "=== Full Deployment ==="
    cmd_build
    cmd_deploy
    cmd_install
    cmd_enable
    cmd_start
    log_gold "=== Deployment Complete ==="
}

cmd_help() {
    echo "kindly-av1 Activation Server Deployment"
    echo ""
    echo "Usage: $0 [command]"
    echo ""
    echo "Commands:"
    echo "  build     Build release binary locally"
    echo "  deploy    Deploy files to kindly-hub"
    echo "  install   Install systemd service"
    echo "  enable    Enable service to start on boot"
    echo "  start     Start the service"
    echo "  stop      Stop the service"
    echo "  status    Check service status"
    echo "  logs      View service logs (follow mode)"
    echo "  all       Full deployment (build, deploy, install, enable, start)"
    echo "  help      Show this help"
}

# Main
case "${1:-help}" in
    build)   cmd_build ;;
    deploy)  cmd_deploy ;;
    install) cmd_install ;;
    enable)  cmd_enable ;;
    start)   cmd_start ;;
    stop)    cmd_stop ;;
    status)  cmd_status ;;
    logs)    cmd_logs ;;
    all)     cmd_all ;;
    help)    cmd_help ;;
    *)       log_error "Unknown command: $1"; cmd_help; exit 1 ;;
esac
