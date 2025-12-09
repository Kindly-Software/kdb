#!/bin/bash
# Monitor atomic_capsule HTTP server health and metrics
# Framework: UCE34 Q33 (verification), Q34 (auditability)
# ASSUM: #ASSUME_LOCALHOST_ACCESSIBLE (health checks use localhost)

set -e

# Configuration
SERVER_HEALTH="http://localhost:443/health"
SERVER_READY="http://localhost:443/ready"
SERVER_METRICS="http://localhost:443/metrics"
TIMEOUT=5
LOG_FILE="/home/samuel/Primitives/logs/monitor.log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Ensure log directory exists
mkdir -p "$(dirname "$LOG_FILE")"

# Function to log with timestamp
log() {
    local level=$1
    shift
    local message="$@"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    echo "[${timestamp}] [${level}] ${message}" >> "$LOG_FILE"
    echo -e "${message}"
}

# Function to log errors
log_error() {
    log "ERROR" "$@"
}

# Function to log info
log_info() {
    log "INFO" "$@"
}

# Function to log warning
log_warn() {
    log "WARN" "$@"
}

echo "========================================"
echo "Atomic Capsule HTTP Server Monitor"
echo "Started: $(date '+%Y-%m-%d %H:%M:%S')"
echo "========================================"

# 1. Health Check (Liveness)
echo ""
echo "🏥 Checking health (liveness)..."
HEALTH_STATUS=$(curl -s -w "%{http_code}" -o /tmp/health_response.json --connect-timeout $TIMEOUT "$SERVER_HEALTH" 2>/dev/null || echo "000")

if [ "$HEALTH_STATUS" = "200" ]; then
    UPTIME=$(jq -r '.uptime_seconds // "unknown"' /tmp/health_response.json 2>/dev/null || echo "unknown")
    VERSION=$(jq -r '.version // "unknown"' /tmp/health_response.json 2>/dev/null || echo "unknown")
    echo -e "${GREEN}✅ Health: OK${NC} (uptime: ${UPTIME}s, version: ${VERSION})"
    log_info "Health check passed (HTTP $HEALTH_STATUS)"
else
    echo -e "${RED}❌ Health: FAILED${NC} (HTTP $HEALTH_STATUS)"
    log_error "Health check failed with HTTP status $HEALTH_STATUS"
fi

# 2. Readiness Check
echo ""
echo "🚦 Checking readiness..."
READY_STATUS=$(curl -s -w "%{http_code}" -o /tmp/ready_response.json --connect-timeout $TIMEOUT "$SERVER_READY" 2>/dev/null || echo "000")

if [ "$READY_STATUS" = "200" ]; then
    TLS=$(jq -r '.tls // "unknown"' /tmp/ready_response.json 2>/dev/null || echo "unknown")
    CIRCUIT=$(jq -r '.circuit_breaker // "unknown"' /tmp/ready_response.json 2>/dev/null || echo "unknown")
    CONNECTIONS=$(jq -r '.connections // "unknown"' /tmp/ready_response.json 2>/dev/null || echo "unknown")
    echo -e "${GREEN}✅ Ready: OK${NC} (TLS: $TLS, Circuit: $CIRCUIT, Connections: $CONNECTIONS)"
    log_info "Readiness check passed (HTTP $READY_STATUS)"
else
    echo -e "${YELLOW}⚠️  Ready: NOT READY${NC} (HTTP $READY_STATUS)"
    log_warn "Readiness check returned HTTP status $READY_STATUS"
fi

# 3. Metrics Collection
echo ""
echo "📊 Collecting metrics..."
METRICS=$(curl -s --connect-timeout $TIMEOUT "$SERVER_METRICS" 2>/dev/null || echo "")

if [ -n "$METRICS" ]; then
    echo -e "${GREEN}✅ Metrics: Available${NC}"
    log_info "Metrics endpoint accessible"

    # Parse key metrics (Prometheus format)
    REQUESTS=$(echo "$METRICS" | grep "^http_requests_total" | awk '{print $2}' | head -1 || echo "0")
    ERRORS=$(echo "$METRICS" | grep "^http_errors_total" | awk '{print $2}' | head -1 || echo "0")
    CIRCUIT=$(echo "$METRICS" | grep "^circuit_breaker_state" | awk '{print $2}' | head -1 || echo "0")

    echo "  📈 Total requests: ${REQUESTS:-0}"
    echo "  ⚠️  Total errors: ${ERRORS:-0}"
    echo "  🔌 Circuit breaker state: ${CIRCUIT:-unknown} (0=closed, 1=open)"

    # Calculate error rate
    if [ "$REQUESTS" != "0" ] && [ "$REQUESTS" != "" ]; then
        ERROR_RATE=$(echo "scale=2; ($ERRORS / $REQUESTS) * 100" | bc 2>/dev/null || echo "N/A")
        echo "  📉 Error rate: ${ERROR_RATE}%"
        log_info "Metrics: requests=$REQUESTS, errors=$ERRORS, error_rate=${ERROR_RATE}%"

        # Alert if error rate > 5%
        if [ "$ERROR_RATE" != "N/A" ]; then
            ERROR_RATE_INT=$(echo "$ERROR_RATE" | cut -d'.' -f1)
            if [ "$ERROR_RATE_INT" -gt 5 ]; then
                echo -e "${YELLOW}⚠️  High error rate: ${ERROR_RATE}%${NC}"
                log_warn "High error rate detected: ${ERROR_RATE}%"
            fi
        fi
    fi

    # Check circuit breaker state
    if [ "$CIRCUIT" = "1" ]; then
        echo -e "${YELLOW}⚠️  Circuit breaker is OPEN${NC}"
        log_warn "Circuit breaker is open"
    fi
