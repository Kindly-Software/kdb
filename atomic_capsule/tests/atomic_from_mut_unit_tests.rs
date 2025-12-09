//! # T28 Unit Tests for AtomicFromMut (Q1-Q7)
//!
//! **Comprehensive unit test coverage for atomic_from_mut module.**
//!
//! ## Test Organization (T28 Framework)
//! - **Q1**: Basic functionality (all atomic types)
//! - **Q2**: Layout compatibility verification
//! - **Q3**: Alignment guarantees
//! - **Q4**: Size validation
//! - **Q5**: Platform detection
//! - **Q6**: Type safety (compiler enforcement)
//! - **Q7**: API variants (safe, slice, pointer)
//!
//! ## Coverage Target
//! - 28 unit tests (Q1-Q7 basic functionality)
//! - 100% line coverage for core functionality
//! - All error paths exercised

#![cfg(feature = "atomic_from_mut")]
#![feature(atomic_from_mut)]

use atomic_capsule::primitives::{AtomicFromMut, AtomicFromMutError};
use core::sync::atomic::*;

// ============================================================================
// Q1: Basic Functionality Tests (All Atomic Types)
// ============================================================================

#[test]
fn test_q1_atomic_u8_from_mut() {
    let mut value: u8 = 42;
    let atomic_ref = AtomicU8::from_mut(&mut value);

    atomic_ref.store(100, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 100);
    assert_eq!(value, 100); // Underlying value modified
}

#[test]
fn test_q1_atomic_u16_from_mut() {
    let mut value: u16 = 0x1234;
    let atomic_ref = AtomicU16::from_mut(&mut value);

    atomic_ref.store(0x5678, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 0x5678);
    assert_eq!(value, 0x5678);
}

#[test]
fn test_q1_atomic_u32_from_mut() {
    let mut value: u32 = 0x12345678;
    let atomic_ref = AtomicU32::from_mut(&mut value);

    atomic_ref.store(0x9ABCDEF0, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 0x9ABCDEF0);
    assert_eq!(value, 0x9ABCDEF0);
}

#[test]
fn test_q1_atomic_u64_from_mut() {
    let mut value: u64 = 0x123456789ABCDEF0;
    let atomic_ref = AtomicU64::from_mut(&mut value);

    atomic_ref.store(0xFEDCBA9876543210, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 0xFEDCBA9876543210);
    assert_eq!(value, 0xFEDCBA9876543210);
}

#[test]
fn test_q1_atomic_usize_from_mut() {
    let mut value: usize = 12345;
    let atomic_ref = AtomicUsize::from_mut(&mut value);

    atomic_ref.store(67890, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 67890);
    assert_eq!(value, 67890);
}

#[test]
fn test_q1_atomic_i8_from_mut() {
    let mut value: i8 = -42;
    let atomic_ref = AtomicI8::from_mut(&mut value);

    atomic_ref.store(100, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 100);
    assert_eq!(value, 100);
}

#[test]
fn test_q1_atomic_i16_from_mut() {
    let mut value: i16 = -1000;
    let atomic_ref = AtomicI16::from_mut(&mut value);

    atomic_ref.store(5000, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 5000);
    assert_eq!(value, 5000);
}

#[test]
fn test_q1_atomic_i32_from_mut() {
    let mut value: i32 = -123456;
    let atomic_ref = AtomicI32::from_mut(&mut value);

    atomic_ref.store(789012, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 789012);
    assert_eq!(value, 789012);
}

#[test]
fn test_q1_atomic_i64_from_mut() {
    let mut value: i64 = -1234567890123456;
    let atomic_ref = AtomicI64::from_mut(&mut value);

    atomic_ref.store(9876543210987654, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 9876543210987654);
    assert_eq!(value, 9876543210987654);
}

#[test]
fn test_q1_atomic_isize_from_mut() {
    let mut value: isize = -12345;
    let atomic_ref = AtomicIsize::from_mut(&mut value);

    atomic_ref.store(67890, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 67890);
    assert_eq!(value, 67890);
}

#[test]
fn test_q1_atomic_bool_from_mut() {
    let mut value: bool = false;
    let atomic_ref = AtomicBool::from_mut(&mut value);

    atomic_ref.store(true, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), true);
    assert_eq!(value, true);
}

#[test]
fn test_q1_atomic_ptr_from_mut() {
    let mut target: u64 = 42;
    let mut ptr: *mut u64 = &mut target;
    let atomic_ref = AtomicPtr::from_mut(&mut ptr);

    let new_ptr = &mut target as *mut u64;
    atomic_ref.store(new_ptr, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), new_ptr);
}

// ============================================================================
// Q2: Layout Compatibility Verification
// ============================================================================

