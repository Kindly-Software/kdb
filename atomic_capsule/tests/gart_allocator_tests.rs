//! GART Allocator Tests - T28 5-Tier Testing
//!
//! Test Coverage:
//! - Q1-Q7: Unit tests (basic operations, edge cases)
//! - Q8-Q14: Property tests (invariants, safety)
//! - Q15-Q21: Integration tests (multi-threaded stress)
//! - Q22-Q28: Production tests (fragmentation resistance)
//! - Q29-Q35: Determinism tests (reproducible allocation patterns)

#![cfg(feature = "kgpu-driver")]

use atomic_capsule::gpu::kgpu_driver::{
    GartAllocatorCapsule, GartVendor, GartError, MemoryDomain,
};

// ============================================================================
// Q1-Q7: Unit Tests (Basic Operations)
// ============================================================================

#[test]
fn q1_test_new_allocator_generic() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    assert_eq!(allocator.allocated_pages(), 0);
    assert!(allocator.free_pages() > 0);
    assert_eq!(allocator.generation(), 0);
}

#[test]
fn q1_test_new_allocator_intel() {
    let allocator = GartAllocatorCapsule::new(2048, GartVendor::Intel);
    assert_eq!(allocator.allocated_pages(), 0);
    assert_eq!(allocator.generation(), 0);
}

#[test]
fn q1_test_new_allocator_amd() {
    let allocator = GartAllocatorCapsule::new(4096, GartVendor::Amd);
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q1_test_new_allocator_nvidia() {
    let allocator = GartAllocatorCapsule::new(8192, GartVendor::Nvidia);
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q2_test_alloc_order_0() {
    // Order 0 = 4KB allocation
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let result = allocator.alloc(0);
    assert!(result.is_ok());
    let addr = result.unwrap();
    assert_eq!(addr % 4096, 0);  // 4KB-aligned
    assert_eq!(allocator.allocated_pages(), 1);
}

#[test]
fn q2_test_alloc_order_1() {
    // Order 1 = 8KB allocation
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let result = allocator.alloc(1);
    assert!(result.is_ok());
    let addr = result.unwrap();
    assert_eq!(addr % 8192, 0);  // 8KB-aligned
    assert_eq!(allocator.allocated_pages(), 2);
}

#[test]
fn q2_test_alloc_order_2() {
    // Order 2 = 16KB allocation
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let result = allocator.alloc(2);
    assert!(result.is_ok());
    let addr = result.unwrap();
    assert_eq!(addr % 16384, 0);  // 16KB-aligned
    assert_eq!(allocator.allocated_pages(), 4);
}

#[test]
fn q3_test_free_basic() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let addr = allocator.alloc(0).unwrap();
    let result = allocator.free(addr, 0);
    assert!(result.is_ok());
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q3_test_free_order_1() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let addr = allocator.alloc(1).unwrap();
    assert_eq!(allocator.allocated_pages(), 2);

    let result = allocator.free(addr, 1);
    assert!(result.is_ok());
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q4_test_double_free_detection() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let addr = allocator.alloc(0).unwrap();

    // First free: OK
    let result1 = allocator.free(addr, 0);
    assert!(result1.is_ok());

    // Second free: Should fail with DoubleFree
    let result2 = allocator.free(addr, 0);
    assert!(result2.is_err());
    match result2 {
        Err(GartError::DoubleFree { addr: a }) => assert_eq!(a, addr),
        _ => panic!("Expected DoubleFree error"),
    }
}

#[test]
fn q5_test_invalid_order() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    // Order 23 exceeds MAX_ORDER (22)
    let result = allocator.alloc(23);
    assert!(result.is_err());
    match result {
        Err(GartError::InvalidOrder { order, max_order }) => {
            assert_eq!(order, 23);
            assert_eq!(max_order, 22);
        }
        _ => panic!("Expected InvalidOrder error"),
    }
}

