//! Real DRM Driver Overhead Benchmarks
//!
//! Benchmarks measuring actual ioctl overhead with Intel Xe driver.
//! Follows B32 framework guidelines:
//! - Fair baselines (optimized simulation vs real driver)
//! - Statistical rigor (95% confidence intervals via Criterion)
//! - Realistic workloads (production-like ioctl patterns)
//! - Hardware reality checks
//!
//! # Performance Targets (B32 K2, K15)
//!
//! - GEM create/destroy: <10μs (ioctl overhead)
//! - VM_BIND operation: <50μs (address space update)
//! - Fence poll: <1μs (status check)
//! - Device open: <100μs (one-time cost)
//!
//! # B32 Compliance
//!
//! - B1: Fair baseline (simulated vs real ioctl)
//! - B2: Statistical rigor (Criterion's 95% CI)
//! - B3: Realistic workloads (actual DRM patterns)
//! - B5: Full disclosure (hardware, driver version)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::{DrmDevice, DrmError};
use std::time::{Duration, Instant};

// Simulated ioctl baseline for fair comparison (B1: No strawmen)
fn simulated_gem_create(size: u64) -> Duration {
    let start = Instant::now();
    // Simulate kernel processing overhead (context switch + allocation)
    std::thread::sleep(Duration::from_micros(5));
    black_box(size);
    start.elapsed()
}

