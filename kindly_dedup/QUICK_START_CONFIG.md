# Ground Truth Configuration - Quick Start

## 30-Second Quick Start

### Production (Recommended)

```rust
use kindly_dedup::benchmarking::{UniversalGroundTruthGenerator, GroundTruthConfig};

let config = GroundTruthConfig::production();
let ground_truth = UniversalGroundTruthGenerator::compute_ground_truth_with_config(
    &corpus,
    0.85,
    config
)?;
```

**What it does**: Auto-selects best strategy for corpus size, 100% recall, all optimizations enabled.

## Choose Your Mode

### 1. Production (Auto-Select)

```rust
let config = GroundTruthConfig::production();
```

- **Auto-selects** based on size:
  - <5K docs → Exhaustive (gold standard)
  - 5K-100K docs → ExhaustiveCompound (24× speedup, 100% recall)
  - >100K docs → LshAccelerated (168-240× speedup, 94-98% recall)
- **100% recall required** by default
- **All optimizations enabled** (SIMD, parallel)

### 2. Fast (Large Datasets)

```rust
let config = GroundTruthConfig::fast();
```

- **LSH-accelerated**: 23-240× speedup
- **94-98% recall**: Acceptable for most use cases
- **Use for**: Rapid experimentation, >100K docs

### 3. Balanced (Production Validation)

```rust
let config = GroundTruthConfig::balanced();
```

- **ExhaustiveCompound**: T1+T2+T4 compound optimizations
- **100% recall**: Exact Jaccard on all pairs
- **24× speedup**: 8× parallel × 4× SIMD × 0.75 efficiency
- **Use for**: Production benchmarks, 1K-100K docs

### 4. Precision (Financial/Healthcare/Legal)

```rust
let config = GroundTruthConfig::precision();
```

- **Exhaustive**: Mathematical gold standard
- **100% recall**: Guaranteed
- **Use for**: Absolute correctness required (SOX, SOC2, GDPR, HIPAA)

### 5. Single-Threaded (Debugging)

```rust
let config = GroundTruthConfig::single_threaded();
```

- **Disables**: SIMD, parallel
- **Use for**: Reproducible debugging

## Performance Cheat Sheet

| Corpus Size | Best Config | Time | Recall |
|-------------|-------------|------|--------|
| 1K docs | `production()` | <1s | 100% |
| 10K docs | `balanced()` | ~10s | 100% |
| 100K docs | `balanced()` | ~17min | 100% |
| 1M docs | `fast()` | ~7-10min | 94-98% |

## Custom Configuration

```rust
let config = GroundTruthConfig {
    strategy: Some(GroundTruthStrategy::ExhaustiveCompound),  // Force strategy
    enable_simd: true,
    enable_parallel: true,
    num_threads: Some(8),       // Use 8 cores
    chunk_size: None,           // Auto-tune
    require_100_percent_recall: true,  // Enforce 100% recall
    enable_monitoring: true,     // Log configuration
};
```

## When to Use What?

### Use `production()` when:
- ✅ You want automatic optimization
- ✅ Corpus size varies
- ✅ 100% recall required

### Use `fast()` when:
- ✅ Corpus > 100K docs
- ✅ 94-98% recall acceptable
- ✅ Speed is critical

### Use `balanced()` when:
- ✅ 1K-100K docs
- ✅ 100% recall required
- ✅ Best performance with accuracy

### Use `precision()` when:
- ✅ Financial/healthcare/legal
- ✅ Compliance requirements
- ✅ Absolute correctness required

## Full Documentation

See `GROUND_TRUTH_CONFIG_GUIDE.md` for:
- Performance comparison tables
- Migration guide
- Advanced topics
- Framework compliance
