//! Test: Capsule with ZST marker field (PhantomData)
//!
//! T28 Q2 (Edge Cases): Testing PhantomData zero-sized type field
//! UCE34 Q10: Capsules can contain zero-sized type markers
//!
//! Expected: Compilation succeeds with PhantomData marker

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;
use core::marker::PhantomData;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct LifetimeCapsule {
    state: AtomicU64,
    _phantom: PhantomData<()>,
    _padding: [u8; 56],
}

fn main() {
    let capsule = LifetimeCapsule {
        state: AtomicU64::new(42),
        _phantom: PhantomData,
        _padding: [0u8; 56],
    };

    // Verify it can be used in a scope
    {
        let _ref = &capsule;
        println!("Lifetime capsule created and borrowed successfully!");
    }
}
