//! # Borrow<Q> Zero-Allocation Benchmarks (Phase 1.2)
//!
//! **B32 Framework**: Honest measurement of String allocation elimination
//!
//! ## Expected Results
//! - Owned String lookup: ~20ns String allocation overhead
//! - Borrowed &str lookup: 0ns allocation (baseline operation only)
//! - **Speedup**: ~20ns improvement (allocation cost savings)
//!
//! ## Methodology (B32 Framework)
//! - Fair baseline: Same CPU, same compiler, same iteration count (1000+)
//! - Measure: Direct comparison String-owned vs &str-borrowed lookups
//! - Reality check: 10-50% typical gains, 20ns is absolute allocation cost
//! - 95% CI via Criterion (1000+ iterations)

use atomic_capsule::collections::ConcurrentMapCapsule;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

// ===========================================================================
// Benchmark 1: Single-threaded String allocation cost
// ===========================================================================

fn bench_owned_string_lookup(c: &mut Criterion) {
    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Pre-populate with 1000 String keys
    for i in 0..1000 {
        map.insert(format!("key{:04}", i), i);
    }

    c.bench_function("owned_string_lookup", |b| {
        b.iter(|| {
            for i in 0..100 {
                // Allocate new String on every lookup (expensive)
                let key = format!("key{:04}", black_box(i));
                black_box(map.get(&key));
            }
        });
    });
}

fn bench_borrowed_str_lookup(c: &mut Criterion) {
    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Pre-populate with 1000 String keys
    for i in 0..1000 {
        map.insert(format!("key{:04}", i), i);
    }

    // Pre-allocate keys (simulate real-world where keys exist)
    let keys: Vec<String> = (0..100).map(|i| format!("key{:04}", i)).collect();

    c.bench_function("borrowed_str_lookup", |b| {
        b.iter(|| {
            for key_str in &keys {
                // Use &str (zero allocation)
                black_box(map.get(black_box(key_str.as_str())));
            }
        });
    });
}

// ===========================================================================
// Benchmark 2: Direct comparison (owned vs borrowed)
// ===========================================================================

fn bench_lookup_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("string_lookup_comparison");
    group.measurement_time(Duration::from_secs(5));

    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Pre-populate
    for i in 0..1000 {
        map.insert(format!("key{:04}", i), i);
    }

    // Owned String lookup
    group.bench_function("owned", |b| {
        b.iter(|| {
            let key = format!("key{:04}", black_box(500));
            black_box(map.get(&key));
        });
    });

    // Borrowed &str lookup
    group.bench_function("borrowed", |b| {
        b.iter(|| {
            black_box(map.get(black_box("key0500")));
        });
    });

    group.finish();
}

// ===========================================================================
// Benchmark 3: Scaling with different key lengths
// ===========================================================================

fn bench_key_length_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("key_length_scaling");
    group.measurement_time(Duration::from_secs(5));

    for key_len in [10, 50, 100, 500].iter() {
        let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

        // Create keys of specified length
        let key = "x".repeat(*key_len);
        map.insert(key.clone(), 42);

        // Owned lookup
        group.bench_with_input(
            BenchmarkId::new("owned", key_len),
            key_len,
            |b, _key_len| {
                b.iter(|| {
                    let allocated_key = "x".repeat(black_box(*key_len));
                    black_box(map.get(&allocated_key));
                });
            },
        );

        // Borrowed lookup
        let key_str = key.as_str();
        group.bench_with_input(
            BenchmarkId::new("borrowed", key_len),
            key_len,
            |b, _key_len| {
                b.iter(|| {
                    black_box(map.get(black_box(key_str)));
                });
            },
        );
    }

    group.finish();
}

// ===========================================================================
// Benchmark 4: Multi-threaded allocation pressure
// ===========================================================================

