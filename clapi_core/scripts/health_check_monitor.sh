#!/usr/bin/env bash
# Continuous Health Check Monitoring
# I20 Q20: Production monitoring and alerting
# Target: Detect degradation within 30 seconds

set -euo pipefail

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# Configuration
HEALTH_ENDPOINT="${HEALTH_ENDPOINT:-http://localhost:8080/health}"
METRICS_ENDPOINT="${METRICS_ENDPOINT:-http://localhost:8080/metrics}"
CHECK_INTERVAL_SEC="${CHECK_INTERVAL_SEC:-10}"
ALERT_THRESHOLD_FAILURES="${ALERT_THRESHOLD_FAILURES:-3}"
LATENCY_WARN_MS="${LATENCY_WARN_MS:-100}"
LATENCY_ERROR_MS="${LATENCY_ERROR_MS:-500}"

# State
consecutive_failures=0
total_checks=0
total_failures=0
start_time=$(date +%s)

# Logging
log_info() {
    echo -e "${GREEN}[$(date +'%H:%M:%S')]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[$(date +'%H:%M:%S')]${NC} $*"
}

log_error() {
    echo -e "${RED}[$(date +'%H:%M:%S')]${NC} $*"
}

# Alert function (can be extended to PagerDuty, Slack, etc.)
send_alert() {
    local severity=$1
    local message=$2

    log_error "ALERT [$severity]: $message"

    # TODO: Integrate with alerting system
    # Example: curl -X POST https://api.pagerduty.com/incidents ...
}

# Check health endpoint
check_health() {
    local start_ns
    start_ns=$(date +%s%N)

    local response
    local http_code
    local latency_ms

    # Make request with timeout
    response=$(curl -s -w "\n%{http_code}" --max-time 5 "$HEALTH_ENDPOINT" 2>/dev/null || echo "FAILED\n000")
    http_code=$(echo "$response" | tail -1)
    local body
    body=$(echo "$response" | head -n -1)

    local end_ns
    end_ns=$(date +%s%N)
    latency_ms=$(( (end_ns - start_ns) / 1000000 ))

    total_checks=$((total_checks + 1))

    # Validate response
    if [[ "$http_code" != "200" ]]; then
        consecutive_failures=$((consecutive_failures + 1))
        total_failures=$((total_failures + 1))

        log_error "Health check FAILED (HTTP $http_code) - Latency: ${latency_ms}ms"

        if [[ $consecutive_failures -ge $ALERT_THRESHOLD_FAILURES ]]; then
            send_alert "CRITICAL" "Service unhealthy after $consecutive_failures consecutive failures"
        fi

        return 1
    fi

    # Check latency
    if [[ $latency_ms -gt $LATENCY_ERROR_MS ]]; then
        log_error "Health check SLOW (${latency_ms}ms > ${LATENCY_ERROR_MS}ms threshold)"
        send_alert "WARNING" "Health endpoint latency degraded: ${latency_ms}ms"
    elif [[ $latency_ms -gt $LATENCY_WARN_MS ]]; then
        log_warn "Health check latency elevated: ${latency_ms}ms"
    else
        log_info "Health check OK - Latency: ${latency_ms}ms"
    fi

    # Reset failure counter on success
    consecutive_failures=0

    # Parse and validate health response
    if command -v jq &> /dev/null; then
        local status
        status=$(echo "$body" | jq -r '.status // "unknown"')

        if [[ "$status" != "healthy" ]]; then
            log_warn "Service reports status: $status"
            return 1
        fi

        # Extract component statuses
        local components
        components=$(echo "$body" | jq -r '.components // {}' | jq -r 'to_entries[] | "\(.key): \(.value)"')
        if [[ -n "$components" ]]; then
            echo "  Components:"
            echo "$components" | while read -r line; do
                echo "    - $line"
            done
        fi
    fi

    return 0
}

