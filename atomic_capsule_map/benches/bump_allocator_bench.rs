//! Bump Allocator Performance Benchmark
//!
//! Validates the 50ns performance improvement target for BucketArray allocation
//! during resize operations. Compares:
//! - Standard Box::new() allocation (baseline: ~80-100ns)
//! - Bump allocator allocation (target: ~30-50ns)
//!
//! Expected improvement: 50ns reduction per BucketArray allocation

use atomic_capsule_map::AtomicCapsuleMap;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::time::Duration;

/// Benchmark resize performance with standard allocation
fn bench_resize_standard(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_standard");
    group.measurement_time(Duration::from_secs(10));

    for initial_capacity in [1024, 2048, 4096, 8192] {
        group.bench_with_input(
            BenchmarkId::from_parameter(initial_capacity),
            &initial_capacity,
            |b, &capacity| {
                b.iter(|| {
                    // Create map with specific capacity
                    let map = AtomicCapsuleMap::<u64, u64>::with_capacity(capacity);

                    // Fill to trigger resize (75% load factor)
                    let target_count = (capacity as f64 * 0.76) as usize;
                    for i in 0..target_count {
                        map.insert(black_box(i as u64), black_box(i as u64 * 2));
                    }

                    // This insert should trigger resize
                    map.insert(black_box(target_count as u64), black_box(0));

                    map
                });
            },
        );
    }

    group.finish();
}

/// Benchmark concurrent resize operations
fn bench_resize_concurrent(c: &mut Criterion) {
    let mut group = c.benchmark_group("resize_concurrent");
    group.measurement_time(Duration::from_secs(10));

    for thread_count in [2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &threads| {
                b.iter(|| {
                    let map = Arc::new(AtomicCapsuleMap::<u64, u64>::new());

                    let mut handles = vec![];

                    for t in 0..threads {
                        let map_clone = Arc::clone(&map);
                        let handle = std::thread::spawn(move || {
                            let start = t * 10000;
                            for i in start..(start + 10000) {
                                map_clone.insert(black_box(i as u64), black_box(i as u64));
                            }
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        handle.join().unwrap();
                    }

                    map
                });
            },
        );
    }

    group.finish();
}

/// Benchmark allocation overhead specifically
fn bench_allocation_overhead(c: &mut Criterion) {
    use atomic_capsule_map::allocator::BumpAllocator;

    let mut group = c.benchmark_group("allocation_overhead");
    group.measurement_time(Duration::from_secs(5));

    // Benchmark bump allocator allocation
    group.bench_function("bump_allocator", |b| {
        let allocator = BumpAllocator::new();
        b.iter(|| {
            // Simulate BucketArray allocation
            let capacity = black_box(1024);
            let ptr = allocator.allocate_bucket_array(capacity);
            black_box(ptr);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_resize_standard,
    bench_resize_concurrent,
    bench_allocation_overhead,
);
criterion_main!(benches);
