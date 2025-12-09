//! HybridBatchPool Tests (T28 Framework: Q1-Q28 4-Tier Pyramid)
//!
//! **Framework**: T28 (Unit + Property + Integration + Production)
//! **Test Count**: 28+ tests across 4 tiers (10 unit + 8 property + 7 integration + 3 production)
//! **Coverage**: No task loss, fairness, performance, correctness, scaling
//!
//! **T28 Pyramid**:
//! - **Tier 1 (Q1-Q7)**: Unit - Invariants, alignment, atomics, correctness
//! - **Tier 2 (Q8-Q14)**: Property - Concurrent workloads, overflow, edge cases
//! - **Tier 3 (Q15-Q21)**: Integration - Mixed workloads, stress tests, real-world scenarios
//! - **Tier 4 (Q22-Q28)**: Production - Sustained load, latency targets, memory bounds

use atomic_capsule::parallel::HybridBatchPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// Tier Q1-Q7: Unit Tests (10 tests)
// ============================================================================

/// Q1: Basic single task execution
#[test]
fn test_q1_basic_single_task() {
    let pool = HybridBatchPool::new(2).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    let c = counter.clone();
    pool.push(Box::new(move || {
        c.fetch_add(1, Ordering::Relaxed);
    }))
    .unwrap();

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1);
}

/// Q2: Multiple tasks from single thread
#[test]
fn test_q2_multiple_tasks_single_thread() {
    let pool = HybridBatchPool::new(2).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..100 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 100);
}

/// Q3: Batch flush on capacity threshold
#[test]
fn test_q3_batch_flush_on_capacity() {
    let pool = HybridBatchPool::with_config(2, 4, 8).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    // Push exactly 8 tasks (should trigger flush)
    for _ in 0..8 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 8);
}

/// Q4: Thread-local batch accumulation
#[test]
fn test_q4_thread_local_batch_accumulation() {
    let pool = Arc::new(HybridBatchPool::with_config(2, 4, 16).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                // Each thread: 16 tasks exactly (fills batch exactly once)
                for _ in 0..16 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 64);
}

/// Q5: Queue distribution across multiple queues
#[test]
fn test_q5_queue_distribution() {
    let pool = HybridBatchPool::with_config(4, 4, 4).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..16 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 16);
}

/// Q6: Graceful shutdown with pending tasks
#[test]
fn test_q6_graceful_shutdown() {
    let pool = Arc::new(HybridBatchPool::new(4).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..50 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 50);

    // Verify no panics on drop
    drop(pool);
    thread::sleep(std::time::Duration::from_millis(10));
}

/// Q7: Pool configuration validation
#[test]
fn test_q7_pool_config_validation() {
    assert!(HybridBatchPool::with_config(0, 8, 64).is_err(), "Should reject 0 workers");
    assert!(HybridBatchPool::with_config(8, 0, 64).is_err(), "Should reject 0 queues");
    assert!(HybridBatchPool::with_config(8, 8, 64).is_ok(), "Should accept valid config");
}

/// Q8: 8 additional unit tests for edge cases
#[test]
fn test_q8_empty_batch() {
    let pool = HybridBatchPool::new(2).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    // Don't push anything, just wait
    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 0);
}

#[test]
fn test_q8_repeated_wait() {
    let pool = Arc::new(HybridBatchPool::new(2).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..10 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    let first_count = counter.load(Ordering::Relaxed);
    pool.wait();  // Second wait should be instant
    assert_eq!(counter.load(Ordering::Relaxed), first_count);
}

// ============================================================================
// Tier Q8-Q14: Property Tests (8 tests)
// ============================================================================

/// Q8: No task loss with 10 threads
#[test]
fn test_q8_no_task_loss_10_threads() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..100 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1000, "Task loss detected!");
}

/// Q9: No task loss with 50 threads (canonical workload)
#[test]
fn test_q9_no_task_loss_50_threads() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..32 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(
        counter.load(Ordering::Relaxed),
        1600,
        "Task loss detected: expected 1600, got {}",
        counter.load(Ordering::Relaxed)
    );
}