# Check metrics endpoint
check_metrics() {
    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" --max-time 5 "$METRICS_ENDPOINT" 2>/dev/null || echo "FAILED\n000")
    http_code=$(echo "$response" | tail -1)
    local body
    body=$(echo "$response" | head -n -1)

    if [[ "$http_code" != "200" ]]; then
        log_warn "Metrics endpoint unavailable (HTTP $http_code)"
        return 1
    fi

    # Parse key metrics if jq available
    if command -v jq &> /dev/null; then
        # Circuit breaker state
        local cb_state
        cb_state=$(echo "$body" | jq -r '.circuit_breaker.state // "unknown"')

        if [[ "$cb_state" == "open" ]]; then
            log_error "Circuit breaker OPEN"
            send_alert "CRITICAL" "Circuit breaker in OPEN state"
        elif [[ "$cb_state" == "half_open" ]]; then
            log_warn "Circuit breaker HALF-OPEN (recovering)"
        fi

        # Budget utilization
        local budget_util
        budget_util=$(echo "$body" | jq -r '.budget.utilization_percent // 0')

        if (( $(echo "$budget_util > 90" | bc -l) )); then
            log_warn "Budget utilization high: ${budget_util}%"
        fi
    fi

    return 0
}

# Print statistics
print_stats() {
    local uptime=$(($(date +%s) - start_time))
    local success_rate=0

    if [[ $total_checks -gt 0 ]]; then
        success_rate=$(echo "scale=2; 100 * (1 - $total_failures / $total_checks)" | bc)
    fi

    echo
    log_info "=== Health Check Statistics ==="
    log_info "Uptime: ${uptime}s"
    log_info "Total checks: $total_checks"
    log_info "Total failures: $total_failures"
    log_info "Success rate: ${success_rate}%"
    log_info "Consecutive failures: $consecutive_failures"
    echo
}

# Signal handlers
trap 'print_stats; exit 0' SIGINT SIGTERM

# Main monitoring loop
main() {
    log_info "=== Health Check Monitor Started ==="
    log_info "Health endpoint: $HEALTH_ENDPOINT"
    log_info "Metrics endpoint: $METRICS_ENDPOINT"
    log_info "Check interval: ${CHECK_INTERVAL_SEC}s"
    log_info "Alert threshold: $ALERT_THRESHOLD_FAILURES consecutive failures"
    log_info "Latency thresholds: WARN=${LATENCY_WARN_MS}ms, ERROR=${LATENCY_ERROR_MS}ms"
    echo

    while true; do
        # Run health check
        check_health

        # Run metrics check (non-blocking)
        check_metrics || true

        # Print periodic stats (every 10 checks)
        if [[ $((total_checks % 10)) -eq 0 ]]; then
            print_stats
        fi

        # Wait for next interval
        sleep "$CHECK_INTERVAL_SEC"
    done
}

# Usage information
usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Continuous health check monitoring for clapi_core service.

Environment Variables:
  HEALTH_ENDPOINT              Health check URL (default: http://localhost:8080/health)
  METRICS_ENDPOINT             Metrics URL (default: http://localhost:8080/metrics)
  CHECK_INTERVAL_SEC           Check interval in seconds (default: 10)
  ALERT_THRESHOLD_FAILURES     Consecutive failures before alert (default: 3)
  LATENCY_WARN_MS              Warning latency threshold (default: 100)
  LATENCY_ERROR_MS             Error latency threshold (default: 500)

Examples:
  # Monitor local instance
  $0

  # Monitor remote instance with custom interval
  HEALTH_ENDPOINT=http://prod.example.com/health CHECK_INTERVAL_SEC=5 $0

  # Monitor with strict latency requirements
  LATENCY_WARN_MS=50 LATENCY_ERROR_MS=200 $0

EOF
}

# Parse arguments
if [[ "${1:-}" == "--help" ]] || [[ "${1:-}" == "-h" ]]; then
    usage
    exit 0
fi

main "$@"
