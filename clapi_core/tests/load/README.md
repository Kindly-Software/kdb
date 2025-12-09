# Load Testing Framework

Comprehensive production-grade load tests for clapi_core.

## Quick Start

```bash
# Run all load tests (WARNING: Takes 1-2 hours)
cargo test --all-features -- --ignored load_test

# Run specific test suite
cargo test --test budget_registry_load_test --all-features -- --ignored

# Run with progress output
cargo test scenario1_baseline_e2e_performance --all-features -- --ignored --nocapture
```

## Test Suites

| Suite | File | Scenarios | Focus |
|-------|------|-----------|-------|
| Budget Registry | `budget_registry_load_test.rs` | 5 | 10K-100K budgets, const hash |
| Circuit Breaker | `circuit_breaker_load_test.rs` | 4 | State transitions, failover |
| OAuth + Payment | `oauth_payment_load_test.rs` | 8 | Sessions, payments, idempotency |
| Full Stack | `full_stack_load_test.rs` | 4 | End-to-end integration |
| Sustained | `sustained_load_test.rs` | 5 | 5-minute continuous load |
| Stress | `stress_test.rs` | 7 | Pathological scenarios |

**Total**: 33 scenarios across 6 test modules

## Test Framework (`mod.rs`)

Core infrastructure:
- `LoadTestConfig` - Test configuration
- `LoadTestResults` - Statistical results (P50, P95, P99, P999)
- `LatencyCollector` - Thread-safe latency measurement
- `ProgressTracker` - Real-time monitoring
- `LoadTestHarness` - Test orchestration

## Performance Targets

| Component | P50 Target | Expected |
|-----------|-----------|----------|
| Budget operations | <100ns | 60-100ns |
| OAuth verification | <50ns | 30-50ns |
| Circuit breaker | <5ns | 5-10ns |
| Full stack | <10ms | 1-10ms |

## Hardware Requirements

**Recommended**:
- CPU: 16+ cores
- RAM: 16GB+
- Cooling: Active (for sustained tests)
- OS: Linux

**Monitoring**:
- CPU temp: `sensors`
- CPU freq: `watch -n 1 grep MHz /proc/cpuinfo`
- Memory: `htop`

## Results

See `/home/samuel/Primitives/clapi_core/LOAD_TEST_RESULTS.md` for comprehensive results documentation.

## Framework Compliance

✅ T28 Q22-Q28 (Production testing)
✅ B32 (Statistical rigor, fair baselines)
✅ UCE34 Q30-Q32 (Production deployment validation)
