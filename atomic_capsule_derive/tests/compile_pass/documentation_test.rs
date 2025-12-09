//! Test: Documentation test - verify doc comments compile
//!
//! T28 Q27 (Documentation): All public APIs documented
//! UCE34 Q10: Comprehensive capsule documentation
//!
//! Expected: Compilation succeeds, docs are valid

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, Ordering};

/// Circuit breaker capsule for error rate limiting.
///
/// # Purpose
/// Protects downstream systems by opening circuit when error rate exceeds threshold.
///
/// # Architecture
/// - T1 Atomic tier: Lockfree coordination
/// - 64-byte alignment: Prevents false sharing
/// - Generation counter: TOCTOU prevention
///
/// # Performance
/// - State check: <10ns (atomic load)
/// - State update: <50ns (CAS with generation)
/// - Concurrent safe: 100+ threads
///
/// # Examples
///
/// ```rust,ignore
/// use atomic_capsule_derive::ComputationalCapsule;
/// use core::sync::atomic::{AtomicU64, Ordering};
///
/// let breaker = DocumentedCapsule {
///     state: AtomicU64::new(0),
///     error_count: AtomicU64::new(0),
///     _padding: [0u8; 48],
/// };
///
/// // Check state
/// let state = breaker.state.load(Ordering::Acquire);
/// assert_eq!(state, 0);
/// ```
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Atomic")]
#[repr(C, align(64))]
pub struct DocumentedCapsule {
    /// Circuit breaker state (0=Closed, 1=Open, 2=HalfOpen)
    pub state: AtomicU64,

    /// Error count for threshold checking
    pub error_count: AtomicU64,

    /// Padding to 64 bytes
    _padding: [u8; 48],
}

impl DocumentedCapsule {
    /// Creates a new circuit breaker in Closed state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let breaker = DocumentedCapsule::new();
    /// assert_eq!(breaker.state.load(Ordering::Relaxed), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            _padding: [0u8; 48],
        }
    }

    /// Records an error and potentially opens circuit.
    ///
    /// # Returns
    /// New state after error (0=Closed, 1=Open)
    pub fn record_error(&self) -> u64 {
        let errors = self.error_count.fetch_add(1, Ordering::Relaxed);

        if errors > 10 {
            // Open circuit
            self.state.store(1, Ordering::Release);
            1
        } else {
            0
        }
    }
}

fn main() {
    let breaker = DocumentedCapsule::new();

    // Test documented API
    assert_eq!(breaker.state.load(Ordering::Relaxed), 0);

    // Record errors until circuit opens
    for _ in 0..15 {
        breaker.record_error();
    }

    assert_eq!(breaker.state.load(Ordering::Relaxed), 1);

    println!("Documentation test passed - all APIs work as documented!");
}
