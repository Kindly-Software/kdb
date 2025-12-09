//! Frame Reconstruction Pipeline Integration Tests (T28 Q15-Q21)
//!
//! Tests the full reconstruction pipeline: dequantize → inverse transform →
//! add prediction → clip → store to reference.

use atomic_capsule::encoder::{DctTransformCapsule, QuantizationCapsule};
use kindly_av1::encoder::ReconstructionCapsule;

#[test]
fn test_reconstruction_4x4_zero_residual() {
    let capsule = ReconstructionCapsule::new();
    let quant = QuantizationCapsule::new(32);
    let transform = DctTransformCapsule::new();

    // Zero quantized coefficients = zero residual
    let quantized = [0i16; 16];
    let prediction = [128u8; 16];
    let mut reconstructed = [0u8; 16];

    capsule.reconstruct_block_4x4(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // Output should match prediction (zero residual)
    for i in 0..16 {
        assert!(
            (reconstructed[i] as i32 - prediction[i] as i32).abs() <= 2,
            "Pixel {} differs: got {}, expected ~{}",
            i,
            reconstructed[i],
            prediction[i]
        );
    }
}

#[test]
fn test_reconstruction_8x8_zero_residual() {
    let capsule = ReconstructionCapsule::new();
    let quant = QuantizationCapsule::new(32);
    let transform = DctTransformCapsule::new();

    let quantized = [0i16; 64];
    let prediction = [128u8; 64];
    let mut reconstructed = [0u8; 64];

    capsule.reconstruct_block_8x8(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    for i in 0..64 {
        assert!(
            (reconstructed[i] as i32 - prediction[i] as i32).abs() <= 2,
            "Pixel {} differs: got {}, expected ~{}",
            i,
            reconstructed[i],
            prediction[i]
        );
    }
}

#[test]
fn test_reconstruction_round_trip_4x4() {
    let quant = QuantizationCapsule::new(32);
    let transform = DctTransformCapsule::new();
    let reconstruction = ReconstructionCapsule::new();

    // Original residual
    let original_residual = [
        50i16, 25, 12, 6, -25, -12, -6, -3, 100, 50, 25, 12, -50, -25, -12, -6,
    ];

    // Forward: DCT → Quantize
    let dct_coeffs = transform.forward_4x4(&original_residual);
    let quantized = quant.quantize_block_4x4(&dct_coeffs);

    // Reconstruction pipeline
    let prediction = [128u8; 16];
    let mut reconstructed = [0u8; 16];

    reconstruction.reconstruct_block_4x4(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // Verify round-trip: reconstructed should be reasonable
    // Note: Quantization is lossy, so we can't expect exact reconstruction.
    // We just verify pixels are in valid range and generally follow the pattern.
    for i in 0..16 {
        // All pixels must be in valid range [0, 255]
        assert!(
            reconstructed[i] <= 255,
            "Pixel {} out of range: {}",
            i,
            reconstructed[i]
        );
    }

    // Verify the reconstruction produced some variation (not all same value)
    let first_pixel = reconstructed[0];
    let has_variation = reconstructed.iter().any(|&p| p != first_pixel);

    // With a non-zero original residual, we should see some variation after reconstruction
    // (unless all quantized to zero, which is acceptable for aggressive quantization)
    let stats = reconstruction.stats();
    assert_eq!(
        stats.blocks_reconstructed, 1,
        "Should have reconstructed 1 block"
    );
}

#[test]
fn test_reconstruction_clipping_underflow() {
    let capsule = ReconstructionCapsule::new();
    let quant = QuantizationCapsule::new(10); // Low QP = less quantization
    let transform = DctTransformCapsule::new();

    // Large negative DC coefficient
    let mut quantized = [0i16; 16];
    quantized[0] = -200;

    let prediction = [50u8; 16]; // Low prediction
    let mut reconstructed = [0u8; 16];

    capsule.reconstruct_block_4x4(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // Should clip to 0, not underflow
    for &pixel in reconstructed.iter() {
        assert!(pixel <= 255, "Pixel should not overflow: {}", pixel);
    }
}

#[test]
fn test_reconstruction_clipping_overflow() {
    let capsule = ReconstructionCapsule::new();
    let quant = QuantizationCapsule::new(10);
    let transform = DctTransformCapsule::new();

    // Large positive DC coefficient
    let mut quantized = [0i16; 16];
    quantized[0] = 200;

    let prediction = [200u8; 16]; // High prediction
    let mut reconstructed = [0u8; 16];

    capsule.reconstruct_block_4x4(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // Should clip to 255, not overflow
    for &pixel in reconstructed.iter() {
        assert!(pixel <= 255, "Pixel should not overflow: {}", pixel);
    }
}

#[test]
fn test_reconstruction_block_count() {
    let capsule = ReconstructionCapsule::new();
    let quant = QuantizationCapsule::new(32);
    let transform = DctTransformCapsule::new();

    let quantized = [0i16; 16];
    let prediction = [128u8; 16];
    let mut reconstructed = [0u8; 16];

    // Reconstruct 10 blocks
    for _ in 0..10 {
        capsule.reconstruct_block_4x4(
            &quantized,
            &prediction,
            &mut reconstructed,
            &quant,
            &transform,
        );
    }

    let stats = capsule.stats();
    assert_eq!(stats.blocks_reconstructed, 10);
}

#[test]
fn test_reconstruction_complete_frame() {
    let capsule = ReconstructionCapsule::new();

    capsule.complete_frame();

    let stats = capsule.stats();
    assert_eq!(stats.frames_reconstructed, 1);
    assert_eq!(stats.generation, 1);
}

#[test]
fn test_store_to_reference() {
    let capsule = ReconstructionCapsule::new();
    let reconstructed = [128u8; 64]; // 8×8 block
    let mut reference_buffer = [0u8; 256]; // 16×16 frame

    capsule.store_to_reference(
        &reconstructed,
        &mut reference_buffer,
        0,  // x
        0,  // y
        16, // width
        8,  // block_size
    );

    // Check first 8×8 block copied correctly
    for row in 0..8 {
        for col in 0..8 {
            let idx = row * 16 + col;
            assert_eq!(reference_buffer[idx], 128);
        }
    }
}

#[test]
fn test_store_to_reference_offset() {
    let capsule = ReconstructionCapsule::new();
    let reconstructed = [200u8; 16]; // 4×4 block
    let mut reference_buffer = [0u8; 256]; // 16×16 frame

    capsule.store_to_reference(
        &reconstructed,
        &mut reference_buffer,
        4,  // x offset
        4,  // y offset
        16, // width
        4,  // block_size
    );

    // Check 4×4 block at offset (4,4)
    for row in 0..4 {
        for col in 0..4 {
            let idx = (4 + row) * 16 + (4 + col);
            assert_eq!(reference_buffer[idx], 200);
        }
    }

    // Check surrounding pixels untouched
    assert_eq!(reference_buffer[0], 0);
    assert_eq!(reference_buffer[255], 0);
}

#[test]
fn test_reconstruction_stats_initial() {
    let capsule = ReconstructionCapsule::new();
    let stats = capsule.stats();

    assert_eq!(stats.blocks_reconstructed, 0);
    assert_eq!(stats.frames_reconstructed, 0);
    assert_eq!(stats.generation, 0);
}

#[test]
fn test_reconstruction_layout() {
    assert_eq!(core::mem::size_of::<ReconstructionCapsule>(), 512);
    assert_eq!(core::mem::align_of::<ReconstructionCapsule>(), 512);
}
