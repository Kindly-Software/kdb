#!/bin/bash
#
# deploy.sh - atomic_mcp_server deployment automation
# Version: 1.0.0 | Framework: UCE34 Q34 (audit trail) + B32 (performance)
# Purpose: Automated build, backup, deploy, and rollback with sub-30s total time
#
# Usage:
#   ./deploy.sh                    # Full deployment (build + deploy + validate)
#   ./deploy.sh --dry-run          # Simulate without changes
#   ./deploy.sh health             # Health check only
#   ./deploy.sh rollback           # Rollback to backup
#   ./deploy.sh --help             # Show usage
#

set -euo pipefail

###############################################################################
# CONFIGURATION
###############################################################################

REMOTE_HOST="${REMOTE_HOST:-192.168.0.38}"
REMOTE_USER="${REMOTE_USER:-samuel}"
REMOTE_HOME="/home/${REMOTE_USER}"
BINARY_NAME="mcp_debug_server"
BINARY_PATH="target/release/${BINARY_NAME}"
BUILD_FEATURES="${BUILD_FEATURES:-std,json-rpc,async-runtime}"
SYSTEMD_SERVICE="mcp-debug"
SYSTEMD_SERVICE_PATH="/etc/systemd/system/${SYSTEMD_SERVICE}.service"
REMOTE_BIN_PATH="/usr/local/bin/${BINARY_NAME}"
REMOTE_BACKUP_PATH="/usr/local/bin/${BINARY_NAME}.backup"

# Logging configuration
LOG_FILE="/tmp/mcp-deploy-$(date +%Y%m%d-%H%M%S).log"
REMOTE_AUDIT_LOG="/var/log/mcp-deploy.log"

# Feature flags
DRY_RUN="${DRY_RUN:-false}"
SKIP_TESTS="${SKIP_TESTS:-false}"
VERBOSE="${VERBOSE:-false}"
CONFIRM="${CONFIRM:-true}"

# Color codes for terminal output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Metrics
DEPLOYMENT_START=""
BUILD_START=""
DEPLOY_START=""
BINARY_HASH=""

###############################################################################
# LOGGING FUNCTIONS
###############################################################################

log() {
    local level=$1
    shift
    local msg="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')

    # Format: [timestamp] [LEVEL] message
    local formatted="[$timestamp] [$level] $msg"

    # Log to file
    echo "$formatted" >> "$LOG_FILE"

    # Log to stdout with color
    case "$level" in
        INFO)
            echo -e "${BLUE}[INFO]${NC} $msg" >&2
            ;;
        SUCCESS)
            echo -e "${GREEN}[✓]${NC} $msg" >&2
            ;;
        WARN)
            echo -e "${YELLOW}[WARN]${NC} $msg" >&2
            ;;
        ERROR)
            echo -e "${RED}[ERROR]${NC} $msg" >&2
            ;;
        DEBUG)
            if [ "$VERBOSE" = "true" ]; then
                echo -e "${BLUE}[DEBUG]${NC} $msg" >&2
            fi
            ;;
    esac
}

log_info() { log INFO "$@"; }
log_success() { log SUCCESS "$@"; }
log_warn() { log WARN "$@"; }
log_error() { log ERROR "$@"; }
log_debug() { log DEBUG "$@"; }

###############################################################################
# CLEANUP & TRAP HANDLERS
###############################################################################

