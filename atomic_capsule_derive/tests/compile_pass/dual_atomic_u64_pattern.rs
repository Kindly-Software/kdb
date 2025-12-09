//! Test: DualAtomicU64 pattern (primary + secondary + generation)
//!
//! T28 Q1 (Core Behaviors): Testing dual-channel coordination pattern
//! UCE34 Q10: DualAtomicU64 for TOCTOU prevention
//! ASSUM: Generation counter prevents torn reads
//!
//! Expected: Compilation succeeds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct DualAtomicPattern {
    generation: AtomicU64,  // Generation counter (TOCTOU prevention)
    primary: AtomicU64,     // Primary value
    secondary: AtomicU64,   // Secondary value
    _padding: [u8; 40],
}

fn main() {
    use core::sync::atomic::Ordering;

    let capsule = DualAtomicPattern {
        generation: AtomicU64::new(0),
        primary: AtomicU64::new(0),
        secondary: AtomicU64::new(0),
        _padding: [0u8; 40],
    };

    // Simulate write with generation increment
    capsule.generation.fetch_add(1, Ordering::Release);
    capsule.primary.store(100, Ordering::Release);
    capsule.secondary.store(200, Ordering::Release);
    capsule.generation.fetch_add(1, Ordering::Release);

    // Simulate consistent read with generation check
    loop {
        let gen_before = capsule.generation.load(Ordering::Acquire);
        let primary = capsule.primary.load(Ordering::Relaxed);
        let secondary = capsule.secondary.load(Ordering::Relaxed);
        let gen_after = capsule.generation.load(Ordering::Acquire);

        if gen_before == gen_after && gen_before % 2 == 0 {
            // Consistent read
            assert_eq!(primary, 100);
            assert_eq!(secondary, 200);
            break;
        }
    }

    println!("DualAtomicU64 pattern verified with generation counters!");
}
