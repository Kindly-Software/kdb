# Integration Guide - Grafana + Prometheus

**Read Time**: 20-30 minutes
**Target Audience**: DevOps, SREs, Platform Engineers
**Prerequisites**: Basic Docker knowledge, Grafana/Prometheus familiarity

---

## Overview

This guide shows how to integrate Clapi Core with Grafana and Prometheus for comprehensive monitoring.

**What You'll Build**:
- Prometheus metrics export from Clapi Core
- Grafana dashboards for real-time monitoring
- Alerts for critical conditions (budget exhaustion, circuit breaker trips)
- Long-term metric storage and analysis

---

## Architecture

```
┌─────────────┐
│ Clapi Core  │
│ :8080       │ Metrics HTTP endpoint
└──────┬──────┘
       │
       │ HTTP GET /metrics (Prometheus format)
       ▼
┌──────────────┐
│ Prometheus   │
│ :9090        │ Scrapes metrics every 15s
└──────┬───────┘
       │
       │ PromQL queries
       ▼
┌──────────────┐
│   Grafana    │
│ :3000        │ Visualizes metrics
└──────────────┘
```

---

## Step 1: Enable Prometheus Metrics in Clapi Core

### Configuration

Edit `clapi.toml`:

```toml
[server]
listen_addr = "0.0.0.0:8080"
default_budget_cents = 100_00

[metrics]
enabled = true
prometheus_endpoint = "/metrics"  # Expose at /metrics
include_histograms = true          # P50/P95/P99 latency percentiles
include_gauges = true              # Current values (budget remaining, active slots)
include_counters = true            # Cumulative counts (total requests, errors)
```

### Verify Metrics Endpoint

```bash
curl http://localhost:8080/metrics
```

**Expected Output** (Prometheus format):
```prometheus
# HELP clapi_budget_check_duration_seconds Budget check operation latency
# TYPE clapi_budget_check_duration_seconds histogram
clapi_budget_check_duration_seconds_bucket{le="0.00001"} 1523
clapi_budget_check_duration_seconds_bucket{le="0.00005"} 4892
clapi_budget_check_duration_seconds_bucket{le="0.0001"} 9845
clapi_budget_check_duration_seconds_sum 0.592
clapi_budget_check_duration_seconds_count 10000

# HELP clapi_active_budget_slots Active budget slots
# TYPE clapi_active_budget_slots gauge
clapi_active_budget_slots 1523

# HELP clapi_circuit_breaker_state Circuit breaker state (0=Closed, 1=HalfOpen, 2=Open)
# TYPE clapi_circuit_breaker_state gauge
clapi_circuit_breaker_state{provider_id="anthropic"} 0
clapi_circuit_breaker_state{provider_id="openai"} 0
```

---

## Step 2: Set Up Prometheus

### Docker Compose Configuration

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  clapi:
    image: clapi-core:latest
    ports:
      - "8080:8080"
    volumes:
      - ./clapi.toml:/etc/clapi/clapi.toml
    environment:
      - RUST_LOG=info

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--storage.tsdb.retention.time=30d'  # Retain 30 days
    depends_on:
      - clapi

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/dashboards:/etc/grafana/provisioning/dashboards
      - ./grafana/datasources:/etc/grafana/provisioning/datasources
    environment:
      - GF_SECURITY_ADMIN_USER=admin
      - GF_SECURITY_ADMIN_PASSWORD=admin  # Change in production!
    depends_on:
      - prometheus

volumes:
  prometheus-data:
  grafana-data:
```

### Prometheus Configuration

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s      # Scrape every 15 seconds
  evaluation_interval: 15s  # Evaluate rules every 15 seconds

scrape_configs:
  - job_name: 'clapi'
    static_configs:
      - targets: ['clapi:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
    scrape_timeout: 10s
```

### Start Services

```bash
docker-compose up -d

# Verify Prometheus is scraping
curl http://localhost:9090/api/v1/targets

# Expected: "health": "up" for clapi target
```

---

## Step 3: Configure Grafana

### Add Prometheus Data Source

1. Open Grafana: `http://localhost:3000` (admin/admin)
2. Navigate: Configuration → Data Sources → Add data source
3. Select: **Prometheus**
4. Configure:
   - **Name**: Clapi Prometheus
   - **URL**: `http://prometheus:9090`
   - **Access**: Server (default)
5. Click **Save & Test** (should show "Data source is working")

### Create Dashboard

#### Method 1: Import JSON Dashboard

Create `grafana/dashboards/clapi-dashboard.json`:

```json
{
  "dashboard": {
    "title": "Clapi Core Monitoring",
    "panels": [
      {
        "id": 1,
        "title": "Budget Check Latency (P99)",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.99, rate(clapi_budget_check_duration_seconds_bucket[5m]))",
            "legendFormat": "P99 Latency"
          }
        ],
        "yaxes": [
          {"format": "s", "label": "Latency"}
        ]
      },
      {
        "id": 2,
        "title": "Active Budget Slots",
        "type": "graph",
        "targets": [
          {
            "expr": "clapi_active_budget_slots",
            "legendFormat": "Active Slots"
          }
        ]
      },
      {
        "id": 3,
        "title": "Circuit Breaker State",
        "type": "stat",
        "targets": [
          {
            "expr": "clapi_circuit_breaker_state",
            "legendFormat": "{{provider_id}}"
          }
        ]
      }
    ]
  }
}
```

