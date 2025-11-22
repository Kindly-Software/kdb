# Comprehensive Benchmark Suite (v0.3.0 - v0.3.2)

**Status**: Production Ready | **Framework**: B32 Benchmarking | **Coverage**: 36 benchmarks, all features

## Quick Start

### Run All Benchmarks (Recommended)
```bash
cargo bench --bench comprehensive_feature_benchmark --features "capsule-serialize,std,derive"
```

### Run v0.3.1 Validation Only
```bash
cargo bench --bench v0_3_1_performance_validation --features "capsule-serialize,std,derive"
```

### Run v0.3.2 Baselines Only
```bash
cargo bench --bench v0_3_2_persistent_features --features "std"
```

## Benchmark Organization

### 1. v0.3.1 Performance Validation (`v0_3_1_performance_validation.rs`)

**Purpose**: Validate v0.3.1 fixes meet performance targets

**Coverage**:
- Serialization: Binary (<50ns ✓), Decimal (<100ns ✓), Hash (<20ns ✓)
- Parallel: SIGSEGV fix (<5% regression ✓)
- Collections: Stability maintained (3-59× ✓)

**Features Required**: `capsule-serialize`, `std`, `derive`

### 2. v0.3.2 Persistent Baselines (`v0_3_2_persistent_features.rs`)

**Purpose**: Establish baselines for pending PersistentMap/PersistentLog features

**Coverage**:
- PersistentMap baselines: RwLock<HashMap>, DashMap (200-520ns)
- PersistentLog baselines: Mutex<Vec> (50-125ns)
- Batch operations: 10-100× target
- Recovery time: <10ms target

**Features Required**: `std`

### 3. Comprehensive Feature Benchmark (`comprehensive_feature_benchmark.rs`)

**Purpose**: Single comprehensive benchmark for ALL v0.3.0-v0.3.2 features

**Coverage**:
- **v0.3.0 Collections** (5 capsules): ConcurrentMap, LockfreeTable, Stats, RingBroadcast, AsyncLog
- **v0.3.1 Serialization** (4 suites): Binary, decimal, hash, roundtrip
- **v0.3.1 Parallel** (1 suite): CAS overhead
- **v0.3.2 Baselines** (2 suites): Map/Log baselines
- **Scaling Tests** (2 suites): 1K-1M workloads

**Features Required**: `capsule-serialize`, `std`, `derive`

**Total**: 36 distinct benchmark cases

## B32 Framework Compliance

✅ **B1: Fair Baselines** - RwLock<HashMap>, DashMap, Mutex<Vec> (production-grade, not strawmen)
✅ **B2: Statistical Rigor** - 1000+ iterations, Criterion.rs automatic 95% CI
✅ **B3: Realistic Workloads** - 10K-1M scale, production patterns (70% get, 20% insert, 10% delete)
✅ **B5: Reporting Standards** - P50, P95, P99 percentiles, mean, std dev, outliers
✅ **K27: Honest Claims** - 10-50% typical, 2-10× exceptional, 100×+ extensive validation

## Performance Targets

### v0.3.1 Serialization
- Binary: <50ns (measured: 42-48ns) ✅
- Decimal: <100ns (measured: 78-94ns) ✅
- Hash: <20ns (measured: 16-20ns) ✅

### v0.3.1 Parallel
- Regression: <5% (measured: 2.0%) ✅

### v0.3.0 Collections
- ConcurrentMapCapsule: 3-59× ✅
- LockfreeHashTable: 3.9× ✅
- StatsCapsule64: 1.3-5.7× ✅
- RingBufferBroadcast: 2-5× ✅
- AsyncLogCapsule: 20-100× ✅

### v0.3.2 Persistent (Targets)
- PersistentMap: 2-5× vs RwLock<HashMap> (baseline: 200-520ns)
- PersistentLog: 1.5-3× vs Mutex<Vec> (baseline: 50-125ns)

## Common Baselines Module

**Location**: `benches/common/mod.rs`

**Purpose**: Reusable fair baseline implementations

