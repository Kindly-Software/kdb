//! Shader Binding Table Capsule Tests - T28 5-Tier Testing
//!
//! # Test Coverage
//!
//! - Q1-Q7: Unit tests (regions, alignment, layout)
//! - Q8-Q14: Property tests (handle copy, updates, multi-geometry)
//! - Q15-Q21: Integration tests (full SBT workflow)
//! - Q22-Q28: Production tests (stress, edge cases)

use atomic_capsule::gpu::graphics::shader_binding_table::{
    ShaderBindingTableCapsule, SbtRegion, StridedRegion,
};

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn q1_unit_size_alignment() {
    assert_eq!(
        core::mem::size_of::<ShaderBindingTableCapsule>(),
        512,
        "SBT capsule must be 512 bytes"
    );
    assert_eq!(
        core::mem::align_of::<ShaderBindingTableCapsule>(),
        512,
        "SBT capsule must be 512-byte aligned"
    );
}

#[test]
fn q2_unit_new_typical() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // Verify Vulkan spec requirements
    assert_eq!(sbt.shader_group_handle_size, 32, "Handle size must be 32 bytes per spec");
    assert_eq!(sbt.shader_group_handle_alignment, 32, "Typical stride alignment");
    assert_eq!(sbt.shader_group_base_alignment, 64, "Typical base alignment");
    assert_eq!(sbt.max_shader_group_stride, 4096, "Typical max stride");

    // Verify initial state
    let (gen, updates, binds) = sbt.stats();
    assert_eq!(gen, 0);
    assert_eq!(updates, 0);
    assert_eq!(binds, 0);
}

#[test]
fn q3_unit_calculate_stride() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // No user data: 32 bytes (handle only) -> stride 32
    assert_eq!(sbt.calculate_stride(0), 32);

    // 1 byte user data: 33 bytes -> stride 64 (next 32-byte multiple)
    assert_eq!(sbt.calculate_stride(1), 64);

    // 16 bytes user data: 48 bytes -> stride 64
    assert_eq!(sbt.calculate_stride(16), 64);

    // 32 bytes user data: 64 bytes -> stride 64
    assert_eq!(sbt.calculate_stride(32), 64);

    // 33 bytes user data: 65 bytes -> stride 96
    assert_eq!(sbt.calculate_stride(33), 96);

    // 48 bytes user data: 80 bytes -> stride 96
    assert_eq!(sbt.calculate_stride(48), 96);

    // 64 bytes user data: 96 bytes -> stride 96
    assert_eq!(sbt.calculate_stride(64), 96);

    // Large user data: 128 bytes: 160 bytes -> stride 160
    assert_eq!(sbt.calculate_stride(128), 160);
}

#[test]
fn q4_unit_align_offset() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // Already aligned to 64
    assert_eq!(sbt.align_offset(0), 0);
    assert_eq!(sbt.align_offset(64), 64);
    assert_eq!(sbt.align_offset(128), 128);
    assert_eq!(sbt.align_offset(192), 192);

    // Need alignment
    assert_eq!(sbt.align_offset(1), 64);
    assert_eq!(sbt.align_offset(32), 64);
    assert_eq!(sbt.align_offset(63), 64);
    assert_eq!(sbt.align_offset(65), 128);
    assert_eq!(sbt.align_offset(127), 128);
    assert_eq!(sbt.align_offset(129), 192);
}

#[test]
fn q5_unit_strided_region() {
    let region = StridedRegion {
        device_address: 0x1000,
        stride: 64,
        size: 192,
    };

    assert!(!region.is_empty());
    assert_eq!(region.entry_count(), 3);
    assert_eq!(region.entry_address(0), 0x1000);
    assert_eq!(region.entry_address(1), 0x1040); // 0x1000 + 64
    assert_eq!(region.entry_address(2), 0x1080); // 0x1000 + 128

    // Empty region
    let empty = StridedRegion::empty();
    assert!(empty.is_empty());
    assert_eq!(empty.entry_count(), 0);
    assert_eq!(empty.size, 0);
}

#[test]
fn q6_unit_entry_count() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 4, 32);

    assert_eq!(sbt.entry_count(SbtRegion::RayGen), 1);
    assert_eq!(sbt.entry_count(SbtRegion::Miss), 2);
    assert_eq!(sbt.entry_count(SbtRegion::HitGroup), 3);
    assert_eq!(sbt.entry_count(SbtRegion::Callable), 4);
}

