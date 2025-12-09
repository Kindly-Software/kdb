//! Performance Benchmarks for ContextCapsule
//!
//! Following B32 framework for fair performance measurement:
//! - Target: <5ns for can_submit() hot path
//! - Target: <100ns for full read() operation
//! - Baseline: Compare against mutex-based implementation
//! - Hardware: Validate on real Intel hardware

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use kiang::context::{ContextCapsule, ContextState, ContextUpdate};
use std::hint::black_box as hint_black_box;
use std::sync::{Arc, Mutex};

// ============================================================================
// Baseline: Mutex-based Context (for comparison)
// ============================================================================

struct MutexContext {
    state: Mutex<ContextState>,
    context_id: Mutex<u16>,
}

impl MutexContext {
    fn new() -> Self {
        Self {
            state: Mutex::new(ContextState::Ready),
            context_id: Mutex::new(0),
        }
    }

    fn can_submit(&self) -> bool {
        *self.state.lock().unwrap() == ContextState::Ready
    }

    fn set_state(&self, state: ContextState) {
        *self.state.lock().unwrap() = state;
    }
}

// ============================================================================
// Hot Path Benchmarks - can_submit()
// ============================================================================

fn bench_can_submit_hot_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("can_submit_hot_path");

    // Atomic capsule implementation
    let capsule = Arc::new(ContextCapsule::new());
    let update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    capsule.publish(update);

    group.bench_function("atomic_capsule", |b| {
        b.iter(|| hint_black_box(capsule.can_submit()))
    });

    // Mutex baseline
    let mutex_ctx = Arc::new(MutexContext::new());

    group.bench_function("mutex_baseline", |b| {
        b.iter(|| hint_black_box(mutex_ctx.can_submit()))
    });

    group.finish();
}

// ============================================================================
// Full Read Benchmarks
// ============================================================================

fn bench_full_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_read");

    let capsule = Arc::new(ContextCapsule::new());
    let update = ContextUpdate {
        context_id: 1,
        priority: 3,
        state: ContextState::Ready,
        last_fence: 12345,
        batch_count: 100,
        error_count: 5,
        timestamp_us: 1000000,
        resource_gen: 10,
        mem_usage_mb: 512,
        submission_count: 5000,
    };
    capsule.publish(update);

    group.bench_function("atomic_read", |b| b.iter(|| hint_black_box(capsule.read())));

    group.finish();
}

// ============================================================================
// Publish Performance
// ============================================================================

fn bench_publish(c: &mut Criterion) {
    let mut group = c.benchmark_group("publish");

    let capsule = Arc::new(ContextCapsule::new());

    group.bench_function("two_phase_commit", |b| {
        let mut counter = 0u32;
        b.iter(|| {
            let update = ContextUpdate {
                context_id: (counter % 100) as u16,
                priority: (counter % 16) as u8,
                state: ContextState::Ready,
                last_fence: counter as u64,
                batch_count: (counter % 1000) as u16,
                error_count: 0,
                timestamp_us: counter,
                resource_gen: 1,
                mem_usage_mb: 128,
                submission_count: counter,
            };
            capsule.publish(update);
            counter += 1;
        })
    });

    group.finish();
}

// ============================================================================
// State Transition Benchmarks
// ============================================================================

fn bench_state_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_transitions");

    let capsule = Arc::new(ContextCapsule::new());
    let update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    capsule.publish(update);

    group.bench_function("mark_error", |b| {
        b.iter(|| {
            capsule.mark_error();
        })
    });

    group.bench_function("reset", |b| {
        b.iter(|| {
            capsule.reset();
        })
    });

    group.finish();
}

// ============================================================================
// Concurrent Access Benchmarks
// ============================================================================

fn bench_concurrent_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_reads");

    for num_threads in [1, 2, 4, 8, 16].iter() {
        let capsule = Arc::new(ContextCapsule::new());
        let update = ContextUpdate {
            context_id: 1,
            priority: 0,
            state: ContextState::Ready,
            last_fence: 0,
            batch_count: 0,
            error_count: 0,
            timestamp_us: 0,
            resource_gen: 0,
            mem_usage_mb: 0,
            submission_count: 0,
        };
        capsule.publish(update);

        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            num_threads,
            |b, &threads| {
                b.iter(|| {
                    let handles: Vec<_> = (0..threads)
                        .map(|_| {
                            let capsule = capsule.clone();
                            std::thread::spawn(move || {
                                for _ in 0..100 {
                                    hint_black_box(capsule.can_submit());
                                }
                            })
                        })
                        .collect();

                    for handle in handles {
                        handle.join().unwrap();
                    }
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Batch Increment Benchmarks
// ============================================================================

fn bench_batch_increment(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_increment");

    let capsule = Arc::new(ContextCapsule::new());
    let update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    capsule.publish(update);

    group.bench_function("single_thread", |b| {
        b.iter(|| {
            capsule.increment_batch_count();
        })
    });

    group.finish();
}

// ============================================================================
// Memory Ordering Comparison
// ============================================================================

fn bench_memory_ordering(c: &mut Criterion) {
    use std::sync::atomic::{AtomicU64, Ordering};

    let mut group = c.benchmark_group("memory_ordering");

    let atomic = Arc::new(AtomicU64::new(0));

    group.bench_function("relaxed_load", |b| {
        b.iter(|| hint_black_box(atomic.load(Ordering::Relaxed)))
    });

    group.bench_function("acquire_load", |b| {
        b.iter(|| hint_black_box(atomic.load(Ordering::Acquire)))
    });

    group.bench_function("seqcst_load", |b| {
        b.iter(|| hint_black_box(atomic.load(Ordering::SeqCst)))
    });

    group.finish();
}

// ============================================================================
// Realistic Scenario: Command Submission Decision
// ============================================================================

fn bench_realistic_command_submission(c: &mut Criterion) {
    let mut group = c.benchmark_group("realistic_scenario");

    let capsule = Arc::new(ContextCapsule::new());
    let update = ContextUpdate {
        context_id: 1,
        priority: 0,
        state: ContextState::Ready,
        last_fence: 0,
        batch_count: 0,
        error_count: 0,
        timestamp_us: 0,
        resource_gen: 0,
        mem_usage_mb: 0,
        submission_count: 0,
    };
    capsule.publish(update);

    group.bench_function("check_and_increment", |b| {
        b.iter(|| {
            // Realistic scenario: check if can submit, then increment batch count
            if capsule.can_submit() {
                capsule.increment_batch_count();
                hint_black_box(true)
            } else {
                hint_black_box(false)
            }
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_can_submit_hot_path,
    bench_full_read,
    bench_publish,
    bench_state_transitions,
    bench_concurrent_reads,
    bench_batch_increment,
    bench_memory_ordering,
    bench_realistic_command_submission,
);

criterion_main!(benches);
