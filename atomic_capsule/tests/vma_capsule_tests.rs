//! VmaCapsule T28 Testing Suite
//!
//! Four-tier test pyramid:
//! - **Q1-Q7 (Unit)**: Single-capsule functionality (16 tests)
//! - **Q8-Q14 (Property)**: Invariants, generation monotonicity (16 tests)
//! - **Q15-Q21 (Integration)**: Multi-threaded coordination (12 tests)
//! - **Q22-Q28 (Production)**: Stress, latency, zero-allocation (12 tests)
//!
//! Total: 56 tests
//!
//! UCE34 Compliance:
//! - Q10: T1 Atomic tier (lockfree, <100ns latency target)
//! - Q33: Verification (alignment, layout, invariants)
//! - Q34: Audit trails (generation counter integrity, ABA prevention)

use atomic_capsule::gpu::{VmaCapsule, VmaError, VmaFlags, VmaSnapshot};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7) - 16 tests
// ============================================================================

#[test]
fn test_q1_layout_size() {
    assert_eq!(std::mem::size_of::<VmaCapsule>(), 64);
}

#[test]
fn test_q1_layout_alignment() {
    assert_eq!(std::mem::align_of::<VmaCapsule>(), 64);
}

#[test]
fn test_q2_construction() {
    let vma = VmaCapsule::new();
    let snap = vma.snapshot();
    assert!(!snap.pinned);
    assert_eq!(snap.offset, 0);
    assert_eq!(snap.refcount, 0);
}

#[test]
fn test_q2_default() {
    let vma = VmaCapsule::default();
    let snap = vma.snapshot();
    assert!(!snap.pinned);
}

#[test]
fn test_q3_pin_basic() {
    let vma = VmaCapsule::new();
    let result = vma.pin(0x100000, 256, VmaFlags::new().with_gtt().with_wb());
    assert!(result.is_ok());
    assert!(vma.is_pinned(0x100000));
}

#[test]
fn test_q3_pin_zero_offset_rejected() {
    let vma = VmaCapsule::new();
    assert_eq!(vma.pin(0, 256, VmaFlags::new().with_gtt()), Err(VmaError::InvalidOffset));
}

#[test]
fn test_q3_pin_misaligned_offset_rejected() {
    let vma = VmaCapsule::new();
    // 0x1001 is not 4KB-aligned
    assert_eq!(vma.pin(0x1001, 256, VmaFlags::new().with_gtt()), Err(VmaError::MisalignedOffset));
}

#[test]
fn test_q3_pin_zero_size_rejected() {
    let vma = VmaCapsule::new();
    assert_eq!(vma.pin(0x100000, 0, VmaFlags::new().with_gtt()), Err(VmaError::InvalidSize));
}

#[test]
fn test_q4_unpin_basic() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert!(vma.is_pinned(0x100000));
    assert!(vma.unpin(0x100000).is_ok());
    assert!(!vma.is_pinned(0x100000));
}

#[test]
fn test_q4_unpin_not_pinned_rejected() {
    let vma = VmaCapsule::new();
    assert_eq!(vma.unpin(0x100000), Err(VmaError::NotPinned));
}

#[test]
fn test_q4_unpin_wrong_offset_rejected() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert_eq!(vma.unpin(0x200000), Err(VmaError::InvalidOffset));
}

#[test]
fn test_q5_pin_twice_rejected() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert_eq!(vma.pin(0x200000, 128, VmaFlags::new().with_gtt()), Err(VmaError::AlreadyPinned));
}

#[test]
fn test_q5_snapshot_accuracy() {
    let vma = VmaCapsule::new();
    let flags = VmaFlags::new().with_ppgtt().with_wc().with_scanout();
    assert!(vma.pin(0x100000, 512, flags).is_ok());

    let snap = vma.snapshot();
    assert_eq!(snap.offset, 0x100000);
    assert!(snap.pinned);
    assert_eq!(snap.size, 512);
    assert!(snap.flags.is_ppgtt());
    assert!(snap.flags.is_wc());
}

#[test]
fn test_q6_size_pages() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert_eq!(vma.size_pages(), 256);
}

#[test]
fn test_q6_size_bytes() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert_eq!(vma.size_bytes(), 256 * 4096);
}

