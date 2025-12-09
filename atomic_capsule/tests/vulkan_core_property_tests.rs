//! Property Tests for VulkanCoreCapsule (T28 Q8-Q14)
//!
//! # Test Coverage
//!
//! - Q8: Concurrent handle access (multiple readers, single writer)
//! - Q9: Observability counter monotonicity
//! - Q10: State transition validation
//! - Q11: Queue family uniqueness
//! - Q12: Device limits validation
//! - Q13: Memory ordering correctness
//! - Q14: Handle lifecycle safety

#![cfg(all(test, feature = "std"))]

use atomic_capsule::gpu::graphics::{
    VulkanCoreCapsule, VulkanVersion, QueueCapability, PhysicalDeviceType, MemoryProperty,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q8: CONCURRENT HANDLE ACCESS (Multi-threaded stress test)
// ============================================================================

#[test]
fn test_concurrent_handle_reads() {
    // Property: Multiple threads can safely read handles concurrently
    const NUM_THREADS: usize = 16;
    const READS_PER_THREAD: usize = 10_000;

    let capsule = Arc::new(VulkanCoreCapsule::new());

    // Writer thread: Set instance handle
    unsafe {
        capsule.set_instance(0xDEADBEEF, VulkanVersion::V1_3);
        capsule.set_device(0xCAFEBABE);
    }

    // Spawn reader threads
    let mut handles = vec![];
    for _ in 0..NUM_THREADS {
        let capsule: Arc<VulkanCoreCapsule> = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..READS_PER_THREAD {
                assert!(capsule.has_instance());
                assert_eq!(capsule.get_instance(), 0xDEADBEEF);
                assert!(capsule.has_device());
                assert_eq!(capsule.get_device(), 0xCAFEBABE);
                assert_eq!(capsule.api_version(), VulkanVersion::V1_3);
            }
        }));
    }

    // Wait for all readers
    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_counter_increments() {
    // Property: Atomic counters never lose updates under contention
    const NUM_THREADS: usize = 16;
    const INCREMENTS_PER_THREAD: usize = 1_000;

    let capsule = Arc::new(VulkanCoreCapsule::new());
    let mut handles = vec![];

    for _ in 0..NUM_THREADS {
        let capsule: Arc<VulkanCoreCapsule> = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..INCREMENTS_PER_THREAD {
                capsule.increment_commands();
                capsule.increment_allocations();
                capsule.increment_api_calls();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total counts
    assert_eq!(
        capsule.total_commands(),
        (NUM_THREADS * INCREMENTS_PER_THREAD) as u64
    );
    assert_eq!(
        capsule.total_allocations(),
        (NUM_THREADS * INCREMENTS_PER_THREAD) as u64
    );
    assert_eq!(
        capsule.total_api_calls(),
        (NUM_THREADS * INCREMENTS_PER_THREAD) as u64
    );
}

// ============================================================================
// Q9: OBSERVABILITY COUNTER MONOTONICITY
// ============================================================================

#[test]
fn test_counter_monotonicity() {
    // Property: Counters never decrease
    let capsule = VulkanCoreCapsule::new();
    let mut prev_commands = 0u64;
    let mut prev_allocations = 0u64;
    let mut prev_api_calls = 0u64;

    for _ in 0..1000 {
        capsule.increment_commands();
        let curr_commands = capsule.total_commands();
        assert!(curr_commands > prev_commands, "Commands counter decreased");
        prev_commands = curr_commands;

        capsule.increment_allocations();
        let curr_allocations = capsule.total_allocations();
        assert!(
            curr_allocations > prev_allocations,
            "Allocations counter decreased"
        );
        prev_allocations = curr_allocations;

        capsule.increment_api_calls();
        let curr_api_calls = capsule.total_api_calls();
        assert!(curr_api_calls > prev_api_calls, "API calls counter decreased");
        prev_api_calls = curr_api_calls;
    }
}

// ============================================================================
// Q10: STATE TRANSITION VALIDATION
// ============================================================================

#[test]
fn test_state_transition_sequence() {
    // Property: State transitions follow valid Vulkan lifecycle
    let capsule = VulkanCoreCapsule::new();

    // Initial state: No instance, no device
    assert!(!capsule.has_instance());
    assert!(!capsule.has_device());

    // Transition 1: Create instance
    unsafe {
        capsule.set_instance(0x1000, VulkanVersion::V1_3);
    }
    assert!(capsule.has_instance());
    assert!(!capsule.has_device());

    // Transition 2: Select physical device
    unsafe {
        capsule.set_physical_device(0x2000, PhysicalDeviceType::DiscreteGpu);
    }
    assert_eq!(capsule.get_physical_device(), 0x2000);
    assert_eq!(capsule.device_type(), PhysicalDeviceType::DiscreteGpu);

    // Transition 3: Create logical device
    unsafe {
        capsule.set_device(0x3000);
    }
    assert!(capsule.has_device());

    // Transition 4: Create queues
    unsafe {
        capsule.set_queues(0x4000, 0, 0x5000, 1, 0x6000, 2);
    }
    assert_eq!(capsule.get_graphics_queue(), 0x4000);
    assert_eq!(capsule.get_compute_queue(), 0x5000);
    assert_eq!(capsule.get_transfer_queue(), 0x6000);
    assert_eq!(capsule.graphics_family(), 0);
    assert_eq!(capsule.compute_family(), 1);
    assert_eq!(capsule.transfer_family(), 2);
}

#[test]
fn test_handle_zero_initialization() {
    // Property: Uninitialized handles are 0 (VK_NULL_HANDLE)
    let capsule = VulkanCoreCapsule::new();
    assert_eq!(capsule.get_instance(), 0);
    assert_eq!(capsule.get_physical_device(), 0);
    assert_eq!(capsule.get_device(), 0);
    assert_eq!(capsule.get_graphics_queue(), 0);
    assert_eq!(capsule.get_compute_queue(), 0);
    assert_eq!(capsule.get_transfer_queue(), 0);
}

// ============================================================================
// Q11: QUEUE FAMILY UNIQUENESS
// ============================================================================

#[test]
fn test_queue_family_initialization() {
    // Property: Uninitialized queue families are u32::MAX
    let capsule = VulkanCoreCapsule::new();
    assert_eq!(capsule.graphics_family(), u32::MAX);
    assert_eq!(capsule.compute_family(), u32::MAX);
    assert_eq!(capsule.transfer_family(), u32::MAX);
}

#[test]
fn test_queue_family_distinct_indices() {
    // Property: Queue families can have distinct or shared indices
    let capsule = VulkanCoreCapsule::new();

    // Scenario 1: Distinct families (e.g., AMD discrete GPU)
    unsafe {
        capsule.set_queues(0x1000, 0, 0x2000, 1, 0x3000, 2);
    }
    assert_eq!(capsule.graphics_family(), 0);
    assert_eq!(capsule.compute_family(), 1);
    assert_eq!(capsule.transfer_family(), 2);

    // Scenario 2: Shared graphics+compute (e.g., integrated GPU)
    let capsule2 = VulkanCoreCapsule::new();
    unsafe {
        capsule2.set_queues(0x1000, 0, 0x1000, 0, 0x3000, 1);
    }
    assert_eq!(capsule2.graphics_family(), 0);
    assert_eq!(capsule2.compute_family(), 0);
    assert_eq!(capsule2.transfer_family(), 1);

    // Scenario 3: All shared (rare, but valid)
    let capsule3 = VulkanCoreCapsule::new();
    unsafe {
        capsule3.set_queues(0x1000, 0, 0x1000, 0, 0x1000, 0);
    }
    assert_eq!(capsule3.graphics_family(), 0);
    assert_eq!(capsule3.compute_family(), 0);
    assert_eq!(capsule3.transfer_family(), 0);
}

// ============================================================================
// Q12: DEVICE LIMITS VALIDATION
// ============================================================================

#[test]
fn test_device_limits_storage() {
    // Property: Device limits are stored accurately
    let capsule = VulkanCoreCapsule::new();

    unsafe {
        capsule.set_limits(
            [65535, 65535, 65535],        // max work group count
            [1024, 1024, 64],              // max work group size
            256,                           // max push constants
            4096,                          // max memory allocations
        );
    }

    assert_eq!(capsule.max_work_group_count(), &[65535, 65535, 65535]);
    assert_eq!(capsule.max_work_group_size(), &[1024, 1024, 64]);
    assert_eq!(capsule.max_push_constants_size(), 256);
    assert_eq!(capsule.max_memory_allocation_count(), 4096);
}

#[test]
fn test_device_limits_zero_initialization() {
    // Property: Limits start at 0 before set_limits()
    let capsule = VulkanCoreCapsule::new();
    assert_eq!(capsule.max_work_group_count(), &[0, 0, 0]);
    assert_eq!(capsule.max_work_group_size(), &[0, 0, 0]);
    assert_eq!(capsule.max_push_constants_size(), 0);
    assert_eq!(capsule.max_memory_allocation_count(), 0);
}

// ============================================================================
// Q13: MEMORY ORDERING CORRECTNESS
// ============================================================================

#[test]
fn test_release_acquire_semantics() {
    // Property: Release store synchronizes with Acquire load
    let capsule = Arc::new(VulkanCoreCapsule::new());

    let writer = {
        let capsule: Arc<VulkanCoreCapsule> = Arc::clone(&capsule);
        thread::spawn(move || {
            unsafe {
                capsule.set_instance(0x12345678, VulkanVersion::V1_3);
                // Release store ensures all prior writes are visible
            }
        })
    };

    writer.join().unwrap();

    // Reader: Acquire load synchronizes with Release store
    assert_eq!(capsule.get_instance(), 0x12345678);
    assert_eq!(capsule.api_version(), VulkanVersion::V1_3);
}

#[test]
fn test_relaxed_ordering_counters() {
    // Property: Relaxed counters eventually become visible
    let capsule = Arc::new(VulkanCoreCapsule::new());

    let writer = {
        let capsule: Arc<VulkanCoreCapsule> = Arc::clone(&capsule);
        thread::spawn(move || {
            for _ in 0..1000 {
                capsule.increment_commands();
            }
        })
    };

    writer.join().unwrap();

    // Eventually consistent: Relaxed load sees final value
    assert_eq!(capsule.total_commands(), 1000);
}

// ============================================================================
// Q14: HANDLE LIFECYCLE SAFETY
// ============================================================================

#[test]
fn test_handle_overwrite_safety() {
    // Property: Handles can be safely overwritten (e.g., device recreation)
    let capsule = VulkanCoreCapsule::new();

    // First lifecycle
    unsafe {
        capsule.set_instance(0x1000, VulkanVersion::V1_2);
        capsule.set_device(0x2000);
    }
    assert_eq!(capsule.get_instance(), 0x1000);
    assert_eq!(capsule.get_device(), 0x2000);

    // Second lifecycle (device recreation)
    unsafe {
        capsule.set_instance(0x3000, VulkanVersion::V1_3);
        capsule.set_device(0x4000);
    }
    assert_eq!(capsule.get_instance(), 0x3000);
    assert_eq!(capsule.get_device(), 0x4000);
    assert_eq!(capsule.api_version(), VulkanVersion::V1_3);
}

#[test]
fn test_partial_initialization_safety() {
    // Property: Capsule is safe to query at any initialization stage
    let capsule = VulkanCoreCapsule::new();

    // Stage 0: Uninitialized
    assert!(!capsule.has_instance());
    assert!(!capsule.has_device());

    // Stage 1: Instance only
    unsafe {
        capsule.set_instance(0x1000, VulkanVersion::V1_3);
    }
    assert!(capsule.has_instance());
    assert!(!capsule.has_device());

    // Stage 2: Instance + physical device
    unsafe {
        capsule.set_physical_device(0x2000, PhysicalDeviceType::IntegratedGpu);
    }
    assert!(capsule.has_instance());
    assert!(!capsule.has_device());

    // Stage 3: Full initialization
    unsafe {
        capsule.set_device(0x3000);
        capsule.set_queues(0x4000, 0, 0x5000, 1, 0x6000, 2);
    }
    assert!(capsule.has_instance());
    assert!(capsule.has_device());
}

// ============================================================================
// ADDITIONAL PROPERTY TESTS
// ============================================================================

#[test]
fn test_vulkan_version_comparison() {
    // Property: Version comparison respects semantic versioning
    assert!(VulkanVersion::V1_3 as u32 > VulkanVersion::V1_2 as u32);
    assert!(VulkanVersion::V1_2 as u32 > VulkanVersion::V1_1 as u32);
    assert!(VulkanVersion::V1_1 as u32 > VulkanVersion::V1_0 as u32);
}

#[test]
fn test_device_type_selection_order() {
    // Property: Discrete GPU always preferred over integrated
    let types = [
        PhysicalDeviceType::Cpu,
        PhysicalDeviceType::VirtualGpu,
        PhysicalDeviceType::IntegratedGpu,
        PhysicalDeviceType::DiscreteGpu,
        PhysicalDeviceType::Other,
    ];

    let mut sorted_types = types.clone();
    sorted_types.sort_by_key(|t: &PhysicalDeviceType| std::cmp::Reverse(t.selection_score()));

    assert_eq!(sorted_types[0], PhysicalDeviceType::DiscreteGpu);
    assert_eq!(sorted_types[1], PhysicalDeviceType::IntegratedGpu);
    assert_eq!(sorted_types[2], PhysicalDeviceType::VirtualGpu);
    assert_eq!(sorted_types[3], PhysicalDeviceType::Cpu);
    assert_eq!(sorted_types[4], PhysicalDeviceType::Other);
}

#[test]
fn test_queue_capability_bitmask_all_combinations() {
    // Property: All capability flag combinations are valid
    let capabilities = [
        QueueCapability::Graphics,
        QueueCapability::Compute,
        QueueCapability::Transfer,
        QueueCapability::SparseBinding,
        QueueCapability::Protected,
    ];

    // Test all 2^5 = 32 combinations
    for i in 0..32u32 {
        let flags = (0..5)
            .filter(|bit| (i & (1 << bit)) != 0)
            .map(|bit| capabilities[bit] as u32)
            .fold(0u32, |acc, cap| acc | cap);

        // Verify each capability in the combination
        for (bit, cap) in capabilities.iter().enumerate() {
            let expected = (i & (1 << bit)) != 0;
            assert_eq!(QueueCapability::is_set(flags, *cap), expected);
        }
    }
}

#[test]
fn test_memory_property_bitmask_all_combinations() {
    // Property: All memory property combinations are valid
    let properties = [
        MemoryProperty::DeviceLocal,
        MemoryProperty::HostVisible,
        MemoryProperty::HostCoherent,
        MemoryProperty::HostCached,
        MemoryProperty::LazilyAllocated,
    ];

    // Test common combinations
    let common_combinations = [
        (MemoryProperty::DeviceLocal as u32, vec![MemoryProperty::DeviceLocal]),
        (
            MemoryProperty::HostVisible as u32 | MemoryProperty::HostCoherent as u32,
            vec![MemoryProperty::HostVisible, MemoryProperty::HostCoherent],
        ),
        (
            MemoryProperty::HostVisible as u32 | MemoryProperty::HostCached as u32,
            vec![MemoryProperty::HostVisible, MemoryProperty::HostCached],
        ),
    ];

    for (flags, expected_props) in common_combinations.iter() {
        for prop in &properties {
            let expected: bool = expected_props.iter().any(|p: &MemoryProperty| p == prop);
            assert_eq!(MemoryProperty::is_set(*flags, *prop), expected);
        }
    }
}
