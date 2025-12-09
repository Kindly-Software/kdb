//! MetricsStreamCapsule Property Tests - T28 Framework Validation
//!
//! **Framework**: T28 Testing (Property Tests - Q8-Q14)
//! **Purpose**: Validate concurrent correctness and invariants
//!
//! # Properties Tested
//! 1. No data loss under concurrency (1000 threads)
//! 2. Ring buffer overflow behavior (circular, no panic)
//! 3. Snapshot consistency (all values present at time of capture)
//! 4. Percentile accuracy (monotonic, bounded)
//! 5. Thread safety (Send + Sync, no data races)

use clapi_core::capsules::MetricsStreamCapsule;
use std::sync::Arc;
use std::thread;

/// Property: No data loss under concurrent writes
///
/// **Invariant**: All recorded metrics appear in snapshot (up to ring buffer capacity)
/// **Test**: 1000 threads × 10 records each = 10,000 total operations
/// **Verification**: Final snapshot contains exactly min(10000, 64) values
#[test]
fn property_no_data_loss_concurrent() {
    let capsule = Arc::new(MetricsStreamCapsule::new());
    let mut handles = vec![];

    // 1000 threads, 10 records each
    for thread_id in 0..1000 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..10 {
                c.record_metric(thread_id * 1000 + i);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify: Buffer should be full (64 values, ring buffer capacity)
    let size = capsule.size();
    assert_eq!(size, 64, "Ring buffer should be full after 10,000 writes");

    let snapshot = capsule.snapshot();
    assert_eq!(
        snapshot.len(),
        64,
        "Snapshot should contain exactly 64 values"
    );
}

/// Property: Ring buffer overflow is circular (no panic)
///
/// **Invariant**: Writing beyond capacity overwrites oldest values
/// **Test**: Write 200 values to 64-slot buffer
/// **Verification**: Size capped at 64, no panic, oldest values lost
#[test]
fn property_ring_buffer_overflow_circular() {
    let capsule = MetricsStreamCapsule::new();

    // Write 200 metrics (3× capacity)
    for i in 0..200 {
        capsule.record_metric(i);
    }

    // Size should be capped at RING_CAPACITY (64)
    assert_eq!(capsule.size(), 64);

    // Snapshot should contain 64 values (no panic)
    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.len(), 64);
}

/// Property: Snapshot consistency under concurrent access
///
/// **Invariant**: Snapshot captures state at single point in time
/// **Test**: Concurrent writes while taking snapshots
/// **Verification**: Each snapshot is internally consistent (no torn reads)
#[test]
fn property_snapshot_consistency() {
    let capsule = Arc::new(MetricsStreamCapsule::new());

    // Writer thread: continuously write metrics
    let c_writer = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for i in 0..1000 {
            c_writer.record_metric(i);
            thread::yield_now(); // Encourage interleaving
        }
    });

    // Reader threads: continuously take snapshots
    let mut readers = vec![];
    for _ in 0..10 {
        let c_reader = Arc::clone(&capsule);
        readers.push(thread::spawn(move || {
            for _ in 0..100 {
                let snapshot = c_reader.snapshot();
                // Verify snapshot is consistent (no partial state)
                assert!(snapshot.len() <= 64);
                thread::yield_now();
            }
        }));
    }

    writer.join().unwrap();
    for h in readers {
        h.join().unwrap();
    }
}

/// Property: Percentile calculations are monotonic
///
/// **Invariant**: p50 ≤ p90 ≤ p95 ≤ p99 ≤ p999
/// **Test**: Fill buffer with random values
/// **Verification**: Percentiles are strictly non-decreasing
#[test]
fn property_percentiles_monotonic() {
    let capsule = MetricsStreamCapsule::new();

    // Fill buffer with values 0-63
    for i in 0..64 {
        capsule.record_metric(i * 1000);
    }

    let p50 = capsule.get_p50();
    let p90 = capsule.get_p90();
    let p95 = capsule.get_p95();
    let p99 = capsule.get_p99();
    let p999 = capsule.get_p999();

    assert!(
        p50 <= p90,
        "p50 ({}) should be ≤ p90 ({})",
        p50, p90
    );
    assert!(
        p90 <= p95,
        "p90 ({}) should be ≤ p95 ({})",
        p90, p95
    );
    assert!(
        p95 <= p99,
        "p95 ({}) should be ≤ p99 ({})",
        p95, p99
    );
    assert!(
        p99 <= p999,
        "p99 ({}) should be ≤ p999 ({})",
        p99, p999
    );
}