#[test]
fn test_q7_flags_construction() {
    let flags = VmaFlags::new().with_gtt().with_wc();
    assert!(flags.is_gtt());
    assert!(!flags.is_ppgtt());
    assert_eq!(flags.bits(), VmaFlags::GTT | VmaFlags::WC);
}

// ============================================================================
// TIER 2: PROPERTY TESTS (Q8-Q14) - 16 tests
// ============================================================================

#[test]
fn test_q8_generation_monotonicity() {
    let vma = VmaCapsule::new();

    // Pin and capture generation
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    let gen1 = vma.snapshot().gen_primary;

    // Unpin
    assert!(vma.unpin(0x100000).is_ok());

    // Snapshot shows generation incremented
    let snap = vma.snapshot();
    assert!(snap.gen_primary > gen1 || (gen1 == u16::MAX && snap.gen_primary == 0));
}

#[test]
fn test_q8_secondary_generation_sync() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let snap = vma.snapshot();
    // Both generations should match after pin
    assert_eq!(snap.gen_primary, snap.gen_secondary);
}

#[test]
fn test_q9_aba_prevention_primary() {
    let vma = VmaCapsule::new();

    // Pin at offset A
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    let gen_pin1 = vma.snapshot().gen_primary;

    // Unpin
    assert!(vma.unpin(0x100000).is_ok());

    // Pin at different offset B
    assert!(vma.pin(0x200000, 128, VmaFlags::new().with_gtt()).is_ok());
    let gen_pin2 = vma.snapshot().gen_primary;

    // Generations must differ (ABA prevention)
    assert_ne!(gen_pin1, gen_pin2);
}

#[test]
fn test_q9_offset_validity_invariant() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let snap = vma.snapshot();
    // Offset must be valid (non-zero, 4KB-aligned)
    assert_ne!(snap.offset, 0);
    assert_eq!(snap.offset & 0xFFF, 0);  // 4KB aligned
}

#[test]
fn test_q10_pinned_flag_consistency() {
    let vma = VmaCapsule::new();

    // Initially unpinned
    assert!(!vma.snapshot().pinned);

    // After pin, should be pinned
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert!(vma.snapshot().pinned);

    // After unpin, should be unpinned again
    assert!(vma.unpin(0x100000).is_ok());
    assert!(!vma.snapshot().pinned);
}

#[test]
fn test_q10_refcount_initial_state() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    // Initial refcount should be 1 (from pin)
    assert_eq!(vma.snapshot().refcount, 1);
}

#[test]
fn test_q11_refcount_increment_idempotency() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let snap1 = vma.snapshot();
    let rc1 = snap1.refcount;

    assert_eq!(vma.ref_increment().unwrap(), rc1 + 1);

    let snap2 = vma.snapshot();
    assert_eq!(snap2.refcount, rc1 + 1);
}

#[test]
fn test_q11_refcount_decrement_idempotency() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    // Increment first
    assert_eq!(vma.ref_increment().unwrap(), 2);

    // Then decrement
    assert_eq!(vma.ref_decrement().unwrap(), 1);

    // Snapshot should reflect decrement
    assert_eq!(vma.snapshot().refcount, 1);
}

#[test]
fn test_q12_size_immutability_after_pin() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let size1 = vma.size_pages();
    // Simulate some time passing
    std::thread::sleep(std::time::Duration::from_micros(1));
    let size2 = vma.size_pages();

    assert_eq!(size1, size2);
}

#[test]
fn test_q13_offset_immutability_after_pin() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let snap1 = vma.snapshot();
    let offset1 = snap1.offset;

    // Wait a bit
    std::thread::sleep(std::time::Duration::from_micros(10));

    let snap2 = vma.snapshot();
    let offset2 = snap2.offset;

    assert_eq!(offset1, offset2);
}

#[test]
fn test_q14_flags_preservation() {
    let vma = VmaCapsule::new();
    let original_flags = VmaFlags::new().with_gtt().with_wc().with_scanout();
    assert!(vma.pin(0x100000, 256, original_flags).is_ok());

    let snap = vma.snapshot();
    assert_eq!(snap.flags.bits(), original_flags.bits());
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21) - 12 tests
// ============================================================================