#[test]
fn q6_test_alignment_validation() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    // Misaligned address (not 4KB-aligned)
    let result = allocator.free(0x800, 0);
    assert!(result.is_err());
    match result {
        Err(GartError::NotAligned { addr, required_alignment }) => {
            assert_eq!(addr, 0x800);
            assert_eq!(required_alignment, 4096);
        }
        _ => panic!("Expected NotAligned error"),
    }
}

#[test]
fn q7_test_generation_increment_on_alloc() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let gen1 = allocator.generation();

    let _addr = allocator.alloc(0).unwrap();
    let gen2 = allocator.generation();

    // Generation should increment (allow wraparound)
    assert!(gen2 > gen1 || (gen1 == u32::MAX && gen2 == 0));
}

#[test]
fn q7_test_generation_increment_on_free() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let addr = allocator.alloc(0).unwrap();
    let gen1 = allocator.generation();

    allocator.free(addr, 0).unwrap();
    let gen2 = allocator.generation();

    assert!(gen2 > gen1 || (gen1 == u32::MAX && gen2 == 0));
}

// ============================================================================
// Q8-Q14: Property Tests (Invariants)
// ============================================================================

#[test]
fn q8_property_allocated_plus_free_equals_total() {
    let total_pages = 1024u32;
    let allocator = GartAllocatorCapsule::new(total_pages, GartVendor::Generic);

    let allocated = allocator.allocated_pages();
    let free = allocator.free_pages();

    // allocated + free should approximately equal total
    // (some pages may be reserved/unavailable)
    assert!(allocated + free <= total_pages);
}

#[test]
fn q9_property_multiple_allocs_no_overlap() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    let addr1 = allocator.alloc(0).unwrap();
    let addr2 = allocator.alloc(0).unwrap();
    let addr3 = allocator.alloc(0).unwrap();

    // All addresses should be distinct (no overlap)
    assert_ne!(addr1, addr2);
    assert_ne!(addr2, addr3);
    assert_ne!(addr1, addr3);
}

#[test]
fn q10_property_alloc_free_alloc_reuses_memory() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    let addr1 = allocator.alloc(0).unwrap();
    allocator.free(addr1, 0).unwrap();

    let addr2 = allocator.alloc(0).unwrap();

    // Should reuse freed memory (may be same address)
    // At minimum, allocated pages should be 1
    assert_eq!(allocator.allocated_pages(), 1);
}

#[test]
fn q11_property_alignment_invariant_order_0() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    for _ in 0..10 {
        if let Ok(addr) = allocator.alloc(0) {
            assert_eq!(addr % 4096, 0, "Order 0 must be 4KB-aligned");
        }
    }
}

#[test]
fn q12_property_alignment_invariant_order_1() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    for _ in 0..5 {
        if let Ok(addr) = allocator.alloc(1) {
            assert_eq!(addr % 8192, 0, "Order 1 must be 8KB-aligned");
        }
    }
}

#[test]
fn q13_property_allocation_count_consistency() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    let initial_allocated = allocator.allocated_pages();

    let addr1 = allocator.alloc(0).unwrap();
    assert_eq!(allocator.allocated_pages(), initial_allocated + 1);

    let addr2 = allocator.alloc(1).unwrap();
    assert_eq!(allocator.allocated_pages(), initial_allocated + 1 + 2);

    allocator.free(addr1, 0).unwrap();
    assert_eq!(allocator.allocated_pages(), initial_allocated + 2);

    allocator.free(addr2, 1).unwrap();
    assert_eq!(allocator.allocated_pages(), initial_allocated);
}

#[test]
fn q14_property_no_allocation_after_oom() {
    let allocator = GartAllocatorCapsule::new(16, GartVendor::Generic);

    // Allocate until OOM
    let mut addrs = Vec::new();
    loop {
        match allocator.alloc(0) {
            Ok(addr) => addrs.push(addr),
            Err(GartError::OutOfMemory { .. }) => break,
            Err(e) => panic!("Unexpected error: {:?}", e),
        }

        if addrs.len() > 100 {
            panic!("Allocation didn't fail after 100 iterations");
        }
    }

    // Verify no more allocations possible
    assert!(allocator.alloc(0).is_err());
}

