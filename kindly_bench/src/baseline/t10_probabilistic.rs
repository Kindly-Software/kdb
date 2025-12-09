//! T10 Probabilistic baseline generation
//!
//! # Baseline Strategy
//!
//! **Optimized**: Approximate algorithms (MinHash, LSH, HyperLogLog, etc.)
//! **Baseline**: Exact algorithms (manual implementation required)
//!
//! # Why Manual?
//!
//! Approximate algorithms have accuracy trade-offs:
//! - MinHash → Exact Jaccard similarity (O(n²) vs O(n))
//! - HyperLogLog → Exact count distinct (O(n) space vs O(1))
//! - Bloom filter → Exact set membership (O(n) vs O(1))
//!
//! Fair baseline requires well-optimized exact algorithm (not naive implementation).
//!
//! # Manual Baseline Guide
//!
//! See full guide in `docs/MANUAL_BASELINE_GUIDE.md`

use super::{BaselineGenerator, ManualBaselineFn};

/// T10 Probabilistic baseline generator (Approximate → Exact)
pub struct T10ProbabilisticBaseline;

impl<T> BaselineGenerator<T> for T10ProbabilisticBaseline {
    fn generate_baseline(&self) -> Option<ManualBaselineFn<T>> {
        // Manual baseline required - cannot auto-generate
        None
    }

    fn is_auto_generated(&self) -> bool {
        false
    }

    fn manual_guide(&self) -> &'static str {
        r#"
# T10 Probabilistic - Manual Baseline Guide

## Baseline Strategy
**Optimized**: Approximate algorithms (MinHash, LSH, HyperLogLog)
**Baseline**: Exact algorithms (YOU provide this)

## How to Write Fair Exact Baseline

1. **Identify approximate algorithm**
2. **Write equivalent exact algorithm** (optimized!)
3. **Benchmark both implementations**
4. **Report accuracy vs speed tradeoff**

## Example: MinHash Deduplication

```rust
// Approximate (optimized, T10 Probabilistic)
let signatures = documents.iter()
    .map(|doc| MinHashSignatureCapsule::from_document(doc))
    .collect();
let duplicates = lsh_find_duplicates(signatures, threshold = 0.85);
// Accuracy: 90-99% recall, Speed: 60K docs/sec (38× speedup)

// Exact baseline (YOU write this)
fn exact_jaccard_deduplication(documents: &[Document]) -> Vec<(usize, usize)> {
    // All-pairs Jaccard similarity (O(n²))
    // Use optimized set intersection (NOT naive nested loops!)
    let mut duplicates = Vec::new();
    for i in 0..documents.len() {
        for j in (i+1)..documents.len() {
            let jaccard = compute_jaccard_optimized(&documents[i], &documents[j]);
            if jaccard >= 0.85 {
                duplicates.push((i, j));
            }
        }
    }
    duplicates
}

// Benchmark
let config = BenchmarkConfig::builder()
    .tier(Tier::T10Probabilistic)
    .baseline_manual(Box::new(|| exact_jaccard_deduplication(&documents)))
    .build();
```

## Expected Results
- **BREAKTHROUGH**: 100-1000× speedup (proven in kindly_dedup: 38-366×)
- **Accuracy trade-off**: 90-99% recall (F1 ≥90%)

## Fair Baseline Checklist
✓ Uses optimized exact algorithm (not naive O(n³))
✓ Same problem definition (e.g., Jaccard threshold)
✓ Optimized set operations (intersection, union)
✓ Reports accuracy metrics (recall, precision, F1)
✗ Naive nested loops (STRAWMAN!)
        "#
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_t10_probabilistic_baseline_is_manual() {
        let baseline = T10ProbabilisticBaseline;
        assert!(!baseline.is_auto_generated());
        assert!(baseline.generate_baseline::<()>().is_none());
    }

    #[test]
    fn test_t10_probabilistic_baseline_has_guide() {
        let baseline = T10ProbabilisticBaseline;
        let guide = baseline.manual_guide();
        assert!(guide.contains("Approximate"));
        assert!(guide.contains("Exact"));
        assert!(guide.contains("MinHash"));
    }
}
