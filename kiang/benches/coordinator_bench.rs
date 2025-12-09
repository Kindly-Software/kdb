//! Performance Benchmarks for Queue and Batch Coordinators
//!
//! Validates performance targets following B32 framework and UCE30 empirical validation.
//!
//! # Performance Targets (UCE29 Constraints, Q30 Validation)
//!
//! - Queue selection: <5ns (single atomic load + branch)
//! - Load update: <15ns (atomic CAS operation)
//! - Batch decision: <5ns (single load + comparison)
//! - State publication: <50ns (4 atomic stores)
//!
//! # Measurement Methodology (B32 Framework)
//!
//! - Criterion benchmarks with 95% confidence intervals
//! - Minimum 1000 iterations per measurement
//! - Statistical outlier detection
//! - Comparison against baseline (simple atomic operations)

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::thread;

use kiang::batch_coordinator::{BatchCoordinator, BatchHintCapsule, BatchState};
use kiang::command::{Command, CommandType};
use kiang::queue_coordinator::{QueueCoordinatorCapsule, QueueId, QueueState};

// ============================================================================
// Queue Coordinator Benchmarks
// ============================================================================

/// Benchmark: Queue selection latency (target: <5ns)
///
/// #VERIFY_ORDERING_SUFFICIENT: Measures impact of Relaxed vs Acquire ordering
fn bench_queue_selection(c: &mut Criterion) {
    let qcc = QueueCoordinatorCapsule::new();

    // Setup: Publish state with realistic loads
    let state = QueueState {
        active_queues: 0b1111,
        render_load: 100,
        compute_load: 150,
        copy_load: 50,
        video_load: 75,
        render_priority: 128,
        compute_priority: 128,
        copy_priority: 128,
        video_priority: 128,
        hints: 0,
    };
    qcc.publish(state);

    c.bench_function("queue_selection_render", |b| {
        b.iter(|| {
            let queue = qcc.select_queue(black_box(CommandType::Render), black_box(128));
            black_box(queue);
        })
    });

    c.bench_function("queue_selection_compute", |b| {
        b.iter(|| {
            let queue = qcc.select_queue(black_box(CommandType::Compute), black_box(128));
            black_box(queue);
        })
    });

    c.bench_function("queue_selection_copy", |b| {
        b.iter(|| {
            let queue = qcc.select_queue(black_box(CommandType::Copy), black_box(128));
            black_box(queue);
        })
    });
}

