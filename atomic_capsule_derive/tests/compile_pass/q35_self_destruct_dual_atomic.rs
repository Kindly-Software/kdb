//! Test: Q35 Self-Destruct - Capsule with DualAtomicU64 pattern
//!
//! T28 Q16 (Integration): Testing Q35 with dual atomic fields for full poison support
//! UCE34 Q35: Dual atomic fields enable primary/secondary channel poisoning
//!
//! Expected: Compilation succeeds with poison propagation support
//!
//! Note: Uses raw AtomicU64 pair pattern since derive macro validates at compile-time
//! In production, use atomic_capsule::patterns::DualAtomicU64

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct DualAtomicSelfDestructCapsule {
    /// Primary coordination channel
    primary: AtomicU64,
    /// Secondary channel for poison tracking
    secondary: AtomicU64,
    /// Generation counter for TOCTOU prevention
    generation: AtomicU64,
    /// Padding to 64 bytes (64 - 3*8 = 40)
    _padding: [u8; 40],
}

fn main() {
    let _capsule = DualAtomicSelfDestructCapsule {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        _padding: [0u8; 40],
    };

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<DualAtomicSelfDestructCapsule>();
    assert_sync::<DualAtomicSelfDestructCapsule>();

    // Verify alignment and size
    assert_eq!(core::mem::align_of::<DualAtomicSelfDestructCapsule>(), 64);
    assert_eq!(core::mem::size_of::<DualAtomicSelfDestructCapsule>(), 64);

    println!("Q35 DualAtomicU64 pattern self-destruct capsule compiled successfully!");
}