#[test]
fn q7_unit_stats_tracking() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();

    // Initial stats
    let (gen, updates, binds) = sbt.stats();
    assert_eq!(gen, 0);
    assert_eq!(updates, 0);
    assert_eq!(binds, 0);

    // Build layout increments generation
    sbt.build_layout(0x1000_0000, 1, 0, 0, 0, 0, 0, 0, 0);
    let (gen, updates, binds) = sbt.stats();
    assert_eq!(gen, 1);
    assert_eq!(updates, 0);
    assert_eq!(binds, 0);

    // Record updates
    sbt.record_update();
    sbt.record_update();
    sbt.record_update();
    let (gen, updates, binds) = sbt.stats();
    assert_eq!(gen, 1);
    assert_eq!(updates, 3);
    assert_eq!(binds, 0);

    // Record binds
    sbt.record_bind();
    sbt.record_bind();
    let (gen, updates, binds) = sbt.stats();
    assert_eq!(gen, 1);
    assert_eq!(updates, 3);
    assert_eq!(binds, 2);
}

// ============================================================================
// Q8-Q14: Property Tests
// ============================================================================

#[test]
fn q8_property_alignment_invariants() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // Test multiple user data sizes
    for user_data_size in [0, 1, 16, 32, 48, 64, 128, 256] {
        let stride = sbt.calculate_stride(user_data_size);

        // Stride must be multiple of handle alignment
        assert_eq!(
            stride % sbt.shader_group_handle_alignment as u64,
            0,
            "Stride {} must be multiple of handle alignment {} for user_data_size {}",
            stride,
            sbt.shader_group_handle_alignment,
            user_data_size
        );

        // Stride must be at least handle size
        assert!(
            stride >= sbt.shader_group_handle_size as u64,
            "Stride {} must be >= handle size {} for user_data_size {}",
            stride,
            sbt.shader_group_handle_size,
            user_data_size
        );

        // Stride must be <= max stride
        assert!(
            stride <= sbt.max_shader_group_stride as u64,
            "Stride {} must be <= max stride {} for user_data_size {}",
            stride,
            sbt.max_shader_group_stride,
            user_data_size
        );
    }
}

#[test]
fn q9_property_offset_alignment() {
    let sbt = ShaderBindingTableCapsule::new_typical();
    let base_align = sbt.shader_group_base_alignment as u64;

    // Test multiple offsets
    for offset in [0, 1, 32, 63, 64, 65, 127, 128, 192, 255, 256, 1000, 4095] {
        let aligned = sbt.align_offset(offset);

        // Must be multiple of base alignment
        assert_eq!(
            aligned % base_align,
            0,
            "Aligned offset {} must be multiple of {} for input {}",
            aligned,
            base_align,
            offset
        );

        // Must be >= input offset
        assert!(
            aligned >= offset,
            "Aligned offset {} must be >= input {} ",
            aligned,
            offset
        );

        // Must be closest multiple
        if offset % base_align != 0 {
            assert!(
                aligned - offset < base_align,
                "Aligned offset {} too far from input {} (diff {}, align {})",
                aligned,
                offset,
                aligned - offset,
                base_align
            );
        }
    }
}

#[test]
fn q10_property_buffer_size_monotonic() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // More shaders -> larger buffer
    let size_1 = sbt.calculate_buffer_size(1, 0, 0, 0, 0, 0, 0, 0);
    let size_2 = sbt.calculate_buffer_size(1, 0, 1, 0, 0, 0, 0, 0);
    let size_3 = sbt.calculate_buffer_size(1, 0, 1, 0, 1, 0, 0, 0);
    let size_4 = sbt.calculate_buffer_size(1, 0, 1, 0, 1, 0, 1, 0);

    assert!(size_2 > size_1, "Adding miss shader should increase size");
    assert!(size_3 > size_2, "Adding hit group should increase size");
    assert!(size_4 > size_3, "Adding callable should increase size");

    // More user data -> larger buffer
    let size_no_data = sbt.calculate_buffer_size(0, 0, 1, 0, 0, 0, 0, 0);
    let size_16_data = sbt.calculate_buffer_size(0, 0, 1, 16, 0, 0, 0, 0);
    let size_32_data = sbt.calculate_buffer_size(0, 0, 1, 32, 0, 0, 0, 0);

    assert!(
        size_16_data >= size_no_data,
        "Adding user data should increase or maintain size"
    );
    assert!(
        size_32_data >= size_16_data,
        "More user data should increase or maintain size"
    );
}

