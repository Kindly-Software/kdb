//! ProviderCircuitArray - Per-provider circuit breaker tracking (v0.3.0)
//!
//! Tier 1 (Atomic) + Tier 4 (Batch) - Independent failure tracking for up to 16 providers
//!
//! Solves the fundamental problem: Global circuit breaker affects ALL providers.
//! If Provider A fails 10%, Provider B (healthy) gets blocked incorrectly.
//!
//! # Architecture
//! - ProviderCircuitStatus (64B, T1 Atomic): Per-provider state tracking
//! - ProviderCircuitArray (1KB, T4 Batch): 16 independent circuit breakers
//! - Lockfree slot allocation via CAS (O(16) bounded search)
//! - Independent failure tracking: Provider A failure doesn't affect Provider B
//!
//! # Performance
//! - Status check: <100ns (single atomic load)
//! - Record success/failure: <100ns (atomic CAS)
//! - Slot allocation: <200ns (bounded O(16) search)
//!
//! # Memory Layout
//! ```text
//! ProviderCircuitStatus (64B):
//! [0-7]     provider_id: AtomicU64              // Provider identifier (0 = empty)
//! [8-15]    state_packed: AtomicU64             // failures(20)|successes(20)|state(2)|gen(22)
//! [16-23]   failure_rate_bp: AtomicU64          // Basis points (0-10000)
//! [24-31]   last_state_change_ns: AtomicU64     // Timestamp of last state change
//! [32-39]   cooldown_remaining_ns: AtomicI64    // Cooldown timer (negative = active)
//! [40-63]   _padding: [u8; 24]                  // Cache alignment
//!
//! ProviderCircuitArray (1KB):
//! [0-1023]  circuits: [ProviderCircuitStatus; 16]  // 64B × 16 = 1024B
//! ```
//!
//! # Safety
//! - #ASSUME: CAS ensures single initialization per slot
//! - #VERIFY: Property test validates no slot collisions (1000 threads, 16 providers)
//! - #ASSUME: Lockfree search finds circuit or reports full
//! - #VERIFY: Integration test validates failover semantics
//! - #ASSUME: Failure rate calculation uses saturating arithmetic
//! - #VERIFY: Unit tests validate basis point calculation (0-10000 range)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Circuit breaker state (2-bit encoded)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CircuitState {
    /// Circuit closed - provider healthy (<5% failure rate)
    Closed = 0,
    /// Circuit half-open - monitoring recovery (5-10% failure rate)
    HalfOpen = 1,
    /// Circuit open - provider failing (>10% failure rate)
    Open = 2,
}

impl CircuitState {
    /// Decode from 2-bit value
    fn from_bits(bits: u64) -> Self {
        match bits & 0b11 {
            0 => Self::Closed,
            1 => Self::HalfOpen,
            _ => Self::Open,
        }
    }

    /// Encode to 2-bit value
    fn to_bits(self) -> u64 {
        self as u64
    }
}

/// Circuit breaker thresholds
pub mod thresholds {
    /// Basis points threshold for opening circuit (10%)
    pub const OPEN_THRESHOLD_BP: u64 = 1000; // 10.00%

    /// Basis points threshold for half-open state (5%)
    pub const HALF_OPEN_THRESHOLD_BP: u64 = 500; // 5.00%

    /// Cooldown period before attempting recovery (60 seconds)
    pub const COOLDOWN_NS: i64 = 60_000_000_000; // 60s in nanoseconds

    /// Minimum samples before circuit can trip
    pub const MIN_SAMPLES: u64 = 10;
}

