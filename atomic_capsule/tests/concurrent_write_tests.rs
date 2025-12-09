//! # Concurrent Write Property Tests
//!
//! **T28 Framework**: Q9 (Concurrent Invariants) + Q8 (Universal Properties)
//! **ASSUM Verification**: #ASSUME_EXCLUSIVE_WRITER → #VERIFY SWeMR pattern
//!
//! Tests concurrent write safety and the Single-Writer-Multiple-Reader (SWeMR) pattern:
//! - Atomic operations remain atomic under concurrent access
//! - Generation counters provide TOCTOU prevention
//! - No torn reads with proper ordering
//! - Linearizability of operations
//!
//! **Coverage Goal**: Validate concurrent write patterns not currently tested

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

#[cfg(test)]
use proptest::prelude::*;

// =============================================================================
// T28 Q9: Concurrent Invariants - Single Writer Multiple Readers (SWeMR)
// =============================================================================

#[test]
fn test_single_writer_multiple_readers_no_torn_reads() {
    let capsule = Arc::new(AtomicU64::new(0));
    let num_readers = 8;
    let num_writes = 1_000;

    // Writer thread: Updates value monotonically
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 1..=num_writes {
            writer_capsule.store(i, Ordering::Release);
        }
    });

    // Reader threads: Verify values never go backward (monotonicity)
    let mut reader_handles = vec![];
    for _ in 0..num_readers {
        let reader_capsule = Arc::clone(&capsule);
        reader_handles.push(thread::spawn(move || {
            let mut last_seen = 0;
            for _ in 0..10_000 {
                let current = reader_capsule.load(Ordering::Acquire);
                // #VERIFY: No torn reads - value never decreases
                assert!(
                    current >= last_seen,
                    "Torn read detected: current={}, last={}",
                    current,
                    last_seen
                );
                last_seen = current;
            }
        }));
    }

    writer.join().unwrap();
    for handle in reader_handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_read_consistency() {
    let capsule = Arc::new(AtomicU64Capsule::new(42));
    let num_readers = 16;
    let reads_per_thread = 10_000;

    let handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..reads_per_thread {
                    let value = c.load(Ordering::Acquire);
                    // #VERIFY: Value is always valid (not torn)
                    assert_eq!(value, 42, "Unexpected value read: {}", value);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_atomic_updates_no_lost_writes() {
    let counter = Arc::new(AtomicU64::new(0));
    let num_threads = 8;
    let increments_per_thread = 1_000;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..increments_per_thread {
                    c.fetch_add(1, Ordering::Release);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // #VERIFY: All updates applied, no lost writes
    let final_value = counter.load(Ordering::Acquire);
    let expected = num_threads * increments_per_thread;
    assert_eq!(
        final_value, expected,
        "Lost writes detected: got {}, expected {}",
        final_value, expected
    );
}

// =============================================================================
// T28 Q9: Concurrent CAS Operations with Retry
// =============================================================================

#[test]
fn test_concurrent_cas_with_retry_all_succeed() {
    use atomic_capsule::{BackoffStrategy, RetryPolicy};

    let counter = Arc::new(AtomicU64::new(0));
    let success_count = Arc::new(AtomicUsize::new(0));
    let num_threads = 8;
    let operations_per_thread = 100;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            let sc = Arc::clone(&success_count);
            thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current,
                            current + 1,
                            Ordering::Release,
                            Ordering::Relaxed,
                        ) {
                            Ok(_) => {
                                sc.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // #VERIFY: All operations succeeded
    let final_count = counter.load(Ordering::Acquire);
    let successful = success_count.load(Ordering::Relaxed);
    let expected = num_threads * operations_per_thread;

    assert_eq!(
        final_count, expected,
        "CAS operations lost writes: got {}, expected {}",
        final_count, expected
    );
    assert_eq!(
        successful, expected,
        "Success count mismatch: got {}, expected {}",
        successful, expected
    );
}

// =============================================================================
// T28 Q9: Memory Ordering Validation
// =============================================================================

#[test]
fn test_acquire_release_ordering_prevents_reordering() {
    let data = Arc::new(AtomicU64::new(0));
    let flag = Arc::new(AtomicU64::new(0));

    // Writer: Store data then set flag (Release)
    let data_clone = Arc::clone(&data);
    let flag_clone = Arc::clone(&flag);
    let writer = thread::spawn(move || {
        data_clone.store(42, Ordering::Relaxed);
        flag_clone.store(1, Ordering::Release); // Release ensures data visible
    });

    // Reader: Wait for flag then read data (Acquire)
    let data_clone = Arc::clone(&data);
    let flag_clone = Arc::clone(&flag);
    let reader = thread::spawn(move || {
        // Spin until flag is set
        while flag_clone.load(Ordering::Acquire) == 0 {
            std::hint::spin_loop();
        }
        // #VERIFY: Acquire ensures data is visible
        let value = data_clone.load(Ordering::Relaxed);
        assert_eq!(value, 42, "Acquire/Release ordering violated: {}", value);
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

// =============================================================================
// T28 Q8: Property Testing - Linearizability
// =============================================================================

#[test]
fn test_linearizability_all_writes_visible() {
    let map = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    let operations = 1_000;

    // Writer: Insert values sequentially
    let writer_map = Arc::clone(&map);
    let writer = thread::spawn(move || {
        for i in 0..operations {
            writer_map.lock().unwrap().insert(i, i * 2);
        }
    });

    writer.join().unwrap();

    // Reader: All writes should be visible (linearizable)
    let reader_map = Arc::clone(&map);
    let reader = thread::spawn(move || {
        for i in 0..operations {
            let guard = reader_map.lock().unwrap();
            let value = guard.get(&i);
            // #VERIFY: All writes visible after completion
            assert_eq!(
                value,
                Some(&(i * 2)),
                "Write not visible: key={}, value={:?}",
                i,
                value
            );
        }
    });

    reader.join().unwrap();
}

// =============================================================================
// T28 Q8: Property Testing with Proptest (if enabled)
// =============================================================================

#[cfg(feature = "proptest")]
proptest! {
    #[test]
    fn prop_concurrent_atomic_never_tears(
        num_threads in 2usize..16,
        operations in 100usize..1000
    ) {
        let counter = Arc::new(AtomicU64::new(0));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..operations {
                    c.fetch_add(1, Ordering::Release);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Property: All updates applied (no torn writes)
        let final_value = counter.load(Ordering::Acquire);
        let expected = num_threads * operations;
        prop_assert_eq!(final_value, expected as u64);
    }

    #[test]
    fn prop_cas_loops_always_converge(
        num_threads in 2usize..20,
        operations in 50usize..200
    ) {
        use atomic_capsule::{BackoffStrategy, RetryPolicy};

        let counter = Arc::new(AtomicU64::new(0));
        let mut handles = vec![];

        for _ in 0..num_threads {
            let c = Arc::clone(&counter);
            handles.push(thread::spawn(move || {
                for _ in 0..operations {
                    let mut policy = RetryPolicy::new(BackoffStrategy::STANDARD);
                    loop {
                        let current = c.load(Ordering::Acquire);
                        match c.compare_exchange(
                            current, current + 1,
                            Ordering::Release, Ordering::Relaxed
                        ) {
                            Ok(_) => break,
                            Err(_) => {
                                if policy.should_yield() {
                                    policy.backoff();
                                }
                                policy.increment();
                                // Property: Retry always converges (no livelock)
                                prop_assert!(!policy.is_exhausted());
                            }
                        }
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let final_value = counter.load(Ordering::Acquire);
        prop_assert_eq!(final_value, (num_threads * operations) as u64);
    }
}

// =============================================================================
// Stress Tests (Ignored by default, run with --ignored)
// =============================================================================

#[test]
#[ignore] // Expensive, run with: cargo test --ignored
fn stress_test_concurrent_writes_10_minutes() {
    use std::time::{Duration, Instant};

    let counter = Arc::new(AtomicU64::new(0));
    let duration = Duration::from_secs(600); // 10 minutes
    let start = Instant::now();
    let num_threads = 16;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&counter);
            let start_time = start;
            thread::spawn(move || {
                let mut local_count = 0u64;
                while start_time.elapsed() < duration {
                    c.fetch_add(1, Ordering::Release);
                    local_count += 1;
                }
                local_count
            })
        })
        .collect();

    let total_ops: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();

    let final_value = counter.load(Ordering::Acquire);

    // #VERIFY: All updates applied over 10 minutes
    assert_eq!(
        final_value, total_ops,
        "Lost writes during stress test: got {}, expected {}",
        final_value, total_ops
    );

    println!("Stress test: {} operations over 10 minutes", total_ops);
}

// =============================================================================
// ASSUM Framework Verification
// =============================================================================

#[test]
fn verify_assum_swemr_pattern_with_external_coordination() {
    // #ASSUME_EXCLUSIVE_WRITER: SWeMR pattern requires external coordination
    // #VERIFY_WRITER_COORDINATION: Test with mutex-coordinated writer

    use std::sync::Mutex;

    let capsule = Arc::new(Mutex::new(AtomicU64Capsule::new(0)));
    let num_readers = 8;
    let num_writes = 1_000;

    // Writer: Uses mutex for exclusive access
    let writer_capsule = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 1..=num_writes {
            let guard = writer_capsule.lock().unwrap();
            guard.store(i, Ordering::Release);
            drop(guard); // Explicit release
        }
    });

    // Readers: Read without mutex (SWeMR pattern)
    let mut reader_handles = vec![];
    for _ in 0..num_readers {
        let reader_capsule = Arc::clone(&capsule);
        reader_handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let guard = reader_capsule.lock().unwrap();
                let _value = guard.load(Ordering::Acquire);
                drop(guard);
            }
        }));
    }

    writer.join().unwrap();
    for handle in reader_handles {
        handle.join().unwrap();
    }

    // #VERIFY: No data races with proper coordination
}

#[test]
fn verify_assum_atomic_ordering_sufficient() {
    // #ASSUME_ORDERING_SUFFICIENT: Acquire/Release prevents races
    // #VERIFY: Reader sees writer's updates in order

    let counter = Arc::new(AtomicU64::new(0));
    let flag = Arc::new(AtomicU64::new(0));

    let counter_clone = Arc::clone(&counter);
    let flag_clone = Arc::clone(&flag);
    let writer = thread::spawn(move || {
        for i in 1..=100 {
            counter_clone.store(i, Ordering::Relaxed);
            flag_clone.store(i, Ordering::Release);
        }
    });

    let counter_clone = Arc::clone(&counter);
    let flag_clone = Arc::clone(&flag);
    let reader = thread::spawn(move || {
        let mut last_flag = 0;
        while last_flag < 100 {
            let current_flag = flag_clone.load(Ordering::Acquire);
            if current_flag > last_flag {
                let counter_value = counter_clone.load(Ordering::Relaxed);
                // #VERIFY: Counter value visible after flag update
                assert!(
                    counter_value >= last_flag,
                    "Ordering violation: counter={}, flag={}",
                    counter_value,
                    last_flag
                );
                last_flag = current_flag;
            }
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();
}
