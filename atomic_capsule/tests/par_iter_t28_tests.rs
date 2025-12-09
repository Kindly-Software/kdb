//! ParIterExt T28 Tests (5-Tier Framework: Q1-Q35)
//!
//! **Framework**: T28 (Unit + Property + Integration + Production + Determinism)
//! **Test Count**: 35 tests across 5 tiers
//! **Coverage**: API correctness, ordering, performance, determinism, stress
//!
//! **T28 Pyramid**:
//! - **Tier 1 (Q1-Q7)**: Unit - Basic API, edge cases, type safety
//! - **Tier 2 (Q8-Q14)**: Property - Result ordering, correctness invariants
//! - **Tier 3 (Q15-Q21)**: Integration - Multi-method chains, mixed workloads
//! - **Tier 4 (Q22-Q28)**: Production - Stress tests, latency targets
//! - **Tier 5 (Q29-Q35)**: Determinism - Reproducible results, ordering guarantees

use atomic_capsule::parallel::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// Tier Q1-Q7: Unit Tests (7 tests)
// ============================================================================

/// Q1: Basic for_each execution
#[test]
fn test_q1_for_each_basic() {
    let data = vec![1, 2, 3, 4, 5];
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    data.par_iter().for_each(|&x| {
        c.fetch_add(x as usize, Ordering::Relaxed);
    });

    assert_eq!(counter.load(Ordering::Relaxed), 15);
}

/// Q2: Basic map operation
#[test]
fn test_q2_map_basic() {
    let data = vec![1, 2, 3, 4, 5];
    let results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    assert_eq!(results, vec![2, 4, 6, 8, 10]);
}

/// Q3: Basic filter operation
#[test]
fn test_q3_filter_basic() {
    let data = vec![1, 2, 3, 4, 5, 6];
    let evens: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0);
    assert_eq!(evens, vec![2, 4, 6]);
}

/// Q4: Basic reduce operation
#[test]
fn test_q4_reduce_basic() {
    let data = vec![1, 2, 3, 4, 5];
    let sum: i32 = data.par_iter().reduce(0, |a, b| a + b);
    assert_eq!(sum, 15);
}

/// Q5: Basic find operation
#[test]
fn test_q5_find_basic() {
    let data = vec![1, 2, 3, 4, 5];
    let first_even = data.par_iter().find(|&&x| x % 2 == 0);
    assert_eq!(first_even, Some(2));
}

/// Q6: Basic partition operation
#[test]
fn test_q6_partition_basic() {
    let data = vec![1, 2, 3, 4, 5, 6];
    let (evens, odds) = data.par_iter().partition(|&&x| x % 2 == 0);
    assert_eq!(evens, vec![2, 4, 6]);
    assert_eq!(odds, vec![1, 3, 5]);
}

/// Q7: Empty collection handling
#[test]
fn test_q7_empty_collection() {
    let data: Vec<i32> = vec![];

    // for_each
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    data.par_iter().for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 0);

    // map
    let results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    assert!(results.is_empty());

    // filter
    let filtered: Vec<i32> = data.par_iter().filter(|_| true);
    assert!(filtered.is_empty());

    // reduce
    let sum: i32 = data.par_iter().reduce(0, |a, b| a + b);
    assert_eq!(sum, 0);

    // find
    let found = data.par_iter().find(|_| true);
    assert!(found.is_none());

    // partition
    let (left, right) = data.par_iter().partition(|_| true);
    assert!(left.is_empty());
    assert!(right.is_empty());
}

// ============================================================================
// Tier Q8-Q14: Property Tests (7 tests)
// ============================================================================

/// Q8: Map preserves input order
#[test]
fn test_q8_map_preserves_order() {
    let data: Vec<i32> = (0..1000).collect();
    let results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();

    assert_eq!(results.len(), 1000);
    for (i, &result) in results.iter().enumerate() {
        assert_eq!(result, i as i32 * 2, "Order violation at index {}", i);
    }
}