Import: Dashboards → Import → Upload JSON file

#### Method 2: Manual Dashboard Creation

1. Create Dashboard: Dashboards → New Dashboard
2. Add Panel
3. Configure queries (see examples below)

---

## Key Metrics & Queries

### Latency Metrics

**Budget Check P99**:
```promql
histogram_quantile(0.99, rate(clapi_budget_check_duration_seconds_bucket[5m]))
```

**Provider Request P99**:
```promql
histogram_quantile(0.99, rate(clapi_provider_request_duration_seconds_bucket[5m])) by (provider_id)
```

**Circuit Breaker Check**:
```promql
histogram_quantile(0.99, rate(clapi_circuit_breaker_check_duration_seconds_bucket[5m]))
```

### Throughput Metrics

**Budget Operations per Second**:
```promql
rate(clapi_budget_operations_total[1m])
```

**Provider Requests per Second (by provider)**:
```promql
rate(clapi_provider_requests_total[1m]) by (provider_id)
```

**HTTP Requests per Second**:
```promql
rate(clapi_http_requests_total[1m]) by (endpoint)
```

### Resource Metrics

**Active Budget Slots**:
```promql
clapi_active_budget_slots
```

**Slot Utilization %**:
```promql
(clapi_active_budget_slots / clapi_max_budget_slots) * 100
```

**Memory Usage** (if exported):
```promql
clapi_memory_usage_bytes
```

### Error Metrics

**Budget Exhaustion Rate**:
```promql
rate(clapi_budget_exhausted_total[5m])
```

**Circuit Breaker Trips per Hour**:
```promql
increase(clapi_circuit_breaker_trips_total[1h]) by (provider_id)
```

**Provider Timeout Rate**:
```promql
rate(clapi_provider_timeouts_total[5m]) by (provider_id)
```

---

## Alerts

### Prometheus Alert Rules

Create `prometheus/alerts.yml`:

```yaml
groups:
  - name: clapi_alerts
    interval: 30s
    rules:
      # CRITICAL: All providers down
      - alert: AllProvidersDown
        expr: sum(clapi_circuit_breaker_state == 2) == count(clapi_circuit_breaker_state)
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "All AI providers are unavailable"
          description: "All providers have open circuit breakers for >5 minutes"

      # CRITICAL: Budget slots exhausted
      - alert: BudgetSlotsFull
        expr: (clapi_active_budget_slots / clapi_max_budget_slots) > 0.95
        for: 10m
        labels:
          severity: critical
        annotations:
          summary: "Budget slots >95% full"
          description: "Active slots: {{ $value | humanizePercentage }}"

      # CRITICAL: High P99 latency
      - alert: HighP99Latency
        expr: histogram_quantile(0.99, rate(clapi_budget_check_duration_seconds_bucket[5m])) > 0.0005
        for: 15m
        labels:
          severity: critical
        annotations:
          summary: "Budget check P99 latency >500ns"
          description: "P99: {{ $value | humanizeDuration }}"

      # WARNING: Low budget
      - alert: LowBudget
        expr: clapi_remaining_budget_cents < 1000_00
        for: 1h
        labels:
          severity: warning
        annotations:
          summary: "Budget below $1000"
          description: "Remaining: ${{ $value | humanize }}"

      # WARNING: Circuit breaker open
      - alert: CircuitBreakerOpen
        expr: clapi_circuit_breaker_state == 2
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Circuit breaker open for {{ $labels.provider_id }}"
          description: "Provider unhealthy for >5 minutes"

      # WARNING: High timeout rate
      - alert: HighTimeoutRate
        expr: rate(clapi_provider_timeouts_total[5m]) > 0.05
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "Provider timeout rate >5%"
          description: "Timeout rate: {{ $value | humanizePercentage }} for {{ $labels.provider_id }}"
```

Update `prometheus.yml`:

```yaml
rule_files:
  - '/etc/prometheus/alerts.yml'

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']  # Optional: Add AlertManager
```

---

## Grafana Dashboard Panels

### Panel 1: Latency Overview

**Panel Type**: Time Series (Graph)

**Metrics**:
```promql
# P50
histogram_quantile(0.50, rate(clapi_budget_check_duration_seconds_bucket[5m]))

# P95
histogram_quantile(0.95, rate(clapi_budget_check_duration_seconds_bucket[5m]))

# P99
histogram_quantile(0.99, rate(clapi_budget_check_duration_seconds_bucket[5m]))

# P999
histogram_quantile(0.999, rate(clapi_budget_check_duration_seconds_bucket[5m]))
```

