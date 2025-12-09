//! Test: Valid Persistent tier documentation
//!
//! T28 Q1 (Core Behaviors): Testing Persistent tier label (T9 Extended)
//! UCE34 Q10: Tier 9 Persistent capsules for crash-safe storage
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Persistent")]
#[repr(C, align(64))]
struct PersistentTierCapsule {
    file_offset: AtomicU64,
    sync_generation: AtomicU64,
    _padding: [u8; 48],
}

fn main() {
    let capsule = PersistentTierCapsule {
        file_offset: AtomicU64::new(0),
        sync_generation: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    println!("Persistent tier capsule (T9 Extended) label verified!");
}
