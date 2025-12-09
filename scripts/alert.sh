#!/bin/bash
# Alert system for atomic_capsule HTTP server
# Framework: UCE34 Q34 (auditability - audit trail of alerts)
# ASSUM: #ASSUME_ALERT_LOG_WRITABLE (logs can be written to disk)

set -e

# Configuration
ALERT_LOG="/home/samuel/Primitives/logs/alerts.log"
ALERT_EMAIL="${ALERT_EMAIL:-alerts@kindly.software}"
ENABLE_EMAIL="${ENABLE_EMAIL:-false}"

# Server endpoints
SERVER_HEALTH="http://localhost:443/health"
SERVER_METRICS="http://localhost:443/metrics"

# Alert thresholds
HEALTH_TIMEOUT=5
ERROR_RATE_THRESHOLD=5  # %
CPU_THRESHOLD=85        # %
MEMORY_THRESHOLD=90     # %
DISK_THRESHOLD=90       # %
CIRCUIT_BREAKER_THRESHOLD=1  # Open state

# Ensure log directory exists
mkdir -p "$(dirname "$ALERT_LOG")"

# Function to alert
alert() {
    local level=$1
    local message=$2
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')

    # Log alert with Q34 compliance (audit trail)
    echo "[${timestamp}] [${level}] ${message}" >> "$ALERT_LOG"

    # Print to stderr
    echo "[${level}] ${message}" >&2

    # Send email if enabled (requires mailutils)
    if [ "$ENABLE_EMAIL" = "true" ] && command -v mail &> /dev/null; then
        {
            echo "Alert Level: $level"
            echo "Time: $timestamp"
            echo "Message: $message"
            echo ""
            echo "Server: $(hostname)"
            echo "Logs: $ALERT_LOG"
        } | mail -s "[$level] Atomic Capsule Alert: $message" "$ALERT_EMAIL"
    fi
}

# Function to alert critical
alert_critical() {
    alert "CRITICAL" "$1"
}

# Function to alert warning
alert_warning() {
    alert "WARNING" "$1"
}

echo "Alert Monitor Started: $(date '+%Y-%m-%d %H:%M:%S')"

# 1. Check Health Endpoint
echo "Checking health endpoint..."
HEALTH=$(curl -s -w "%{http_code}" -o /tmp/health.json --connect-timeout $HEALTH_TIMEOUT "$SERVER_HEALTH" 2>/dev/null || echo "000")

if [ "$HEALTH" != "200" ]; then
    alert_critical "Health check failed (HTTP $HEALTH)"
fi

# 2. Check Metrics and Error Rate
echo "Checking metrics..."
METRICS=$(curl -s --connect-timeout $HEALTH_TIMEOUT "$SERVER_METRICS" 2>/dev/null || echo "")

if [ -z "$METRICS" ]; then
    alert_critical "Metrics endpoint unavailable"
else
    # Parse metrics
    REQUESTS=$(echo "$METRICS" | grep "^http_requests_total" | awk '{print $2}' | head -1 || echo "0")
    ERRORS=$(echo "$METRICS" | grep "^http_errors_total" | awk '{print $2}' | head -1 || echo "0")

    # Calculate error rate
    if [ "$REQUESTS" != "0" ] && [ "$REQUESTS" != "" ] && [ "$ERRORS" != "0" ]; then
        ERROR_RATE=$(echo "scale=2; ($ERRORS / $REQUESTS) * 100" | bc 2>/dev/null || echo "0")
        ERROR_RATE_INT=$(echo "$ERROR_RATE" | cut -d'.' -f1)

        if [ "$ERROR_RATE_INT" -gt "$ERROR_RATE_THRESHOLD" ]; then
            alert_warning "High error rate: ${ERROR_RATE}% (threshold: ${ERROR_RATE_THRESHOLD}%)"
        fi
    fi

    # Check circuit breaker state
    CIRCUIT=$(echo "$METRICS" | grep "^circuit_breaker_state" | awk '{print $2}' | head -1 || echo "0")
    if [ "$CIRCUIT" = "$CIRCUIT_BREAKER_THRESHOLD" ]; then
        alert_warning "Circuit breaker is OPEN (state=$CIRCUIT)"
    fi
fi

# 3. Check System Resources
echo "Checking system resources..."

# CPU
CPU_IDLE=$(top -bn1 2>/dev/null | grep "Cpu(s)" | awk '{print $8}' | cut -d'%' -f1 || echo "")
if [ -n "$CPU_IDLE" ]; then
    CPU_USED=$(echo "100 - $CPU_IDLE" | bc 2>/dev/null || echo "0")
    CPU_USED_INT=$(echo "$CPU_USED" | cut -d'.' -f1)

    if [ "$CPU_USED_INT" -gt "$CPU_THRESHOLD" ]; then
        alert_warning "High CPU usage: ${CPU_USED}% (threshold: ${CPU_THRESHOLD}%)"
    fi
fi

# Memory
MEM_INFO=$(free 2>/dev/null | grep Mem)
if [ -n "$MEM_INFO" ]; then
    MEM_USED=$(echo "$MEM_INFO" | awk '{print $3}')
    MEM_TOTAL=$(echo "$MEM_INFO" | awk '{print $2}')
    MEM_PERCENT=$(echo "scale=1; ($MEM_USED / $MEM_TOTAL) * 100" | bc 2>/dev/null || echo "0")
    MEM_PERCENT_INT=$(echo "$MEM_PERCENT" | cut -d'.' -f1)

    if [ "$MEM_PERCENT_INT" -gt "$MEMORY_THRESHOLD" ]; then
        alert_critical "High memory usage: ${MEM_PERCENT}% (threshold: ${MEMORY_THRESHOLD}%)"
    fi
fi

# Disk
DISK_USAGE=$(df /home/samuel 2>/dev/null | tail -1 | awk '{print $5}' | cut -d'%' -f1 || echo "0")
if [ "$DISK_USAGE" -gt "$DISK_THRESHOLD" ]; then
    alert_warning "High disk usage: ${DISK_USAGE}% (threshold: ${DISK_THRESHOLD}%)"
fi

# 4. Check Service Status
echo "Checking service status..."
if ! systemctl is-active --quiet atomic-http-server 2>/dev/null; then
    alert_critical "Service atomic-http-server is not running"
fi

# 5. Check Log Files
echo "Checking log files..."
if [ -f "/var/log/atomic-capsule.log" ]; then
    # Look for ERROR or PANIC in recent logs
    ERROR_COUNT=$(tail -100 /var/log/atomic-capsule.log 2>/dev/null | grep -i "ERROR\|PANIC" | wc -l || echo "0")
    if [ "$ERROR_COUNT" -gt 5 ]; then
        alert_warning "Multiple errors in logs: $ERROR_COUNT in last 100 lines"
    fi
fi

echo "Alert Monitor Completed: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Alert log: $ALERT_LOG"
