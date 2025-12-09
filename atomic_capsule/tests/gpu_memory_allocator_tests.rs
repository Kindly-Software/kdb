// GPU Memory Allocator Tests - T28 Framework (28 Tests Across 4 Tiers)
// Testing MemoryAllocatorCapsule (T1 Atomic + T9 Persistent)
//
// Tier Breakdown:
// - Q1-Q7 (8 tests): Unit tests (basic functionality, edge cases)
// - Q8-Q14 (7 tests): Property tests (invariants, monotonicity, consistency)
// - Q15-Q21 (7 tests): Integration tests (multi-threaded, persistence, recovery)
// - Q22-Q28 (6 tests): Production tests (stress, sustained load, leak detection, regression)
//
// Total: 28 T28 tests covering all 4 tiers

#![cfg(all(feature = "std", any(feature = "gpu-intel", feature = "gpu-cuda", feature = "gpu-rocm")))]

use atomic_capsule::gpu::hal::{MemoryAllocatorCapsule, BuddyAllocError};
use std::sync::Arc;
use std::thread;

// ═══════════════════════════════════════════════════════════════════════════════
// Q1-Q7: Unit Tests (Basic functionality, edge cases, errors)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t28_q1_allocator_creation() {
    let alloc = MemoryAllocatorCapsule::new();
    assert_eq!(alloc.total_allocated(), 0);
    assert_eq!(alloc.allocation_count(), 0);
    assert_eq!(alloc.deallocation_count(), 0);
    assert_eq!(alloc.peak_allocated(), 0);
}

#[test]
fn t28_q2_simple_allocation() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT);
    assert!(addr.is_ok());
    let addr = addr.unwrap();
    assert!(addr > 0);
    assert_eq!(alloc.total_allocated(), 512);
    assert_eq!(alloc.allocation_count(), 1);
}

#[test]
fn t28_q3_alignment_validation() {
    let alloc = MemoryAllocatorCapsule::new();
    let result = alloc.allocate(512, 32); // Wrong alignment (requires 64B)
    assert!(result.is_err());
    assert!(matches!(result, Err(BuddyAllocError::AlignmentError { .. })));
}

#[test]
fn t28_q4_power_of_two_rounding() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(600, MemoryAllocatorCapsule::ALIGNMENT);
    assert!(addr.is_ok());
    // 600 should be rounded up to 1024 (next power-of-2)
    assert_eq!(alloc.total_allocated(), 1024);
}

#[test]
fn t28_q5_deallocation() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    assert_eq!(alloc.total_allocated(), 512);

    let result = alloc.deallocate(addr);
    assert!(result.is_ok());
    assert_eq!(alloc.total_allocated(), 0);
    assert_eq!(alloc.deallocation_count(), 1);
}

#[test]
fn t28_q6_invalid_deallocation() {
    let alloc = MemoryAllocatorCapsule::new();
    let result = alloc.deallocate(0x12345678);
    assert!(result.is_err());
    assert!(matches!(result, Err(BuddyAllocError::AddressNotFound { .. })));
}

#[test]
fn t28_q7_pool_exhaustion() {
    let alloc = MemoryAllocatorCapsule::new();
    let mut addrs = vec![];

    // Try to allocate more than pool capacity (32 slots)
    for i in 0..(MemoryAllocatorCapsule::MAX_SLOTS + 5) {
        match alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT) {
            Ok(addr) => addrs.push(addr),
            Err(BuddyAllocError::PoolExhausted) => {
                // Expected - pool is full
                assert!(i >= MemoryAllocatorCapsule::MAX_SLOTS);
                break;
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    assert_eq!(addrs.len(), MemoryAllocatorCapsule::MAX_SLOTS);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Q8-Q14: Property Tests (Invariants, monotonicity, idempotency, consistency)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t28_q8_alloc_count_monotonic() {
    let alloc = MemoryAllocatorCapsule::new();
    let count1 = alloc.allocation_count();
    let _ = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT);
    let count2 = alloc.allocation_count();
    let _ = alloc.allocate(1024, MemoryAllocatorCapsule::ALIGNMENT);
    let count3 = alloc.allocation_count();

    assert!(count2 > count1);
    assert!(count3 > count2);
}

#[test]
fn t28_q9_peak_memory_invariant() {
    let alloc = MemoryAllocatorCapsule::new();
    let _ = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT);
    let peak = alloc.peak_allocated();
    let current = alloc.total_allocated();
    assert!(peak >= current);
    assert_eq!(peak, 512);
}

#[test]
fn t28_q10_dealloc_count_consistency() {
    let alloc = MemoryAllocatorCapsule::new();
    assert_eq!(alloc.deallocation_count(), 0);

    let addr1 = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let addr2 = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    assert_eq!(alloc.deallocation_count(), 0);

    let _ = alloc.deallocate(addr1);
    assert_eq!(alloc.deallocation_count(), 1);

    let _ = alloc.deallocate(addr2);
    assert_eq!(alloc.deallocation_count(), 2);
}

#[test]
fn t28_q11_multiple_allocations_distinct_addresses() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr1 = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let addr2 = alloc.allocate(1024, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let addr3 = alloc.allocate(2048, MemoryAllocatorCapsule::ALIGNMENT).unwrap();

    assert_ne!(addr1, addr2);
    assert_ne!(addr2, addr3);
    assert_ne!(addr1, addr3);
    assert_eq!(alloc.total_allocated(), 512 + 1024 + 2048);
}

#[test]
fn t28_q12_idempotent_snapshot() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();

    let snap1 = alloc.snapshot();
    let snap2 = alloc.snapshot();

    assert_eq!(snap1.total_allocated, snap2.total_allocated);
    assert_eq!(snap1.allocation_count, snap2.allocation_count);
    assert_eq!(snap1.slots.len(), snap2.slots.len());
}