else
    echo -e "${RED}❌ Metrics: UNAVAILABLE${NC}"
    log_error "Metrics endpoint not accessible"
fi

# 4. System Resources
echo ""
echo "💻 System resources..."

# CPU usage
CPU_IDLE=$(top -bn1 2>/dev/null | grep "Cpu(s)" | awk '{print $8}' | cut -d'%' -f1)
if [ -n "$CPU_IDLE" ]; then
    CPU_USED=$(echo "100 - $CPU_IDLE" | bc 2>/dev/null || echo "unknown")
    echo "  CPU usage: ${CPU_USED}%"

    if (( $(echo "$CPU_USED > 80" | bc -l 2>/dev/null) )); then
        echo -e "${YELLOW}⚠️  High CPU usage!${NC}"
        log_warn "High CPU usage: ${CPU_USED}%"
    fi
else
    echo "  CPU usage: unable to determine"
fi

# Memory usage
MEM_INFO=$(free -h 2>/dev/null | grep Mem)
if [ -n "$MEM_INFO" ]; then
    MEM_TOTAL=$(echo "$MEM_INFO" | awk '{print $2}')
    MEM_USED=$(echo "$MEM_INFO" | awk '{print $3}')
    MEM_PERCENT=$(echo "$MEM_INFO" | awk '{printf "%.1f", $3/$2 * 100}')
    echo "  Memory: ${MEM_USED} / ${MEM_TOTAL} (${MEM_PERCENT}%)"

    if (( $(echo "$MEM_PERCENT > 90" | bc -l 2>/dev/null) )); then
        echo -e "${RED}❌ High memory usage!${NC}"
        log_error "High memory usage: ${MEM_PERCENT}%"
    fi
else
    echo "  Memory: unable to determine"
fi

# Disk usage
DISK_USAGE=$(df -h /home/samuel 2>/dev/null | tail -1 | awk '{print $5}' | cut -d'%' -f1)
if [ -n "$DISK_USAGE" ]; then
    echo "  Disk usage: ${DISK_USAGE}%"

    if [ "$DISK_USAGE" -gt 85 ]; then
        echo -e "${YELLOW}⚠️  High disk usage!${NC}"
        log_warn "High disk usage: ${DISK_USAGE}%"
    fi
else
    echo "  Disk usage: unable to determine"
fi

# 5. Service Status
echo ""
echo "🔧 Service status..."

if systemctl is-active --quiet atomic-http-server 2>/dev/null; then
    echo -e "${GREEN}✅ Service: Running${NC}"
    log_info "Service is running"

    # Get service uptime
    SERVICE_STATUS=$(systemctl status atomic-http-server 2>/dev/null | grep "Active:" || echo "")
    echo "  Status: $SERVICE_STATUS"
else
    echo -e "${RED}❌ Service: STOPPED${NC}"
    log_error "Service is not running"
fi

# 6. Network Connections
echo ""
echo "🌐 Network connections..."
LISTEN_COUNT=$(netstat -tnl 2>/dev/null | grep LISTEN | wc -l || echo "unknown")
ESTABLISHED=$(netstat -tn 2>/dev/null | grep ESTABLISHED | wc -l || echo "unknown")
echo "  Listening ports: $LISTEN_COUNT"
echo "  Established connections: $ESTABLISHED"
log_info "Network: listen=$LISTEN_COUNT, established=$ESTABLISHED"

# 7. Summary
echo ""
echo "========================================"
echo "✅ Monitoring check complete"
echo "Timestamp: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Log file: $LOG_FILE"
echo "========================================"

# Clean up temp files
rm -f /tmp/health_response.json /tmp/ready_response.json

# Exit with appropriate code
if [ "$HEALTH_STATUS" = "200" ] && [ "$READY_STATUS" = "200" ]; then
    exit 0
else
    exit 1
fi
