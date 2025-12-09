//! T28 Q29-Q35 Determinism Tier Safety Tests for DualAtomicU64
//!
//! **Framework**: T28 Testing Framework Q29-Q35 (Production determinism tier)
//! **Capsule**: DualAtomicU64 (T1 Atomic tier, 128-byte aligned dual-channel)
//! **Status**: Production-ready determinism validation
//!
//! ## Test Coverage (28 tests)
//! - Q29-Q30: GenerationWriteGuard (7 tests)
//! - Q31-Q32: ConsistentRead (7 tests)
//! - Q33-Q34: Safe API (7 tests)
//! - Q35: Concurrent Determinism (7 tests)
//!
//! ## ASSUM Framework
//! - #ASSUME_GENERATION_INCREMENT: Guard increments generation AFTER writes complete (even on panic)
//! - #VERIFY_GENERATION_INCREMENT: test_generation_guard_panic_safety validates panic path
//! - #ASSUME_CONSISTENT_READ: Generation matching proves TOCTOU consistency
//! - #VERIFY_CONSISTENT_READ: test_consistent_read_detects_concurrent_write validates detection
//! - #ASSUME_ACQUIRE_RELEASE: Acquire/Release establishes happens-before
//! - #VERIFY_ACQUIRE_RELEASE: test_safe_api_acquire_release_pairing validates synchronization
//! - #ASSUME_MONOTONIC_GENERATION: Concurrent writes maintain monotonicity
//! - #VERIFY_MONOTONIC_GENERATION: test_concurrent_generation_monotonicity validates (10K iterations)

use atomic_capsule::patterns::dual_atomic::{ConsistentRead, DualAtomicU64, GenerationWriteGuard};
use core::sync::atomic::Ordering;
use std::panic;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Q29-Q30: GenerationWriteGuard Tests (7 tests)
// ============================================================================

/// Q29: GenerationWriteGuard increments generation on drop
///
/// # ASSUM Framework
/// - #ASSUME_DROP_INCREMENT: Guard increments generation when dropped
/// - #VERIFY_DROP_INCREMENT: Single-threaded drop validation
#[test]
fn test_generation_guard_increments_on_drop() {
    let dual = DualAtomicU64::new(0, 0);

    assert_eq!(dual.load_secondary_acquire(), 0);

    {
        let _guard = dual.begin_write();
        dual.store_primary_release(42);
        // Generation still 0 (guard not dropped yet)
        assert_eq!(dual.load_secondary_acquire(), 0);
    } // Guard drops here, increments generation

    // After guard drop, generation incremented
    assert_eq!(dual.load_secondary_acquire(), 1);
    assert_eq!(dual.load_primary_acquire(), 42);
}

/// Q29: GenerationWriteGuard increments generation AFTER writes complete
///
/// # ASSUM Framework
/// - #ASSUME_AFTER_WRITE: Generation increments AFTER all writes in scope
/// - #VERIFY_AFTER_WRITE: Multi-write validation (primary + secondary updates)
#[test]
fn test_generation_guard_increments_after_write() {
    let dual = DualAtomicU64::new(0, 0);

    {
        let _guard = dual.begin_write();
        dual.store_primary_release(100);
        dual.store_secondary_release(200);

        // Generation should still be 200 (guard tracks secondary)
        // Note: This test validates generation increment on DROP, not secondary store
        assert_eq!(dual.load_secondary_acquire(), 200);
    }

    // After guard drop, generation incremented (200 -> 201)
    assert_eq!(dual.load_secondary_acquire(), 201);
    assert_eq!(dual.load_primary_acquire(), 100);
}

/// Q30: GenerationWriteGuard increments generation even on panic
///
/// # ASSUM Framework
/// - #ASSUME_PANIC_SAFETY: Guard increments generation on panic path
/// - #VERIFY_PANIC_SAFETY: Panic recovery validation
///
/// # Critical Safety Property
/// Even if code panics during write, the generation MUST increment to prevent
/// readers from seeing partially-updated state as "consistent".
#[test]
fn test_generation_guard_panic_safety() {
    let dual = DualAtomicU64::new(0, 0);

    // Attempt write that panics mid-update
    let result = panic::catch_unwind(|| {
        let _guard = dual.begin_write();
        dual.store_primary_release(999);
        panic!("Simulated panic during write");
    });

    assert!(result.is_err(), "Expected panic");

    // Generation MUST have incremented (0 -> 1) despite panic
    // This prevents readers from seeing (999, gen=0) as "consistent"
    assert_eq!(
        dual.load_secondary_acquire(),
        1,
        "Generation must increment on panic to prevent stale reads"
    );
    assert_eq!(dual.load_primary_acquire(), 999);
}

