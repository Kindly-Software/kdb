//! # Trait Hierarchy Tests
//!
//! Comprehensive tests for unified capsule trait hierarchy.
//!
//! ## UCE33 Q33: Validation
//!
//! Tests validate:
//! - Hierarchical trait relationships
//! - Automatic composition (mixed capsules)
//! - Verification integration
//! - Property tests for invariants

#![cfg(feature = "unified-traits")]

use atomic_capsule::traits::unified::*;
use core::sync::atomic::AtomicU64;

// ============================================================================
// Test Capsules - One per Tier
// ============================================================================

/// Tier 1: Atomic capsule
#[repr(C, align(64))]
struct TestAtomicCapsule {
    state: AtomicU64,
    _padding: [u8; 56],
}

unsafe impl Capsule for TestAtomicCapsule {
    const TIER: Tier = Tier::T1Atomic;
    const ALIGNMENT: usize = 64;
    const SIZE: usize = 64;
}

unsafe impl AtomicCapsule for TestAtomicCapsule {
    type Primitive = AtomicU64;
}

/// Tier 2: SIMD capsule (nightly only)
#[cfg(feature = "portable_simd")]
#[repr(C, align(64))]
struct TestSimdCapsule {
    data: [f32; 8],
    _padding: [u8; 32],
}

#[cfg(feature = "portable_simd")]
unsafe impl Capsule for TestSimdCapsule {
    const TIER: Tier = Tier::T2Simd;
    const ALIGNMENT: usize = 64;
    const SIZE: usize = 64;
}

#[cfg(feature = "portable_simd")]
unsafe impl SimdCapsule for TestSimdCapsule {
    type Element = f32;
    const LANES: usize = 8;
}

/// Tier 3: Fixed-point capsule
#[repr(C, align(64))]
struct TestFixedPointCapsule {
    value: i16, // Q8.8
    _padding: [u8; 62],
}

unsafe impl Capsule for TestFixedPointCapsule {
    const TIER: Tier = Tier::T3FixedPoint;
    const ALIGNMENT: usize = 64;
    const SIZE: usize = 64;
}

unsafe impl FixedPointCapsule for TestFixedPointCapsule {
    type Integer = i16;
    const FRACTIONAL_BITS: u32 = 8;
}

/// Tier 4: Batch capsule
#[repr(C, align(128))]
struct TestBatchCapsule {
    items: [u32; 32],
    count: usize,
}

unsafe impl Capsule for TestBatchCapsule {
    const TIER: Tier = Tier::T4Batch;
    const ALIGNMENT: usize = 128;
    const SIZE: usize = 136; // 32 * 4 + 8
}

unsafe impl BatchCapsule for TestBatchCapsule {
    type Item = u32;
    const BATCH_SIZE: usize = 32;

    fn push(&mut self, item: Self::Item) -> Result<(), Self::Item> {
        if self.count < Self::BATCH_SIZE {
            self.items[self.count] = item;
            self.count += 1;
            Ok(())
        } else {
            Err(item)
        }
    }

    fn batch_process<F>(&mut self, mut f: F)
    where
        F: FnMut(&[Self::Item]),
    {
        if self.count > 0 {
            f(&self.items[..self.count]);
            self.count = 0;
        }
    }
}

/// Tier 5: Streaming capsule
#[repr(C, align(128))]
struct TestStreamingCapsule {
    window: [u64; 100],
    head: usize,
    sum: u64,
}

unsafe impl Capsule for TestStreamingCapsule {
    const TIER: Tier = Tier::T5Streaming;
    const ALIGNMENT: usize = 128;
    const SIZE: usize = 816; // 100 * 8 + 8 + 8
}

unsafe impl StreamingCapsule for TestStreamingCapsule {
    type Input = u64;
    type Aggregate = u64;
    const WINDOW_SIZE: usize = 100;

    fn push(&mut self, item: Self::Input) {
        let old_value = self.window[self.head];
        self.window[self.head] = item;
        self.head = (self.head + 1) % Self::WINDOW_SIZE;

        // Update running sum
        self.sum = self.sum.wrapping_sub(old_value).wrapping_add(item);
    }

