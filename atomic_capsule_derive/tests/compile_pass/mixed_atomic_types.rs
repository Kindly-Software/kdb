//! Test: Capsule with mixed atomic types (U64, U32, U16, U8, Bool, I64, I32)
//!
//! T28 Q1 (Core Behaviors): Testing diverse atomic field types
//! UCE34 Q10: All atomic types are valid capsule fields
//!
//! Expected: Compilation succeeds, all atomic types work

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, AtomicU32, AtomicU16, AtomicU8, AtomicBool, AtomicI64, AtomicI32};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MixedAtomicsCapsule {
    u64_val: AtomicU64,   // 8 bytes
    u32_val: AtomicU32,   // 4 bytes
    u16_val: AtomicU16,   // 2 bytes
    u8_val: AtomicU8,     // 1 byte
    bool_val: AtomicBool, // 1 byte
    i64_val: AtomicI64,   // 8 bytes
    i32_val: AtomicI32,   // 4 bytes
    _padding: [u8; 36],   // 36 bytes → total 64 bytes
}

fn main() {
    use core::sync::atomic::Ordering;

    let capsule = MixedAtomicsCapsule {
        u64_val: AtomicU64::new(0),
        u32_val: AtomicU32::new(0),
        u16_val: AtomicU16::new(0),
        u8_val: AtomicU8::new(0),
        bool_val: AtomicBool::new(false),
        i64_val: AtomicI64::new(0),
        i32_val: AtomicI32::new(0),
        _padding: [0u8; 36],
    };

    // Test operations on each atomic type
    capsule.u64_val.store(123, Ordering::Relaxed);
    capsule.u32_val.store(456, Ordering::Relaxed);
    capsule.u16_val.store(789, Ordering::Relaxed);
    capsule.u8_val.store(255, Ordering::Relaxed);
    capsule.bool_val.store(true, Ordering::Relaxed);
    capsule.i64_val.store(-123, Ordering::Relaxed);
    capsule.i32_val.store(-456, Ordering::Relaxed);

    assert_eq!(capsule.u64_val.load(Ordering::Relaxed), 123);
    assert_eq!(capsule.bool_val.load(Ordering::Relaxed), true);

    println!("Mixed atomic types capsule verified!");
}