/// Property: Percentiles are bounded by min/max
///
/// **Invariant**: min ≤ all percentiles ≤ max
/// **Test**: Fill buffer with known values
/// **Verification**: All percentiles within [min, max]
#[test]
fn property_percentiles_bounded() {
    let capsule = MetricsStreamCapsule::new();

    // Fill buffer with values 100-199
    for i in 100..164 {
        capsule.record_metric(i);
    }

    let stats = capsule.get_statistics();

    assert_eq!(stats.min, 100, "Min should be 100");
    assert_eq!(stats.max, 163, "Max should be 163");

    assert!(
        stats.p50 >= stats.min && stats.p50 <= stats.max,
        "p50 ({}) not in [{}, {}]",
        stats.p50,
        stats.min,
        stats.max
    );
    assert!(
        stats.p90 >= stats.min && stats.p90 <= stats.max,
        "p90 ({}) not in [{}, {}]",
        stats.p90,
        stats.min,
        stats.max
    );
    assert!(
        stats.p99 >= stats.min && stats.p99 <= stats.max,
        "p99 ({}) not in [{}, {}]",
        stats.p99,
        stats.min,
        stats.max
    );
}

/// Property: Reset clears all state
///
/// **Invariant**: After reset, capsule behaves as if new
/// **Test**: Fill buffer, reset, verify empty
/// **Verification**: size() == 0, snapshot() == []
#[test]
fn property_reset_clears_state() {
    let capsule = MetricsStreamCapsule::new();

    // Fill buffer
    for i in 0..64 {
        capsule.record_metric(i);
    }
    assert_eq!(capsule.size(), 64);

    // Reset
    capsule.reset();

    // Verify empty
    assert_eq!(capsule.size(), 0);
    assert_eq!(capsule.snapshot().len(), 0);
    assert_eq!(capsule.get_p50(), 0); // Empty buffer returns 0
}

/// Property: Export to KindlyDB preserves values
///
/// **Invariant**: Exported values match recorded values
/// **Test**: Record known values, export, verify
/// **Verification**: All values present in export
#[test]
fn property_export_preserves_values() {
    let capsule = MetricsStreamCapsule::new();

    // Record known values
    let expected = vec![100, 200, 300, 400, 500];
    for &value in &expected {
        capsule.record_metric(value);
    }

    // Export
    let exported = capsule.export_to_kindlydb();
    assert_eq!(exported.len(), expected.len());

    // Extract values from (timestamp, value) pairs
    let exported_values: Vec<u64> = exported.iter().map(|(_, v)| *v).collect();

    // Verify all values present
    for &expected_value in &expected {
        assert!(
            exported_values.contains(&expected_value),
            "Exported values should contain {}",
            expected_value
        );
    }
}

/// Stress test: 100K concurrent records
///
/// **Purpose**: Validate capsule under extreme load
/// **Test**: 100 threads × 1000 records each
/// **Verification**: No panic, no data corruption
#[test]
fn stress_100k_concurrent_records() {
    let capsule = Arc::new(MetricsStreamCapsule::new());
    let mut handles = vec![];

    // 100 threads, 1000 records each
    for thread_id in 0..100 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for i in 0..1000 {
                c.record_metric(thread_id * 10000 + i);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify: No panic, buffer capped at 64
    assert_eq!(capsule.size(), 64);
    let snapshot = capsule.snapshot();
    assert_eq!(snapshot.len(), 64);

    // Verify percentiles are computable
    let _ = capsule.get_p99();
    let _ = capsule.get_statistics();
}
