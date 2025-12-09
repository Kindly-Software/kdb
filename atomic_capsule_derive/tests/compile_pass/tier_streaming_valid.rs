//! Test: Valid Streaming tier documentation
//!
//! T28 Q1 (Core Behaviors): Testing Streaming tier label
//! UCE34 Q10: Tier 5 Streaming capsules for continuous computation
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Streaming")]
#[repr(C, align(64))]
struct StreamingTierCapsule {
    stream_offset: AtomicU64,
    bytes_processed: AtomicU64,
    _padding: [u8; 48],
}

fn main() {
    let capsule = StreamingTierCapsule {
        stream_offset: AtomicU64::new(0),
        bytes_processed: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    println!("Streaming tier capsule (T5) label verified!");
}
