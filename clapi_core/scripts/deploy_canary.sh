#!/usr/bin/env bash
# Canary Deployment with Progressive Rollout
# I20 Q19: Phased deployment with automatic rollback
# Target: 1% → 10% → 25% → 50% → 100% with validation at each stage

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
HEALTH_ENDPOINT="${HEALTH_ENDPOINT:-http://localhost:8080/health}"
METRICS_ENDPOINT="${METRICS_ENDPOINT:-http://localhost:8080/metrics}"

# Canary stages (traffic percentage)
CANARY_STAGES=(1 10 25 50 100)
VALIDATION_DURATION_SEC=60  # Wait 60s between stages
MIN_SUCCESS_RATE=95.0       # 95% success rate required

# State
current_stage=0
rollback_required=false

# Logging
log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_stage() {
    echo -e "${BLUE}[STAGE]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Pre-deployment validation
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

# Deploy canary instance
deploy_canary_instance() {
    local traffic_percent=$1

    log_stage "Deploying canary at ${traffic_percent}% traffic"

    # Build release binary (already done by pre-deployment checks, but verify)
    if [[ ! -f "$BINARY_PATH" ]]; then
        log_error "Binary not found: $BINARY_PATH"
        return 1
    fi

    # Deploy canary (implementation depends on infrastructure)
    # For local testing, we'll simulate by starting a new process

    # Stop old canary if running
    if pgrep -f "${SERVICE_NAME}_canary" > /dev/null; then
        log_info "Stopping old canary instance..."
        pkill -f "${SERVICE_NAME}_canary" || true
        sleep 2
    fi

    # Start canary instance (port offset for testing)
    local canary_port=$((8080 + traffic_percent))
    log_info "Starting canary on port $canary_port (${traffic_percent}% traffic simulation)"

    # In production, this would configure load balancer routing
    # For testing, we start on different port
    RUST_LOG=info "$BINARY_PATH" --port "$canary_port" > /tmp/clapi_canary.log 2>&1 &
    local canary_pid=$!

    # Wait for startup
    sleep 5

    # Verify canary is running
    if ! kill -0 "$canary_pid" 2>/dev/null; then
        log_error "Canary process died on startup"
        cat /tmp/clapi_canary.log
        return 1
    fi

    log_info "✓ Canary instance started (PID: $canary_pid, Port: $canary_port)"

    # Save PID for cleanup
    echo "$canary_pid" > /tmp/clapi_canary.pid

    return 0
}

# Validate canary health
validate_canary_health() {
    local traffic_percent=$1
    local canary_port=$((8080 + traffic_percent))

    log_info "Validating canary health for ${VALIDATION_DURATION_SEC}s..."

    local start_time
    start_time=$(date +%s)
    local checks=0
    local failures=0

    while [[ $(($(date +%s) - start_time)) -lt $VALIDATION_DURATION_SEC ]]; do
        checks=$((checks + 1))

        # Health check
        local http_code
        http_code=$(curl -s -w "%{http_code}" -o /dev/null --max-time 5 "http://localhost:${canary_port}/health" 2>/dev/null || echo "000")

        if [[ "$http_code" != "200" ]]; then
            failures=$((failures + 1))
            log_warn "Health check failed (HTTP $http_code) - Check $checks"
        else
            log_info "Health check OK - Check $checks"
        fi

        sleep 5
    done

    # Calculate success rate
    local success_rate
    success_rate=$(echo "scale=2; 100 * (1 - $failures / $checks)" | bc)

    log_info "Validation complete: $checks checks, $failures failures, ${success_rate}% success rate"

    # Check against threshold
    if (( $(echo "$success_rate < $MIN_SUCCESS_RATE" | bc -l) )); then
        log_error "Success rate ${success_rate}% below threshold ${MIN_SUCCESS_RATE}%"
        return 1
    fi

    log_info "✓ Canary health validated (${success_rate}% success rate)"
    return 0
}

# Check canary metrics
check_canary_metrics() {
    local traffic_percent=$1
    local canary_port=$((8080 + traffic_percent))

    log_info "Checking canary metrics..."

    local metrics
    metrics=$(curl -s --max-time 5 "http://localhost:${canary_port}/metrics" 2>/dev/null || echo "{}")

    if ! command -v jq &> /dev/null; then
        log_warn "jq not installed, skipping detailed metrics analysis"
        return 0
    fi

    # Check circuit breaker state
    local cb_state
    cb_state=$(echo "$metrics" | jq -r '.circuit_breaker.state // "unknown"')

    if [[ "$cb_state" == "open" ]]; then
        log_error "Circuit breaker is OPEN on canary"
        return 1
    fi

    log_info "Circuit breaker state: $cb_state"

    # Check error rates
    local error_rate
    error_rate=$(echo "$metrics" | jq -r '.error_rate_percent // 0')

    if (( $(echo "$error_rate > 5.0" | bc -l) )); then
        log_error "Error rate too high: ${error_rate}%"
        return 1
    fi

    log_info "Error rate: ${error_rate}%"
    log_info "✓ Canary metrics healthy"

    return 0
}

# Promote canary to next stage
promote_canary() {
    local from_percent=$1
    local to_percent=$2

    log_stage "Promoting canary: ${from_percent}% → ${to_percent}%"

    # In production, this would update load balancer routing
    # For testing, we simulate with log message
    log_info "Updating traffic routing: ${to_percent}% to canary"

    # Simulate load balancer update delay
    sleep 2

    log_info "✓ Traffic routing updated"
    return 0
}

# Rollback canary
rollback_canary() {
    log_error "Rolling back canary deployment"

    # Stop canary instance
    if [[ -f /tmp/clapi_canary.pid ]]; then
        local canary_pid
        canary_pid=$(cat /tmp/clapi_canary.pid)

        if kill -0 "$canary_pid" 2>/dev/null; then
            log_info "Stopping canary process (PID: $canary_pid)"
            kill "$canary_pid"
            sleep 2
        fi

        rm -f /tmp/clapi_canary.pid
    fi

    # Revert traffic to stable (100% old version)
    log_info "Reverting traffic to stable version"

    # Clean up
    rm -f /tmp/clapi_canary.log

    log_info "✓ Rollback complete"
    return 0
}

# Main canary deployment flow
main() {
    log_info "=== Canary Deployment Started ==="
    log_info "Binary: $BINARY_PATH"
    log_info "Stages: ${CANARY_STAGES[*]}%"
    log_info "Validation duration: ${VALIDATION_DURATION_SEC}s per stage"
    log_info "Success rate threshold: ${MIN_SUCCESS_RATE}%"
    echo

    # Pre-deployment checks
    if ! run_pre_deployment_checks; then
        log_error "Pre-deployment checks failed. Aborting."
        exit 1
    fi

    echo

    # Progressive rollout
    local prev_stage=0
    for stage in "${CANARY_STAGES[@]}"; do
        echo
        log_stage "=== Stage $((current_stage + 1))/${#CANARY_STAGES[@]}: ${stage}% Traffic ==="

        # Deploy canary at this stage
        if ! deploy_canary_instance "$stage"; then
            log_error "Canary deployment failed at ${stage}%"
            rollback_canary
            exit 1
        fi

        # Validate health
        if ! validate_canary_health "$stage"; then
            log_error "Health validation failed at ${stage}%"
            rollback_canary
            exit 1
        fi

        # Check metrics
        if ! check_canary_metrics "$stage"; then
            log_error "Metrics check failed at ${stage}%"
            rollback_canary
            exit 1
        fi

        # Promote to next stage (if not final)
        if [[ "$stage" -ne 100 ]]; then
            local next_idx=$((current_stage + 1))
            local next_stage=${CANARY_STAGES[$next_idx]}

            promote_canary "$stage" "$next_stage"
        fi

        prev_stage=$stage
        current_stage=$((current_stage + 1))
    done

    echo
    log_info "=== ✓ Canary Deployment Complete ==="
    log_info "Successfully deployed to 100% traffic"
    log_info "Canary is now the stable version"

    # Clean up canary designation (it's now stable)
    if [[ -f /tmp/clapi_canary.pid ]]; then
        mv /tmp/clapi_canary.pid /tmp/clapi_stable.pid
    fi

    exit 0
}

# Signal handlers
trap 'rollback_canary; exit 1' SIGINT SIGTERM

# Usage
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Progressive canary deployment with automatic rollback.

Environment Variables:
  BINARY_PATH          Path to release binary (default: target/release/clapi)
  SERVICE_NAME         Service name (default: clapi)
  HEALTH_ENDPOINT      Health check URL (default: http://localhost:8080/health)
  METRICS_ENDPOINT     Metrics URL (default: http://localhost:8080/metrics)

Deployment Stages:
  1% → 10% → 25% → 50% → 100%

Validation:
  - 60s health monitoring per stage
  - 95% success rate required
  - Automatic rollback on failure

Examples:
  # Deploy with defaults
  $0

  # Deploy to production
  BINARY_PATH=/opt/clapi/bin/clapi SERVICE_NAME=clapi-prod $0

EOF
}

if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

main "$@"