/// Per-provider circuit breaker status (64B, Tier 1 Atomic)
///
/// Independent failure tracking for a single provider.
/// Supports up to 1,048,576 (2^20) successes and failures before overflow.
///
/// # Safety
/// - #ASSUME: AtomicU64 operations are lockfree on 64-bit platforms
/// - #VERIFY: Property test validates state consistency across threads
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct ProviderCircuitStatus {
    /// Provider identifier (0 = empty slot)
    provider_id: AtomicU64,

    /// Packed state: failures(20)|successes(20)|state(2)|generation(22)
    state_packed: AtomicU64,

    /// Failure rate in basis points (0-10000)
    failure_rate_bp: AtomicU64,

    /// Timestamp of last state change (nanoseconds)
    last_state_change_ns: AtomicU64,

    /// Cooldown timer (negative = cooldown active)
    cooldown_remaining_ns: AtomicI64,

    /// Cache-line padding to 64 bytes
    _padding: [u8; 24],
}

impl Default for ProviderCircuitStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderCircuitStatus {
    /// Create new empty circuit status
    pub const fn new() -> Self {
        Self {
            provider_id: AtomicU64::new(0),
            state_packed: AtomicU64::new(0),
            failure_rate_bp: AtomicU64::new(0),
            last_state_change_ns: AtomicU64::new(0),
            cooldown_remaining_ns: AtomicI64::new(0),
            _padding: [0u8; 24],
        }
    }

    /// Initialize circuit for a specific provider
    ///
    /// # Safety
    /// - #ASSUME: Called only once per slot via CAS
    /// - #VERIFY: Caller ensures provider_id != 0
    pub fn init(&self, provider_id: u64, now_ns: u64) {
        debug_assert!(provider_id != 0, "Provider ID cannot be zero");

        // Initialize state: 0 failures, 0 successes, Closed, generation 1
        let initial_state = Self::pack_state(0, 0, CircuitState::Closed, 1);
        self.state_packed.store(initial_state, Ordering::Relaxed);
        self.failure_rate_bp.store(0, Ordering::Relaxed);
        self.last_state_change_ns.store(now_ns, Ordering::Relaxed);
        self.cooldown_remaining_ns.store(0, Ordering::Relaxed);

        // Set provider_id last (marks slot as initialized)
        self.provider_id.store(provider_id, Ordering::Release);
    }

    /// Record a successful request
    ///
    /// Updates success counter and recalculates failure rate.
    /// May close circuit if failure rate drops below threshold.
    ///
    /// # Performance
    /// - Target: <100ns
    /// - Atomic operations: 1 CAS loop (typically 1 iteration)
    pub fn record_success(&self, now_ns: u64) {
        loop {
            let current = self.state_packed.load(Ordering::Acquire);
            let (failures, successes, state, gen) = Self::unpack_state(current);

            // Saturating increment (max 2^20 - 1)
            let new_successes = successes.saturating_add(1).min(0xFFFFF);

            // Recalculate failure rate
            let total = (failures + new_successes) as u64;
            let new_rate_bp = if total > 0 {
                ((failures as u64 * 10000) / total).min(10000)
            } else {
                0
            };

            // Determine new state based on failure rate
            let new_state = if new_rate_bp < thresholds::HALF_OPEN_THRESHOLD_BP {
                CircuitState::Closed
            } else if new_rate_bp < thresholds::OPEN_THRESHOLD_BP {
                CircuitState::HalfOpen
            } else {
                CircuitState::Open
            };

            let new_packed = Self::pack_state(failures, new_successes, new_state, gen + 1);

            if self
                .state_packed
                .compare_exchange_weak(
                    current,
                    new_packed,
                    Ordering::Release,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Update failure rate and timestamp
                self.failure_rate_bp.store(new_rate_bp, Ordering::Relaxed);

                if new_state != state {
                    self.last_state_change_ns.store(now_ns, Ordering::Relaxed);
                }

                break;
            }
        }
    }