fn bench_concurrent_borrow_savings(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let mut group = c.benchmark_group("concurrent_borrow");
    group.measurement_time(Duration::from_secs(10));

    let map = Arc::new(ConcurrentMapCapsule::<String, u64>::new());

    // Pre-populate
    for i in 0..1000 {
        map.insert(format!("key{:04}", i), i);
    }

    // Owned String lookups (high allocation pressure)
    group.bench_function("8_threads_owned", |b| {
        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..8 {
                let map_clone = Arc::clone(&map);
                handles.push(thread::spawn(move || {
                    for i in 0..100 {
                        let key = format!("key{:04}", black_box(i));
                        black_box(map_clone.get(&key));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    // Borrowed &str lookups (zero allocation)
    group.bench_function("8_threads_borrowed", |b| {
        // Pre-allocate keys once (simulate real-world)
        let keys: Vec<String> = (0..100).map(|i| format!("key{:04}", i)).collect();
        let keys = Arc::new(keys);

        b.iter(|| {
            let mut handles = vec![];

            for _ in 0..8 {
                let map_clone = Arc::clone(&map);
                let keys_clone = Arc::clone(&keys);
                handles.push(thread::spawn(move || {
                    for key_str in keys_clone.iter() {
                        black_box(map_clone.get(black_box(key_str.as_str())));
                    }
                }));
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
    });

    group.finish();
}

// ===========================================================================
// Benchmark 5: Vec<u8> vs &[u8] comparison
// ===========================================================================

fn bench_vec_vs_slice(c: &mut Criterion) {
    let mut group = c.benchmark_group("vec_vs_slice");
    group.measurement_time(Duration::from_secs(5));

    let map: ConcurrentMapCapsule<Vec<u8>, u64> = ConcurrentMapCapsule::new();

    // Pre-populate with Vec<u8> keys
    for i in 0..100 {
        map.insert(vec![i as u8, (i >> 8) as u8], i);
    }

    // Owned Vec<u8> lookup (allocates)
    group.bench_function("owned_vec", |b| {
        b.iter(|| {
            let key = vec![black_box(50u8), 0];
            black_box(map.get(&key));
        });
    });

    // Borrowed &[u8] lookup (zero allocation)
    group.bench_function("borrowed_slice", |b| {
        b.iter(|| {
            let key_slice: &[u8] = &[black_box(50u8), 0];
            black_box(map.get(key_slice));
        });
    });

    group.finish();
}

// ===========================================================================
// Benchmark 6: Contains_key and remove performance
// ===========================================================================

fn bench_contains_key_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("contains_remove_borrow");
    group.measurement_time(Duration::from_secs(5));

    // contains_key owned
    {
        let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
        for i in 0..1000 {
            map.insert(format!("key{:04}", i), i);
        }

        group.bench_function("contains_key_owned", |b| {
            b.iter(|| {
                let key = format!("key{:04}", black_box(500));
                black_box(map.contains_key(&key));
            });
        });
    }

    // contains_key borrowed
    {
        let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
        for i in 0..1000 {
            map.insert(format!("key{:04}", i), i);
        }

        group.bench_function("contains_key_borrowed", |b| {
            b.iter(|| {
                black_box(map.contains_key(black_box("key0500")));
            });
        });
    }

    // remove owned
    {
        group.bench_function("remove_owned", |b| {
            b.iter_batched(
                || {
                    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
                    for i in 0..1000 {
                        map.insert(format!("key{:04}", i), i);
                    }
                    map
                },
                |map| {
                    let key = format!("key{:04}", black_box(500));
                    black_box(map.remove(&key));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    // remove borrowed
    {
        group.bench_function("remove_borrowed", |b| {
            b.iter_batched(
                || {
                    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();
                    for i in 0..1000 {
                        map.insert(format!("key{:04}", i), i);
                    }
                    map
                },
                |map| {
                    black_box(map.remove(black_box("key0500")));
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_owned_string_lookup,
    bench_borrowed_str_lookup,
    bench_lookup_comparison,
    bench_key_length_scaling,
    bench_concurrent_borrow_savings,
    bench_vec_vs_slice,
    bench_contains_key_remove
);
criterion_main!(benches);