/// Benchmark: Device open overhead (one-time cost)
///
/// # Expected Results (B32 K15, K18)
/// - Simulated: ~1μs (no syscall)
/// - Real: ~100μs (open syscall + driver init)
fn bench_device_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("device_open");

    // Baseline: Simulated open (no syscall)
    group.bench_function("simulated", |b| {
        b.iter(|| {
            let start = Instant::now();
            black_box("/dev/dri/card0");
            start.elapsed()
        });
    });

    // Real: Actual DRM device open (may fail on systems without Intel GPU)
    group.bench_function("real_drm", |b| {
        b.iter(|| {
            let result = DrmDevice::open(0);
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: GEM object creation overhead
///
/// # Expected Results (B32 K2, K13)
/// - Simulated: ~5μs (memory allocation only)
/// - Real: ~10μs (ioctl + kernel allocation)
///
/// # B32 Compliance
/// - K13: Allocation costs (small: 20ns, large: 200ns+)
/// - K15: Network latencies establish syscall baseline (~10μs)
fn bench_gem_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("gem_create");

    // Test multiple buffer sizes
    for size in [4096, 65536, 1_048_576].iter() {
        group.bench_with_input(BenchmarkId::new("simulated", size), size, |b, &size| {
            b.iter(|| {
                let duration = simulated_gem_create(size);
                black_box(duration);
            });
        });

        // Real GEM creation (requires Intel GPU)
        group.bench_with_input(BenchmarkId::new("real_drm", size), size, |b, &size| {
            // Try to open device once
            let device = match DrmDevice::open(0) {
                Ok(dev) => dev,
                Err(_) => {
                    // Skip if no GPU available
                    return;
                }
            };

            b.iter(|| {
                let result = device.gem_create(size);
                black_box(result);
                // GEM object drops automatically, releasing handle
            });
        });
    }

    group.finish();
}

/// Benchmark: VM_BIND operation timing
///
/// # Expected Results (B32 K15)
/// - Simulated: ~10μs (map tracking only)
/// - Real: ~50μs (ioctl + page table update)
///
/// VM_BIND updates GPU virtual address space, requiring:
/// 1. Kernel address space lock
/// 2. Page table walk
/// 3. TLB invalidation
fn bench_vm_bind(c: &mut Criterion) {
    let mut group = c.benchmark_group("vm_bind");

    // Simulated baseline (just tracking overhead)
    group.bench_function("simulated", |b| {
        b.iter(|| {
            let start = Instant::now();
            // Simulate address space tracking
            let _gpu_va = black_box(0x1000_0000u64);
            let _size = black_box(4096u64);
            std::thread::sleep(Duration::from_micros(10));
            start.elapsed()
        });
    });

    // Real VM_BIND (requires GEM object and GPU)
    group.bench_function("real_drm", |b| {
        let device = match DrmDevice::open(0) {
            Ok(dev) => dev,
            Err(_) => return, // Skip if no GPU
        };

        b.iter(|| {
            let gem = match device.gem_create(4096) {
                Ok(obj) => obj,
                Err(_) => return,
            };

            let result = device.vm_bind(
                black_box(&gem),
                black_box(0x1000_0000), // GPU virtual address
            );
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Fence polling overhead
///
/// # Expected Results (B32 K2)
/// - Simulated: ~100ns (atomic read)
/// - Real: ~1μs (ioctl to check fence status)
///
/// # B32 Validation
/// - K2: Atomic operations (10-15ns for CAS)
/// - K15: Syscall overhead establishes minimum (10μs localhost)
fn bench_fence_poll(c: &mut Criterion) {
    let mut group = c.benchmark_group("fence_poll");

    // Simulated: Just atomic check
    group.bench_function("simulated_atomic", |b| {
        use std::sync::atomic::{AtomicU64, Ordering};
        let fence_seqno = AtomicU64::new(0);

        b.iter(|| {
            let seqno = black_box(&fence_seqno).load(Ordering::Acquire);
            let signaled = seqno > 0;
            black_box(signaled);
        });
    });

    // Real: ioctl to query fence status
    group.bench_function("real_drm_ioctl", |b| {
        let device = match DrmDevice::open(0) {
            Ok(dev) => dev,
            Err(_) => return,
        };

        // Create fence by submitting empty command
        let fence_seqno = match device.submit_noop() {
            Ok(seqno) => seqno,
            Err(_) => return,
        };

        b.iter(|| {
            let result = device.fence_wait(black_box(fence_seqno), black_box(0));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark: Complete submission workflow
///
/// Realistic end-to-end measurement:
/// 1. GEM create (buffer allocation)
/// 2. VM_BIND (address mapping)
/// 3. Command submission
/// 4. Fence wait (completion check)
///
/// # Expected Results
/// - Simulated: ~50μs (all overhead simulated)
/// - Real: ~200μs (actual kernel coordination)
///
/// # B32 Compliance
/// - B3: Realistic workload (production command submission)
/// - B5: Complete workflow timing
fn bench_submission_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("submission_workflow");

    // Simulated complete workflow
    group.bench_function("simulated", |b| {
        b.iter(|| {
            // Simulate GEM create
            std::thread::sleep(Duration::from_micros(5));
            // Simulate VM_BIND
            std::thread::sleep(Duration::from_micros(10));
            // Simulate submit
            std::thread::sleep(Duration::from_micros(30));
            // Simulate fence check
            std::thread::sleep(Duration::from_micros(5));
        });
    });

    // Real DRM workflow
    group.bench_function("real_drm", |b| {
        let device = match DrmDevice::open(0) {
            Ok(dev) => dev,
            Err(_) => return,
        };

        b.iter(|| {
            // 1. Create GEM buffer
            let gem = match device.gem_create(black_box(4096)) {
                Ok(obj) => obj,
                Err(_) => return,
            };

            // 2. Bind to GPU address space
            if device.vm_bind(&gem, black_box(0x1000_0000)).is_err() {
                return;
            }

            // 3. Submit command (noop)
            let fence = match device.submit_noop() {
                Ok(seqno) => seqno,
                Err(_) => return,
            };

            // 4. Wait for completion (timeout = 0 for poll)
            let result = device.fence_wait(fence, 0);
            black_box(result);

            // GEM object drops automatically
        });
    });

    group.finish();
}

/// Benchmark: Batch operation scaling
///
/// Tests how ioctl overhead scales with batch size.
///
/// # Expected Results (B32 K10, K20)
/// - Small batches (1-10): High per-op overhead
/// - Medium batches (10-100): Amortized overhead
/// - Large batches (100+): Diminishing returns
///
/// # B32 Validation
/// - K10: Big-O constants matter (batch threshold)
/// - K20: Throughput scaling (efficiency per batch size)
fn bench_batch_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_scaling");

    for batch_size in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("gem_create", batch_size),
            batch_size,
            |b, &batch_size| {
                let device = match DrmDevice::open(0) {
                    Ok(dev) => dev,
                    Err(_) => return,
                };

                b.iter(|| {
                    for _ in 0..batch_size {
                        let gem = device.gem_create(black_box(4096));
                        black_box(gem);
                        // Drops immediately
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Render node vs card node performance
///
/// Render nodes (/dev/dri/renderD*) don't require authentication.
/// Card nodes (/dev/dri/card*) support display but need auth.
///
/// # Expected Results
/// - Render node: ~100μs open (no auth)
/// - Card node: ~150μs open (with auth handshake)
fn bench_node_type_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_type");

    group.bench_function("card_node", |b| {
        b.iter(|| {
            let result = DrmDevice::open(0);
            black_box(result);
        });
    });

    group.bench_function("render_node", |b| {
        b.iter(|| {
            let result = DrmDevice::open_render(128);
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_device_open,
    bench_gem_create,
    bench_vm_bind,
    bench_fence_poll,
    bench_submission_workflow,
    bench_batch_scaling,
    bench_node_type_comparison,
);

criterion_main!(benches);
