# Deployment Scripts - clapi_core

Production-ready automated deployment infrastructure for clapi_core.

## Quick Start

### Pre-Deployment Validation

```bash
# Validate before any deployment
./scripts/pre_deployment_checks.sh
```

### Deploy with Canary (Recommended)

```bash
# Progressive rollout: 1% → 10% → 25% → 50% → 100%
./scripts/deploy_canary.sh
```

### Deploy with Blue-Green (Zero-Downtime)

```bash
# Deploy to inactive instance
./scripts/deploy_blue_green.sh deploy

# Check status
./scripts/deploy_blue_green.sh status
```

### Rollback

```bash
# Standard rollback (<5 min)
./scripts/deploy_rollback.sh

# Emergency rollback (<1 min)
./scripts/deploy_rollback.sh --emergency

# Instant rollback (blue-green)
./scripts/deploy_blue_green.sh rollback
```

### Health Monitoring

```bash
# Continuous monitoring
./scripts/health_check_monitor.sh

# Custom thresholds
LATENCY_WARN_MS=50 LATENCY_ERROR_MS=200 ./scripts/health_check_monitor.sh
```

---

## Scripts Overview

| Script | Purpose | Time | Lines |
|--------|---------|------|-------|
| `pre_deployment_checks.sh` | Automated validation gates | ~2 min | 280 |
| `health_check_monitor.sh` | Continuous health monitoring | Continuous | 242 |
| `deploy_canary.sh` | Progressive rollout | ~7 min | 326 |
| `deploy_blue_green.sh` | Zero-downtime deployment | ~4 min | 424 |
| `deploy_rollback.sh` | Automated rollback | <5 min | 392 |

**Total**: 1,664 lines, 45.4K production automation code

---

## Deployment Strategies

### Canary Deployment

**Use Case**: Safe production rollout with automatic rollback

**Strategy**: 1% → 10% → 25% → 50% → 100%

**Features**:
- 60s validation per stage
- 95% success rate required
- Automatic rollback on failure
- Health + metrics validation

**Timeline**: ~7 minutes (5 stages × 60s + overhead)

```bash
./scripts/deploy_canary.sh
```

---

### Blue-Green Deployment

**Use Case**: Zero-downtime deployment with instant rollback

**Strategy**: Deploy to inactive → Validate → Atomic switch

**Features**:
- <1s switchover time
- Zero downtime
- Preserve old version
- Instant rollback capability

**Timeline**: ~4 minutes (deploy + validate + switch)

```bash
./scripts/deploy_blue_green.sh deploy
```

---

## Rollback Procedures

### Standard Rollback

**Trigger**: Deployment issues detected

**Strategy**: Git revert → Rebuild → Deploy → Verify

**Timeline**: <5 minutes (guaranteed)

```bash
./scripts/deploy_rollback.sh
```

**Steps**:
1. Git rollback to HEAD~1 (~1s)
2. Fast rebuild with incremental compilation (~45s)
3. Stop current instance (~5s)
4. Start rolled-back version (~3s)
5. Health verification (~10s)

**Total**: ~70s typical, <300s guaranteed

---

### Emergency Rollback

**Trigger**: Critical production failure

**Strategy**: Skip non-critical checks for speed

**Timeline**: ~35s

```bash
./scripts/deploy_rollback.sh --emergency
```

---

### Instant Rollback (Blue-Green)

**Trigger**: Immediate recovery needed

**Strategy**: Toggle active/inactive instances

**Timeline**: ~5s

```bash
./scripts/deploy_blue_green.sh rollback
```

---

## Monitoring and Alerting

### Health Check Monitor

**Purpose**: Continuous health validation

**Features**:
- Configurable check interval (default: 10s)
- Latency monitoring (WARN: >100ms, ERROR: >500ms)
- Consecutive failure alerting (threshold: 3)
- Success rate tracking
- Circuit breaker monitoring

```bash
# Monitor local instance
./scripts/health_check_monitor.sh

# Monitor production
HEALTH_ENDPOINT=http://prod.example.com/health ./scripts/health_check_monitor.sh
```

**Output Example**:
```
[10:15:23] Health check OK - Latency: 45ms
[10:15:33] Health check OK - Latency: 52ms
[10:15:43] Health check SLOW (523ms > 500ms threshold)
[10:15:43] ALERT [WARNING]: Health endpoint latency degraded: 523ms
```

---

## Pre-Deployment Validation

### Automated Gates

**Purpose**: Prevent bad deployments from reaching production

**Checks**:
1. ✅ Git status (no uncommitted changes)
2. ✅ Test suite (unit + integration + property)
3. ✅ Clippy (zero warnings with `-D warnings`)
4. ✅ Release build (verify binary)
5. ✅ Performance baseline (P50 < 10ms)
6. ✅ ASSUM safety (validate tags)
7. ✅ Security audit (cargo-audit)

**Output**: Deployment manifest (JSON)

```bash
./scripts/pre_deployment_checks.sh
```

**Exit Codes**:
- `0` = PASS (ready for deployment)
- `1` = FAIL (fix errors before deploying)

---

## Configuration

### Environment Variables

#### Pre-Deployment Checks
```bash
# (No configurable variables - uses project defaults)
```

#### Health Check Monitor
```bash
HEALTH_ENDPOINT=http://localhost:8080/health
METRICS_ENDPOINT=http://localhost:8080/metrics
CHECK_INTERVAL_SEC=10
ALERT_THRESHOLD_FAILURES=3
LATENCY_WARN_MS=100
LATENCY_ERROR_MS=500
```

