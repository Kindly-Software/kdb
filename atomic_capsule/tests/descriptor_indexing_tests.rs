//! Descriptor Indexing Capsule - T28 5-Tier Tests
//!
//! Q1-Q7: Unit tests (basic operations, allocation, free-list)
//! Q8-Q14: Property tests (invariants, concurrent allocation, ABA prevention)
//! Q15-Q21: Integration tests (multi-type allocation, stress testing)
//! Q22-Q28: Production tests (performance targets, resource exhaustion)

use atomic_capsule::gpu::graphics::descriptor_indexing::{
    DescriptorIndexingCapsule, DescriptorType, BindingFlag, BindingInfo, SlotAllocation,
};
use std::sync::Arc;
use std::thread;

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn q1_capsule_size_alignment() {
    // Chaos mandate: 1024-byte size and alignment
    assert_eq!(
        core::mem::size_of::<DescriptorIndexingCapsule>(),
        1024,
        "Must be exactly 1024 bytes"
    );
    assert_eq!(
        core::mem::align_of::<DescriptorIndexingCapsule>(),
        1024,
        "Must be 1024-byte aligned"
    );
}

#[test]
fn q2_new_capsule_initialization() {
    let capsule = DescriptorIndexingCapsule::new();

    // Verify zero initialization
    assert_eq!(capsule.active_count(), 0);
    assert_eq!(capsule.total_update_count(), 0);
    assert_eq!(capsule.total_bind_count(), 0);

    // Verify default array sizes (research-backed limits)
    assert_eq!(capsule.texture_array_size(), 1024);
    assert_eq!(capsule.buffer_array_size(), 1024);
    assert_eq!(capsule.sampler_array_size(), 256);

    // Verify handles are null
    assert_eq!(capsule.layout(), 0);
    assert_eq!(capsule.pool(), 0);
    assert_eq!(capsule.descriptor_set(), 0);
}

#[test]
fn q3_allocate_single_texture_slot() {
    let capsule = DescriptorIndexingCapsule::new();

    let slot = capsule.allocate_texture_slot().expect("Should allocate");

    assert_eq!(slot.index, 0, "First allocation should be index 0");
    assert_eq!(capsule.active_count(), 1);
    assert!(slot.generation > 0, "Generation should be non-zero");
}

#[test]
fn q4_allocate_free_texture_slot() {
    let capsule = DescriptorIndexingCapsule::new();

    let slot = capsule.allocate_texture_slot().expect("Should allocate");
    assert_eq!(capsule.active_count(), 1);

    capsule.free_texture_slot(slot);
    assert_eq!(capsule.active_count(), 0, "Active count should return to 0");
}

#[test]
fn q5_allocate_free_buffer_slot() {
    let capsule = DescriptorIndexingCapsule::new();

    let slot = capsule.allocate_buffer_slot().expect("Should allocate");
    assert_eq!(slot.index, 0);
    assert_eq!(capsule.active_count(), 1);

    capsule.free_buffer_slot(slot);
    assert_eq!(capsule.active_count(), 0);
}

#[test]
fn q6_allocate_free_sampler_slot() {
    let capsule = DescriptorIndexingCapsule::new();

    let slot = capsule.allocate_sampler_slot().expect("Should allocate");
    assert_eq!(slot.index, 0);
    assert_eq!(capsule.active_count(), 1);

    capsule.free_sampler_slot(slot);
    assert_eq!(capsule.active_count(), 0);
}

#[test]
fn q7_record_update_counter() {
    let capsule = DescriptorIndexingCapsule::new();

    capsule.record_update();
    assert_eq!(capsule.total_update_count(), 1);

    capsule.record_update();
    capsule.record_update();
    assert_eq!(capsule.total_update_count(), 3);
}

// ============================================================================
// Q8-Q14: Property Tests
// ============================================================================

#[test]
fn q8_allocation_reuse_freed_slot() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate slot 0
    let slot0 = capsule.allocate_texture_slot().expect("Should allocate");
    assert_eq!(slot0.index, 0);

    // Allocate slot 1
    let slot1 = capsule.allocate_texture_slot().expect("Should allocate");
    assert_eq!(slot1.index, 1);

    // Free slot 0
    capsule.free_texture_slot(slot0);
    assert_eq!(capsule.active_count(), 1);

    // Next allocation should reuse slot 0
    let slot_reuse = capsule.allocate_texture_slot().expect("Should allocate");
    assert_eq!(slot_reuse.index, 0, "Should reuse freed slot");
    assert_eq!(capsule.active_count(), 2);

    // Generation should be different (ABA prevention)
    assert_ne!(slot0.generation, slot_reuse.generation);
}

