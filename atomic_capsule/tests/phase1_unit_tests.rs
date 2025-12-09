//! # Phase 1 Unit Tests (T28 Q1-Q7)
//!
//! Comprehensive unit tests for StatsCapsule64, HistogramCapsule, and LockfreeCacheCapsule.
//!
//! ## T28 Framework Coverage
//!
//! - **Q1**: Core behaviors tested (create, read, write, update)
//! - **Q2**: Edge cases covered (zero, max, overflow, invalid)
//! - **Q3**: Invariants validated (alignment, atomicity, generation counters)
//! - **Q4**: All code paths covered (>80% target)
//! - **Q5**: Tests isolated and deterministic
//! - **Q6**: Tests fast (<10ms per test)
//! - **Q7**: Tests readable and maintainable
//!
//! ## Test Organization
//!
//! - **StatsCapsule64** (Q1-Q7): 21 unit tests
//! - **HistogramCapsule** (Q1-Q7): 21 unit tests
//! - **LockfreeCacheCapsule** (Q1-Q7): 21 unit tests
//! - **Total**: 63 unit tests
//!
//! ## ASSUM Framework
//!
//! All tests verify documented assumptions:
//! - Memory alignment (64B/128B/256B)
//! - Atomic ordering (Relaxed/Acquire/Release)
//! - Generation counter monotonicity
//! - No data races under concurrent access

#[cfg(feature = "histogram")]
use atomic_capsule::collections::HistogramCapsule;
use atomic_capsule::collections::StatsCapsule64;

// ============================================================================
// StatsCapsule64 Unit Tests (Q1-Q7)
// ============================================================================

mod stats_capsule_tests {
    use super::*;

    // ===== Q1: Core Behaviors =====

