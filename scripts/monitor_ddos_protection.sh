#!/bin/bash
# Real-time DDoS protection monitoring
# Monitor kernel parameters, conntrack, and iptables

set -e

INTERVAL="${1:-1}"  # Default 1 second refresh

clear

while true; do
    clear
    echo "===== DDoS Protection Monitor (Updated every ${INTERVAL}s) ====="
    echo ""

    # Kernel Parameters
    echo "KERNEL PARAMETERS:"
    echo "  SYN Protection:"
    echo "    tcp_syncookies=$(sysctl -n net.ipv4.tcp_syncookies 2>/dev/null || echo 'N/A')"
    echo "    tcp_max_syn_backlog=$(sysctl -n net.ipv4.tcp_max_syn_backlog 2>/dev/null || echo 'N/A')"

    echo "  Connection Tracking:"
    local nf_current=$(cat /proc/net/nf_conntrack 2>/dev/null | wc -l || echo "0")
    local nf_max=$(sysctl -n net.netfilter.nf_conntrack_max 2>/dev/null || echo "1000000")
    local nf_percent=$(( (nf_current * 100) / nf_max ))
    echo "    conntrack: $nf_current / $nf_max ($nf_percent%)"

    echo "  Connection Limits:"
    echo "    somaxconn=$(sysctl -n net.core.somaxconn 2>/dev/null || echo 'N/A')"
    echo "    netdev_max_backlog=$(sysctl -n net.core.netdev_max_backlog 2>/dev/null || echo 'N/A')"

    echo ""
    echo "SOCKET STATES:"
    local established=$(ss -tan | grep ESTAB | wc -l)
    local time_wait=$(ss -tan | grep TIME_WAIT | wc -l)
    local syn_recv=$(ss -tan | grep SYN_RECV | wc -l)
    echo "  Established: $established"
    echo "  TIME_WAIT: $time_wait"
    echo "  SYN_RECV: $syn_recv"

    echo ""
    echo "IPTABLES INPUT CHAIN (First 15 rules):"
    sudo iptables -L INPUT -n -v 2>/dev/null | head -20 || echo "  (requires sudo)"

    echo ""
    echo "PACKET DROPS (Sample):"
    sudo iptables -L INPUT -n -v 2>/dev/null | grep -i "drop\|dpt:" | head -5 || echo "  (requires sudo)"

    echo ""
    echo "SYSTEM RESOURCES:"
    local mem_usage=$(free | awk 'NR==2 {printf "%.1f%%", $3/$2*100}')
    local cpu_load=$(uptime | awk -F'load average:' '{print $2}')
    echo "  Memory: $mem_usage"
    echo "  Load Average: $cpu_load"

    echo ""
    echo "Press Ctrl+C to exit. Refreshing in ${INTERVAL}s..."
    sleep "$INTERVAL"
done