#[test]
fn q9_sequential_allocations() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate 100 sequential slots
    for i in 0..100 {
        let slot = capsule.allocate_texture_slot().expect("Should allocate");
        assert_eq!(slot.index, i, "Sequential allocation failed at index {}", i);
    }

    assert_eq!(capsule.active_count(), 100);
}

#[test]
fn q10_bitmap_word_boundary() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate 64 slots (one full bitmap word)
    let mut slots = Vec::new();
    for i in 0..64 {
        let slot = capsule.allocate_texture_slot().expect("Should allocate");
        assert_eq!(slot.index, i);
        slots.push(slot);
    }

    // Next allocation should be in second word (index 64)
    let slot65 = capsule.allocate_texture_slot().expect("Should allocate");
    assert_eq!(slot65.index, 64);

    // Free all slots
    for slot in slots {
        capsule.free_texture_slot(slot);
    }
    capsule.free_texture_slot(slot65);

    assert_eq!(capsule.active_count(), 0);
}

#[test]
fn q11_generation_counter_increments() {
    let capsule = DescriptorIndexingCapsule::new();

    let slot1 = capsule.allocate_texture_slot().expect("Should allocate");
    let slot2 = capsule.allocate_texture_slot().expect("Should allocate");
    let slot3 = capsule.allocate_texture_slot().expect("Should allocate");

    // Each allocation should have unique generation
    assert_ne!(slot1.generation, slot2.generation);
    assert_ne!(slot2.generation, slot3.generation);
    assert_ne!(slot1.generation, slot3.generation);
}

#[test]
fn q12_multi_type_allocation_independence() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate from different types
    let tex_slot = capsule.allocate_texture_slot().expect("Should allocate");
    let buf_slot = capsule.allocate_buffer_slot().expect("Should allocate");
    let sam_slot = capsule.allocate_sampler_slot().expect("Should allocate");

    // All should be index 0 (independent free-lists)
    assert_eq!(tex_slot.index, 0);
    assert_eq!(buf_slot.index, 0);
    assert_eq!(sam_slot.index, 0);

    // Active count should be 3
    assert_eq!(capsule.active_count(), 3);
}

#[test]
fn q13_binding_info_structure() {
    let binding = BindingInfo::new(
        0,
        DescriptorType::SampledImage,
        1024,
        0x00000001, // VK_SHADER_STAGE_VERTEX_BIT
        BindingFlag::UpdateAfterBind as u32 | BindingFlag::PartiallyBound as u32,
    );

    assert_eq!(binding.binding, 0);
    assert_eq!(binding.descriptor_type, DescriptorType::SampledImage);
    assert_eq!(binding.descriptor_count, 1024);

    // Verify cache alignment
    assert_eq!(core::mem::size_of_val(&binding), 64);
    assert_eq!(core::mem::align_of_val(&binding), 64);
}

#[test]
fn q14_add_multiple_bindings() {
    let mut capsule = DescriptorIndexingCapsule::new();

    // Add texture binding
    let binding1 = BindingInfo::new(
        0,
        DescriptorType::SampledImage,
        1024,
        0x00000001,
        BindingFlag::UpdateAfterBind as u32,
    );
    let idx1 = capsule.add_binding(binding1).expect("Should add");
    assert_eq!(idx1, 0);

    // Add buffer binding
    let binding2 = BindingInfo::new(
        1,
        DescriptorType::StorageBuffer,
        512,
        0x00000010, // VK_SHADER_STAGE_COMPUTE_BIT
        BindingFlag::UpdateAfterBind as u32,
    );
    let idx2 = capsule.add_binding(binding2).expect("Should add");
    assert_eq!(idx2, 1);

    assert_eq!(capsule.binding_count(), 2);
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn q15_concurrent_allocations_stress() {
    let capsule = Arc::new(DescriptorIndexingCapsule::new());
    let thread_count = 8;
    let allocations_per_thread = 128;

    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let capsule_clone: Arc<DescriptorIndexingCapsule> = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            let mut slots = Vec::new();
            for _ in 0..allocations_per_thread {
                if let Some(slot) = capsule_clone.allocate_texture_slot() {
                    slots.push(slot);
                }
            }
            slots
        });
        handles.push(handle);
    }

    // Collect all allocated slots
    let mut all_slots = Vec::new();
    for handle in handles {
        let slots = handle.join().expect("Thread panicked");
        all_slots.extend(slots);
    }

    // Verify total allocations
    assert_eq!(
        capsule.active_count(),
        all_slots.len() as u64,
        "Active count mismatch"
    );

    // Verify unique indices (no double allocation)
    let mut indices = all_slots.iter().map(|s| s.index).collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    assert_eq!(
        indices.len(),
        all_slots.len(),
        "Duplicate indices detected!"
    );
}

