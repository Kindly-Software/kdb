//! Simple example demonstrating #[derive(ComputationalCapsule)]

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

/// Circuit breaker capsule with automatic verification
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Atomic")]
#[repr(C, align(64))]
struct CircuitBreakerCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

/// SIMD capsule for venue scoring
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, tier = "SIMD")]
#[repr(C, align(128))]
struct SimdVenueScorer {
    scores_a: [f64; 4],
    scores_b: [f64; 4],
    _padding: [u8; 64],
}

fn main() {
    // Create capsules
    let breaker = CircuitBreakerCapsule {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };

    let scorer = SimdVenueScorer {
        scores_a: [0.0; 4],
        scores_b: [0.0; 4],
        _padding: [0u8; 64],
    };

    // Verify capsules are Send + Sync (thread-safe)
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<CircuitBreakerCapsule>();
    assert_sync::<CircuitBreakerCapsule>();
    assert_send::<SimdVenueScorer>();
    assert_sync::<SimdVenueScorer>();

    println!("✓ All capsules compiled and verified successfully!");
    println!(
        "  Circuit breaker: alignment={}, size={}",
        core::mem::align_of::<CircuitBreakerCapsule>(),
        core::mem::size_of::<CircuitBreakerCapsule>()
    );
    println!(
        "  SIMD scorer: alignment={}, size={}",
        core::mem::align_of::<SimdVenueScorer>(),
        core::mem::size_of::<SimdVenueScorer>()
    );
}