    /// Record a failed request
    ///
    /// Updates failure counter and recalculates failure rate.
    /// May open circuit if failure rate exceeds threshold.
    ///
    /// # Performance
    /// - Target: <100ns
    /// - Atomic operations: 1 CAS loop (typically 1 iteration)
    pub fn record_failure(&self, now_ns: u64) {
        loop {
            let current = self.state_packed.load(Ordering::Acquire);
            let (failures, successes, state, gen) = Self::unpack_state(current);

            // Saturating increment (max 2^20 - 1)
            let new_failures = failures.saturating_add(1).min(0xFFFFF);

            // Recalculate failure rate
            let total = (new_failures + successes) as u64;
            let new_rate_bp = if total > 0 {
                ((new_failures as u64 * 10000) / total).min(10000)
            } else {
                0
            };

            // Determine new state based on failure rate
            let new_state = if total < thresholds::MIN_SAMPLES {
                CircuitState::Closed // Not enough samples
            } else if new_rate_bp >= thresholds::OPEN_THRESHOLD_BP {
                CircuitState::Open
            } else if new_rate_bp >= thresholds::HALF_OPEN_THRESHOLD_BP {
                CircuitState::HalfOpen
            } else {
                CircuitState::Closed
            };

            let new_packed = Self::pack_state(new_failures, successes, new_state, gen + 1);

            if self
                .state_packed
                .compare_exchange_weak(
                    current,
                    new_packed,
                    Ordering::Release,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                // Update failure rate and timestamp
                self.failure_rate_bp.store(new_rate_bp, Ordering::Relaxed);

                if new_state != state && new_state == CircuitState::Open {
                    // Circuit opened, set cooldown
                    self.last_state_change_ns.store(now_ns, Ordering::Relaxed);
                    self.cooldown_remaining_ns
                        .store(-thresholds::COOLDOWN_NS, Ordering::Relaxed);
                }

                break;
            }
        }
    }

    /// Check if circuit is open (provider should be blocked)
    ///
    /// # Performance
    /// - Target: <50ns (single atomic load)
    #[inline]
    pub fn is_open(&self) -> bool {
        let packed = self.state_packed.load(Ordering::Relaxed);
        let (_, _, state, _) = Self::unpack_state(packed);
        state == CircuitState::Open
    }

    /// Get current circuit state
    #[inline]
    pub fn state(&self) -> CircuitState {
        let packed = self.state_packed.load(Ordering::Relaxed);
        let (_, _, state, _) = Self::unpack_state(packed);
        state
    }

    /// Get current failure rate in basis points (0-10000)
    #[inline]
    pub fn failure_rate_bp(&self) -> u64 {
        self.failure_rate_bp.load(Ordering::Relaxed)
    }

    /// Get provider ID (0 = empty slot)
    #[inline]
    pub fn provider_id(&self) -> u64 {
        self.provider_id.load(Ordering::Relaxed)
    }

    /// Get success and failure counts (for testing/diagnostics)
    #[inline]
    pub fn get_counts(&self) -> (u32, u32) {
        let packed = self.state_packed.load(Ordering::Relaxed);
        let (failures, successes, _, _) = Self::unpack_state(packed);
        (failures, successes)
    }

    /// Pack state into 64-bit word
    /// Format: failures(20) | successes(20) | state(2) | generation(22)
    fn pack_state(failures: u32, successes: u32, state: CircuitState, generation: u32) -> u64 {
        let f = ((failures as u64) & 0xFFFFF) << 44;
        let s = ((successes as u64) & 0xFFFFF) << 24;
        let st = (state.to_bits() & 0b11) << 22;
        let g = (generation as u64) & 0x3FFFFF;

        f | s | st | g
    }

    /// Unpack 64-bit word into (failures, successes, state, generation)
    fn unpack_state(packed: u64) -> (u32, u32, CircuitState, u32) {
        let failures = ((packed >> 44) & 0xFFFFF) as u32;
        let successes = ((packed >> 24) & 0xFFFFF) as u32;
        let state = CircuitState::from_bits((packed >> 22) & 0b11);
        let generation = (packed & 0x3FFFFF) as u32;

        (failures, successes, state, generation)
    }
}

/// Array of 16 per-provider circuit breakers (1KB, Tier 1 + Tier 4)
///
/// Lockfree allocation and independent tracking for up to 16 providers.
/// Supports concurrent access from multiple threads without locks.
///
/// # Safety
/// - #ASSUME: CAS on provider_id ensures single initialization per slot
/// - #VERIFY: Property test validates no slot collisions (1000 threads)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 1024)]
#[repr(C, align(64))]
pub struct ProviderCircuitArray {
    /// 16 independent circuit breakers (64B each = 1024B total)
    circuits: [ProviderCircuitStatus; 16],
}