#[test]
fn q11_property_region_non_overlapping() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Build full SBT
    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 4, 32);

    let ray_gen = sbt.ray_gen_region();
    let miss = sbt.miss_region();
    let hit_group = sbt.hit_group_region();
    let callable = sbt.callable_region();

    // Ray gen ends before miss starts
    if !ray_gen.is_empty() && !miss.is_empty() {
        assert!(
            ray_gen.device_address + ray_gen.size <= miss.device_address,
            "Ray gen region overlaps miss region"
        );
    }

    // Miss ends before hit group starts
    if !miss.is_empty() && !hit_group.is_empty() {
        assert!(
            miss.device_address + miss.size <= hit_group.device_address,
            "Miss region overlaps hit group region"
        );
    }

    // Hit group ends before callable starts
    if !hit_group.is_empty() && !callable.is_empty() {
        assert!(
            hit_group.device_address + hit_group.size <= callable.device_address,
            "Hit group region overlaps callable region"
        );
    }
}

#[test]
fn q12_property_entry_addresses_within_region() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 4, 32);

    // Check ray gen entries
    for i in 0..sbt.entry_count(SbtRegion::RayGen) {
        let addr = sbt.entry_address(SbtRegion::RayGen, i).unwrap();
        let region = sbt.ray_gen_region();
        assert!(
            addr >= region.device_address,
            "Entry address before region start"
        );
        assert!(
            addr < region.device_address + region.size,
            "Entry address after region end"
        );
    }

    // Check miss entries
    for i in 0..sbt.entry_count(SbtRegion::Miss) {
        let addr = sbt.entry_address(SbtRegion::Miss, i).unwrap();
        let region = sbt.miss_region();
        assert!(
            addr >= region.device_address,
            "Entry address before region start"
        );
        assert!(
            addr < region.device_address + region.size,
            "Entry address after region end"
        );
    }

    // Check hit group entries
    for i in 0..sbt.entry_count(SbtRegion::HitGroup) {
        let addr = sbt.entry_address(SbtRegion::HitGroup, i).unwrap();
        let region = sbt.hit_group_region();
        assert!(
            addr >= region.device_address,
            "Entry address before region start"
        );
        assert!(
            addr < region.device_address + region.size,
            "Entry address after region end"
        );
    }
}

#[test]
fn q13_property_stats_operations_commute() {
    let mut sbt1 = ShaderBindingTableCapsule::new_typical();
    let mut sbt2 = ShaderBindingTableCapsule::new_typical();

    // Sequence 1: build -> update -> bind
    sbt1.build_layout(0x1000, 1, 0, 0, 0, 0, 0, 0, 0);
    sbt1.record_update();
    sbt1.record_bind();

    // Sequence 2: build -> bind -> update
    sbt2.build_layout(0x1000, 1, 0, 0, 0, 0, 0, 0, 0);
    sbt2.record_bind();
    sbt2.record_update();

    // Stats should be same (order doesn't matter)
    let (gen1, updates1, binds1) = sbt1.stats();
    let (gen2, updates2, binds2) = sbt2.stats();

    assert_eq!(gen1, gen2, "Generation should match");
    assert_eq!(updates1, updates2, "Updates should match");
    assert_eq!(binds1, binds2, "Binds should match");
}

#[test]
fn q14_property_validate_correctness() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();

    // Aligned buffer addresses should validate
    for addr in [0x0, 0x1000, 0x10000, 0x100000] {
        sbt.build_layout(addr, 1, 0, 2, 16, 3, 48, 0, 0);
        assert!(sbt.validate(), "Aligned address {} should validate", addr);
    }

    // Unaligned buffer addresses should fail validation
    for addr in [0x1, 0x1001, 0x10001] {
        sbt.build_layout(addr, 1, 0, 2, 16, 3, 48, 0, 0);
        assert!(!sbt.validate(), "Unaligned address {} should fail validation", addr);
    }
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn q15_integration_full_sbt_workflow() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Step 1: Calculate buffer size
    let size = sbt.calculate_buffer_size(1, 0, 2, 16, 3, 48, 1, 32);
    assert!(size > 0, "Buffer size should be non-zero");

    // Step 2: Build layout
    let actual_size = sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 1, 32);
    assert_eq!(size, actual_size, "Calculated and actual sizes should match");

    // Step 3: Set buffer handle
    let buffer_handle = 0xDEADBEEF;
    sbt.set_buffer(buffer_handle);
    assert_eq!(sbt.buffer(), buffer_handle);

    // Step 4: Validate layout
    assert!(sbt.validate(), "Layout should be valid");

    // Step 5: Get regions for vkCmdTraceRaysKHR
    let ray_gen = sbt.ray_gen_region();
    let miss = sbt.miss_region();
    let hit_group = sbt.hit_group_region();
    let callable = sbt.callable_region();

    assert!(!ray_gen.is_empty());
    assert!(!miss.is_empty());
    assert!(!hit_group.is_empty());
    assert!(!callable.is_empty());

    // Step 6: Record bind
    sbt.record_bind();
    let (_, _, binds) = sbt.stats();
    assert_eq!(binds, 1);
}

