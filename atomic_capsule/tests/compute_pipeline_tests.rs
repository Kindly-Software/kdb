//! T28 Tests for ComputePipelineCapsule - T7 Heterogeneous Tier
//!
//! Testing Strategy:
//! - Q1-Q7: Unit tests (creation, dispatch, specialization, subgroups)
//! - Q8-Q14: Property tests (workgroup limits, atomics, concurrency)

#![cfg(any(feature = "gpu-cuda", feature = "gpu-rocm", feature = "gpu-intel", feature = "gpu-all"))]

use atomic_capsule::gpu::graphics::{ComputePipelineCapsule, SubgroupFeature, SpecConstant};
use std::sync::Arc;
use std::thread;

// ===== Q1-Q7: Unit Tests =====

#[test]
fn q1_create_basic_pipeline() {
    let pipeline = ComputePipelineCapsule::new(
        0x12345678, // pipeline
        0xABCDEF00, // layout
        0x98765432, // shader
        256,
        1,
        1,
    );

    assert_eq!(pipeline.pipeline(), 0x12345678);
    assert_eq!(pipeline.pipeline_layout(), 0xABCDEF00);
    assert_eq!(pipeline.shader_module(), 0x98765432);
    assert_eq!(pipeline.local_size(), (256, 1, 1));
    assert_eq!(pipeline.local_invocations(), 256);
}

#[test]
fn q2_local_size_calculations() {
    // 1D workgroup
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);
    assert_eq!(pipeline.local_invocations(), 256);

    // 2D workgroup
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 16, 16, 1);
    assert_eq!(pipeline.local_invocations(), 256);

    // 3D workgroup
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 8, 8, 4);
    assert_eq!(pipeline.local_invocations(), 256);
}

#[test]
fn q3_workgroup_size_validation() {
    // Valid sizes
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);
    assert!(pipeline.validate_workgroup_size());

    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 16, 16, 4);
    assert!(pipeline.validate_workgroup_size());

    // Invalid: Exceeds max_workgroup_size[0]
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 2048, 1, 1);
    assert!(!pipeline.validate_workgroup_size());

    // Invalid: Exceeds max_workgroup_invocations
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 32, 32, 2); // 2048 > 1024
    assert!(!pipeline.validate_workgroup_size());
}

#[test]
fn q4_specialization_constants() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Add constants
    assert!(pipeline.add_spec_constant(0, 256));
    assert!(pipeline.add_spec_constant(1, 2));
    assert!(pipeline.add_spec_constant(2, 42));

    // Verify
    let constants = pipeline.spec_constants();
    assert_eq!(constants.len(), 3);
    assert_eq!(constants[0].id, 0);
    assert_eq!(constants[0].value, 256);
    assert_eq!(constants[1].id, 1);
    assert_eq!(constants[1].value, 2);
    assert_eq!(constants[2].id, 2);
    assert_eq!(constants[2].value, 42);

    // Clear
    pipeline.clear_spec_constants();
    assert_eq!(pipeline.spec_constants().len(), 0);
}

#[test]
fn q5_specialization_overflow() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Fill up 16 constants
    for i in 0..16 {
        assert!(pipeline.add_spec_constant(i, i as u64));
    }

    // 17th should fail
    assert!(!pipeline.add_spec_constant(16, 16));
    assert_eq!(pipeline.spec_constants().len(), 16);
}

#[test]
fn q6_subgroup_operations() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // NVIDIA warp size
    pipeline.set_subgroup_size(32);
    assert_eq!(pipeline.subgroup_size(), 32);
    assert_eq!(pipeline.subgroups_per_workgroup(), 8); // 256 / 32

    // AMD wavefront size
    pipeline.set_subgroup_size(64);
    assert_eq!(pipeline.subgroup_size(), 64);
    assert_eq!(pipeline.subgroups_per_workgroup(), 4); // 256 / 64
}

#[test]
fn q7_subgroup_features() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Enable features
    pipeline.enable_subgroup_features(SubgroupFeature::Basic as u32);
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Basic));
    assert!(!pipeline.has_subgroup_feature(SubgroupFeature::Arithmetic));

    pipeline.enable_subgroup_features(SubgroupFeature::Arithmetic as u32);
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Basic));
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Arithmetic));

    // Multiple features at once
    let features = SubgroupFeature::Ballot as u32
        | SubgroupFeature::Shuffle as u32
        | SubgroupFeature::Quad as u32;
    pipeline.enable_subgroup_features(features);
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Ballot));
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Shuffle));
    assert!(pipeline.has_subgroup_feature(SubgroupFeature::Quad));
}

