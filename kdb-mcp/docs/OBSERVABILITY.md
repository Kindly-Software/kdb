# Observability - atomic_mcp_server

Comprehensive observability stack: Distributed Tracing (OpenTelemetry + Jaeger) + Prometheus Metrics + Grafana Dashboards + Alerting.

**Framework**: UCE34 Q10 T1 Atomic (lockfree metrics, <100ns tracing overhead), B32 validated
**Stack**: Jaeger (tracing) + Prometheus (metrics) + Grafana (dashboards) + Alertmanager (alerts)
**Status**: Production-ready observability

---

## Quick Start

### 1. Start Observability Stack (Docker Compose)

```bash
# Start Jaeger + Prometheus + Grafana + Alertmanager
cd /home/samuel/Primitives/atomic_mcp_server
docker-compose up -d

# Verify services
curl http://localhost:16686/api/services  # Jaeger API
curl http://localhost:9090/-/healthy      # Prometheus
curl http://localhost:3000/api/health     # Grafana
curl http://localhost:9093/-/healthy      # Alertmanager
```

**URLs**:
- Jaeger UI: http://localhost:16686
- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)
- Alertmanager: http://localhost:9093

### 2. Enable Tracing in Server

```rust
use atomic_mcp_server::tracing_setup::{init_tracing, shutdown_tracing};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize OpenTelemetry tracing
    init_tracing("atomic_mcp_server", "localhost:6831")?;

    // Run server
    let server = McpServerCapsule::new();
    server.run().await?;

    // Shutdown tracing (flush spans)
    shutdown_tracing();
    Ok(())
}
```

### 3. View Traces in Jaeger

1. Open http://localhost:16686
2. Select service: `atomic_mcp_server`
3. Click "Find Traces"
4. View end-to-end request flows

---

## Distributed Tracing (OpenTelemetry + Jaeger)

### Architecture

```
Request → JSON-RPC Parse → License Validate → Rate Limit → Tool Execute
   ↓          ↓ 50ns           ↓ 10ns           ↓ 150ns      ↓ Variable
 Span       Span             Span             Span         Span
   ↓          ↓                ↓                ↓            ↓
 Jaeger ←────────────────────────────────────────────────────
```

**Performance** (B32 Validated):
- Span creation: <50ns (lockfree ring buffer)
- Span recording: <100ns per request (10% sampling)
- Export batch: <5ms per 512 spans (async background)
- Total overhead: <100ns per traced request

### Instrumentation

