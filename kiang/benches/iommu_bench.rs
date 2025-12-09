//! IOMMU Capsule Benchmarks
//!
//! Validates Phase 4 IOMMU integration performance targets:
//! - is_mapped() check: <5ns (hot path)
//! - map/unmap operations: <100ns
//! - Capsule read: <20ns

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::iommu::{IommuCapsule, IommuManager, IommuState, flags};

fn bench_iommu_capsule_read(c: &mut Criterion) {
    let capsule = IommuCapsule::new();
    let state = IommuState::new(1);
    capsule.publish(state);

    c.bench_function("iommu_capsule_read", |b| {
        b.iter(|| {
            let state = black_box(&capsule).read();
            black_box(state)
        })
    });
}

fn bench_is_operational(c: &mut Criterion) {
    let capsule = IommuCapsule::new();
    let state = IommuState::new(1);
    capsule.publish(state);

    c.bench_function("iommu_is_operational", |b| {
        b.iter(|| {
            let operational = black_box(&capsule).is_operational();
            black_box(operational)
        })
    });
}

fn bench_mapping_count(c: &mut Criterion) {
    let capsule = IommuCapsule::new();
    let state = IommuState {
        mapping_count: 42,
        domain_id: 1,
        total_mapped_mb: 1024,
        last_map_us: 500,
        valid: true,
    };
    capsule.publish(state);

    c.bench_function("iommu_mapping_count", |b| {
        b.iter(|| {
            let count = black_box(&capsule).mapping_count();
            black_box(count)
        })
    });
}

fn bench_is_mapped_hot_path(c: &mut Criterion) {
    let mut manager = IommuManager::new(1, 100, 4096);

    // Create 10 mappings
    for i in 0..10 {
        let base = 0x1000_0000 + (i * 0x100_0000);
        manager
            .map(
                base,
                0x2000_0000 + (i * 0x100_0000),
                4 * 1024 * 1024,
                flags::READ | flags::WRITE,
            )
            .unwrap();
    }

    c.bench_function("iommu_is_mapped_hot_path", |b| {
        b.iter(|| {
            // Check address in middle mapping (realistic hot path)
            let mapped = black_box(&manager).is_mapped(black_box(0x1500_0000));
            black_box(mapped)
        })
    });
}

fn bench_map_operation(c: &mut Criterion) {
    c.bench_function("iommu_map_operation", |b| {
        b.iter_batched(
            || IommuManager::new(1, 1000, 8192),
            |mut manager| {
                manager
                    .map(0x1000_0000, 0x2000_0000, 4 * 1024 * 1024, flags::READ)
                    .unwrap();
                black_box(manager)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_unmap_operation(c: &mut Criterion) {
    c.bench_function("iommu_unmap_operation", |b| {
        b.iter_batched(
            || {
                let mut manager = IommuManager::new(1, 1000, 8192);
                // Pre-populate with mappings
                for i in 0..100 {
                    let base = 0x1000_0000 + (i * 0x100_0000);
                    manager
                        .map(
                            base,
                            0x2000_0000 + (i * 0x100_0000),
                            4 * 1024 * 1024,
                            flags::READ,
                        )
                        .unwrap();
                }
                manager
            },
            |mut manager| {
                manager.unmap(0x1000_0000).unwrap();
                black_box(manager)
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

fn bench_get_mapping(c: &mut Criterion) {
    let mut manager = IommuManager::new(1, 100, 4096);

    // Create mappings
    for i in 0..10 {
        let base = 0x1000_0000 + (i * 0x100_0000);
        manager
            .map(
                base,
                0x2000_0000 + (i * 0x100_0000),
                4 * 1024 * 1024,
                flags::READ | flags::WRITE,
            )
            .unwrap();
    }

    c.bench_function("iommu_get_mapping", |b| {
        b.iter(|| {
            let mapping = black_box(&manager).get_mapping(black_box(0x1500_0000));
            black_box(mapping)
        })
    });
}

fn bench_concurrent_reads(c: &mut Criterion) {
    let capsule = IommuCapsule::new();
    let state = IommuState::new(1);
    capsule.publish(state);

    c.bench_function("iommu_concurrent_reads_single_threaded", |b| {
        b.iter(|| {
            // Simulate concurrent reads in single thread
            for _ in 0..8 {
                let state = black_box(&capsule).read();
                black_box(state);
            }
        })
    });
}

fn bench_mapping_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("iommu_mapping_scalability");

    for &mapping_count in &[10, 50, 100, 500] {
        group.bench_with_input(
            BenchmarkId::from_parameter(mapping_count),
            &mapping_count,
            |b, &count| {
                let mut manager = IommuManager::new(1, count as u32, 8192);

                // Pre-populate
                for i in 0..count {
                    let base = 0x1000_0000 + (i as u64 * 0x100_0000);
                    manager
                        .map(
                            base,
                            0x2000_0000 + (i as u64 * 0x100_0000),
                            4 * 1024 * 1024,
                            flags::READ,
                        )
                        .unwrap();
                }

                b.iter(|| {
                    // Check address in middle of mappings
                    let target = 0x1000_0000 + ((count / 2) as u64 * 0x100_0000);
                    let mapped = black_box(&manager).is_mapped(black_box(target));
                    black_box(mapped)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_iommu_capsule_read,
    bench_is_operational,
    bench_mapping_count,
    bench_is_mapped_hot_path,
    bench_map_operation,
    bench_unmap_operation,
    bench_get_mapping,
    bench_concurrent_reads,
    bench_mapping_scalability,
);
criterion_main!(benches);
