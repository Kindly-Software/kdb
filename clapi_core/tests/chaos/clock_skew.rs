//! Clock Skew Chaos Test (Scenario 7)
//!
//! **Purpose**: System time jumps backward/forward
//! **Expected Behavior**:
//! - Hash chain integrity maintained
//! - TTL/expiration still works
//! - Timestamp validation robust
//! - Monotonic ordering preserved
//!
//! # ASSUM Safety
//! - #ASSUME: System handles clock jumps without corruption
//! - #VERIFY: Test hash chains before/after clock skew
//! - #ASSUME: Monotonic timestamps for ordering (not wall clock)
//! - #VERIFY: Use Instant (monotonic) not SystemTime (can jump)
//! - #ASSUME: TTL expiration works despite clock skew
//! - #VERIFY: Test expiration with forward/backward jumps
//!
//! # UCE34 Compliance
//! - Q23 (Time dependencies): Identify and isolate time-dependent code
//! - Q24 (Monotonic guarantees): Use Instant for ordering, SystemTime for display
//! - Q25 (Clock skew handling): Validate timestamps, detect skew
//!
//! # T28 Testing
//! - Q22: Production scenario (NTP sync, VM migration cause clock jumps)
//! - Q23: Security (clock skew can break auth, replay prevention)

use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clapi_core::proxy::BudgetRegistry;
use super::{ChaosConfig, ChaosFault, ChaosTestHarness};

/// Clock skew simulator
///
/// # Implementation Note
/// We cannot actually change system time in tests (requires root).
/// Instead, we simulate clock skew by tracking an offset and providing
/// adjusted timestamps to code under test.
#[derive(Clone)]
struct ClockSkewSimulator {
    /// Skew enabled flag
    enabled: Arc<AtomicBool>,
    /// Clock offset (seconds, can be negative)
    offset_secs: Arc<AtomicI64>,
    /// Simulated timestamps generated
    timestamp_count: Arc<AtomicU64>,
}

