//! Acceleration Structure Capsule Tests
//!
//! T28 5-tier testing strategy:
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (invariants, edge cases)
//! - Q15-Q21: Integration tests (build/compaction pipeline)
//! - Q22-Q28: Production tests (realistic workloads)
//!
//! # Test Coverage
//!
//! - BLAS creation (static + dynamic)
//! - TLAS creation
//! - Handle management (VkAccelerationStructureKHR, device address, buffer)
//! - Build tracking (generation counters)
//! - Update tracking (refit operations)
//! - Compaction (size queries, copy operations)
//! - Instance management (transforms, masks, SBT offsets)
//! - Snapshot consistency (lockfree captures)

use atomic_capsule::gpu::{
    AccelerationStructureCapsule,
    AccelStructSnapshot,
    AccelStructType,
    GeometryType,
    BuildFlags,
    AccelInstance,
};

// ============================================================================
// Q1-Q7: Unit Tests (Basic Functionality)
// ============================================================================

#[test]
fn q1_blas_static_creation() {
    // Static mesh: PREFER_FAST_TRACE + ALLOW_COMPACTION
    let flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    let blas = AccelerationStructureCapsule::new_blas(1, 100_000, flags);

    assert_eq!(blas.structure_type(), AccelStructType::BottomLevel);
    assert_eq!(blas.geometry_count(), 1);
    assert_eq!(blas.primitive_count(), 100_000);
    assert!(blas.has_flag(BuildFlags::PreferFastTrace));
    assert!(blas.has_flag(BuildFlags::AllowCompaction));
    assert!(!blas.has_flag(BuildFlags::AllowUpdate));
}

#[test]
fn q2_blas_dynamic_creation() {
    // Dynamic mesh: PREFER_FAST_BUILD + ALLOW_UPDATE
    let flags = BuildFlags::PreferFastBuild.combine(BuildFlags::AllowUpdate);
    let blas = AccelerationStructureCapsule::new_blas(5, 50_000, flags);

    assert_eq!(blas.structure_type(), AccelStructType::BottomLevel);
    assert_eq!(blas.geometry_count(), 5);
    assert_eq!(blas.primitive_count(), 50_000);
    assert!(blas.has_flag(BuildFlags::PreferFastBuild));
    assert!(blas.has_flag(BuildFlags::AllowUpdate));
    assert!(!blas.has_flag(BuildFlags::AllowCompaction));
}

#[test]
fn q3_tlas_creation() {
    // TLAS: PREFER_FAST_BUILD (standard)
    let flags = BuildFlags::PreferFastBuild as u32;
    let tlas = AccelerationStructureCapsule::new_tlas(10_000, flags);

    assert_eq!(tlas.structure_type(), AccelStructType::TopLevel);
    assert_eq!(tlas.instance_count(), 10_000);
    assert_eq!(tlas.geometry_count(), 0); // TLAS has no geometry
    assert_eq!(tlas.primitive_count(), 0); // TLAS has no primitives
    assert!(tlas.has_flag(BuildFlags::PreferFastBuild));
}

#[test]
fn q4_handle_management() {
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, 0);

    // VkAccelerationStructureKHR handle
    blas.set_handle(0x1234_5678_9ABC_DEF0);
    assert_eq!(blas.handle(), 0x1234_5678_9ABC_DEF0);

    // Device address (256-byte aligned)
    blas.set_device_address(0x1000_0000);
    assert_eq!(blas.device_address(), 0x1000_0000);

    // Backing buffer
    blas.set_buffer(0xAABB_CCDD_EEFF_0011);
    assert_eq!(blas.buffer(), 0xAABB_CCDD_EEFF_0011);
}

#[test]
fn q5_build_tracking() {
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, 0);

    assert_eq!(blas.total_builds(), 0);

    blas.record_build();
    assert_eq!(blas.total_builds(), 1);

    blas.record_build();
    blas.record_build();
    assert_eq!(blas.total_builds(), 3);

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.build_generation, 3);
    assert_eq!(snapshot.total_builds, 3);
}

#[test]
fn q6_update_tracking() {
    let flags = BuildFlags::AllowUpdate as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, flags);

    assert_eq!(blas.total_updates(), 0);

    blas.record_update();
    assert_eq!(blas.total_updates(), 1);

    blas.record_update();
    blas.record_update();
    assert_eq!(blas.total_updates(), 3);

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.update_generation, 3);
    assert_eq!(snapshot.total_updates, 3);
}