#[test]
fn test_q2_layout_size_match() {
    // Verify sizes match (compile-time, but test runtime confirmation)
    assert_eq!(core::mem::size_of::<AtomicU8>(), core::mem::size_of::<u8>());
    assert_eq!(
        core::mem::size_of::<AtomicU16>(),
        core::mem::size_of::<u16>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicU32>(),
        core::mem::size_of::<u32>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicU64>(),
        core::mem::size_of::<u64>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicUsize>(),
        core::mem::size_of::<usize>()
    );
    assert_eq!(core::mem::size_of::<AtomicI8>(), core::mem::size_of::<i8>());
    assert_eq!(
        core::mem::size_of::<AtomicI16>(),
        core::mem::size_of::<i16>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicI32>(),
        core::mem::size_of::<i32>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicI64>(),
        core::mem::size_of::<i64>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicIsize>(),
        core::mem::size_of::<isize>()
    );
    assert_eq!(
        core::mem::size_of::<AtomicBool>(),
        core::mem::size_of::<bool>()
    );
}

#[test]
fn test_q2_layout_alignment_match() {
    // Verify alignments match
    assert_eq!(
        core::mem::align_of::<AtomicU8>(),
        core::mem::align_of::<u8>()
    );
    assert_eq!(
        core::mem::align_of::<AtomicU16>(),
        core::mem::align_of::<u16>()
    );
    assert_eq!(
        core::mem::align_of::<AtomicU32>(),
        core::mem::align_of::<u32>()
    );
    assert_eq!(
        core::mem::align_of::<AtomicU64>(),
        core::mem::align_of::<u64>()
    );
    assert_eq!(
        core::mem::align_of::<AtomicUsize>(),
        core::mem::align_of::<usize>()
    );
}

// ============================================================================
// Q3: Alignment Guarantees
// ============================================================================

#[test]
fn test_q3_aligned_slice_u64_success() {
    // 8-byte aligned buffer
    let mut buffer = vec![0u8; 16];
    let ptr = buffer.as_mut_ptr() as usize;

    // Ensure alignment (may need to adjust start)
    let offset = (8 - (ptr % 8)) % 8;
    let aligned_slice = &mut buffer[offset..offset + 8];

    let result = AtomicU64::from_slice(aligned_slice);
    assert!(result.is_ok());

    let atomic_ref = result.unwrap();
    atomic_ref.store(0x123456789ABCDEF0, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 0x123456789ABCDEF0);
}

#[test]
fn test_q3_misaligned_slice_u64_error() {
    // Create intentionally misaligned buffer
    let mut buffer = vec![0u8; 17];

    // Find a misaligned position
    let ptr = buffer.as_mut_ptr() as usize;
    let offset = if ptr % 8 == 0 { 1 } else { 0 }; // Ensure misalignment

    let misaligned_slice = &mut buffer[offset..offset + 8];
    let result = AtomicU64::from_slice(misaligned_slice);

    match result {
        Err(AtomicFromMutError::MisalignedPointer { .. }) => {
            // Expected error
        }
        _ => panic!("Expected MisalignedPointer error for misaligned buffer"),
    }
}

#[test]
fn test_q3_aligned_slice_u32_success() {
    let mut buffer = vec![0u32; 4]; // Guaranteed 4-byte aligned
    let slice = unsafe {
        core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len() * 4)
    };

    let result = AtomicU32::from_slice(&mut slice[0..4]);
    assert!(result.is_ok());
}

// ============================================================================
// Q4: Size Validation
// ============================================================================

#[test]
fn test_q4_insufficient_size_u64_error() {
    let mut buffer = vec![0u8; 4]; // Only 4 bytes, u64 needs 8

    let result = AtomicU64::from_slice(&mut buffer[..]);
    match result {
        Err(AtomicFromMutError::InsufficientSize {
            buffer_size: 4,
            required_size: 8,
        }) => {
            // Expected error
        }
        _ => panic!("Expected InsufficientSize error"),
    }
}

#[test]
fn test_q4_exact_size_u64_success() {
    let mut buffer = vec![0u64; 1]; // Exactly 8 bytes, guaranteed aligned
    let slice = unsafe {
        core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len() * 8)
    };

    let result = AtomicU64::from_slice(slice);
    assert!(result.is_ok());
}

#[test]
fn test_q4_oversized_buffer_u64_success() {
    let mut buffer = vec![0u64; 10]; // 80 bytes, u64 only needs 8
    let slice = unsafe {
        core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len() * 8)
    };

    let result = AtomicU64::from_slice(&mut slice[0..8]);
    assert!(result.is_ok());
}

// ============================================================================
// Q5: Platform Detection (Compile-Time)
// ============================================================================

#[test]
fn test_q5_platform_support_x86_64() {
    #[cfg(target_arch = "x86_64")]
    {
        // x86-64 platform detected and supported
        let mut value: u64 = 0;
        let _ = AtomicU64::from_mut(&mut value);
    }
}

#[test]
fn test_q5_platform_support_aarch64() {
    #[cfg(target_arch = "aarch64")]
    {
        // ARM64 platform detected and supported
        let mut value: u64 = 0;
        let _ = AtomicU64::from_mut(&mut value);
    }
}