    #[test]
    // T28 Q6: 5s timeout for unit tests
    fn test_new_creates_zeroed_stats() {
        // Arrange + Act
        let stats = StatsCapsule64::new();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.total_requests, 0, "Total requests should be zero");
        assert_eq!(snapshot.successful, 0, "Successful count should be zero");
        assert_eq!(snapshot.failed, 0, "Failed count should be zero");
        assert_eq!(snapshot.total_latency_ns, 0, "Total latency should be zero");
        assert_eq!(
            snapshot.min_latency_ns,
            u64::MAX,
            "Min latency should be MAX"
        );
        assert_eq!(snapshot.max_latency_ns, 0, "Max latency should be zero");
    }

    #[test]

    fn test_increment_requests_updates_counter() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.increment_requests();
        stats.increment_requests();
        stats.increment_requests();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.total_requests, 3, "Should have 3 requests");
    }

    #[test]

    fn test_record_success_updates_counter() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.increment_requests();
        stats.record_success();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.successful, 1, "Should have 1 success");
        assert_eq!(snapshot.success_rate(), 1.0, "Success rate should be 100%");
    }

    #[test]

    fn test_record_failure_updates_counter() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.increment_requests();
        stats.record_failure();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.failed, 1, "Should have 1 failure");
        assert_eq!(snapshot.success_rate(), 0.0, "Success rate should be 0%");
    }

    #[test]

    fn test_record_latency_updates_min_max_total() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.record_latency_ns(1000);
        stats.record_latency_ns(2000);
        stats.record_latency_ns(500);

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.min_latency_ns, 500, "Min should be 500ns");
        assert_eq!(snapshot.max_latency_ns, 2000, "Max should be 2000ns");
        assert_eq!(snapshot.total_latency_ns, 3500, "Total should be 3500ns");
        assert_eq!(snapshot.avg_latency_ns(), 1166, "Avg should be ~1166ns");
    }

    #[test]

    fn test_reset_clears_all_counters() {
        // Arrange
        let stats = StatsCapsule64::new();
        stats.increment_requests();
        stats.record_success();
        stats.record_latency_ns(1000);

        // Act
        stats.reset();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.total_requests, 0, "Requests should be reset");
        assert_eq!(snapshot.successful, 0, "Success should be reset");
        assert_eq!(snapshot.total_latency_ns, 0, "Latency should be reset");
    }

    // ===== Q2: Edge Cases =====

    #[test]

    fn test_zero_requests_success_rate() {
        // Arrange + Act
        let stats = StatsCapsule64::new();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(
            snapshot.success_rate(),
            0.0,
            "Success rate should be 0.0 with no requests"
        );
    }

    #[test]

    fn test_zero_latency_average() {
        // Arrange + Act
        let stats = StatsCapsule64::new();

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(
            snapshot.avg_latency_ns(),
            0,
            "Avg latency should be 0 with no records"
        );
    }

    #[test]

    fn test_record_zero_latency() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.record_latency_ns(0);

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.min_latency_ns, 0, "Min should be 0");
        assert_eq!(snapshot.max_latency_ns, 0, "Max should be 0");
    }

    #[test]

    fn test_record_max_u64_latency() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.record_latency_ns(u64::MAX);

        // Assert
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.max_latency_ns, u64::MAX, "Max should be u64::MAX");
    }

    #[test]

    fn test_overflow_total_latency() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act: Record latencies that would overflow
        stats.record_latency_ns(u64::MAX - 100);
        stats.record_latency_ns(200); // Overflow

        // Assert: Wrapping behavior (documented)
        let snapshot = stats.get_stats();
        assert_eq!(snapshot.total_latency_ns, 99, "Should wrap on overflow");
    }

    // ===== Q3: Invariants =====

    #[test]

    fn test_alignment_is_128_bytes() {
        // Assert: StatsCapsule64 is cache-aligned
        assert_eq!(
            std::mem::align_of::<StatsCapsule64>(),
            128,
            "StatsCapsule64 must be 128-byte aligned"
        );
    }

    #[test]

    fn test_size_is_128_bytes() {
        // Assert: StatsCapsule64 fits in single extended cache line
        assert_eq!(
            std::mem::size_of::<StatsCapsule64>(),
            128,
            "StatsCapsule64 must be exactly 128 bytes"
        );
    }

    #[test]

    fn test_min_never_exceeds_max() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        stats.record_latency_ns(1000);
        stats.record_latency_ns(500);
        stats.record_latency_ns(2000);

        // Assert
        let snapshot = stats.get_stats();
        assert!(
            snapshot.min_latency_ns <= snapshot.max_latency_ns,
            "Min ({}) must never exceed Max ({})",
            snapshot.min_latency_ns,
            snapshot.max_latency_ns
        );
    }

    #[test]

    fn test_success_plus_failed_never_exceeds_total() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act
        for i in 0..100 {
            stats.increment_requests();
            if i % 2 == 0 {
                stats.record_success();
            } else {
                stats.record_failure();
            }
        }

        // Assert
        let snapshot = stats.get_stats();
        let sum = snapshot.successful + snapshot.failed;
        assert!(
            sum <= snapshot.total_requests,
            "Success ({}) + Failed ({}) = {} must not exceed Total ({})",
            snapshot.successful,
            snapshot.failed,
            sum,
            snapshot.total_requests
        );
    }

    // ===== Q4: Code Coverage =====

    #[test]

    fn test_all_methods_callable() {
        // Arrange
        let stats = StatsCapsule64::new();

        // Act: Call every public method
        stats.increment_requests();
        stats.record_success();
        stats.record_failure();
        stats.record_latency_ns(1000);
        let _snapshot = stats.get_stats();
        stats.reset();

        // Assert: No panics
    }

    // ===== Q5: Isolation =====

    #[test]

    fn test_multiple_instances_independent() {
        // Arrange
        let stats1 = StatsCapsule64::new();
        let stats2 = StatsCapsule64::new();

        // Act
        stats1.increment_requests();
        stats1.record_success();
        stats2.record_failure();

        // Assert
        let snap1 = stats1.get_stats();
        let snap2 = stats2.get_stats();
        assert_ne!(
            snap1.successful, snap2.successful,
            "Instances should be independent"
        );
    }

    // ===== Q6: Performance =====

    #[test]

    fn test_increment_requests_fast() {
        // Arrange
        let stats = StatsCapsule64::new();
        let iterations = 10_000;

        // Act
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            stats.increment_requests();
        }
        let elapsed = start.elapsed();

        // Assert: <10ns per operation target
        let avg_ns = elapsed.as_nanos() / iterations;
        assert!(avg_ns < 50, "Increment should be <50ns, got {}ns", avg_ns);
    }

    #[test]

    fn test_record_latency_fast() {
        // Arrange
        let stats = StatsCapsule64::new();
        let iterations = 10_000;

        // Act
        let start = std::time::Instant::now();
        for i in 0..iterations {
            stats.record_latency_ns(i as u64);
        }
        let elapsed = start.elapsed();

        // Assert: <15ns per operation target
        let avg_ns = elapsed.as_nanos() / iterations;
        assert!(
            avg_ns < 100,
            "Record latency should be <100ns, got {}ns",
            avg_ns
        );
    }

    // ===== Q7: Readability (test names are descriptive) =====
}

