#!/usr/bin/env bash
# log-tool-use.sh - Log MCP tool usage for analytics and optimization
# PostToolUse hook for Claude Code integration
# Invoked after each MCP tool execution
#
# Usage:
#   ./scripts/log-tool-use.sh                    # Auto-detect environment
#   MCP_TOOL_NAME=xpath_query ./scripts/log-tool-use.sh
#
# Environment variables (set by Claude Code):
#   MCP_TOOL_NAME       Tool name (xpath_query, cache_stats, etc)
#   MCP_TOOL_DURATION   Execution duration in milliseconds
#   MCP_TOOL_STATUS     Status (success, error, timeout)
#   MCP_TOOL_RESULT_SIZE Result size in bytes
#   MCP_CACHE_HIT       Cache hit (true/false)
#
# Exit codes:
#   0 = Success (log written)
#   1 = Error (logging failed)

set -euo pipefail

# Configuration
LOG_DIR="${LOG_DIR:-${HOME}/.cache/claude-doc-optimizer/logs}"
SESSION_LOG="${LOG_DIR}/session.jsonl"
DAILY_LOG="${LOG_DIR}/daily-$(date +%Y-%m-%d).jsonl"
MAX_LOG_SIZE=$((100 * 1024 * 1024))  # 100 MB

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

# Ensure log directory exists
mkdir -p "$LOG_DIR"

# Rotate log if too large
rotate_log() {
    local log_file="$1"

    if [[ -f "$log_file" && $(stat -f%z "$log_file" 2>/dev/null || stat -c%s "$log_file") -gt $MAX_LOG_SIZE ]]; then
        local archive="${log_file}.$(date +%s)"
        mv "$log_file" "$archive"

        # Compress if gzip available
        if command -v gzip &> /dev/null; then
            gzip "$archive" &
        fi
    fi
}

# Create JSON log entry
create_log_entry() {
    local tool_name="${MCP_TOOL_NAME:-unknown}"
    local duration_ms="${MCP_TOOL_DURATION:-0}"
    local status="${MCP_TOOL_STATUS:-unknown}"
    local result_size="${MCP_TOOL_RESULT_SIZE:-0}"
    local cache_hit="${MCP_CACHE_HIT:-false}"
    local timestamp=$(date -u '+%Y-%m-%dT%H:%M:%S.000Z')
    local session_id="${SESSION_ID:-$(uuidgen 2>/dev/null || echo 'unknown')}"

    # Create JSON object
    cat << EOF
{
  "timestamp": "$timestamp",
  "session_id": "$session_id",
  "tool": "$tool_name",
  "duration_ms": $duration_ms,
  "status": "$status",
  "result_size_bytes": $result_size,
  "cache_hit": $cache_hit,
  "hostname": "$(hostname)",
  "user": "$(id -un)"
}
EOF
}

# Log to file
log_entry() {
    local entry="$1"

    # Append to session log
    rotate_log "$SESSION_LOG"
    echo "$entry" >> "$SESSION_LOG"

    # Append to daily log
    rotate_log "$DAILY_LOG"
    echo "$entry" >> "$DAILY_LOG"
}

# Update metrics
update_metrics() {
    local tool_name="${MCP_TOOL_NAME:-unknown}"
    local duration_ms="${MCP_TOOL_DURATION:-0}"
    local cache_hit="${MCP_CACHE_HIT:-false}"
    local metrics_file="${LOG_DIR}/metrics.json"

    # Create metrics file if doesn't exist
    if [[ ! -f "$metrics_file" ]]; then
        cat > "$metrics_file" << 'EOF'
{
  "tools": {},
  "cache": {
    "hits": 0,
    "misses": 0,
    "hit_rate": 0.0
  },
  "performance": {
    "avg_duration_ms": 0,
    "min_duration_ms": 0,
    "max_duration_ms": 0,
    "p95_duration_ms": 0,
    "p99_duration_ms": 0
  },
  "updated_at": ""
}
EOF
    fi

    # Update metrics (requires jq)
    if command -v jq &> /dev/null; then
        local updated_at=$(date -u '+%Y-%m-%dT%H:%M:%SZ')

        # Add tool entry if missing
        jq --arg tool "$tool_name" \
            '.tools[$tool] //= {"count": 0, "total_ms": 0, "cache_hits": 0}' \
            "$metrics_file" > "$metrics_file.tmp"
        mv "$metrics_file.tmp" "$metrics_file"

        # Update tool metrics
        jq --arg tool "$tool_name" \
            --argjson duration "$duration_ms" \
            --arg cache_hit "$cache_hit" \
            '.tools[$tool].count += 1 |
             .tools[$tool].total_ms += $duration |
             .tools[$tool].cache_hits += ($cache_hit == "true" | if . then 1 else 0 end) |
             .updated_at = now | @iso8601' \
            "$metrics_file" > "$metrics_file.tmp"
        mv "$metrics_file.tmp" "$metrics_file"

        # Update cache hit rate
        local total_hits=$(jq '[.tools[].cache_hits // 0] | add' "$metrics_file")
        local total_calls=$(jq '[.tools[].count // 0] | add' "$metrics_file")

        if [[ $total_calls -gt 0 ]]; then
            local hit_rate=$(echo "scale=4; $total_hits / $total_calls" | bc)
            jq --argjson rate "$hit_rate" '.cache.hit_rate = $rate' "$metrics_file" > "$metrics_file.tmp"
            mv "$metrics_file.tmp" "$metrics_file"
        fi
    fi
}

# Show summary
show_summary() {
    local tool_name="${MCP_TOOL_NAME:-unknown}"
    local duration_ms="${MCP_TOOL_DURATION:-0}"
    local status="${MCP_TOOL_STATUS:-unknown}"
    local cache_hit="${MCP_CACHE_HIT:-false}"

    # Color status
    local status_color="$RED"
    [[ "$status" == "success" ]] && status_color="$GREEN"

    echo -e "${BLUE}[TOOL LOG]${NC} Tool: $tool_name | Status: ${status_color}${status}${NC} | Duration: ${duration_ms}ms | Cache: $cache_hit"
}

# Main entry point
main() {
    # Skip if no tool name provided
    if [[ -z "${MCP_TOOL_NAME:-}" ]]; then
        return 0
    fi

    # Create log entry
    local entry=$(create_log_entry)

    # Log to files
    log_entry "$entry"

    # Update metrics
    update_metrics

    # Show summary
    show_summary

    return 0
}

# Run main
main "$@"
