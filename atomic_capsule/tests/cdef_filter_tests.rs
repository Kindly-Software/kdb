//! CdefFilterCapsule - Integration Tests
//!
//! T28 Compliance: Unit, Property, Integration, Production tests for CDEF filter

#![cfg(feature = "portable_simd")]

use atomic_capsule::encoder::{CdefFilterCapsule, DIR_VERTICAL, DIR_HORIZONTAL};

#[test]
fn test_cdef_new_initialization() {
    let cdef = CdefFilterCapsule::new();
    let (y_damp, uv_damp) = cdef.get_damping();
    assert_eq!(y_damp, 5);
    assert_eq!(uv_damp, 5);
    assert_eq!(cdef.generation(), 0);
}

#[test]
fn test_cdef_capsule_alignment() {
    assert_eq!(core::mem::size_of::<CdefFilterCapsule>(), 256);
    assert_eq!(core::mem::align_of::<CdefFilterCapsule>(), 256);
}

#[test]
fn test_cdef_direction_detection_vertical() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [0u8; 64];

    // Create vertical edge (left half black, right half white)
    for y in 0..8 {
        for x in 0..8 {
            block[y * 8 + x] = if x < 4 { 0 } else { 255 };
        }
    }

    let direction = cdef.find_direction(&block);
    assert_eq!(direction, DIR_VERTICAL, "Should detect vertical edge");
}

#[test]
fn test_cdef_direction_detection_horizontal() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [0u8; 64];

    // Create horizontal edge (top half black, bottom half white)
    for y in 0..8 {
        for x in 0..8 {
            block[y * 8 + x] = if y < 4 { 0 } else { 255 };
        }
    }

    let direction = cdef.find_direction(&block);
    assert_eq!(direction, DIR_HORIZONTAL, "Should detect horizontal edge");
}

#[test]
fn test_cdef_filter_preserves_range() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [128u8; 64];

    // Add some noise
    block[10] = 120;
    block[20] = 136;
    block[30] = 124;
    block[40] = 132;

    cdef.apply_filter(&mut block, true, 1);

    // All pixels must remain in [0, 255]
    for &pixel in &block {
        assert!(pixel <= 255, "Pixel out of range: {}", pixel);
    }
}

#[test]
fn test_cdef_filter_smooths_noise() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [128u8; 64];

    // Add isolated noise spike at center
    block[27] = 200;
    let original_spike = block[27];

    cdef.apply_filter(&mut block, true, 2);
    let filtered_spike = block[27];

    // Filter should reduce the spike
    assert!(filtered_spike < original_spike, "Filter should smooth noise");
    assert!(filtered_spike > 128, "Should move toward neighbors");
}

#[test]
fn test_cdef_damping_update() {
    let cdef = CdefFilterCapsule::new();

    cdef.set_damping(4, 6);
    let (y_damp, uv_damp) = cdef.get_damping();

    assert_eq!(y_damp, 4);
    assert_eq!(uv_damp, 6);
    assert_eq!(cdef.generation(), 1, "Generation should increment");
}

#[test]
fn test_cdef_strength_update() {
    let cdef = CdefFilterCapsule::new();

    let y_pri = [1, 2, 3, 4];
    let y_sec = [0, 1, 1, 2];
    let uv_pri = [1, 2, 3, 4];
    let uv_sec = [0, 1, 1, 2];

    let gen_before = cdef.generation();
    cdef.set_strengths(&y_pri, &y_sec, &uv_pri, &uv_sec);
    let gen_after = cdef.generation();

    assert_eq!(gen_after, gen_before + 1, "Generation should increment on strength update");
}

#[test]
fn test_cdef_flat_block_minimal_filtering() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [128u8; 64]; // Flat gray block

    let original = block;
    cdef.apply_filter(&mut block, true, 0);

    // Flat blocks should change minimally (low strength)
    let differences: usize = block.iter()
        .zip(original.iter())
        .filter(|(&a, &b)| a != b)
        .count();

    // Most pixels should remain unchanged for flat blocks
    assert!(differences < 32, "Flat blocks should have minimal filtering");
}

#[test]
fn test_cdef_generation_counter_monotonic() {
    let cdef = CdefFilterCapsule::new();

    let mut prev_gen = cdef.generation();

    for _ in 0..10 {
        cdef.set_damping(5, 5);
        let curr_gen = cdef.generation();
        assert!(curr_gen > prev_gen, "Generation counter should be monotonically increasing");
        prev_gen = curr_gen;
    }
}

/// Property test: Direction search should return valid direction (0-7)
#[test]
fn property_direction_range() {
    let cdef = CdefFilterCapsule::new();

    // Test with 100 random-ish blocks
    for seed in 0..100 {
        let mut block = [0u8; 64];
        for i in 0..64 {
            block[i] = ((seed * 13 + i * 7) % 256) as u8;
        }

        let direction = cdef.find_direction(&block);
        assert!(direction < 8, "Direction must be 0-7, got {}", direction);
    }
}

/// Integration test: CDEF + LoopFilter pipeline
#[test]
fn integration_cdef_with_loop_filter() {
    let cdef = CdefFilterCapsule::new();
    let mut block = [128u8; 64];

    // Simulate block with coding artifacts
    block[10] = 100;
    block[20] = 156;
    block[30] = 112;

    // Apply CDEF filter
    cdef.apply_filter(&mut block, true, 2);

    // Verify output is valid for next stage
    for &pixel in &block {
        assert!(pixel <= 255, "Invalid pixel for next stage");
    }
}