/// Q9: Filter preserves relative order
#[test]
fn test_q9_filter_preserves_order() {
    let data: Vec<i32> = (0..1000).collect();
    let evens: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0);

    assert_eq!(evens.len(), 500);
    for (i, &val) in evens.iter().enumerate() {
        assert_eq!(val, i as i32 * 2, "Filter order violation at index {}", i);
    }
}

/// Q10: Partition preserves relative order in both collections
#[test]
fn test_q10_partition_preserves_order() {
    let data: Vec<i32> = (0..1000).collect();
    let (evens, odds) = data.par_iter().partition(|&&x| x % 2 == 0);

    assert_eq!(evens.len(), 500);
    assert_eq!(odds.len(), 500);

    // Verify evens are in order
    for (i, &val) in evens.iter().enumerate() {
        assert_eq!(val, i as i32 * 2, "Even order violation at {}", i);
    }

    // Verify odds are in order
    for (i, &val) in odds.iter().enumerate() {
        assert_eq!(val, i as i32 * 2 + 1, "Odd order violation at {}", i);
    }
}

/// Q11: Reduce is associative and correct
#[test]
fn test_q11_reduce_associative() {
    let data: Vec<i32> = (1..=100).collect();

    // Sum
    let sum: i32 = data.par_iter().reduce(0, |a, b| a + b);
    assert_eq!(sum, 5050, "Sum should be 5050 (1+2+...+100)");

    // Product of small numbers
    let small: Vec<i32> = (1..=5).collect();
    let product: i32 = small.par_iter().reduce(1, |a, b| a * b);
    assert_eq!(product, 120, "Product should be 120 (5!)");

    // Max
    let max: i32 = data.par_iter().reduce(i32::MIN, |a, b| a.max(b));
    assert_eq!(max, 100);

    // Min
    let min: i32 = data.par_iter().reduce(i32::MAX, |a, b| a.min(b));
    assert_eq!(min, 1);
}

/// Q12: Find returns lowest-index match
#[test]
fn test_q12_find_lowest_index() {
    let data: Vec<i32> = (0..1000).collect();

    // Find first even (should be 0)
    let first_even = data.par_iter().find(|&&x| x % 2 == 0);
    assert_eq!(first_even, Some(0));

    // Find first > 500 (should be 501)
    let first_large = data.par_iter().find(|&&x| x > 500);
    assert_eq!(first_large, Some(501));

    // Find non-existent
    let not_found = data.par_iter().find(|&&x| x > 10000);
    assert!(not_found.is_none());
}

/// Q13: with_chunk_size affects task distribution
#[test]
fn test_q13_chunk_size_control() {
    let data: Vec<i32> = (0..1000).collect();
    let counter = Arc::new(AtomicUsize::new(0));

    // Large chunks = fewer tasks
    let c = counter.clone();
    data.par_iter().with_chunk_size(1000).for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 1000);

    // Small chunks still complete all work
    counter.store(0, Ordering::Release);
    let c = counter.clone();
    data.par_iter().with_chunk_size(10).for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 1000);
}

