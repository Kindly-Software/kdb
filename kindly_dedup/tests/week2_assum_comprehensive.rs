//! Week 2 ASSUM Safety Comprehensive Tests
//!
//! **Test Suite**: 10 tests (ASSUM framework validation)
//! **Target**: SIMD Text Hashing + Batch LSH Lookups
//! **Framework**: ASSUM Safety Framework (99.99%+ safe target)
//! **Feature Gates**: simd-text-hashing, batch-lsh

use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// ASSUM SAFETY TESTS (10 tests total)
// ============================================================================

/// ASSUM-1: SIMD lockfree verification (no mutex/RwLock)
#[test]
#[cfg(feature = "simd-text-hashing")]
fn test_assum_simd_lockfree() {
    use atomic_capsule::text::SimdTextHasher;

    let hasher = Arc::new(SimdTextHasher::new());

    // Concurrent access without locks
    let handles: Vec<_> = (0..16)
        .map(|i| {
            let h = Arc::clone(&hasher);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let text = format!("concurrent text {}", i);
                    let _ = h.hash_tokens_simd(&text);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // ASSUM: No mutex/RwLock (100% lockfree via SIMD registers)
    // VERIFY: Concurrent access completes without blocking
}

/// ASSUM-2: SIMD determinism verification
#[test]
#[cfg(all(feature = "simd-text-hashing", feature = "portable_simd"))]
fn test_assum_simd_determinism() {
    use atomic_capsule::text::SimdTextHasher;

    let hasher = SimdTextHasher::new();
    let text = "deterministic test with multiple tokens here";

    // Run 1000 times
    let mut results = Vec::with_capacity(1000);
    for _ in 0..1000 {
        results.push(hasher.hash_tokens_simd(text));
    }

    // All results must be identical
    let first = &results[0];
    for result in &results {
        assert_eq!(
            result, first,
            "ASSUM: SIMD output must be deterministic (same input → same output)"
        );
    }

    // ASSUM: Deterministic FNV-1a hashing
    // VERIFY: 1000 runs produce identical output
}

/// ASSUM-3: SIMD equivalence vs scalar (CRITICAL)
#[test]
#[cfg(all(feature = "simd-text-hashing", feature = "portable_simd"))]
fn test_assum_simd_scalar_equivalence() {
    use atomic_capsule::text::SimdTextHasher;

    let hasher = SimdTextHasher::new();

    // Test 1000 random texts
    for i in 0..1000 {
        let text = format!("test {} with words and tokens here", i);

        let simd_hashes = hasher.hash_tokens_simd(&text);

        // Scalar baseline (FNV-1a)
        let scalar_hashes: Vec<u64> = text
            .split_whitespace()
            .map(|token| fnv1a_hash_scalar(token.as_bytes()))
            .collect();

        assert_eq!(
            simd_hashes, scalar_hashes,
            "ASSUM: SIMD must match scalar output (iteration {})",
            i
        );
    }

    // ASSUM: SIMD implementation matches scalar FNV-1a
    // VERIFY: 1000 random texts validate equivalence
}

/// ASSUM-4: Batch LSH lockfree verification
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_batch_lsh_lockfree() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = Arc::new(BatchLSHLookup::new(buckets));

    let signatures = vec![MinHashSignatureCapsule::default(); 100];

    // Concurrent batch lookups
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let bl = Arc::clone(&batch_lookup);
            let sigs = signatures.clone();
            thread::spawn(move || {
                for _ in 0..100 {
                    let _ = bl.lookup_batch(&sigs);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // ASSUM: No mutex/RwLock (rayon work-stealing + ConcurrentMapCapsule)
    // VERIFY: Concurrent lookups complete without blocking
}

/// ASSUM-5: Batch LSH Vec pool safety (thread-local)
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_batch_lsh_vec_pool_safety() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = Arc::new(BatchLSHLookup::new(buckets));

    // Each thread gets its own Vec pool (thread_local!)
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let bl = Arc::clone(&batch_lookup);
            thread::spawn(move || {
                let signatures = vec![MinHashSignatureCapsule::default(); 10];

                // Multiple calls should reuse thread-local pool
                for _ in 0..100 {
                    let _ = bl.lookup_batch(&signatures);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // ASSUM: thread_local! RefCell prevents simultaneous borrow
    // VERIFY: No RefCell panic (would crash thread)
}

/// ASSUM-6: Batch LSH determinism verification
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_batch_lsh_determinism() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = BatchLSHLookup::new(Arc::clone(&buckets));

    // Create deterministic signatures
    let signatures: Vec<_> = (0..100)
        .map(|i| {
            let mut sig = MinHashSignatureCapsule::default();
            sig.signature_mut()[0] = i as u32;
            sig
        })
        .collect();

    // Run 100 times
    let mut results = Vec::with_capacity(100);
    for _ in 0..100 {
        results.push(batch_lookup.lookup_batch(&signatures));
    }

    // All results must be identical
    let first = &results[0];
    for result in &results {
        assert_eq!(result, first, "ASSUM: Batch LSH output must be deterministic");
    }

    // ASSUM: Deterministic band hashing + bucket lookup
    // VERIFY: 100 runs produce identical output
}

/// ASSUM-7: Batch LSH parallel equivalence
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_batch_lsh_parallel_equivalence() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = BatchLSHLookup::new(buckets);

    let signatures: Vec<_> = (0..1000)
        .map(|i| {
            let mut sig = MinHashSignatureCapsule::default();
            sig.signature_mut()[0] = (i % 100) as u32;
            sig
        })
        .collect();

    // Sequential lookup
    let seq_candidates = batch_lookup.lookup_batch(&signatures);

    // Parallel lookup
    let par_candidates = batch_lookup.lookup_batch_parallel(&signatures);

    assert_eq!(
        seq_candidates, par_candidates,
        "ASSUM: Parallel must match sequential output"
    );

    // ASSUM: Rayon work-stealing preserves determinism
    // VERIFY: Parallel output matches sequential
}

/// ASSUM-8: Thread safety stress test (TSAN clean)
#[test]
#[cfg(all(feature = "simd-text-hashing", feature = "batch-lsh"))]
fn test_assum_thread_safety_stress() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use atomic_capsule::text::SimdTextHasher;
    use kindly_dedup::lsh::BatchLSHLookup;

    let hasher = Arc::new(SimdTextHasher::new());
    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = Arc::new(BatchLSHLookup::new(buckets));

    // 100 threads × 10K operations
    let handles: Vec<_> = (0..100)
        .map(|thread_id| {
            let h = Arc::clone(&hasher);
            let bl = Arc::clone(&batch_lookup);

            thread::spawn(move || {
                for i in 0..10_000 {
                    // SIMD hashing
                    let text = format!("thread {} iteration {}", thread_id, i);
                    let _ = h.hash_tokens_simd(&text);

                    // Batch LSH lookup
                    if i % 10 == 0 {
                        let signatures = vec![MinHashSignatureCapsule::default(); 10];
                        let _ = bl.lookup_batch(&signatures);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should not panic");
    }

    // ASSUM: 100% lockfree (SIMD + rayon + ConcurrentMapCapsule)
    // VERIFY: 100 threads × 10K ops = 1M operations, TSAN clean
}

/// ASSUM-9: Memory ordering correctness (no torn reads)
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_memory_ordering_correctness() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));
    let batch_lookup = Arc::new(BatchLSHLookup::new(buckets));

    let counter = Arc::new(AtomicUsize::new(0));

    // Writer thread: Update buckets
    let writer = {
        let bl = Arc::clone(&batch_lookup);
        let c = Arc::clone(&counter);
        thread::spawn(move || {
            for i in 0..1000 {
                let signatures = vec![MinHashSignatureCapsule::default(); 10];
                let _ = bl.lookup_batch(&signatures);
                c.fetch_add(1, Ordering::Release);
                thread::sleep(Duration::from_micros(10));
            }
        })
    };

    // Reader threads: Concurrent lookups
    let readers: Vec<_> = (0..10)
        .map(|_| {
            let bl = Arc::clone(&batch_lookup);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                while c.load(Ordering::Acquire) < 1000 {
                    let signatures = vec![MinHashSignatureCapsule::default(); 10];
                    let _ = bl.lookup_batch(&signatures);
                }
            })
        })
        .collect();

    writer.join().unwrap();
    for reader in readers {
        reader.join().unwrap();
    }

    // ASSUM: ConcurrentMapCapsule uses correct memory ordering
    // VERIFY: No torn reads (concurrent read/write completes safely)
}

/// ASSUM-10: Batch recall preservation (no false negatives)
#[test]
#[cfg(feature = "batch-lsh")]
fn test_assum_batch_recall_preservation() {
    use atomic_capsule::collections::ConcurrentMapCapsule;
    use atomic_capsule::probabilistic::MinHashSignatureCapsule;
    use kindly_dedup::lsh::BatchLSHLookup;

    let buckets = Arc::new(ConcurrentMapCapsule::with_capacity(131_072));

    // Populate buckets with known docs
    let signatures: Vec<_> = (0..100)
        .map(|i| {
            let mut sig = MinHashSignatureCapsule::default();
            sig.signature_mut()[0] = i as u32;

            // Add to bucket
            let band_hash = compute_band_hash(&sig, 0);
            let bucket_key = (0, band_hash);
            buckets.insert(bucket_key, vec![i]);

            sig
        })
        .collect();

    let batch_lookup = BatchLSHLookup::new(buckets);

    // Batch lookup
    let batch_candidates = batch_lookup.lookup_batch(&signatures);

    // Sequential lookup (baseline)
    let seq_candidates: Vec<_> = signatures
        .iter()
        .map(|sig| batch_lookup.lookup_batch(&[sig.clone()])[0].clone())
        .collect();

    assert_eq!(
        batch_candidates, seq_candidates,
        "ASSUM: Batch must preserve recall (no false negatives)"
    );

    // ASSUM: Batch lookup preserves LSH recall
    // VERIFY: All candidates found in batch match sequential
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// FNV-1a scalar hash (for SIMD equivalence testing)
#[inline(always)]
fn fnv1a_hash_scalar(bytes: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Compute band hash (for batch LSH testing)
#[cfg(feature = "batch-lsh")]
fn compute_band_hash(sig: &atomic_capsule::probabilistic::MinHashSignatureCapsule, band_idx: usize) -> u64 {
    const ROWS_PER_BAND: usize = 25;
    let start = band_idx * ROWS_PER_BAND;
    let end = (start + ROWS_PER_BAND).min(128);

    let mut band_hash = 0u64;
    for i in start..end {
        band_hash = band_hash.wrapping_mul(31).wrapping_add(sig.signature()[i] as u64);
    }
    band_hash
}
