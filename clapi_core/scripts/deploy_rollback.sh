#!/usr/bin/env bash
# Automated Rollback with <5 Minute Guarantee
# I20 Q20: Fast rollback for production incidents
# Target: Complete rollback in <5 minutes (git revert + rebuild + deploy)

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

BINARY_PATH="${BINARY_PATH:-target/release/clapi}"
SERVICE_NAME="${SERVICE_NAME:-clapi}"
ROLLBACK_TIMEOUT_SEC="${ROLLBACK_TIMEOUT_SEC:-300}"  # 5 minutes
STATE_DIR="/tmp/clapi_deployment"

# Timing
ROLLBACK_START_TIME=$(date +%s)

# Logging
log_info() {
    local elapsed=$(($(date +%s) - ROLLBACK_START_TIME))
    echo -e "${GREEN}[+${elapsed}s]${NC} $*"
}

log_warn() {
    local elapsed=$(($(date +%s) - ROLLBACK_START_TIME))
    echo -e "${YELLOW}[+${elapsed}s]${NC} $*"
}

log_error() {
    local elapsed=$(($(date +%s) - ROLLBACK_START_TIME))
    echo -e "${RED}[+${elapsed}s]${NC} $*"
}

# Check elapsed time
check_timeout() {
    local elapsed=$(($(date +%s) - ROLLBACK_START_TIME))

    if [[ $elapsed -ge $ROLLBACK_TIMEOUT_SEC ]]; then
        log_error "Rollback timeout exceeded (${elapsed}s > ${ROLLBACK_TIMEOUT_SEC}s)"
        return 1
    fi

    local remaining=$((ROLLBACK_TIMEOUT_SEC - elapsed))
    log_info "Time remaining: ${remaining}s"

    return 0
}

# Git rollback (fast)
rollback_git() {
    log_info "Step 1: Git rollback"

    # Get current commit
    local current_commit
    current_commit=$(git rev-parse HEAD)
    log_info "Current commit: $current_commit"

    # Find previous deployment commit
    # Look for commits with deployment tags or recent commits
    local prev_commit
    prev_commit=$(git rev-parse HEAD~1)
    log_info "Rolling back to: $prev_commit"

    # Save current state for recovery
    local recovery_branch="rollback-recovery-$(date +%s)"
    git branch "$recovery_branch" HEAD
    log_info "Created recovery branch: $recovery_branch"

    # Rollback (hard reset to previous commit)
    if ! git reset --hard "$prev_commit"; then
        log_error "Git rollback failed"
        return 1
    fi

    log_info "✓ Git rollback complete"
    check_timeout || return 1

    return 0
}

# Fast rebuild (optimized)
fast_rebuild() {
    log_info "Step 2: Fast rebuild"

    # Use incremental compilation (keep build cache)
    export CARGO_INCREMENTAL=1

    # Build with optimizations but skip some checks for speed
    if ! cargo build --release --all-features 2>&1 | grep -E "(Compiling|Finished)" || true; then
        log_error "Fast rebuild failed"
        return 1
    fi

    # Verify binary
    if [[ ! -f "$BINARY_PATH" ]]; then
        log_error "Binary not found after rebuild"
        return 1
    fi

    log_info "✓ Rebuild complete"
    check_timeout || return 1

    return 0
}

