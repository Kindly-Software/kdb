// TIER 1: UNIT TESTS - Shard Capsule
// T28 Testing Framework - Individual Component Testing
//
// Tests: Create NetworkShardCapsule, is_healthy(), heartbeat_fresh(), update_heartbeat(), record_rpc_latency()

#![allow(dead_code)]

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Network Shard Capsule (256B aligned)
///
/// # T8 Network Tier
/// - Distributed shard metadata
/// - Health monitoring
/// - Performance metrics
#[repr(C, align(256))]
pub struct NetworkShardCapsule {
    // Shard identity
    pub shard_id: u16,
    pub replica_id: u8,

    // Network location
    pub server_ipv4: u32,
    pub server_port: u16,

    // Health monitoring (T1 atomic)
    pub health_status: AtomicU8, // 0=healthy, 1=degraded, 2=failed
    pub last_heartbeat_ns: AtomicU64,
    pub documents_count: AtomicU64,

    // Performance metrics (T1 atomic, EMA)
    pub rpc_latency_ns: AtomicU64, // P50 latency
    pub rpc_errors_total: AtomicU64,
    pub load_percentage: AtomicU8,

    // Coordination
    pub generation: AtomicU64,

    _padding: [u8; 168],
}

impl NetworkShardCapsule {
    /// Create new shard capsule
    ///
    /// # T28 Unit Test Support
    /// - Initializes all fields to zero
    /// - Sets healthy status
    pub fn new(shard_id: u16, server_ipv4: u32, server_port: u16) -> Self {
        Self {
            shard_id,
            replica_id: 0,
            server_ipv4,
            server_port,
            health_status: AtomicU8::new(0), // Healthy
            last_heartbeat_ns: AtomicU64::new(current_timestamp_ns()),
            documents_count: AtomicU64::new(0),
            rpc_latency_ns: AtomicU64::new(0),
            rpc_errors_total: AtomicU64::new(0),
            load_percentage: AtomicU8::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 168],
        }
    }

    /// Check if shard is healthy (atomic read)
    ///
    /// # T28 Unit Test Support
    /// - Returns true if status == 0 (healthy)
    /// - Returns false if status == 1 (degraded) or 2 (failed)
    #[inline(always)]
    pub fn is_healthy(&self) -> bool {
        let status = self.health_status.load(Ordering::Acquire);
        status == 0
    }

    /// Check if heartbeat is recent (atomic read)
    ///
    /// # T28 Unit Test Support
    /// - Compares last_heartbeat_ns to current time
    /// - Returns true if within timeout
    pub fn heartbeat_fresh(&self, timeout_ns: u64) -> bool {
        let last_seen = self.last_heartbeat_ns.load(Ordering::Acquire);
        let now = current_timestamp_ns();
        (now - last_seen) < timeout_ns
    }

    /// Update heartbeat (called by shard server)
    ///
    /// # T28 Unit Test Support
    /// - Sets last_heartbeat_ns to current time
    /// - Sets health_status to 0 (healthy)
    /// - Increments generation counter
    pub fn update_heartbeat(&self) {
        self.last_heartbeat_ns
            .store(current_timestamp_ns(), Ordering::Release);
        self.health_status.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Record RPC latency (EMA with atomic CAS)
    ///
    /// # T28 Unit Test Support
    /// - Updates exponential moving average
    /// - Alpha = 0.1 (10% new value, 90% old value)
    pub fn record_rpc_latency(&self, latency_ns: u64) {
        const ALPHA_Q16: u64 = 6554; // 0.1 in Q16.16 fixed-point

        let mut retries = 0;
        while retries < 8 {
            let old_ema = self.rpc_latency_ns.load(Ordering::Relaxed);

            // EMA formula: new_ema = alpha * latency + (1 - alpha) * old_ema
            let new_ema = ((ALPHA_Q16 * latency_ns) + ((65536 - ALPHA_Q16) * old_ema)) / 65536;

            if self
                .rpc_latency_ns
                .compare_exchange_weak(old_ema, new_ema, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return;
            }

            retries += 1;
        }
        // Give up after 8 retries (acceptable for approximate EMA)
    }

    /// Mark shard as failed
    pub fn mark_failed(&self) {
        self.health_status.store(2, Ordering::Release);
    }

    /// Get current generation
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get current load percentage
    pub fn load(&self) -> u8 {
        self.load_percentage.load(Ordering::Acquire)
    }

    /// Set load percentage
    pub fn set_load(&self, load: u8) {
        assert!(load <= 100, "Load must be 0-100");
        self.load_percentage.store(load, Ordering::Release);
    }
}

