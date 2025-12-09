# Prometheus MCP Integration for Claude Code

## Overview

The `prometheus-mcp-server` provides Claude Code with direct access to Prometheus metrics via the Model Context Protocol (MCP). This enables natural language queries of metrics, targets, and performance data from Kindly infrastructure.

## Architecture

```
+------------------+     MCP/STDIO      +-------------------------+     HTTP      +------------------+
|  Claude Code     | -----------------> | prometheus-mcp-server   | -----------> |  Prometheus      |
|  (local)         |                    | (Docker container)      |              |  (kindly-hub)    |
+------------------+                    +-------------------------+              +------------------+
                                                                                 192.168.0.38:9090
```

## Installation

### Prerequisites

- Docker installed locally
- Network access to Prometheus server (192.168.0.38:9090)
- Claude Code CLI installed

### Quick Start

```bash
# 1. Pull the Docker image
docker pull ghcr.io/pab1it0/prometheus-mcp-server:latest

# 2. Add to Claude Code (already configured)
claude mcp add prometheus -s user -- docker run -i --rm --network=host \
  -e PROMETHEUS_URL=http://192.168.0.38:9090 \
  ghcr.io/pab1it0/prometheus-mcp-server:latest

# 3. Verify configuration
cat ~/.claude.json | grep -A 15 '"prometheus"'
```

## Available MCP Tools

| Tool | Description | Input Schema |
|------|-------------|--------------|
| `execute_query` | Execute PromQL instant query | `query` (string), `time` (optional) |
| `execute_range_query` | Execute PromQL range query | `query`, `start`, `end`, `step` (all strings) |
| `list_metrics` | List available metrics | `limit`, `offset`, `filter_pattern` (optional) |
| `get_metric_metadata` | Get metric metadata | `metric` (string) |
| `get_targets` | Get scrape targets | (none) |
| `health_check` | Health check | (none) |

## Kindly Infrastructure Metrics

### Current Targets (kindly-hub)

| Job | Instance | Port | Health | Description |
|-----|----------|------|--------|-------------|
| prometheus | localhost:9090 | 9090 | UP | Prometheus self-monitoring |
| node | localhost:9100 | 9100 | UP | Node exporter (system metrics) |
| caddy | localhost:2019 | 2019 | UP | Caddy reverse proxy |
| kindly-services | localhost:8082 | 8082 | DOWN | Kindly application services |

### Metric Count

- **Total metrics available**: 561
- **Scrape interval**: 5s (prometheus), 15s (others)

## Example Queries

### System Health

```promql
# All targets up/down status
up

# CPU usage (1 minute average)
100 - (avg by (instance) (rate(node_cpu_seconds_total{mode="idle"}[1m])) * 100)

# Memory usage percentage
100 * (1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes))

# Disk usage
100 - (node_filesystem_avail_bytes{mountpoint="/"} / node_filesystem_size_bytes{mountpoint="/"}) * 100
```

### Network

```promql
# Network receive rate (bytes/sec)
rate(node_network_receive_bytes_total[5m])

# Network transmit rate (bytes/sec)
rate(node_network_transmit_bytes_total[5m])

# Network errors
rate(node_network_receive_errs_total[5m]) + rate(node_network_transmit_errs_total[5m])
```

### Prometheus Performance

```promql
# Prometheus scrape duration
prometheus_target_scrape_pool_sync_total

# Prometheus memory usage
process_resident_memory_bytes{job="prometheus"}

# TSDB head chunks
prometheus_tsdb_head_chunks
```

### Caddy Metrics

```promql
# Caddy HTTP requests
caddy_admin_http_requests_total

# Caddy config reload status
caddy_config_last_reload_successful

# Upstream health
caddy_reverse_proxy_upstreams_healthy
```

## Claude Code Usage Examples

Once configured, ask Claude Code natural language questions like:

- "What's the current CPU usage on kindly-hub?"
- "Show me memory usage over the last hour"
- "Are all Prometheus targets healthy?"
- "List all available metrics containing 'cpu'"
- "What's the disk usage on kindly-hub?"
- "Show me network traffic trends"

## Firewall Configuration

The following UFW rule was added to kindly-hub to allow Prometheus access from the LAN:

```bash
sudo ufw allow from 192.168.0.0/24 to any port 9090 proto tcp comment 'Prometheus from LAN'
```

## Troubleshooting

### Connection Issues

```bash
# Test Prometheus connectivity
curl -s "http://192.168.0.38:9090/api/v1/query?query=up"

# Verify firewall rule on kindly-hub
ssh samuel@kindly-hub "sudo ufw status | grep 9090"

# Test MCP server directly
echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}' | \
  docker run -i --rm --network=host -e PROMETHEUS_URL=http://192.168.0.38:9090 \
  ghcr.io/pab1it0/prometheus-mcp-server:latest
```

### MCP Server Not Starting

```bash
# Check Docker image
docker images | grep prometheus-mcp

# Re-pull image
docker pull ghcr.io/pab1it0/prometheus-mcp-server:latest

# Check Claude Code config
claude mcp list
```

### Prometheus Not Responding

```bash
# SSH to kindly-hub and check service
ssh samuel@kindly-hub "systemctl status prometheus"

# Check port binding
ssh samuel@kindly-hub "ss -tlnp | grep 9090"
```

## Technical Details

- **MCP Protocol Version**: 2024-11-05
- **FastMCP Version**: 2.11.3
- **Server Name**: Prometheus MCP
- **Transport**: STDIO
- **Docker Image**: `ghcr.io/pab1it0/prometheus-mcp-server:latest`

## References

- [prometheus-mcp-server GitHub](https://github.com/pab1it0/prometheus-mcp-server)
- [Model Context Protocol Specification](https://modelcontextprotocol.io/)
- [PromQL Documentation](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [Prometheus API Documentation](https://prometheus.io/docs/prometheus/latest/querying/api/)
