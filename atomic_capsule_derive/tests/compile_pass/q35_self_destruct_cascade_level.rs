//! Test: Q35 Self-Destruct - Custom cascade_level = 5
//!
//! T28 Q18 (Integration): Testing custom cascade level configuration
//! UCE34 Q35: cascade_level defines position in capsule hierarchy (0-15)
//!
//! Expected: Compilation succeeds with cascade_level = 5

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, cascade_level = 5)]
#[repr(C, align(64))]
struct MidLevelCapsule {
    /// State for cascade propagation
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Parent reference (cascade_level 4)
    parent_id: AtomicU64,
    /// Child count (cascade_level 6+)
    child_count: AtomicU64,
    /// Padding to 64 bytes
    _padding: [u8; 32],
}

fn main() {
    let capsule = MidLevelCapsule {
        state: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        parent_id: AtomicU64::new(0),
        child_count: AtomicU64::new(0),
        _padding: [0u8; 32],
    };

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<MidLevelCapsule>();
    assert_sync::<MidLevelCapsule>();

    // Verify alignment and size
    assert_eq!(core::mem::align_of::<MidLevelCapsule>(), 64);
    assert_eq!(core::mem::size_of::<MidLevelCapsule>(), 64);

    println!("Q35 cascade_level = 5 capsule compiled successfully!");
}