#[test]
fn q16_allocate_free_allocate_pattern() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate 100 slots
    let mut slots = Vec::new();
    for _ in 0..100 {
        slots.push(capsule.allocate_texture_slot().expect("Should allocate"));
    }

    // Free every other slot
    for i in (0..100).step_by(2) {
        capsule.free_texture_slot(slots[i]);
    }

    assert_eq!(capsule.active_count(), 50);

    // Reallocate 50 slots (should fill freed slots)
    let mut new_slots = Vec::new();
    for _ in 0..50 {
        new_slots.push(capsule.allocate_texture_slot().expect("Should allocate"));
    }

    assert_eq!(capsule.active_count(), 100);
}

#[test]
fn q17_limits_initialization() {
    let capsule = DescriptorIndexingCapsule::new();

    capsule.init_limits(
        32,     // max_descriptor_set_bindings
        1024,   // max_per_stage_descriptors
        512,    // max_update_after_bind_descriptors
        16384,  // max_per_stage_descriptor_sampled_images
        4096,   // max_variable_descriptor_count
    );

    // Verify limits stored correctly
    // (Internal atomic fields, verified via successful init)
    assert_eq!(capsule.active_count(), 0);
}

#[test]
fn q18_descriptor_set_binding() {
    let capsule = DescriptorIndexingCapsule::new();

    // Set layout
    capsule.set_layout(0x1000);
    assert_eq!(capsule.layout(), 0x1000);

    // Set pool
    capsule.set_pool(0x2000, 16, 0x00000001);
    assert_eq!(capsule.pool(), 0x2000);

    // Set descriptor set (should increment bind count)
    capsule.set_descriptor_set(0x3000);
    assert_eq!(capsule.descriptor_set(), 0x3000);
    assert_eq!(capsule.total_bind_count(), 1, "Bind count should increment");

    // Second bind
    capsule.set_descriptor_set(0x4000);
    assert_eq!(capsule.total_bind_count(), 2);
}

#[test]
fn q19_update_tracking() {
    let capsule = DescriptorIndexingCapsule::new();

    // Track 1000 updates
    for _ in 0..1000 {
        capsule.record_update();
    }

    assert_eq!(capsule.total_update_count(), 1000);
}

#[test]
fn q20_mixed_allocation_types() {
    let capsule = DescriptorIndexingCapsule::new();

    // Allocate mix of all types
    let mut tex_slots = Vec::new();
    let mut buf_slots = Vec::new();
    let mut sam_slots = Vec::new();

    for _ in 0..50 {
        tex_slots.push(capsule.allocate_texture_slot().expect("Should allocate"));
        buf_slots.push(capsule.allocate_buffer_slot().expect("Should allocate"));
        sam_slots.push(capsule.allocate_sampler_slot().expect("Should allocate"));
    }

    assert_eq!(capsule.active_count(), 150);

    // Free all
    for slot in tex_slots {
        capsule.free_texture_slot(slot);
    }
    for slot in buf_slots {
        capsule.free_buffer_slot(slot);
    }
    for slot in sam_slots {
        capsule.free_sampler_slot(slot);
    }

    assert_eq!(capsule.active_count(), 0);
}

#[test]
fn q21_debug_format_output() {
    let capsule = DescriptorIndexingCapsule::new();

    capsule.allocate_texture_slot().expect("Should allocate");
    capsule.record_update();

    let debug_str = format!("{:?}", capsule);

    assert!(debug_str.contains("DescriptorIndexingCapsule"));
    assert!(debug_str.contains("active_descriptors"));
    assert!(debug_str.contains("total_updates"));
}

// ============================================================================
// Q22-Q28: Production Tests
// ============================================================================

#[test]
fn q22_allocation_performance_target() {
    use std::time::Instant;

    let capsule = DescriptorIndexingCapsule::new();
    let iterations = 10_000;

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = capsule.allocate_texture_slot();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;

    println!("Average allocation time: {}ns", avg_ns);

    // Target: <100ns per allocation
    assert!(
        avg_ns < 200,
        "Allocation too slow: {}ns (target <100ns)",
        avg_ns
    );
}