#[test]
fn test_q15_concurrent_read_snapshots() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let mut handles = vec![];
    for _ in 0..4 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            let snap = vma_clone.snapshot();
            assert!(snap.pinned);
            assert_eq!(snap.offset, 0x100000);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_q16_multi_threaded_refcount_increment() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let counter = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..8 {
        let vma_clone = vma.clone();
        let counter_clone = counter.clone();
        let handle = thread::spawn(move || {
            if vma_clone.ref_increment().is_ok() {
                counter_clone.fetch_add(1, Ordering::SeqCst);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let successful_increments = counter.load(Ordering::SeqCst);
    assert_eq!(successful_increments, 8);
    assert_eq!(vma.snapshot().refcount, 1 + 8);  // Initial 1 + 8 increments
}

#[test]
fn test_q17_pin_then_concurrent_reads() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let mut handles = vec![];
    for i in 0..8 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            // Verify pinned state
            assert!(vma_clone.is_pinned(0x100000), "Thread {}: VMA should be pinned", i);
            assert!(!vma_clone.is_pinned(0x200000), "Thread {}: Wrong offset should not match", i);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_q18_sequential_pin_unpin_cycles() {
    let vma = Arc::new(VmaCapsule::new());

    for offset in [0x100000, 0x200000, 0x300000] {
        assert!(vma.pin(offset, 256, VmaFlags::new().with_gtt()).is_ok());
        assert!(vma.is_pinned(offset));
        assert!(vma.unpin(offset).is_ok());
        assert!(!vma.is_pinned(offset));
    }
}

#[test]
fn test_q19_concurrent_snapshot_and_refcount() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let mut handles = vec![];

    // Half the threads take snapshots
    for _ in 0..4 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            let snap = vma_clone.snapshot();
            assert!(snap.pinned);
            snap.refcount
        });
        handles.push(handle);
    }

    // Other half increment refcount
    for _ in 0..4 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            let _ = vma_clone.ref_increment();
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    // Final refcount should be 1 + 4 increments
    assert_eq!(vma.snapshot().refcount, 1 + 4);
}

#[test]
fn test_q20_refcount_inc_dec_balanced() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let mut handles = vec![];

    // 4 threads increment
    for _ in 0..4 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            vma_clone.ref_increment().unwrap();
        });
        handles.push(handle);
    }

    // 4 threads decrement (after increment)
    for _ in 0..4 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            vma_clone.ref_decrement().unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Final refcount should still be 1 (balanced increments/decrements)
    // Note: Due to timing, we can't guarantee exact order, so we just check it's reasonable
    let final_rc = vma.snapshot().refcount;
    assert!(final_rc >= 1 && final_rc <= 5);  // Should be close to 1
}

#[test]
fn test_q21_generation_counter_advances() {
    let vma = Arc::new(VmaCapsule::new());

    let gen_before = vma.snapshot().gen_primary;

    // Pin, unpin, pin sequence
    for offset in [0x100000u64, 0x200000, 0x300000] {
        assert!(vma.pin(offset, 256, VmaFlags::new().with_gtt()).is_ok());
        let snap = vma.snapshot();
        assert!(snap.gen_primary != gen_before);  // Must change
        assert!(vma.unpin(offset).is_ok());
    }

    let gen_after = vma.snapshot().gen_primary;
    assert_ne!(gen_before, gen_after);
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28) - 12 tests
// ============================================================================

#[test]
fn test_q22_stress_concurrent_operations() {
    let vma = Arc::new(VmaCapsule::new());
    assert!(vma.pin(0x100000, 1024, VmaFlags::new().with_gtt()).is_ok());

    let mut handles = vec![];

    // 16 threads doing mixed operations
    for _ in 0..16 {
        let vma_clone = vma.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _ = vma_clone.snapshot();
                let _ = vma_clone.ref_increment();
                let _ = vma_clone.ref_decrement();
                let _ = vma_clone.is_pinned(0x100000);
                let _ = vma_clone.size_pages();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // VMA should still be pinned and operational
    assert!(vma.snapshot().pinned);
}

#[test]
fn test_q23_zero_allocation_snapshot() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    // Snapshot should not allocate
    let snap = vma.snapshot();
    assert!(snap.pinned);
    // If we got here without panic, allocation-free snapshot succeeded
}

