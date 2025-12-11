//! Test: Q35 Self-Destruct - Basic capsule with AtomicU64
//!
//! T28 Q15 (Integration): Testing Q35 self-destruct auto-generation
//! UCE34 Q35: Mandatory self-destruction for capsule protection
//!
//! Expected: Compilation succeeds with SelfDestructible trait generated

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BasicSelfDestructCapsule {
    /// State for self-destruct poison tracking
    state: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// Padding to 64 bytes
    _padding: [u8; 48],
}

fn main() {
    let capsule = BasicSelfDestructCapsule {
        state: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    // Verify Send + Sync traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<BasicSelfDestructCapsule>();
    assert_sync::<BasicSelfDestructCapsule>();

    // Verify alignment and size at runtime
    assert_eq!(core::mem::align_of::<BasicSelfDestructCapsule>(), 64);
    assert_eq!(core::mem::size_of::<BasicSelfDestructCapsule>(), 64);

    println!("Q35 basic self-destruct capsule compiled successfully!");
}