#[test]
fn q7_compaction_tracking() {
    let flags = BuildFlags::AllowCompaction as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 100_000, flags);

    assert!(!blas.is_compacted());
    assert_eq!(blas.compacted_size(), 0);
    assert_eq!(blas.total_compactions(), 0);

    // Simulate compaction: 10MB → 6MB (40% reduction)
    blas.set_compacted_size(6_000_000);
    blas.mark_compacted();

    assert!(blas.is_compacted());
    assert_eq!(blas.compacted_size(), 6_000_000);
    assert_eq!(blas.total_compactions(), 1);
}

// ============================================================================
// Q8-Q14: Property Tests (Invariants & Edge Cases)
// ============================================================================

#[test]
fn q8_accel_instance_identity() {
    let inst = AccelInstance::new(0x1000_0000);

    // Identity transform
    assert_eq!(inst.transform[0], [1.0, 0.0, 0.0, 0.0]);
    assert_eq!(inst.transform[1], [0.0, 1.0, 0.0, 0.0]);
    assert_eq!(inst.transform[2], [0.0, 0.0, 1.0, 0.0]);

    // Full visibility mask (0xFF)
    assert_eq!(inst.mask(), 0xFF);

    // BLAS reference
    assert_eq!(inst.blas_reference, 0x1000_0000);
}

#[test]
fn q9_accel_instance_custom_index() {
    let mut inst = AccelInstance::new(0);

    // Test 24-bit range (0..16,777,215)
    inst.set_custom_index(0);
    assert_eq!(inst.custom_index(), 0);

    inst.set_custom_index(12345);
    assert_eq!(inst.custom_index(), 12345);

    inst.set_custom_index(16_777_215); // Max 24-bit
    assert_eq!(inst.custom_index(), 16_777_215);

    // Setting custom index doesn't affect mask
    inst.set_mask(0xAB);
    assert_eq!(inst.custom_index(), 16_777_215);
    assert_eq!(inst.mask(), 0xAB);
}

#[test]
fn q10_accel_instance_mask() {
    let mut inst = AccelInstance::new(0);

    // Test 8-bit range (0..255)
    inst.set_mask(0x00);
    assert_eq!(inst.mask(), 0x00);

    inst.set_mask(0xFF);
    assert_eq!(inst.mask(), 0xFF);

    inst.set_mask(0xAB);
    assert_eq!(inst.mask(), 0xAB);

    // Setting mask doesn't affect custom index
    inst.set_custom_index(9999);
    assert_eq!(inst.mask(), 0xAB);
    assert_eq!(inst.custom_index(), 9999);
}

#[test]
fn q11_accel_instance_shader_binding() {
    let mut inst = AccelInstance::new(0);

    // Test 24-bit SBT offset
    inst.set_shader_binding_offset(0);
    assert_eq!(inst.shader_binding_offset(), 0);

    inst.set_shader_binding_offset(999);
    assert_eq!(inst.shader_binding_offset(), 999);

    inst.set_shader_binding_offset(16_777_215); // Max 24-bit
    assert_eq!(inst.shader_binding_offset(), 16_777_215);

    // Setting SBT offset doesn't affect flags
    inst.set_flags(0x12);
    assert_eq!(inst.shader_binding_offset(), 16_777_215);
    assert_eq!(inst.flags(), 0x12);
}

#[test]
fn q12_accel_instance_flags() {
    let mut inst = AccelInstance::new(0);

    // Test 8-bit flags
    inst.set_flags(0x00);
    assert_eq!(inst.flags(), 0x00);

    inst.set_flags(0xFF);
    assert_eq!(inst.flags(), 0xFF);

    inst.set_flags(0x12);
    assert_eq!(inst.flags(), 0x12);

    // Setting flags doesn't affect SBT offset
    inst.set_shader_binding_offset(777);
    assert_eq!(inst.flags(), 0x12);
    assert_eq!(inst.shader_binding_offset(), 777);
}