/// Benchmark: Load update latency (target: <15ns)
///
/// #VERIFY_TOCTOU_PREVENTED: Measures CAS loop performance under contention
fn bench_load_update(c: &mut Criterion) {
    let qcc = QueueCoordinatorCapsule::new();
    qcc.publish(QueueState::new_all_active());

    c.bench_function("load_update_single_thread", |b| {
        b.iter(|| {
            qcc.update_load(black_box(QueueId::Render0), black_box(1));
        })
    });

    // Benchmark under contention
    let qcc_arc = Arc::new(QueueCoordinatorCapsule::new());
    qcc_arc.publish(QueueState::new_all_active());

    c.bench_function("load_update_contended", |b| {
        b.iter(|| {
            let qcc_clone = Arc::clone(&qcc_arc);
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let qcc = Arc::clone(&qcc_clone);
                    thread::spawn(move || {
                        for _ in 0..100 {
                            qcc.update_load(QueueId::Render0, 1);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

/// Benchmark: Full state read latency (target: <20ns)
///
/// #VERIFY_INVARIANT: Measures version check and checksum validation overhead
fn bench_state_read(c: &mut Criterion) {
    let qcc = QueueCoordinatorCapsule::new();
    qcc.publish(QueueState::new_all_active());

    c.bench_function("state_read_full", |b| {
        b.iter(|| {
            let state = qcc.read();
            black_box(state);
        })
    });
}

/// Benchmark: State publication latency (target: <50ns)
///
/// #VERIFY_STATE_MACHINE: Measures two-phase commit overhead
fn bench_state_publish(c: &mut Criterion) {
    let qcc = QueueCoordinatorCapsule::new();

    let state = QueueState {
        active_queues: 0b1111,
        render_load: 100,
        compute_load: 200,
        copy_load: 50,
        video_load: 75,
        render_priority: 128,
        compute_priority: 128,
        copy_priority: 128,
        video_priority: 128,
        hints: 0xDEADBEEF,
    };

    c.bench_function("state_publish", |b| {
        b.iter(|| {
            qcc.publish(black_box(state));
        })
    });
}

// ============================================================================
// Batch Coordinator Benchmarks
// ============================================================================

/// Benchmark: Batching decision latency (target: <5ns)
///
/// #VERIFY_ORDERING_SUFFICIENT: Measures Relaxed ordering performance
fn bench_batch_decision(c: &mut Criterion) {
    let bhc = BatchHintCapsule::with_thresholds(32, 1000);

    let cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 1,
        size: 1024,
        priority: 128,
    };

    c.bench_function("batch_decision_should_batch", |b| {
        b.iter(|| {
            let should = bhc.should_batch(black_box(&cmd), black_box(500));
            black_box(should);
        })
    });

    // With pending commands
    for _ in 0..20 {
        bhc.increment_pending_render();
    }

    c.bench_function("batch_decision_with_pending", |b| {
        b.iter(|| {
            let should = bhc.should_batch(black_box(&cmd), black_box(500));
            black_box(should);
        })
    });
}

/// Benchmark: Pending counter update latency (target: <15ns)
///
/// #VERIFY_COUNTER_ACCURACY: Measures atomic counter performance
fn bench_pending_counter(c: &mut Criterion) {
    let bhc = BatchHintCapsule::with_thresholds(1000, 5000);

    c.bench_function("counter_increment", |b| {
        b.iter(|| {
            bhc.increment_pending_render();
        })
    });

    c.bench_function("counter_decrement", |b| {
        b.iter(|| {
            bhc.decrement_pending_render();
        })
    });

    // Benchmark under contention
    let bhc_arc = Arc::new(BatchHintCapsule::with_thresholds(1000, 5000));

    c.bench_function("counter_contended", |b| {
        b.iter(|| {
            let bhc_clone = Arc::clone(&bhc_arc);
            let handles: Vec<_> = (0..4)
                .map(|_| {
                    let bhc = Arc::clone(&bhc_clone);
                    thread::spawn(move || {
                        for _ in 0..100 {
                            bhc.increment_pending_render();
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        })
    });
}

/// Benchmark: Submission time recording (target: <10ns)
fn bench_submission_time(c: &mut Criterion) {
    let bhc = BatchHintCapsule::with_thresholds(32, 1000);

    c.bench_function("record_submission_time", |b| {
        b.iter(|| {
            bhc.record_submission_time(black_box(1000));
        })
    });
}

/// Benchmark: BatchCoordinator integration
fn bench_batch_coordinator_integration(c: &mut Criterion) {
    let coordinator = BatchCoordinator::with_thresholds(32, 1000);

    let cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 1,
        size: 1024,
        priority: 128,
    };

    c.bench_function("batch_coordinator_should_batch", |b| {
        b.iter(|| {
            let should = coordinator.should_batch(black_box(&cmd));
            black_box(should);
        })
    });

    c.bench_function("batch_coordinator_record", |b| {
        b.iter(|| {
            coordinator.record_submission();
        })
    });
}

// ============================================================================
// Baseline Comparisons (B32 Framework - Fair Baselines)
// ============================================================================

/// Baseline: Simple atomic load (for comparison)
fn bench_baseline_atomic_load(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};

    let atomic = AtomicU64::new(12345);

    c.bench_function("baseline_atomic_load_relaxed", |b| {
        b.iter(|| {
            let val = atomic.load(black_box(Ordering::Relaxed));
            black_box(val);
        })
    });

    c.bench_function("baseline_atomic_load_acquire", |b| {
        b.iter(|| {
            let val = atomic.load(black_box(Ordering::Acquire));
            black_box(val);
        })
    });
}

/// Baseline: Simple atomic CAS (for comparison)
fn bench_baseline_atomic_cas(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};

    let atomic = AtomicU64::new(0);

    c.bench_function("baseline_atomic_cas", |b| {
        b.iter(|| {
            let current = atomic.load(Ordering::Relaxed);
            let _ = atomic.compare_exchange_weak(
                current,
                black_box(current + 1),
                Ordering::Release,
                Ordering::Relaxed,
            );
        })
    });
}

// ============================================================================
// Scaling Benchmarks (Multi-threaded Performance)
// ============================================================================

/// Benchmark queue selection scaling across thread counts
fn bench_queue_selection_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_selection_scaling");

    let qcc = Arc::new(QueueCoordinatorCapsule::new());
    qcc.publish(QueueState::new_all_active());

    for thread_count in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &tc| {
                b.iter(|| {
                    let qcc_clone = Arc::clone(&qcc);
                    let handles: Vec<_> = (0..tc)
                        .map(|_| {
                            let qcc = Arc::clone(&qcc_clone);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    let queue = qcc.select_queue(CommandType::Render, 128);
                                    black_box(queue);
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

/// Benchmark load update scaling across thread counts
fn bench_load_update_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_update_scaling");

    for thread_count in [1, 2, 4, 8] {
        group.bench_with_input(
            BenchmarkId::from_parameter(thread_count),
            &thread_count,
            |b, &tc| {
                let qcc = Arc::new(QueueCoordinatorCapsule::new());
                qcc.publish(QueueState::new_all_active());

                b.iter(|| {
                    let qcc_clone = Arc::clone(&qcc);
                    let handles: Vec<_> = (0..tc)
                        .map(|_| {
                            let qcc = Arc::clone(&qcc_clone);
                            thread::spawn(move || {
                                for _ in 0..1000 {
                                    qcc.update_load(QueueId::Render0, 1);
                                }
                            })
                        })
                        .collect();

                    for h in handles {
                        h.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    coordinator_benches,
    bench_queue_selection,
    bench_load_update,
    bench_state_read,
    bench_state_publish,
    bench_batch_decision,
    bench_pending_counter,
    bench_submission_time,
    bench_batch_coordinator_integration,
    bench_baseline_atomic_load,
    bench_baseline_atomic_cas,
    bench_queue_selection_scaling,
    bench_load_update_scaling,
);

criterion_main!(coordinator_benches);