/// Q30: GenerationWriteGuard complete() method increments immediately
///
/// # ASSUM Framework
/// - #ASSUME_EXPLICIT_COMPLETE: complete() increments generation immediately
/// - #VERIFY_EXPLICIT_COMPLETE: Immediate increment validation
#[test]
fn test_generation_guard_complete_method() {
    let dual = DualAtomicU64::new(0, 0);

    {
        let guard = dual.begin_write();
        dual.store_primary_release(42);

        // Explicitly complete (increments NOW)
        let new_gen = guard.complete();
        assert_eq!(new_gen, 1, "complete() returns new generation value");

        // Generation already incremented
        assert_eq!(dual.load_secondary_acquire(), 1);
    } // Guard drops, but should NOT increment again

    // After guard drop, generation still 1 (not double-incremented)
    assert_eq!(dual.load_secondary_acquire(), 1);
}

/// Q30: GenerationWriteGuard complete() prevents double increment
///
/// # ASSUM Framework
/// - #ASSUME_NO_DOUBLE_INCREMENT: complete() + drop does NOT double-increment
/// - #VERIFY_NO_DOUBLE_INCREMENT: Single-increment validation
#[test]
fn test_generation_guard_complete_prevents_double_increment() {
    let dual = DualAtomicU64::new(0, 5);

    {
        let guard = dual.begin_write();
        dual.store_primary_release(123);

        // Explicitly complete
        let gen1 = guard.complete();
        assert_eq!(gen1, 6);

        // Generation incremented once
        assert_eq!(dual.load_secondary_acquire(), 6);
    } // Guard drops, but completed flag prevents second increment

    // After drop, generation STILL 6 (not 7)
    assert_eq!(dual.load_secondary_acquire(), 6);
}

/// Q30: Nested GenerationWriteGuards increment independently
///
/// # ASSUM Framework
/// - #ASSUME_NESTED_GUARDS: Each guard increments independently
/// - #VERIFY_NESTED_GUARDS: Multi-guard validation
#[test]
fn test_generation_guard_nested_guards() {
    let dual = DualAtomicU64::new(0, 0);

    {
        let _outer = dual.begin_write();
        dual.store_primary_release(10);

        {
            let _inner = dual.begin_write();
            dual.store_primary_release(20);
        } // Inner guard drops, gen 0 -> 1

        assert_eq!(dual.load_secondary_acquire(), 1);
        dual.store_primary_release(30);
    } // Outer guard drops, gen 1 -> 2

    assert_eq!(dual.load_secondary_acquire(), 2);
    assert_eq!(dual.load_primary_acquire(), 30);
}

/// Q30: Concurrent GenerationWriteGuards maintain safety
///
/// # ASSUM Framework
/// - #ASSUME_CONCURRENT_GUARDS: Multiple threads can use guards concurrently
/// - #VERIFY_CONCURRENT_GUARDS: Multi-threaded guard stress test
///
/// # Performance
/// - 16 threads × 1000 writes = 16K total writes
/// - Generation should reach 16,000
#[test]
fn test_generation_guard_concurrent() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    const THREADS: usize = 16;
    const WRITES_PER_THREAD: usize = 1000;

    for _ in 0..THREADS {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..WRITES_PER_THREAD {
                let _guard = dual_clone.begin_write();
                dual_clone.store_primary_release(i as u64);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Generation incremented 16,000 times
    assert_eq!(
        dual.load_secondary_acquire(),
        (THREADS * WRITES_PER_THREAD) as u64
    );
}

// ============================================================================
// Q31-Q32: ConsistentRead Tests (7 tests)
// ============================================================================

/// Q31: ConsistentRead basic usage
///
/// # ASSUM Framework
/// - #ASSUME_CONSISTENT_BASIC: read_consistent() returns Some when no concurrent write
/// - #VERIFY_CONSISTENT_BASIC: Single-threaded consistency validation
#[test]
fn test_consistent_read_basic() {
    let dual = DualAtomicU64::new(42, 0);

    let consistent = dual.read_consistent();
    assert!(consistent.is_some(), "Read should be consistent");

    let read = consistent.unwrap();
    assert_eq!(*read.value(), 42);
    assert_eq!(read.generation(), 0);
}

/// Q31: ConsistentRead detects concurrent write
///
/// # ASSUM Framework
/// - #ASSUME_DETECT_CONCURRENT: read_consistent() returns None on concurrent write
/// - #VERIFY_DETECT_CONCURRENT: Multi-threaded TOCTOU detection
///
/// # Performance
/// - 100 iterations to increase detection probability
#[test]
fn test_consistent_read_detects_concurrent_write() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let detected = Arc::new(AtomicBool::new(false));

    let dual_writer = Arc::clone(&dual);
    let writer = thread::spawn(move || {
        for i in 0..1000 {
            dual_writer.write_with_generation(i);
            thread::yield_now();
        }
    });

    let dual_reader = Arc::clone(&dual);
    let detected_reader = Arc::clone(&detected);
    let reader = thread::spawn(move || {
        for _ in 0..1000 {
            if dual_reader.read_consistent().is_none() {
                detected_reader.store(true, Ordering::Release);
                break;
            }
            thread::yield_now();
        }
    });

    writer.join().unwrap();
    reader.join().unwrap();

    // At least one inconsistent read should be detected
    assert!(
        detected.load(Ordering::Acquire),
        "Should detect at least one concurrent write"
    );
}