// ===== Q8-Q14: Property Tests =====

#[test]
fn q8_optimal_workgroup_size_nvidia() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 250, 1, 1);

    pipeline.set_subgroup_size(32); // NVIDIA warp
    let optimal = pipeline.optimal_workgroup_size();
    assert_eq!(optimal, (256, 1, 1)); // Rounded up: 250 → 256 (8 × 32)

    // Already optimal
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);
    pipeline.set_subgroup_size(32);
    let optimal = pipeline.optimal_workgroup_size();
    assert_eq!(optimal, (256, 1, 1));
}

#[test]
fn q9_optimal_workgroup_size_amd() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 250, 1, 1);

    pipeline.set_subgroup_size(64); // AMD wavefront
    let optimal = pipeline.optimal_workgroup_size();
    assert_eq!(optimal, (256, 1, 1)); // Rounded up: 250 → 256 (4 × 64)

    // Larger workgroup
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 500, 1, 1);
    pipeline.set_subgroup_size(64);
    let optimal = pipeline.optimal_workgroup_size();
    assert_eq!(optimal, (512, 1, 1)); // Rounded up: 500 → 512 (8 × 64)
}

#[test]
fn q10_dispatch_recording() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Single dispatch
    pipeline.record_dispatch(1024, 1, 1);
    assert_eq!(pipeline.total_dispatches(), 1);
    assert_eq!(pipeline.total_invocations(), 256 * 1024); // local × groups

    // Multiple dispatches
    pipeline.record_dispatch(100, 10, 1);
    assert_eq!(pipeline.total_dispatches(), 2);
    assert_eq!(
        pipeline.total_invocations(),
        256 * 1024 + 256 * 100 * 10
    );
}

#[test]
fn q11_dispatch_statistics() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Record 10 dispatches
    for _ in 0..10 {
        pipeline.record_dispatch(100, 1, 1);
    }

    assert_eq!(pipeline.total_dispatches(), 10);
    assert_eq!(pipeline.total_invocations(), 256 * 100 * 10);

    let avg = pipeline.avg_invocations_per_dispatch();
    assert!((avg - 25600.0).abs() < 1e-6); // 256 × 100
}

#[test]
fn q12_cache_hit_tracking() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // 10 dispatches, 8 cache hits
    for _ in 0..10 {
        pipeline.record_dispatch(100, 1, 1);
    }
    for _ in 0..8 {
        pipeline.record_cache_hit();
    }

    assert_eq!(pipeline.cache_hits(), 8);
    let hit_rate = pipeline.cache_hit_rate();
    assert!((hit_rate - 0.8).abs() < 1e-6); // 8/10 = 0.8
}

#[test]
fn q13_pipeline_hot_swap() {
    let pipeline = ComputePipelineCapsule::new(0x11111111, 0, 0, 256, 1, 1);

    // Swap pipeline
    let old = pipeline.set_pipeline(0x22222222);
    assert_eq!(old, 0x11111111);
    assert_eq!(pipeline.pipeline(), 0x22222222);
    assert_eq!(pipeline.pipeline_switches(), 1);

    // Swap again
    let old = pipeline.set_pipeline(0x33333333);
    assert_eq!(old, 0x22222222);
    assert_eq!(pipeline.pipeline(), 0x33333333);
    assert_eq!(pipeline.pipeline_switches(), 2);
}

#[test]
fn q14_concurrent_dispatches() {
    let pipeline = Arc::new(ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1));

    // Spawn 4 threads, each recording 100 dispatches
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let pipeline = Arc::clone(&pipeline);
            thread::spawn(move || {
                for _ in 0..100 {
                    pipeline.record_dispatch(10, 1, 1);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    // 4 threads × 100 dispatches = 400 total
    assert_eq!(pipeline.total_dispatches(), 400);
    assert_eq!(pipeline.total_invocations(), 256 * 10 * 400);
}

// ===== Additional Tests =====

#[test]
fn test_push_constants() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    pipeline.set_push_constants(0, 64);
    assert_eq!(pipeline.push_constants(), (0, 64));

    pipeline.set_push_constants(64, 32);
    assert_eq!(pipeline.push_constants(), (64, 32));
}

#[test]
fn test_device_limits() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // Default limits
    let limits = pipeline.device_limits();
    assert_eq!(limits.0, [1024, 1024, 64]);
    assert_eq!(limits.1, 1024);
    assert_eq!(limits.2, 32768);
    assert_eq!(limits.3, 128);

    // Update limits (AMD Radeon RX 7900 XTX)
    pipeline.set_device_limits([1024, 1024, 64], 2048, 65536, 256);

    let limits = pipeline.device_limits();
    assert_eq!(limits.0, [1024, 1024, 64]);
    assert_eq!(limits.1, 2048);
    assert_eq!(limits.2, 65536);
    assert_eq!(limits.3, 256);
}

