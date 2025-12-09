//! Integration tests for alignment tiers.
//!
//! Validates that alignment tiers work correctly with real structures.

use atomic_capsule::{AlignmentMarker, AlignmentTier, ColdTier, HotTier, WarmTier};

#[repr(C, align(64))]
struct TestHotCapsule {
    data: [u8; 64],
}

impl AlignmentTier for TestHotCapsule {
    const TIER: &'static str = "hot";
    const ALIGNMENT: usize = 64;
}

#[repr(C, align(128))]
struct TestWarmCapsule {
    data: [u8; 128],
}

impl AlignmentTier for TestWarmCapsule {
    const TIER: &'static str = "warm";
    const ALIGNMENT: usize = 128;
}

#[repr(C, align(256))]
struct TestColdCapsule {
    data: [u8; 256],
}

impl AlignmentTier for TestColdCapsule {
    const TIER: &'static str = "cold";
    const ALIGNMENT: usize = 256;
}

#[test]
fn test_hot_capsule_alignment() {
    let capsule = TestHotCapsule { data: [0u8; 64] };
    let ptr = &capsule as *const _ as usize;

    // Verify 64-byte alignment
    assert_eq!(ptr % 64, 0, "Hot capsule must be 64-byte aligned");
    assert!(TestHotCapsule::verify_alignment());
}

#[test]
fn test_warm_capsule_alignment() {
    let capsule = TestWarmCapsule { data: [0u8; 128] };
    let ptr = &capsule as *const _ as usize;

    // Verify 128-byte alignment
    assert_eq!(ptr % 128, 0, "Warm capsule must be 128-byte aligned");
    assert!(TestWarmCapsule::verify_alignment());
}

#[test]
fn test_cold_capsule_alignment() {
    let capsule = TestColdCapsule { data: [0u8; 256] };
    let ptr = &capsule as *const _ as usize;

    // Verify 256-byte alignment
    assert_eq!(ptr % 256, 0, "Cold capsule must be 256-byte aligned");
    assert!(TestColdCapsule::verify_alignment());
}

#[test]
fn test_alignment_marker_hot() {
    let marker = AlignmentMarker::<HotTier>::new();
    assert_eq!(marker.alignment(), 64);
    assert_eq!(marker.tier(), "hot");
}

#[test]
fn test_alignment_marker_warm() {
    let marker = AlignmentMarker::<WarmTier>::new();
    assert_eq!(marker.alignment(), 128);
    assert_eq!(marker.tier(), "warm");
}

#[test]
fn test_alignment_marker_cold() {
    let marker = AlignmentMarker::<ColdTier>::new();
    assert_eq!(marker.alignment(), 256);
    assert_eq!(marker.tier(), "cold");
}

#[test]
fn test_tier_verification() {
    assert!(HotTier::verify_alignment());
    assert!(WarmTier::verify_alignment());
    assert!(ColdTier::verify_alignment());
}

#[test]
fn test_tier_constants() {
    assert_eq!(HotTier::ALIGNMENT, 64);
    assert_eq!(WarmTier::ALIGNMENT, 128);
    assert_eq!(ColdTier::ALIGNMENT, 256);

    assert_eq!(HotTier::TIER, "hot");
    assert_eq!(WarmTier::TIER, "warm");
    assert_eq!(ColdTier::TIER, "cold");
}

/// Test that alignment doesn't break with boxed allocation
#[test]
fn test_boxed_alignment() {
    let hot = Box::new(TestHotCapsule { data: [0u8; 64] });
    let ptr = &*hot as *const _ as usize;
    assert_eq!(ptr % 64, 0, "Boxed hot capsule must maintain alignment");

    let warm = Box::new(TestWarmCapsule { data: [0u8; 128] });
    let ptr = &*warm as *const _ as usize;
    assert_eq!(ptr % 128, 0, "Boxed warm capsule must maintain alignment");

    let cold = Box::new(TestColdCapsule { data: [0u8; 256] });
    let ptr = &*cold as *const _ as usize;
    assert_eq!(ptr % 256, 0, "Boxed cold capsule must maintain alignment");
}

/// Test alignment with array of capsules
#[test]
fn test_array_alignment() {
    let hot_array: [TestHotCapsule; 4] = [
        TestHotCapsule { data: [0u8; 64] },
        TestHotCapsule { data: [0u8; 64] },
        TestHotCapsule { data: [0u8; 64] },
        TestHotCapsule { data: [0u8; 64] },
    ];

    // Each element should maintain alignment
    for (i, capsule) in hot_array.iter().enumerate() {
        let ptr = capsule as *const _ as usize;
        assert_eq!(ptr % 64, 0, "Array element {} must be 64-byte aligned", i);
    }
}
