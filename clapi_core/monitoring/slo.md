# Service Level Objectives (SLOs) for clapi_core

## Overview

This document defines the Service Level Objectives (SLOs) for clapi_core, a 100% lockfree AI call protection proxy with computational capsule architecture.

## Primary SLOs

### 1. Latency SLO

**Objective:** Maintain predictable latency for all proxy operations.

| Metric | Target | Measurement Window | Success Criteria |
|--------|--------|-------------------|------------------|
| P50 Latency | <10ms | 1 minute rolling | 99.5% of windows meet target |
| P95 Latency | <50ms | 1 minute rolling | 99.5% of windows meet target |
| P99 Latency | <100ms | 1 minute rolling | 99.0% of windows meet target |
| P999 Latency | <200ms | 1 minute rolling | 95.0% of windows meet target |

**Measurement:**
```promql
# P50 latency
histogram_quantile(0.50, rate(clapi_proxy_latency_ns_bucket[1m]))

# P95 latency
histogram_quantile(0.95, rate(clapi_proxy_latency_ns_bucket[1m]))

# P99 latency
histogram_quantile(0.99, rate(clapi_proxy_latency_ns_bucket[1m]))

# P999 latency
histogram_quantile(0.999, rate(clapi_proxy_latency_ns_bucket[1m]))
```

**Rationale:**
- P50 <10ms ensures most requests have negligible overhead (<1% of 100ms provider latency)
- P99 <100ms ensures tail latency doesn't impact user experience
- Targets based on computational capsule architecture (atomic operations <100ns)

---

### 2. Availability SLO

**Objective:** Maintain high availability for all clapi_core services.

| Metric | Target | Measurement Window | Success Criteria |
|--------|--------|-------------------|------------------|
| Uptime | >99.9% | 30 days rolling | <43 minutes downtime/month |
| Request Success Rate | >99.99% | 1 hour rolling | <0.01% error rate |

**Measurement:**
```promql
# Uptime percentage
avg_over_time(up{job="clapi_core"}[30d]) * 100

# Request success rate
(1 - (rate(clapi_proxy_errors_total[1h]) / rate(clapi_proxy_requests_total[1h]))) * 100
```

**Rationale:**
- 99.9% uptime = 43 minutes/month downtime (industry standard for critical services)
- 99.99% success rate ensures robust error handling

---

### 3. Error Rate SLO

**Objective:** Minimize errors across all operations.

| Operation | Error Rate Target | Measurement Window | Severity |
|-----------|------------------|-------------------|----------|
| Proxy Requests | <0.1% | 1 hour rolling | Critical |
| Budget Deductions | <0.01% | 1 hour rolling | Critical |
| OAuth Verification | <1.0% | 1 hour rolling | Warning |
| Payment Recording | <0.5% | 1 hour rolling | Critical |
| Circuit Breaker | <5% failure rate | 5 minutes rolling | Warning |

**Measurement:**
```promql
# Proxy error rate
(rate(clapi_proxy_errors_total[1h]) / rate(clapi_proxy_requests_total[1h])) * 100

# Budget deduction error rate
(rate(clapi_budget_exhausted_count[1h]) / rate(clapi_budget_deductions_total[1h])) * 100

# OAuth failure rate
(rate(clapi_oauth_verification_failures_total[1h]) / rate(clapi_oauth_sessions_total[1h])) * 100

# Payment failure rate
(rate(clapi_payments_failed_total[1h]) / rate(clapi_payments_recorded_total[1h])) * 100

# Circuit breaker failure rate
clapi_circuit_failure_rate_bp / 100
```

**Rationale:**
- Low error rates ensure system reliability
- Different targets reflect criticality of operations

---

### 4. Circuit Health SLO

**Objective:** Maintain healthy circuit breaker states across all providers.

| Metric | Target | Measurement Window | Success Criteria |
|--------|--------|-------------------|------------------|
| Open Circuits | <2 providers | 5 minutes rolling | 99% of time |
| Circuit Recovery | <60s cooldown | Per circuit trip | 95% of trips |
| Failover Success | >99.5% | 1 hour rolling | Automatic fallback |

**Measurement:**
```promql
# Count of open circuits
count(clapi_circuit_state == 2)

# Circuit recovery time
clapi_circuit_recovery_time_ns / 1e9

# Failover success rate
(1 - (rate(clapi_proxy_all_circuits_open_total[1h]) / rate(clapi_proxy_requests_total[1h]))) * 100
```

