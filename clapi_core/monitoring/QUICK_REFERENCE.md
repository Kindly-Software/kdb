# clapi_core Monitoring - On-Call Quick Reference

## 🚨 Critical Alerts (PagerDuty)

| Alert | Threshold | Playbook | First Action |
|-------|-----------|----------|--------------|
| **ProxyLatencyP50Exceeded** | P50 >15ms | latency_spike_playbook.md | Check provider health |
| **AllCircuitsOpen** | 16 circuits open | all_circuits_open_playbook.md | **EMERGENCY** - Check provider status pages |
| **HighErrorRate** | Error rate >5% | high_error_rate_playbook.md | Identify error source (provider/budget/oauth) |
| **ServiceDown** | No metrics >1min | service_down_playbook.md | Restart service immediately |
| **MemoryLeak** | >100MB/hour | memory_leak_playbook.md | Hot restart service |
| **HighCPUUsage** | CPU >90% | cpu_saturation_playbook.md | Scale horizontally |
| **OAuthFailureRate** | >10% failures | oauth_failure_playbook.md | Check KindlyDB connection |
| **PaymentFailureRate** | >5% failures | payment_failure_playbook.md | Check Stripe webhook |
| **BudgetExhaustionRate** | >50% exhausted | budget_exhaustion_playbook.md | Increase budgets or add refill |

## ⚠️ Warning Alerts (Slack)

| Alert | Threshold | Action |
|-------|-----------|--------|
| **SomeCircuitsOpen** | >5 circuits open | Monitor, may escalate to critical |
| **ProxyLatencyP50Trending** | P50 trending up >20% | Investigate before SLO violation |
| **ModerateBudgetExhaustion** | >25% exhausted | Plan budget increase |
| **HighContention** | CAS retry >10% | Consider horizontal scaling |
| **ThermalThrottling** | CPU frequency reduced | Check cooling/temperature |

## 📊 Key Metrics Endpoints

```bash
# All metrics
curl http://localhost:8080/metrics

# Circuit breaker only
curl http://localhost:8080/metrics/circuit_breaker

# Budget metrics
curl http://localhost:8080/metrics/budget

# Health check
curl http://localhost:8080/health
```

## 🎯 SLO Targets (Quick Check)

| SLO | Target | Query |
|-----|--------|-------|
| **P50 Latency** | <10ms | `histogram_quantile(0.50, rate(clapi_proxy_latency_ns_bucket[1m]))` |
| **P99 Latency** | <100ms | `histogram_quantile(0.99, rate(clapi_proxy_latency_ns_bucket[1m]))` |
| **Availability** | >99.9% | `avg_over_time(up{job="clapi_core"}[30d]) * 100` |
| **Error Rate** | <0.1% | `(rate(clapi_proxy_errors_total[1h]) / rate(clapi_proxy_requests_total[1h])) * 100` |

## 🔧 Common Commands

### Check Service Status
```bash
systemctl status clapi_core
journalctl -u clapi_core -n 100 --since "5 minutes ago"
```

### Restart Service
```bash
# Hot restart (zero downtime)
systemctl reload clapi_core

# Cold restart
systemctl restart clapi_core
```

### Check Circuit States
```bash
curl http://localhost:8080/metrics/circuit_breaker | grep clapi_circuit_state
# 0 = Closed (healthy)
# 1 = HalfOpen (monitoring)
# 2 = Open (failing)
```

### Check Budget Exhaustion
```bash
curl http://localhost:8080/metrics | grep -E "(budget_exhausted_count|budget_active_count)"
```

### Check Memory/CPU
```bash
curl http://localhost:8080/metrics | grep -E "(memory_bytes|cpu_usage_percent)"
```

## 📱 Escalation

1. **Level 1**: On-call engineer (PagerDuty)
2. **Level 2**: Engineering lead (if unresolved in 30 minutes)
3. **Level 3**: CTO (if customer-facing impact)

**Slack**: #clapi-incidents
**Email**: oncall@clapi.example.com
**Phone**: PagerDuty escalation policy

## 🌐 External Resources

- **Anthropic Status**: https://status.anthropic.com
- **OpenAI Status**: https://status.openai.com
- **Google Cloud Status**: https://status.cloud.google.com
- **Grafana**: http://localhost:3000
- **Prometheus**: http://localhost:9090

## 📖 Documentation

- **Full playbooks**: `/home/samuel/Primitives/clapi_core/monitoring/playbooks/`
- **SLO definition**: `/home/samuel/Primitives/clapi_core/monitoring/slo.md`
- **README**: `/home/samuel/Primitives/clapi_core/monitoring/README.md`

---

**Keep this reference card handy during on-call shifts!**

**Last Updated**: 2025-10-19