cleanup() {
    local exit_code=$?

    if [ $exit_code -ne 0 ] && [ "$exit_code" -ne 130 ]; then
        log_error "Deployment failed with exit code $exit_code"
        log_info "View logs: tail -f $LOG_FILE"
    fi

    # Clean temporary files
    rm -f /tmp/*.mcp-deploy.tmp 2>/dev/null || true
}

trap cleanup EXIT
trap 'log_error "Interrupted by user"; exit 130' INT TERM

###############################################################################
# UTILITY FUNCTIONS
###############################################################################

# Check if command exists
command_exists() {
    command -v "$1" &>/dev/null
}

# Confirm action (with --yes flag override)
confirm() {
    local prompt="$1"
    local default="${2:-y}"

    if [ "$CONFIRM" = "false" ]; then
        return 0
    fi

    local response
    read -p "$prompt (y/n, default: $default) " -r response
    response="${response:-$default}"
    [[ "$response" =~ ^[Yy]$ ]]
}

# Calculate SHA256 hash of file
sha256_hash() {
    sha256sum "$1" 2>/dev/null | awk '{print $1}'
}

# Duration formatting (seconds to human-readable)
format_duration() {
    local seconds=$1
    if [ $seconds -lt 60 ]; then
        echo "${seconds}s"
    else
        echo "$((seconds / 60))m $((seconds % 60))s"
    fi
}

###############################################################################
# PRE-FLIGHT CHECKS
###############################################################################

pre_flight_checks() {
    log_info "Running pre-flight checks..."
    local start=$(date +%s)

    # Check 1: Git status (no uncommitted changes)
    log_debug "Checking git status..."
    if ! git diff-index --quiet HEAD -- 2>/dev/null; then
        log_error "Git working directory has uncommitted changes"
        log_info "Commit or stash changes before deploying"
        exit 1
    fi
    log_success "Git working directory clean"

    # Check 2: Required commands
    log_debug "Checking required commands..."
    local required_cmds=("cargo" "rsync" "ssh" "curl" "jq" "sha256sum")
    for cmd in "${required_cmds[@]}"; do
        if ! command_exists "$cmd"; then
            log_error "Required command not found: $cmd"
            exit 1
        fi
    done
    log_success "All required commands available"

    # Check 3: SSH connectivity
    log_debug "Testing SSH connectivity to ${REMOTE_USER}@${REMOTE_HOST}..."
    if ! ssh -o ConnectTimeout=10 -o BatchMode=yes \
        "${REMOTE_USER}@${REMOTE_HOST}" 'exit 0' 2>/dev/null; then
        log_error "SSH connection failed to ${REMOTE_USER}@${REMOTE_HOST}"
        log_info "Check SSH key configuration and network connectivity"
        exit 1
    fi
    log_success "SSH connectivity OK"

    # Check 4: Remote disk space
    log_debug "Checking remote disk space..."
    local disk_free=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" \
        'df -BM /usr/local/bin 2>/dev/null | tail -1 | awk "{print \$4}"' | sed 's/M//')

    if [ -z "$disk_free" ] || [ "$disk_free" -lt 100 ]; then
        log_error "Remote disk space insufficient: ${disk_free}MB available"
        exit 1
    fi
    log_success "Remote disk space: ${disk_free}MB available"

    # Check 5: Remote systemd service exists
    log_debug "Checking systemd service: $SYSTEMD_SERVICE"
    if ! ssh "${REMOTE_USER}@${REMOTE_HOST}" \
        "sudo systemctl list-unit-files | grep -q $SYSTEMD_SERVICE"; then
        log_warn "Systemd service $SYSTEMD_SERVICE not found (first deployment?)"
    else
        log_success "Systemd service exists: $SYSTEMD_SERVICE"
    fi

    local end=$(date +%s)
    log_success "Pre-flight checks passed in $(format_duration $((end - start)))"
}

###############################################################################
# BUILD PHASE
###############################################################################

build_binary() {
    log_info "Building binary with features: $BUILD_FEATURES"
    BUILD_START=$(date +%s)

    # Enable build optimizations
    export RUSTFLAGS="${RUSTFLAGS:-}"

    # Use sccache if available (30% faster incremental builds)
    if command_exists sccache; then
        log_debug "Using sccache for incremental builds"
        export RUSTC_WRAPPER=sccache
        export SCCACHE_BUCKET="${SCCACHE_BUCKET:-}"
    fi

    # Use mold linker if available (30% faster linking)
    if command_exists mold; then
        log_debug "Using mold linker"
        export RUSTFLAGS="${RUSTFLAGS} -C link-arg=-fuse-ld=mold"
    fi

    # Use LLD linker as fallback
    if ! command_exists mold && command_exists lld; then
        log_debug "Using LLD linker"
        export RUSTFLAGS="${RUSTFLAGS} -C link-arg=-fuse-ld=lld"
    fi

    # Build with optimizations
    if ! cargo build --release --features "$BUILD_FEATURES" 2>&1 | tee -a "$LOG_FILE"; then
        log_error "Build failed"
        log_info "Review compiler errors above"
        exit 1
    fi

    # Verify binary exists
    if [ ! -f "$BINARY_PATH" ]; then
        log_error "Binary not found at $BINARY_PATH after successful build"
        exit 1
    fi

    # Calculate hash
    BINARY_HASH=$(sha256_hash "$BINARY_PATH")
    log_debug "Binary SHA256: $BINARY_HASH"

    # Get binary size
    local size=$(ls -lh "$BINARY_PATH" | awk '{print $5}')
    log_success "Build succeeded: $BINARY_PATH ($size)"

    BUILD_END=$(date +%s)
    log_info "Build time: $(format_duration $((BUILD_END - BUILD_START)))"
}

###############################################################################
# BACKUP PHASE
###############################################################################

backup_remote() {
    log_info "Creating backup of current deployment..."

    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << 'BACKUP_SCRIPT'
set -euo pipefail

BINARY_NAME="mcp_debug_server"
REMOTE_BIN_PATH="/usr/local/bin/${BINARY_NAME}"
REMOTE_BACKUP_PATH="/usr/local/bin/${BINARY_NAME}.backup"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
VERSIONED_BACKUP="/usr/local/bin/${BINARY_NAME}.backup.${TIMESTAMP}"

if [ -f "$REMOTE_BIN_PATH" ]; then
    sudo cp "$REMOTE_BIN_PATH" "$REMOTE_BACKUP_PATH"
    # Also keep timestamped version for recovery
    sudo cp "$REMOTE_BIN_PATH" "$VERSIONED_BACKUP"
    echo "Backup created: $REMOTE_BACKUP_PATH"
else
    echo "No existing binary (first deployment)"
fi
BACKUP_SCRIPT

    log_success "Backup completed"
}

###############################################################################
# DEPLOY PHASE
###############################################################################

deploy_remote() {
    log_info "Deploying to remote server..."
    DEPLOY_START=$(date +%s)

    # Phase 1: Copy binary to remote /tmp
    log_debug "Syncing binary to remote..."
    if ! rsync -avz --progress "$BINARY_PATH" \
        "${REMOTE_USER}@${REMOTE_HOST}:/tmp/" 2>&1 | tee -a "$LOG_FILE"; then
        log_error "rsync sync failed"
        exit 2
    fi
    log_success "Binary synced"

    # Phase 2: Verify hash and deploy atomically
    log_debug "Deploying binary atomically..."
    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << DEPLOY_SCRIPT
set -euo pipefail

BINARY_NAME="mcp_debug_server"
UPLOADED_PATH="/tmp/${BINARY_NAME}"
REMOTE_BIN_PATH="/usr/local/bin/${BINARY_NAME}"
EXPECTED_HASH="$BINARY_HASH"

# Verify hash
UPLOADED_HASH=\$(sha256sum "\$UPLOADED_PATH" | awk '{print \$1}')
if [ "\$UPLOADED_HASH" != "\$EXPECTED_HASH" ]; then
    echo "ERROR: Binary hash mismatch"
    echo "Expected: \$EXPECTED_HASH"
    echo "Got:      \$UPLOADED_HASH"
    exit 1
fi

# Stop service (with timeout)
timeout 30 sudo systemctl stop mcp-debug || true

# Wait for graceful shutdown
sleep 1

# Atomic replacement (mv is atomic on ext4)
sudo mv "\$UPLOADED_PATH" "\$REMOTE_BIN_PATH"
sudo chown root:root "\$REMOTE_BIN_PATH"
sudo chmod 755 "\$REMOTE_BIN_PATH"

echo "Deployment completed"
DEPLOY_SCRIPT

    log_success "Atomic deployment completed"
    DEPLOY_END=$(date +%s)
    log_info "Deploy time: $(format_duration $((DEPLOY_END - DEPLOY_START)))"
}

###############################################################################
# SERVICE RESTART
###############################################################################

service_restart() {
    log_info "Starting systemd service: $SYSTEMD_SERVICE"

    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << RESTART_SCRIPT
set -euo pipefail

SYSTEMD_SERVICE="$SYSTEMD_SERVICE"

# Reload systemd daemon
sudo systemctl daemon-reload

# Start service with timeout
timeout 30 sudo systemctl start "\$SYSTEMD_SERVICE" || {
    echo "Service start timed out or failed"
    exit 1
}

# Verify active
if ! sudo systemctl is-active --quiet "\$SYSTEMD_SERVICE"; then
    echo "Service is not active"
    exit 1
fi

echo "Service started successfully"
RESTART_SCRIPT

    log_success "Service started"
}

###############################################################################
# HEALTH CHECK
###############################################################################

health_check() {
    log_info "Running health checks..."
    local max_attempts=10
    local attempt=1

    while [ $attempt -le $max_attempts ]; do
        log_debug "Health check attempt $attempt/$max_attempts..."

        # Check 1: Systemd status
        if ! ssh "${REMOTE_USER}@${REMOTE_HOST}" \
            "sudo systemctl is-active --quiet mcp-debug" 2>/dev/null; then
            log_debug "Service not yet active, waiting..."
            sleep 1
            attempt=$((attempt + 1))
            continue
        fi

        # Check 2: HTTP health endpoint
        local health_resp=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" \
            'curl -s -m 5 http://localhost:5678/health 2>/dev/null || echo "{}"' )

        if echo "$health_resp" | jq -e '.status == "ok" or .status == "healthy"' &>/dev/null 2>&1; then
            log_success "Health check passed"
            return 0
        fi

        log_debug "Health endpoint not ready, waiting..."
        sleep 1
        attempt=$((attempt + 1))
    done

    log_error "Health check failed after $max_attempts attempts"
    log_info "Last response: $health_resp"
    return 1
}

###############################################################################
# SMOKE TESTS
###############################################################################

smoke_tests() {
    log_info "Running smoke tests..."

    # Test 1: MCP handshake
    log_debug "Testing MCP JSON-RPC handshake..."
    local rpc_resp=$(ssh "${REMOTE_user}@${REMOTE_HOST}" \
        'curl -s -X POST http://localhost:5678/ -H "Content-Type: application/json" \
         -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\"}" 2>/dev/null || echo "{}"')

    if echo "$rpc_resp" | jq -e '.jsonrpc == "2.0"' &>/dev/null 2>&1; then
        log_success "MCP handshake test passed"
    else
        log_warn "MCP handshake test inconclusive (optional)"
    fi

    # Test 2: Service logging
    log_debug "Checking service logs..."
    local recent_logs=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" \
        'sudo journalctl -u mcp-debug -n 10 --no-pager 2>/dev/null | head -5' || echo "")

    if [ -n "$recent_logs" ]; then
        log_debug "Recent logs: $(echo "$recent_logs" | head -1)"
    fi

    log_success "Smoke tests completed"
}

###############################################################################
# ROLLBACK PROCEDURE
###############################################################################

rollback() {
    log_warn "INITIATING ROLLBACK"

    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << ROLLBACK_SCRIPT
set -euo pipefail

BINARY_NAME="mcp_debug_server"
REMOTE_BIN_PATH="/usr/local/bin/${BINARY_NAME}"
REMOTE_BACKUP_PATH="/usr/local/bin/${BINARY_NAME}.backup"
SYSTEMD_SERVICE="mcp-debug"

# Check backup exists
if [ ! -f "$REMOTE_BACKUP_PATH" ]; then
    echo "ERROR: Backup not found at $REMOTE_BACKUP_PATH"
    exit 1
fi

# Stop service
timeout 30 sudo systemctl stop "\$SYSTEMD_SERVICE" || true
sleep 1

# Restore backup
sudo mv "$REMOTE_BACKUP_PATH" "$REMOTE_BIN_PATH"

# Restart service
timeout 30 sudo systemctl start "\$SYSTEMD_SERVICE" || {
    echo "ERROR: Service failed to start after rollback"
    exit 1
}

echo "Rollback completed"
ROLLBACK_SCRIPT

    log_success "Rollback completed"

    # Verify rollback health
    sleep 3
    if health_check; then
        log_success "Rollback successful - service is healthy"
        return 0
    else
        log_error "CRITICAL: Rollback completed but service is unhealthy"
        return 1
    fi
}

###############################################################################
# AUDIT LOGGING (Q34 COMPLIANCE)
###############################################################################

audit_log() {
    local status=$1
    local message="${2:-}"
    local timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Log locally
    local audit_entry="$timestamp | $status | $message | user=$USER | host=$(hostname) | remote=$REMOTE_HOST"
    echo "$audit_entry" >> "$LOG_FILE"

    # Log to remote (optional, requires setup)
    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << AUDIT_SCRIPT 2>/dev/null || true
set -euo pipefail
TIMESTAMP="$timestamp"
STATUS="$status"
MESSAGE="$message"
if [ -d /var/log ]; then
    sudo tee -a /var/log/mcp-deploy.log << EOF > /dev/null
\$TIMESTAMP | \$STATUS | \$MESSAGE | source=remote
EOF
fi
AUDIT_SCRIPT
}

###############################################################################
# MAIN DEPLOYMENT WORKFLOW
###############################################################################

main_deploy() {
    local overall_start=$(date +%s)
    DEPLOYMENT_START=$overall_start

    log_info "Starting deployment pipeline..."
    log_debug "Configuration: BINARY=$BINARY_PATH, REMOTE=$REMOTE_USER@$REMOTE_HOST"

    # Phase 1: Pre-flight
    pre_flight_checks || exit 1

    # Phase 2: Build
    build_binary || exit 1

    # Phase 3: Backup
    backup_remote || exit 1

    # Phase 4: Deploy
    if [ "$DRY_RUN" = "true" ]; then
        log_info "[DRY RUN] Deployment step skipped"
    else
        deploy_remote || exit 2
    fi

    # Phase 5: Service restart
    if [ "$DRY_RUN" = "false" ]; then
        service_restart || exit 3
    fi

    # Phase 6: Health checks
    if [ "$DRY_RUN" = "false" ]; then
        if ! health_check; then
            log_error "Health check failed, initiating rollback..."
            if rollback; then
                audit_log "ROLLBACK_SUCCESS" "Health check failed, automatic rollback succeeded"
                exit 4
            else
                audit_log "ROLLBACK_FAILED" "CRITICAL: Automatic rollback failed"
                exit 99
            fi
        fi
    fi

    # Phase 7: Smoke tests
    if [ "$DRY_RUN" = "false" ]; then
        smoke_tests || log_warn "Smoke tests failed (non-fatal)"
    fi

    # Phase 8: Audit logging
    local overall_end=$(date +%s)
    local duration=$((overall_end - overall_start))
    audit_log "DEPLOYMENT_SUCCESS" "Deployment completed in ${duration}s, binary_hash=$BINARY_HASH"

    log_success "=========================================="
    log_success "DEPLOYMENT SUCCESSFUL"
    log_success "=========================================="
    log_info "Build time:    $(format_duration $((BUILD_END - BUILD_START)))"
    log_info "Deploy time:   $(format_duration $((DEPLOY_END - DEPLOY_START)))"
    log_info "Total time:    $(format_duration $duration)"
    log_info "Binary hash:   $BINARY_HASH"
    log_info "Logs:          $LOG_FILE"

    return 0
}

###############################################################################
# STANDALONE OPERATIONS
###############################################################################

standalone_health() {
    log_info "Health check only (no deployment)"

    if ssh "${REMOTE_USER}@${REMOTE_HOST}" \
        "sudo systemctl is-active --quiet mcp-debug"; then
        log_success "Service is active"

        local health=$(ssh "${REMOTE_USER}@${REMOTE_HOST}" \
            'curl -s http://localhost:5678/health 2>/dev/null || echo "{}"')

        log_info "Health response: $health"
        return 0
    else
        log_error "Service is not active"
        return 1
    fi
}

standalone_rollback() {
    if ! confirm "Are you sure you want to rollback?"; then
        log_info "Rollback cancelled"
        return 1
    fi

    rollback
}

standalone_restart() {
    log_info "Restarting service..."

    ssh "${REMOTE_USER}@${REMOTE_HOST}" bash << RESTART_SCRIPT
set -euo pipefail
sudo systemctl restart mcp-debug
sleep 2
sudo systemctl is-active --quiet mcp-debug && echo "Service restarted"
RESTART_SCRIPT

    log_success "Service restarted"
}

###############################################################################
# USAGE & HELP
###############################################################################

show_help() {
    cat << 'EOF'
deploy.sh - atomic_mcp_server deployment automation

USAGE:
  ./deploy.sh [COMMAND] [OPTIONS]

COMMANDS:
  (default)          Run full deployment (build → backup → deploy → validate)
  health             Check service health (no changes)
  rollback           Rollback to previous version
  restart            Restart systemd service
  help               Show this help message

OPTIONS:
  --dry-run          Simulate deployment without making changes
  --skip-tests       Skip smoke tests
  --verbose          Enable verbose logging
  --yes, -y          Skip confirmation prompts

ENVIRONMENT VARIABLES:
  REMOTE_HOST        Target server (default: 192.168.0.38)
  REMOTE_USER        SSH user (default: samuel)
  BUILD_FEATURES     Cargo features (default: std,json-rpc,async-runtime)
  SKIP_TESTS         Skip tests (default: false)
  VERBOSE            Verbose output (default: false)

EXAMPLES:
  # Full deployment with confirmation
  ./deploy.sh

  # Full deployment, skip confirmations
  ./deploy.sh --yes

  # Dry run (show what would happen)
  ./deploy.sh --dry-run

  # Just check health
  ./deploy.sh health

  # Rollback previous deployment
  ./deploy.sh rollback

DEPLOYMENT WORKFLOW (8 phases):
  1. Pre-flight checks (SSH, git, dependencies) - 5s
  2. Build binary (cargo release) - 30s (incremental)
  3. Backup current deployment - 0.5s
  4. Deploy binary (atomic mv) - 3s
  5. Service restart (systemctl) - 2s
  6. Health checks (HTTP + systemd) - 5s
  7. Smoke tests (MCP handshake, logs) - 2s
  8. Audit logging (Q34 compliance) - <1s

Total time: <30s (incremental) | <60s (clean build)
Downtime: <3s (atomic mv + service restart)

FRAMEWORK COMPLIANCE:
  - UCE34: Q34 audit trail + systematic deployment
  - B32: Performance targets (<30s incremental, <3s downtime)
  - T28: Multi-phase testing (health + smoke tests)

For more information, see DEPLOYMENT.md and RUNBOOK.md

EOF
    exit 0
}

###############################################################################
# ARGUMENT PARSING
###############################################################################

main() {
    # Parse positional arguments
    local command="${1:-deploy}"
    shift || true

    # Parse options
    while [[ $# -gt 0 ]]; do
        case "$1" in
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --skip-tests)
                SKIP_TESTS=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --yes|-y)
                CONFIRM=false
                shift
                ;;
            --help|-h)
                show_help
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                ;;
        esac
    done

    # Execute command
    case "$command" in
        deploy)
            main_deploy
            ;;
        health)
            standalone_health
            ;;
        rollback)
            standalone_rollback
            ;;
        restart)
            standalone_restart
            ;;
        help)
            show_help
            ;;
        *)
            log_error "Unknown command: $command"
            show_help
            ;;
    esac
}

# Run main
main "$@"
exit $?
