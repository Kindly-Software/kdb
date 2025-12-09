# Production Deployment Runbook - P3 Enhancements
**Version**: 1.0
**Date**: October 22, 2025
**Target**: clapi_core v0.5.0
**Deployment Type**: I20-Capsule (Big Bang 100%)

---

## Table of Contents

1. [Pre-Deployment Checklist](#pre-deployment-checklist)
2. [Phase 1: Immediate Deployment](#phase-1-immediate-deployment-high-confidence)
3. [Phase 2: Post-Validation Deployment](#phase-2-post-validation-deployment-medium-high-confidence)
4. [Phase 3: Final Deployment](#phase-3-final-deployment-medium-confidence)
5. [Monitoring & Success Metrics](#monitoring--success-metrics)
6. [Rollback Procedures](#rollback-procedures)
7. [Post-Deployment Validation](#post-deployment-validation)

---

## Pre-Deployment Checklist

**Completion Required**: ALL items must be checked before proceeding

### Code Quality
```bash
- [ ] All 252+ tests passing (100%)
      cargo test --lib --no-default-features --features proxy-only -- --test-threads=1

- [ ] Zero compilation errors
      cargo build --release --no-default-features --features proxy-only

- [ ] Clippy verification clean (0 critical warnings)
      cargo clippy --no-default-features --features proxy-only -- -D warnings

- [ ] Capsule verification complete (80 automatic + 8 manual)
      grep -r "#\[derive(ComputationalCapsule)\]" src/ | wc -l
      # Expected: 80
```

### Performance Validation
```bash
- [ ] All benchmarks meet targets
      cargo bench --bench p3_e7_health_check
      cargo bench --bench p3_e8_cache_performance
      cargo bench --bench p3_e4_config_reload
      # Expected: All <target latency

- [ ] SIGABRT workaround documented (if applicable)
      # See P3_TROUBLESHOOTING.md for single-threaded test workaround
```

### Configuration
```bash
- [ ] Feature flags configured (if any)
      # For P3: No feature flags needed (deterministic capsules)

- [ ] Environment variables set
      export CLAPI_CONFIG=/opt/clapi/config.toml
      export CLAPI_LOG_LEVEL=info
      export CLAPI_PORT=8080
```

### Infrastructure
```bash
- [ ] Monitoring alerts configured
      # Prometheus alert rules in config/alert_rules.yml

- [ ] Kubernetes manifests validated
      kubectl apply --dry-run=client -f k8s/hpa.yaml
      kubectl apply --dry-run=client -f k8s/pdb.yaml

- [ ] Grafana dashboards imported
      # Import dashboards/clapi-overview.json
```

### Communication
```bash
- [ ] Rollback plan reviewed
      # See "Rollback Procedures" section below

- [ ] Stakeholders notified
      # Email: team@clapi.dev
      # Slack: #clapi-deployments

- [ ] On-call engineer identified
      # Name: __________________
      # Phone: _________________
```

---

## Phase 1: Immediate Deployment (High Confidence)

**Features**: E7 (Health), E4 (Config), E8 (Cache), E6/E11 (Infrastructure)
**Deployment Approach**: Big Bang 100% (I20-Capsule pattern)
**Estimated Time**: 15-30 minutes
**Risk Level**: MINIMAL
**Confidence**: 99%

### Step 1: Build Release Binary (5 minutes)

```bash
# Navigate to project directory
cd /home/samuel/Primitives/clapi_core

# Clean previous builds
cargo clean

# Build release binary with production features
cargo build --release --no-default-features --features proxy-only

# Verify binary
ls -lh target/release/clapi
# Expected: ~5-10MB binary

# Run quick sanity check
./target/release/clapi --version
# Expected: clapi 0.5.0
```

**Success Criteria**: Binary builds without errors, version matches v0.5.0

### Step 2: Run Final Validation (5 minutes)

```bash
# Run all Phase 1 tests
cargo test --lib --no-default-features --features proxy-only -- \
  --test-threads=1 \
  p3_e7_health_check_tests \
  p3_e4_config_tests \
  p3_e8_response_cache_tests \
  infrastructure_tests

# Expected output:
# test result: ok. 96 passed; 0 failed; 0 ignored

# Run Phase 1 benchmarks
cargo bench --bench p3_e7_health_check
cargo bench --bench p3_e4_config_reload
cargo bench --bench p3_e8_cache_performance

# Expected: All benchmarks meet targets
# - Health check read: <20ns
# - Config reload: <15ns
# - Cache hit: <30ns
```

**Success Criteria**: 96/98 tests passing (98%), all benchmarks meet targets

### Step 3: Deploy to Staging (5 minutes)

```bash
# Copy binary to staging server
scp target/release/clapi staging:/opt/clapi/clapi.new

# SSH to staging
ssh staging

# Backup current binary
sudo cp /opt/clapi/clapi /opt/clapi/clapi.backup

# Replace with new binary
sudo mv /opt/clapi/clapi.new /opt/clapi/clapi
sudo chmod +x /opt/clapi/clapi

# Restart service
sudo systemctl restart clapi

# Check service status
sudo systemctl status clapi
# Expected: active (running)
```

**Success Criteria**: Service restarts successfully, status shows "active (running)"

### Step 4: Verify Staging (10 minutes)

```bash
# Basic health check
curl http://staging:8080/health
# Expected: {"status":"healthy","components":[...]}

# Deep health check (all components)
curl "http://staging:8080/health?deep=true"
# Expected: All components show "healthy" status

# Verify metrics endpoint
curl http://staging:8080/metrics
# Expected: Prometheus text format
# HELP clapi_requests_total Total number of requests
# TYPE clapi_requests_total counter
# clapi_requests_total 0

# Test cache functionality (100 requests)
for i in {1..100}; do
  curl -s -X POST http://staging:8080/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{"model":"gpt-4","messages":[{"role":"user","content":"test '${i}'"}]}' \
    > /dev/null
done

# Check cache hit rate
curl -s http://staging:8080/metrics | grep clapi_cache_hits_total
# Expected: 15-20 hits (15-20% hit rate)

# Test config reload
curl -X POST http://staging:8080/admin/reload-config
# Expected: {"status":"success","reloaded":true}

# Verify Prometheus metrics export
curl -s http://staging:8080/metrics | grep -E "clapi_(requests|cache|health)"
# Expected: All metrics present
```

**Success Criteria**:
- ✅ Health endpoint returns 200 OK
- ✅ All components "healthy"
- ✅ Metrics endpoint works
- ✅ Cache hit rate 15-20%
- ✅ Config reload works

### Step 5: Deploy to Production (5 minutes)

```bash
# Copy binary from local machine to production
scp target/release/clapi production:/opt/clapi/clapi.new

# SSH to production
ssh production

# Backup current binary
sudo cp /opt/clapi/clapi /opt/clapi/clapi.backup

# Replace with new binary
sudo mv /opt/clapi/clapi.new /opt/clapi/clapi
sudo chmod +x /opt/clapi/clapi

# Restart service
sudo systemctl restart clapi

# Check service status
sudo systemctl status clapi
# Expected: active (running)
```

**Success Criteria**: Service restarts successfully in production

### Step 6: Monitor Production (30 minutes)

```bash
# Watch metrics (refresh every 5 seconds)
watch -n 5 'curl -s http://production:8080/metrics | \
  grep -E "(clapi_requests_total|clapi_cache_hits|clapi_health)"'

# Watch health (refresh every 10 seconds)
watch -n 10 'curl -s http://production:8080/health'

# Watch logs in real-time
ssh production "sudo journalctl -u clapi -f"

# Watch CPU/memory usage
ssh production "top -b -n 1 | grep clapi"
# Expected: CPU <50%, Memory <512MB
```

**Success Criteria** (30-minute validation):
- ✅ Error rate <0.01%
- ✅ P99 latency <100ms
- ✅ Cache hit rate 15-20%
- ✅ All health checks passing
- ✅ CPU <50%, Memory <512MB
- ✅ No crashes or SIGABRT

### Rollback (if needed, <5 minutes)

If any issues detected during monitoring:

**Option 1: Revert to Backup Binary**
```bash
ssh production
sudo cp /opt/clapi/clapi.backup /opt/clapi/clapi
sudo systemctl restart clapi
```

**Option 2: Git Revert**
```bash
cd /home/samuel/Primitives/clapi_core
git revert ada2ce3
cargo build --release --no-default-features --features proxy-only
scp target/release/clapi production:/opt/clapi/
ssh production "sudo systemctl restart clapi"
```

---

## Phase 2: Post-Validation Deployment (Medium-High Confidence)

**Features**: E1 (Tracing), E5 (Capacity), E9 (Dedup)
**Prerequisites**: 100% test validation complete (single-threaded run)
**Deployment Approach**: Big Bang 100% (after validation)
**Estimated Time**: 1-2 hours (including validation)
**Risk Level**: LOW
**Confidence**: 90%

### Step 1: Complete Test Validation (30 minutes)

```bash
# Run single-threaded tests for E1, E5, E9
cargo test --lib --no-default-features --features proxy-only -- \
  --test-threads=1 \
  p3_e1_distributed_tracing_tests \
  p3_operations_integration_tests \
  p3_e9_deduplication_tests

# Expected: 107/107 tests passing (100%)
# E1 Tracing: 46/46 passing
# E5 Capacity: 13/13 passing
# E9 Dedup: 48/48 passing

# If any tests fail: DO NOT PROCEED, investigate failures
```

**Success Criteria**: 107/107 tests passing (100%)

### Step 2-6: Same as Phase 1

Follow the same deployment steps as Phase 1:
- Step 2: Run final validation
- Step 3: Deploy to staging
- Step 4: Verify staging
- Step 5: Deploy to production
- Step 6: Monitor production (30 minutes)

### Additional Verification for Phase 2 Features

```bash
# E1: Verify tracing spans exported
curl -s http://production:8080/metrics | grep clapi_traces_exported_total
# Expected: Increasing count

# E5: Check capacity forecasts in logs
ssh production "sudo journalctl -u clapi -n 100 | grep 'capacity forecast'"
# Expected: "seconds till exhaustion: 86400" (or similar)

# E9: Verify deduplication effectiveness
curl -s http://production:8080/metrics | grep clapi_dedup_effectiveness
# Expected: 5-10% reduction (e.g., "0.07" = 7%)
```

**Success Criteria** (Phase 2 specific):
- ✅ Tracing spans exported to OTLP endpoint
- ✅ Capacity forecasts in logs (realistic predictions)
- ✅ Dedup effectiveness 5-10%
- ✅ All Phase 1 success criteria still met

---

## Phase 3: Final Deployment (Medium Confidence)

**Features**: E2/E3 (Anomaly Detection)
**Prerequisites**: Distribution test fixes applied (10 tests fixed)
**Deployment Approach**: Big Bang 100% (after fixes)
**Estimated Time**: 1-2 hours (including fixes)
**Risk Level**: MEDIUM
**Confidence**: 85%

### Step 1: Fix Distribution Tests (1-2 hours)

```bash
# Open test file
vim tests/p3_e2_anomaly_tests.rs

# Replace uniform distribution with bell curve
# Find all instances of:
#   let latencies: Vec<f64> = (0..N).map(|_| rng.gen_range(0.0..1000.0)).collect();
#
# Replace with:
#   let latencies = generate_realistic_distribution(N, 100.0, 20.0);

# Save and run tests
cargo test --lib --test p3_e2_anomaly_tests -- --test-threads=1

# Expected: 48/48 tests passing (was 29/48)
```

**Success Criteria**: 48/48 anomaly tests passing (100%)

### Step 2-6: Same as Phase 1

Follow the same deployment steps as Phase 1

### Additional Verification for Phase 3 Features

```bash
# E2/E3: Verify anomalies detected
ssh production "sudo journalctl -u clapi -n 100 | grep 'anomaly detected'"
# Expected: Low count (few anomalies)

# Verify SIMD percentile performance
cargo bench --bench p3_e2_anomaly_detection
# Expected: 2.5× faster than scalar

# Verify metrics registry
curl -s http://production:8080/metrics | grep clapi_anomaly_detected_total
# Expected: Low count (system is healthy)
```

**Success Criteria** (Phase 3 specific):
- ✅ Anomalies detected (low count expected)
- ✅ SIMD percentile 2.5× faster
- ✅ Metrics registry exports all metrics
- ✅ All Phase 1 & Phase 2 success criteria still met

---

## Monitoring & Success Metrics

### Health Checks (Continuous Monitoring)

**Liveness Check** (any component working):
```bash
curl http://production:8080/health
# Expected: HTTP 200, {"status":"healthy"}
```

**Readiness Check** (all critical components healthy):
```bash
curl "http://production:8080/health?deep=true"
# Expected: All components "healthy"
```

**Monitoring Script** (run continuously):
```bash
#!/bin/bash
# health_monitor.sh
while true; do
  STATUS=$(curl -s http://production:8080/health | jq -r '.status')
  if [ "$STATUS" != "healthy" ]; then
    echo "[ALERT] Health check failed: $STATUS"
    # Trigger alert (email, Slack, PagerDuty)
  fi
  sleep 30
done
```

### Metrics (Prometheus Format)

**Request Metrics**:
```bash
# Total requests
curl -s http://production:8080/metrics | grep clapi_requests_total
# Expected: Steadily increasing

# Request latency (percentiles)
curl -s http://production:8080/metrics | grep clapi_request_latency_us
# Expected:
# clapi_request_latency_us{quantile="0.5"} 45000  # p50 = 45μs
# clapi_request_latency_us{quantile="0.95"} 80000  # p95 = 80μs
# clapi_request_latency_us{quantile="0.99"} 95000  # p99 = 95μs
```

**Cache Metrics (E8)**:
```bash
curl -s http://production:8080/metrics | grep clapi_cache
# Expected:
# clapi_cache_hits_total 150        # Cache hits
# clapi_cache_misses_total 850      # Cache misses
# clapi_cache_hit_rate 0.15         # 15% hit rate
```

**Deduplication Metrics (E9)**:
```bash
curl -s http://production:8080/metrics | grep clapi_dedup
# Expected:
# clapi_dedup_requests_total 1000   # Total deduplicated
# clapi_dedup_saved_total 70        # Requests saved
# clapi_dedup_effectiveness 0.07    # 7% reduction
```

**Anomaly Metrics (E2/E3)**:
```bash
curl -s http://production:8080/metrics | grep clapi_anomaly
# Expected:
# clapi_anomaly_detected_total{severity="low"} 5
# clapi_anomaly_detected_total{severity="medium"} 2
# clapi_anomaly_detected_total{severity="high"} 0
# clapi_anomaly_detected_total{severity="critical"} 0
```

### Success Metrics Table

| Metric | Target | Threshold | Action |
|--------|--------|-----------|--------|
| **Error Rate** | <0.01% | >0.1% | Alert + rollback if sustained 5 min |
| **P99 Latency** | <100ms | >150ms | Warning if sustained 10 min |
| **Cache Hit Rate** | 15-20% | <10% | Warning, investigate |
| **Dedup Effectiveness** | 5-10% | <3% | Warning, investigate |
| **Health Checks** | 100% pass | <99% | Alert immediately |
| **CPU Usage** | <50% | >80% | Warning, investigate |
| **Memory RSS** | <512MB | >1GB | Warning, check for leaks |
| **Crashes** | 0 | Any SIGABRT | Alert + immediate rollback |

### Monitoring Dashboard (Grafana)

Import dashboard: `dashboards/clapi-overview.json`

**Panels**:
1. Request rate (requests/sec)
2. Latency percentiles (p50/p95/p99)
3. Error rate (%)
4. Cache hit rate (%)
5. Dedup effectiveness (%)
6. Anomalies detected (count by severity)
7. Health check status (green/red)
8. CPU usage (%)
9. Memory usage (MB)

---

## Rollback Procedures

### Immediate Rollback (Critical Issue)

**Trigger**: Error rate >0.1% sustained OR any crashes/SIGABRT

**Time**: <5 minutes

**Option 1: Binary Rollback** (fastest, 2 minutes)
```bash
ssh production
sudo cp /opt/clapi/clapi.backup /opt/clapi/clapi
sudo systemctl restart clapi
sudo systemctl status clapi
# Expected: active (running)

# Verify rollback success
curl http://production:8080/health
# Expected: HTTP 200
```

**Option 2: Git Revert** (slower, 5 minutes)
```bash
cd /home/samuel/Primitives/clapi_core
git revert ada2ce3
cargo build --release --no-default-features --features proxy-only
scp target/release/clapi production:/opt/clapi/
ssh production "sudo systemctl restart clapi"
```

### Partial Rollback (Specific Feature Issue)

**Trigger**: Specific feature causing problems (e.g., cache, dedup)

**Note**: P3 features use I20-Capsule pattern (no feature flags). Partial rollback requires code changes.

**Workaround**:
```bash
# Temporarily disable problematic feature
ssh production
sudo vim /opt/clapi/config.toml

# Example: Disable cache
# [cache]
# enabled = false

sudo systemctl restart clapi
```

**Permanent Fix**: Deploy code with feature disabled/fixed

### Validation After Rollback

```bash
# Verify health
curl http://production:8080/health
# Expected: HTTP 200

# Verify error rate dropped
curl -s http://production:8080/metrics | grep clapi_errors_total
# Expected: Low count, not increasing

# Verify latency normalized
curl -s http://production:8080/metrics | grep clapi_request_latency_us
# Expected: p99 <100ms
```

**Success Criteria**:
- ✅ Error rate <0.01%
- ✅ P99 latency <100ms
- ✅ No crashes
- ✅ Health checks passing

---

## Post-Deployment Validation

### Hour 1: Continuous Monitoring

```bash
# Set up continuous monitoring
watch -n 10 'curl -s http://production:8080/health'
watch -n 30 'curl -s http://production:8080/metrics | grep -E "(error|latency|health)"'
ssh production "sudo journalctl -u clapi -f"
```

**Checklist** (every 10 minutes):
- [ ] Health check: HTTP 200 OK
- [ ] Error rate: <0.01%
- [ ] P99 latency: <100ms
- [ ] No crashes in logs
- [ ] CPU <50%, Memory <512MB

### Hour 6: First Checkpoint

```bash
# Generate 6-hour report
curl -s http://production:8080/metrics > metrics_6h.txt

# Analyze error rate
grep clapi_errors_total metrics_6h.txt
# Expected: Very low count

# Analyze latency
grep clapi_request_latency_us metrics_6h.txt | grep quantile=\"0.99\"
# Expected: <100ms

# Analyze cache hit rate
grep clapi_cache_hit_rate metrics_6h.txt
# Expected: 15-20%

# Analyze dedup effectiveness
grep clapi_dedup_effectiveness metrics_6h.txt
# Expected: 5-10%
```

**First Checkpoint Criteria**:
- ✅ Error rate <0.01% sustained 6 hours
- ✅ P99 latency <100ms sustained 6 hours
- ✅ Cache hit rate 15-20%
- ✅ Dedup effectiveness 5-10%
- ✅ No crashes or memory leaks
- ✅ No customer complaints

### Hour 24: Final Validation

```bash
# Generate 24-hour report
curl -s http://production:8080/metrics > metrics_24h.txt

# Compare with baseline (before deployment)
diff metrics_baseline.txt metrics_24h.txt
# Expected: No regressions, improvements in cache/dedup
```

**Final Validation Criteria**:
- ✅ Error rate <0.01% sustained 24 hours
- ✅ P99 latency <100ms sustained 24 hours
- ✅ Cache hit rate stable 15-20%
- ✅ Dedup effectiveness stable 5-10%
- ✅ No crashes or memory leaks
- ✅ No customer regressions reported
- ✅ All metrics stable (no anomalies)

### Success Declaration (After 24 Hours)

If all validation criteria met:

```bash
# Tag release
cd /home/samuel/Primitives/clapi_core
git tag -a v0.5.0-p3-complete -m "P3 enhancements complete and validated in production"
git push origin v0.5.0-p3-complete

# Create GitHub release
gh release create v0.5.0-p3-complete \
  --title "v0.5.0 - P3 Enhancements Complete" \
  --notes-file DEPLOYMENT_SUCCESS_NOTES.md

# Announce success
cat <<EOF | mail -s "P3 Deployment Success" team@clapi.dev
P3 deployment complete and validated.

All 11 features deployed successfully:
- Health Check (E7)
- Config Reload (E4)
- Response Cache (E8)
- Infrastructure (E6/E11)
- Distributed Tracing (E1)
- Capacity Planning (E5)
- Deduplication (E9)
- Anomaly Detection (E2/E3)

Metrics:
- Error rate: <0.01%
- P99 latency: <100ms
- Cache hit rate: 15-20%
- Dedup effectiveness: 5-10%
- 24 hours stable, zero issues

Status: SUCCESS ✅
EOF

# Archive deployment runbook with timestamps
cp DEPLOYMENT_RUNBOOK_P3.md DEPLOYMENT_RUNBOOK_P3_EXECUTED_$(date +%Y%m%d).md
```

---

## Emergency Contacts

**On-Call Engineer**: [Name], [Phone], [Email]
**Backup Engineer**: [Name], [Phone], [Email]
**Team Lead**: [Name], [Email]
**PagerDuty**: [URL]
**Slack Channel**: #clapi-deployments

---

## Appendix: Common Issues & Solutions

### Issue: Health Check Fails

**Symptom**: `curl http://production:8080/health` returns 500 or times out

**Diagnosis**:
```bash
ssh production "sudo journalctl -u clapi -n 100"
# Look for error messages
```

**Solution**:
1. Check if service is running: `sudo systemctl status clapi`
2. Restart service: `sudo systemctl restart clapi`
3. If still failing: Rollback to previous binary

### Issue: High Latency (P99 >150ms)

**Symptom**: Metrics show P99 latency >150ms

**Diagnosis**:
```bash
# Check CPU usage
ssh production "top -b -n 1 | grep clapi"

# Check for lock contention (should be zero in lockfree code)
ssh production "sudo perf record -p $(pgrep clapi) -g -- sleep 10"
ssh production "sudo perf report"
```

**Solution**:
1. If CPU high: Scale horizontally (add more instances)
2. If lock contention: Report as bug (shouldn't happen with lockfree capsules)
3. If neither: Investigate provider latency (upstream issue)

### Issue: Memory Leak (RSS Increasing)

**Symptom**: Memory usage increasing over time

**Diagnosis**:
```bash
# Monitor RSS over time
ssh production "watch -n 60 'ps aux | grep clapi | grep -v grep'"
```

**Solution**:
1. If leak confirmed: Immediate rollback
2. Report as critical bug (shouldn't happen with Rust's ownership model)
3. Investigate with valgrind or heaptrack in dev environment

---

**Runbook Version**: 1.0
**Last Updated**: October 22, 2025
**Maintained By**: clapi_core Team
**Review Cycle**: Every release
