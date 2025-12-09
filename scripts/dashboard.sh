#!/bin/bash
# Real-time CLI dashboard for atomic_capsule HTTP server
# Framework: UCE34 Q33 (verification via real-time display)
# Updates every 1 second with live metrics

# Configuration
REFRESH_INTERVAL=1
SERVER_HEALTH="http://localhost:443/health"
SERVER_READY="http://localhost:443/ready"
SERVER_METRICS="http://localhost:443/metrics"
TIMEOUT=2

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

# Function to draw box
draw_box() {
    local title="$1"
    local width=60

    echo -e "${BLUE}┌─${BOLD}${title}${NC}${BLUE}$(printf '─%.0s' $(seq 1 $((width - ${#title} - 2))))┐${NC}"
}

draw_bottom() {
    local width=60
    echo -e "${BLUE}└$(printf '─%.0s' $(seq 1 $((width - 2))))┘${NC}"
}

# Main dashboard function
show_dashboard() {
    clear

    # Header
    echo -e "${BOLD}${CYAN}═════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}  Atomic Capsule HTTP Server - Real-Time Dashboard${NC}"
    echo -e "${BOLD}${CYAN}═════════════════════════════════════════════════════════${NC}"
    echo ""

    # Timestamp
    echo -e "${MAGENTA}Updated: $(date '+%Y-%m-%d %H:%M:%S')${NC}"
    echo ""

    # 1. Health Status
    draw_box " LIVENESS CHECK (Health) "
    HEALTH=$(curl -s -w "%{http_code}" -o /tmp/dashboard_health.json --connect-timeout $TIMEOUT "$SERVER_HEALTH" 2>/dev/null || echo "000")

    if [ "$HEALTH" = "200" ]; then
        UPTIME=$(jq -r '.uptime_seconds // "N/A"' /tmp/dashboard_health.json 2>/dev/null || echo "N/A")
        VERSION=$(jq -r '.version // "N/A"' /tmp/dashboard_health.json 2>/dev/null || echo "N/A")
        echo -e "  Status:      ${GREEN}✅ HEALTHY${NC}"
        echo -e "  Uptime:      ${UPTIME}s"
        echo -e "  Version:     ${VERSION}"
    else
        echo -e "  Status:      ${RED}❌ UNHEALTHY${NC} (HTTP $HEALTH)"
    fi
    draw_bottom
    echo ""

    # 2. Readiness Status
    draw_box " READINESS CHECK (Ready) "
    READY=$(curl -s -w "%{http_code}" -o /tmp/dashboard_ready.json --connect-timeout $TIMEOUT "$SERVER_READY" 2>/dev/null || echo "000")

    if [ "$READY" = "200" ]; then
        TLS=$(jq -r '.tls // "N/A"' /tmp/dashboard_ready.json 2>/dev/null || echo "N/A")
        CIRCUIT=$(jq -r '.circuit_breaker // "N/A"' /tmp/dashboard_ready.json 2>/dev/null || echo "N/A")
        CONNECTIONS=$(jq -r '.connections // "N/A"' /tmp/dashboard_ready.json 2>/dev/null || echo "N/A")
        echo -e "  Status:      ${GREEN}✅ READY${NC}"
        echo -e "  TLS:         ${TLS}"
        echo -e "  Circuit:     ${CIRCUIT}"
        echo -e "  Connections: ${CONNECTIONS}"
    else
        echo -e "  Status:      ${YELLOW}⚠️  NOT READY${NC} (HTTP $READY)"
    fi
    draw_bottom
    echo ""

    # 3. Metrics
    draw_box " PERFORMANCE METRICS "
    METRICS=$(curl -s --connect-timeout $TIMEOUT "$SERVER_METRICS" 2>/dev/null || echo "")

    if [ -n "$METRICS" ]; then
        REQUESTS=$(echo "$METRICS" | grep "^http_requests_total" | awk '{print $2}' | head -1 || echo "0")
        ERRORS=$(echo "$METRICS" | grep "^http_errors_total" | awk '{print $2}' | head -1 || echo "0")
        CIRCUIT_STATE=$(echo "$METRICS" | grep "^circuit_breaker_state" | awk '{print $2}' | head -1 || echo "0")

        # Calculate error rate
        if [ "$REQUESTS" != "0" ] && [ "$REQUESTS" != "" ]; then
            ERROR_RATE=$(echo "scale=1; ($ERRORS / $REQUESTS) * 100" | bc 2>/dev/null || echo "0")
        else
            ERROR_RATE="0"
        fi

        # Format metrics with colors
        if [ "$CIRCUIT_STATE" = "0" ]; then
            CIRCUIT_DISPLAY="${GREEN}CLOSED${NC}"
        else
            CIRCUIT_DISPLAY="${RED}OPEN${NC}"
        fi

        if (( $(echo "$ERROR_RATE > 5" | bc -l 2>/dev/null) )); then
            ERROR_DISPLAY="${RED}${ERROR_RATE}%${NC}"
        else
            ERROR_DISPLAY="${GREEN}${ERROR_RATE}%${NC}"
        fi

        echo -e "  Total Requests:  ${CYAN}${REQUESTS}${NC}"
        echo -e "  Total Errors:    ${CYAN}${ERRORS}${NC}"
        echo -e "  Error Rate:      ${ERROR_DISPLAY}"
        echo -e "  Circuit Breaker: ${CIRCUIT_DISPLAY}"

        # Parse latency percentiles if available
        P50=$(echo "$METRICS" | grep "http_request_duration_seconds_bucket.*le=\"0.05\"" | awk '{print $2}' | head -1 || echo "")
        P99=$(echo "$METRICS" | grep "http_request_duration_seconds_bucket.*le=\"0.1\"" | awk '{print $2}' | head -1 || echo "")

        if [ -n "$P50" ] || [ -n "$P99" ]; then
            echo ""
            [ -n "$P50" ] && echo -e "  P50 Latency:     ${P50} requests"
            [ -n "$P99" ] && echo -e "  P99 Latency:     ${P99} requests"
        fi
    else
        echo -e "  ${RED}❌ Metrics unavailable${NC}"
    fi
    draw_bottom
    echo ""

    # 4. System Resources
    draw_box " SYSTEM RESOURCES "

    # CPU
    CPU_IDLE=$(top -bn1 2>/dev/null | grep "Cpu(s)" | awk '{print $8}' | cut -d'%' -f1 || echo "")
    if [ -n "$CPU_IDLE" ]; then
        CPU_USED=$(echo "100 - $CPU_IDLE" | bc 2>/dev/null || echo "0")
        CPU_INT=$(echo "$CPU_USED" | cut -d'.' -f1)

        if [ "$CPU_INT" -gt 80 ]; then
            CPU_COLOR="${RED}"
        elif [ "$CPU_INT" -gt 50 ]; then
            CPU_COLOR="${YELLOW}"
        else
            CPU_COLOR="${GREEN}"
        fi

        # Draw CPU bar
        BAR_WIDTH=40
        BAR_FILL=$(echo "scale=0; ($CPU_INT * $BAR_WIDTH) / 100" | bc 2>/dev/null || echo "0")
        CPU_BAR="["
        for ((i=0; i<$BAR_WIDTH; i++)); do
            if [ $i -lt "$BAR_FILL" ]; then
                CPU_BAR="${CPU_BAR}█"
            else
                CPU_BAR="${CPU_BAR}░"
            fi
        done
        CPU_BAR="${CPU_BAR}]"

        echo -e "  CPU:     ${CPU_COLOR}${CPU_BAR}${NC} ${CPU_INT}%"
    fi

    # Memory
    MEM_INFO=$(free -h 2>/dev/null | grep Mem)
    if [ -n "$MEM_INFO" ]; then
        MEM_TOTAL=$(echo "$MEM_INFO" | awk '{print $2}')
        MEM_USED=$(echo "$MEM_INFO" | awk '{print $3}')
        MEM_PERCENT=$(echo "$MEM_INFO" | awk '{printf "%.0f", $3/$2 * 100}' | cut -d'.' -f1)

        if [ "$MEM_PERCENT" -gt 90 ]; then
            MEM_COLOR="${RED}"
        elif [ "$MEM_PERCENT" -gt 75 ]; then
            MEM_COLOR="${YELLOW}"
        else
            MEM_COLOR="${GREEN}"
        fi

        # Draw memory bar
        BAR_FILL=$(echo "scale=0; ($MEM_PERCENT * $BAR_WIDTH) / 100" | bc 2>/dev/null || echo "0")
        MEM_BAR="["
        for ((i=0; i<$BAR_WIDTH; i++)); do
            if [ $i -lt "$BAR_FILL" ]; then
                MEM_BAR="${MEM_BAR}█"
            else
                MEM_BAR="${MEM_BAR}░"
            fi
        done
        MEM_BAR="${MEM_BAR}]"

        echo -e "  Memory:  ${MEM_COLOR}${MEM_BAR}${NC} ${MEM_PERCENT}% (${MEM_USED}/${MEM_TOTAL})"
    fi

    # Disk
    DISK_INFO=$(df -h /home/samuel 2>/dev/null | tail -1)
    if [ -n "$DISK_INFO" ]; then
        DISK_TOTAL=$(echo "$DISK_INFO" | awk '{print $2}')
        DISK_USED=$(echo "$DISK_INFO" | awk '{print $3}')
        DISK_PERCENT=$(echo "$DISK_INFO" | awk '{print $5}' | cut -d'%' -f1)

        if [ "$DISK_PERCENT" -gt 90 ]; then
            DISK_COLOR="${RED}"
        elif [ "$DISK_PERCENT" -gt 75 ]; then
            DISK_COLOR="${YELLOW}"
        else
            DISK_COLOR="${GREEN}"
        fi

        # Draw disk bar
        BAR_FILL=$(echo "scale=0; ($DISK_PERCENT * $BAR_WIDTH) / 100" | bc 2>/dev/null || echo "0")
        DISK_BAR="["
        for ((i=0; i<$BAR_WIDTH; i++)); do
            if [ $i -lt "$BAR_FILL" ]; then
                DISK_BAR="${DISK_BAR}█"
            else
                DISK_BAR="${DISK_BAR}░"
            fi
        done
        DISK_BAR="${DISK_BAR}]"

        echo -e "  Disk:    ${DISK_COLOR}${DISK_BAR}${NC} ${DISK_PERCENT}% (${DISK_USED}/${DISK_TOTAL})"
    fi

    draw_bottom
    echo ""

    # 5. Service Status
    draw_box " SERVICE STATUS "

    if systemctl is-active --quiet atomic-http-server 2>/dev/null; then
        echo -e "  Process:  ${GREEN}✅ RUNNING${NC}"

        # Try to get PID
        PID=$(systemctl show -p MainPID --value atomic-http-server 2>/dev/null || echo "N/A")
        echo -e "  PID:      ${CYAN}${PID}${NC}"

        # Get restart count
        RESTARTS=$(systemctl show -p NRestarts --value atomic-http-server 2>/dev/null || echo "0")
        echo -e "  Restarts: ${CYAN}${RESTARTS}${NC}"
    else
        echo -e "  Process:  ${RED}❌ STOPPED${NC}"
    fi

    draw_bottom
    echo ""

    # 6. Network
    draw_box " NETWORK CONNECTIONS "
    LISTEN=$(netstat -tnl 2>/dev/null | grep LISTEN | wc -l || echo "0")
    ESTABLISHED=$(netstat -tn 2>/dev/null | grep ESTABLISHED | wc -l || echo "0")
    TIME_WAIT=$(netstat -tn 2>/dev/null | grep TIME_WAIT | wc -l || echo "0")

    echo -e "  Listening:    ${CYAN}${LISTEN}${NC} ports"
    echo -e "  Established:  ${CYAN}${ESTABLISHED}${NC} connections"
    echo -e "  Time Wait:    ${CYAN}${TIME_WAIT}${NC} connections"

    draw_bottom
    echo ""

    # Footer
    echo -e "${BOLD}${CYAN}═════════════════════════════════════════════════════════${NC}"
    echo -e "${MAGENTA}Press Ctrl+C to exit | Refreshing every ${REFRESH_INTERVAL}s${NC}"
    echo -e "${BOLD}${CYAN}═════════════════════════════════════════════════════════${NC}"
}

# Main loop
if [ "$1" = "once" ]; then
    # Single run mode for testing
    show_dashboard
else
    # Continuous mode with watch-like behavior
    while true; do
        show_dashboard
        sleep "$REFRESH_INTERVAL"
    done
fi