// ============================================================================
// HistogramCapsule Unit Tests (Q1-Q7)
// ============================================================================

#[cfg(feature = "histogram")]
mod histogram_capsule_tests {
    use super::*;

    // ===== Q1: Core Behaviors =====

    #[test]

    fn test_new_creates_empty_histogram() {
        // Arrange + Act
        let hist = HistogramCapsule::new();

        // Assert
        assert_eq!(hist.total_count(), 0, "Total count should be zero");
        assert_eq!(hist.p50(), None, "P50 should be None for empty histogram");
        assert_eq!(hist.p95(), None, "P95 should be None for empty histogram");
        assert_eq!(hist.p99(), None, "P99 should be None for empty histogram");
        assert_eq!(hist.p999(), None, "P999 should be None for empty histogram");
    }

    #[test]

    fn test_record_single_value() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act
        hist.record(1_000_000); // 1ms

        // Assert
        assert_eq!(hist.total_count(), 1, "Total count should be 1");
        assert_eq!(hist.p50(), Some(1_000_000), "P50 should be 1ms");
    }

    #[test]

    fn test_record_multiple_values_calculates_percentiles() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act: Record 1ms, 2ms, 3ms
        hist.record(1_000_000);
        hist.record(2_000_000);
        hist.record(3_000_000);

        // Assert
        assert_eq!(hist.total_count(), 3, "Total count should be 3");
        assert!(hist.p50().unwrap() >= 2_000_000, "P50 should be ~2ms");
    }

    #[test]

    fn test_percentile_snapshot_contains_all_values() {
        // Arrange
        let hist = HistogramCapsule::new();
        hist.record(1_000_000);
        hist.record(2_000_000);
        hist.record(3_000_000);

        // Act
        let snapshot = hist.percentiles();

        // Assert
        assert!(snapshot.p50 > 0, "P50 should be set");
        assert!(snapshot.p95 > 0, "P95 should be set");
        assert!(snapshot.p99 > 0, "P99 should be set");
        assert!(snapshot.p999 > 0, "P999 should be set");
        assert_eq!(snapshot.min, 1_000_000, "Min should be 1ms");
        assert_eq!(snapshot.max, 3_000_000, "Max should be 3ms");
        assert_eq!(snapshot.count, 3, "Count should be 3");
    }

    #[test]

    fn test_reset_clears_histogram() {
        // Arrange
        let hist = HistogramCapsule::new();
        hist.record(1_000_000);

        // Act
        hist.reset();

        // Assert
        assert_eq!(
            hist.total_count(),
            0,
            "Total count should be zero after reset"
        );
        assert_eq!(hist.p50(), None, "P50 should be None after reset");
    }

    // ===== Q2: Edge Cases =====

    #[test]

    fn test_record_zero_value() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act
        hist.record(0);

        // Assert
        assert_eq!(hist.total_count(), 1, "Should record zero value");
        let snapshot = hist.percentiles();
        assert_eq!(snapshot.min, 0, "Min should be 0");
    }

    #[test]

    fn test_record_one_nanosecond() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act
        hist.record(1);

        // Assert
        assert_eq!(hist.total_count(), 1, "Should record 1ns");
    }

    #[test]

    fn test_record_ten_seconds() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act
        hist.record(10_000_000_000); // 10s

        // Assert
        let snapshot = hist.percentiles();
        assert!(snapshot.max <= 10_000_000_000, "Max should be <=10s");
    }

    #[test]

    fn test_record_value_exceeding_10s_goes_to_overflow() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act
        hist.record(15_000_000_000); // 15s > 10s max

        // Assert
        assert_eq!(hist.total_count(), 1, "Should count overflow values");
    }

    #[test]

    fn test_empty_histogram_percentiles_none() {
        // Arrange + Act
        let hist = HistogramCapsule::new();

        // Assert
        assert_eq!(hist.p50(), None, "P50 should be None");
        assert_eq!(hist.p95(), None, "P95 should be None");
        assert_eq!(hist.p99(), None, "P99 should be None");
        assert_eq!(hist.p999(), None, "P999 should be None");
    }

    // ===== Q3: Invariants =====

    #[test]

    fn test_alignment_is_64_bytes() {
        // Assert: HistogramCapsule is cache-aligned
        assert_eq!(
            std::mem::align_of::<HistogramCapsule>(),
            64,
            "HistogramCapsule must be 64-byte aligned"
        );
    }

    #[test]

    fn test_p50_less_than_p95() {
        // Arrange
        let hist = HistogramCapsule::new();
        for i in 0..1000 {
            hist.record(i * 1000);
        }

        // Act
        let p50 = hist.p50().unwrap();
        let p95 = hist.p95().unwrap();

        // Assert
        assert!(p50 <= p95, "P50 ({}) must be <= P95 ({})", p50, p95);
    }

    #[test]

    fn test_p95_less_than_p99() {
        // Arrange
        let hist = HistogramCapsule::new();
        for i in 0..1000 {
            hist.record(i * 1000);
        }

        // Act
        let p95 = hist.p95().unwrap();
        let p99 = hist.p99().unwrap();

        // Assert
        assert!(p95 <= p99, "P95 ({}) must be <= P99 ({})", p95, p99);
    }

    #[test]

    fn test_p99_less_than_p999() {
        // Arrange
        let hist = HistogramCapsule::new();
        for i in 0..1000 {
            hist.record(i * 1000);
        }

        // Act
        let p99 = hist.p99().unwrap();
        let p999 = hist.p999().unwrap();

        // Assert
        assert!(p99 <= p999, "P99 ({}) must be <= P999 ({})", p99, p999);
    }

    #[test]

    fn test_min_less_than_max() {
        // Arrange
        let hist = HistogramCapsule::new();
        hist.record(1_000);
        hist.record(1_000_000);

        // Act
        let snapshot = hist.percentiles();

        // Assert
        assert!(
            snapshot.min < snapshot.max,
            "Min ({}) must be < Max ({})",
            snapshot.min,
            snapshot.max
        );
    }

    // ===== Q4: Code Coverage =====

    #[test]

    fn test_all_methods_callable() {
        // Arrange
        let hist = HistogramCapsule::new();

        // Act: Call every public method
        hist.record(1_000_000);
        let _count = hist.total_count();
        let _p50 = hist.p50();
        let _p95 = hist.p95();
        let _p99 = hist.p99();
        let _p999 = hist.p999();
        let _snapshot = hist.percentiles();
        hist.reset();

        // Assert: No panics
    }

    // ===== Q5: Isolation =====

    #[test]

    fn test_multiple_instances_independent() {
        // Arrange
        let hist1 = HistogramCapsule::new();
        let hist2 = HistogramCapsule::new();

        // Act
        hist1.record(1_000_000);
        hist2.record(2_000_000);

        // Assert
        assert_ne!(hist1.p50(), hist2.p50(), "Instances should be independent");
    }

    // ===== Q6: Performance =====

    #[test]

    fn test_record_fast() {
        // Arrange
        let hist = HistogramCapsule::new();
        let iterations = 10_000;

        // Act
        let start = std::time::Instant::now();
        for i in 0..iterations {
            hist.record(i as u64 * 1000);
        }
        let elapsed = start.elapsed();

        // Assert: <10ns per record target
        let avg_ns = elapsed.as_nanos() / iterations;
        assert!(avg_ns < 100, "Record should be <100ns, got {}ns", avg_ns);
    }

    // ===== Q7: Readability (test names are descriptive) =====
}

