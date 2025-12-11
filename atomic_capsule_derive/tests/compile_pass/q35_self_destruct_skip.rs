//! Test: Q35 Self-Destruct - Skip for pure SIMD capsule
//!
//! T28 Q19 (Integration): Testing skip_self_destruct = true for stateless capsules
//! UCE34 Q35: Stateless SIMD capsules can opt-out with documented justification
//! ASSUM: Pure SIMD primitive with no coordination state - self-destruct not applicable
//!
//! Expected: Compilation succeeds, SelfDestructible trait NOT generated

use atomic_capsule_derive::ComputationalCapsule;

/// Pure SIMD capsule - no atomic fields needed
/// #ASSUME_SIMD_STATELESS: Pure SIMD primitive with no coordination state
/// #VERIFY_SIMD: Self-destruct not applicable - no shared state to poison
#[derive(ComputationalCapsule)]
#[capsule(alignment = 32, size = 32, tier = "SIMD", skip_self_destruct = true)]
#[repr(C, align(32))]
struct SimdF32x8Capsule {
    /// 8 f32 values for SIMD operations (8 * 4 = 32 bytes)
    data: [f32; 8],
}

fn main() {
    let capsule = SimdF32x8Capsule {
        data: [0.0f32; 8],
    };

    // Verify traits (still Send + Sync for SIMD data)
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<SimdF32x8Capsule>();
    assert_sync::<SimdF32x8Capsule>();

    // Verify alignment and size
    assert_eq!(core::mem::align_of::<SimdF32x8Capsule>(), 32);
    assert_eq!(core::mem::size_of::<SimdF32x8Capsule>(), 32);

    println!("Q35 skip_self_destruct (SIMD) capsule compiled successfully!");
}