**Visualization**:
- Y-axis: Seconds (format: `s`)
- Legend: P50, P95, P99, P999
- Threshold: 300ns (red line at 0.0000003)

---

### Panel 2: Throughput

**Panel Type**: Time Series (Graph)

**Metrics**:
```promql
# Total throughput
sum(rate(clapi_budget_operations_total[1m]))

# Per-operation breakdown
rate(clapi_budget_operations_total[1m]) by (operation)
```

**Visualization**:
- Y-axis: ops/s
- Legend: Total, Allocate, Deduct, Deallocate
- Stack: Enabled

---

### Panel 3: Circuit Breaker Status

**Panel Type**: Stat (Single Value)

**Metrics**:
```promql
clapi_circuit_breaker_state
```

**Value Mappings**:
- 0 → ✅ Closed (Green)
- 1 → ⚠️ HalfOpen (Yellow)
- 2 → ❌ Open (Red)

**Repeat**: By `provider_id` variable

---

### Panel 4: Error Rate

**Panel Type**: Time Series (Graph)

**Metrics**:
```promql
# Budget exhaustion
rate(clapi_budget_exhausted_total[5m])

# Circuit breaker trips
rate(clapi_circuit_breaker_trips_total[5m]) by (provider_id)

# Provider timeouts
rate(clapi_provider_timeouts_total[5m]) by (provider_id)
```

**Visualization**:
- Y-axis: errors/s
- Legend: By error type
- Threshold: >1% (yellow), >5% (red)

---

### Panel 5: Resource Utilization

**Panel Type**: Gauge

**Metrics**:
```promql
# Slot utilization
(clapi_active_budget_slots / clapi_max_budget_slots) * 100
```

**Thresholds**:
- 0-80%: Green
- 80-95%: Yellow
- 95-100%: Red

---

## Step 4: Test the Integration

### Generate Load

```bash
# Install hey (HTTP load generator)
go install github.com/rakyll/hey@latest

# Generate 1000 requests over 10 seconds
hey -n 1000 -c 10 -m POST \
  -H "Authorization: Bearer budget_test" \
  -H "Content-Type: application/json" \
  -d '{"model":"claude-3-5-sonnet-20241022","max_tokens":10,"messages":[{"role":"user","content":"test"}]}' \
  http://localhost:8080/v1/chat/completions
```

### Verify Metrics in Grafana

1. Open dashboard: `http://localhost:3000`
2. Observe real-time updates:
   - Latency should spike during load
   - Throughput should increase
   - Active slots should increase/decrease
   - No circuit breaker trips (if providers healthy)

---

## Step 5: Production Deployment

### High-Availability Setup

```yaml
# docker-compose-ha.yml
version: '3.8'

services:
  clapi-1:
    image: clapi-core:latest
    # ... config ...

  clapi-2:
    image: clapi-core:latest
    # ... config ...

  clapi-3:
    image: clapi-core:latest
    # ... config ...

  prometheus:
    image: prom/prometheus:latest
    volumes:
      - ./prometheus-ha.yml:/etc/prometheus/prometheus.yml
```

Update `prometheus-ha.yml`:

```yaml
scrape_configs:
  - job_name: 'clapi'
    static_configs:
      - targets:
          - 'clapi-1:8080'
          - 'clapi-2:8080'
          - 'clapi-3:8080'
    relabel_configs:
      - source_labels: [__address__]
        target_label: instance
```

---

## Troubleshooting

### Prometheus Not Scraping

**Check**:
```bash
curl http://localhost:9090/api/v1/targets
```

**Solutions**:
- Verify Clapi metrics endpoint: `curl http://localhost:8080/metrics`
- Check Prometheus logs: `docker logs prometheus`
- Verify network connectivity: `docker exec prometheus ping clapi`

---

### Grafana Can't Connect to Prometheus

**Check**:
```bash
docker exec grafana curl http://prometheus:9090/api/v1/query?query=up
```

**Solutions**:
- Verify Prometheus is running: `docker ps | grep prometheus`
- Check data source URL: Should be `http://prometheus:9090` (Docker network)
- Test from host: `curl http://localhost:9090/api/v1/query?query=up`

---

### Missing Metrics in Grafana

**Check**:
```bash
# Query Prometheus directly
curl 'http://localhost:9090/api/v1/query?query=clapi_budget_check_duration_seconds_count'
```

**Solutions**:
- Verify metric exists in Prometheus: Query Explorer
- Check metric name spelling (case-sensitive)
- Verify time range in Grafana (last 6 hours default)

---

## Further Reading

- **[Prometheus Docs](https://prometheus.io/docs/)** - Official Prometheus documentation
- **[Grafana Docs](https://grafana.com/docs/)** - Official Grafana documentation
- **[PromQL Cheat Sheet](https://promlabs.com/promql-cheat-sheet/)** - Query language reference

---

**Document Version**: 1.0
**Line Count**: ~500 lines
**Last Updated**: 2025-10-21