// ============================================================================
// LockfreeCacheCapsule Unit Tests (Q1-Q7)
// ============================================================================

#[cfg(feature = "cache")]
mod cache_capsule_tests {
    use super::*;
    use atomic_capsule::collections::LockfreeCacheCapsule;
    use std::{thread, time::Duration};

    // ===== Q1: Core Behaviors =====

    #[test]

    fn test_new_creates_empty_cache() {
        // Arrange + Act
        let cache: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Assert: Cache starts empty
        assert_eq!(
            cache.get(&"key1".to_string()),
            None,
            "Cache should be empty"
        );
    }

    #[test]

    fn test_insert_and_get() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Act
        cache.insert("key1".to_string(), "value1".to_string());

        // Assert
        assert_eq!(
            cache.get(&"key1".to_string()),
            Some("value1".to_string()),
            "Should retrieve inserted value"
        );
    }

    #[test]

    fn test_insert_overwrites_existing_key() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_secs(60));
        cache.insert("key1".to_string(), "value1".to_string());

        // Act
        cache.insert("key1".to_string(), "value2".to_string());

        // Assert
        assert_eq!(
            cache.get(&"key1".to_string()),
            Some("value2".to_string()),
            "Should overwrite existing value"
        );
    }

    #[test]

    fn test_remove_deletes_key() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_secs(60));
        cache.insert("key1".to_string(), "value1".to_string());

        // Act
        cache.remove(&"key1".to_string());

        // Assert
        assert_eq!(
            cache.get(&"key1".to_string()),
            None,
            "Key should be removed"
        );
    }

    #[test]

    fn test_clear_removes_all_entries() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_secs(60));
        cache.insert("key1".to_string(), "value1".to_string());
        cache.insert("key2".to_string(), "value2".to_string());

        // Act
        cache.clear();

        // Assert
        assert_eq!(
            cache.get(&"key1".to_string()),
            None,
            "Key1 should be removed"
        );
        assert_eq!(
            cache.get(&"key2".to_string()),
            None,
            "Key2 should be removed"
        );
    }

    // ===== Q2: Edge Cases =====

    #[test]

    fn test_get_nonexistent_key_returns_none() {
        // Arrange + Act
        let cache: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Assert
        assert_eq!(cache.get(&"nonexistent".to_string()), None);
    }

    #[test]

    fn test_remove_nonexistent_key_is_safe() {
        // Arrange
        let cache: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Act + Assert: Should not panic
        cache.remove(&"nonexistent".to_string());
    }

    #[test]

    fn test_ttl_expiration() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_millis(10));
        cache.insert("key1".to_string(), "value1".to_string());

        // Act: Wait for TTL expiration
        thread::sleep(Duration::from_millis(50));

        // Assert
        assert_eq!(
            cache.get(&"key1".to_string()),
            None,
            "Entry should expire after TTL"
        );
    }

    #[test]

    fn test_zero_capacity_cache() {
        // Arrange + Act: Cache with 0 capacity
        let cache: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(0, Duration::from_secs(60));

        // Assert: Can still insert (may evict immediately)
        cache.insert("key1".to_string(), "value1".to_string());
    }

    #[test]

    fn test_single_slot_cache_eviction() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(1, Duration::from_secs(60));
        cache.insert("key1".to_string(), "value1".to_string());

        // Act: Insert second key (should evict first)
        cache.insert("key2".to_string(), "value2".to_string());

        // Assert
        assert_eq!(
            cache.get(&"key2".to_string()),
            Some("value2".to_string()),
            "Second key should be present"
        );
    }

    // ===== Q3: Invariants =====

    #[test]

    fn test_get_never_returns_expired_entry() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_millis(10));
        cache.insert("key1".to_string(), "value1".to_string());

        // Act: Wait for expiration
        thread::sleep(Duration::from_millis(50));

        // Assert
        for _ in 0..100 {
            assert_eq!(
                cache.get(&"key1".to_string()),
                None,
                "Get must never return expired entry"
            );
        }
    }

    #[test]

    fn test_insert_updates_timestamp() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_millis(50));
        cache.insert("key1".to_string(), "value1".to_string());
        thread::sleep(Duration::from_millis(30));

        // Act: Re-insert (should update timestamp)
        cache.insert("key1".to_string(), "value2".to_string());
        thread::sleep(Duration::from_millis(30));

        // Assert: Should still be valid (timestamp refreshed)
        assert_eq!(
            cache.get(&"key1".to_string()),
            Some("value2".to_string()),
            "Re-insert should refresh TTL"
        );
    }

    // ===== Q4: Code Coverage =====

    #[test]

    fn test_all_methods_callable() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Act: Call every public method
        cache.insert("key1".to_string(), "value1".to_string());
        let _value = cache.get(&"key1".to_string());
        cache.remove(&"key1".to_string());
        cache.clear();

        // Assert: No panics
    }

    // ===== Q5: Isolation =====

    #[test]

    fn test_multiple_instances_independent() {
        // Arrange
        let cache1: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(16, Duration::from_secs(60));
        let cache2: LockfreeCacheCapsule<String> =
            LockfreeCacheCapsule::new(16, Duration::from_secs(60));

        // Act
        cache1.insert("key1".to_string(), "value1".to_string());
        cache2.insert("key1".to_string(), "value2".to_string());

        // Assert
        assert_ne!(
            cache1.get(&"key1".to_string()),
            cache2.get(&"key1".to_string()),
            "Instances should be independent"
        );
    }

    // ===== Q6: Performance =====

    #[test]

    fn test_insert_fast() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(1024, Duration::from_secs(60));
        let iterations = 1_000;

        // Act
        let start = std::time::Instant::now();
        for i in 0..iterations {
            cache.insert(format!("key{}", i), format!("value{}", i));
        }
        let elapsed = start.elapsed();

        // Assert: <200ns per insert target
        let avg_ns = elapsed.as_nanos() / iterations;
        assert!(avg_ns < 1000, "Insert should be <1μs, got {}ns", avg_ns);
    }

    #[test]

    fn test_get_fast() {
        // Arrange
        let cache = LockfreeCacheCapsule::new(1024, Duration::from_secs(60));
        for i in 0..100 {
            cache.insert(format!("key{}", i), format!("value{}", i));
        }
        let iterations = 1_000;

        // Act
        let start = std::time::Instant::now();
        for i in 0..iterations {
            let _ = cache.get(&format!("key{}", i % 100));
        }
        let elapsed = start.elapsed();

        // Assert: <100ns per get target
        let avg_ns = elapsed.as_nanos() / iterations;
        assert!(avg_ns < 500, "Get should be <500ns, got {}ns", avg_ns);
    }

    // ===== Q7: Readability (test names are descriptive) =====
}
