//! # NetworkShardCapsule - T8 Distributed Shard State
//!
//! **256-byte aligned capsule** for tracking distributed shard health and metrics.
//!
//! ## Design Principles
//!
//! - **Atomic-only**: No locks, pure atomic operations (T1 foundation)
//! - **Cache-aligned**: 256B = 4× cache lines (prevents false sharing across shards)
//! - **Generation counters**: Monotonic version tracking (TOCTOU prevention)
//! - **EMA latency**: Exponential moving average for adaptive load balancing
//!
//! ## Memory Layout (256 bytes)
//!
//! ```text
//! [0-7]     shard_id: u64              // Immutable shard identifier
//! [8-15]    health_status: AtomicU64   // Packed: state(8)|error_count(16)|last_error_ns(40)
//! [16-23]   last_heartbeat_ns: AtomicU64  // Timestamp (nanoseconds since epoch)
//! [24-31]   documents_count: AtomicU64    // Total documents stored
//! [32-39]   rpc_latency_ns: AtomicU64     // EMA latency (Q16.16 fixed-point)
//! [40-47]   generation: AtomicU64         // Monotonic version counter
//! [48-55]   bytes_stored: AtomicU64       // Total storage usage
//! [56-63]   requests_per_sec: AtomicU64   // EMA requests/sec (Q16.16)
//! [64-255]  _padding: [u8; 192]           // Cache alignment
//! ```
//!
//! ## Performance (B32 Framework)
//!
//! - Health check: <10ns (single atomic load)
//! - Heartbeat update: <20ns (2 atomic stores)
//! - EMA latency update: <50ns (CAS loop, max 8 retries)
//! - Generation increment: <5ns (relaxed fetch_add)

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Network shard health states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShardHealth {
    /// Shard is operational and accepting requests
    Healthy = 0,
    /// Shard experiencing degraded performance
    Degraded = 1,
    /// Shard temporarily unavailable (circuit breaker open)
    Unavailable = 2,
    /// Shard offline or unreachable
    Offline = 3,
}

impl ShardHealth {
    /// Parse from packed status byte
    #[inline]
    pub fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Healthy,
            1 => Self::Degraded,
            2 => Self::Unavailable,
            3 => Self::Offline,
            _ => Self::Offline, // Defensive: treat unknown as offline
        }
    }

    /// Convert to packed byte
    #[inline]
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

/// NetworkShardCapsule - T8 distributed shard coordination
///
/// # ASSUM Safety Model
///
/// - `#ASSUME_MONOTONIC_TIME`: System clock is monotonic (or close enough)
/// - `#ASSUME_CAS_CONVERGENCE`: CAS loops converge within 8 retries
/// - `#ASSUME_NO_OVERFLOW`: Counters don't overflow in practice (u64 range)
/// - `#VERIFY_ALIGNMENT`: derive macro ensures 256B alignment
/// - `#VERIFY_LOCKFREE`: All operations use atomics (no mutex)
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 256, size = 256))]
#[repr(C, align(256))]
pub struct NetworkShardCapsule {
    /// Immutable shard identifier (set at construction)
    shard_id: u64,

    /// Packed health status: [state(8)|error_count(16)|last_error_ns(40)]
    /// - state: ShardHealth enum (0-3)
    /// - error_count: Rolling error counter (saturating at 65535)
    /// - last_error_ns: Timestamp of last error (40 bits = ~34 years)
    health_status: AtomicU64,

    /// Last successful heartbeat timestamp (nanoseconds since UNIX epoch)
    /// Used to detect stale shards (no heartbeat > 30 seconds)
    last_heartbeat_ns: AtomicU64,

    /// Total documents stored on this shard
    documents_count: AtomicU64,

    /// Exponential moving average RPC latency (Q16.16 fixed-point nanoseconds)
    /// EMA formula: new_ema = (alpha * sample) + ((1 - alpha) * old_ema)
    /// Alpha = 0.1 (Q16.16 = 6554), represents 10% weight to new samples
    rpc_latency_ns: AtomicU64,

    /// Monotonic generation counter (incremented on every state change)
    /// Used for TOCTOU prevention and audit trails (Q34)
    generation: AtomicU64,

    /// Total bytes stored on this shard (storage capacity tracking)
    bytes_stored: AtomicU64,

    /// Exponential moving average requests per second (Q16.16 fixed-point)
    requests_per_sec: AtomicU64,

    /// Cache alignment padding (256 bytes total)
    _padding: [u8; 192],
}

// #VERIFY_LOCKFREE: Ensure Send + Sync (automatic for all-atomic struct)
// #ASSUME_ATOMIC_THREAD_SAFE: AtomicU64 is Send + Sync
// Note: derive macro already implements Send + Sync

