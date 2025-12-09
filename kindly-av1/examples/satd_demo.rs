//! SATD (Sum of Absolute Transformed Differences) Demo
//!
//! Demonstrates the usage of `compute_satd_4x4()` for frequency-domain
//! distortion measurement using Hadamard transform.
//!
//! Run with: `cargo run --example satd_demo --features nightly`

use kindly_av1::encoder::IntraPredictionCapsule;

fn main() {
    println!("=== SATD (Sum of Absolute Transformed Differences) Demo ===\n");

    // Example 1: Identical blocks (zero distortion)
    println!("Example 1: Identical Blocks");
    let original = [128u16; 16];
    let predicted = [128u16; 16];
    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    println!("  Original:  {:?}", &original[..4]);
    println!("  Predicted: {:?}", &predicted[..4]);
    println!("  SATD: {} (expected: 0)\n", satd);

    // Example 2: Constant DC offset
    println!("Example 2: Constant DC Offset");
    let original = [128u16; 16];
    let predicted = [100u16; 16];
    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    println!("  Original:  {:?}", &original[..4]);
    println!("  Predicted: {:?}", &predicted[..4]);
    println!(
        "  SATD: {} (expected: 224 - DC energy concentrated)\n",
        satd
    );

    // Example 3: Strong vertical edge
    println!("Example 3: Strong Vertical Edge");
    let mut original = [0u16; 16];
    for row in 0..4 {
        for col in 0..4 {
            original[row * 4 + col] = if col < 2 { 100 } else { 200 };
        }
    }
    let predicted = [150u16; 16]; // Smooth prediction misses edge
    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    println!("  Original (vertical edge):");
    for row in 0..4 {
        println!("    {:?}", &original[row * 4..(row + 1) * 4]);
    }
    println!("  Predicted (smooth): {:?}", &predicted[..4]);
    println!("  SATD: {} (high due to edge)\n", satd);

    // Example 4: Checkerboard pattern (high-frequency content)
    println!("Example 4: Checkerboard Pattern");
    let mut original = [0u16; 16];
    let mut predicted = [0u16; 16];
    for i in 0..16 {
        let row = i / 4;
        let col = i % 4;
        original[i] = if (row + col) % 2 == 0 { 200 } else { 100 };
        predicted[i] = if (row + col) % 2 == 0 { 100 } else { 200 }; // Inverted
    }
    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    println!("  Original (checkerboard):");
    for row in 0..4 {
        println!("    {:?}", &original[row * 4..(row + 1) * 4]);
    }
    println!("  Predicted (inverted checkerboard):");
    for row in 0..4 {
        println!("    {:?}", &predicted[row * 4..(row + 1) * 4]);
    }
    println!(
        "  SATD: {} (very high due to high-frequency content)\n",
        satd
    );

    // Example 5: Horizontal gradient
    println!("Example 5: Horizontal Gradient");
    let mut original = [0u16; 16];
    let mut predicted = [0u16; 16];
    for row in 0..4 {
        for col in 0..4 {
            original[row * 4 + col] = 100 + (col as u16 * 20);
            predicted[row * 4 + col] = 100 + (col as u16 * 10); // Half the gradient
        }
    }
    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    println!("  Original (strong gradient):");
    for row in 0..4 {
        println!("    {:?}", &original[row * 4..(row + 1) * 4]);
    }
    println!("  Predicted (weak gradient):");
    for row in 0..4 {
        println!("    {:?}", &predicted[row * 4..(row + 1) * 4]);
    }
    println!("  SATD: {} (moderate due to gradient mismatch)\n", satd);

    // Example 6: SATD vs SAD comparison
    println!("Example 6: SATD vs SAD Comparison");
    let original = [
        150, 160, 170, 180, 140, 150, 160, 170, 130, 140, 150, 160, 120, 130, 140, 150,
    ];
    let predicted = [
        145, 155, 165, 175, 135, 145, 155, 165, 125, 135, 145, 155, 115, 125, 135, 145,
    ];

    let satd = IntraPredictionCapsule::compute_satd_4x4(&original, &predicted);
    let sad: u32 = original
        .iter()
        .zip(predicted.iter())
        .map(|(&o, &p)| (o as i32 - p as i32).abs() as u32)
        .sum();

    println!("  Original:  {:?}", &original[..4]);
    println!("  Predicted: {:?}", &predicted[..4]);
    println!("  SATD: {}", satd);
    println!("  SAD:  {}", sad);
    println!("  Ratio (SATD/SAD): {:.2}", satd as f64 / sad as f64);
    println!("  Note: SATD typically >= SAD for natural content\n");

    println!("=== Demo Complete ===");
    println!("SATD provides better edge detection and RDO correlation than SAD.");
    println!("Use SATD for mode decision and rate-distortion optimization.");
}