/// Q32: ConsistentRead map function preserves consistency proof
///
/// # ASSUM Framework
/// - #ASSUME_MAP_PRESERVES: map() preserves generation proof
/// - #VERIFY_MAP_PRESERVES: Type-state validation
#[test]
fn test_consistent_read_map_function() {
    let dual = DualAtomicU64::new(10, 5);

    let read = dual.read_consistent().expect("Read should be consistent");
    assert_eq!(*read.value(), 10);
    assert_eq!(read.generation(), 5);

    // Map to doubled value
    let doubled = read.map(|v| v * 2);
    assert_eq!(*doubled.value(), 20);
    assert_eq!(doubled.generation(), 5, "Generation preserved through map");
}

/// Q32: ConsistentRead into_value consumes the read
///
/// # ASSUM Framework
/// - #ASSUME_INTO_VALUE: into_value() consumes ConsistentRead
/// - #VERIFY_INTO_VALUE: Ownership validation
#[test]
fn test_consistent_read_into_value() {
    let dual = DualAtomicU64::new(123, 7);

    let read = dual.read_consistent().expect("Read should be consistent");
    let value = read.into_value();

    assert_eq!(value, 123);
}

/// Q32: with_consistent retries on contention
///
/// # ASSUM Framework
/// - #ASSUME_WITH_RETRY: with_consistent() spins until consistent read
/// - #VERIFY_WITH_RETRY: Retry behavior validation
#[test]
fn test_with_consistent_retries_on_contention() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));

    let dual_writer = Arc::clone(&dual);
    let writer = thread::spawn(move || {
        for i in 1..=100 {
            dual_writer.write_with_generation(i);
            thread::sleep(Duration::from_micros(10));
        }
    });

    let dual_reader = Arc::clone(&dual);
    let reader = thread::spawn(move || {
        // with_consistent will retry until it gets a consistent read
        let value = dual_reader.with_consistent(|v| v);
        // Should eventually succeed (value between 1-100)
        assert!(value > 0 && value <= 100);
    });

    writer.join().unwrap();
    reader.join().unwrap();
}

/// Q32: with_consistent closure execution
///
/// # ASSUM Framework
/// - #ASSUME_CLOSURE_EXECUTION: Closure runs with consistent value
/// - #VERIFY_CLOSURE_EXECUTION: Closure behavior validation
#[test]
fn test_with_consistent_closure_execution() {
    let dual = DualAtomicU64::new(50, 0);

    let result = dual.with_consistent(|v| {
        // Closure receives consistent value
        assert_eq!(v, 50);
        v * 3
    });

    assert_eq!(result, 150);
}

/// Q32: ConsistentRead generation matches before/after
///
/// # ASSUM Framework
/// - #ASSUME_GENERATION_MATCH: Generation unchanged during consistent read
/// - #VERIFY_GENERATION_MATCH: Generation stability validation
#[test]
fn test_consistent_read_generation_matches() {
    let dual = DualAtomicU64::new(77, 12);

    for _ in 0..100 {
        if let Some(read) = dual.read_consistent() {
            // Generation in ConsistentRead is the "before" generation
            let gen_before = read.generation();
            let gen_after = dual.load_secondary_acquire();

            // Generation should match (no concurrent write occurred)
            assert_eq!(gen_before, gen_after);
            break;
        }
    }
}