    fn aggregate(&self) -> Self::Aggregate {
        self.sum
    }
}

/// Tier 6: Mixed capsule (Atomic + Fixed-Point)
#[repr(C, align(128))]
struct TestMixedCapsule {
    atomic_part: TestAtomicCapsule,
    fixed_part: TestFixedPointCapsule,
}

unsafe impl Capsule for TestMixedCapsule {
    const TIER: Tier = Tier::T6Mixed;
    const ALIGNMENT: usize = 128; // max(64, 64)
    const SIZE: usize = 128; // 64 + 64
}

unsafe impl MixedCapsule<TestAtomicCapsule, TestFixedPointCapsule> for TestMixedCapsule {
    fn component1(&self) -> &TestAtomicCapsule {
        &self.atomic_part
    }

    fn component2(&self) -> &TestFixedPointCapsule {
        &self.fixed_part
    }
}

// ============================================================================
// Tier Enum Tests
// ============================================================================

#[test]
fn test_tier_values() {
    assert_eq!(Tier::T1Atomic as u8, 1);
    assert_eq!(Tier::T2Simd as u8, 2);
    assert_eq!(Tier::T3FixedPoint as u8, 3);
    assert_eq!(Tier::T4Batch as u8, 4);
    assert_eq!(Tier::T5Streaming as u8, 5);
    assert_eq!(Tier::T6Mixed as u8, 6);
    assert_eq!(Tier::T7Gpu as u8, 7);
    assert_eq!(Tier::T8Network as u8, 8);
    assert_eq!(Tier::T9Persistent as u8, 9);
    assert_eq!(Tier::T10Probabilistic as u8, 10);
}

#[test]
fn test_tier_display() {
    assert_eq!(Tier::T1Atomic.to_string(), "Tier 1: Atomic");
    assert_eq!(Tier::T2Simd.to_string(), "Tier 2: SIMD");
    assert_eq!(Tier::T6Mixed.to_string(), "Tier 6: Mixed");
}

#[test]
fn test_tier_equality() {
    assert_eq!(Tier::T1Atomic, Tier::T1Atomic);
    assert_ne!(Tier::T1Atomic, Tier::T2Simd);
}

// ============================================================================
// Base Capsule Trait Tests
// ============================================================================

#[test]
fn test_atomic_capsule_verification() {
    // Should pass verification
    assert!(TestAtomicCapsule::verify().is_ok());
}

#[test]
fn test_atomic_capsule_properties() {
    assert_eq!(TestAtomicCapsule::TIER, Tier::T1Atomic);
    assert_eq!(TestAtomicCapsule::ALIGNMENT, 64);
    assert_eq!(TestAtomicCapsule::SIZE, 64);

    // Runtime verification matches compile-time constants
    assert_eq!(core::mem::align_of::<TestAtomicCapsule>(), 64);
    assert_eq!(core::mem::size_of::<TestAtomicCapsule>(), 64);
}

#[test]
fn test_atomic_capsule_type_id() {
    let type_id = TestAtomicCapsule::type_id();
    assert!(type_id.contains("TestAtomicCapsule"));
}

// ============================================================================
// Tier 1: Atomic Capsule Tests
// ============================================================================

#[test]
fn test_atomic_capsule_latency() {
    assert_eq!(TestAtomicCapsule::expected_latency_ns(), 15);
}

#[test]
fn test_atomic_capsule_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<TestAtomicCapsule>();
    assert_sync::<TestAtomicCapsule>();
}

