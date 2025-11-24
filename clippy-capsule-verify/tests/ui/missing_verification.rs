// Test: Capsule without verification should trigger warning

#![warn(clippy::missing_capsule_verification)]

use core::sync::atomic::AtomicU64;

// BAD: Missing verification
#[repr(C, align(64))]
struct UnverifiedCapsule {
    state: AtomicU64,
}

// BAD: Has align but no verification
#[repr(C, align(128))]
struct AnotherUnverified {
    primary: AtomicU64,
    secondary: AtomicU64,
}

fn main() {
    let _ = UnverifiedCapsule {
        state: AtomicU64::new(0),
    };

    let _ = AnotherUnverified {
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
    };
}
