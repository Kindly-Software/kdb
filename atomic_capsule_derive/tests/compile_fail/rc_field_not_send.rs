//! Test: Rc field is not Send/Sync (compile should fail or warn)
//!
//! T28 Q2 (Edge Cases): Testing thread-safety requirements
//! UCE34 Q10: Capsules MUST be Send + Sync
//!
//! Expected: Compilation error - Rc is not Send/Sync
//!
//! Note: This test verifies that capsules with non-Send fields
//! cannot be used across threads (enforced by trait bounds)

use atomic_capsule_derive::ComputationalCapsule;
use std::rc::Rc;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct BadCapsuleWithRc {
    state: Rc<u64>,  // ❌ Not Send/Sync
    _padding: [u8; 48],
}

fn main() {
    // This should fail: Rc prevents Send/Sync
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<BadCapsuleWithRc>();  // ❌ Should fail
    assert_sync::<BadCapsuleWithRc>();  // ❌ Should fail
}
