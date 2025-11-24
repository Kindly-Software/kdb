// Test: Capsule with verification should NOT trigger warning

#![warn(clippy::missing_capsule_verification)]

use core::sync::atomic::AtomicU64;

// GOOD: Has manual verification
#[repr(C, align(64))]
struct VerifiedCapsule {
    state: AtomicU64,
}

// Manual verification macro (creates const _: () = { ... })
const _: () = {
    assert!(core::mem::align_of::<VerifiedCapsule>() == 64);
    assert!(core::mem::size_of::<VerifiedCapsule>() == 8);
};

// GOOD: Has derive macro
#[derive(ComputationalCapsule)]
#[repr(C, align(128))]
struct DerivedCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
}

fn main() {
    let _ = VerifiedCapsule {
        state: AtomicU64::new(0),
    };

    let _ = DerivedCapsule {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
    };
}