impl ProviderCircuitArray {
    /// Create new empty circuit array
    pub const fn new() -> Self {
        // Array initialization with const fn (no Copy required in const context)
        Self {
            circuits: [
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
                ProviderCircuitStatus::new(),
            ],
        }
    }

    /// Find or initialize circuit for a specific provider
    ///
    /// Performs lockfree bounded search (O(16)) to find existing circuit
    /// or allocate a new slot via CAS.
    ///
    /// Returns None if all 16 slots are full.
    ///
    /// # Performance
    /// - Target: <200ns (bounded O(16) search)
    /// - Atomic operations: Up to 16 loads + 1 CAS (initialization)
    ///
    /// # Safety
    /// - #ASSUME: CAS prevents double-initialization
    /// - #VERIFY: Property test validates slot uniqueness
    pub fn get_or_init(&self, provider_id: u64, now_ns: u64) -> Option<&ProviderCircuitStatus> {
        debug_assert!(provider_id != 0, "Provider ID cannot be zero");

        // First pass: Search for existing circuit
        for circuit in &self.circuits {
            let id = circuit.provider_id.load(Ordering::Relaxed);
            if id == provider_id {
                return Some(circuit);
            }
        }

        // Second pass: Try to claim empty slot
        for circuit in &self.circuits {
            let id = circuit.provider_id.load(Ordering::Acquire);
            if id == 0 {
                // Empty slot, try to claim via CAS
                match circuit.provider_id.compare_exchange(
                    0,
                    provider_id,
                    Ordering::Release,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {
                        // Successfully claimed, initialize (but provider_id already set by CAS)
                        let initial_state = ProviderCircuitStatus::pack_state(0, 0, CircuitState::Closed, 1);
                        circuit.state_packed.store(initial_state, Ordering::Relaxed);
                        circuit.failure_rate_bp.store(0, Ordering::Relaxed);
                        circuit.last_state_change_ns.store(now_ns, Ordering::Relaxed);
                        circuit.cooldown_remaining_ns.store(0, Ordering::Relaxed);
                        return Some(circuit);
                    }
                    Err(existing_id) => {
                        // CAS failed, another thread claimed this slot
                        // Check if they claimed it for our provider_id
                        if existing_id == provider_id {
                            return Some(circuit);
                        }
                        // Continue searching
                    }
                }
            }
        }

        // Third pass: Check if another thread initialized our provider
        for circuit in &self.circuits {
            let id = circuit.provider_id.load(Ordering::Relaxed);
            if id == provider_id {
                return Some(circuit);
            }
        }

        // All slots full
        None
    }

    /// Record success for a specific provider
    ///
    /// # Performance
    /// - Target: <300ns (200ns search + 100ns update)
    pub fn record_success(&self, provider_id: u64, now_ns: u64) {
        if let Some(circuit) = self.get_or_init(provider_id, now_ns) {
            circuit.record_success(now_ns);
        }
        // Silently ignore if slots exhausted (graceful degradation)
    }

    /// Record failure for a specific provider
    ///
    /// # Performance
    /// - Target: <300ns (200ns search + 100ns update)
    pub fn record_failure(&self, provider_id: u64, now_ns: u64) {
        if let Some(circuit) = self.get_or_init(provider_id, now_ns) {
            circuit.record_failure(now_ns);
        }
        // Silently ignore if slots exhausted (graceful degradation)
    }

    /// Check if provider circuit is open (should block requests)
    ///
    /// Returns false if provider not found (fail-open policy for unknown providers).
    ///
    /// # Performance
    /// - Target: <250ns (200ns search + 50ns check)
    pub fn is_provider_open(&self, provider_id: u64, now_ns: u64) -> bool {
        self.get_or_init(provider_id, now_ns)
            .map(|c| c.is_open())
            .unwrap_or(false)
    }

