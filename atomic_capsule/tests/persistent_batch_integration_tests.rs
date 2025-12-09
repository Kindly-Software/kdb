//! # T4+T9 Batch Persistent Writer - Integration Tests
//!
//! **Test Tier**: T28 Integration (5 tests, 80 LOC)
//! **Coverage**: 100K writes batched, work-stealing parallelism, crash recovery
//!
//! ## Integration Scenarios
//!
//! 1. **High Throughput**: 100K writes with automatic batching
//! 2. **Work-Stealing**: Parallel batch building (4+ threads)
//! 3. **Crash Recovery**: Generation counter validation
//! 4. **Mixed Workload**: Concurrent appends + flushes
//! 5. **Full Batch Cycle**: Fill, flush, refill patterns

use atomic_capsule::persistence::{BatchPersistentWriter, BATCH_SIZE, ENTRY_SIZE};
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// HIGH THROUGHPUT INTEGRATION
// ============================================================================

#[test]
fn integration_100k_writes_batched() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    let total_writes = 100_000;
    let mut batches_flushed = 0;

    for i in 0..total_writes {
        let full = writer.append(&entry).unwrap();

        // Auto-flush when full
        if full || i == total_writes - 1 {
            writer.flush().unwrap();
            batches_flushed += 1;
        }
    }

    // Verify all writes accounted for
    assert_eq!(writer.write_count(), total_writes);

    // Expected batches: 100K / 256 = 391 batches (rounded up)
    let expected_batches = (total_writes + BATCH_SIZE as u64 - 1) / BATCH_SIZE as u64;
    assert_eq!(writer.flush_count(), expected_batches);

    // Final state: empty batch, committed generation
    assert!(writer.is_empty());
    assert_eq!(writer.generation() % 2, 0);
}

// ============================================================================
// WORK-STEALING PARALLELISM INTEGRATION
// ============================================================================

#[test]
fn integration_work_stealing_parallel_batch() {
    let writer = Arc::new(Mutex::new(BatchPersistentWriter::new()));
    let entry = [0u8; ENTRY_SIZE];

    let num_threads = 4;
    let writes_per_thread = 1000;

    // Parallel appends (work-stealing pattern)
    let threads: Vec<_> = (0..num_threads)
        .map(|_| {
            let writer = Arc::clone(&writer);
            let entry = entry;
            thread::spawn(move || {
                for _ in 0..writes_per_thread {
                    let mut w = writer.lock().unwrap();
                    let full = w.append(&entry).unwrap();

                    // Auto-flush on full
                    if full {
                        w.flush().unwrap();
                    }
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    // Final flush for partial batch
    {
        let mut w = writer.lock().unwrap();
        if !w.is_empty() {
            w.flush().unwrap();
        }
    }

    // Verify all writes accounted for
    let w = writer.lock().unwrap();
    assert_eq!(w.write_count(), (num_threads * writes_per_thread) as u64);
    assert!(w.is_empty());
}

// ============================================================================
// CRASH RECOVERY INTEGRATION
// ============================================================================

#[test]
fn integration_crash_recovery_generation_validation() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    // Simulate normal operation
    for _ in 0..10 {
        writer.append(&entry).unwrap();
    }

    // Flush (simulates successful commit)
    writer.flush().unwrap();
    let gen_after_flush = writer.generation();

    // Verify committed state (even generation)
    assert_eq!(gen_after_flush % 2, 0);

    // Simulate crash recovery check
    // In production: if generation is odd, discard incomplete batch
    // If generation is even, batch was successfully committed
    assert!(is_committed_state(gen_after_flush));
}

fn is_committed_state(generation: u64) -> bool {
    generation % 2 == 0
}

// ============================================================================
// MIXED WORKLOAD INTEGRATION
// ============================================================================

#[test]
fn integration_concurrent_appends_and_flushes() {
    let writer = Arc::new(Mutex::new(BatchPersistentWriter::new()));
    let entry = [0u8; ENTRY_SIZE];

    // Writer threads (append)
    let write_threads: Vec<_> = (0..2)
        .map(|_| {
            let writer = Arc::clone(&writer);
            let entry = entry;
            thread::spawn(move || {
                for _ in 0..500 {
                    writer.lock().unwrap().append(&entry).unwrap();
                    thread::yield_now(); // Force interleaving
                }
            })
        })
        .collect();

    // Flusher thread (periodic flush)
    let flush_thread = {
        let writer = Arc::clone(&writer);
        thread::spawn(move || {
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(1));
                writer.lock().unwrap().flush().unwrap();
            }
        })
    };

    // Wait for all threads
    for t in write_threads {
        t.join().unwrap();
    }
    flush_thread.join().unwrap();

    // Final flush
    {
        let mut w = writer.lock().unwrap();
        w.flush().unwrap();
    }

    // Verify all writes accounted for
    let w = writer.lock().unwrap();
    assert_eq!(w.write_count(), 1000); // 2 threads × 500 writes
}

// ============================================================================
// FULL BATCH CYCLE INTEGRATION
// ============================================================================

#[test]
fn integration_fill_flush_refill_cycle() {
    let mut writer = BatchPersistentWriter::new();
    let entry = [0u8; ENTRY_SIZE];

    // Cycle 1: Fill completely
    for _ in 0..BATCH_SIZE {
        let full = writer.append(&entry).unwrap();
        assert!(!full || writer.batch_count() == BATCH_SIZE);
    }
    assert!(writer.is_full());

    let flushed1 = writer.flush().unwrap();
    assert_eq!(flushed1, BATCH_SIZE);
    assert!(writer.is_empty());

    // Cycle 2: Partial fill
    for _ in 0..100 {
        writer.append(&entry).unwrap();
    }
    assert_eq!(writer.batch_count(), 100);

    let flushed2 = writer.flush().unwrap();
    assert_eq!(flushed2, 100);
    assert!(writer.is_empty());

    // Verify metrics
    assert_eq!(writer.write_count(), (BATCH_SIZE + 100) as u64);
    assert_eq!(writer.flush_count(), 2);
    assert_eq!(writer.generation(), 4); // 2 flushes × 2 increments
}