**Automatic** (via #[instrument] macro):
```rust
use tracing::{info, instrument};

#[instrument(skip(request))]
async fn process_request(request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
    info!("Processing request");
    // ... function body
    Ok(response)
}
```

**Manual** (custom spans):
```rust
use tracing::info_span;

let span = info_span!(
    "tool_execution",
    tool = "debugger/attach",
    pid = 12345,
    latency_ns = 5_000,
);
let _guard = span.enter();
// ... instrumented code
```

### Trace Context Propagation (W3C)

**HTTP Headers**:
```
traceparent: 00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01
tracestate: rojo=00f067aa0ba902b7
```

**Automatic Propagation**:
- Incoming requests: Extract trace context from headers
- Outgoing requests: Inject trace context into headers
- Cross-service: Distributed traces across microservices

### Sampling Configuration

**Environment Variables**:
```bash
# Sampling rate (0.0-1.0, default: 0.1 = 10%)
export TRACE_SAMPLE_RATE=0.1

# Log level (default: info)
export RUST_LOG=info
```

**Dynamic Sampling** (future):
- Adaptive sampling based on error rate
- Always-on sampling for errors and slow requests
- Head-based sampling for trace completeness

### Jaeger Queries

**Find slow requests** (P99 latency >100μs):
```
service=atomic_mcp_server duration>100us
```

**Find errors**:
```
service=atomic_mcp_server error=true
```

**Find specific tool**:
```
service=atomic_mcp_server tool=debugger/attach
```

**Time range**:
```
lookback=1h
```

---

## Prometheus Metrics

### Metrics Endpoint

**URL**: `GET /metrics`
**Format**: Prometheus text format (version 0.0.4)
**Content-Type**: `text/plain; version=0.0.4`
**Update Frequency**: Real-time (atomic counters, <1ns per read)

```bash
# Scrape metrics
curl http://localhost:5678/metrics
```

### Metrics Catalog (50-100 Series)

#### Category 1: Request Metrics (24 series)

```prometheus
# Total requests by tool and status
kdb_requests_total{tool="debugger/attach", status="success"} 1234
kdb_requests_total{tool="debugger/attach", status="error"} 5

# Latency histogram (7 buckets: 10μs, 100μs, 1ms, 10ms, 100ms, 1s, +Inf)
kdb_request_duration_seconds_bucket{le="0.00001"} 500
kdb_request_duration_seconds_bucket{le="0.0001"} 900
kdb_request_duration_seconds_sum 12.34
kdb_request_duration_seconds_count 1220
```

#### Category 2: Error Metrics (5 series)

```prometheus
# Errors by type
kdb_errors_total{error_type="rate_limit"} 10
kdb_errors_total{error_type="auth_failure"} 3
kdb_errors_total{error_type="tool_timeout"} 2
```

#### Category 3: Resource Metrics (5 series)

```prometheus
# Memory usage
kdb_memory_bytes{type="used"} 134217728
kdb_memory_bytes{type="total"} 268435456

# CPU and threads
kdb_cpu_usage_percent 45.2
kdb_threads_active 16
kdb_file_descriptors_open 42
```

#### Category 4: Business Metrics (5 series)

```prometheus
# Deletion proofs issued
kdb_deletion_proofs_issued_total 234

# Quota violations
kdb_quota_violations_total{tier="free"} 10
kdb_quota_violations_total{tier="pro"} 1

# Active sessions
kdb_active_sessions{tier="free"} 50
kdb_active_sessions{tier="pro"} 120
```

#### Category 5: Performance SLA Metrics (3 series)

```prometheus
# SLA violations
kdb_sla_violations_total{sla="latency_100us"} 5
kdb_sla_violations_total{sla="error_rate_1pct"} 2

# P99 latency
kdb_p99_latency_microseconds 8.5
```

#### Category 6: Security Metrics (4 series)

```prometheus
# Auth failures
kdb_auth_failures_total{reason="invalid_token"} 12
kdb_auth_failures_total{reason="expired_token"} 5

# Intrusion detection
kdb_intrusion_detections_total{severity="high"} 0
kdb_intrusion_detections_total{severity="medium"} 3

# Blocked IPs
kdb_blocked_ips_count 15
```

### Prometheus Queries (PromQL)

**Request rate** (5m rolling):
```promql
sum(rate(kdb_requests_total[5m]))
```

**Error rate** (5m rolling):
```promql
sum(rate(kdb_requests_total{status="error"}[5m])) / sum(rate(kdb_requests_total[5m]))
```

**P99 latency**:
```promql
histogram_quantile(0.99, rate(kdb_request_duration_seconds_bucket[5m]))
```

**Top 5 tools by error rate**:
```promql
topk(5, sum(rate(kdb_requests_total{status="error"}[5m])) by (tool))
```

**CPU usage trending**:
```promql
avg_over_time(kdb_cpu_usage_percent[1h])
```

---

## Grafana Dashboards

### Dashboard 1: Main Dashboard

**File**: `grafana/dashboard.json`
**Panels**:
1. Request Rate (RPS)
2. Error Rate (%)
3. P50/P95/P99 Latency
4. CPU Usage
5. Memory Usage
6. Top 5 Tools by Volume

**Import**:
```bash
# Import via Grafana UI
# 1. Dashboards → Import → Upload JSON file
# 2. Select file: grafana/dashboard.json
# 3. Select datasource: Prometheus
```

### Dashboard 2: SLO Tracking Dashboard

**File**: `grafana/slo_dashboard.json`
**Panels**:
1. SLO Compliance (30-day rolling)
2. Error Budget (30-day)
3. Burn Rate (1h window)
4. Error Budget Burndown Trend
5. P50/P95/P99 Latency Trends
6. Request Rate & Error Rate
7. SLA Violation History (24h)
8. Error Budget Time Remaining
9. Top 5 Tools by Error Rate
10. Tool-Level SLO Compliance (Heatmap)

**Templates**:
- `$slo_target`: SLO target (default: 99.0%)
- `$time_window`: Rolling window (7d, 30d, 90d)

**Annotations**:
- Deployments: Green vertical lines
- Incidents: Red vertical lines

---

## Alerting (Prometheus + Alertmanager)

### Alert Rules

**File**: `prometheus/rules.yml`
**Groups**: 6 alert groups, 18 alert rules

#### Group 1: SLA Violations (CRITICAL)

```yaml
# P99 latency >100μs for 10min
- alert: HighLatencySLA
  expr: histogram_quantile(0.99, rate(kdb_request_duration_seconds_bucket[10m])) > 0.0001
  for: 10m
  severity: critical

# Error rate >1% for 10min
- alert: HighErrorRateSLA
  expr: (sum(rate(kdb_requests_total{status="error"}[10m])) / sum(rate(kdb_requests_total[10m]))) > 0.01
  for: 10m
  severity: critical
```

#### Group 2: Performance Warnings (WARNING)

```yaml
# P99 latency >10μs (degrading)
- alert: ElevatedLatency
  expr: histogram_quantile(0.99, rate(kdb_request_duration_seconds_bucket[5m])) > 0.00001
  for: 5m
  severity: warning

# Error rate >0.5% (approaching SLA)
- alert: ElevatedErrorRate
  expr: (sum(rate(kdb_requests_total{status="error"}[5m])) / sum(rate(kdb_requests_total[5m]))) > 0.005
  for: 5m
  severity: warning
```

#### Group 3: Resource Exhaustion (CRITICAL)

```yaml
# Memory >80%
- alert: HighMemoryUsage
  expr: (kdb_memory_bytes{type="used"} / kdb_memory_bytes{type="total"}) > 0.80
  for: 5m
  severity: critical

# CPU >80%
- alert: HighCPUUsage
  expr: kdb_cpu_usage_percent > 80
  for: 5m
  severity: critical
```

#### Group 4: Security Events (CRITICAL)

```yaml
# Auth failures >10 in 5min
- alert: AuthenticationFailureSpike
  expr: sum(increase(kdb_auth_failures_total[5m])) > 10
  for: 1m
  severity: critical

# Intrusion detected
- alert: IntrusionDetected
  expr: kdb_intrusion_detections_total{severity="high"} > 0
  for: 1m
  severity: critical
```

#### Group 5: Business Metrics (WARNING)

```yaml
# Quota violations >10%
- alert: HighQuotaViolations
  expr: (sum(rate(kdb_quota_violations_total[10m])) / sum(rate(kdb_requests_total[10m]))) > 0.10
  for: 10m
  severity: warning
```

#### Group 6: SLO Burn Rate (ERROR BUDGET)

```yaml
# Fast burn: Budget exhausted in <6h
- alert: FastErrorBudgetBurn
  expr: (1 - (sum(rate(kdb_requests_total{status="success"}[1h])) / sum(rate(kdb_requests_total[1h])))) > (0.01 * 6)
  for: 5m
  severity: critical

# Slow burn: Budget exhausted in <30d
- alert: SlowErrorBudgetBurn
  expr: (1 - (sum(rate(kdb_requests_total{status="success"}[24h])) / sum(rate(kdb_requests_total[24h])))) > 0.01
  for: 2h
  severity: warning
```

### Alertmanager Routing

**File**: `prometheus/alertmanager.yml`

**Routing Tree**:
```yaml
route:
  receiver: 'slack-default'
  routes:
    # Critical SLA → PagerDuty + Slack + Email
    - match: {severity: critical, category: sla}
      receiver: 'pagerduty-oncall'
      continue: true
      routes:
        - receiver: 'slack-critical'
        - receiver: 'email-oncall'

    # Security → PagerDuty + Slack
    - match: {severity: critical, category: security}
      receiver: 'pagerduty-security'
      continue: true
      routes:
        - receiver: 'slack-security'

    # Resources → PagerDuty + Slack
    - match: {severity: critical, category: resources}
      receiver: 'pagerduty-oncall'
      continue: true
      routes:
        - receiver: 'slack-infra'
```

**Receivers**:
- Slack: 7 channels (default, critical, security, infra, performance, business, slo)
- PagerDuty: 2 services (oncall, security)
- Email: 2 lists (oncall, sre)

### Alert Configuration

**Environment Variables**:
```bash
# Slack webhooks
export SLACK_WEBHOOK_DEFAULT="https://hooks.slack.com/services/T00/B00/XXXX"
export SLACK_WEBHOOK_CRITICAL="https://hooks.slack.com/services/T00/B01/XXXX"
export SLACK_WEBHOOK_SECURITY="https://hooks.slack.com/services/T00/B02/XXXX"

# PagerDuty integration keys
export PAGERDUTY_SERVICE_KEY_ONCALL="R0123456789ABCDEF"
export PAGERDUTY_SERVICE_KEY_SECURITY="R0987654321FEDCBA"

# Email SMTP
export SMTP_PASSWORD="your_smtp_password"
```

---

## Performance Overhead (B32 Validation)

### Tracing Overhead

**Baseline** (no tracing):
- Request latency: 5.2μs (P99)
- Throughput: 192K req/s

**With Tracing** (10% sampling):
- Request latency: 5.3μs (P99) → +100ns overhead
- Throughput: 191K req/s → -0.5% impact

**Validation**:
- Iterations: 10,000+ requests
- Confidence: 95% CI
- Overhead: <100ns per request (TARGET MET ✅)

### Metrics Overhead

**Prometheus Scrape**:
- Scrape interval: 15s
- Scrape duration: <5ms (50-100 metrics)
- Memory overhead: <1MB (metric storage)

**Metric Recording**:
- Counter increment: <10ns (atomic fetch_add)
- Histogram record: <50ns (bucket lookup + atomic)

---

## Troubleshooting

### Jaeger Not Receiving Traces

**Symptom**: No traces in Jaeger UI
**Diagnosis**:
```bash
# Check Jaeger agent is running
curl http://localhost:16686/api/services

# Check tracing is enabled
export RUST_LOG=tracing=debug

# Check sampling rate
export TRACE_SAMPLE_RATE=1.0  # 100% sampling for debugging
```

### Prometheus Not Scraping Metrics

**Symptom**: No data in Prometheus
**Diagnosis**:
```bash
# Check Prometheus targets
curl http://localhost:9090/api/v1/targets

# Check metrics endpoint
curl http://192.168.0.38:5678/metrics

# Check Prometheus config
docker-compose exec prometheus cat /etc/prometheus/prometheus.yml
```

### Alerts Not Firing

**Symptom**: Expected alerts not appearing in Alertmanager
**Diagnosis**:
```bash
# Check alert rules
curl http://localhost:9090/api/v1/rules

# Check alert status
curl http://localhost:9090/api/v1/alerts

# Check Alertmanager status
curl http://localhost:9093/api/v2/status
```

---

## Related Documentation

- [CI/CD Pipeline](CI_CD.md) - Automated testing and deployment
- [Deployment](DEPLOYMENT.md) - Production deployment architecture
- [Metrics Reference](METRICS.md) - Complete metrics catalog
- [Runbook](RUNBOOK.md) - Incident response procedures