#[test]
fn test_failed_dispatches() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    pipeline.record_dispatch(100, 1, 1);
    pipeline.record_dispatch_failure();
    pipeline.record_dispatch(100, 1, 1);
    pipeline.record_dispatch_failure();

    assert_eq!(pipeline.total_dispatches(), 2);
    assert_eq!(pipeline.failed_dispatches(), 2);
}

#[test]
fn test_specialization_recompiles() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    assert_eq!(pipeline.specialization_recompiles(), 0);

    pipeline.add_spec_constant(0, 256);
    assert_eq!(pipeline.specialization_recompiles(), 1);

    pipeline.add_spec_constant(1, 2);
    assert_eq!(pipeline.specialization_recompiles(), 2);
}

#[test]
fn test_spec_constant_types() {
    let u32_const = SpecConstant::from_u32(0, 42);
    assert_eq!(u32_const.id, 0);
    assert_eq!(u32_const.value, 42);
    assert_eq!(u32_const.size, 4);

    let i32_const = SpecConstant::from_i32(1, -42);
    assert_eq!(i32_const.id, 1);
    assert_eq!(i32_const.value, (-42i32) as u32 as u64);
    assert_eq!(i32_const.size, 4);

    let f32_const = SpecConstant::from_f32(2, 3.14159);
    assert_eq!(f32_const.id, 2);
    assert_eq!(f32_const.value, 3.14159f32.to_bits() as u64);
    assert_eq!(f32_const.size, 4);
}

#[test]
fn test_subgroup_feature_combine() {
    let features = SubgroupFeature::combine(&[
        SubgroupFeature::Basic,
        SubgroupFeature::Arithmetic,
        SubgroupFeature::Ballot,
    ]);

    assert_eq!(
        features,
        SubgroupFeature::Basic as u32
            | SubgroupFeature::Arithmetic as u32
            | SubgroupFeature::Ballot as u32
    );

    assert!(SubgroupFeature::Basic.is_set(features));
    assert!(SubgroupFeature::Arithmetic.is_set(features));
    assert!(SubgroupFeature::Ballot.is_set(features));
    assert!(!SubgroupFeature::Shuffle.is_set(features));
}

#[test]
fn test_concurrent_cache_hits() {
    let pipeline = Arc::new(ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1));

    // Spawn 8 threads, each recording cache hits
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let pipeline = Arc::clone(&pipeline);
            thread::spawn(move || {
                for _ in 0..100 {
                    pipeline.record_cache_hit();
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(pipeline.cache_hits(), 800); // 8 × 100
}

#[test]
fn test_subgroups_per_workgroup_rounding() {
    let mut pipeline = ComputePipelineCapsule::new(0, 0, 0, 250, 1, 1);

    pipeline.set_subgroup_size(32);
    // 250 / 32 = 7.8125 → rounds up to 8
    assert_eq!(pipeline.subgroups_per_workgroup(), 8);

    pipeline.set_subgroup_size(64);
    // 250 / 64 = 3.90625 → rounds up to 4
    assert_eq!(pipeline.subgroups_per_workgroup(), 4);
}

#[test]
fn test_3d_dispatch() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 8, 8, 8);

    // 3D dispatch: 10 × 10 × 10 workgroups
    pipeline.record_dispatch(10, 10, 10);

    let expected_invocations = 8 * 8 * 8 * 10 * 10 * 10; // 512 × 1000 = 512,000
    assert_eq!(pipeline.total_invocations(), expected_invocations);
}

#[test]
fn test_zero_cache_hit_rate() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    // No dispatches yet
    assert_eq!(pipeline.cache_hit_rate(), 0.0);

    // Dispatches but no cache hits
    pipeline.record_dispatch(100, 1, 1);
    assert_eq!(pipeline.cache_hit_rate(), 0.0);
}

#[test]
fn test_perfect_cache_hit_rate() {
    let pipeline = ComputePipelineCapsule::new(0, 0, 0, 256, 1, 1);

    for _ in 0..10 {
        pipeline.record_dispatch(100, 1, 1);
        pipeline.record_cache_hit();
    }

    assert!((pipeline.cache_hit_rate() - 1.0).abs() < 1e-6); // 10/10 = 1.0
}
