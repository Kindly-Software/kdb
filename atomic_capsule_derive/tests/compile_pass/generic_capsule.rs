//! Test: Generic capsule with type parameter
//!
//! T28 Q2 (Edge Cases): Testing generic type support
//! UCE34 Q10: Capsules can be generic over types
//!
//! Expected: Compilation succeeds for generic capsule

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;
use core::marker::PhantomData;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct GenericCapsule<T: Send + Sync> {
    state: AtomicU64,
    _phantom: PhantomData<T>,
    _padding: [u8; 56],
}

fn main() {
    // Instantiate with concrete type
    let capsule_u32: GenericCapsule<u32> = GenericCapsule {
        state: AtomicU64::new(0),
        _phantom: PhantomData,
        _padding: [0u8; 56],
    };

    let capsule_string: GenericCapsule<String> = GenericCapsule {
        state: AtomicU64::new(0),
        _phantom: PhantomData,
        _padding: [0u8; 56],
    };

    // Verify traits
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<GenericCapsule<u32>>();
    assert_sync::<GenericCapsule<u32>>();

    println!("Generic capsule verified for multiple type parameters!");
}