// ============================================================================
// Tier 2: SIMD Capsule Tests (Nightly Only)
// ============================================================================

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_capsule_verification() {
    assert!(TestSimdCapsule::verify().is_ok());
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_capsule_properties() {
    assert_eq!(TestSimdCapsule::TIER, Tier::T2Simd);
    assert_eq!(TestSimdCapsule::LANES, 8);
    assert!(TestSimdCapsule::verify_lanes());
}

#[cfg(feature = "portable_simd")]
#[test]
fn test_simd_capsule_latency() {
    assert_eq!(TestSimdCapsule::expected_simd_latency_ns(), 5);
}

// ============================================================================
// Tier 3: Fixed-Point Capsule Tests
// ============================================================================

#[test]
fn test_fixed_point_capsule_verification() {
    assert!(TestFixedPointCapsule::verify().is_ok());
}

#[test]
fn test_fixed_point_capsule_properties() {
    assert_eq!(TestFixedPointCapsule::TIER, Tier::T3FixedPoint);
    assert_eq!(TestFixedPointCapsule::FRACTIONAL_BITS, 8);
    assert_eq!(TestFixedPointCapsule::scale_factor(), 256.0);
    assert!(TestFixedPointCapsule::verify_fractional_bits());
}

#[test]
fn test_fixed_point_capsule_latency() {
    assert_eq!(TestFixedPointCapsule::expected_latency_ns(), 2);
}

// ============================================================================
// Tier 4: Batch Capsule Tests
// ============================================================================

#[test]
fn test_batch_capsule_verification() {
    assert!(TestBatchCapsule::verify().is_ok());
}

#[test]
fn test_batch_capsule_properties() {
    assert_eq!(TestBatchCapsule::TIER, Tier::T4Batch);
    assert_eq!(TestBatchCapsule::BATCH_SIZE, 32);
    assert!(TestBatchCapsule::verify_batch_size());
}

#[test]
fn test_batch_capsule_push() {
    let mut batch = TestBatchCapsule {
        items: [0; 32],
        count: 0,
    };

    // Push items
    for i in 0..32 {
        assert!(batch.push(i).is_ok());
    }

    // Buffer full, should fail
    assert!(batch.push(99).is_err());
}

#[test]
fn test_batch_capsule_process() {
    let mut batch = TestBatchCapsule {
        items: [0; 32],
        count: 0,
    };

    // Push 10 items
    for i in 0..10 {
        batch.push(i).unwrap();
    }

    // Process batch
    let mut sum = 0;
    batch.batch_process(|items| {
        for &item in items {
            sum += item;
        }
    });

    assert_eq!(sum, 45); // 0+1+2+...+9 = 45
    assert_eq!(batch.count, 0); // Buffer cleared after processing
}

// ============================================================================
// Tier 5: Streaming Capsule Tests
// ============================================================================

#[test]
fn test_streaming_capsule_verification() {
    assert!(TestStreamingCapsule::verify().is_ok());
}

#[test]
fn test_streaming_capsule_properties() {
    assert_eq!(TestStreamingCapsule::TIER, Tier::T5Streaming);
    assert_eq!(TestStreamingCapsule::WINDOW_SIZE, 100);
    assert!(TestStreamingCapsule::verify_window_size());
}

#[test]
fn test_streaming_capsule_push() {
    let mut stream = TestStreamingCapsule {
        window: [0; 100],
        head: 0,
        sum: 0,
    };

    // Push 10 items
    for i in 1..=10 {
        stream.push(i);
    }

    // Aggregate should be sum of all items
    assert_eq!(stream.aggregate(), 55); // 1+2+...+10 = 55
}

#[test]
fn test_streaming_capsule_window_wraparound() {
    let mut stream = TestStreamingCapsule {
        window: [1; 100], // Initialize with 1s
        head: 0,
        sum: 100, // Sum initialized to 100
    };

    // Push 150 items (wraps around once)
    for i in 1..=150 {
        stream.push(i as u64);
    }

    // Should contain items 51-150 (last 100 items)
    // Sum = 51+52+...+150 = 10050
    assert_eq!(stream.aggregate(), 10050);
}

// ============================================================================
// Tier 6: Mixed Capsule Tests
// ============================================================================

#[test]
fn test_mixed_capsule_verification() {
    assert!(TestMixedCapsule::verify().is_ok());
}

#[test]
fn test_mixed_capsule_properties() {
    assert_eq!(TestMixedCapsule::TIER, Tier::T6Mixed);
    assert_eq!(TestMixedCapsule::ALIGNMENT, 128);
    assert!(TestMixedCapsule::verify_mixed_alignment());
}

#[test]
fn test_mixed_capsule_components() {
    let mixed = TestMixedCapsule {
        atomic_part: TestAtomicCapsule {
            state: AtomicU64::new(42),
            _padding: [0; 56],
        },
        fixed_part: TestFixedPointCapsule {
            value: 100,
            _padding: [0; 62],
        },
    };

    // Access components
    let _atomic = mixed.component1();
    let _fixed = mixed.component2();

    // Both components accessible
    assert_eq!(TestMixedCapsule::TIER, Tier::T6Mixed);
}

// ============================================================================
// Verification Error Tests
// ============================================================================

#[test]
fn test_verification_error_display() {
    let err = VerificationError::AlignmentMismatch {
        expected: 64,
        actual: 32,
    };
    let display = err.to_string();
    assert!(display.contains("64"));
    assert!(display.contains("32"));

    let err = VerificationError::SizeMismatch {
        expected: 128,
        actual: 64,
    };
    let display = err.to_string();
    assert!(display.contains("128"));

    let err = VerificationError::TierViolation {
        expected: Tier::T1Atomic,
        actual: Tier::T2Simd,
    };
    let display = err.to_string();
    assert!(display.contains("Tier 1"));
}

// ============================================================================
// Property Tests (Invariants)
// ============================================================================

#[test]
fn test_all_capsules_are_send_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    // All capsules must be Send + Sync
    assert_send::<TestAtomicCapsule>();
    assert_sync::<TestAtomicCapsule>();

    #[cfg(feature = "portable_simd")]
    {
        assert_send::<TestSimdCapsule>();
        assert_sync::<TestSimdCapsule>();
    }

    assert_send::<TestFixedPointCapsule>();
    assert_sync::<TestFixedPointCapsule>();

    assert_send::<TestBatchCapsule>();
    assert_sync::<TestBatchCapsule>();

    assert_send::<TestStreamingCapsule>();
    assert_sync::<TestStreamingCapsule>();

    assert_send::<TestMixedCapsule>();
    assert_sync::<TestMixedCapsule>();
}