// ============================================================================
// Q33-Q34: Safe API Tests (7 tests)
// ============================================================================

/// Q33: Safe API Acquire/Release pairing
///
/// # ASSUM Framework
/// - #ASSUME_ACQUIRE_RELEASE_PAIRING: load_*_acquire pairs with store_*_release
/// - #VERIFY_ACQUIRE_RELEASE_PAIRING: Happens-before validation
#[test]
fn test_safe_api_acquire_release_pairing() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));

    let dual_writer = Arc::clone(&dual);
    let writer = thread::spawn(move || {
        dual_writer.store_primary_release(999);
        dual_writer.store_secondary_release(111);
    });

    writer.join().unwrap();

    // Acquire loads see writes from Release stores
    let primary = dual.load_primary_acquire();
    let secondary = dual.load_secondary_acquire();

    assert_eq!(primary, 999);
    assert_eq!(secondary, 111);
}

/// Q33: Safe API prevents ordering misuse
///
/// # ASSUM Framework
/// - #ASSUME_SAFE_API_PREVENTS_MISUSE: Type system prevents incorrect ordering
/// - #VERIFY_SAFE_API_PREVENTS_MISUSE: API usage validation
///
/// This test validates that the safe API provides the correct orderings
/// without requiring the caller to choose.
#[test]
fn test_safe_api_prevents_ordering_misuse() {
    let dual = DualAtomicU64::new(0, 0);

    // Safe API methods have fixed orderings (no manual Ordering parameter)
    dual.store_primary_release(42);
    let value = dual.load_primary_acquire();
    assert_eq!(value, 42);

    // Publish generation with correct ordering
    let old_gen = dual.publish_generation();
    assert_eq!(old_gen, 0);
    assert_eq!(dual.load_secondary_acquire(), 1);
}

/// Q34: Safe API relaxed metrics isolation
///
/// # ASSUM Framework
/// - #ASSUME_RELAXED_METRICS_ISOLATION: Relaxed loads isolated from coordination
/// - #VERIFY_RELAXED_METRICS_ISOLATION: Non-synchronization validation
///
/// Relaxed loads should NOT be used for coordination, only metrics.
#[test]
fn test_safe_api_relaxed_metrics_isolation() {
    let dual = DualAtomicU64::new(123, 456);

    // Relaxed loads for metrics (no synchronization)
    let primary_metric = dual.load_primary_relaxed_metrics();
    let secondary_metric = dual.load_secondary_relaxed_metrics();

    assert_eq!(primary_metric, 123);
    assert_eq!(secondary_metric, 456);

    // These loads provide no guarantees about observing concurrent writes
}

/// Q34: write_with_generation atomic publication
///
/// # ASSUM Framework
/// - #ASSUME_WRITE_WITH_GEN_ATOMIC: write_with_generation is atomic publish
/// - #VERIFY_WRITE_WITH_GEN_ATOMIC: Atomicity validation
#[test]
fn test_write_with_generation_atomic() {
    let dual = DualAtomicU64::new(0, 0);

    dual.write_with_generation(42);

    // Primary updated, generation incremented
    assert_eq!(dual.load_primary_acquire(), 42);
    assert_eq!(dual.load_secondary_acquire(), 1);
}

/// Q34: cas_with_generation success increments generation
///
/// # ASSUM Framework
/// - #ASSUME_CAS_SUCCESS_INCREMENT: CAS success increments generation
/// - #VERIFY_CAS_SUCCESS_INCREMENT: Success path validation
#[test]
fn test_cas_with_generation_success() {
    let dual = DualAtomicU64::new(10, 0);

    let result = dual.cas_with_generation(10, 20);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 10, "CAS returns old value");
    assert_eq!(dual.load_primary_acquire(), 20);
    assert_eq!(dual.load_secondary_acquire(), 1, "Generation incremented");
}