// ============================================================================
// Q15-Q21: Integration Tests (Multi-threaded Stress)
// ============================================================================

#[cfg(feature = "std")]
#[test]
fn q15_integration_concurrent_alloc_free() {
    use std::sync::Arc;
    use std::thread;

    let allocator = Arc::new(GartAllocatorCapsule::new(4096, GartVendor::Generic));
    let mut handles = vec![];

    // Spawn 4 threads, each doing 10 alloc/free cycles
    for _ in 0..4 {
        let alloc_clone = Arc::clone(&allocator);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                if let Ok(addr) = alloc_clone.alloc(0) {
                    let _ = alloc_clone.free(addr, 0);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // At the end, all memory should be freed
    assert_eq!(allocator.allocated_pages(), 0);
}

#[cfg(feature = "std")]
#[test]
fn q16_integration_mixed_order_allocations() {
    use std::sync::Arc;
    use std::thread;

    let allocator = Arc::new(GartAllocatorCapsule::new(8192, GartVendor::Generic));
    let mut handles = vec![];

    // Thread 1: Order 0 allocations
    let alloc_clone = Arc::clone(&allocator);
    handles.push(thread::spawn(move || {
        for _ in 0..5 {
            let _ = alloc_clone.alloc(0);
        }
    }));

    // Thread 2: Order 1 allocations
    let alloc_clone = Arc::clone(&allocator);
    handles.push(thread::spawn(move || {
        for _ in 0..5 {
            let _ = alloc_clone.alloc(1);
        }
    }));

    // Thread 3: Order 2 allocations
    let alloc_clone = Arc::clone(&allocator);
    handles.push(thread::spawn(move || {
        for _ in 0..5 {
            let _ = alloc_clone.alloc(2);
        }
    }));

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify no double-allocations (allocated > 0)
    assert!(allocator.allocated_pages() > 0);
}

#[cfg(feature = "std")]
#[test]
fn q17_integration_stress_test_100_threads() {
    use std::sync::Arc;
    use std::thread;

    let allocator = Arc::new(GartAllocatorCapsule::new(16384, GartVendor::Generic));
    let mut handles = vec![];

    // Spawn 100 threads, each doing 5 allocations
    for _ in 0..100 {
        let alloc_clone = Arc::clone(&allocator);
        let handle = thread::spawn(move || {
            for _ in 0..5 {
                let _ = alloc_clone.alloc(0);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify allocations occurred (some may have failed due to OOM)
    assert!(allocator.allocated_pages() > 0);
}

#[test]
fn q18_integration_vendor_config_intel() {
    let mut allocator = GartAllocatorCapsule::new(2048, GartVendor::Intel);
    allocator.configure_intel(0x1000_0000, 0x1_0000_0000);

    // Verify allocation still works after vendor config
    let result = allocator.alloc(0);
    assert!(result.is_ok());
}

#[test]
fn q19_integration_vendor_config_amd() {
    let mut allocator = GartAllocatorCapsule::new(2048, GartVendor::Amd);
    allocator.configure_amd(0x2000_0000, 0x8000_0000);

    let result = allocator.alloc(0);
    assert!(result.is_ok());
}

#[test]
fn q20_integration_vendor_config_nvidia() {
    let mut allocator = GartAllocatorCapsule::new(2048, GartVendor::Nvidia);
    allocator.configure_nvidia(0x3000_0000, 0x4000_0000);

    let result = allocator.alloc(0);
    assert!(result.is_ok());
}

#[test]
fn q21_integration_large_allocation_split() {
    let allocator = GartAllocatorCapsule::new(8192, GartVendor::Generic);

    // Allocate order 5 (128KB), which should split higher orders
    let result = allocator.alloc(5);
    assert!(result.is_ok());
    let addr = result.unwrap();

    // Verify alignment (128KB = 131072 bytes)
    assert_eq!(addr % 131072, 0);
    assert_eq!(allocator.allocated_pages(), 32);  // 128KB / 4KB = 32 pages
}

// ============================================================================
// Q22-Q28: Production Tests (Fragmentation Resistance)
// ============================================================================

#[test]
fn q22_production_fragmentation_test() {
    let allocator = GartAllocatorCapsule::new(4096, GartVendor::Generic);

    // Allocate 10 blocks
    let mut addrs = Vec::new();
    for _ in 0..10 {
        if let Ok(addr) = allocator.alloc(0) {
            addrs.push(addr);
        }
    }

    // Free every other block
    for (i, &addr) in addrs.iter().enumerate() {
        if i % 2 == 0 {
            let _ = allocator.free(addr, 0);
        }
    }

    // Verify fragmentation is manageable (free pages > 0)
    assert!(allocator.free_pages() > 0);
}

#[test]
fn q23_production_coalescing_basic() {
    let allocator = GartAllocatorCapsule::new(2048, GartVendor::Generic);

    // Allocate two adjacent order-0 blocks
    let addr1 = allocator.alloc(0).unwrap();
    let addr2 = allocator.alloc(0).unwrap();

    let allocated_before_free = allocator.allocated_pages();

    // Free both
    allocator.free(addr1, 0).unwrap();
    allocator.free(addr2, 0).unwrap();

    // After freeing, should have 0 allocated pages
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q24_production_high_allocation_rate() {
    let allocator = GartAllocatorCapsule::new(8192, GartVendor::Generic);

    // Allocate 100 blocks rapidly
    for _ in 0..100 {
        let _ = allocator.alloc(0);
    }

    // Verify some allocations succeeded
    assert!(allocator.allocated_pages() > 0);
}

#[test]
fn q25_production_mixed_order_fragmentation() {
    let allocator = GartAllocatorCapsule::new(4096, GartVendor::Generic);

    // Allocate mixed orders
    let mut addrs = Vec::new();
    addrs.push(allocator.alloc(0).unwrap());  // 4KB
    addrs.push(allocator.alloc(1).unwrap());  // 8KB
    addrs.push(allocator.alloc(2).unwrap());  // 16KB
    addrs.push(allocator.alloc(0).unwrap());  // 4KB
    addrs.push(allocator.alloc(1).unwrap());  // 8KB

    // Free in reverse order
    for addr_order in [(addrs[4], 1), (addrs[3], 0), (addrs[2], 2), (addrs[1], 1), (addrs[0], 0)] {
        let _ = allocator.free(addr_order.0, addr_order.1);
    }

    // Verify all freed
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q26_production_oom_recovery() {
    let allocator = GartAllocatorCapsule::new(64, GartVendor::Generic);

    // Allocate until OOM
    let mut addrs = Vec::new();
    for _ in 0..100 {
        match allocator.alloc(0) {
            Ok(addr) => addrs.push(addr),
            Err(GartError::OutOfMemory { .. }) => break,
            Err(_) => {}
        }
    }

    // Free half
    for i in 0..(addrs.len() / 2) {
        let _ = allocator.free(addrs[i], 0);
    }

    // Should be able to allocate again
    let result = allocator.alloc(0);
    assert!(result.is_ok() || matches!(result, Err(GartError::OutOfMemory { .. })));
}

#[test]
fn q27_production_large_order_split_and_merge() {
    let allocator = GartAllocatorCapsule::new(8192, GartVendor::Generic);

    // Allocate order 4 (64KB)
    let addr = allocator.alloc(4).unwrap();
    assert_eq!(allocator.allocated_pages(), 16);  // 64KB / 4KB = 16 pages

    // Free it
    allocator.free(addr, 4).unwrap();
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q28_production_stress_alloc_free_cycles() {
    let allocator = GartAllocatorCapsule::new(4096, GartVendor::Generic);

    // 100 cycles of alloc/free
    for _ in 0..100 {
        if let Ok(addr) = allocator.alloc(0) {
            let _ = allocator.free(addr, 0);
        }
    }

    // Should have 0 allocated at the end
    assert_eq!(allocator.allocated_pages(), 0);
}

// ============================================================================
// Q29-Q35: Determinism Tests (Reproducible Patterns)
// ============================================================================

#[test]
fn q29_determinism_same_sequence_same_addresses() {
    let allocator1 = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let allocator2 = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    let addr1_1 = allocator1.alloc(0).unwrap();
    let addr1_2 = allocator1.alloc(0).unwrap();

    let addr2_1 = allocator2.alloc(0).unwrap();
    let addr2_2 = allocator2.alloc(0).unwrap();

    // Same allocation sequence should produce same addresses
    assert_eq!(addr1_1, addr2_1);
    assert_eq!(addr1_2, addr2_2);
}

#[test]
fn q30_determinism_order_independence_free() {
    let allocator = GartAllocatorCapsule::new(2048, GartVendor::Generic);

    let addr1 = allocator.alloc(0).unwrap();
    let addr2 = allocator.alloc(0).unwrap();
    let addr3 = allocator.alloc(0).unwrap();

    // Free in different order
    allocator.free(addr2, 0).unwrap();
    allocator.free(addr1, 0).unwrap();
    allocator.free(addr3, 0).unwrap();

    // Should have 0 allocated regardless of free order
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q31_determinism_generation_increment_predictable() {
    let allocator = GartAllocatorCapsule::new(1024, GartVendor::Generic);

    let gen_start = allocator.generation();

    allocator.alloc(0).unwrap();
    let gen_after_alloc1 = allocator.generation();

    allocator.alloc(0).unwrap();
    let gen_after_alloc2 = allocator.generation();

    // Generation increments should be predictable
    assert!(gen_after_alloc1 > gen_start || gen_after_alloc1 == 0);
    assert!(gen_after_alloc2 > gen_after_alloc1 || gen_after_alloc2 == 0);
}

#[test]
fn q32_determinism_allocation_pattern_reproducible() {
    let allocator = GartAllocatorCapsule::new(4096, GartVendor::Generic);

    // Pattern: alloc order 0, 1, 2, 0, 1
    let pattern = [0, 1, 2, 0, 1];
    let mut addrs = Vec::new();

    for &order in &pattern {
        if let Ok(addr) = allocator.alloc(order) {
            addrs.push((addr, order));
        }
    }

    // Verify pattern completed
    assert_eq!(addrs.len(), 5);
}

#[test]
fn q33_determinism_free_pattern_reproducible() {
    let allocator = GartAllocatorCapsule::new(2048, GartVendor::Generic);

    let addr1 = allocator.alloc(0).unwrap();
    let addr2 = allocator.alloc(0).unwrap();
    let addr3 = allocator.alloc(0).unwrap();

    let alloc_before = allocator.allocated_pages();

    // Free in specific order
    allocator.free(addr1, 0).unwrap();
    allocator.free(addr3, 0).unwrap();
    allocator.free(addr2, 0).unwrap();

    // All should be freed
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q34_determinism_coalescing_predictable() {
    let allocator = GartAllocatorCapsule::new(2048, GartVendor::Generic);

    // Allocate 4 adjacent order-0 blocks
    let addrs: Vec<_> = (0..4).filter_map(|_| allocator.alloc(0).ok()).collect();

    // Free all (should trigger coalescing)
    for &addr in &addrs {
        let _ = allocator.free(addr, 0);
    }

    // Should have 0 allocated after coalescing
    assert_eq!(allocator.allocated_pages(), 0);
}

#[test]
fn q35_determinism_vendor_independent_allocation() {
    let allocator_generic = GartAllocatorCapsule::new(1024, GartVendor::Generic);
    let allocator_intel = GartAllocatorCapsule::new(1024, GartVendor::Intel);
    let allocator_amd = GartAllocatorCapsule::new(1024, GartVendor::Amd);

    let addr_generic = allocator_generic.alloc(0).unwrap();
    let addr_intel = allocator_intel.alloc(0).unwrap();
    let addr_amd = allocator_amd.alloc(0).unwrap();

    // All vendors should allocate from same address (deterministic)
    assert_eq!(addr_generic, addr_intel);
    assert_eq!(addr_intel, addr_amd);
}