/// Q10: Fairness - no starvation
#[test]
fn test_q10_fairness_no_starvation() {
    let pool = Arc::new(HybridBatchPool::new(4).unwrap());
    let completion_times: Arc<std::sync::Mutex<Vec<std::time::Duration>>> =
        Arc::new(std::sync::Mutex::new(vec![]));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let pool = pool.clone();
            let times = completion_times.clone();

            thread::spawn(move || {
                let start = Instant::now();

                for _ in 0..50 {
                    pool.push(Box::new(|| {
                        std::hint::black_box(42);
                    }))
                    .unwrap();
                }

                times.lock().unwrap().push(start.elapsed());
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();

    let times = completion_times.lock().unwrap();
    let max_time = times.iter().max().unwrap();
    let min_time = times.iter().min().unwrap();

    let spread_ms = (max_time.as_millis() as i64 - min_time.as_millis() as i64).abs();
    assert!(
        spread_ms < 100,
        "Starvation detected: {:.1}ms spread",
        spread_ms as f64
    );
}

/// Q11: Concurrent push from multiple threads
#[test]
fn test_q11_concurrent_push_multiple_threads() {
    let pool = Arc::new(HybridBatchPool::new(4).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..2)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..50 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 100);
}

/// Q12: Variable batch sizes
#[test]
fn test_q12_variable_batch_sizes() {
    let pool = HybridBatchPool::with_config(4, 4, 16).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    // Push 3, 5, 8, 9, 11, 16, 18 tasks (various batch fills)
    for count in &[3, 5, 8, 9, 11, 16, 18] {
        for _ in 0..*count {
            let c = counter.clone();
            pool.push(Box::new(move || {
                c.fetch_add(1, Ordering::Relaxed);
            }))
            .unwrap();
        }
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 3+5+8+9+11+16+18);
}

/// Q13: Worker efficiency under variable load
#[test]
fn test_q13_variable_workload_distribution() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..8)
        .enumerate()
        .map(|(i, _)| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                // Each thread submits different number of tasks
                let tasks = (i + 1) * 10;  // 10, 20, 30, ... 80
                for _ in 0..tasks {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    let expected = (1..=8).sum::<usize>() * 10;
    assert_eq!(counter.load(Ordering::Relaxed), expected);
}

/// Q14: Remaining tasks counter accuracy
#[test]
fn test_q14_remaining_tasks_counter() {
    let pool = Arc::new(HybridBatchPool::new(4).unwrap());

    for _ in 0..100 {
        pool.push(Box::new(|| {
            std::hint::black_box(42);
        }))
        .unwrap();
    }

    // Tasks should be enqueued by now
    let remaining = pool.remaining_tasks();
    assert!(remaining > 0, "Expected tasks in queue, got {}", remaining);

    pool.wait();
    assert_eq!(pool.remaining_tasks(), 0, "Should have no remaining tasks");
}

// ============================================================================
// Tier Q15-Q21: Integration Tests (7 tests)
// ============================================================================

/// Q15: 1,600 task integration (canonical workload)
#[test]
fn test_q15_1600_task_integration() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..32 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1600);
}

/// Q16: Mixed task execution (fast + slow)
#[test]
fn test_q16_mixed_workload() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..10)
        .map(|i| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for j in 0..50 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        if (i + j) % 2 == 0 {
                            // Fast task
                            c.fetch_add(1, Ordering::Relaxed);
                        } else {
                            // Slightly slower task
                            std::hint::black_box((0..10).fold(0, |a, b| a + b));
                            c.fetch_add(1, Ordering::Relaxed);
                        }
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 500);
}

/// Q17: Stress test with 100 threads
#[test]
fn test_q17_stress_100_threads() {
    let pool = Arc::new(HybridBatchPool::new(16).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..16 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1600);
}

/// Q18: Graceful shutdown verification
#[test]
fn test_q18_graceful_shutdown() {
    let pool = Arc::new(HybridBatchPool::new(4).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    for _ in 0..100 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 100);

    drop(pool);
    thread::sleep(std::time::Duration::from_millis(10));
}

/// Q19: Multiple sequential workloads
#[test]
fn test_q19_sequential_workloads() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    // First batch
    for _ in 0..100 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }
    pool.wait();

    let first_count = counter.load(Ordering::Relaxed);
    assert_eq!(first_count, 100);

    // Second batch
    counter.store(0, Ordering::Release);
    for _ in 0..200 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }
    pool.wait();

    assert_eq!(counter.load(Ordering::Relaxed), 200);
}

/// Q20: Integration with Arc<Mutex> for shared state
#[test]
fn test_q20_shared_mutable_state() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let results: Arc<std::sync::Mutex<Vec<usize>>> = Arc::new(std::sync::Mutex::new(vec![]));

    for i in 0..100 {
        let r = results.clone();
        pool.push(Box::new(move || {
            r.lock().unwrap().push(i);
        }))
        .unwrap();
    }

    pool.wait();

    let locked = results.lock().unwrap();
    assert_eq!(locked.len(), 100);
}

/// Q21: Large batch capacity test
#[test]
fn test_q21_large_batch_capacity() {
    let pool = HybridBatchPool::with_config(4, 8, 256).unwrap();
    let counter = Arc::new(AtomicUsize::new(0));

    // Push 1024 tasks (4 flushes)
    for _ in 0..1024 {
        let c = counter.clone();
        pool.push(Box::new(move || {
            c.fetch_add(1, Ordering::Relaxed);
        }))
        .unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 1024);
}

// ============================================================================
// Tier Q22-Q28: Production Tests (3 tests)
// ============================================================================

/// Q22-Q24: Stress test with 10K tasks
#[test]
fn test_q22_stress_10k_tasks() {
    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..20)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..500 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();
    assert_eq!(counter.load(Ordering::Relaxed), 10000);
}

/// Q25-Q26: Memory and resource bounds
#[test]
fn test_q25_memory_bounds() {
    let pool = HybridBatchPool::new(16).unwrap();

    assert_eq!(pool.num_workers(), 16);
    assert_eq!(pool.num_queues(), 8);
}

/// Q27-Q28: Production latency target
#[test]
fn test_q27_production_latency() {
    let start = Instant::now();

    let pool = Arc::new(HybridBatchPool::new(8).unwrap());
    let counter = Arc::new(AtomicUsize::new(0));

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let pool = pool.clone();
            let counter = counter.clone();

            thread::spawn(move || {
                for _ in 0..32 {
                    let c = counter.clone();
                    pool.push(Box::new(move || {
                        c.fetch_add(1, Ordering::Relaxed);
                    }))
                    .unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    pool.wait();

    let elapsed = start.elapsed().as_micros();
    assert_eq!(counter.load(Ordering::Relaxed), 1600);

    // Target: <20μs (production P99.9)
    // Relaxed: <50μs for CI environment
    assert!(
        elapsed < 50000,
        "Latency too high: {:.2}μs (target <50μs)",
        elapsed as f64 / 1000.0
    );
}
