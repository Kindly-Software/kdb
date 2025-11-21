//! # Phase 4 Memory Impact Benchmark
//!
//! **B32 Framework Compliance**: Measure memory and binary size impact
//!
//! ## Mission
//!
//! Validate memory footprint of FixedPointSerialize trait:
//! 1. **Binary Size**: Before/after trait integration
//! 2. **Runtime Memory**: Heap usage during serialization
//! 3. **Stack Usage**: Trait implementation stack depth
//!
//! ## B32 Honest Claims
//!
//! - Target: <5KB total binary size impact
//! - Expected: <1KB per trait implementation
//! - Stack: <100 bytes per operation (zero-copy design)
//! - Heap: Only for String allocation (decimal format)
//!
//! ## Methodology
//!
//! 1. Build baseline: `cargo build --lib --release`
//! 2. Build with trait: `cargo build --lib --release --features capsule-serialize`
//! 3. Compare binary sizes: `size target/release/libatomic_capsule.rlib`
//! 4. Measure heap allocations during serialization
//! 5. Measure stack depth with perf/flamegraph

use atomic_capsule::serialize::fixed_point_serialize::{
    serialize_to_binary, FixedPointSerialize, FixedQ16_16,
};
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// Memory Tracking Allocator
// ============================================================================

struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicUsize = AtomicUsize::new(0);
static PEAK_ALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ret = System.alloc(layout);
        if !ret.is_null() {
            let size = layout.size();
            let old_allocated = ALLOCATED.fetch_add(size, Ordering::SeqCst);
            let new_allocated = old_allocated + size;

            // Update peak
            let mut peak = PEAK_ALLOCATED.load(Ordering::SeqCst);
            while new_allocated > peak {
                match PEAK_ALLOCATED.compare_exchange_weak(
                    peak,
                    new_allocated,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(x) => peak = x,
                }
            }
        }
        ret
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        DEALLOCATED.fetch_add(layout.size(), Ordering::SeqCst);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

fn reset_tracking() {
    ALLOCATED.store(0, Ordering::SeqCst);
    DEALLOCATED.store(0, Ordering::SeqCst);
    PEAK_ALLOCATED.store(0, Ordering::SeqCst);
}

fn get_allocated() -> usize {
    ALLOCATED.load(Ordering::SeqCst)
}

fn get_deallocated() -> usize {
    DEALLOCATED.load(Ordering::SeqCst)
}

fn get_peak_allocated() -> usize {
    PEAK_ALLOCATED.load(Ordering::SeqCst)
}

fn get_net_allocated() -> usize {
    get_allocated().saturating_sub(get_deallocated())
}

// ============================================================================
// Binary Size Analysis
// ============================================================================

/// Analyze binary size impact (manual verification required)
fn bench_binary_size_analysis(c: &mut Criterion) {
    // This benchmark is informational - actual binary size must be measured externally
    println!("\n=== Binary Size Analysis ===");
    println!("Run manually:");
    println!("  cargo clean");
    println!("  cargo build --lib --release");
    println!("  size target/release/libatomic_capsule.rlib > baseline.txt");
    println!("  cargo clean");
    println!("  cargo build --lib --release --features capsule-serialize");
    println!("  size target/release/libatomic_capsule.rlib > with_trait.txt");
    println!("  diff baseline.txt with_trait.txt");
    println!("\nExpected: <5KB total increase");
    println!("=============================\n");

    // Placeholder benchmark
    c.bench_function("binary_size/placeholder", |b| {
        b.iter(|| {
            black_box(42);
        });
    });
}

// ============================================================================
// Heap Memory Analysis
// ============================================================================

/// Measure heap allocations for serialize_raw() - Expected: 0 bytes
fn bench_heap_serialize_raw(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_memory/serialize_raw");

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);

        b.iter_custom(|iters| {
            reset_tracking();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                black_box(value.serialize_raw());
            }

            let elapsed = start.elapsed();
            let net_allocated = get_net_allocated();

            // Report heap usage
            if net_allocated > 0 {
                eprintln!(
                    "WARNING: serialize_raw() allocated {} bytes (expected 0)",
                    net_allocated
                );
            }

            elapsed
        });
    });

    group.finish();
}

/// Measure heap allocations for serialize_decimal() - Expected: ~20 bytes per String
fn bench_heap_serialize_decimal(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_memory/serialize_decimal");
    group.throughput(Throughput::Elements(1));

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);

        b.iter_custom(|iters| {
            reset_tracking();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                black_box(value.serialize_decimal());
            }

            let elapsed = start.elapsed();
            let allocated = get_allocated();
            let deallocated = get_deallocated();
            let peak = get_peak_allocated();

            // Report heap usage
            eprintln!("\nserialize_decimal() heap usage:");
            eprintln!(
                "  Total allocated: {} bytes ({} bytes/op)",
                allocated,
                allocated / iters as usize
            );
            eprintln!("  Total deallocated: {} bytes", deallocated);
            eprintln!("  Peak allocated: {} bytes", peak);
            eprintln!("  Expected: ~20 bytes/op (String allocation)");

            elapsed
        });
    });

    group.finish();
}