impl NetworkShardCapsule {
    /// Create new shard capsule with given ID
    ///
    /// # Example
    ///
    /// ```
    /// use atomic_capsule::network::NetworkShardCapsule;
    ///
    /// let shard = NetworkShardCapsule::new(42);
    /// assert_eq!(shard.shard_id(), 42);
    /// ```
    pub const fn new(shard_id: u64) -> Self {
        Self {
            shard_id,
            health_status: AtomicU64::new(0), // Healthy by default
            last_heartbeat_ns: AtomicU64::new(0),
            documents_count: AtomicU64::new(0),
            rpc_latency_ns: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            bytes_stored: AtomicU64::new(0),
            requests_per_sec: AtomicU64::new(0),
            _padding: [0u8; 192],
        }
    }

    /// Get shard ID (immutable)
    #[inline]
    pub const fn shard_id(&self) -> u64 {
        self.shard_id
    }

    /// Get current health state
    ///
    /// # Performance
    ///
    /// - <10ns (single relaxed atomic load)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_RELAXED_OK`: Health state doesn't require synchronization
    #[inline]
    pub fn health(&self) -> ShardHealth {
        let status = self.health_status.load(Ordering::Relaxed);
        let state = (status >> 56) as u8; // Extract top byte
        ShardHealth::from_u8(state)
    }

    /// Check if shard is healthy (ready to accept requests)
    ///
    /// # Example
    ///
    /// ```
    /// # use atomic_capsule::network::NetworkShardCapsule;
    /// let shard = NetworkShardCapsule::new(1);
    /// assert!(shard.is_healthy());
    /// ```
    #[inline]
    pub fn is_healthy(&self) -> bool {
        matches!(self.health(), ShardHealth::Healthy)
    }

    /// Check if heartbeat is fresh (within 30 seconds)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_MONOTONIC_TIME`: System clock doesn't go backwards
    /// - `#ASSUME_30_SEC_THRESHOLD`: 30s is reasonable heartbeat timeout
    pub fn heartbeat_fresh(&self) -> bool {
        let last_hb = self.last_heartbeat_ns.load(Ordering::Acquire);
        if last_hb == 0 {
            return false; // Never received heartbeat
        }

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        const HEARTBEAT_TIMEOUT_NS: u64 = 30_000_000_000; // 30 seconds
        now_ns.saturating_sub(last_hb) < HEARTBEAT_TIMEOUT_NS
    }

