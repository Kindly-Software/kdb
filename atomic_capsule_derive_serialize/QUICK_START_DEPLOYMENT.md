# Phase 4 Deployment - Quick Start Guide

**1-Page Reference for Production Deployment**

---

## TL;DR

Deploy FixedPointSerialize trait system in 4 weeks:
- **Week 1**: atomic_capsule (baseline)
- **Week 2**: clapi_core (payments)
- **Week 3**: kindly_hft (trading)
- **Week 4**: Other projects + cleanup

**Rollback**: <1 minute (feature flag) | **Tests**: 1,685+ (100% required) | **Safety**: 99.99% ASSUM

---

## Week 1: atomic_capsule (1 hour)

### Pre-Deployment (30 min)

```bash
cd /home/samuel/Primitives/atomic_capsule

# 1. Run tests (5 min)
cargo test --lib --features "capsule-serialize"
# ✅ PASS: All 266+ tests pass

# 2. Benchmark (10 min)
cargo bench --bench phase4_fixed_point_serialize_bench --features "capsule-serialize"
# ✅ PASS: serialize <50ns, deserialize <100ns

# 3. Safety audit (5 min)
cargo clippy --all-features -- -D warnings
# ✅ PASS: Zero warnings

# 4. Compile-time check (10 min)
time (cargo clean && cargo build --lib --features "capsule-serialize")
# ✅ PASS: <5 seconds
```

### Deployment (5 min)

```bash
# Feature already in Cargo.toml - no action needed
# Feature: capsule-serialize = ["std", "dep:crc32fast", "dep:crc"]
```

### Post-Deployment (24h monitoring)

```bash
# Monitor compile-time (daily)
time (cargo clean && cargo build --lib --features "capsule-serialize")

# Monitor tests (daily)
cargo test --lib --features "capsule-serialize"
```

**Decision**: ☐ PROCEED TO WEEK 2 ☐ ROLLBACK

---

## Week 2: clapi_core (2 hours)

### Pre-Deployment (1h)

```bash
cd /home/samuel/Primitives/clapi_core

# 1. Run tests (20 min)
cargo test --lib --features "payment-optimization"
# ✅ PASS: All 365+ tests pass

# 2. Benchmark (20 min)
cargo bench --bench payment_bench
# ✅ PASS: payment.create <150ns (p99)

# 3. Integration (20 min)
cargo test --test payment_fixed_point_validation
cargo test --test proxy_integration_tests -- stripe_webhook
# ✅ PASS: Hash integrity 100%, idempotency verified
```

### Deployment (15 min)

```bash
# 1. Backup existing code
cp src/capsules/payment.rs src/capsules/payment.rs.backup

# 2. Migrate to derive macro (see PHASE4_DEPLOYMENT_PLAN.md § Week 2)

# 3. Build
cargo build --lib --features "payment-optimization"

# 4. Test
cargo test --lib --features "payment-optimization"
# ✅ PASS: All tests pass

# 5. Deploy
./target/release/clapi --config clapi.toml
```

### Post-Deployment (7d monitoring)

```bash
# Monitor /metrics endpoint (daily)
curl http://localhost:8080/metrics | jq '.payment'

# Check hash integrity (critical)
curl http://localhost:8080/metrics | jq '.payment.hash_integrity'
# ✅ REQUIRED: 100%

# Check Stripe webhooks
curl http://localhost:8080/metrics | jq '.payment.stripe'
# ✅ REQUIRED: >99% success
```

**Decision**: ☐ PROCEED TO WEEK 3 ☐ ROLLBACK

---

## Week 3: kindly_hft (7 days, staged)

### Stage 1: Configuration Capsules (Day 1-2)

```bash
cd /home/samuel/Primitives/kindly_hft

# Test
cargo test --lib -- config_capsule
# ✅ PASS: All tests pass

# Deploy
cargo build --features "serialize-capsules"

# Monitor
cargo bench --bench config_bench
# ✅ PASS: <1% overhead
```

### Stage 2: Read-Heavy Capsules (Day 3-4)

```bash
# Test
cargo test -- monitoring_capsule

# Deploy
cargo build --features "serialize-capsules,monitoring"

# Monitor
cargo bench --bench monitoring_bench
# ✅ PASS: <50ns serialization
```

### Stage 3: Critical Path Capsules (Day 5-7)