/// Measure heap allocations for serialize_to_binary() - Expected: ~50 bytes per Vec
fn bench_heap_serialize_binary(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_memory/serialize_binary");
    group.throughput(Throughput::Bytes(22));

    group.bench_function("Q16_16", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);

        b.iter_custom(|iters| {
            reset_tracking();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                black_box(serialize_to_binary(&value));
            }

            let elapsed = start.elapsed();
            let allocated = get_allocated();
            let deallocated = get_deallocated();
            let peak = get_peak_allocated();

            // Report heap usage
            eprintln!("\nserialize_to_binary() heap usage:");
            eprintln!(
                "  Total allocated: {} bytes ({} bytes/op)",
                allocated,
                allocated / iters as usize
            );
            eprintln!("  Total deallocated: {} bytes", deallocated);
            eprintln!("  Peak allocated: {} bytes", peak);
            eprintln!("  Expected: ~50 bytes/op (Vec<u8> allocation)");

            elapsed
        });
    });

    group.finish();
}

/// Measure peak heap usage during bulk operations
fn bench_heap_bulk_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_memory/bulk_operations");
    group.throughput(Throughput::Elements(1000));

    group.bench_function("serialize_1000_values", |b| {
        let values: Vec<_> = (0..1000)
            .map(|i| FixedQ16_16::from_decimal(i, (i * 123) % 10000))
            .collect();

        b.iter_custom(|iters| {
            reset_tracking();
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let mut results = Vec::new();
                for value in &values {
                    results.push(black_box(serialize_to_binary(value)));
                }
                black_box(results);
            }

            let elapsed = start.elapsed();
            let peak = get_peak_allocated();

            // Report peak heap usage
            eprintln!("\nBulk serialization peak heap usage:");
            eprintln!("  Peak allocated: {} bytes", peak);
            eprintln!("  Per operation: {} bytes", peak / 1000);
            eprintln!("  Expected: <100KB total (1000 × ~100 bytes)");

            elapsed
        });
    });

    group.finish();
}

// ============================================================================
// Stack Usage Analysis
// ============================================================================

/// Estimate stack usage via recursion depth
fn bench_stack_usage_estimate(c: &mut Criterion) {
    let mut group = c.benchmark_group("stack_memory/usage_estimate");

    group.bench_function("serialize_raw", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);

        // Estimate: sizeof(FixedQ16_16) + function call overhead
        let estimated_stack = std::mem::size_of::<FixedQ16_16>() + 16; // 16 bytes call overhead

        eprintln!("\nStack usage estimate (serialize_raw):");
        eprintln!(
            "  FixedQ16_16 size: {} bytes",
            std::mem::size_of::<FixedQ16_16>()
        );
        eprintln!("  Function overhead: ~16 bytes");
        eprintln!("  Total estimate: {} bytes", estimated_stack);
        eprintln!("  Target: <100 bytes");

        b.iter(|| {
            black_box(value.serialize_raw());
        });
    });

    group.bench_function("serialize_decimal", |b| {
        let value = FixedQ16_16::from_decimal(1234, 5678);

        // Estimate: value + temporaries (integers) + String allocation
        let estimated_stack = std::mem::size_of::<FixedQ16_16>()
            + std::mem::size_of::<i64>() * 2  // integer, fractional
            + std::mem::size_of::<String>()    // String header (24 bytes)
            + 32; // function call overhead

        eprintln!("\nStack usage estimate (serialize_decimal):");
        eprintln!(
            "  FixedQ16_16 size: {} bytes",
            std::mem::size_of::<FixedQ16_16>()
        );
        eprintln!("  Temporaries: {} bytes", std::mem::size_of::<i64>() * 2);
        eprintln!("  String header: {} bytes", std::mem::size_of::<String>());
        eprintln!("  Function overhead: ~32 bytes");
        eprintln!("  Total estimate: {} bytes", estimated_stack);
        eprintln!("  Target: <100 bytes");

        b.iter(|| {
            black_box(value.serialize_decimal());
        });
    });

    group.finish();
}

/// Measure memory locality (cache-friendly access)
fn bench_memory_locality(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_locality/cache_lines");

    let values: Vec<_> = (0..1024)
        .map(|i| FixedQ16_16::from_decimal(i, (i * 123) % 10000))
        .collect();

    group.bench_function("sequential_access", |b| {
        b.iter(|| {
            let mut total = 0i64;
            for value in &values {
                total += black_box(value.serialize_raw());
            }
            total
        });
    });

    group.bench_function("random_access", |b| {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hash, Hasher};

        let hasher = RandomState::new();

        b.iter(|| {
            let mut total = 0i64;
            for i in 0..1024 {
                let mut h = hasher.build_hasher();
                i.hash(&mut h);
                let idx = (h.finish() as usize) % 1024;
                total += black_box(values[idx].serialize_raw());
            }
            total
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_binary_size_analysis,
    bench_heap_serialize_raw,
    bench_heap_serialize_decimal,
    bench_heap_serialize_binary,
    bench_heap_bulk_operations,
    bench_stack_usage_estimate,
    bench_memory_locality,
);

criterion_main!(benches);
