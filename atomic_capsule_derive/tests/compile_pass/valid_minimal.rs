//! Minimal valid capsule - alignment only

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64)]
#[repr(C, align(64))]
struct MinimalCapsule {
    data: u64,
    _padding: [u8; 56],
}

fn main() {
    println!("Minimal capsule compiled successfully!");
}