# Stop current instance
stop_current() {
    log_info "Step 3: Stop current instance"

    # Find running processes
    local pids
    pids=$(pgrep -f "$SERVICE_NAME" || true)

    if [[ -z "$pids" ]]; then
        log_warn "No running instances found"
        return 0
    fi

    log_info "Stopping processes: $pids"

    # Graceful shutdown
    for pid in $pids; do
        if kill -0 "$pid" 2>/dev/null; then
            kill "$pid" || true
        fi
    done

    # Wait up to 10s for graceful shutdown
    local waited=0
    while pgrep -f "$SERVICE_NAME" > /dev/null && [[ $waited -lt 10 ]]; do
        sleep 1
        waited=$((waited + 1))
    done

    # Force kill if needed
    pids=$(pgrep -f "$SERVICE_NAME" || true)
    if [[ -n "$pids" ]]; then
        log_warn "Force killing: $pids"
        for pid in $pids; do
            kill -9 "$pid" 2>/dev/null || true
        done
    fi

    # Clean up PID files
    rm -f "$STATE_DIR"/*.pid 2>/dev/null || true

    log_info "✓ Current instance stopped"
    check_timeout || return 1

    return 0
}

# Start rolled-back version
start_rollback_version() {
    log_info "Step 4: Start rolled-back version"

    local port="${ROLLBACK_PORT:-8080}"
    local log_file="/tmp/clapi_rollback.log"

    # Start service
    RUST_LOG=info "$BINARY_PATH" --port "$port" > "$log_file" 2>&1 &
    local pid=$!

    # Save PID
    mkdir -p "$STATE_DIR"
    echo "$pid" > "$STATE_DIR/rollback.pid"

    log_info "Started service (PID: $pid, Port: $port)"

    # Wait for startup
    sleep 3

    # Verify process is running
    if ! kill -0 "$pid" 2>/dev/null; then
        log_error "Service died on startup"
        cat "$log_file"
        return 1
    fi

    log_info "✓ Rollback version started"
    check_timeout || return 1

    return 0
}

# Verify rollback
verify_rollback() {
    log_info "Step 5: Verify rollback"

    local port="${ROLLBACK_PORT:-8080}"
    local max_attempts=30
    local attempt=0

    while [[ $attempt -lt $max_attempts ]]; do
        local http_code
        http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:${port}/health" 2>/dev/null || echo "000")

        if [[ "$http_code" == "200" ]]; then
            log_info "✓ Health check passed"

            # Run a few more checks to ensure stability
            local failures=0
            for i in {1..5}; do
                http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:${port}/health" 2>/dev/null || echo "000")
                if [[ "$http_code" != "200" ]]; then
                    failures=$((failures + 1))
                fi
                sleep 1
            done

            if [[ $failures -eq 0 ]]; then
                log_info "✓ Rollback verified (5/5 health checks passed)"
                return 0
            else
                log_warn "Rollback unstable ($failures/5 checks failed)"
            fi
        fi

        attempt=$((attempt + 1))
        sleep 1
    done

    log_error "Rollback verification failed (timeout)"
    return 1
}

# Update deployment state
update_deployment_state() {
    log_info "Step 6: Update deployment state"

    # Update active color (if using blue-green)
    if [[ -f "$STATE_DIR/active_color" ]]; then
        local current_color
        current_color=$(cat "$STATE_DIR/active_color")

        local new_color
        if [[ "$current_color" == "blue" ]]; then
            new_color="green"
        else
            new_color="blue"
        fi

        echo "$new_color" > "$STATE_DIR/active_color"
        log_info "Updated active color: $current_color → $new_color"
    fi

    # Record rollback event
    local rollback_log="$STATE_DIR/rollback_history.log"
    local timestamp
    timestamp=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    local commit
    commit=$(git rev-parse HEAD)

    echo "$timestamp ROLLBACK commit=$commit elapsed=${ROLLBACK_ELAPSED}s" >> "$rollback_log"

    log_info "✓ Deployment state updated"
    return 0
}

# Emergency rollback (skip some checks)
emergency_rollback() {
    log_error "=== EMERGENCY ROLLBACK ==="
    log_warn "Skipping non-critical checks for speed"

    # Stop everything
    pkill -9 -f "$SERVICE_NAME" 2>/dev/null || true
    rm -f "$STATE_DIR"/*.pid 2>/dev/null || true

    # Git rollback
    git reset --hard HEAD~1 || {
        log_error "Git rollback failed"
        return 1
    }

    # Fast rebuild (no verification)
    cargo build --release --all-features || {
        log_error "Rebuild failed"
        return 1
    }

    # Start immediately
    RUST_LOG=info "$BINARY_PATH" > /tmp/clapi_emergency.log 2>&1 &
    local pid=$!
    echo "$pid" > "$STATE_DIR/emergency.pid"

    # Quick health check
    sleep 5
    local http_code
    http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 2 "http://localhost:8080/health" 2>/dev/null || echo "000")

    if [[ "$http_code" == "200" ]]; then
        log_info "✓ Emergency rollback successful"
        return 0
    else
        log_error "Emergency rollback failed health check"
        cat /tmp/clapi_emergency.log
        return 1
    fi
}

# Main rollback flow
main() {
    log_info "=== Automated Rollback Started ==="
    log_info "Target: <${ROLLBACK_TIMEOUT_SEC}s (5 minutes)"
    log_info "Project: $PROJECT_ROOT"
    echo

    # Check if we're in emergency mode
    if [[ "${EMERGENCY:-false}" == "true" ]]; then
        emergency_rollback
        exit $?
    fi

    # Standard rollback flow
    local failed=0

    rollback_git || failed=1
    [[ $failed -eq 0 ]] || { log_error "Git rollback failed"; exit 1; }

    fast_rebuild || failed=1
    [[ $failed -eq 0 ]] || { log_error "Rebuild failed"; exit 1; }

    stop_current || failed=1
    [[ $failed -eq 0 ]] || { log_error "Stop failed"; exit 1; }

    start_rollback_version || failed=1
    [[ $failed -eq 0 ]] || { log_error "Start failed"; exit 1; }

    verify_rollback || failed=1
    [[ $failed -eq 0 ]] || { log_error "Verification failed"; exit 1; }

    update_deployment_state || true  # Non-critical

    # Final timing
    ROLLBACK_ELAPSED=$(($(date +%s) - ROLLBACK_START_TIME))

    echo
    log_info "=== ✓ Rollback Complete ==="
    log_info "Total time: ${ROLLBACK_ELAPSED}s"

    if [[ $ROLLBACK_ELAPSED -lt $ROLLBACK_TIMEOUT_SEC ]]; then
        log_info "✓ Rollback completed within target (<${ROLLBACK_TIMEOUT_SEC}s)"
    else
        log_warn "⚠ Rollback exceeded target (${ROLLBACK_ELAPSED}s > ${ROLLBACK_TIMEOUT_SEC}s)"
    fi

    # Verify <5 minute guarantee
    if [[ $ROLLBACK_ELAPSED -ge 300 ]]; then
        log_error "CRITICAL: Rollback exceeded 5 minute guarantee"
        exit 1
    fi

    exit 0
}

# Usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Automated rollback with <5 minute guarantee.

Options:
  --emergency         Emergency mode (skip non-critical checks)
  --help              Show this help

Environment Variables:
  BINARY_PATH              Path to release binary (default: target/release/clapi)
  SERVICE_NAME             Service name (default: clapi)
  ROLLBACK_PORT            Port for rollback version (default: 8080)
  ROLLBACK_TIMEOUT_SEC     Rollback timeout (default: 300s / 5min)
  EMERGENCY                Emergency mode flag (default: false)

Rollback Steps:
  1. Git rollback (reset to HEAD~1)
  2. Fast rebuild (incremental compilation)
  3. Stop current instance
  4. Start rolled-back version
  5. Verify rollback
  6. Update deployment state

Target: <5 minutes total time

Examples:
  # Standard rollback
  $0

  # Emergency rollback (skip checks)
  $0 --emergency

  # Custom timeout
  ROLLBACK_TIMEOUT_SEC=180 $0

EOF
}

# Parse arguments
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

if [[ "${1:-}" == "--emergency" ]]; then
    export EMERGENCY=true
fi

main "$@"