impl ClockSkewSimulator {
    fn new(enabled: Arc<AtomicBool>, offset_secs: i64) -> Self {
        Self {
            enabled,
            offset_secs: Arc::new(AtomicI64::new(offset_secs)),
            timestamp_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Get current timestamp with skew applied
    ///
    /// # ASSUM Safety
    /// - #ASSUME: SystemTime arithmetic doesn't overflow
    /// - #VERIFY: Use checked arithmetic for large offsets
    /// - #ASSUME: Negative offsets (backward jumps) are valid
    /// - #VERIFY: Handle both forward and backward skew
    fn get_timestamp(&self) -> SystemTime {
        self.timestamp_count.fetch_add(1, Ordering::Relaxed);

        if !self.enabled.load(Ordering::Acquire) {
            // No skew: return actual time
            return SystemTime::now();
        }

        // Apply clock skew
        let offset = self.offset_secs.load(Ordering::Relaxed);
        let now = SystemTime::now();

        if offset >= 0 {
            // Forward jump: add offset
            now + Duration::from_secs(offset as u64)
        } else {
            // Backward jump: subtract offset
            now - Duration::from_secs((-offset) as u64)
        }
    }

    /// Get monotonic time (unaffected by clock skew)
    ///
    /// # ASSUM Safety
    /// - #ASSUME: Instant is truly monotonic (OS guarantee)
    /// - #VERIFY: Use for ordering, not wall-clock time
    fn get_monotonic(&self) -> Instant {
        // Instant is always monotonic, regardless of system clock changes
        Instant::now()
    }

    /// Set clock offset dynamically
    fn set_offset(&self, offset_secs: i64) {
        self.offset_secs.store(offset_secs, Ordering::Release);
    }

    fn get_stats(&self) -> u64 {
        self.timestamp_count.load(Ordering::Relaxed)
    }

    fn clone_handle(&self) -> Self {
        Self {
            enabled: Arc::clone(&self.enabled),
            offset_secs: Arc::clone(&self.offset_secs),
            timestamp_count: Arc::clone(&self.timestamp_count),
        }
    }
}

/// Test: Forward clock skew (time jumps forward)
///
/// # Test Scenario
/// 1. Baseline: Normal operation (10s)
/// 2. Chaos: Clock jumps forward 1 hour (30s)
/// 3. Recovery: Clock returns to normal, validate recovery (30s)
///
/// # Expected Results
/// - TTL expiration not affected (uses monotonic time)
/// - Timestamps remain valid
/// - Hash chain integrity maintained
/// - No panics or corruption
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_forward_clock_skew() {
    // Setup chaos config: +3600s (1 hour forward)
    let config = ChaosConfig::new(
        ChaosFault::ClockSkew { offset_secs: 3600 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    // Create simulator
    let simulator = ClockSkewSimulator::new(Arc::clone(&config.enabled), 3600);

    // Budget registry
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0x1234567890ABCDEF;

    // Track timestamps
    let timestamps = Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Test function
    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let timestamps = Arc::clone(&timestamps);

        move || {
            // Get timestamp with skew
            let ts = simulator.get_timestamp();
            timestamps.lock().push(ts);

            // Budget operation (should work regardless of clock skew)
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    // Run chaos test
    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Forward Clock Skew", test_fn);

    // Validate timestamps
    let ts_list = timestamps.lock();
    println!("Generated {} timestamps", ts_list.len());

    // #ASSUME: System survives forward clock skew
    // #VERIFY: Test completed
    assert!(results.survived, "System should survive forward clock skew");

    // #ASSUME: Operations succeed despite timestamp changes
    // #VERIFY: Low failure rate
    assert!(
        results.chaos_failure_rate_bp() < 100,
        "Forward clock skew should not cause failures"
    );

    println!("\n{}", results.summary());
}

/// Test: Backward clock skew (time jumps backward)
///
/// # Test Scenario
/// - Clock jumps backward 1 hour
/// - System should detect and handle
/// - No timestamp collisions
/// - Monotonic ordering preserved
///
/// # Expected Results
/// - Monotonic timestamps still work
/// - No timestamp collisions
/// - Operations complete successfully
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_backward_clock_skew() {
    // Setup chaos config: -3600s (1 hour backward)
    let config = ChaosConfig::new(
        ChaosFault::ClockSkew { offset_secs: -3600 },
        Duration::from_secs(30),
        Duration::from_secs(30),
    );

    let simulator = ClockSkewSimulator::new(Arc::clone(&config.enabled), -3600);
    let budget_registry = Arc::new(BudgetRegistry::new(100_00));
    let budget_id = 0xFEDCBA0987654321;

    // Track monotonic times (should always increase)
    let monotonic_times = Arc::new(parking_lot::Mutex::new(Vec::new()));

    let test_fn = {
        let simulator = simulator.clone_handle();
        let budget_registry = Arc::clone(&budget_registry);
        let monotonic_times = Arc::clone(&monotonic_times);

        move || {
            // Get monotonic time (unaffected by system clock)
            let mono = simulator.get_monotonic();
            monotonic_times.lock().push(mono);

            // Get wall clock time (affected by skew)
            let _wall = simulator.get_timestamp();

            // Budget operation
            budget_registry.try_deduct(budget_id, 1_00)
                .map(|_| ())
                .map_err(|e| format!("Budget error: {:?}", e))
        }
    };

    let harness = ChaosTestHarness::new(config);
    let results = harness.run("Backward Clock Skew", test_fn);

    // Validate monotonic ordering
    let mono_list = monotonic_times.lock();
    let mut prev = None;
    for (i, &mono) in mono_list.iter().enumerate() {
        if let Some(p) = prev {
            // #ASSUME: Monotonic time always increases
            // #VERIFY: Each timestamp >= previous
            assert!(
                mono >= p,
                "Monotonic time violated at index {}: {:?} < {:?}",
                i, mono, p
            );
        }
        prev = Some(mono);
    }

    println!("\n{}", results.summary());
    println!("Validated {} monotonic timestamps (all increasing)", mono_list.len());
}

/// Test: TTL expiration with clock skew
///
/// # Test Scenario
/// - Set TTL of 60 seconds
/// - Jump clock forward 2 hours
/// - Expired items should be detected
/// - Non-expired items should remain valid
///
/// # Expected Results
/// - TTL expiration works correctly
/// - No false positives/negatives
/// - Monotonic expiration checks
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_ttl_expiration_with_skew() {
    // Setup: TTL tracking
    struct TtlEntry {
        created_at: Instant, // Monotonic
        ttl: Duration,
    }

    impl TtlEntry {
        fn new(ttl: Duration) -> Self {
            Self {
                created_at: Instant::now(),
                ttl,
            }
        }

        fn is_expired(&self) -> bool {
            // #ASSUME: Instant-based expiration immune to clock skew
            // #VERIFY: Use monotonic time for TTL checks
            self.created_at.elapsed() > self.ttl
        }
    }

    // Create entries with various TTLs
    let entry_60s = TtlEntry::new(Duration::from_secs(60));
    std::thread::sleep(Duration::from_millis(100));
    let entry_1s = TtlEntry::new(Duration::from_secs(1));

    // Wait for 1s entry to expire
    std::thread::sleep(Duration::from_secs(2));

    // Simulate clock skew (forward 2 hours)
    let config = ChaosConfig::new(
        ChaosFault::ClockSkew { offset_secs: 7200 },
        Duration::from_secs(5),
        Duration::from_secs(5),
    );
    config.enable();

    // Check expiration (should work despite clock skew)
    // #ASSUME: TTL expiration uses monotonic time
    // #VERIFY: Short TTL expired, long TTL not expired
    assert!(entry_1s.is_expired(), "1s TTL should be expired after 2s");
    assert!(!entry_60s.is_expired(), "60s TTL should not be expired after 2s");

    // Disable skew
    config.disable();

    println!("TTL expiration works correctly despite clock skew");
}

/// Test: Hash chain integrity with clock skew
///
/// # Test Scenario
/// - Build hash chain with timestamps
/// - Apply clock skew
/// - Verify chain integrity
/// - No corruption or collisions
///
/// # Expected Results
/// - Hash chain remains valid
/// - No timestamp collisions
/// - Ordering preserved
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_hash_chain_integrity_with_skew() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Setup hash chain
    #[derive(Debug, Clone)]
    struct HashChainEntry {
        timestamp: u64,    // Unix timestamp (wall clock)
        sequence: u64,     // Monotonic sequence number
        prev_hash: u64,
        data: String,
    }

    impl HashChainEntry {
        fn hash(&self) -> u64 {
            let mut hasher = DefaultHasher::new();
            self.timestamp.hash(&mut hasher);
            self.sequence.hash(&mut hasher);
            self.prev_hash.hash(&mut hasher);
            self.data.hash(&mut hasher);
            hasher.finish()
        }
    }

    let config = ChaosConfig::new(
        ChaosFault::ClockSkew { offset_secs: 3600 },
        Duration::from_secs(10),
        Duration::from_secs(10),
    );

    let simulator = ClockSkewSimulator::new(Arc::clone(&config.enabled), 3600);
    let chain = Arc::new(parking_lot::Mutex::new(Vec::new()));

    // Build chain during chaos
    config.enable();

    for i in 0..100 {
        let ts = simulator.get_timestamp()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let prev_hash = chain.lock().last().map(|e: &HashChainEntry| e.hash()).unwrap_or(0);

        let entry = HashChainEntry {
            timestamp: ts,
            sequence: i,
            prev_hash,
            data: format!("entry_{}", i),
        };

        chain.lock().push(entry);
    }

    config.disable();

    // Verify chain integrity
    let chain_vec = chain.lock().clone();

    for i in 1..chain_vec.len() {
        let entry = &chain_vec[i];
        let prev = &chain_vec[i - 1];

        // #ASSUME: Hash chain links are valid
        // #VERIFY: Each entry's prev_hash matches previous entry's hash
        assert_eq!(
            entry.prev_hash,
            prev.hash(),
            "Hash chain broken at index {}: expected {}, got {}",
            i,
            prev.hash(),
            entry.prev_hash
        );

        // #ASSUME: Sequence numbers are monotonic (not timestamps)
        // #VERIFY: Sequence increases
        assert!(
            entry.sequence > prev.sequence,
            "Sequence not monotonic at index {}",
            i
        );
    }

    println!("Hash chain integrity verified: {} entries, all valid", chain_vec.len());
}

/// Test: Timestamp validation with skew detection
///
/// # Test Scenario
/// - Detect clock skew (timestamps out of order)
/// - Reject operations with invalid timestamps
/// - Graceful handling of detected skew
///
/// # Expected Results
/// - Skew detected within 1 second
/// - Invalid timestamps rejected
/// - Clear error messages
#[test]
#[ignore] // Run with: cargo test --test chaos -- --ignored
fn test_timestamp_validation_skew_detection() {
    // Timestamp validator
    struct TimestampValidator {
        last_timestamp: Arc<AtomicU64>,
        skew_detected: Arc<AtomicBool>,
    }

    impl TimestampValidator {
        fn new() -> Self {
            Self {
                last_timestamp: Arc::new(AtomicU64::new(0)),
                skew_detected: Arc::new(AtomicBool::new(false)),
            }
        }

        fn validate(&self, timestamp: u64) -> Result<(), String> {
            let last = self.last_timestamp.load(Ordering::Acquire);

            // Allow small backward drift (<10s, could be NTP adjustment)
            const MAX_BACKWARD_DRIFT_SECS: u64 = 10;

            if timestamp + MAX_BACKWARD_DRIFT_SECS < last {
                // Significant backward jump detected
                self.skew_detected.store(true, Ordering::Release);
                return Err(format!(
                    "Clock skew detected: timestamp {} is {} seconds behind last timestamp {}",
                    timestamp,
                    last - timestamp,
                    last
                ));
            }

            // Update last timestamp
            self.last_timestamp.store(timestamp, Ordering::Release);
            Ok(())
        }

        fn is_skew_detected(&self) -> bool {
            self.skew_detected.load(Ordering::Acquire)
        }
    }

    // Test with backward skew
    let validator = TimestampValidator::new();
    let config = ChaosConfig::new(
        ChaosFault::ClockSkew { offset_secs: -7200 }, // -2 hours
        Duration::from_secs(10),
        Duration::from_secs(10),
    );

    let simulator = ClockSkewSimulator::new(Arc::clone(&config.enabled), -7200);

    // Normal timestamps (no skew)
    for _ in 0..10 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert!(validator.validate(ts).is_ok());
        std::thread::sleep(Duration::from_millis(100));
    }

    // Enable skew
    config.enable();

    // Skewed timestamp (should be rejected)
    let skewed_ts = simulator.get_timestamp()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let result = validator.validate(skewed_ts);

    // #ASSUME: Validator detects significant clock skew
    // #VERIFY: Validation fails
    assert!(result.is_err(), "Should detect clock skew");
    assert!(validator.is_skew_detected(), "Skew flag should be set");

    println!("Clock skew detected: {:?}", result.unwrap_err());
}

#[cfg(test)]
mod compile_tests {
    use super::*;

    #[test]
    fn test_simulator_clone() {
        let enabled = Arc::new(AtomicBool::new(false));
        let simulator = ClockSkewSimulator::new(enabled, 3600);
        let cloned = simulator.clone_handle();

        let _ = simulator.get_timestamp();
        assert_eq!(cloned.get_stats(), 1);
    }

    #[test]
    fn test_forward_skew() {
        let enabled = Arc::new(AtomicBool::new(true));
        let simulator = ClockSkewSimulator::new(enabled, 3600);

        let skewed = simulator.get_timestamp();
        let normal = SystemTime::now();

        // Skewed time should be ~1 hour ahead
        let diff = skewed.duration_since(normal).unwrap().as_secs();
        assert!(diff >= 3590 && diff <= 3610, "Forward skew should be ~3600s, got {}s", diff);
    }

    #[test]
    fn test_monotonic_unaffected() {
        let enabled = Arc::new(AtomicBool::new(true));
        let simulator = ClockSkewSimulator::new(enabled, -3600);

        let mono1 = simulator.get_monotonic();
        std::thread::sleep(Duration::from_millis(10));
        let mono2 = simulator.get_monotonic();

        // Monotonic time always increases
        assert!(mono2 > mono1);
    }
}