/// Get current timestamp in nanoseconds
fn current_timestamp_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

// ============================================================================
// TIER 1: UNIT TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // ------------------------------------------------------------------------
    // Test Group 1: Create NetworkShardCapsule (aligned 256B)
    // ------------------------------------------------------------------------

    #[test]
    fn test_create_shard_capsule() {
        let capsule = NetworkShardCapsule::new(42, 0x7F000001, 8000);

        assert_eq!(capsule.shard_id, 42);
        assert_eq!(capsule.server_ipv4, 0x7F000001); // 127.0.0.1
        assert_eq!(capsule.server_port, 8000);
    }

    #[test]
    fn test_shard_capsule_alignment() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Check 256B alignment
        let ptr = &capsule as *const _ as usize;
        assert_eq!(ptr % 256, 0, "Capsule must be 256B aligned");
    }

    #[test]
    fn test_shard_capsule_size() {
        let size = std::mem::size_of::<NetworkShardCapsule>();
        assert_eq!(size, 256, "Capsule must be exactly 256 bytes");
    }

    #[test]
    fn test_shard_capsule_initial_state() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Should start healthy
        assert!(capsule.is_healthy());
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.load(), 0);
    }

    // ------------------------------------------------------------------------
    // Test Group 2: is_healthy() checks status
    // ------------------------------------------------------------------------

    #[test]
    fn test_is_healthy_initial() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Should start healthy (status = 0)
        assert!(capsule.is_healthy());
    }

    #[test]
    fn test_is_healthy_after_degraded() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Mark degraded
        capsule.health_status.store(1, Ordering::Release);

        assert!(!capsule.is_healthy());
    }

    #[test]
    fn test_is_healthy_after_failed() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Mark failed
        capsule.mark_failed();

        assert!(!capsule.is_healthy());
    }

    #[test]
    fn test_is_healthy_after_recovery() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Mark failed
        capsule.mark_failed();
        assert!(!capsule.is_healthy());

        // Recover via heartbeat
        capsule.update_heartbeat();
        assert!(capsule.is_healthy());
    }

    // ------------------------------------------------------------------------
    // Test Group 3: heartbeat_fresh() checks timeout
    // ------------------------------------------------------------------------

    #[test]
    fn test_heartbeat_fresh_immediate() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Should be fresh immediately
        let timeout = 10_000_000_000; // 10 seconds
        assert!(capsule.heartbeat_fresh(timeout));
    }

    #[test]
    fn test_heartbeat_fresh_after_delay() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Wait 50ms
        thread::sleep(Duration::from_millis(50));

        // Should still be fresh (within 10 second timeout)
        let timeout = 10_000_000_000; // 10 seconds
        assert!(capsule.heartbeat_fresh(timeout));
    }

    #[test]
    fn test_heartbeat_stale_after_timeout() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Set last heartbeat to 100ms ago
        let old_time = current_timestamp_ns() - 100_000_000; // 100ms ago
        capsule.last_heartbeat_ns.store(old_time, Ordering::Release);

        // Check with 50ms timeout (should be stale)
        let timeout = 50_000_000; // 50ms
        assert!(!capsule.heartbeat_fresh(timeout));
    }

    #[test]
    fn test_heartbeat_fresh_exactly_at_timeout() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Set last heartbeat to exactly timeout ago
        let timeout = 100_000_000; // 100ms
        let old_time = current_timestamp_ns() - timeout;
        capsule.last_heartbeat_ns.store(old_time, Ordering::Release);

        // Should be stale (>= timeout)
        // Note: Might be flaky due to timing, use > instead of >=
        thread::sleep(Duration::from_millis(1));
        assert!(!capsule.heartbeat_fresh(timeout));
    }

    // ------------------------------------------------------------------------
    // Test Group 4: update_heartbeat() increments counter
    // ------------------------------------------------------------------------

    #[test]
    fn test_update_heartbeat_sets_timestamp() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        let before = capsule.last_heartbeat_ns.load(Ordering::Acquire);

        thread::sleep(Duration::from_millis(10));

        capsule.update_heartbeat();

        let after = capsule.last_heartbeat_ns.load(Ordering::Acquire);

        assert!(after > before, "Heartbeat timestamp should increase");
    }

    #[test]
    fn test_update_heartbeat_sets_healthy() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Mark failed
        capsule.mark_failed();
        assert!(!capsule.is_healthy());

        // Update heartbeat
        capsule.update_heartbeat();

        // Should be healthy now
        assert!(capsule.is_healthy());
    }

    #[test]
    fn test_update_heartbeat_increments_generation() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        let gen_before = capsule.generation();

        capsule.update_heartbeat();

        let gen_after = capsule.generation();

        assert_eq!(gen_after, gen_before + 1);
    }

    #[test]
    fn test_update_heartbeat_multiple_times() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        for i in 0..10 {
            capsule.update_heartbeat();
            assert_eq!(capsule.generation(), i + 1);
        }
    }

    // ------------------------------------------------------------------------
    // Test Group 5: record_rpc_latency() EMA calculation
    // ------------------------------------------------------------------------

    #[test]
    fn test_record_rpc_latency_initial() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        capsule.record_rpc_latency(1000);

        let ema = capsule.rpc_latency_ns.load(Ordering::Acquire);

        // First value: EMA = 0.1 * 1000 + 0.9 * 0 = 100
        // (Using Q16.16 fixed-point: 6554 * 1000 / 65536 ≈ 100)
        assert!(
            ema > 90 && ema < 110,
            "Initial EMA should be ~100, got {}",
            ema
        );
    }

    #[test]
    fn test_record_rpc_latency_ema_update() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Record 1000ns
        capsule.record_rpc_latency(1000);
        let ema1 = capsule.rpc_latency_ns.load(Ordering::Acquire);

        // Record 2000ns
        capsule.record_rpc_latency(2000);
        let ema2 = capsule.rpc_latency_ns.load(Ordering::Acquire);

        // EMA should increase (closer to 2000)
        assert!(ema2 > ema1, "EMA should increase: {} -> {}", ema1, ema2);
    }

    #[test]
    fn test_record_rpc_latency_ema_convergence() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        // Record 1000ns many times
        for _ in 0..100 {
            capsule.record_rpc_latency(1000);
        }

        let ema = capsule.rpc_latency_ns.load(Ordering::Acquire);

        // Should converge to ~1000
        assert!(
            ema > 900 && ema < 1100,
            "EMA should converge to ~1000, got {}",
            ema
        );
    }

    #[test]
    fn test_record_rpc_latency_concurrent() {
        use std::sync::Arc;

        let capsule = Arc::new(NetworkShardCapsule::new(0, 0, 0));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let c = Arc::clone(&capsule);
                thread::spawn(move || {
                    for _ in 0..100 {
                        c.record_rpc_latency(1000);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // EMA should be updated (no crashes)
        let ema = capsule.rpc_latency_ns.load(Ordering::Acquire);
        assert!(ema > 0, "EMA should be non-zero");
    }

    // ------------------------------------------------------------------------
    // Test Group 6: Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_load_percentage_boundary() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        capsule.set_load(0);
        assert_eq!(capsule.load(), 0);

        capsule.set_load(100);
        assert_eq!(capsule.load(), 100);
    }

    #[test]
    #[should_panic(expected = "Load must be 0-100")]
    fn test_load_percentage_overflow() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);
        capsule.set_load(101); // Should panic
    }

    #[test]
    fn test_generation_monotonic() {
        let capsule = NetworkShardCapsule::new(0, 0, 0);

        let mut last_gen = capsule.generation();

        for _ in 0..100 {
            capsule.update_heartbeat();
            let current_gen = capsule.generation();

            assert!(current_gen > last_gen, "Generation must be monotonic");
            last_gen = current_gen;
        }
    }
}