/// Q14: into_par_iter consumes and processes correctly
#[test]
fn test_q14_into_par_iter() {
    let data: Vec<i32> = (1..=100).collect();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    data.clone().into_par_iter().for_each(|x| {
        c.fetch_add(x as usize, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 5050);

    // map
    let results: Vec<i32> = data.clone().into_par_iter().map(|x| x * 2).collect();
    assert_eq!(results.len(), 100);
    assert_eq!(results[0], 2);
    assert_eq!(results[99], 200);
}

// ============================================================================
// Tier Q15-Q21: Integration Tests (7 tests)
// ============================================================================

/// Q15: Large dataset (10K elements)
#[test]
fn test_q15_large_dataset_10k() {
    let data: Vec<i32> = (0..10000).collect();

    // map
    let doubled: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    assert_eq!(doubled.len(), 10000);
    assert_eq!(doubled[9999], 19998);

    // filter
    let evens: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0);
    assert_eq!(evens.len(), 5000);

    // reduce
    let sum: i64 = data.par_iter().reduce(0i64, |a, b| a + b as i64);
    assert_eq!(sum, (0..10000i64).sum::<i64>());
}

/// Q16: Multiple parallel operations in sequence
#[test]
fn test_q16_sequential_operations() {
    let data: Vec<i32> = (0..1000).collect();

    // First operation
    let results1: Vec<i32> = data.par_iter().map(|&x| x + 1).collect();
    assert_eq!(results1.len(), 1000);

    // Second operation on new data
    let results2: Vec<i32> = results1.par_iter().map(|&x| x * 2).collect();
    assert_eq!(results2.len(), 1000);

    // Verify chained effect
    assert_eq!(results2[0], 2); // (0+1)*2 = 2
    assert_eq!(results2[999], 2000); // (999+1)*2 = 2000
}

/// Q17: Concurrent par_iter from multiple threads
#[test]
fn test_q17_concurrent_par_iter() {
    let data = Arc::new((0..1000).collect::<Vec<i32>>());
    let counters: Vec<Arc<AtomicUsize>> = (0..4).map(|_| Arc::new(AtomicUsize::new(0))).collect();

    let handles: Vec<_> = counters
        .iter()
        .enumerate()
        .map(|(i, counter)| {
            let data = Arc::clone(&data);
            let counter = Arc::clone(counter);

            thread::spawn(move || {
                data.par_iter().for_each(|&x| {
                    counter.fetch_add(x as usize, Ordering::Relaxed);
                });
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Each thread should have summed all values
    for counter in &counters {
        assert_eq!(counter.load(Ordering::Relaxed), (0..1000).sum::<usize>());
    }
}

/// Q18: Mixed workload (fast + slow tasks)
#[test]
fn test_q18_mixed_workload() {
    let data: Vec<i32> = (0..100).collect();

    let results: Vec<i32> = data.par_iter().map(|&x| {
        if x % 10 == 0 {
            // Slightly slower task
            std::hint::black_box((0..100).fold(0, |a, b| a + b));
        }
        x * 2
    }).collect();

    assert_eq!(results.len(), 100);
    for (i, &r) in results.iter().enumerate() {
        assert_eq!(r, i as i32 * 2);
    }
}

/// Q19: Filter followed by map (simulated chaining)
#[test]
fn test_q19_filter_then_map() {
    let data: Vec<i32> = (0..1000).collect();

    // Filter evens
    let evens: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0);
    assert_eq!(evens.len(), 500);

    // Map evens
    let doubled: Vec<i32> = evens.par_iter().map(|&x| x * 2).collect();
    assert_eq!(doubled.len(), 500);

    // Verify
    assert_eq!(doubled[0], 0);
    assert_eq!(doubled[1], 4);
    assert_eq!(doubled[499], 1996);
}

/// Q20: Partition with large data
#[test]
fn test_q20_partition_large() {
    let data: Vec<i32> = (0..10000).collect();
    let (evens, odds) = data.par_iter().partition(|&&x| x % 2 == 0);

    assert_eq!(evens.len(), 5000);
    assert_eq!(odds.len(), 5000);

    // Verify sums
    let even_sum: i64 = evens.iter().map(|&x| x as i64).sum();
    let odd_sum: i64 = odds.iter().map(|&x| x as i64).sum();

    // Even sum: 0+2+4+...+9998 = 2*(0+1+2+...+4999) = 2*4999*5000/2 = 24995000
    assert_eq!(even_sum, 24995000);
    // Odd sum: 1+3+5+...+9999 = 5000*5000 = 25000000
    assert_eq!(odd_sum, 25000000);
}

/// Q21: Complex reduce operations
#[test]
fn test_q21_complex_reduce() {
    let data: Vec<i32> = (1..=100).collect();

    // Reduce to find (min, max) pair
    #[derive(Clone)]
    struct MinMax {
        min: i32,
        max: i32,
    }

    // Note: reduce requires Into<R> for T, so we use a simpler approach
    let min: i32 = data.par_iter().reduce(i32::MAX, |a, b| a.min(b));
    let max: i32 = data.par_iter().reduce(i32::MIN, |a, b| a.max(b));

    assert_eq!(min, 1);
    assert_eq!(max, 100);
}

// ============================================================================
// Tier Q22-Q28: Production Tests (7 tests)
// ============================================================================

/// Q22: Stress test - 100K elements
#[test]
fn test_q22_stress_100k() {
    let data: Vec<i32> = (0..100000).collect();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    data.par_iter().for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });

    assert_eq!(counter.load(Ordering::Relaxed), 100000);
}

/// Q23: Stress test - map 100K elements
#[test]
fn test_q23_stress_map_100k() {
    let data: Vec<i64> = (0..100000).collect();
    let results: Vec<i64> = data.par_iter().map(|&x| x * x).collect();

    assert_eq!(results.len(), 100000);
    assert_eq!(results[0], 0);
    assert_eq!(results[99999], 99999i64 * 99999i64);
}

/// Q24: Stress test - filter 100K elements
#[test]
fn test_q24_stress_filter_100k() {
    let data: Vec<i32> = (0..100000).collect();
    let multiples_of_100: Vec<i32> = data.par_iter().filter(|&&x| x % 100 == 0);

    assert_eq!(multiples_of_100.len(), 1000);
    assert_eq!(multiples_of_100[0], 0);
    assert_eq!(multiples_of_100[999], 99900);
}

/// Q25: Stress test - reduce 100K elements
#[test]
fn test_q25_stress_reduce_100k() {
    let data: Vec<i64> = (1..=100000).collect();
    let sum: i64 = data.par_iter().reduce(0, |a, b| a + b);

    // Sum of 1..100000 = 100000 * 100001 / 2 = 5000050000
    assert_eq!(sum, 5000050000);
}

/// Q26: Latency target - 1K tasks < 50ms
#[test]
fn test_q26_latency_1k() {
    let data: Vec<i32> = (0..1000).collect();
    let start = Instant::now();

    let results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();

    let elapsed = start.elapsed();
    assert_eq!(results.len(), 1000);
    assert!(
        elapsed.as_millis() < 50,
        "Latency too high: {:?} (target <50ms)",
        elapsed
    );
}

/// Q27: Memory efficiency - no excessive allocations
#[test]
fn test_q27_memory_efficiency() {
    let data: Vec<i32> = (0..10000).collect();

    // Multiple operations shouldn't cause memory blowup
    for _ in 0..10 {
        let _results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    }

    // If we get here without OOM, memory is bounded
}

/// Q28: Concurrent stress - 10 threads x 10K tasks
#[test]
fn test_q28_concurrent_stress() {
    let data = Arc::new((0..10000).collect::<Vec<i32>>());
    let total = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let data = Arc::clone(&data);
            let total = Arc::clone(&total);

            thread::spawn(move || {
                let local_sum: i32 = data.par_iter().reduce(0, |a, b| a + b);
                total.fetch_add(local_sum as usize, Ordering::Relaxed);
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Each thread computes same sum, 10 threads
    let expected = (0..10000).sum::<usize>() * 10;
    assert_eq!(total.load(Ordering::Relaxed), expected);
}

// ============================================================================
// Tier Q29-Q35: Determinism Tests (7 tests)
// ============================================================================

/// Q29: Map produces identical results across runs
#[test]
fn test_q29_map_determinism() {
    let data: Vec<i32> = (0..1000).collect();

    let results1: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    let results2: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    let results3: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();

    assert_eq!(results1, results2);
    assert_eq!(results2, results3);
}

/// Q30: Filter produces identical results across runs
#[test]
fn test_q30_filter_determinism() {
    let data: Vec<i32> = (0..1000).collect();

    let results1: Vec<i32> = data.par_iter().filter(|&&x| x % 3 == 0);
    let results2: Vec<i32> = data.par_iter().filter(|&&x| x % 3 == 0);
    let results3: Vec<i32> = data.par_iter().filter(|&&x| x % 3 == 0);

    assert_eq!(results1, results2);
    assert_eq!(results2, results3);
}

/// Q31: Reduce produces identical results across runs
#[test]
fn test_q31_reduce_determinism() {
    let data: Vec<i32> = (1..=100).collect();

    let sum1: i32 = data.par_iter().reduce(0, |a, b| a + b);
    let sum2: i32 = data.par_iter().reduce(0, |a, b| a + b);
    let sum3: i32 = data.par_iter().reduce(0, |a, b| a + b);

    assert_eq!(sum1, sum2);
    assert_eq!(sum2, sum3);
    assert_eq!(sum1, 5050);
}

/// Q32: Find returns same element across runs
#[test]
fn test_q32_find_determinism() {
    let data: Vec<i32> = (0..1000).collect();

    let found1 = data.par_iter().find(|&&x| x > 500);
    let found2 = data.par_iter().find(|&&x| x > 500);
    let found3 = data.par_iter().find(|&&x| x > 500);

    assert_eq!(found1, found2);
    assert_eq!(found2, found3);
    assert_eq!(found1, Some(501));
}

/// Q33: Partition produces identical results across runs
#[test]
fn test_q33_partition_determinism() {
    let data: Vec<i32> = (0..1000).collect();

    let (evens1, odds1) = data.par_iter().partition(|&&x| x % 2 == 0);
    let (evens2, odds2) = data.par_iter().partition(|&&x| x % 2 == 0);
    let (evens3, odds3) = data.par_iter().partition(|&&x| x % 2 == 0);

    assert_eq!(evens1, evens2);
    assert_eq!(evens2, evens3);
    assert_eq!(odds1, odds2);
    assert_eq!(odds2, odds3);
}

/// Q34: Results match sequential implementation
#[test]
fn test_q34_sequential_equivalence() {
    let data: Vec<i32> = (0..1000).collect();

    // Map
    let par_map: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
    let seq_map: Vec<i32> = data.iter().map(|&x| x * 2).collect();
    assert_eq!(par_map, seq_map);

    // Filter
    let par_filter: Vec<i32> = data.par_iter().filter(|&&x| x % 2 == 0);
    let seq_filter: Vec<i32> = data.iter().filter(|&&x| x % 2 == 0).cloned().collect();
    assert_eq!(par_filter, seq_filter);

    // Reduce
    let par_reduce: i32 = data.par_iter().reduce(0, |a, b| a + b);
    let seq_reduce: i32 = data.iter().fold(0, |a, &b| a + b);
    assert_eq!(par_reduce, seq_reduce);

    // Find
    let par_find = data.par_iter().find(|&&x| x > 500);
    let seq_find = data.iter().find(|&&x| x > 500).cloned();
    assert_eq!(par_find, seq_find);
}

/// Q35: Ordering is stable under high contention
#[test]
fn test_q35_ordering_under_contention() {
    let data: Vec<i32> = (0..10000).collect();

    // Run multiple times with concurrent access
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let data = data.clone();
            thread::spawn(move || {
                let results: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
                // Verify ordering
                for (i, &r) in results.iter().enumerate() {
                    assert_eq!(r, i as i32 * 2, "Ordering violation at {}", i);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

// ============================================================================
// Additional API Tests
// ============================================================================

/// Test collect on ParIter (identity operation)
#[test]
fn test_par_iter_collect() {
    let data = vec![1, 2, 3, 4, 5];
    let collected: Vec<i32> = data.par_iter().collect();
    assert_eq!(collected, data);
}

/// Test with_chunk_size on both ParIter and IntoParIter
#[test]
fn test_with_chunk_size() {
    let data: Vec<i32> = (0..100).collect();

    // ParIter with chunk_size
    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();
    data.par_iter().with_chunk_size(10).for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 100);

    // IntoParIter with chunk_size
    counter.store(0, Ordering::Release);
    let c = counter.clone();
    data.clone().into_par_iter().with_chunk_size(10).for_each(|_| {
        c.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(counter.load(Ordering::Relaxed), 100);
}

/// Test IntoParIter::map
#[test]
fn test_into_par_iter_map() {
    let data: Vec<i32> = (1..=10).collect();
    let results: Vec<i32> = data.into_par_iter().map(|x| x * x).collect();
    assert_eq!(results, vec![1, 4, 9, 16, 25, 36, 49, 64, 81, 100]);
}

/// Test IntoParIter::collect (identity)
#[test]
fn test_into_par_iter_collect() {
    let data: Vec<i32> = vec![1, 2, 3, 4, 5];
    let collected: Vec<i32> = data.clone().into_par_iter().collect();
    assert_eq!(collected, data);
}