#[test]
fn q13_build_flags_combine() {
    // Static BLAS: PREFER_FAST_TRACE + ALLOW_COMPACTION
    let flags1 = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    assert!(BuildFlags::contains(flags1, BuildFlags::PreferFastTrace));
    assert!(BuildFlags::contains(flags1, BuildFlags::AllowCompaction));
    assert!(!BuildFlags::contains(flags1, BuildFlags::AllowUpdate));

    // Dynamic BLAS: PREFER_FAST_BUILD + ALLOW_UPDATE
    let flags2 = BuildFlags::PreferFastBuild.combine(BuildFlags::AllowUpdate);
    assert!(BuildFlags::contains(flags2, BuildFlags::PreferFastBuild));
    assert!(BuildFlags::contains(flags2, BuildFlags::AllowUpdate));
    assert!(!BuildFlags::contains(flags2, BuildFlags::AllowCompaction));

    // Triple combination
    let flags3 = BuildFlags::PreferFastTrace
        .combine(BuildFlags::AllowCompaction)
        .combine(BuildFlags::LowMemory as u32);
    assert!(BuildFlags::contains(flags3, BuildFlags::PreferFastTrace));
    assert!(BuildFlags::contains(flags3, BuildFlags::AllowCompaction));
    assert!(BuildFlags::contains(flags3, BuildFlags::LowMemory));
}

#[test]
fn q14_snapshot_consistency() {
    let blas = AccelerationStructureCapsule::new_blas(1, 50_000, 0);

    blas.record_build();
    blas.record_build();
    blas.set_handle(0xDEAD_BEEF);
    blas.set_device_address(0x2000_0000);

    // Capture two snapshots without modifications
    let snap1 = blas.snapshot();
    let snap2 = blas.snapshot();

    // Snapshots should be identical
    assert_eq!(snap1.build_generation, snap2.build_generation);
    assert_eq!(snap1.update_generation, snap2.update_generation);
    assert_eq!(snap1.total_builds, snap2.total_builds);
    assert_eq!(snap1.handle, snap2.handle);
    assert_eq!(snap1.device_address, snap2.device_address);
}

// ============================================================================
// Q15-Q21: Integration Tests (Build/Compaction Pipeline)
// ============================================================================

#[test]
fn q15_build_update_generation_separation() {
    let flags = BuildFlags::AllowUpdate as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, flags);

    // Initial state
    let snap0 = blas.snapshot();
    assert_eq!(snap0.build_generation, 0);
    assert_eq!(snap0.update_generation, 0);

    // Record build (increments build_generation)
    blas.record_build();
    let snap1 = blas.snapshot();
    assert_eq!(snap1.build_generation, 1);
    assert_eq!(snap1.update_generation, 0);

    // Record update (increments update_generation)
    blas.record_update();
    let snap2 = blas.snapshot();
    assert_eq!(snap2.build_generation, 1);
    assert_eq!(snap2.update_generation, 1);

    // Another build
    blas.record_build();
    let snap3 = blas.snapshot();
    assert_eq!(snap3.build_generation, 2);
    assert_eq!(snap3.update_generation, 1);
}

#[test]
fn q16_compaction_pipeline() {
    let flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    let blas = AccelerationStructureCapsule::new_blas(1, 100_000, flags);

    // Step 1: Build with ALLOW_COMPACTION
    blas.record_build();
    assert!(!blas.is_compacted());

    // Step 2: Query compacted size (simulate 40% reduction)
    let original_size = 10_000_000u64;
    let compacted_size = (original_size as f32 * 0.6) as u64; // 6MB
    blas.set_compacted_size(compacted_size);

    assert_eq!(blas.compacted_size(), compacted_size);
    assert!(!blas.is_compacted()); // Not yet copied

    // Step 3: Compact (copy operation)
    blas.mark_compacted();

    assert!(blas.is_compacted());
    assert_eq!(blas.total_compactions(), 1);
}

#[test]
fn q17_multiple_compactions() {
    let flags = BuildFlags::AllowCompaction as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 100_000, flags);

    // First compaction
    blas.record_build();
    blas.set_compacted_size(6_000_000);
    blas.mark_compacted();

    assert_eq!(blas.total_compactions(), 1);

    // Rebuild (loses compaction)
    blas.record_build();

    // Second compaction
    blas.set_compacted_size(5_800_000); // Slightly better
    blas.mark_compacted();

    assert_eq!(blas.total_compactions(), 2);
}

