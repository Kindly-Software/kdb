//! Test: Valid Network tier documentation
//!
//! T28 Q1 (Core Behaviors): Testing Network tier label (T8 Extended)
//! UCE34 Q10: Tier 8 Network capsules for zero-copy networking
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64, tier = "Network")]
#[repr(C, align(64))]
struct NetworkTierCapsule {
    socket_fd: AtomicU64,
    bytes_sent: AtomicU64,
    _padding: [u8; 48],
}

fn main() {
    let capsule = NetworkTierCapsule {
        socket_fd: AtomicU64::new(0),
        bytes_sent: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    println!("Network tier capsule (T8 Extended) label verified!");
}