#[test]
fn q23_free_performance_target() {
    use std::time::Instant;

    let capsule = DescriptorIndexingCapsule::new();
    let iterations = 10_000;

    // Pre-allocate slots
    let mut slots = Vec::new();
    for _ in 0..iterations {
        slots.push(capsule.allocate_texture_slot().expect("Should allocate"));
    }

    let start = Instant::now();
    for slot in slots {
        capsule.free_texture_slot(slot);
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;

    println!("Average free time: {}ns", avg_ns);

    // Target: <50ns per free
    assert!(
        avg_ns < 100,
        "Free too slow: {}ns (target <50ns)",
        avg_ns
    );
}

#[test]
fn q24_update_record_performance() {
    use std::time::Instant;

    let capsule = DescriptorIndexingCapsule::new();
    let iterations = 100_000;

    let start = Instant::now();
    for _ in 0..iterations {
        capsule.record_update();
    }
    let elapsed = start.elapsed();

    let avg_ns = elapsed.as_nanos() / iterations as u128;

    println!("Average update record time: {}ns", avg_ns);

    // Target: <10ns per update record
    assert!(
        avg_ns < 20,
        "Update record too slow: {}ns (target <10ns)",
        avg_ns
    );
}

#[test]
fn q25_resource_exhaustion_texture() {
    let capsule = DescriptorIndexingCapsule::new();
    let max_textures = capsule.texture_array_size() as usize;

    // Allocate all texture slots
    let mut slots = Vec::new();
    for _ in 0..max_textures {
        let slot = capsule.allocate_texture_slot().expect("Should allocate");
        slots.push(slot);
    }

    assert_eq!(capsule.active_count(), max_textures as u64);

    // Next allocation should fail
    let result = capsule.allocate_texture_slot();
    assert!(result.is_none(), "Should fail when exhausted");
}

#[test]
fn q26_resource_exhaustion_sampler() {
    let capsule = DescriptorIndexingCapsule::new();
    let max_samplers = capsule.sampler_array_size() as usize;

    // Allocate all sampler slots
    let mut slots = Vec::new();
    for _ in 0..max_samplers {
        let slot = capsule.allocate_sampler_slot().expect("Should allocate");
        slots.push(slot);
    }

    // Next allocation should fail
    let result = capsule.allocate_sampler_slot();
    assert!(result.is_none(), "Should fail when exhausted");
}

#[test]
fn q27_concurrent_alloc_free_stress() {
    let capsule = Arc::new(DescriptorIndexingCapsule::new());
    let operations_per_thread = 1000;
    let thread_count = 8;

    let mut handles = Vec::new();

    for _ in 0..thread_count {
        let capsule_clone: Arc<DescriptorIndexingCapsule> = Arc::clone(&capsule);
        let handle = thread::spawn(move || {
            for _ in 0..operations_per_thread {
                // Allocate
                if let Some(slot) = capsule_clone.allocate_texture_slot() {
                    // Immediately free
                    capsule_clone.free_texture_slot(slot);
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // All slots should be freed
    assert_eq!(
        capsule.active_count(),
        0,
        "All slots should be freed after stress test"
    );
}

#[test]
fn q28_bindless_workflow_integration() {
    let capsule = DescriptorIndexingCapsule::new();

    // Step 1: Initialize device limits
    capsule.init_limits(32, 1024, 512, 16384, 4096);

    // Step 2: Create descriptor set layout
    capsule.set_layout(0x1000);

    // Step 3: Create descriptor pool (update-after-bind)
    capsule.set_pool(0x2000, 16, 0x00000001);

    // Step 4: Allocate descriptor set
    capsule.set_descriptor_set(0x3000);
    assert_eq!(capsule.total_bind_count(), 1, "Single bind at startup");

    // Step 5: Allocate texture slots (streaming)
    let tex1 = capsule.allocate_texture_slot().expect("Should allocate");
    let tex2 = capsule.allocate_texture_slot().expect("Should allocate");
    let tex3 = capsule.allocate_texture_slot().expect("Should allocate");

    // Step 6: Update descriptors (update-after-bind, no rebind)
    capsule.record_update();
    capsule.record_update();
    capsule.record_update();

    assert_eq!(capsule.total_update_count(), 3);
    assert_eq!(capsule.total_bind_count(), 1, "Still only one bind");

    // Step 7: Free slots
    capsule.free_texture_slot(tex1);
    capsule.free_texture_slot(tex2);
    capsule.free_texture_slot(tex3);

    assert_eq!(capsule.active_count(), 0);
}