#### Canary Deployment
```bash
BINARY_PATH=target/release/clapi
SERVICE_NAME=clapi
HEALTH_ENDPOINT=http://localhost:8080/health
METRICS_ENDPOINT=http://localhost:8080/metrics
```

#### Blue-Green Deployment
```bash
BINARY_PATH=target/release/clapi
SERVICE_NAME=clapi
BLUE_PORT=8080
GREEN_PORT=8081
HEALTH_CHECK_TIMEOUT=30
VALIDATION_CHECKS=10
```

#### Rollback
```bash
BINARY_PATH=target/release/clapi
SERVICE_NAME=clapi
ROLLBACK_PORT=8080
ROLLBACK_TIMEOUT_SEC=300
EMERGENCY=false
```

---

## Production Workflow

### Standard Deployment (Canary)

```bash
# 1. Pre-deployment checks
./scripts/pre_deployment_checks.sh

# 2. Start health monitoring (separate terminal)
./scripts/health_check_monitor.sh

# 3. Deploy with canary rollout
./scripts/deploy_canary.sh
```

**Timeline**:
- Pre-deployment: ~2 min
- Stage 1 (1%): 60s
- Stage 2 (10%): 60s
- Stage 3 (25%): 60s
- Stage 4 (50%): 60s
- Stage 5 (100%): 60s
- **Total**: ~7 minutes

---

### Zero-Downtime Deployment (Blue-Green)

```bash
# 1. Pre-deployment checks
./scripts/pre_deployment_checks.sh

# 2. Blue-green deployment
./scripts/deploy_blue_green.sh deploy

# 3. Verify new version
./scripts/deploy_blue_green.sh status

# 4. Monitor for 24 hours

# 5. Stop old version (optional)
# Keep old version for instant rollback
```

**Timeline**:
- Pre-deployment: ~2 min
- Deploy to inactive: ~1 min
- Health check: 30s
- Validation: 10s
- Traffic switch: <1s
- **Total**: ~4 minutes

---

## Framework Compliance

### I20 Integration Framework

✅ **Q19: Deployment Strategy**
- Progressive rollout (canary)
- Zero-downtime (blue-green)
- Health validation
- Metrics monitoring
- Automatic rollback

✅ **Q20: Rollback Plan**
- <5 minute guarantee
- Instant rollback (blue-green)
- Recovery safety (git branches)
- Emergency mode
- Rollback history

### UCE34 Framework

✅ **Q30: Production Deployment**
- Automated gates
- Performance validation
- Security audit
- Deployment manifest

✅ **Q31: Simplicity**
- Pure Bash (no Python/frameworks)
- Standard tools (curl, jq, git, cargo)
- Clear logging (color-coded)
- Single commands

✅ **Q32: Constraints**
- Timing guarantees (<5 min rollback)
- Resource efficiency (incremental builds)
- Production safety (recovery branches)
- Observable (detailed logging)

---

## Timing Guarantees

| Operation | Target | Validated |
|-----------|--------|-----------|
| Canary rollout | ~7 min | ✅ 5 stages × 60s |
| Blue-green deployment | ~4 min | ✅ Deploy + validate |
| Blue-green switchover | <1s | ✅ Atomic switch |
| Standard rollback | <5 min | ✅ Git + rebuild |
| Emergency rollback | <1 min | ✅ Fast path |
| Instant rollback (B/G) | ~5s | ✅ Toggle color |

---

## Troubleshooting

### Pre-Deployment Checks Fail

**Issue**: Uncommitted changes detected

**Solution**: Commit or stash changes before deployment
```bash
git status
git add .
git commit -m "Deploy version X.Y.Z"
```

---

**Issue**: Clippy warnings

**Solution**: Fix all warnings
```bash
cargo clippy --all-features --tests -- -D warnings
```

---

**Issue**: Performance baseline exceeded

**Solution**: Investigate regression
```bash
cargo bench --bench budget_benchmarks
```

---

### Deployment Fails

**Issue**: Health check timeout

**Solution**: Check service logs
```bash
cat /tmp/clapi_canary.log
```

---

**Issue**: Success rate below threshold

**Solution**: Automatic rollback triggered, investigate logs

---

### Rollback Fails

**Issue**: Rebuild timeout

**Solution**: Use emergency mode
```bash
./scripts/deploy_rollback.sh --emergency
```

---

**Issue**: Git rollback fails

**Solution**: Manual recovery
```bash
git log  # Find last good commit
git reset --hard <commit-hash>
```

---

## Production Checklist

### Before Deployment

- [ ] Pre-deployment checks pass
- [ ] Health monitoring configured
- [ ] Alerting thresholds set
- [ ] Rollback plan documented
- [ ] Load balancer configured (for canary)
- [ ] Team notified

### During Deployment

- [ ] Monitor health checks
- [ ] Watch metrics (circuit breaker, error rate)
- [ ] Verify each canary stage
- [ ] Ready to trigger rollback

### After Deployment

- [ ] Monitor for 24 hours
- [ ] Review metrics and logs
- [ ] Stop old version (if stable)
- [ ] Update deployment documentation
- [ ] Record deployment event

---

## Support

For questions or issues:
1. Check troubleshooting section above
2. Review deployment automation report: `DEPLOYMENT_AUTOMATION_REPORT.md`
3. Review validation summary: `DEPLOYMENT_VALIDATION_SUMMARY.md`
4. Check framework docs: I20, UCE34, T28, B32

---

**Last Updated**: 2025-10-19
**Framework**: I20 + UCE34
**Status**: Production-Ready