/// Q34: cas_with_generation failure does NOT increment generation
///
/// # ASSUM Framework
/// - #ASSUME_CAS_FAILURE_NO_INCREMENT: CAS failure leaves generation unchanged
/// - #VERIFY_CAS_FAILURE_NO_INCREMENT: Failure path validation
#[test]
fn test_cas_with_generation_failure_no_increment() {
    let dual = DualAtomicU64::new(10, 5);

    let result = dual.cas_with_generation(999, 20);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), 10, "CAS returns current value");
    assert_eq!(dual.load_primary_acquire(), 10, "Primary unchanged");
    assert_eq!(dual.load_secondary_acquire(), 5, "Generation NOT incremented");
}

/// Q34: publish_generation ordering
///
/// # ASSUM Framework
/// - #ASSUME_PUBLISH_GENERATION_RELEASE: publish_generation uses Release ordering
/// - #VERIFY_PUBLISH_GENERATION_RELEASE: Ordering validation
#[test]
fn test_publish_generation_ordering() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));

    let dual_writer = Arc::clone(&dual);
    let writer = thread::spawn(move || {
        dual_writer.store_primary_release(888);
        dual_writer.publish_generation();
    });

    writer.join().unwrap();

    // Acquire load sees Release publish
    let gen = dual.load_secondary_acquire();
    assert_eq!(gen, 1);

    let value = dual.load_primary_acquire();
    assert_eq!(value, 888);
}

// ============================================================================
// Q35: Concurrent Determinism Tests (7 tests)
// ============================================================================

/// Q35: Concurrent generation monotonicity
///
/// # ASSUM Framework
/// - #ASSUME_MONOTONIC_GENERATION: Concurrent writes maintain monotonic generation
/// - #VERIFY_MONOTONIC_GENERATION: Multi-threaded monotonicity validation
///
/// # Performance
/// - 8 threads × 10,000 writes = 80K total writes
/// - Generation should be exactly 80,000
#[test]
fn test_concurrent_generation_monotonicity() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    const THREADS: usize = 8;
    const WRITES_PER_THREAD: usize = 10_000;

    for _ in 0..THREADS {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..WRITES_PER_THREAD {
                dual_clone.write_with_generation(i as u64);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Generation incremented exactly THREADS * WRITES_PER_THREAD times
    assert_eq!(
        dual.load_secondary_acquire(),
        (THREADS * WRITES_PER_THREAD) as u64,
        "Generation must be monotonic under concurrent writes"
    );
}

/// Q35: Concurrent no torn reads
///
/// # ASSUM Framework
/// - #ASSUME_NO_TORN_READS: Atomic operations prevent torn reads
/// - #VERIFY_NO_TORN_READS: Concurrent read validation
///
/// # Performance
/// - 4 writers × 1000 writes = 4K writes
/// - 4 readers × 1000 reads = 4K reads
#[test]
fn test_concurrent_no_torn_reads() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    // 4 writers updating primary
    for i in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            let base = i * 1000;
            for j in 0..1000 {
                dual_clone.write_with_generation(base + j);
            }
        }));
    }

    // 4 readers checking consistency
    for _ in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for _ in 0..1000 {
                let value = dual_clone.load_primary_acquire();
                // Value should be a valid write (0-3999)
                assert!(value < 4000, "Torn read detected: {}", value);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

/// Q35: Concurrent writer-reader safety
///
/// # ASSUM Framework
/// - #ASSUME_WRITER_READER_SAFETY: Writers and readers don't interfere
/// - #VERIFY_WRITER_READER_SAFETY: Mixed workload validation
///
/// # Performance
/// - 1 writer × 10K writes
/// - 7 readers × 10K reads
#[test]
fn test_concurrent_writer_reader_safety() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    // Single writer
    let dual_writer = Arc::clone(&dual);
    handles.push(thread::spawn(move || {
        for i in 0..10_000 {
            dual_writer.write_with_generation(i);
        }
    }));

    // 7 readers
    for _ in 0..7 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                let _value = dual_clone.load_primary_acquire();
                let _gen = dual_clone.load_secondary_acquire();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final generation should be 10,000
    assert_eq!(dual.load_secondary_acquire(), 10_000);
}