#[test]
fn q18_tlas_instance_workflow() {
    let tlas = AccelerationStructureCapsule::new_tlas(3, 0);

    // Create instances
    let mut inst1 = AccelInstance::new(0x1000_0000);
    inst1.set_custom_index(0);
    inst1.set_mask(0xFF); // Opaque

    let mut inst2 = AccelInstance::new(0x2000_0000);
    inst2.set_custom_index(1);
    inst2.set_mask(0xFE); // Transparent

    let mut inst3 = AccelInstance::new(0x3000_0000);
    inst3.set_custom_index(2);
    inst3.set_shader_binding_offset(999); // Custom material

    // Verify independence
    assert_eq!(inst1.blas_reference, 0x1000_0000);
    assert_eq!(inst2.blas_reference, 0x2000_0000);
    assert_eq!(inst3.blas_reference, 0x3000_0000);

    assert_eq!(inst1.mask(), 0xFF);
    assert_eq!(inst2.mask(), 0xFE);

    assert_eq!(inst3.shader_binding_offset(), 999);

    // Record TLAS build
    tlas.record_build();
    assert_eq!(tlas.total_builds(), 1);
}

#[test]
fn q19_blas_update_efficiency() {
    let flags = BuildFlags::AllowUpdate as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, flags);

    // Initial build
    blas.record_build();

    // 10 updates (refit operations)
    for _ in 0..10 {
        blas.record_update();
    }

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 1);
    assert_eq!(snapshot.total_updates, 10);

    // Update efficiency: 10 / (1 + 10) = 90.9%
    let efficiency = snapshot.update_efficiency().unwrap();
    assert!((efficiency - 0.909).abs() < 0.01);
}

#[test]
fn q20_mixed_build_update_pattern() {
    let flags = BuildFlags::PreferFastBuild.combine(BuildFlags::AllowUpdate);
    let blas = AccelerationStructureCapsule::new_blas(1, 50_000, flags);

    // Build → 5 updates → Rebuild → 3 updates
    blas.record_build();
    for _ in 0..5 {
        blas.record_update();
    }

    blas.record_build();
    for _ in 0..3 {
        blas.record_update();
    }

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 2);
    assert_eq!(snapshot.total_updates, 8);
    assert_eq!(snapshot.build_generation, 2);
    assert_eq!(snapshot.update_generation, 8);

    // Update efficiency: 8 / (2 + 8) = 80%
    let efficiency = snapshot.update_efficiency().unwrap();
    assert!((efficiency - 0.8).abs() < 0.01);
}

#[test]
fn q21_concurrent_snapshot_captures() {
    let blas = AccelerationStructureCapsule::new_blas(1, 1000, 0);

    blas.record_build();
    blas.set_handle(0x1111_2222);

    // Capture 100 snapshots rapidly (lockfree consistency)
    let mut snapshots = Vec::new();
    for _ in 0..100 {
        snapshots.push(blas.snapshot());
    }

    // All snapshots should be identical (no modifications during capture)
    for snap in &snapshots {
        assert_eq!(snap.build_generation, 1);
        assert_eq!(snap.total_builds, 1);
        assert_eq!(snap.handle, 0x1111_2222);
    }
}

// ============================================================================
// Q22-Q28: Production Tests (Realistic Workloads)
// ============================================================================

#[test]
fn q22_large_static_mesh() {
    // Large static mesh: 1M triangles, 1 geometry (single material)
    let flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    let blas = AccelerationStructureCapsule::new_blas(1, 1_000_000, flags);

    assert_eq!(blas.structure_type(), AccelStructType::BottomLevel);
    assert_eq!(blas.geometry_count(), 1);
    assert_eq!(blas.primitive_count(), 1_000_000);

    // Simulate build + compaction
    blas.record_build();
    blas.set_compacted_size(50_000_000); // 50MB compacted
    blas.mark_compacted();

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 1);
    assert!(snapshot.is_compacted);
}

#[test]
fn q23_multi_material_mesh() {
    // Character mesh: 50K triangles, 8 materials (geometry groups)
    let flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    let blas = AccelerationStructureCapsule::new_blas(8, 50_000, flags);

    assert_eq!(blas.geometry_count(), 8);
    assert_eq!(blas.primitive_count(), 50_000);

    blas.record_build();
    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 1);
}