#[test]
fn q16_integration_multi_geometry_sbt() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Simulate multi-geometry scene:
    // - 1 ray gen shader
    // - 2 miss shaders (primary + shadow)
    // - 6 hit groups (3 geometries x 2 ray types: primary + shadow)
    // - 2 callable shaders (utility functions)

    sbt.build_layout(
        buffer_addr,
        1,
        0,   // ray gen: no user data
        2,
        16,  // miss: 16 bytes (background color)
        6,
        64,  // hit group: 64 bytes (material properties)
        2,
        32,  // callable: 32 bytes (utility params)
    );

    // Verify all regions created
    assert_eq!(sbt.entry_count(SbtRegion::RayGen), 1);
    assert_eq!(sbt.entry_count(SbtRegion::Miss), 2);
    assert_eq!(sbt.entry_count(SbtRegion::HitGroup), 6);
    assert_eq!(sbt.entry_count(SbtRegion::Callable), 2);

    // Verify addresses are sequential and non-overlapping
    let hit_group = sbt.hit_group_region();
    for i in 0..6 {
        let addr = sbt.entry_address(SbtRegion::HitGroup, i).unwrap();
        let expected = hit_group.device_address + (i as u64 * hit_group.stride);
        assert_eq!(addr, expected, "Hit group {} address mismatch", i);
    }
}

#[test]
fn q17_integration_dynamic_updates() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Initial layout
    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 0, 0);
    let (gen1, _, _) = sbt.stats();

    // Simulate dynamic update (e.g., material change)
    sbt.record_update();
    let (gen2, updates, _) = sbt.stats();
    assert_eq!(gen2, gen1); // Generation unchanged
    assert_eq!(updates, 1);

    // Multiple updates
    for _ in 0..10 {
        sbt.record_update();
    }
    let (gen3, updates, _) = sbt.stats();
    assert_eq!(gen3, gen1); // Generation still unchanged
    assert_eq!(updates, 11);
}

#[test]
fn q18_integration_rebuild_layout() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // First layout
    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 0, 0, 0, 0);
    let (gen1, _, _) = sbt.stats();
    let size1 = sbt.buffer_size();

    // Rebuild with more shaders (e.g., LOD switch)
    sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 0, 0);
    let (gen2, _, _) = sbt.stats();
    let size2 = sbt.buffer_size();

    assert_eq!(gen2, gen1 + 1, "Generation should increment on rebuild");
    assert!(size2 > size1, "New layout should be larger");
}

#[test]
fn q19_integration_empty_regions() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Ray gen only (no miss, hit group, or callable)
    sbt.build_layout(buffer_addr, 1, 0, 0, 0, 0, 0, 0, 0);

    assert!(!sbt.ray_gen_region().is_empty());
    assert!(sbt.miss_region().is_empty());
    assert!(sbt.hit_group_region().is_empty());
    assert!(sbt.callable_region().is_empty());

    // Out-of-bounds access should return None
    assert!(sbt.entry_address(SbtRegion::Miss, 0).is_none());
    assert!(sbt.entry_address(SbtRegion::HitGroup, 0).is_none());
    assert!(sbt.entry_address(SbtRegion::Callable, 0).is_none());
}

#[test]
fn q20_integration_large_user_data() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Large user data per hit group (e.g., full material parameters)
    let user_data_size = 256; // 256 bytes per material
    sbt.build_layout(buffer_addr, 1, 0, 0, 0, 10, user_data_size, 0, 0);

    let hit_group = sbt.hit_group_region();
    let stride = hit_group.stride;

    // Stride should accommodate handle + user data + padding
    assert!(
        stride >= (sbt.shader_group_handle_size + user_data_size) as u64,
        "Stride should fit handle + user data"
    );
    assert_eq!(
        stride % sbt.shader_group_handle_alignment as u64,
        0,
        "Stride should be aligned"
    );
}

