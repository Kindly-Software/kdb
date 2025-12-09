//! T8 Network baseline generation
//!
//! # Baseline Strategy
//!
//! **Optimized**: Distributed coordination (multiple nodes)
//! **Baseline**: Single-node version (manual implementation required)
//!
//! # Why Manual?
//!
//! Distributed systems have complex coordination:
//! - Network topology (ring, tree, all-reduce)
//! - Data partitioning strategies
//! - Communication patterns
//!
//! Automatic single-node baseline would miss optimization opportunities.
//! Fair baseline requires well-optimized single-node multi-threaded code.
//!
//! # Manual Baseline Guide
//!
//! See full guide in `docs/MANUAL_BASELINE_GUIDE.md`

use super::{BaselineGenerator, ManualBaselineFn};

/// T8 Network baseline generator (Distributed → Single-node)
pub struct T8NetworkBaseline;

impl<T> BaselineGenerator<T> for T8NetworkBaseline {
    fn generate_baseline(&self) -> Option<ManualBaselineFn<T>> {
        // Manual baseline required - cannot auto-generate
        None
    }

    fn is_auto_generated(&self) -> bool {
        false
    }

    fn manual_guide(&self) -> &'static str {
        r#"
# T8 Network - Manual Baseline Guide

## Baseline Strategy
**Optimized**: Distributed coordination (multiple nodes)
**Baseline**: Single-node version (YOU provide this)

## How to Write Fair Single-Node Baseline

1. **Identify distributed operations** (all-reduce, scatter-gather, etc.)
2. **Write equivalent single-node multi-threaded code**
3. **Benchmark both implementations**

## Example: Distributed Training

```rust
// Distributed (optimized, 8 nodes)
let cluster = NetworkCluster::new(8);
let model = DistributedModel::shard_across(cluster);
model.train_epoch(data);  // Pipeline parallelism

// Single-node baseline (YOU write this)
fn train_single_node(model: &Model, data: &Data) {
    // Multi-threaded training on single GPU/CPU
    // Use data parallelism (NOT naive single-threaded!)
    rayon::scope(|s| {
        for batch in data.batches(threads) {
            s.spawn(|_| model.train_batch(batch));
        }
    });
}

// Benchmark
let config = BenchmarkConfig::builder()
    .tier(Tier::T8Network)
    .instant_timer()  // Wall-clock timing
    .baseline_manual(Box::new(|| train_single_node(&model, &data)))
    .build();
```

## Expected Results
- **TYPICAL**: 2-3× speedup (network overhead limits scaling)
- **EXCEPTIONAL**: 3-10× speedup (low network latency)
- **BREAKTHROUGH**: 10-50× speedup (embarrassingly parallel workloads)

## Fair Baseline Checklist
✓ Multi-threaded single-node code
✓ Same algorithm as distributed version
✓ Optimized data parallelism
✓ Realistic dataset (not toy problem)
✗ Single-threaded code (STRAWMAN!)
        "#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t8_network_baseline_is_manual() {
        let baseline = T8NetworkBaseline;
        assert!(!baseline.is_auto_generated());
        assert!(baseline.generate_baseline::<()>().is_none());
    }

    #[test]
    fn test_t8_network_baseline_has_guide() {
        let baseline = T8NetworkBaseline;
        let guide = baseline.manual_guide();
        assert!(guide.contains("Distributed"));
        assert!(guide.contains("Single-node"));
    }
}