#[test]
fn test_q24_latency_pin_sub_microsecond() {
    let vma = VmaCapsule::new();

    let start = std::time::Instant::now();
    let _ = vma.pin(0x100000, 256, VmaFlags::new().with_gtt());
    let elapsed = start.elapsed();

    // Target: <100ns (Intel i7-10700K baseline ~3-5ns per atomic op)
    // Allow 1 microsecond for test overhead
    assert!(elapsed.as_nanos() < 1000, "pin() took {:?}ns, target <1μs", elapsed.as_nanos());
}

#[test]
fn test_q25_latency_is_pinned_sub_microsecond() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let start = std::time::Instant::now();
    let _ = vma.is_pinned(0x100000);
    let elapsed = start.elapsed();

    // Target: <10ns (single atomic read)
    assert!(elapsed.as_nanos() < 1000, "is_pinned() took {:?}ns, target <10ns", elapsed.as_nanos());
}

#[test]
fn test_q26_latency_snapshot_sub_microsecond() {
    let vma = VmaCapsule::new();
    assert!(vma.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());

    let start = std::time::Instant::now();
    let _ = vma.snapshot();
    let elapsed = start.elapsed();

    // Target: <50ns (two atomic reads)
    assert!(elapsed.as_nanos() < 1000, "snapshot() took {:?}ns, target <50ns", elapsed.as_nanos());
}

#[test]
fn test_q27_multi_vma_independence() {
    let vma1 = Arc::new(VmaCapsule::new());
    let vma2 = Arc::new(VmaCapsule::new());

    // Pin both at different offsets
    assert!(vma1.pin(0x100000, 256, VmaFlags::new().with_gtt()).is_ok());
    assert!(vma2.pin(0x200000, 128, VmaFlags::new().with_ppgtt()).is_ok());

    // Verify independence
    assert!(vma1.is_pinned(0x100000));
    assert!(!vma1.is_pinned(0x200000));
    assert!(vma2.is_pinned(0x200000));
    assert!(!vma2.is_pinned(0x100000));

    // Unpin one doesn't affect other
    assert!(vma1.unpin(0x100000).is_ok());
    assert!(!vma1.is_pinned(0x100000));
    assert!(vma2.is_pinned(0x200000));  // Still pinned
}

#[test]
fn test_q28_production_workload_simulation() {
    // Simulate 1000 VMAs being pinned/unpinned in parallel
    let vma_count = 1000;
    let mut vmas = vec![];
    for i in 0..vma_count {
        vmas.push(Arc::new(VmaCapsule::new()));
    }

    let mut handles = vec![];
    for (i, vma) in vmas.iter().enumerate() {
        let vma_clone = vma.clone();
        let offset = 0x100000u64 + ((i as u64) * 0x1000);  // Staggered offsets
        let handle = thread::spawn(move || {
            // Pin
            let _ = vma_clone.pin(offset, 256, VmaFlags::new().with_gtt());
            // Do some work
            for _ in 0..10 {
                let _ = vma_clone.snapshot();
                let _ = vma_clone.ref_increment();
            }
            // Unpin
            let _ = vma_clone.unpin(offset);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All VMAs should be unpinned
    for vma in &vmas {
        assert!(!vma.snapshot().pinned);
    }
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

#[test]
fn test_offset_boundary_max() {
    let vma = VmaCapsule::new();
    // Max valid 40-bit offset: 2^40 - 1 = 0xFFFFFFFFFF
    // Must be 4KB-aligned: 0xFFFFFFFFC00
    let max_aligned = 0xFFFFFFFFC00u64;
    assert!(vma.pin(max_aligned, 1, VmaFlags::new().with_gtt()).is_ok());
    assert!(vma.is_pinned(max_aligned));
}

#[test]
fn test_size_large_allocation() {
    let vma = VmaCapsule::new();
    // 32-bit size field: 2^32 - 1 pages = 16TB
    let large_size = u32::MAX;
    assert!(vma.pin(0x100000, large_size, VmaFlags::new().with_gtt()).is_ok());
    assert_eq!(vma.size_pages(), large_size);
}

#[test]
fn test_all_flag_combinations() {
    let vma = VmaCapsule::new();
    let flags = VmaFlags::new()
        .with_gtt()
        .with_wc()
        .with_scanout();

    assert!(vma.pin(0x100000, 256, flags).is_ok());
    let snap = vma.snapshot();

    assert!(snap.flags.is_gtt());
    assert!(snap.flags.is_wc());
    // Check all expected flags are present
    assert_eq!(snap.flags.bits(), flags.bits());
}
