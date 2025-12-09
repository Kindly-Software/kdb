//! Valid 128-byte aligned capsule (DualAtomicU64 pattern)

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
struct DualChannelCapsule {
    primary: AtomicU64,
    secondary: AtomicU64,
    _padding: [u8; 112],
}

fn main() {
    println!("Valid 128-byte aligned capsule compiled successfully!");
}
