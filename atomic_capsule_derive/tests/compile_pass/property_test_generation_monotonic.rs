//! Test: Property test - generation counter is monotonic
//!
//! T28 Q8-Q9 (Property Testing): Generation counter invariants
//! UCE34 Q10: Generation must always increase
//! ASSUM: Generation counter prevents TOCTOU
//!
//! Expected: Compilation succeeds, property holds

use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::{AtomicU64, Ordering};

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct PropertyCapsule {
    generation: AtomicU64,
    value: AtomicU64,
    _padding: [u8; 48],
}

impl PropertyCapsule {
    fn update(&self, new_value: u64) {
        self.generation.fetch_add(1, Ordering::Release);
        self.value.store(new_value, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }
}

fn main() {
    let capsule = PropertyCapsule {
        generation: AtomicU64::new(0),
        value: AtomicU64::new(0),
        _padding: [0u8; 48],
    };

    // Property: Generation is monotonically increasing
    let mut last_gen = 0u64;
    for i in 1..=1000 {
        capsule.update(i);

        let current_gen = capsule.generation.load(Ordering::Acquire);

        // Invariant: Generation always increases
        assert!(
            current_gen > last_gen,
            "Generation must be monotonic: {} <= {}",
            current_gen,
            last_gen
        );

        last_gen = current_gen;
    }

    println!("Property verified: Generation counter is monotonically increasing!");
    println!("Final generation: {}", last_gen);
}
