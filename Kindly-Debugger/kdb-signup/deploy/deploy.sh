#!/usr/bin/env bash
# KDB Signup Service Deployment Script
# Target: kindly-hub (192.168.0.38)
# Usage: ./deploy.sh [--no-build] [--restart-only]

set -euo pipefail

# Configuration
REMOTE_HOST="kindly-hub"
REMOTE_IP="192.168.0.38"
REMOTE_USER="samuel"
SERVICE_NAME="kdb-signup"
LOCAL_BIN_PATH="target/release/${SERVICE_NAME}"
REMOTE_INSTALL_DIR="/opt/${SERVICE_NAME}"
REMOTE_CONFIG_DIR="/etc/kdb"
REMOTE_LOG_DIR="/var/log/${SERVICE_NAME}"
SERVICE_FILE="deploy/${SERVICE_NAME}.service"
ENV_TEMPLATE="deploy/${SERVICE_NAME}.env.template"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Parse arguments
NO_BUILD=0
RESTART_ONLY=0
for arg in "$@"; do
    case $arg in
        --no-build)
            NO_BUILD=1
            shift
            ;;
        --restart-only)
            RESTART_ONLY=1
            shift
            ;;
    esac
done

echo -e "${GREEN}[KDB Signup Deployment]${NC}"
echo "Target: ${REMOTE_USER}@${REMOTE_HOST} (${REMOTE_IP})"
echo ""

# Check SSH connectivity
echo -e "${YELLOW}[1/9]${NC} Checking SSH connectivity..."
if ! ssh -o ConnectTimeout=5 "${REMOTE_USER}@${REMOTE_HOST}" "echo 'SSH OK'" &>/dev/null; then
    echo -e "${RED}ERROR: Cannot connect to ${REMOTE_HOST}${NC}"
    echo "Please verify:"
    echo "  1. SSH is running on ${REMOTE_HOST}"
    echo "  2. SSH keys are configured"
    echo "  3. Network connectivity to ${REMOTE_IP}"
    exit 1
fi
echo "✓ SSH connection verified"

# Restart-only mode
if [ $RESTART_ONLY -eq 1 ]; then
    echo -e "${YELLOW}[RESTART-ONLY MODE]${NC}"
    ssh "${REMOTE_USER}@${REMOTE_HOST}" "sudo systemctl restart ${SERVICE_NAME}"
    echo -e "${GREEN}✓ Service restarted${NC}"

    # Verify health
    echo -e "${YELLOW}[Health Check]${NC}"
    sleep 2
    ssh "${REMOTE_USER}@${REMOTE_HOST}" "curl -sf http://localhost:8091/health" || {
        echo -e "${RED}ERROR: Health check failed${NC}"
        exit 1
    }
    echo -e "${GREEN}✓ Service healthy${NC}"
    exit 0
fi

# Build binary
if [ $NO_BUILD -eq 0 ]; then
    echo -e "${YELLOW}[2/9]${NC} Building release binary..."
    cargo build --release --bin "${SERVICE_NAME}"
    echo "✓ Binary built: ${LOCAL_BIN_PATH}"
else
    echo -e "${YELLOW}[2/9]${NC} Skipping build (--no-build flag)"
    if [ ! -f "${LOCAL_BIN_PATH}" ]; then
        echo -e "${RED}ERROR: Binary not found at ${LOCAL_BIN_PATH}${NC}"
        exit 1
    fi
fi

# Verify binary size and permissions
echo -e "${YELLOW}[3/9]${NC} Verifying binary..."
BIN_SIZE=$(stat -c%s "${LOCAL_BIN_PATH}" 2>/dev/null || stat -f%z "${LOCAL_BIN_PATH}")
BIN_SIZE_MB=$((BIN_SIZE / 1024 / 1024))
echo "  Size: ${BIN_SIZE_MB} MB"
if [ $BIN_SIZE_MB -lt 1 ]; then
    echo -e "${RED}WARNING: Binary size is suspiciously small${NC}"
fi
echo "✓ Binary verified"

# Create remote directories
echo -e "${YELLOW}[4/9]${NC} Creating remote directories..."
ssh "${REMOTE_USER}@${REMOTE_HOST}" "sudo mkdir -p ${REMOTE_INSTALL_DIR} ${REMOTE_CONFIG_DIR} ${REMOTE_LOG_DIR}"
echo "✓ Directories created"

# Create kdb user if not exists
echo -e "${YELLOW}[5/9]${NC} Creating kdb user..."
ssh "${REMOTE_USER}@${REMOTE_HOST}" "
    if ! id kdb &>/dev/null; then
        sudo useradd -r -s /bin/false -d ${REMOTE_INSTALL_DIR} -c 'KDB Signup Service' kdb
        echo '✓ User kdb created'
    else
        echo '✓ User kdb already exists'
    fi
"

