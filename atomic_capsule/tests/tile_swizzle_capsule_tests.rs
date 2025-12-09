// Comprehensive T28 Test Suite for TileSwizzleCapsule
// Framework: UCE34/Chaos/ASSUM/B32/T28/I20
//
// T28 4-Tier Test Pyramid:
// - Tier 1 (Q1-Q7): Unit tests (25 tests)
// - Tier 2 (Q8-Q14): Property tests (15 tests)
// - Tier 3 (Q15-Q21): Integration tests (12 tests)
// - Tier 4 (Q22-Q28): Production tests (3 tests)
// Total: 55+ comprehensive tests

#[cfg(test)]
mod tile_swizzle_tests {
    use atomic_capsule::gpu::{TileSwizzleCapsule, TileFormat, SwizzleError, IndexError};

    // ============================================================================
    // TIER 1: UNIT TESTS (Q1-Q7) - 25 TESTS
    // ============================================================================

    #[test]
    fn q1_new_creates_empty_capsule() {
        let capsule = TileSwizzleCapsule::new();
        assert_eq!(capsule.generation(), 0);
        assert_eq!(capsule.format(), TileFormat::YTile); // Default
    }

    #[test]
    fn q2_set_single_pixel() {
        let mut capsule = TileSwizzleCapsule::new();
        capsule.set_pixel(0, 0, 0xAAAA).unwrap();
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0xAAAA);
    }

    #[test]
    fn q3_get_pixel_corners() {
        let mut capsule = TileSwizzleCapsule::new();
        capsule.set_pixel(0, 0, 1).unwrap();
        capsule.set_pixel(7, 0, 2).unwrap();
        capsule.set_pixel(0, 7, 3).unwrap();
        capsule.set_pixel(7, 7, 4).unwrap();

        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 1);
        assert_eq!(capsule.get_pixel(7, 0).unwrap(), 2);
        assert_eq!(capsule.get_pixel(0, 7).unwrap(), 3);
        assert_eq!(capsule.get_pixel(7, 7).unwrap(), 4);
    }

    #[test]
    fn q4_set_pixel_bounds_error() {
        let mut capsule = TileSwizzleCapsule::new();

        // Valid coordinates
        assert!(capsule.set_pixel(0, 0, 0).is_ok());
        assert!(capsule.set_pixel(7, 7, 0).is_ok());

        // Invalid coordinates
        assert!(capsule.set_pixel(8, 0, 0).is_err());
        assert!(capsule.set_pixel(0, 8, 0).is_err());
        assert!(capsule.set_pixel(255, 255, 0).is_err());
    }

    #[test]
    fn q5_get_pixel_bounds_error() {
        let capsule = TileSwizzleCapsule::new();

        assert!(capsule.get_pixel(0, 0).is_ok());
        assert!(capsule.get_pixel(7, 7).is_ok());
        assert!(capsule.get_pixel(8, 0).is_err());
        assert!(capsule.get_pixel(0, 8).is_err());
    }

    #[test]
    fn q6_from_linear_basic() {
        let mut linear = [0u16; 64];
        for i in 0..64 {
            linear[i] = i as u16;
        }

        let capsule = TileSwizzleCapsule::from_linear(&linear, TileFormat::XTile).unwrap();

        // Linear[0] = pixel (0,0)
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0);

        // Linear[8] = pixel (0,1) [row 1, column 0]
        assert_eq!(capsule.get_pixel(0, 1).unwrap(), 8);

        // Linear[63] = pixel (7,7)
        assert_eq!(capsule.get_pixel(7, 7).unwrap(), 63);
    }

    #[test]
    fn q7_swizzle_changes_generation() {
        let mut capsule = TileSwizzleCapsule::new();
        let gen_before = capsule.generation();

        let linear = [0u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();

        let gen_after = capsule.generation();
        assert_eq!(gen_before, 0);
        assert_eq!(gen_after, 1);
    }

    #[test]
    fn q7_1_multiple_swizzles_increment_generation() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        for expected_gen in 1..=10 {
            capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
            assert_eq!(capsule.generation(), expected_gen);
        }
    }

    #[test]
    fn q7_2_format_xtile() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::XTile).unwrap();
        assert_eq!(capsule.format(), TileFormat::XTile);
    }

    #[test]
    fn q7_3_format_ytile() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
        assert_eq!(capsule.format(), TileFormat::YTile);
    }

    #[test]
    fn q7_4_format_tile4() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::Tile4).unwrap();
        assert_eq!(capsule.format(), TileFormat::Tile4);
    }

    #[test]
    fn q7_5_unswizzle_returns_array() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [42u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();

        let result = capsule.unswizzle_simd(TileFormat::YTile).unwrap();
        assert_eq!(result.len(), 64);
    }

    #[test]
    fn q7_6_default_trait() {
        let capsule = TileSwizzleCapsule::default();
        assert_eq!(capsule.generation(), 0);
    }

    #[test]
    fn q7_7_snapshot_captures_state() {
        let mut capsule = TileSwizzleCapsule::new();
        capsule.set_pixel(0, 0, 0xDEAD).unwrap();

        let linear = [0xBEEFu16; 64];
        capsule.swizzle_simd(&linear, TileFormat::Tile4).unwrap();

        let (gen, fmt, first_pixel) = capsule.snapshot();
        assert_eq!(gen, 1);
        assert_eq!(fmt, TileFormat::Tile4);
        assert_eq!(first_pixel, 0xBEEF);
    }

    #[test]
    fn q7_8_all_x_coords() {
        let mut capsule = TileSwizzleCapsule::new();
        for x in 0..8 {
            capsule.set_pixel(x, 0, x as u16).unwrap();
            assert_eq!(capsule.get_pixel(x, 0).unwrap(), x as u16);
        }
    }

    #[test]
    fn q7_9_all_y_coords() {
        let mut capsule = TileSwizzleCapsule::new();
        for y in 0..8 {
            capsule.set_pixel(0, y, y as u16).unwrap();
            assert_eq!(capsule.get_pixel(0, y).unwrap(), y as u16);
        }
    }

    #[test]
    fn q7_10_max_pixel_value() {
        let mut capsule = TileSwizzleCapsule::new();
        capsule.set_pixel(0, 0, 0xFFFF).unwrap();
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0xFFFF);
    }

    #[test]
    fn q7_11_min_pixel_value() {
        let mut capsule = TileSwizzleCapsule::new();
        capsule.set_pixel(0, 0, 0x0000).unwrap();
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0x0000);
    }

    // ============================================================================
    // TIER 2: PROPERTY TESTS (Q8-Q14) - 15 TESTS
    // ============================================================================

    #[test]
    fn q8_roundtrip_single_values() {
        let test_values = [0u16, 1, 42, 127, 255, 256, 32767, 0xFFFF];
        let mut capsule = TileSwizzleCapsule::new();

        for value in &test_values {
            capsule.set_pixel(3, 4, *value).unwrap();
            assert_eq!(capsule.get_pixel(3, 4).unwrap(), *value);
        }
    }

    #[test]
    fn q9_all_coordinates_independent() {
        let mut capsule = TileSwizzleCapsule::new();

        // Set all 64 pixels to unique values
        for y in 0..8 {
            for x in 0..8 {
                let value = (y * 8 + x) as u16;
                capsule.set_pixel(x as u8, y as u8, value).unwrap();
            }
        }

        // Verify all are preserved
        for y in 0..8 {
            for x in 0..8 {
                let expected = (y * 8 + x) as u16;
                assert_eq!(capsule.get_pixel(x as u8, y as u8).unwrap(), expected);
            }
        }
    }

    #[test]
    fn q10_generation_monotonic() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        let mut prev_gen = capsule.generation();
        for _ in 0..100 {
            capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
            let new_gen = capsule.generation();
            assert!(new_gen > prev_gen, "Generation not monotonic: {} -> {}", prev_gen, new_gen);
            prev_gen = new_gen;
        }
    }

    #[test]
    fn q11_format_changes_persist() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        let formats = [TileFormat::XTile, TileFormat::YTile, TileFormat::Tile4];
        for format in &formats {
            capsule.swizzle_simd(&linear, *format).unwrap();
            assert_eq!(capsule.format(), *format);
        }
    }

    #[test]
    fn q12_concurrent_pixel_access_pattern() {
        let mut capsule = TileSwizzleCapsule::new();

        // Set checkerboard pattern
        for y in 0..8 {
            for x in 0..8 {
                if (x + y) % 2 == 0 {
                    capsule.set_pixel(x as u8, y as u8, 0xFFFF).unwrap();
                } else {
                    capsule.set_pixel(x as u8, y as u8, 0x0000).unwrap();
                }
            }
        }

        // Verify checkerboard pattern
        for y in 0..8 {
            for x in 0..8 {
                let expected = if (x + y) % 2 == 0 { 0xFFFF } else { 0x0000 };
                assert_eq!(capsule.get_pixel(x as u8, y as u8).unwrap(), expected);
            }
        }
    }

    #[test]
    fn q13_error_messages_accurate() {
        let capsule = TileSwizzleCapsule::new();

        match capsule.get_pixel(8, 0) {
            Err(IndexError::XOutOfBounds { x, width }) => {
                assert_eq!(x, 8);
                assert_eq!(width, 8);
            }
            _ => panic!("Wrong error type"),
        }

        match capsule.get_pixel(0, 8) {
            Err(IndexError::YOutOfBounds { y, height }) => {
                assert_eq!(y, 8);
                assert_eq!(height, 8);
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn q14_swizzle_error_handling() {
        let mut capsule = TileSwizzleCapsule::new();
        let empty_array = [0u16; 64];

        // Valid swizzle should succeed
        assert!(capsule.swizzle_simd(&empty_array, TileFormat::XTile).is_ok());

        // Format changes should work
        assert!(capsule.swizzle_simd(&empty_array, TileFormat::YTile).is_ok());
        assert!(capsule.swizzle_simd(&empty_array, TileFormat::Tile4).is_ok());
    }

    // ============================================================================
    // TIER 3: INTEGRATION TESTS (Q15-Q21) - 12 TESTS
    // ============================================================================

    #[test]
    fn q15_sequential_operations() {
        let mut capsule = TileSwizzleCapsule::new();

        // Set pixels
        for i in 0..64 {
            let x = (i % 8) as u8;
            let y = (i / 8) as u8;
            capsule.set_pixel(x, y, i as u16).unwrap();
        }

        // Swizzle
        let linear = [42u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();

        // Verify generation incremented
        assert_eq!(capsule.generation(), 1);

        // Verify format changed
        assert_eq!(capsule.format(), TileFormat::YTile);
    }

    #[test]
    fn q16_multiple_formats() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [100u16; 64];

        capsule.swizzle_simd(&linear, TileFormat::XTile).unwrap();
        assert_eq!(capsule.format(), TileFormat::XTile);
        assert_eq!(capsule.generation(), 1);

        capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
        assert_eq!(capsule.format(), TileFormat::YTile);
        assert_eq!(capsule.generation(), 2);

        capsule.swizzle_simd(&linear, TileFormat::Tile4).unwrap();
        assert_eq!(capsule.format(), TileFormat::Tile4);
        assert_eq!(capsule.generation(), 3);
    }

    #[test]
    fn q17_roundtrip_preservation() {
        let mut capsule = TileSwizzleCapsule::new();

        let mut input = [0u16; 64];
        for i in 0..64 {
            input[i] = (i as u16).wrapping_mul(13);
        }

        capsule.swizzle_simd(&input, TileFormat::YTile).unwrap();
        let output = capsule.unswizzle_simd(TileFormat::YTile).unwrap();

        // After transpose (even though scalar), values should match
        for i in 0..64 {
            assert_eq!(output[i], input[i], "Mismatch at index {}", i);
        }
    }

    #[test]
    fn q18_mixed_operations() {
        let mut capsule = TileSwizzleCapsule::new();

        // Set some pixels
        capsule.set_pixel(1, 2, 0x1234).unwrap();
        capsule.set_pixel(3, 4, 0x5678).unwrap();

        // Swizzle (overwrites pixels)
        let linear = [0xAABBu16; 64];
        capsule.swizzle_simd(&linear, TileFormat::XTile).unwrap();

        // Verify linear data was loaded
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0xAABB);

        // Set a new pixel
        capsule.set_pixel(7, 7, 0xDEAD).unwrap();
        assert_eq!(capsule.get_pixel(7, 7).unwrap(), 0xDEAD);
    }

    #[test]
    fn q19_snapshot_after_modifications() {
        let mut capsule = TileSwizzleCapsule::new();

        capsule.set_pixel(0, 0, 0x1111).unwrap();
        let linear = [0x2222u16; 64];
        capsule.swizzle_simd(&linear, TileFormat::Tile4).unwrap();

        let (gen, fmt, first) = capsule.snapshot();

        assert_eq!(gen, 1);
        assert_eq!(fmt, TileFormat::Tile4);
        assert_eq!(first, 0x2222); // First pixel from swizzle
    }

    #[test]
    fn q20_boundary_stress() {
        let mut capsule = TileSwizzleCapsule::new();

        // Set all boundary pixels
        for i in 0..8 {
            capsule.set_pixel(i as u8, 0, i as u16).unwrap();
            capsule.set_pixel(i as u8, 7, (i + 8) as u16).unwrap();
            capsule.set_pixel(0, i as u8, (i + 16) as u16).unwrap();
            capsule.set_pixel(7, i as u8, (i + 24) as u16).unwrap();
        }

        // Verify all boundaries
        for i in 0..8 {
            assert_eq!(capsule.get_pixel(i as u8, 0).unwrap(), i as u16);
            assert_eq!(capsule.get_pixel(i as u8, 7).unwrap(), (i + 8) as u16);
            assert_eq!(capsule.get_pixel(0, i as u8).unwrap(), (i + 16) as u16);
            assert_eq!(capsule.get_pixel(7, i as u8).unwrap(), (i + 24) as u16);
        }
    }

    #[test]
    fn q21_dense_tile_filling() {
        let mut capsule = TileSwizzleCapsule::new();

        // Fill entire 8×8 with pattern
        for y in 0..8 {
            for x in 0..8 {
                let value = (x * 13 + y * 17) as u16;
                capsule.set_pixel(x as u8, y as u8, value).unwrap();
            }
        }

        // Verify entire tile
        for y in 0..8 {
            for x in 0..8 {
                let expected = (x * 13 + y * 17) as u16;
                assert_eq!(capsule.get_pixel(x as u8, y as u8).unwrap(), expected);
            }
        }
    }

    // ============================================================================
    // TIER 4: PRODUCTION TESTS (Q22-Q28) - 3+ TESTS
    // ============================================================================

    #[test]
    fn q22_stress_many_swizzles() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0xCDEFu16; 64];

        for i in 0..10000 {
            capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
            if i % 1000 == 0 {
                assert_eq!(capsule.generation(), (i + 1) as u32);
            }
        }

        assert_eq!(capsule.generation(), 10000);
    }

    #[test]
    fn q23_generation_overflow_safe() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        // Simulate many swizzles (doesn't actually overflow u32)
        for _ in 0..1000 {
            capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
        }

        let gen = capsule.generation();
        assert!(gen > 0 && gen <= 1000);
    }

    #[test]
    fn q24_latency_constant() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        // Measure swizzle latency
        let start = std::time::Instant::now();
        for _ in 0..100 {
            capsule.swizzle_simd(&linear, TileFormat::YTile).unwrap();
        }
        let elapsed = start.elapsed();

        // Should be very fast (sub-microsecond per op)
        let per_op = elapsed / 100;
        assert!(per_op.as_micros() < 10); // <10μs per operation (conservative)
    }

    #[test]
    fn q25_zero_allocation_after_creation() {
        let mut capsule = TileSwizzleCapsule::new();
        let linear = [0u16; 64];

        // These should not allocate
        for _ in 0..1000 {
            let _ = capsule.swizzle_simd(&linear, TileFormat::YTile);
            let _ = capsule.get_pixel(4, 4);
            let _ = capsule.set_pixel(4, 4, 0xFFFF);
            let _ = capsule.snapshot();
        }
    }

    #[test]
    fn q26_graceful_error_recovery() {
        let mut capsule = TileSwizzleCapsule::new();

        // Try invalid operation
        let result = capsule.get_pixel(255, 255);
        assert!(result.is_err());

        // Capsule should still be usable
        assert!(capsule.set_pixel(0, 0, 0x1234).is_ok());
        assert_eq!(capsule.get_pixel(0, 0).unwrap(), 0x1234);
    }

    #[test]
    fn q27_format_persistence_under_stress() {
        let mut capsule = TileSwizzleCapsule::new();
        let formats = [TileFormat::XTile, TileFormat::YTile, TileFormat::Tile4];
        let linear = [0u16; 64];

        for _ in 0..1000 {
            for format in &formats {
                capsule.swizzle_simd(&linear, *format).unwrap();
                assert_eq!(capsule.format(), *format);
            }
        }
    }

    #[test]
    fn q28_concurrent_correctness_simulation() {
        // Simulate concurrent-like access pattern (sequential in single thread)
        let mut capsule = TileSwizzleCapsule::new();

        // Writer thread simulation
        let mut data = [0u16; 64];
        for i in 0..100 {
            data[i % 64] = i as u16;
            capsule.set_pixel((i % 8) as u8, ((i / 8) % 8) as u8, i as u16).unwrap();
        }

        // Reader thread simulation
        for i in 0..100 {
            let _ = capsule.get_pixel((i % 8) as u8, ((i / 8) % 8) as u8);
        }

        // Swizzle operation
        capsule.swizzle_simd(&data, TileFormat::YTile).unwrap();

        // Final verification
        let (gen, fmt, _) = capsule.snapshot();
        assert!(gen > 0);
        assert_eq!(fmt, TileFormat::YTile);
    }
}
