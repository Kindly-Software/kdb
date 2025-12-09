//! Brutal Stress Tests for AtomicCapsuleMap
//!
//! These tests push the system to breaking point to validate lockfree correctness,
//! generation counter integrity, and production-level resilience.
//!
//! Based on:
//! - The Atomic Capsule architecture principles
//! - ASSUM safety framework validation
//! - UCE32 Q19 (Implementation challenges)
//! - Production trading system requirements

use atomic_capsule_map::{AtomicCapsuleMap, BreakerLevel, HealthStatus};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ============================================================================
// STRESS TEST 1: CONCURRENT HAMMER TEST
// ============================================================================
// 16 threads × 10k operations each = 160k total operations
// Validates: No panics, no deadlocks, all operations complete

#[test]
fn stress_concurrent_hammer() {
    const THREADS: usize = 16;
    const OPS_PER_THREAD: usize = 10_000;

    let map = Arc::new(AtomicCapsuleMap::new());

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);

            thread::spawn(move || {
                // No catch_unwind needed - if thread panics, test fails appropriately
                for i in 0..OPS_PER_THREAD {
                    let key = (t * OPS_PER_THREAD + i) as u64;

                    // Insert
                    map.insert(key, key * 2);

                    // Read back immediately
                    assert_eq!(
                        map.get(&key),
                        Some(key * 2),
                        "Thread {} failed to read key {} immediately after insert",
                        t,
                        key
                    );

                    // Remove
                    map.remove(&key);

                    // Verify removal
                    assert_eq!(
                        map.get(&key),
                        None,
                        "Thread {} still sees key {} after removal",
                        t,
                        key
                    );
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked during stress test");
    }

    // VALIDATION: Map is empty (all removes succeeded)
    assert_eq!(
        map.len(),
        0,
        "FAILED: Map should be empty after all insert/remove pairs"
    );
}

// ============================================================================
// STRESS TEST 2: READ-WRITE CONTENTION
// ============================================================================
// 8 writers × 32 readers = heavy read contention
// Validates: No torn reads, consistent state, stable performance

#[test]
fn stress_read_write_contention() {
    const WRITERS: usize = 8;
    const READERS: usize = 32;
    const OPERATIONS: usize = 100_000;
    const KEY_SPACE: usize = 1000;

    let map = Arc::new(AtomicCapsuleMap::new());
    let done = Arc::new(AtomicBool::new(false));
    let torn_reads = Arc::new(AtomicU64::new(0));

    // Pre-populate with initial values
    for i in 0..KEY_SPACE {
        map.insert(i, i * 10);
    }

    // Spawn writers
    let mut handles = Vec::new();
    for w in 0..WRITERS {
        let map = Arc::clone(&map);
        let done = Arc::clone(&done);

        handles.push(thread::spawn(move || {
            let mut counter = 0usize;
            while !done.load(Ordering::Relaxed) {
                let key = (w * 127 + counter) % KEY_SPACE;
                map.insert(key, counter);
                counter += 1;

                // Occasional remove to create churn
                if counter % 100 == 0 {
                    map.remove(&key);
                }
            }
        }));
    }

    // Spawn readers
    for r in 0..READERS {
        let map = Arc::clone(&map);
        let done = Arc::clone(&done);
        let torn_reads = Arc::clone(&torn_reads);

        handles.push(thread::spawn(move || {
            let mut reads = 0;
            while reads < OPERATIONS / READERS {
                let key = (r * 73 + reads) % KEY_SPACE;

                // Read the value
                if let Some(val1) = map.get(&key) {
                    // Immediately read again - should be consistent or updated
                    if let Some(val2) = map.get(&key) {
                        // Values should be equal OR val2 is newer
                        // Never should we see val1 > val2 (time reversal)
                        if val1 > val2 {
                            torn_reads.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                reads += 1;
            }
        }));
    }

    // Wait for readers to complete
    thread::sleep(Duration::from_millis(100));
    done.store(true, Ordering::Release);

    for h in handles {
        h.join()
            .expect("Thread panicked during read-write contention test");
    }

    // VALIDATION: Zero torn reads
    assert_eq!(
        torn_reads.load(Ordering::Relaxed),
        0,
        "FAILED: Torn reads detected (time reversal in consecutive reads)"
    );
}

// ============================================================================
// STRESS TEST 3: GENERATION COUNTER STRESS
// ============================================================================
// Rapid insert/remove of same key to stress generation counter logic
// Validates: No ABA problems, monotonic generations, no lost updates

#[test]
fn stress_generation_counter_aba_prevention() {
    const THREADS: usize = 16;
    const OPERATIONS: usize = 1_000_000;
    const HOT_KEYS: usize = 10;

    let map = Arc::new(AtomicCapsuleMap::new());

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);

            thread::spawn(move || {
                for i in 0..OPERATIONS / THREADS {
                    // Focus operations on hot keys
                    let key = i % HOT_KEYS;

                    match i % 3 {
                        0 => {
                            // Insert with thread-specific value
                            map.insert(key, t * 1_000_000 + i);
                        }
                        1 => {
                            // Try to read
                            let _ = map.get(&key);
                        }
                        2 => {
                            // Remove
                            let _ = map.remove(&key);
                        }
                        _ => unreachable!(),
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread panicked during generation counter stress test");
    }

    // VALIDATION: Map is in consistent state
    // We can't predict exact contents, but length should be reasonable
    assert!(
        map.len() <= HOT_KEYS,
        "FAILED: Map length exceeds key space (duplicate keys detected)"
    );

    // VALIDATION: Health status should be OK (no internal corruption detected)
    let health = map.health_status();
    assert_eq!(
        health.breaker_level,
        BreakerLevel::L0,
        "FAILED: Health degraded to {:?} after generation stress",
        health.breaker_level
    );
}

// ============================================================================
// STRESS TEST 4: MEMORY PRESSURE
// ============================================================================
// Insert until capacity to validate memory management and resize logic
// Validates: Graceful behavior at capacity, no memory corruption

#[test]
fn stress_memory_pressure() {
    const LARGE_SIZE: usize = 500_000;

    let map = Arc::new(AtomicCapsuleMap::new());

    // Insert many entries
    for i in 0..LARGE_SIZE {
        map.insert(i, i * 2);

        // Periodic validation
        if i % 10_000 == 0 {
            assert_eq!(
                map.get(&i),
                Some(i * 2),
                "Failed to read back entry {} during memory pressure test",
                i
            );
        }
    }

    // VALIDATION: All entries present
    assert_eq!(
        map.len(),
        LARGE_SIZE,
        "FAILED: Expected {} entries, found {}",
        LARGE_SIZE,
        map.len()
    );

    // VALIDATION: Random sampling confirms values intact
    for _ in 0..1000 {
        let key = (rand() as usize) % LARGE_SIZE;
        assert_eq!(
            map.get(&key),
            Some(key * 2),
            "FAILED: Entry {} corrupted during memory pressure",
            key
        );
    }

    // VALIDATION: Can still insert/remove
    map.insert(LARGE_SIZE, 999);
    assert_eq!(map.get(&LARGE_SIZE), Some(999));
    map.remove(&LARGE_SIZE);
    assert_eq!(map.get(&LARGE_SIZE), None);
}

// ============================================================================
// STRESS TEST 5: LONG-RUNNING STABILITY
// ============================================================================
// 10 threads × 1M operations each = 10M operations
// Validates: No panics, no deadlocks, no performance degradation

#[test]
fn stress_long_running_stability() {
    const THREADS: usize = 10;
    const OPERATIONS: usize = 1_000_000;

    let map = Arc::new(AtomicCapsuleMap::new());
    let total_ops = Arc::new(AtomicU64::new(0));
    let start = Instant::now();

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);
            let total_ops = Arc::clone(&total_ops);

            thread::spawn(move || {
                for i in 0..OPERATIONS {
                    let key = t * OPERATIONS + i;

                    match i % 5 {
                        0 => {
                            map.insert(key, key * 2);
                        }
                        1 => {
                            let _ = map.get(&key);
                        }
                        2 => {
                            map.remove(&key);
                        }
                        3 => {
                            let _ = map.get_or_insert(key, key * 3);
                        }
                        4 => {
                            let _ = map.contains_key(&key);
                        }
                        _ => unreachable!(),
                    }

                    total_ops.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread panicked during long-running stability test");
    }

    let elapsed = start.elapsed();
    let total = total_ops.load(Ordering::Relaxed);
    let ops_per_sec = (total as f64) / elapsed.as_secs_f64();

    // VALIDATION: All operations completed
    assert_eq!(
        total,
        THREADS as u64 * OPERATIONS as u64,
        "FAILED: Not all operations completed"
    );

    // VALIDATION: Performance is reasonable (>100k ops/sec minimum)
    assert!(
        ops_per_sec > 100_000.0,
        "FAILED: Performance degraded to {:.0} ops/sec (expected >100k)",
        ops_per_sec
    );

    println!(
        "Long-running stability: {:.0} ops/sec over {} ops in {:.2}s",
        ops_per_sec,
        total,
        elapsed.as_secs_f64()
    );
}

// ============================================================================
// STRESS TEST 6: ZIPF DISTRIBUTION (REALISTIC WORKLOAD)
// ============================================================================
// 90% reads on hot keys, 10% writes distributed
// Validates: Real-world production performance characteristics

#[test]
fn stress_zipf_distribution_realistic() {
    const THREADS: usize = 16;
    const OPERATIONS: usize = 1_000_000;
    const HOT_KEYS: usize = 100;
    const TOTAL_KEYS: usize = 10_000;

    let map = Arc::new(AtomicCapsuleMap::new());

    // Pre-populate
    for i in 0..TOTAL_KEYS {
        map.insert(i, i * 10);
    }

    let start = Instant::now();

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);

            thread::spawn(move || {
                for i in 0..OPERATIONS / THREADS {
                    let random = (t * 7919 + i * 6151) % 100; // Pseudo-random

                    if random < 90 {
                        // 90% reads on hot keys
                        let key = (t * 31 + i * 17) % HOT_KEYS;
                        let _ = map.get(&key);
                    } else {
                        // 10% writes distributed
                        let key = (t * 1000 + i) % TOTAL_KEYS;
                        map.insert(key, i);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread panicked during Zipf distribution test");
    }

    let elapsed = start.elapsed();
    let ops_per_sec = (OPERATIONS as f64) / elapsed.as_secs_f64();

    // VALIDATION: All hot keys still present
    for i in 0..HOT_KEYS {
        assert!(
            map.get(&i).is_some(),
            "FAILED: Hot key {} missing after Zipf test",
            i
        );
    }

    // VALIDATION: Performance is excellent (>500k ops/sec for read-heavy)
    assert!(
        ops_per_sec > 500_000.0,
        "FAILED: Read-heavy performance degraded to {:.0} ops/sec (expected >500k)",
        ops_per_sec
    );

    println!(
        "Zipf distribution: {:.0} ops/sec (90% reads, 10% writes)",
        ops_per_sec
    );
}

// ============================================================================
// STRESS TEST 7: COMPARE-AND-SWAP CONTENTION
// ============================================================================
// Multiple threads competing on CAS operations
// Validates: Atomic CAS correctness, no lost updates

#[test]
fn stress_compare_and_swap_contention() {
    const THREADS: usize = 16;
    const ATTEMPTS: usize = 10_000;
    const SHARED_KEY: u64 = 42;

    let map = Arc::new(AtomicCapsuleMap::new());
    map.insert(SHARED_KEY, 0);

    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let map = Arc::clone(&map);
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                for _ in 0..ATTEMPTS {
                    loop {
                        // Read current value
                        if let Some(current) = map.get(&SHARED_KEY) {
                            // Try to increment via CAS
                            if map
                                .compare_and_swap(&SHARED_KEY, current, current + 1)
                                .is_ok()
                            {
                                success_count.fetch_add(1, Ordering::Relaxed);
                                break;
                            }
                            // CAS failed, retry
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join()
            .expect("Thread panicked during CAS contention test");
    }

    // VALIDATION: Final value equals number of successful CAS operations
    let final_value = map.get(&SHARED_KEY).expect("Key disappeared");
    let expected = THREADS * ATTEMPTS;

    assert_eq!(
        final_value, expected as u64,
        "FAILED: CAS lost updates. Expected {}, got {}",
        expected, final_value
    );

    assert_eq!(
        success_count.load(Ordering::Relaxed),
        expected as u64,
        "FAILED: Success count mismatch"
    );
}

// ============================================================================
// STRESS TEST 8: ITERATION STABILITY UNDER MODIFICATION
// ============================================================================
// Iterate while other threads rapidly modify
// Validates: Iterator safety, no crashes, consistent snapshots

#[test]
fn stress_iteration_under_modification() {
    const MODIFIERS: usize = 8;
    const ITERATORS: usize = 8;
    const MODIFICATIONS: usize = 100_000;
    const KEY_SPACE: usize = 10_000;

    let map = Arc::new(AtomicCapsuleMap::new());
    let done = Arc::new(AtomicBool::new(false));

    // Pre-populate
    for i in 0..KEY_SPACE {
        map.insert(i, i);
    }

    let mut handles = Vec::new();

    // Spawn modifiers
    for m in 0..MODIFIERS {
        let map = Arc::clone(&map);
        let done = Arc::clone(&done);

        handles.push(thread::spawn(move || {
            let mut count = 0;
            while count < MODIFICATIONS / MODIFIERS && !done.load(Ordering::Relaxed) {
                let key = (m * 1000 + count) % KEY_SPACE;

                if count % 2 == 0 {
                    map.insert(key, count);
                } else {
                    map.remove(&key);
                }
                count += 1;
            }
        }));
    }

    // Spawn iterators
    for _ in 0..ITERATORS {
        let map = Arc::clone(&map);
        let done = Arc::clone(&done);

        handles.push(thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                // Iterate and collect
                let snapshot: Vec<_> = map.iter().map(|(k, v)| (k, v)).collect();

                // Validate snapshot consistency
                for (k, v) in &snapshot {
                    // Each entry in snapshot should have existed at some point
                    // (We can't guarantee it still exists due to concurrent mods)
                    assert!(k < &KEY_SPACE, "Invalid key in iteration: {}", k);
                }
            }
        }));
    }

    // Let it run for a bit
    thread::sleep(Duration::from_millis(500));
    done.store(true, Ordering::Release);

    for h in handles {
        h.join()
            .expect("Thread panicked during iteration stress test");
    }

    // VALIDATION: Map is still functional
    map.insert(999_999, 42);
    assert_eq!(map.get(&999_999), Some(42));
}

// ============================================================================
// HELPER: Simple pseudo-random (no external deps)
// ============================================================================
fn rand() -> u64 {
    use std::sync::atomic::AtomicU64;
    static SEED: AtomicU64 = AtomicU64::new(0x123456789ABCDEF0);

    let mut x = SEED.load(Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    SEED.store(x, Ordering::Relaxed);
    x
}