# Copy binary to remote
echo -e "${YELLOW}[6/9]${NC} Copying binary to remote..."
scp "${LOCAL_BIN_PATH}" "${REMOTE_USER}@${REMOTE_HOST}:/tmp/${SERVICE_NAME}"
ssh "${REMOTE_USER}@${REMOTE_HOST}" "
    sudo mv /tmp/${SERVICE_NAME} ${REMOTE_INSTALL_DIR}/${SERVICE_NAME}
    sudo chmod 755 ${REMOTE_INSTALL_DIR}/${SERVICE_NAME}
    sudo chown kdb:kdb ${REMOTE_INSTALL_DIR}/${SERVICE_NAME}
"
echo "✓ Binary deployed"

# Copy systemd service file
echo -e "${YELLOW}[7/9]${NC} Installing systemd service..."
scp "${SERVICE_FILE}" "${REMOTE_USER}@${REMOTE_HOST}:/tmp/${SERVICE_NAME}.service"
ssh "${REMOTE_USER}@${REMOTE_HOST}" "
    sudo mv /tmp/${SERVICE_NAME}.service /etc/systemd/system/${SERVICE_NAME}.service
    sudo chmod 644 /etc/systemd/system/${SERVICE_NAME}.service
    sudo systemctl daemon-reload
"
echo "✓ Systemd service installed"

# Check if environment file exists
echo -e "${YELLOW}[8/9]${NC} Checking environment configuration..."
ENV_EXISTS=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" "test -f ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env && echo 1 || echo 0")
if [ "$ENV_EXISTS" -eq 0 ]; then
    echo -e "${YELLOW}WARNING: Environment file not found at ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env${NC}"
    echo "Copying template..."
    scp "${ENV_TEMPLATE}" "${REMOTE_USER}@${REMOTE_HOST}:/tmp/${SERVICE_NAME}.env.template"
    ssh "${REMOTE_USER}@${REMOTE_HOST}" "
        sudo cp /tmp/${SERVICE_NAME}.env.template ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env
        sudo chmod 600 ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env
        sudo chown kdb:kdb ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env
        rm /tmp/${SERVICE_NAME}.env.template
    "
    echo -e "${RED}ACTION REQUIRED: Edit ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env on remote host${NC}"
    echo "Then run: sudo systemctl start ${SERVICE_NAME}"
    exit 0
else
    echo "✓ Environment file exists"
fi

# Set permissions
ssh "${REMOTE_USER}@${REMOTE_HOST}" "
    sudo chown -R kdb:kdb ${REMOTE_INSTALL_DIR}
    sudo chown -R kdb:kdb ${REMOTE_LOG_DIR}
    sudo chmod 700 ${REMOTE_CONFIG_DIR}
    sudo chmod 600 ${REMOTE_CONFIG_DIR}/${SERVICE_NAME}.env
"

# Enable and start service
echo -e "${YELLOW}[9/9]${NC} Starting service..."
ssh "${REMOTE_USER}@${REMOTE_HOST}" "
    sudo systemctl enable ${SERVICE_NAME}
    sudo systemctl restart ${SERVICE_NAME}
"
echo "✓ Service started"

# Wait for service to initialize
echo ""
echo -e "${YELLOW}[Verification]${NC} Waiting for service to initialize..."
sleep 3

# Check service status
echo -e "${YELLOW}Service Status:${NC}"
ssh "${REMOTE_USER}@${REMOTE_HOST}" "sudo systemctl status ${SERVICE_NAME} --no-pager -l" || true

# Health check
echo ""
echo -e "${YELLOW}Health Check:${NC}"
HEALTH_RESPONSE=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" "curl -sf http://localhost:8091/health" 2>/dev/null || echo "FAILED")
if [ "$HEALTH_RESPONSE" != "FAILED" ]; then
    echo -e "${GREEN}✓ Service is healthy${NC}"
    echo "Response: ${HEALTH_RESPONSE}"
else
    echo -e "${RED}✗ Health check failed${NC}"
    echo "Check logs with: ssh ${REMOTE_USER}@${REMOTE_HOST} 'sudo journalctl -u ${SERVICE_NAME} -n 50'"
    exit 1
fi

# Summary
echo ""
echo -e "${GREEN}[Deployment Complete]${NC}"
echo "Service: ${SERVICE_NAME}"
echo "Remote Host: ${REMOTE_HOST} (${REMOTE_IP})"
echo "Install Dir: ${REMOTE_INSTALL_DIR}"
echo "Config Dir: ${REMOTE_CONFIG_DIR}"
echo "Log Dir: ${REMOTE_LOG_DIR}"
echo ""
echo "Useful commands:"
echo "  Status:  ssh ${REMOTE_USER}@${REMOTE_HOST} 'sudo systemctl status ${SERVICE_NAME}'"
echo "  Logs:    ssh ${REMOTE_USER}@${REMOTE_HOST} 'sudo journalctl -u ${SERVICE_NAME} -f'"
echo "  Restart: ssh ${REMOTE_USER}@${REMOTE_HOST} 'sudo systemctl restart ${SERVICE_NAME}'"
echo "  Stop:    ssh ${REMOTE_USER}@${REMOTE_HOST} 'sudo systemctl stop ${SERVICE_NAME}'"
echo "  Health:  ssh ${REMOTE_USER}@${REMOTE_HOST} 'curl http://localhost:8091/health'"