**Rationale:**
- <2 open circuits ensures provider redundancy
- 60s cooldown balances recovery vs. circuit flapping
- 99.5% failover success ensures high availability

---

## Secondary SLOs

### 5. Resource Utilization

| Resource | Target | Measurement Window | Alert Threshold |
|----------|--------|-------------------|-----------------|
| Memory Usage | <80% | 5 minutes rolling | >90% critical |
| CPU Usage | <70% | 5 minutes rolling | >90% critical |
| Active Threads | <100 | 1 minute rolling | >200 warning |

**Measurement:**
```promql
# Memory utilization
(clapi_memory_bytes / node_memory_MemTotal_bytes) * 100

# CPU utilization
clapi_cpu_usage_percent

# Active threads
clapi_threads_active
```

---

### 6. Budget Operations

| Metric | Target | Measurement Window | Success Criteria |
|--------|--------|-------------------|------------------|
| Allocation Latency | <100ns | 1 minute rolling | 99% of allocations |
| Deallocation Latency | <100ns | 1 minute rolling | 99% of deallocations |
| CAS Retry Rate | <5% | 1 minute rolling | 95% of time |

**Measurement:**
```promql
# Allocation latency
histogram_quantile(0.99, rate(clapi_budget_allocation_latency_ns_bucket[1m]))

# CAS retry rate
(rate(clapi_budget_cas_retries_total[1m]) / rate(clapi_budget_deductions_total[1m])) * 100
```

---

## SLO Tracking Dashboard

All SLOs are tracked in Grafana dashboards:

1. **dashboard_slo_overview.json**: Top-level SLO health
2. **dashboard_budget_overview.json**: Budget operation SLOs
3. **dashboard_circuit_breaker.json**: Circuit health SLOs
4. **dashboard_proxy_performance.json**: Latency and availability SLOs
5. **dashboard_system_health.json**: Resource utilization SLOs

---

## SLO Review Cadence

| Review Type | Frequency | Attendees | Action Items |
|-------------|-----------|-----------|--------------|
| Daily SLO Health | Every day | On-call engineer | Investigate violations, file incidents |
| Weekly SLO Review | Every Monday | Engineering team | Identify trends, plan improvements |
| Monthly SLO Report | 1st of month | Engineering + Leadership | Adjust targets, resource planning |
| Quarterly SLO Audit | Every 3 months | All stakeholders | Review targets, revise SLOs |

---

## SLO Budget

**Monthly Error Budget:**

| SLO | Target | Error Budget (per month) | Remaining Budget Calculation |
|-----|--------|-------------------------|----------------------------|
| 99.9% Uptime | 99.9% | 43 minutes downtime | `(43 - actual_downtime) minutes` |
| 99.99% Success | 99.99% | 1 failure per 10,000 requests | `(0.01% - actual_error_rate)` |
| P50 <10ms | 99.5% | 0.5% violations | `(0.5% - actual_violations)` |

**Error Budget Policy:**

- **>50% budget remaining:** Continue feature development
- **25-50% budget remaining:** Focus on reliability improvements
- **<25% budget remaining:** Freeze non-critical features, focus on SLO recovery
- **0% budget exhausted:** Incident declared, all hands on deck

---

## SLO Incident Response

When an SLO is violated:

1. **Alert fires** (via AlertManager → PagerDuty/Slack)
2. **On-call engineer investigates** (using runbook)
3. **Incident declared** (if violation persists >5 minutes)
4. **Root cause analysis** (post-incident review within 24 hours)
5. **SLO budget updated** (track remaining budget)

---

## Framework Compliance

✅ **I20 Integration:** All integration points monitored via SLOs
✅ **UCE34 Q30-Q32:** Production monitoring with clear targets
✅ **T28 Q28:** System observability validated via SLO tracking
✅ **B32 Benchmarking:** Latency targets based on honest performance measurements

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-10-19 | Monitoring Expert | Initial SLO definition |

---

## References

- [Prometheus Metrics](/metrics)
- [Grafana Dashboards](./grafana_dashboards/)
- [Alert Rules](./alerter.yml)
- [Incident Playbooks](./playbooks/)