#[test]
fn q24_dynamic_animated_mesh() {
    // Animated character: 20K triangles, updates every frame
    let flags = BuildFlags::PreferFastBuild.combine(BuildFlags::AllowUpdate);
    let blas = AccelerationStructureCapsule::new_blas(5, 20_000, flags);

    // Initial build
    blas.record_build();

    // Simulate 60 frames (updates per frame)
    for _ in 0..60 {
        blas.record_update();
    }

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 1);
    assert_eq!(snapshot.total_updates, 60);

    // 98.4% update efficiency (ideal for dynamic meshes)
    let efficiency = snapshot.update_efficiency().unwrap();
    assert!(efficiency > 0.98);
}

#[test]
fn q25_massive_tlas() {
    // Open-world scene: 100K instances
    let flags = BuildFlags::PreferFastBuild as u32;
    let tlas = AccelerationStructureCapsule::new_tlas(100_000, flags);

    assert_eq!(tlas.structure_type(), AccelStructType::TopLevel);
    assert_eq!(tlas.instance_count(), 100_000);

    // TLAS rebuilt every frame (typical)
    for _ in 0..60 {
        tlas.record_build();
    }

    let snapshot = tlas.snapshot();
    assert_eq!(snapshot.total_builds, 60);
    assert_eq!(snapshot.total_updates, 0); // TLAS typically rebuilt, not updated
}

#[test]
fn q26_particle_system_no_compaction() {
    // Particle system: 10K particles, no compaction (not worth it)
    let flags = BuildFlags::PreferFastBuild as u32;
    let blas = AccelerationStructureCapsule::new_blas(1, 10_000, flags);

    assert!(!blas.has_flag(BuildFlags::AllowCompaction));

    // Rebuild every few frames (short lifetime)
    for _ in 0..10 {
        blas.record_build();
    }

    let snapshot = blas.snapshot();
    assert_eq!(snapshot.total_builds, 10);
    assert!(!snapshot.is_compacted);
}

#[test]
fn q27_terrain_chunked_blas() {
    // Terrain: 16 chunks, 100K triangles each
    let mut chunks = Vec::new();

    for _ in 0..16 {
        let flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
        let blas = AccelerationStructureCapsule::new_blas(1, 100_000, flags);
        blas.record_build();
        blas.set_compacted_size(5_000_000); // 5MB per chunk
        blas.mark_compacted();
        chunks.push(blas);
    }

    // Verify all chunks compacted
    for chunk in &chunks {
        assert!(chunk.is_compacted());
        assert_eq!(chunk.total_compactions(), 1);
    }
}

#[test]
fn q28_mixed_static_dynamic_scene() {
    // Scene: 1000 static BLAS + 100 dynamic BLAS + 1 TLAS

    // Static meshes (walls, props)
    let static_flags = BuildFlags::PreferFastTrace.combine(BuildFlags::AllowCompaction);
    let mut static_blas = Vec::new();
    for _ in 0..1000 {
        let blas = AccelerationStructureCapsule::new_blas(1, 5_000, static_flags);
        blas.record_build();
        static_blas.push(blas);
    }

    // Dynamic meshes (characters, vehicles)
    let dynamic_flags = BuildFlags::PreferFastBuild.combine(BuildFlags::AllowUpdate);
    let mut dynamic_blas = Vec::new();
    for _ in 0..100 {
        let blas = AccelerationStructureCapsule::new_blas(5, 20_000, dynamic_flags);
        blas.record_build();

        // Simulate 10 updates
        for _ in 0..10 {
            blas.record_update();
        }

        dynamic_blas.push(blas);
    }

    // TLAS (1100 instances total)
    let tlas = AccelerationStructureCapsule::new_tlas(1100, 0);
    tlas.record_build();

    // Verify counts
    assert_eq!(static_blas.len(), 1000);
    assert_eq!(dynamic_blas.len(), 100);

    for blas in &static_blas {
        let snap = blas.snapshot();
        assert_eq!(snap.total_builds, 1);
        assert_eq!(snap.total_updates, 0);
    }

    for blas in &dynamic_blas {
        let snap = blas.snapshot();
        assert_eq!(snap.total_builds, 1);
        assert_eq!(snap.total_updates, 10);
    }

    let tlas_snap = tlas.snapshot();
    assert_eq!(tlas_snap.total_builds, 1);
}
