# AnomalyRouter - Shadow Mode Deployment Guide

T6 Mixed orchestration for safe V1 to V2 anomaly detection migration.

## Overview

AnomalyRouter provides a shadow mode deployment pattern for safely migrating from V1 (`AnomalyDetectorCapsule`) to V2 (`AnomalyDetectorV2`). It tracks agreement rates, latency differences, and provides deployment recommendations.

## Quick Start

```rust
use atomic_capsule::protection::{
    AnomalyDetectorCapsule, AnomalyDetectorV2,
    AnomalyRouter, RouterMode, ShadowMetrics,
};

// Create detectors
let v1 = AnomalyDetectorCapsule::new();
let mut v2 = AnomalyDetectorV2::new();

// Initialize both with baseline
let baseline: Vec<u64> = (0..100).map(|i| 1000 + i).collect();
v1.init(&baseline).unwrap();
v2.init(&baseline).unwrap();

// Create router (starts in V1Only mode)
let router = AnomalyRouter::new();

// Enable shadow mode for comparison
router.set_mode(RouterMode::Shadow);

// Check behaviors - V1 decides, V2 runs in parallel
let result = router.check_behavior(&v1, &v2, 1050, 0);

// Check metrics
let metrics = router.metrics();
println!("Agreement rate: {:.2}%", metrics.agreement_rate() * 100.0);
println!("V1 avg latency: {}ns", metrics.avg_v1_latency_ns());
println!("V2 avg latency: {}ns", metrics.avg_v2_latency_ns());
```

## Deployment Modes

### V1Only (Default)
Production-proven detector. Zero overhead from V2.

```rust
router.set_mode(RouterMode::V1Only);
let result = router.check_v1_only(&v1, behavior);
```

### Shadow Mode
V2 runs in parallel, V1 decides. Discrepancies are logged for analysis.

```rust
router.set_mode(RouterMode::Shadow);
let result = router.check_shadow(&v1, &v2, behavior, timestamp_ms);
// result is always from V1
// metrics track V2 performance and agreement
```

### Hybrid Mode
Weighted combination of V1 and V2 decisions. Use for gradual rollout.

```rust
router.set_mode(RouterMode::Hybrid);
router.set_hybrid_weight(80); // 80% V1, 20% V2

let result = router.check_hybrid(&v1, &v2, behavior, timestamp_ms);
```

### V2Only
Full migration after validation.

```rust
router.set_mode(RouterMode::V2Only);
let result = router.check_v2_only(&v2, behavior, timestamp_ms);
```

## Metrics Export

### JSON Export
```rust
let json = router.metrics().to_json();
// Example output:
// {
//   "total_checks": 10000,
//   "agreement": {"normal": 9500, "anomalous": 300},
//   "discrepancies": {"v1_normal_v2_anomalous": 150, "v1_anomalous_v2_normal": 50},
//   "v2_severity": {"critical": 10, "anomalous": 100, "suspicious": 190},
//   "rates": {"agreement": 0.98, "discrepancy": 0.02},
//   "latency_ns": {"v1_avg": 50, "v2_avg": 80},
//   "config": {"mode": "shadow", "hybrid_v1_weight": 100},
//   "generation": 10000
// }
```

### Snapshot
```rust
let snapshot = router.metrics().snapshot();
println!("Total checks: {}", snapshot.total_checks);
println!("Agreement rate: {:.2}%", snapshot.agreement_rate * 100.0);
println!("V1 avg latency: {}ns", snapshot.avg_v1_latency_ns);
```

## Deployment Recommendations

The router provides automatic recommendations based on metrics:

```rust
match router.deployment_recommendation() {
    DeploymentRecommendation::NeedMoreData { current, required } => {
        println!("Need {} more samples (have {}/{})", required - current, current, required);
    }
    DeploymentRecommendation::StayShadow { agreement_rate } => {
        println!("Keep in shadow mode, agreement: {:.2}%", agreement_rate * 100.0);
    }
    DeploymentRecommendation::InvestigateDiscrepancies { discrepancy_rate } => {
        println!("High discrepancy rate: {:.2}% - investigate!", discrepancy_rate * 100.0);
    }
    DeploymentRecommendation::HybridRecommended { agreement_rate } => {
        println!("95-99% agreement ({:.2}%), consider hybrid mode", agreement_rate * 100.0);
    }
    DeploymentRecommendation::ReadyForV2Only => {
        println!("V2 validated! Ready for full deployment.");
    }
    DeploymentRecommendation::V2SlowerButReady { latency_ratio } => {
        println!("V2 ready but {:.1}x slower than V1", latency_ratio);
    }
}
```

## Discrepancy Types

| Type | Meaning | Action |
|------|---------|--------|
| `AgreeNormal` | Both V1 and V2 say normal | Good - agreement |
| `AgreeAnomalous` | Both V1 and V2 say anomalous | Good - agreement |
| `V1NormalV2Anomalous` | V1 says normal, V2 says anomalous | V1 may have false negative |
| `V1AnomalousV2Normal` | V1 says anomalous, V2 says normal | V1 may have false positive |

## Performance Targets

| Mode | Target Latency | Description |
|------|----------------|-------------|
| V1Only | <100ns | Single detector fast path |
| Shadow | <200ns | Both detectors run |
| Hybrid | <150ns | Weighted selection |
| V2Only | <100ns | Single detector fast path |

## Migration Checklist

1. **Deploy shadow mode** - Enable `RouterMode::Shadow`
2. **Collect 10,000+ samples** - Wait for statistical significance
3. **Check agreement rate** - Target >99%
4. **Analyze discrepancies** - Investigate V1/V2 disagreements
5. **Deploy hybrid (optional)** - Gradual rollout with 80/20 weight
6. **Full migration** - Switch to `RouterMode::V2Only`

## Feature Flags

```toml
[dependencies.atomic_capsule]
version = "0.9"
features = ["std", "anomaly-v2", "anomaly-detection"]
```

Required features:
- `anomaly-v2`: Enables AnomalyRouter and V2 detector
- `anomaly-detection`: Enables V1 detector

## Thread Safety

AnomalyRouter is 100% lockfree:
- Mode switching: AtomicU8 with Acquire/Release ordering
- Metrics counters: AtomicU64 with Relaxed ordering (informational)
- Generation counter: AtomicU64 with SeqCst ordering (for snapshots)

## Memory Layout

```
AnomalyRouter (512 bytes, 512-byte aligned):
+------------------------+
| ShadowMetrics (256B)   |
|   - Agreement counters |
|   - Latency tracking   |
|   - Severity counts    |
|   - Configuration      |
+------------------------+
| Padding (256B)         |
+------------------------+
```

## ASSUM Safety

- `#ASSUME_ATOMIC_MODE_SWITCH`: Mode changes are atomic
- `#ASSUME_COUNTER_OVERFLOW_SAFE`: u64 counters won't overflow in practice
- `#ASSUME_V1_STABLE`: V1 detector is production-proven baseline
- `#ASSUME_LOCKFREE_METRICS`: All metrics use Relaxed ordering