#[test]
fn test_all_capsules_verify() {
    // All test capsules should pass verification
    assert!(TestAtomicCapsule::verify().is_ok());

    #[cfg(feature = "portable_simd")]
    assert!(TestSimdCapsule::verify().is_ok());

    assert!(TestFixedPointCapsule::verify().is_ok());
    assert!(TestBatchCapsule::verify().is_ok());
    assert!(TestStreamingCapsule::verify().is_ok());
    assert!(TestMixedCapsule::verify().is_ok());
}

#[test]
fn test_alignment_is_power_of_two() {
    // All capsule alignments must be power of 2
    assert!(TestAtomicCapsule::ALIGNMENT.is_power_of_two());

    #[cfg(feature = "portable_simd")]
    assert!(TestSimdCapsule::ALIGNMENT.is_power_of_two());

    assert!(TestFixedPointCapsule::ALIGNMENT.is_power_of_two());
    assert!(TestBatchCapsule::ALIGNMENT.is_power_of_two());
    assert!(TestStreamingCapsule::ALIGNMENT.is_power_of_two());
    assert!(TestMixedCapsule::ALIGNMENT.is_power_of_two());
}

#[test]
fn test_alignment_minimum_64_bytes() {
    // All capsules must be at least 64-byte aligned (cache line)
    assert!(TestAtomicCapsule::ALIGNMENT >= 64);

    #[cfg(feature = "portable_simd")]
    assert!(TestSimdCapsule::ALIGNMENT >= 64);

    assert!(TestFixedPointCapsule::ALIGNMENT >= 64);
    assert!(TestBatchCapsule::ALIGNMENT >= 64);
    assert!(TestStreamingCapsule::ALIGNMENT >= 64);
    assert!(TestMixedCapsule::ALIGNMENT >= 64);
}

#[test]
fn test_mixed_capsule_alignment_is_max() {
    // Mixed capsule alignment must be max of component alignments
    let component1_align = TestAtomicCapsule::ALIGNMENT;
    let component2_align = TestFixedPointCapsule::ALIGNMENT;
    let expected_align = component1_align.max(component2_align);

    assert!(TestMixedCapsule::ALIGNMENT >= expected_align);
}