#[test]
fn t28_q13_fragmentation_bounds() {
    let alloc = MemoryAllocatorCapsule::new();
    for _ in 0..8 {
        let _ = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT);
    }
    // Fragmentation is bounded by allocation count and power-of-2 rounding
    assert!(alloc.total_allocated() <= 8 * 1024); // 512 → 1024 for each allocation
    assert_eq!(alloc.allocation_count(), 8);
}

#[test]
fn t28_q14_generation_tracking() {
    let alloc = MemoryAllocatorCapsule::new();
    let gen = alloc.mmap_generation.load(std::sync::atomic::Ordering::Relaxed);
    assert!(gen > 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Q15-Q21: Integration Tests (Multi-threaded, persistence, recovery)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t28_q15_snapshot_consistency() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let snapshot = alloc.snapshot();

    assert_eq!(snapshot.total_allocated, 512);
    assert_eq!(snapshot.slots.len(), 1);
    assert_eq!(snapshot.slots[0], (addr, 512));
}

#[test]
fn t28_q16_snapshot_after_dealloc() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let snap1 = alloc.snapshot();
    assert_eq!(snap1.total_allocated, 512);

    let _ = alloc.deallocate(addr);
    let snap2 = alloc.snapshot();
    assert_eq!(snap2.total_allocated, 0);
    assert_eq!(snap2.slots.len(), 0);
}

#[test]
fn t28_q17_allocation_sizes_tracked() {
    let alloc = MemoryAllocatorCapsule::new();
    let addr1 = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
    let addr2 = alloc.allocate(2048, MemoryAllocatorCapsule::ALIGNMENT).unwrap();

    let snapshot = alloc.snapshot();
    assert_eq!(snapshot.slots.len(), 2);
    assert_eq!(snapshot.total_allocated, 512 + 2048);

    let sizes: Vec<u64> = snapshot.slots.iter().map(|(_, size)| *size).collect();
    assert!(sizes.contains(&512));
    assert!(sizes.contains(&2048));
}

#[test]
fn t28_q18_concurrent_allocations() {
    let alloc: Arc<MemoryAllocatorCapsule> = Arc::new(MemoryAllocatorCapsule::new());
    let mut handles = vec![];

    for _ in 0..4 {
        let alloc_clone: Arc<MemoryAllocatorCapsule> = Arc::clone(&alloc);
        let handle = thread::spawn(move || {
            let mut addrs = vec![];
            for _ in 0..4 {
                if let Ok(addr) = alloc_clone.allocate(512, MemoryAllocatorCapsule::ALIGNMENT) {
                    addrs.push(addr);
                }
            }
            addrs
        });
        handles.push(handle);
    }

    let mut all_addrs = vec![];
    for handle in handles {
        let addrs = handle.join().unwrap();
        all_addrs.extend(addrs);
    }

    // Should have allocated up to 16 addresses (4 threads × 4 allocations),
    // but limited by pool size (32 slots)
    assert!(all_addrs.len() <= 16);
    assert!(all_addrs.len() > 0);
}

#[test]
fn t28_q19_mmap_persist() {
    let alloc = MemoryAllocatorCapsule::new();
    let _ = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT);
    let result = alloc.mmap_persist();
    assert!(result.is_ok());
}

#[test]
fn t28_q20_max_size_allocation() {
    let alloc = MemoryAllocatorCapsule::new();
    let result = alloc.allocate(MemoryAllocatorCapsule::MAX_SIZE, MemoryAllocatorCapsule::ALIGNMENT);
    // Should succeed as MAX_SIZE is the largest valid allocation
    assert!(result.is_ok());
}