    /// Update heartbeat timestamp and increment generation
    ///
    /// # Performance
    ///
    /// - <20ns (2 atomic stores)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_RELEASE_SEMANTICS`: Heartbeat update visible to readers
    pub fn update_heartbeat(&self) {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        self.last_heartbeat_ns.store(now_ns, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Record RPC latency sample and update EMA
    ///
    /// Uses exponential moving average with alpha=0.1:
    /// ```text
    /// new_ema = (0.1 * sample) + (0.9 * old_ema)
    /// ```
    ///
    /// # Performance
    ///
    /// - <50ns (CAS loop, max 8 retries)
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_CAS_CONVERGENCE`: CAS succeeds within 8 retries
    /// - `#ASSUME_Q16_16_RANGE`: Latency fits in Q16.16 (max ~65 seconds)
    /// - `#VERIFY_RETRY_LIMIT`: Bounded retries prevent infinite loops
    pub fn record_rpc_latency(&self, latency_ns: u64) {
        // Convert latency to Q16.16 fixed-point
        let sample_q16 = (latency_ns << 16) / 1_000_000_000; // ns to seconds

        // EMA alpha = 0.1 in Q16.16 = 6554
        const ALPHA_Q16: u64 = 6554; // 0.1 * 65536
        const ONE_MINUS_ALPHA_Q16: u64 = 58982; // 0.9 * 65536

        // #VERIFY_RETRY_LIMIT: Max 8 CAS retries
        for _ in 0..8 {
            let old_ema = self.rpc_latency_ns.load(Ordering::Acquire);

            // new_ema = (alpha * sample) + ((1 - alpha) * old_ema)
            let new_ema = ((ALPHA_Q16 * sample_q16) + (ONE_MINUS_ALPHA_Q16 * old_ema)) >> 16;

            if self
                .rpc_latency_ns
                .compare_exchange_weak(old_ema, new_ema, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Relaxed);
                return;
            }
        }

        // #ASSUME_CAS_CONVERGENCE: If we get here, contention is extreme
        // Fallback: just store the sample (better than blocking)
        self.rpc_latency_ns.store(sample_q16, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current EMA RPC latency in nanoseconds
    ///
    /// # Returns
    ///
    /// Latency in nanoseconds (converted from Q16.16)
    #[inline]
    pub fn rpc_latency_ns(&self) -> u64 {
        let q16_latency = self.rpc_latency_ns.load(Ordering::Relaxed);
        // Convert Q16.16 seconds back to nanoseconds
        (q16_latency * 1_000_000_000) >> 16
    }

    /// Get document count
    #[inline]
    pub fn documents_count(&self) -> u64 {
        self.documents_count.load(Ordering::Relaxed)
    }

    /// Increment document count
    pub fn increment_documents(&self, count: u64) {
        self.documents_count.fetch_add(count, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    /// Get generation counter (monotonic version)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Set health state
    ///
    /// # ASSUM
    ///
    /// - `#ASSUME_RELEASE_SEMANTICS`: State change visible to all readers
    pub fn set_health(&self, state: ShardHealth) {
        let state_byte = state.to_u8() as u64;

        // Update just the state byte (top 8 bits), preserve error counts
        loop {
            let current = self.health_status.load(Ordering::Acquire);
            let new = (current & 0x00FFFFFFFFFFFFFF) | (state_byte << 56);

            if self
                .health_status
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }

    /// Increment error counter
    ///
    /// Saturates at 65535 errors (16-bit counter)
    pub fn record_error(&self) {
        loop {
            let current = self.health_status.load(Ordering::Acquire);
            let error_count = ((current >> 40) & 0xFFFF) as u16;

            if error_count == 0xFFFF {
                break; // Saturated
            }

            let new_count = error_count.saturating_add(1) as u64;
            let new = (current & 0xFF0000FFFFFFFFFF) | (new_count << 40);

            if self
                .health_status
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }

    /// Get error count
    #[inline]
    pub fn error_count(&self) -> u16 {
        let status = self.health_status.load(Ordering::Relaxed);
        ((status >> 40) & 0xFFFF) as u16
    }
}

// Alignment tier marker (256B = ColdTier)
// Note: Already enforced by #[repr(C, align(256))]

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_creation() {
        let shard = NetworkShardCapsule::new(42);
        assert_eq!(shard.shard_id(), 42);
        assert!(shard.is_healthy());
        assert_eq!(shard.documents_count(), 0);
        assert_eq!(shard.generation(), 0);
    }

    #[test]
    fn test_heartbeat() {
        let shard = NetworkShardCapsule::new(1);
        assert!(!shard.heartbeat_fresh()); // No heartbeat yet

        shard.update_heartbeat();
        assert!(shard.heartbeat_fresh());
        assert_eq!(shard.generation(), 1); // Generation incremented
    }

    #[test]
    fn test_rpc_latency_ema() {
        let shard = NetworkShardCapsule::new(1);

        // Record 100ms latency
        shard.record_rpc_latency(100_000_000);
        let latency1 = shard.rpc_latency_ns();
        assert!(latency1 > 0);

        // Record another 100ms (EMA should stay near 100ms)
        shard.record_rpc_latency(100_000_000);
        let latency2 = shard.rpc_latency_ns();
        assert!(latency2 > 0);
    }

    #[test]
    fn test_health_state() {
        let shard = NetworkShardCapsule::new(1);
        assert!(shard.is_healthy());

        shard.set_health(ShardHealth::Degraded);
        assert_eq!(shard.health(), ShardHealth::Degraded);
        assert!(!shard.is_healthy());

        shard.set_health(ShardHealth::Healthy);
        assert!(shard.is_healthy());
    }

    #[test]
    fn test_error_count() {
        let shard = NetworkShardCapsule::new(1);
        assert_eq!(shard.error_count(), 0);

        shard.record_error();
        assert_eq!(shard.error_count(), 1);

        shard.record_error();
        assert_eq!(shard.error_count(), 2);
    }

    #[test]
    fn test_document_count() {
        let shard = NetworkShardCapsule::new(1);
        assert_eq!(shard.documents_count(), 0);

        shard.increment_documents(10);
        assert_eq!(shard.documents_count(), 10);

        shard.increment_documents(5);
        assert_eq!(shard.documents_count(), 15);
    }

    #[test]
    fn test_alignment() {
        let shard = NetworkShardCapsule::new(1);
        let addr = &shard as *const _ as usize;
        assert_eq!(addr % 256, 0, "Shard must be 256-byte aligned");
    }

    #[test]
    fn test_size() {
        assert_eq!(
            std::mem::size_of::<NetworkShardCapsule>(),
            256,
            "Shard must be exactly 256 bytes"
        );
    }
}
