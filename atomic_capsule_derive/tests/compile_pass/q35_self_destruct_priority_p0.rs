//! Test: Q35 Self-Destruct - Explicit P0 (Critical) priority
//!
//! T28 Q17 (Integration): Testing explicit priority override to P0
//! UCE34 Q35: P0 = Critical priority for coordination capsules
//!
//! Expected: Compilation succeeds with P0 priority configured

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, priority = "P0")]
#[repr(C, align(64))]
struct CriticalPriorityCapsule {
    /// Primary coordination state
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Critical operation counter
    ops_count: AtomicU64,
    /// Padding to 64 bytes
    _padding: [u8; 40],
}

fn main() {
    let capsule = CriticalPriorityCapsule {
        state: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        ops_count: AtomicU64::new(0),
        _padding: [0u8; 40],
    };

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<CriticalPriorityCapsule>();
    assert_sync::<CriticalPriorityCapsule>();

    // Verify alignment and size
    assert_eq!(core::mem::align_of::<CriticalPriorityCapsule>(), 64);
    assert_eq!(core::mem::size_of::<CriticalPriorityCapsule>(), 64);

    println!("Q35 P0 (Critical) priority capsule compiled successfully!");
}