    /// Get circuit state for a specific provider
    pub fn get_state(&self, provider_id: u64, now_ns: u64) -> Option<CircuitState> {
        self.get_or_init(provider_id, now_ns).map(|c| c.state())
    }

    /// Get failure rate for a specific provider (basis points)
    pub fn get_failure_rate_bp(&self, provider_id: u64, now_ns: u64) -> Option<u64> {
        self.get_or_init(provider_id, now_ns)
            .map(|c| c.failure_rate_bp())
    }

    /// Get count of active providers
    pub fn active_provider_count(&self) -> usize {
        self.circuits
            .iter()
            .filter(|c| c.provider_id() != 0)
            .count()
    }

    /// Get all active provider IDs
    pub fn active_provider_ids(&self) -> Vec<u64> {
        self.circuits
            .iter()
            .filter_map(|c| {
                let id = c.provider_id();
                if id != 0 {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for ProviderCircuitArray {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_status_size() {
        assert_eq!(std::mem::size_of::<ProviderCircuitStatus>(), 64);
    }

    #[test]
    fn test_circuit_status_alignment() {
        assert_eq!(std::mem::align_of::<ProviderCircuitStatus>(), 64);
    }

    #[test]
    fn test_circuit_array_size() {
        assert_eq!(std::mem::size_of::<ProviderCircuitArray>(), 1024);
    }

    #[test]
    fn test_circuit_array_alignment() {
        assert_eq!(std::mem::align_of::<ProviderCircuitArray>(), 64);
    }

    #[test]
    fn test_circuit_state_encoding() {
        assert_eq!(CircuitState::Closed.to_bits(), 0);
        assert_eq!(CircuitState::HalfOpen.to_bits(), 1);
        assert_eq!(CircuitState::Open.to_bits(), 2);

        assert_eq!(CircuitState::from_bits(0), CircuitState::Closed);
        assert_eq!(CircuitState::from_bits(1), CircuitState::HalfOpen);
        assert_eq!(CircuitState::from_bits(2), CircuitState::Open);
        assert_eq!(CircuitState::from_bits(3), CircuitState::Open);
    }

    #[test]
    fn test_pack_unpack_state() {
        let packed = ProviderCircuitStatus::pack_state(123, 456, CircuitState::HalfOpen, 789);
        let (f, s, state, g) = ProviderCircuitStatus::unpack_state(packed);

        assert_eq!(f, 123);
        assert_eq!(s, 456);
        assert_eq!(state, CircuitState::HalfOpen);
        assert_eq!(g, 789);
    }

    #[test]
    fn test_circuit_status_init() {
        let status = ProviderCircuitStatus::new();
        status.init(42, 1000);

        assert_eq!(status.provider_id(), 42);
        assert_eq!(status.state(), CircuitState::Closed);
        assert_eq!(status.failure_rate_bp(), 0);
        assert!(!status.is_open());
    }

    #[test]
    fn test_record_success() {
        let status = ProviderCircuitStatus::new();
        status.init(1, 1000);

        status.record_success(1100);
        let packed = status.state_packed.load(Ordering::Relaxed);
        let (_, successes, _, _) = ProviderCircuitStatus::unpack_state(packed);

        assert_eq!(successes, 1);
        assert_eq!(status.failure_rate_bp(), 0);
    }

    #[test]
    fn test_record_failure() {
        let status = ProviderCircuitStatus::new();
        status.init(1, 1000);

        status.record_failure(1100);
        let packed = status.state_packed.load(Ordering::Relaxed);
        let (failures, _, _, _) = ProviderCircuitStatus::unpack_state(packed);

        assert_eq!(failures, 1);
    }

    #[test]
    fn test_circuit_opens_at_threshold() {
        let status = ProviderCircuitStatus::new();
        status.init(1, 1000);

        // Record 10 successes, 2 failures → 16.7% failure rate → Circuit should open
        for _ in 0..10 {
            status.record_success(1100);
        }
        for _ in 0..2 {
            status.record_failure(1200);
        }

        assert!(status.is_open(), "Circuit should open at >10% failure rate");
        assert!(status.failure_rate_bp() >= thresholds::OPEN_THRESHOLD_BP);
    }

    #[test]
    fn test_circuit_stays_closed_below_threshold() {
        let status = ProviderCircuitStatus::new();
        status.init(1, 1000);

        // Record 95 successes, 5 failures → 5% failure rate → Circuit stays closed
        for _ in 0..95 {
            status.record_success(1100);
        }
        for _ in 0..5 {
            status.record_failure(1200);
        }

        assert!(!status.is_open(), "Circuit should stay closed at <10% failure rate");
        assert!(status.failure_rate_bp() < thresholds::OPEN_THRESHOLD_BP);
    }

    #[test]
    fn test_provider_array_get_or_init() {
        let array = ProviderCircuitArray::new();

        let circuit1 = array.get_or_init(1, 1000);
        assert!(circuit1.is_some());
        assert_eq!(circuit1.unwrap().provider_id(), 1);

        // Same provider returns same circuit
        let circuit1_again = array.get_or_init(1, 1100);
        assert!(circuit1_again.is_some());
        assert_eq!(circuit1_again.unwrap().provider_id(), 1);
    }

    #[test]
    fn test_provider_array_multiple_providers() {
        let array = ProviderCircuitArray::new();

        for i in 1..=16 {
            let circuit = array.get_or_init(i as u64, 1000);
            assert!(circuit.is_some());
            assert_eq!(circuit.unwrap().provider_id(), i as u64);
        }

        assert_eq!(array.active_provider_count(), 16);
    }

    #[test]
    fn test_provider_array_exhaustion() {
        let array = ProviderCircuitArray::new();

        // Fill all 16 slots
        for i in 1..=16 {
            let circuit = array.get_or_init(i as u64, 1000);
            assert!(circuit.is_some());
        }

        // 17th provider fails (graceful degradation)
        let circuit17 = array.get_or_init(17, 1000);
        assert!(circuit17.is_none(), "Should return None when all slots full");
    }

    #[test]
    fn test_provider_array_record_success() {
        let array = ProviderCircuitArray::new();

        array.record_success(1, 1000);
        array.record_success(1, 1100);

        let circuit = array.get_or_init(1, 1200);
        assert!(circuit.is_some());

        let packed = circuit.unwrap().state_packed.load(Ordering::Relaxed);
        let (_, successes, _, _) = ProviderCircuitStatus::unpack_state(packed);
        assert_eq!(successes, 2);
    }

    #[test]
    fn test_provider_array_independent_tracking() {
        let array = ProviderCircuitArray::new();

        // Provider 1: 10 failures (should open)
        for _ in 0..10 {
            array.record_failure(1, 1000);
        }

        // Provider 2: 10 successes (should stay closed)
        for _ in 0..10 {
            array.record_success(2, 1000);
        }

        assert!(
            array.is_provider_open(1, 1100),
            "Provider 1 should have circuit open"
        );
        assert!(
            !array.is_provider_open(2, 1100),
            "Provider 2 should have circuit closed"
        );
    }

    #[test]
    fn test_concurrent_initialization() {
        use std::sync::Arc;
        use std::thread;

        let array = Arc::new(ProviderCircuitArray::new());
        let mut handles = vec![];

        // 10 threads try to initialize provider 1 simultaneously
        for _ in 0..10 {
            let arr = Arc::clone(&array);
            handles.push(thread::spawn(move || {
                arr.get_or_init(1, 1000);
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Only 1 slot should be allocated
        assert_eq!(array.active_provider_count(), 1);
        let circuit = array.get_or_init(1, 2000).unwrap();
        assert_eq!(circuit.provider_id(), 1);
    }

    #[test]
    fn test_active_provider_ids() {
        let array = ProviderCircuitArray::new();

        array.record_success(5, 1000);
        array.record_success(10, 1000);
        array.record_success(15, 1000);

        let mut ids = array.active_provider_ids();
        ids.sort();

        assert_eq!(ids, vec![5, 10, 15]);
    }
}