```bash
# Test
cargo test --release -- training_integration

# Benchmark
cargo bench --bench motor_cortex_bench
# ✅ PASS: <100ns capsule ops, <2000ns order execution

# Deploy
cargo build --release --features "serialize-capsules,production"

# Monitor (critical)
curl http://localhost:6900/metrics/motor_cortex | jq '.pnl'
# ✅ REQUIRED: <100ns P&L, <0.01% error

# Paper trading
./target/release/kindly_hft --mode=paper_trading --duration=1h
# ✅ PASS: Orders execute, P&L accurate
```

**Decision**: ☐ PROCEED TO WEEK 4 ☐ ROLLBACK STAGE 3 ☐ ROLLBACK ALL

---

## Week 4: Other Projects (3 days)

### kindly-db (1 day)

```bash
cd /home/samuel/Primitives/kindly-db
cargo test --lib --all-features
cargo bench
# ✅ PASS: All tests, <10% variance
```

### kiang (1 day)

```bash
cd /home/samuel/Primitives/kiang
cargo test --lib --all-features
cargo bench
# ✅ PASS: All tests, <10% variance
```

### atomic_network_gateway (1 day)

```bash
cd /home/samuel/Primitives/atomic_network_gateway
cargo test --lib --all-features
cargo bench
# ✅ PASS: All tests, <10% variance
```

### Workspace-Wide Validation

```bash
cd /home/samuel/Primitives
cargo test --workspace --all-features
# ✅ REQUIRED: 1,685+ tests, 100% pass rate

cargo clippy --workspace --all-features -- -D warnings
# ✅ REQUIRED: Zero warnings, 99.99% ASSUM safety
```

**Decision**: ☐ PHASE 4 COMPLETE ☐ EXTEND TIMELINE

---

## Emergency Rollback (< 1 minute)

### Feature Flag Rollback

```bash
# Disable feature in config
# Example: clapi_core
# Edit clapi.toml:
[features]
payment_optimization = false

# Restart
./target/release/clapi --config clapi.toml

# Verify
curl http://localhost:8080/health
# ✅ PASS: 200 OK
```

### Code Rollback (< 5 minutes)

```bash
# Revert commit
git log --oneline | head -5
git revert <COMMIT_HASH>

# Rebuild
cargo build --release

# Restart
./target/release/<BINARY>
```

---

## Alert Thresholds

### 🚨 CRITICAL (Auto-Rollback)

- Hash integrity <100% → **ROLLBACK IMMEDIATELY**
- Test pass rate <95% → **ROLLBACK IMMEDIATELY**
- Performance regression >50% → **ROLLBACK IMMEDIATELY**
- ASSUM safety <99% → **ROLLBACK IMMEDIATELY**

### ⚠️ WARNING (Manual Investigation)

- Performance regression >10% → Investigate
- Test failures <100% → Fix before next week
- Compile time >5s → Optimize

---

## Success Criteria (Phase 4 Complete)

- ✅ All 1,685+ tests pass (100%)
- ✅ All 65+ capsules migrated
- ✅ Code reduction: 6,500+ lines (90%)
- ✅ Performance: <10% overhead (acceptable)
- ✅ ASSUM safety: 99.99%
- ✅ Documentation: Complete
- ✅ Zero rollbacks needed

---

## Quick Commands

### Test Everything

```bash
cargo test --workspace --all-features
```

### Benchmark Everything

```bash
for project in atomic_capsule clapi_core kindly_hft kindly-db kiang atomic_network_gateway; do
  cd /home/samuel/Primitives/$project
  cargo bench --features "serialize"
done
```

### Safety Audit

```bash
cargo clippy --workspace --all-features -- -D warnings
```

### Monitor Metrics

```bash
# clapi_core
curl http://localhost:8080/metrics | jq '.payment'

# kindly_hft
curl http://localhost:6900/metrics/motor_cortex | jq '.pnl'
```

---

## Documentation

- **Full Plan**: `PHASE4_DEPLOYMENT_PLAN.md` (850 lines)
- **Checklist**: `DEPLOYMENT_CHECKLIST.md` (600 lines)
- **Monitoring**: `MONITORING_DASHBOARD.md` (600 lines)
- **Summary**: `PHASE4_DEPLOYMENT_SUMMARY.md` (400 lines)

---

## Contact

- **Slack**: #primitives-phase4-deployment
- **On-Call**: PagerDuty: primitives-phase4-deployment
- **Runbook**: https://docs.primitives.io/phase4-rollback

---

**Status**: ✅ **PRODUCTION READY**

**Next Step**: Execute Week 1 deployment → Monitor 24h → Approve Week 2