#[test]
fn t28_q21_size_exceeds_max() {
    let alloc = MemoryAllocatorCapsule::new();
    let result = alloc.allocate(MemoryAllocatorCapsule::MAX_SIZE * 2, MemoryAllocatorCapsule::ALIGNMENT);
    assert!(result.is_err());
    assert!(matches!(result, Err(BuddyAllocError::OutOfMemory { .. })));
}

// ═══════════════════════════════════════════════════════════════════════════════
// Q22-Q28: Production Tests (Stress, sustained load, leak detection, regression)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn t28_q22_stress_allocations() {
    let alloc = MemoryAllocatorCapsule::new();

    // Attempt 1000 allocations to trigger pool exhaustion
    let mut successful = 0;
    for _ in 0..100 {
        match alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT) {
            Ok(_) => successful += 1,
            Err(BuddyAllocError::PoolExhausted) => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    // Should hit pool exhaustion at 32 allocations
    assert_eq!(successful, MemoryAllocatorCapsule::MAX_SLOTS);
}

#[test]
fn t28_q23_sustained_alloc_dealloc() {
    let alloc = MemoryAllocatorCapsule::new();

    // Repeated allocate/deallocate cycles to test state consistency
    for _ in 0..16 {
        let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
        assert_eq!(alloc.total_allocated(), 512);

        let _ = alloc.deallocate(addr);
        assert_eq!(alloc.total_allocated(), 0);
    }

    assert_eq!(alloc.allocation_count(), 16);
    assert_eq!(alloc.deallocation_count(), 16);
}

#[test]
fn t28_q24_memory_leak_detection() {
    let alloc = MemoryAllocatorCapsule::new();
    let mut addrs = vec![];

    // Allocate many addresses
    for _ in 0..16 {
        if let Ok(addr) = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT) {
            addrs.push(addr);
        }
    }

    let allocated_before = alloc.total_allocated();
    assert!(allocated_before > 0);

    // Deallocate all addresses
    for addr in addrs {
        let _ = alloc.deallocate(addr);
    }

    let allocated_after = alloc.total_allocated();
    assert_eq!(allocated_after, 0, "Memory leak detected: {} bytes not freed", allocated_before);
}

#[test]
fn t28_q25_allocation_ordering() {
    let alloc = MemoryAllocatorCapsule::new();
    let mut addrs = vec![];

    for _ in 0..8 {
        let addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
        addrs.push(addr);
    }

    // Addresses should all be distinct
    for i in 0..addrs.len() {
        for j in (i + 1)..addrs.len() {
            assert_ne!(addrs[i], addrs[j], "Duplicate addresses detected");
        }
    }
}

#[test]
fn t28_q26_power_of_two_sizes() {
    let alloc = MemoryAllocatorCapsule::new();

    for size in [512, 1024, 2048, 4096, 8192].iter() {
        let addr = alloc.allocate(*size, MemoryAllocatorCapsule::ALIGNMENT).unwrap();
        assert!(addr > 0);
        let _ = alloc.deallocate(addr);
        assert_eq!(alloc.total_allocated(), 0);
    }
}

#[test]
fn t28_q27_persistent_snapshot_stability() {
    let alloc = MemoryAllocatorCapsule::new();
    let _addr = alloc.allocate(512, MemoryAllocatorCapsule::ALIGNMENT).unwrap();

    // Snapshots should be consistent
    let snap1 = alloc.snapshot();
    let snap2 = alloc.snapshot();
    let snap3 = alloc.snapshot();

    assert_eq!(snap1.total_allocated, snap2.total_allocated);
    assert_eq!(snap2.total_allocated, snap3.total_allocated);
    assert_eq!(snap1.allocation_count, snap2.allocation_count);
    assert_eq!(snap2.allocation_count, snap3.allocation_count);
}

#[test]
fn t28_q28_production_regression_check() {
    let alloc = MemoryAllocatorCapsule::new();

    // Simulate typical production workload with mixed sizes
    let mut addresses = vec![];
    for i in 0..10 {
        for j in 0..3 {
            let size = if (i + j) % 2 == 0 { 512 } else { 2048 };
            if let Ok(addr) = alloc.allocate(size, MemoryAllocatorCapsule::ALIGNMENT) {
                addresses.push(addr);
            }
        }
    }

    // Verify consistency
    let total = alloc.total_allocated();
    let count = alloc.allocation_count();
    assert!(total > 0, "No memory allocated");
    assert_eq!(count as usize, addresses.len(), "Allocation count mismatch");

    // Cleanup and verify deallocation
    let cleanup_count = addresses.len();
    for addr in addresses {
        let result = alloc.deallocate(addr);
        assert!(result.is_ok(), "Deallocation failed");
    }

    assert_eq!(alloc.total_allocated(), 0, "Memory not fully deallocated");
    assert_eq!(
        alloc.deallocation_count() as usize,
        cleanup_count,
        "Deallocation count mismatch"
    );
}
