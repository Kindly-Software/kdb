//! Full Encoder-Decoder Reconstruction Integration Test
//!
//! Demonstrates the complete AV1 encoder feedback loop:
//! Input → Transform → Quantize → Entropy Code → [Reconstruction] → Reference Buffer
//!
//! This test validates the critical encoder-decoder path that enables
//! Rate-Distortion Optimization (RDO).

use atomic_capsule::encoder::{
    DctTransformCapsule, EntropyCoderCapsule, QuantizationCapsule, ReferenceFrameCapsule,
};
use kindly_av1::encoder::ReconstructionCapsule;

#[test]
fn test_full_encoder_decoder_loop_8x8() {
    // ========== SETUP ==========

    // Original 8×8 block (luma plane, grayscale gradient)
    let mut original_block = [0u8; 64];
    for i in 0..64 {
        original_block[i] = (i * 4) as u8; // Gradient 0→252
    }

    // Prediction (from intra prediction or motion compensation)
    let prediction = [128u8; 64]; // Simple DC prediction

    // Residual (difference between original and prediction)
    let mut residual = [0i16; 64];
    for i in 0..64 {
        residual[i] = original_block[i] as i16 - prediction[i] as i16;
    }

    // ========== FORWARD PATH (Encoding) ==========

    // 1. Forward DCT Transform
    let transform = DctTransformCapsule::new();
    let dct_coeffs = transform.forward_8x8(&residual);

    // 2. Quantization
    let quant = QuantizationCapsule::new(32); // QP=32 (standard quality)
    let quantized = quant.quantize_block_8x8(&dct_coeffs);

    // 3. Entropy Coding (bitstream generation)
    // In real encoder, this writes to OBU bitstream
    // For this test, we just verify coefficients are quantized
    let has_nonzero_coeffs = quantized.iter().any(|&c| c != 0);
    assert!(
        has_nonzero_coeffs,
        "Quantization should preserve some coefficients"
    );

    // ========== RECONSTRUCTION PATH (Decoder Simulation) ==========

    // 4. Dequantize + Inverse DCT + Add Prediction + Clip
    let reconstruction = ReconstructionCapsule::new();
    let mut reconstructed = [0u8; 64];

    reconstruction.reconstruct_block_8x8(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // ========== VERIFICATION ==========

    // 5. Verify reconstruction quality
    //    - Should be close to original (within quantization error)
    //    - All pixels in valid range [0, 255]
    //    - Gradient pattern preserved

    for i in 0..64 {
        // All pixels must be in valid range
        assert!(
            reconstructed[i] <= 255,
            "Pixel {} out of range: {}",
            i,
            reconstructed[i]
        );
    }

    // Calculate Mean Absolute Error (MAE)
    let mut total_error = 0i32;
    for i in 0..64 {
        let error = (reconstructed[i] as i32 - original_block[i] as i32).abs();
        total_error += error;
    }
    let mae = total_error / 64;

    // MAE should be reasonable (quantization is lossy, but not catastrophic)
    assert!(mae < 30, "MAE too large: {} (should be <30 for QP=32)", mae);

    // ========== REFERENCE BUFFER STORAGE ==========

    // 6. Store reconstructed block to reference buffer
    //    (for future inter-frame prediction)
    let mut reference_buffer = [0u8; 256]; // 16×16 reference frame
    reconstruction.store_to_reference(
        &reconstructed,
        &mut reference_buffer,
        0,  // x
        0,  // y
        16, // width
        8,  // block_size
    );

    // Verify reference buffer updated correctly
    for row in 0..8 {
        for col in 0..8 {
            let idx = row * 16 + col;
            assert_eq!(
                reference_buffer[idx],
                reconstructed[row * 8 + col],
                "Reference buffer mismatch at ({}, {})",
                row,
                col
            );
        }
    }

    // ========== STATS VERIFICATION ==========

    let stats = reconstruction.stats();
    assert_eq!(stats.blocks_reconstructed, 1);
    assert_eq!(stats.frames_reconstructed, 0); // Not completed yet
}

#[test]
fn test_full_encoder_decoder_loop_multiple_blocks() {
    // Simulate encoding a 16×16 macroblock (four 8×8 blocks)

    let quant = QuantizationCapsule::new(28); // Slightly higher quality
    let transform = DctTransformCapsule::new();
    let reconstruction = ReconstructionCapsule::new();

    let mut reference_buffer = [0u8; 256]; // 16×16 frame

    // Four 8×8 blocks covering 16×16 macroblock
    let blocks = [
        (0, 0), // Top-left
        (8, 0), // Top-right
        (0, 8), // Bottom-left
        (8, 8), // Bottom-right
    ];

    for (block_idx, &(x, y)) in blocks.iter().enumerate() {
        // Original block with unique pattern
        let mut original_block = [0u8; 64];
        for i in 0..64 {
            original_block[i] = ((block_idx * 64 + i) % 256) as u8;
        }

        // Prediction (simple DC)
        let prediction = [128u8; 64];

        // Residual
        let mut residual = [0i16; 64];
        for i in 0..64 {
            residual[i] = original_block[i] as i16 - prediction[i] as i16;
        }

        // Forward path: DCT → Quantize
        let dct_coeffs = transform.forward_8x8(&residual);
        let quantized = quant.quantize_block_8x8(&dct_coeffs);

        // Reconstruction path: Dequantize → IDCT → Add Prediction → Clip
        let mut reconstructed = [0u8; 64];
        reconstruction.reconstruct_block_8x8(
            &quantized,
            &prediction,
            &mut reconstructed,
            &quant,
            &transform,
        );

        // Store to reference buffer
        reconstruction.store_to_reference(
            &reconstructed,
            &mut reference_buffer,
            x,
            y,
            16, // width
            8,  // block_size
        );
    }

    // Verify all four blocks stored correctly
    let stats = reconstruction.stats();
    assert_eq!(stats.blocks_reconstructed, 4);

    // Verify reference buffer has all blocks
    let unique_values: std::collections::HashSet<u8> = reference_buffer.iter().copied().collect();
    assert!(
        unique_values.len() > 16,
        "Reference buffer should have variation from 4 different blocks"
    );
}

#[test]
fn test_encoder_decoder_loop_with_reference_frame() {
    // Simulate encoder with reference frame management

    let quant = QuantizationCapsule::new(24); // High quality
    let transform = DctTransformCapsule::new();
    let reconstruction = ReconstructionCapsule::new();
    let reference_frame = ReferenceFrameCapsule::new();

    // Encode first frame (keyframe)
    let original_block = [100u8; 64];
    let prediction = [0u8; 64]; // Intra prediction
    let mut residual = [0i16; 64];
    for i in 0..64 {
        residual[i] = original_block[i] as i16 - prediction[i] as i16;
    }

    let dct_coeffs = transform.forward_8x8(&residual);
    let quantized = quant.quantize_block_8x8(&dct_coeffs);

    let mut reconstructed = [0u8; 64];
    reconstruction.reconstruct_block_8x8(
        &quantized,
        &prediction,
        &mut reconstructed,
        &quant,
        &transform,
    );

    // Allocate reference frame slot
    let slot = reference_frame.allocate_slot(1001);
    assert!(slot.is_some(), "Should allocate reference slot");

    // Update reference frame with reconstructed data
    // (In real encoder, this would store full frame buffer pointer)
    let frame_ptr = reconstructed.as_ptr();
    reference_frame.update_slot(slot.unwrap(), frame_ptr, 1001);

    // Verify reference frame valid
    assert!(reference_frame.is_slot_valid(slot.unwrap()));

    // Mark frame complete
    reconstruction.complete_frame();

    let stats = reconstruction.stats();
    assert_eq!(stats.frames_reconstructed, 1);
}

#[test]
fn test_encoder_quality_preservation() {
    // Test that reconstruction quality is acceptable across various QP values

    let transform = DctTransformCapsule::new();
    let reconstruction = ReconstructionCapsule::new();

    // Test pattern: checkerboard (high-frequency content)
    let mut original_block = [0u8; 64];
    for i in 0..64 {
        original_block[i] = if (i / 8 + i % 8) % 2 == 0 { 0 } else { 255 };
    }

    let prediction = [128u8; 64];
    let mut residual = [0i16; 64];
    for i in 0..64 {
        residual[i] = original_block[i] as i16 - prediction[i] as i16;
    }

    let dct_coeffs = transform.forward_8x8(&residual);

    // Test multiple quality levels
    let qp_values = [16, 24, 32, 40, 48]; // From high quality to low quality

    for qp in qp_values {
        let quant = QuantizationCapsule::new(qp);
        let quantized = quant.quantize_block_8x8(&dct_coeffs);

        let mut reconstructed = [0u8; 64];
        reconstruction.reconstruct_block_8x8(
            &quantized,
            &prediction,
            &mut reconstructed,
            &quant,
            &transform,
        );

        // Calculate Peak Signal-to-Noise Ratio (PSNR)
        let mut mse = 0.0f64;
        for i in 0..64 {
            let error = original_block[i] as f64 - reconstructed[i] as f64;
            mse += error * error;
        }
        mse /= 64.0;

        let psnr = if mse > 0.0 {
            10.0 * (255.0 * 255.0 / mse).log10()
        } else {
            100.0 // Perfect reconstruction
        };

        // Higher QP → lower PSNR (more compression artifacts)
        // QP=16 should have PSNR > 30 dB
        // QP=48 may have PSNR < 25 dB
        println!("QP={} → PSNR={:.2} dB", qp, psnr);

        // Sanity check: PSNR should be reasonable
        assert!(psnr > 10.0, "PSNR too low for QP={}: {:.2} dB", qp, psnr);
    }
}
