//! Test: Single AtomicU64 doesn't require generation counter
//!
//! T28 Q2 (Edge Cases): Generation counters only needed for multi-field coordination
//! UCE34 Q10: Single atomic operations are naturally atomic
//!
//! Expected: Compilation succeeds (no generation counter warning)

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct SingleAtomicCapsule {
    state: AtomicU64,  // Single atomic - no generation needed
    _padding: [u8; 56],
}

fn main() {
    use core::sync::atomic::Ordering;

    let capsule = SingleAtomicCapsule {
        state: AtomicU64::new(0),
        _padding: [0u8; 56],
    };

    // Single atomic operation is inherently consistent
    capsule.state.store(42, Ordering::Release);
    let value = capsule.state.load(Ordering::Acquire);
    assert_eq!(value, 42);

    println!("Single atomic capsule (no generation counter needed) verified!");
}
