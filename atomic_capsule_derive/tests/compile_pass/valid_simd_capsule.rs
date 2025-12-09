//! Valid SIMD capsule - should compile successfully

use atomic_capsule_derive::ComputationalCapsule;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, tier = "SIMD")]
#[repr(C, align(64))]
struct SimdVenueScorer {
    scores: [f64; 8],
}

fn main() {
    let capsule = SimdVenueScorer {
        scores: [0.0; 8],
    };

    println!("Valid SIMD capsule compiled successfully!");
}