/// Q35: Concurrent multiple writers generation
///
/// # ASSUM Framework
/// - #ASSUME_MULTI_WRITER_GENERATION: Multiple writers maintain generation integrity
/// - #VERIFY_MULTI_WRITER_GENERATION: Concurrent writer validation
///
/// # Performance
/// - 16 threads × 5,000 writes = 80K total writes
#[test]
fn test_concurrent_multiple_writers_generation() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    const THREADS: usize = 16;
    const WRITES_PER_THREAD: usize = 5_000;

    for thread_id in 0..THREADS {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..WRITES_PER_THREAD {
                let value = (thread_id as u64) << 32 | (i as u64);
                dual_clone.write_with_generation(value);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Generation = 80,000
    assert_eq!(
        dual.load_secondary_acquire(),
        (THREADS * WRITES_PER_THREAD) as u64
    );
}

/// Q35: Concurrent read_consistent stress test
///
/// # ASSUM Framework
/// - #ASSUME_CONSISTENT_READ_STRESS: read_consistent() safe under heavy contention
/// - #VERIFY_CONSISTENT_READ_STRESS: Stress test validation
///
/// # Performance
/// - 1 writer × continuous writes
/// - 8 readers × 10K consistent reads
#[test]
fn test_concurrent_read_consistent_stress() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let stop = Arc::new(AtomicBool::new(false));
    let mut handles = vec![];

    // Continuous writer
    let dual_writer = Arc::clone(&dual);
    let stop_writer = Arc::clone(&stop);
    handles.push(thread::spawn(move || {
        let mut counter = 0u64;
        while !stop_writer.load(Ordering::Acquire) {
            dual_writer.write_with_generation(counter);
            counter = counter.wrapping_add(1);
            thread::yield_now();
        }
    }));

    // 8 readers with consistent reads
    for _ in 0..8 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for _ in 0..10_000 {
                // Try consistent read (may fail under contention)
                let _ = dual_clone.read_consistent();
                thread::yield_now();
            }
        }));
    }

    // Wait for readers
    for handle in handles.drain(1..) {
        handle.join().unwrap();
    }

    // Stop writer
    stop.store(true, Ordering::Release);
    handles.into_iter().next().unwrap().join().unwrap();
}

/// Q35: Concurrent write guard stress test
///
/// # ASSUM Framework
/// - #ASSUME_GUARD_STRESS: GenerationWriteGuard safe under heavy contention
/// - #VERIFY_GUARD_STRESS: Guard stress validation
///
/// # Performance
/// - 16 threads × 1,000 guarded writes = 16K total
#[test]
fn test_concurrent_write_guard_stress() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    const THREADS: usize = 16;
    const WRITES_PER_THREAD: usize = 1_000;

    for thread_id in 0..THREADS {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..WRITES_PER_THREAD {
                let _guard = dual_clone.begin_write();
                let value = (thread_id as u64) << 32 | (i as u64);
                dual_clone.store_primary_release(value);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Generation = 16,000
    assert_eq!(
        dual.load_secondary_acquire(),
        (THREADS * WRITES_PER_THREAD) as u64
    );
}

/// Q35: Concurrent mixed API stress test
///
/// # ASSUM Framework
/// - #ASSUME_MIXED_API_STRESS: All APIs safe when mixed under contention
/// - #VERIFY_MIXED_API_STRESS: Mixed workload stress validation
///
/// # Performance
/// - 4 threads × write_with_generation
/// - 4 threads × GenerationWriteGuard
/// - 4 threads × read_consistent
/// - 4 threads × raw load/store
#[test]
fn test_concurrent_mixed_api_stress() {
    let dual = Arc::new(DualAtomicU64::new(0, 0));
    let mut handles = vec![];

    const ITERATIONS: usize = 2_000;

    // 4 threads using write_with_generation
    for _ in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                dual_clone.write_with_generation(i as u64);
            }
        }));
    }

    // 4 threads using GenerationWriteGuard
    for _ in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                let _guard = dual_clone.begin_write();
                dual_clone.store_primary_release(i as u64);
            }
        }));
    }

    // 4 threads using read_consistent
    for _ in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for _ in 0..ITERATIONS {
                let _ = dual_clone.read_consistent();
            }
        }));
    }

    // 4 threads using raw load/store
    for _ in 0..4 {
        let dual_clone = Arc::clone(&dual);
        handles.push(thread::spawn(move || {
            for i in 0..ITERATIONS {
                dual_clone.store_primary_release(i as u64);
                let _ = dual_clone.load_primary_acquire();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Total writes: (4 + 4) threads × 2000 = 16,000
    // (read_consistent and raw load/store don't increment generation)
    assert_eq!(
        dual.load_secondary_acquire(),
        (8 * ITERATIONS) as u64,
        "Generation should reflect 8 writing threads"
    );
}