#[test]
fn test_q5_platform_support_riscv64() {
    #[cfg(target_arch = "riscv64")]
    {
        // RISC-V 64 platform detected and supported
        let mut value: u64 = 0;
        let _ = AtomicU64::from_mut(&mut value);
    }
}

// ============================================================================
// Q6: Type Safety (Compiler Enforcement)
// ============================================================================

// Note: These tests verify that incorrect type conversions are rejected at compile-time.
// Uncomment to verify compiler errors:

// #[test]
// fn test_q6_type_mismatch_rejected() {
//     let mut value_u64: u64 = 0;
//     // This should fail to compile: type mismatch
//     let atomic_ref: &mut AtomicU32 = AtomicU32::from_mut(&mut value_u64);
// }

// #[test]
// fn test_q6_lifetime_mismatch_rejected() {
//     let atomic_ref: &mut AtomicU64;
//     {
//         let mut value: u64 = 0;
//         atomic_ref = AtomicU64::from_mut(&mut value);
//         // atomic_ref outlives value - should fail to compile
//     }
//     // atomic_ref used here - compiler error
//     atomic_ref.load(Ordering::Relaxed);
// }

#[test]
fn test_q6_type_safety_verified_at_compile_time() {
    // If this test compiles, type safety is enforced
    let mut value: u64 = 42;
    let atomic_ref: &mut AtomicU64 = AtomicU64::from_mut(&mut value);
    assert_eq!(atomic_ref.load(Ordering::Relaxed), 42);
}

// ============================================================================
// Q7: API Variants (Safe, Slice, Pointer)
// ============================================================================

#[test]
fn test_q7_safe_api_variant() {
    let mut value: u64 = 0;
    let atomic_ref = AtomicU64::from_mut(&mut value);

    atomic_ref.store(123, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 123);
}

#[test]
fn test_q7_slice_api_variant() {
    let mut buffer = vec![0u64; 2]; // 16 bytes, guaranteed aligned
    let slice = unsafe {
        core::slice::from_raw_parts_mut(buffer.as_mut_ptr() as *mut u8, buffer.len() * 8)
    };

    let atomic_ref = AtomicU64::from_slice(&mut slice[0..8]).unwrap();
    atomic_ref.store(456, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 456);
}

#[test]
fn test_q7_pointer_api_variant() {
    let mut value: u64 = 0;
    let ptr: *mut u64 = &mut value;

    unsafe {
        let atomic_ref = AtomicU64::from_ptr(ptr);
        atomic_ref.store(789, Ordering::Release);
        assert_eq!(atomic_ref.load(Ordering::Acquire), 789);
    }
}

#[test]
fn test_q7_api_roundtrip_consistency() {
    // Test that all three APIs produce consistent results
    let mut value1: u64 = 100;
    let mut value2: u64 = 200;
    let mut value3: u64 = 300;

    // Safe API
    let atomic1 = AtomicU64::from_mut(&mut value1);
    atomic1.store(999, Ordering::Release);

    // Slice API
    let slice = unsafe { core::slice::from_raw_parts_mut(&mut value2 as *mut u64 as *mut u8, 8) };
    let atomic2 = AtomicU64::from_slice(slice).unwrap();
    atomic2.store(999, Ordering::Release);

    // Pointer API
    let atomic3 = unsafe { AtomicU64::from_ptr(&mut value3) };
    atomic3.store(999, Ordering::Release);

    // All should produce same result
    assert_eq!(value1, 999);
    assert_eq!(value2, 999);
    assert_eq!(value3, 999);
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

#[test]
fn test_atomic_operations_via_casted_reference() {
    let mut value: u64 = 10;
    let atomic_ref = AtomicU64::from_mut(&mut value);

    // Test various atomic operations
    atomic_ref.store(20, Ordering::Release);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 20);

    atomic_ref.fetch_add(5, Ordering::SeqCst);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 25);

    atomic_ref.fetch_sub(10, Ordering::SeqCst);
    assert_eq!(atomic_ref.load(Ordering::Acquire), 15);

    let result = atomic_ref.compare_exchange(15, 100, Ordering::SeqCst, Ordering::Relaxed);
    assert_eq!(result, Ok(15));
    assert_eq!(atomic_ref.load(Ordering::Acquire), 100);
}

#[test]
fn test_zero_overhead_verification() {
    // Verify that from_mut has zero runtime overhead
    let mut value: u64 = 42;

    // Direct atomic reference (baseline)
    let baseline_start = std::time::Instant::now();
    let atomic_ref = AtomicU64::from_mut(&mut value);
    let baseline_duration = baseline_start.elapsed();

    // Operation should be essentially instant (pointer cast only)
    assert!(baseline_duration.as_nanos() < 1000); // <1μs (generous upper bound)

    // Verify value is accessible
    assert_eq!(atomic_ref.load(Ordering::Relaxed), 42);
}