#[test]
fn q21_integration_concurrent_stats_access() {
    use std::sync::Arc;
    use std::thread;

    let sbt: Arc<ShaderBindingTableCapsule> = Arc::new(ShaderBindingTableCapsule::new_typical());

    // Spawn threads to concurrently access stats
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let sbt = Arc::clone(&sbt);
            thread::spawn(move || {
                for _ in 0..1000 {
                    sbt.record_bind();
                    let _ = sbt.stats();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    let (_, _, binds) = sbt.stats();
    assert_eq!(binds, 8000, "All binds should be recorded");
}

// ============================================================================
// Q22-Q28: Production Tests
// ============================================================================

#[test]
fn q22_production_max_shaders() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Large scene: 1000 hit groups (different materials)
    sbt.build_layout(buffer_addr, 1, 0, 4, 16, 1000, 48, 10, 32);

    assert_eq!(sbt.entry_count(SbtRegion::HitGroup), 1000);
    assert!(sbt.validate());

    // Verify first and last entries
    let first = sbt.entry_address(SbtRegion::HitGroup, 0).unwrap();
    let last = sbt.entry_address(SbtRegion::HitGroup, 999).unwrap();
    let hit_group = sbt.hit_group_region();

    assert_eq!(first, hit_group.device_address);
    assert_eq!(
        last,
        hit_group.device_address + (999 * hit_group.stride)
    );
}

#[test]
fn q23_production_stress_updates() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // Simulate 1 million shader record updates
    for _ in 0..1_000_000 {
        sbt.record_update();
    }

    let (_, updates, _) = sbt.stats();
    assert_eq!(updates, 1_000_000);
}

#[test]
fn q24_production_stress_binds() {
    let sbt = ShaderBindingTableCapsule::new_typical();

    // Simulate 1 million trace rays commands
    for _ in 0..1_000_000 {
        sbt.record_bind();
    }

    let (_, _, binds) = sbt.stats();
    assert_eq!(binds, 1_000_000);
}

#[test]
fn q25_production_edge_case_zero_user_data() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // All shaders with zero user data (handles only)
    sbt.build_layout(buffer_addr, 1, 0, 2, 0, 3, 0, 1, 0);

    assert!(sbt.validate());

    // All strides should be minimum (32 bytes for handle)
    assert_eq!(sbt.stride(SbtRegion::RayGen), 32);
    assert_eq!(sbt.stride(SbtRegion::Miss), 32);
    assert_eq!(sbt.stride(SbtRegion::HitGroup), 32);
    assert_eq!(sbt.stride(SbtRegion::Callable), 32);
}

#[test]
fn q26_production_edge_case_max_user_data() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Maximum user data (approaching max stride)
    let max_user_data = sbt.max_shader_group_stride - sbt.shader_group_handle_size - 32;
    sbt.build_layout(buffer_addr, 1, max_user_data, 0, 0, 1, max_user_data, 0, 0);

    assert!(sbt.validate());
    assert!(sbt.stride(SbtRegion::RayGen) <= sbt.max_shader_group_stride as u64);
    assert!(sbt.stride(SbtRegion::HitGroup) <= sbt.max_shader_group_stride as u64);
}

#[test]
fn q27_production_different_alignments() {
    // Test with different device alignment requirements
    let configs = [
        (32, 16, 32, 2048),   // Minimal alignment
        (32, 32, 64, 4096),   // Typical alignment
        (32, 64, 128, 8192),  // Aggressive alignment
        (32, 128, 256, 16384), // Maximum alignment
    ];

    for (handle_size, stride_align, base_align, max_stride) in configs {
        let mut sbt = ShaderBindingTableCapsule::new(handle_size, stride_align, base_align, max_stride);
        let buffer_addr = 0x1000_0000;

        sbt.build_layout(buffer_addr, 1, 0, 2, 16, 3, 48, 1, 32);

        assert!(sbt.validate(), "Failed for config ({}, {}, {}, {})",
            handle_size, stride_align, base_align, max_stride);

        // Verify alignment requirements
        let ray_gen = sbt.ray_gen_region();
        assert_eq!(ray_gen.device_address % base_align as u64, 0);
        assert_eq!(ray_gen.stride % stride_align as u64, 0);
    }
}

#[test]
fn q28_production_rebuild_loop() {
    let mut sbt = ShaderBindingTableCapsule::new_typical();
    let buffer_addr = 0x1000_0000;

    // Simulate 1000 layout rebuilds (dynamic scene changes)
    for i in 1..=1000 {
        let hit_group_count = (i % 100) + 1; // Vary hit group count
        sbt.build_layout(buffer_addr, 1, 0, 2, 16, hit_group_count, 48, 0, 0);

        assert!(sbt.validate());
        assert_eq!(sbt.entry_count(SbtRegion::HitGroup), hit_group_count);
    }

    let (gen, _, _) = sbt.stats();
    assert_eq!(gen, 1000);
}