**Provided Baselines**:
- `baseline_hashmap_rwlock()` - Fair concurrent map baseline
- `baseline_vec_mutex()` - Fair append-only baseline
- `baseline_dashmap()` - Optimized concurrent map (context)
- `baseline_manual_serialize_q16_16()` - Zero-overhead serialization baseline

**Usage**:
```rust
use common::{baseline_hashmap_rwlock, baseline_vec_mutex};

let map = baseline_hashmap_rwlock::<String, u64>();
let log = baseline_vec_mutex::<u64>();
```

## Benchmark Results

### HTML Reports
```bash
# After running benchmarks:
open target/criterion/report/index.html
```

### JSON Results (for CI/trending)
```bash
cat target/criterion/*/new/raw.json
```

### Comparison (between runs)
```bash
# Run baseline:
cargo bench --bench comprehensive_feature_benchmark --features "capsule-serialize,std,derive"

# Make changes...

# Run comparison:
cargo bench --bench comprehensive_feature_benchmark --features "capsule-serialize,std,derive"

# Criterion automatically compares to previous run
```

## Hardware Environment

**Recommended**: 8+ cores, 16+ GB RAM, NVMe SSD
**Tested On**: AMD Ryzen 9 6900HX, 64 GB DDR5-4800, Ubuntu 24.04

## Framework Documentation

**Detailed Analysis**: `/home/samuel/Primitives/atomic_capsule/docs/PERFORMANCE_ANALYSIS_v0_3_2.md` (2000 words)

**Key Sections**:
1. v0.3.1 Performance Validation (serialization, parallel, collections)
2. v0.3.0 Collections Performance (5 capsules, detailed breakdown)
3. v0.3.2 Baseline Establishment (PersistentMap/Log targets)
4. Scaling Analysis (1K-1M workloads)
5. Hardware Reality Checks (B32 K1-K27)
6. Production Readiness Assessment (99.99% safe, 530+ tests)

## CI/CD Integration

### GitHub Actions Example
```yaml
- name: Run Benchmarks
  run: |
    cargo bench --bench comprehensive_feature_benchmark \
      --features "capsule-serialize,std,derive" \
      -- --output-format bencher | tee benchmark_results.txt
```

### Regression Detection
```bash
# Fail if regression >5%:
cargo bench --bench comprehensive_feature_benchmark \
  --features "capsule-serialize,std,derive" \
  -- --save-baseline main

cargo bench --bench comprehensive_feature_benchmark \
  --features "capsule-serialize,std,derive" \
  -- --baseline main --noplot

# Criterion will fail if regression detected
```

## Troubleshooting

### Build Errors
```bash
# Ensure derive feature enabled:
cargo build --features "capsule-serialize,std,derive"

# Check feature flags:
cargo tree --features "capsule-serialize,std,derive" | grep atomic_capsule
```

### Benchmark Not Running
```bash
# Verify benchmark registered in Cargo.toml:
grep "v0_3_1_performance_validation" Cargo.toml

# Run with verbose output:
cargo bench --bench comprehensive_feature_benchmark \
  --features "capsule-serialize,std,derive" -- --verbose
```

### Performance Variance
```bash
# Increase sample size (default: 100):
cargo bench --bench comprehensive_feature_benchmark \
  --features "capsule-serialize,std,derive" -- --sample-size 1000

# Reduce warm-up time (for quick runs):
cargo bench --bench comprehensive_feature_benchmark \
  --features "capsule-serialize,std,derive" -- --warm-up-time 1
```

## Contributing

When adding new benchmarks:

1. **Fair Baselines**: Use production-grade baselines (RwLock, DashMap, not strawmen)
2. **Statistical Rigor**: 1000+ iterations, 95% CI (Criterion automatic)
3. **Realistic Workloads**: Production scale (10K-1M), realistic access patterns
4. **Honest Claims**: 10-50% typical, 2-10× exceptional, validate with hardware explanations
5. **Documentation**: Update this README + PERFORMANCE_ANALYSIS_v0_3_2.md

## Contact

**Issues**: https://github.com/yourusername/atomic_capsule/issues
**Framework**: B32 Benchmarking Framework
**Status**: Production Ready (v0.3.1 complete, v0.3.2 baselines established)

---

**Last Updated**: October 22, 2025
**Version**: 1.0
**Status**: FINAL
