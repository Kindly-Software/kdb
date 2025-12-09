#!/usr/bin/env bash
# Blue-Green Deployment for Zero-Downtime
# I20 Q19: Zero-downtime deployment with instant rollback
# Target: Atomic switch, <1s downtime, instant rollback capability

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

BINARY_PATH="${BINARY_PATH:-target/release/clapi}"
SERVICE_NAME="${SERVICE_NAME:-clapi}"
BLUE_PORT="${BLUE_PORT:-8080}"
GREEN_PORT="${GREEN_PORT:-8081}"
HEALTH_CHECK_TIMEOUT="${HEALTH_CHECK_TIMEOUT:-30}"
VALIDATION_CHECKS="${VALIDATION_CHECKS:-10}"

# State files
STATE_DIR="/tmp/clapi_deployment"
mkdir -p "$STATE_DIR"

ACTIVE_COLOR_FILE="$STATE_DIR/active_color"
BLUE_PID_FILE="$STATE_DIR/blue.pid"
GREEN_PID_FILE="$STATE_DIR/green.pid"

# Logging
log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_blue() {
    echo -e "${BLUE}[BLUE]${NC} $*"
}

log_green() {
    echo -e "${CYAN}[GREEN]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Get current active color
get_active_color() {
    if [[ -f "$ACTIVE_COLOR_FILE" ]]; then
        cat "$ACTIVE_COLOR_FILE"
    else
        echo "blue"  # Default to blue
    fi
}

# Set active color
set_active_color() {
    local color=$1
    echo "$color" > "$ACTIVE_COLOR_FILE"
    log_info "Active color: $color"
}

# Get inactive color
get_inactive_color() {
    local active
    active=$(get_active_color)
    if [[ "$active" == "blue" ]]; then
        echo "green"
    else
        echo "blue"
    fi
}

# Get port for color
get_port() {
    local color=$1
    if [[ "$color" == "blue" ]]; then
        echo "$BLUE_PORT"
    else
        echo "$GREEN_PORT"
    fi
}

# Get PID file for color
get_pid_file() {
    local color=$1
    if [[ "$color" == "blue" ]]; then
        echo "$BLUE_PID_FILE"
    else
        echo "$GREEN_PID_FILE"
    fi
}

# Check if instance is running
is_running() {
    local color=$1
    local pid_file
    pid_file=$(get_pid_file "$color")

    if [[ ! -f "$pid_file" ]]; then
        return 1
    fi

    local pid
    pid=$(cat "$pid_file")

    if ! kill -0 "$pid" 2>/dev/null; then
        return 1
    fi

    return 0
}

# Stop instance
stop_instance() {
    local color=$1
    local pid_file
    pid_file=$(get_pid_file "$color")

    if [[ ! -f "$pid_file" ]]; then
        log_warn "$color instance not running (no PID file)"
        return 0
    fi

    local pid
    pid=$(cat "$pid_file")

    if ! kill -0 "$pid" 2>/dev/null; then
        log_warn "$color instance not running (stale PID)"
        rm -f "$pid_file"
        return 0
    fi

    log_info "Stopping $color instance (PID: $pid)"
    kill "$pid"

    # Wait for graceful shutdown (max 10s)
    local waited=0
    while kill -0 "$pid" 2>/dev/null && [[ $waited -lt 10 ]]; do
        sleep 1
        waited=$((waited + 1))
    done

    # Force kill if still running
    if kill -0 "$pid" 2>/dev/null; then
        log_warn "Force killing $color instance"
        kill -9 "$pid"
        sleep 1
    fi

    rm -f "$pid_file"
    log_info "✓ $color instance stopped"
}

# Start instance
start_instance() {
    local color=$1
    local port
    port=$(get_port "$color")
    local pid_file
    pid_file=$(get_pid_file "$color")

    log_info "Starting $color instance on port $port"

    # Verify binary exists
    if [[ ! -f "$BINARY_PATH" ]]; then
        log_error "Binary not found: $BINARY_PATH"
        return 1
    fi

    # Start instance
    local log_file="$STATE_DIR/${color}.log"
    RUST_LOG=info "$BINARY_PATH" --port "$port" > "$log_file" 2>&1 &
    local pid=$!

    # Save PID
    echo "$pid" > "$pid_file"

    # Wait for startup
    sleep 3

    # Verify process is running
    if ! kill -0 "$pid" 2>/dev/null; then
        log_error "$color instance died on startup"
        cat "$log_file"
        return 1
    fi

    log_info "✓ $color instance started (PID: $pid, Port: $port)"
    return 0
}

# Health check
check_health() {
    local color=$1
    local port
    port=$(get_port "$color")

    local start_time
    start_time=$(date +%s)

    log_info "Checking $color instance health (timeout: ${HEALTH_CHECK_TIMEOUT}s)"

    while [[ $(($(date +%s) - start_time)) -lt $HEALTH_CHECK_TIMEOUT ]]; do
        local http_code
        http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:${port}/health" 2>/dev/null || echo "000")

        if [[ "$http_code" == "200" ]]; then
            log_info "✓ $color instance healthy"
            return 0
        fi

        sleep 1
    done

    log_error "$color instance failed health check (timeout)"
    return 1
}

# Validate instance
validate_instance() {
    local color=$1
    local port
    port=$(get_port "$color")

    log_info "Validating $color instance ($VALIDATION_CHECKS checks)"

    local failures=0
    for i in $(seq 1 "$VALIDATION_CHECKS"); do
        local http_code
        http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:${port}/health" 2>/dev/null || echo "000")

        if [[ "$http_code" != "200" ]]; then
            failures=$((failures + 1))
            log_warn "Validation check $i failed (HTTP $http_code)"
        fi

        sleep 1
    done

    local success_rate
    success_rate=$(echo "scale=2; 100 * (1 - $failures / $VALIDATION_CHECKS)" | bc)

    log_info "Validation complete: ${success_rate}% success rate"

    if [[ $failures -gt 2 ]]; then  # Allow up to 2 failures
        log_error "Too many validation failures: $failures/$VALIDATION_CHECKS"
        return 1
    fi

    log_info "✓ $color instance validated"
    return 0
}

# Switch traffic (atomic)
switch_traffic() {
    local from_color=$1
    local to_color=$2

    log_info "Switching traffic: $from_color → $to_color (ATOMIC)"

    # In production, this updates load balancer routing
    # For local testing, we simulate with active color file
    set_active_color "$to_color"

    # Simulate atomic switch delay
    sleep 1

    log_info "✓ Traffic switched to $to_color"
    return 0
}

# Pre-deployment checks
run_pre_deployment_checks() {
    log_info "Running pre-deployment checks..."

    if [[ ! -x "./scripts/pre_deployment_checks.sh" ]]; then
        log_error "Pre-deployment check script not found"
        return 1
    fi

    if ! ./scripts/pre_deployment_checks.sh; then
        log_error "Pre-deployment checks failed"
        return 1
    fi

    log_info "✓ Pre-deployment checks passed"
    return 0
}

# Main deployment flow
main() {
    log_info "=== Blue-Green Deployment Started ==="
    log_info "Binary: $BINARY_PATH"
    log_info "Blue port: $BLUE_PORT"
    log_info "Green port: $GREEN_PORT"
    echo

    # Pre-deployment checks
    if ! run_pre_deployment_checks; then
        log_error "Pre-deployment checks failed. Aborting."
        exit 1
    fi

    echo

    # Determine current state
    local active_color
    active_color=$(get_active_color)
    local inactive_color
    inactive_color=$(get_inactive_color)

    log_info "Current active: $active_color"
    log_info "Deploying to: $inactive_color"
    echo

    # Deploy to inactive color
    log_info "=== Step 1: Deploy New Version (${inactive_color}) ==="

    # Stop old inactive instance (if running)
    if is_running "$inactive_color"; then
        log_warn "$inactive_color instance already running, stopping..."
        stop_instance "$inactive_color"
    fi

    # Start new instance
    if ! start_instance "$inactive_color"; then
        log_error "Failed to start $inactive_color instance"
        exit 1
    fi

    echo

    # Health check
    log_info "=== Step 2: Health Check (${inactive_color}) ==="
    if ! check_health "$inactive_color"; then
        log_error "Health check failed"
        stop_instance "$inactive_color"
        exit 1
    fi

    echo

    # Validation
    log_info "=== Step 3: Validation (${inactive_color}) ==="
    if ! validate_instance "$inactive_color"; then
        log_error "Validation failed"
        stop_instance "$inactive_color"
        exit 1
    fi

    echo

    # Switch traffic
    log_info "=== Step 4: Switch Traffic (${active_color} → ${inactive_color}) ==="
    switch_traffic "$active_color" "$inactive_color"

    echo

    # Keep old version running for rollback
    log_info "=== Step 5: Preserve Old Version (${active_color}) ==="
    log_info "Keeping $active_color running for instant rollback"
    log_info "Run 'scripts/deploy_rollback.sh' to rollback if needed"

    echo

    log_info "=== ✓ Blue-Green Deployment Complete ==="
    log_info "Active: $inactive_color (port $(get_port "$inactive_color"))"
    log_info "Standby: $active_color (port $(get_port "$active_color"))"
    log_info ""
    log_info "Next Steps:"
    log_info "  1. Monitor new version: scripts/health_check_monitor.sh"
    log_info "  2. If stable, stop old version: stop_instance $active_color"
    log_info "  3. If issues, rollback: scripts/deploy_rollback.sh"

    exit 0
}

# Rollback command
rollback() {
    log_error "=== Rollback Initiated ==="

    local active_color
    active_color=$(get_active_color)
    local inactive_color
    inactive_color=$(get_inactive_color)

    log_info "Rolling back: $active_color → $inactive_color"

    # Switch traffic back
    switch_traffic "$active_color" "$inactive_color"

    # Stop failed version
    stop_instance "$active_color"

    log_info "✓ Rollback complete"
    exit 0
}

# Usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Blue-Green deployment with zero downtime.

Commands:
  deploy              Deploy new version (default)
  rollback            Rollback to previous version
  status              Show current deployment status

Environment Variables:
  BINARY_PATH              Path to release binary (default: target/release/clapi)
  SERVICE_NAME             Service name (default: clapi)
  BLUE_PORT                Blue instance port (default: 8080)
  GREEN_PORT               Green instance port (default: 8081)
  HEALTH_CHECK_TIMEOUT     Health check timeout in seconds (default: 30)
  VALIDATION_CHECKS        Number of validation checks (default: 10)

Deployment Steps:
  1. Deploy to inactive color (blue/green)
  2. Health check new instance
  3. Validate new instance
  4. Atomic traffic switch
  5. Keep old version for rollback

Examples:
  # Deploy new version
  $0 deploy

  # Rollback to previous version
  $0 rollback

  # Check deployment status
  $0 status

EOF
}

# Status command
show_status() {
    log_info "=== Deployment Status ==="

    local active_color
    active_color=$(get_active_color)

    echo "Active color: $active_color"
    echo

    for color in blue green; do
        local port
        port=$(get_port "$color")
        local status="STOPPED"
        local pid=""

        if is_running "$color"; then
            status="RUNNING"
            local pid_file
            pid_file=$(get_pid_file "$color")
            pid=$(cat "$pid_file")
        fi

        echo "$color instance:"
        echo "  Status: $status"
        echo "  Port: $port"
        echo "  PID: ${pid:-N/A}"

        if [[ "$status" == "RUNNING" ]]; then
            local http_code
            http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:${port}/health" 2>/dev/null || echo "000")
            echo "  Health: $([ "$http_code" == "200" ] && echo "OK" || echo "FAIL")"
        fi

        echo
    done
}

# Parse command
cmd="${1:-deploy}"

case "$cmd" in
    deploy)
        main
        ;;
    rollback)
        rollback
        ;;
    status)
        show_status
        ;;
    --help|-h)
        usage
        exit 0
        ;;
    *)
        log_error "Unknown command: $cmd"
        usage
        exit 1
        ;;
esac
