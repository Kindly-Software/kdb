// Test: Suppression should work for special cases

#![warn(clippy::missing_capsule_verification)]

use core::sync::atomic::AtomicU64;

// OK: Explicitly suppressed (e.g., FFI types)
#[allow(clippy::missing_capsule_verification)]
#[repr(C, align(64))]
struct FfiCapsule {
    external_data: [u8; 64],
}

// OK: Module-level suppression
#[allow(clippy::missing_capsule_verification)]
mod special_cases {
    use core::sync::atomic::AtomicU64;

    #[repr(C, align(128))]
    pub struct SpecialCapsule {
        state: AtomicU64,
    }
}

fn main() {
    let _ = FfiCapsule {
        external_data: [0; 64],
    };

    let _ = special_cases::SpecialCapsule {
        state: AtomicU64::new(0),
    };
}
