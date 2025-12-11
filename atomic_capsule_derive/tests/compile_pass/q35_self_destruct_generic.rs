//! Test: Q35 Self-Destruct - Generic capsule with type parameter `<T>`
//!
//! T28 Q20 (Integration): Testing Q35 with generic type parameters
//! UCE34 Q35: Generic capsules support self-destruct with proper bounds
//!
//! Expected: Compilation succeeds for generic capsule

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;
use core::marker::PhantomData;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct GenericSelfDestructCapsule<T: Send + Sync> {
    /// State for poison tracking
    state: AtomicU64,
    /// Generation counter
    generation: AtomicU64,
    /// Phantom data for generic type
    _phantom: PhantomData<T>,
    /// Padding to 64 bytes
    _padding: [u8; 48],
}

fn main() {
    // Instantiate with concrete types
    let capsule_u32: GenericSelfDestructCapsule<u32> = GenericSelfDestructCapsule {
        state: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        _phantom: PhantomData,
        _padding: [0u8; 48],
    };

    let capsule_string: GenericSelfDestructCapsule<String> = GenericSelfDestructCapsule {
        state: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        _phantom: PhantomData,
        _padding: [0u8; 48],
    };

    // Verify traits for all instantiations
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<GenericSelfDestructCapsule<u32>>();
    assert_sync::<GenericSelfDestructCapsule<u32>>();
    assert_send::<GenericSelfDestructCapsule<String>>();
    assert_sync::<GenericSelfDestructCapsule<String>>();

    // Verify alignment and size (same for all T due to PhantomData)
    assert_eq!(core::mem::align_of::<GenericSelfDestructCapsule<u32>>(), 64);
    assert_eq!(core::mem::size_of::<GenericSelfDestructCapsule<u32>>(), 64);

    println!("Q35 generic self-destruct capsule compiled successfully!");
}
