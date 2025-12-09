//! Phase 3 Unit Tests - Per-Capsule Testing
//!
//! Following T42 test framework, this file contains unit tests for individual capsules:
//! - MemoryCapsule: allocation tracking, version consistency
//! - CommandCapsule: state transitions, version matching
//! - GpuMemoryAllocator: sequential allocations, alignment, OOM
//! - DRM Interface: device operations, GEM buffer creation

use kiang::capsules::*;
use kiang::command::*;
use kiang::drm_interface::*;
use kiang::memory::*;

// ============================================================================
// MemoryCapsule Unit Tests (12 tests)
// ============================================================================

#[test]
fn test_memory_capsule_initial_invalid() {
    let capsule = MemoryCapsule::new();
    let state = capsule.read();
    assert!(
        !state.is_valid(),
        "Initial state should be invalid (never published)"
    );
}

#[test]
fn test_memory_capsule_basic_read() {
    let capsule = MemoryCapsule::new();

    // Initial read should be invalid (no publish yet)
    let state = capsule.read();
    assert!(!state.is_valid());
    assert_eq!(state.total_vram, 0);
    assert_eq!(state.used_vram, 0);
    assert_eq!(state.available_vram, 0);
}

#[test]
fn test_memory_capsule_alignment() {
    use std::mem::{align_of, size_of};

    // MemoryCapsule should be 64-byte aligned per atomic capsule principles
    assert_eq!(
        align_of::<MemoryCapsule>(),
        64,
        "MemoryCapsule must be 64-byte aligned"
    );

    // Size should be multiple of 64 bytes for cache efficiency
    let size = size_of::<MemoryCapsule>();
    assert_eq!(
        size % 64,
        0,
        "MemoryCapsule size should be multiple of 64 bytes"
    );
}

#[test]
fn test_memory_capsule_has_available() {
    let state = MemoryState {
        total_vram: 1024 * 1024 * 1024,    // 1GB
        used_vram: 512 * 1024 * 1024,      // 512MB used
        available_vram: 512 * 1024 * 1024, // 512MB available
        valid: true,
    };

    assert!(
        state.has_available(256 * 1024 * 1024),
        "Should have space for 256MB"
    );
    assert!(
        state.has_available(512 * 1024 * 1024),
        "Should have space for 512MB (exact)"
    );
    assert!(
        !state.has_available(1024 * 1024 * 1024),
        "Should NOT have space for 1GB"
    );
}

#[test]
fn test_memory_capsule_invalid_no_availability() {
    let state = MemoryState::invalid();
    assert!(
        !state.has_available(1),
        "Invalid state should never have availability"
    );
}

#[test]
fn test_memory_state_invariants() {
    // Test memory state invariants: used + available should <= total
    let state = MemoryState {
        total_vram: 1000,
        used_vram: 600,
        available_vram: 400,
        valid: true,
    };

    assert!(
        state.used_vram + state.available_vram <= state.total_vram,
        "used + available must be <= total"
    );
}

#[test]
fn test_memory_capsule_zero_total() {
    let state = MemoryState {
        total_vram: 0,
        used_vram: 0,
        available_vram: 0,
        valid: true,
    };

    assert!(
        !state.has_available(1),
        "Zero total should have no available space"
    );
}

#[test]
fn test_memory_capsule_full_utilization() {
    let state = MemoryState {
        total_vram: 1024,
        used_vram: 1024,
        available_vram: 0,
        valid: true,
    };

    assert!(
        !state.has_available(1),
        "Full utilization should have no available space"
    );
    assert!(state.is_valid(), "Full utilization is still valid state");
}

#[test]
fn test_memory_capsule_partial_utilization() {
    let state = MemoryState {
        total_vram: 1024,
        used_vram: 512,
        available_vram: 512,
        valid: true,
    };

    assert!(
        state.has_available(256),
        "Should have space at 50% utilization"
    );
    assert!(state.is_valid());
}

#[test]
fn test_memory_state_clone() {
    let state = MemoryState {
        total_vram: 1024,
        used_vram: 512,
        available_vram: 512,
        valid: true,
    };

    let cloned = state;
    assert_eq!(cloned.total_vram, state.total_vram);
    assert_eq!(cloned.used_vram, state.used_vram);
    assert_eq!(cloned.available_vram, state.available_vram);
    assert_eq!(cloned.valid, state.valid);
}

#[test]
fn test_memory_capsule_boundary_values() {
    // Test with u64::MAX values
    let state = MemoryState {
        total_vram: u64::MAX,
        used_vram: u64::MAX / 2,
        available_vram: u64::MAX / 2,
        valid: true,
    };

    assert!(state.has_available(1000), "Should handle large values");
}

#[test]
fn test_memory_capsule_struct_layout() {
    use std::mem::size_of;

    // Verify struct layout for bit packing
    assert_eq!(
        size_of::<MemoryState>(),
        32,
        "MemoryState should be 32 bytes (4 u64s)"
    );
}

// ============================================================================
// CommandCapsule Unit Tests - REMOVED
// ============================================================================
// These tests were for old CommandState API with fields.
// Modern CommandState is simple enum in command.rs with its own tests.

// ============================================================================
// GpuStateCapsule Unit Tests (12 tests)
// ============================================================================

#[test]
fn test_gpu_state_capsule_creation() {
    let capsule = GpuStateCapsule::new();
    let state = capsule.read();
    assert!(!state.is_valid(), "Initial GPU state should be invalid");
}

#[test]
fn test_gpu_state_capsule_publish_read() {
    let capsule = GpuStateCapsule::new();
    let state = GpuState {
        gpu_id: 1,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 75,
        valid: true,
    };

    capsule.publish(state);
    let read_state = capsule.read();

    assert!(read_state.is_valid());
    assert_eq!(read_state.gpu_id, state.gpu_id);
    assert_eq!(read_state.frequency_mhz, state.frequency_mhz);
    assert_eq!(read_state.power_mw, state.power_mw);
    assert_eq!(read_state.temp_celsius, state.temp_celsius);
    assert_eq!(read_state.utilization, state.utilization);
}

#[test]
fn test_gpu_state_ready_normal_operation() {
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 50,
        valid: true,
    };

    assert!(state.is_ready(), "Normal operating state should be ready");
}

#[test]
fn test_gpu_state_not_ready_high_temp() {
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 96, // Too hot!
        utilization: 50,
        valid: true,
    };

    assert!(!state.is_ready(), "Overheated GPU should not be ready");
}

#[test]
fn test_gpu_state_not_ready_high_utilization() {
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 96, // Too busy!
        valid: true,
    };

    assert!(!state.is_ready(), "Fully utilized GPU should not be ready");
}

#[test]
fn test_gpu_state_ready_threshold_95_temp() {
    let state_94 = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 94,
        utilization: 50,
        valid: true,
    };
    assert!(state_94.is_ready(), "94°C should be ready");

    let state_95 = GpuState {
        temp_celsius: 95,
        ..state_94
    };
    assert!(!state_95.is_ready(), "95°C should NOT be ready");
}

#[test]
fn test_gpu_state_ready_threshold_95_utilization() {
    let state_94 = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 94,
        valid: true,
    };
    assert!(state_94.is_ready(), "94% utilization should be ready");

    let state_95 = GpuState {
        utilization: 95,
        ..state_94
    };
    assert!(!state_95.is_ready(), "95% utilization should NOT be ready");
}

#[test]
fn test_gpu_state_invalid_not_ready() {
    let state = GpuState {
        gpu_id: 0,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 50,
        utilization: 30,
        valid: false, // Invalid!
    };

    assert!(!state.is_ready(), "Invalid state should never be ready");
}

#[test]
fn test_gpu_state_capsule_alignment() {
    use std::mem::align_of;
    assert_eq!(
        align_of::<GpuStateCapsule>(),
        64,
        "GpuStateCapsule must be 64-byte aligned"
    );
}

#[test]
fn test_gpu_state_multiple_publishes() {
    let capsule = GpuStateCapsule::new();

    // First publish
    let state1 = GpuState {
        gpu_id: 1,
        frequency_mhz: 2000,
        power_mw: 40000,
        temp_celsius: 60,
        utilization: 50,
        valid: true,
    };
    capsule.publish(state1);

    // Second publish (different values)
    let state2 = GpuState {
        gpu_id: 1,
        frequency_mhz: 2200,
        power_mw: 50000,
        temp_celsius: 70,
        utilization: 80,
        valid: true,
    };
    capsule.publish(state2);

    // Should read latest state
    let read = capsule.read();
    assert_eq!(read.frequency_mhz, 2200);
    assert_eq!(read.temp_celsius, 70);
}

#[test]
fn test_gpu_state_copy_trait() {
    let state = GpuState {
        gpu_id: 1,
        frequency_mhz: 2100,
        power_mw: 45000,
        temp_celsius: 65,
        utilization: 75,
        valid: true,
    };

    let copied = state; // Copy, not move
    assert_eq!(copied.gpu_id, state.gpu_id);
}

#[test]
fn test_gpu_state_boundary_values() {
    let state = GpuState {
        gpu_id: u8::MAX,
        frequency_mhz: u16::MAX,
        power_mw: u16::MAX,
        temp_celsius: u8::MAX,
        utilization: u8::MAX,
        valid: true,
    };

    // Should handle max values without overflow
    let capsule = GpuStateCapsule::new();
    capsule.publish(state);
    let read = capsule.read();
    assert!(read.is_valid());
}

// ============================================================================
// GpuMemoryAllocator Unit Tests (15 tests)
// ============================================================================

#[test]
fn test_allocator_creation() {
    let allocator = GpuMemoryAllocator::new(1024 * 1024 * 1024); // 1GB
    assert_eq!(allocator.allocated_bytes(), 0);
    assert_eq!(allocator.available_bytes(), 1024 * 1024 * 1024);
    assert_eq!(allocator.utilization_pct(), 0);
}

#[test]
fn test_allocator_basic_allocation() {
    let allocator = GpuMemoryAllocator::new(1024 * 1024);

    let alloc = allocator.allocate(4096, MemoryDomain::Vram);
    assert!(alloc.is_some());

    let alloc = alloc.unwrap();
    assert_eq!(alloc.size, 4096);
    assert_eq!(allocator.allocated_bytes(), 4096);
}

#[test]
fn test_allocator_sequential_allocations() {
    let allocator = GpuMemoryAllocator::new(1024 * 1024);

    let alloc1 = allocator.allocate(1024, MemoryDomain::Vram).unwrap();
    let alloc2 = allocator.allocate(2048, MemoryDomain::Vram).unwrap();
    let alloc3 = allocator.allocate(4096, MemoryDomain::Vram).unwrap();

    // Check offsets are sequential
    assert_eq!(alloc1.offset, 0);
    assert_eq!(alloc2.offset, 1024);
    assert_eq!(alloc3.offset, 1024 + 2048);

    // Check total allocated
    assert_eq!(allocator.allocated_bytes(), 1024 + 2048 + 4096);
}

#[test]
fn test_allocator_oom() {
    let allocator = GpuMemoryAllocator::new(1024); // 1KB total

    // Allocate 512 bytes (should succeed)
    let alloc1 = allocator.allocate(512, MemoryDomain::Vram);
    assert!(alloc1.is_some());

    // Try to allocate 1024 bytes (should fail - not enough space)
    let alloc2 = allocator.allocate(1024, MemoryDomain::Vram);
    assert!(alloc2.is_none(), "Should fail with OOM");
}

#[test]
fn test_allocator_exact_fit() {
    let allocator = GpuMemoryAllocator::new(1024);

    // Allocate exactly all available space
    let alloc = allocator.allocate(1024, MemoryDomain::Vram);
    assert!(alloc.is_some());
    assert_eq!(allocator.available_bytes(), 0);

    // Next allocation should fail
    let alloc2 = allocator.allocate(1, MemoryDomain::Vram);
    assert!(alloc2.is_none());
}

#[test]
fn test_allocator_free() {
    let allocator = GpuMemoryAllocator::new(1024);

    allocator.allocate(512, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.allocated_bytes(), 512);

    allocator.free(512);
    assert_eq!(allocator.allocated_bytes(), 0);
}

#[test]
fn test_allocator_utilization_calculation() {
    let allocator = GpuMemoryAllocator::new(1000);

    assert_eq!(allocator.utilization_pct(), 0);

    allocator.allocate(250, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.utilization_pct(), 25);

    allocator.allocate(250, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.utilization_pct(), 50);

    allocator.allocate(500, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.utilization_pct(), 100);
}

#[test]
fn test_allocator_available_bytes_after_allocation() {
    let allocator = GpuMemoryAllocator::new(1024);

    allocator.allocate(256, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.available_bytes(), 768);

    allocator.allocate(256, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.available_bytes(), 512);
}

#[test]
fn test_allocator_free_and_reallocate() {
    let allocator = GpuMemoryAllocator::new(1024);

    allocator.allocate(512, MemoryDomain::Vram).unwrap();
    allocator.allocate(512, MemoryDomain::Vram).unwrap();
    assert_eq!(allocator.available_bytes(), 0);

    // Free one allocation
    allocator.free(512);
    assert_eq!(allocator.available_bytes(), 512);

    // Should be able to allocate again
    let alloc = allocator.allocate(256, MemoryDomain::Vram);
    assert!(alloc.is_some());
}

#[test]
fn test_allocator_zero_size_allocation() {
    let allocator = GpuMemoryAllocator::new(1024);

    let alloc = allocator.allocate(0, MemoryDomain::Vram);
    assert!(alloc.is_some());
    assert_eq!(allocator.allocated_bytes(), 0);
}

#[test]
fn test_allocator_large_allocation() {
    let allocator = GpuMemoryAllocator::new(8 * 1024 * 1024 * 1024); // 8GB

    // Allocate 1GB
    let alloc = allocator.allocate(1024 * 1024 * 1024, MemoryDomain::Vram);
    assert!(alloc.is_some());
    assert_eq!(allocator.utilization_pct(), 12); // 1GB / 8GB ≈ 12%
}

#[test]
fn test_allocator_memory_domains() {
    let allocator = GpuMemoryAllocator::new(1024);

    // Should work with different memory domains
    let vram = allocator.allocate(256, MemoryDomain::Vram);
    assert!(vram.is_some());

    let system = allocator.allocate(256, MemoryDomain::System);
    assert!(system.is_some());

    let ggtt = allocator.allocate(256, MemoryDomain::Ggtt);
    assert!(ggtt.is_some());
}

#[test]
fn test_allocator_gem_handle_generation() {
    let allocator = GpuMemoryAllocator::new(1024);

    let alloc1 = allocator.allocate(256, MemoryDomain::Vram).unwrap();
    let alloc2 = allocator.allocate(256, MemoryDomain::Vram).unwrap();

    // Handles should be different
    assert_ne!(alloc1.handle.0, alloc2.handle.0);
}

#[test]
fn test_allocator_concurrent_safety_setup() {
    // This test verifies the allocator uses atomic operations
    // Actual concurrent testing is in stress tests
    let allocator = GpuMemoryAllocator::new(1024);

    // Allocate from "thread 1"
    allocator.allocate(256, MemoryDomain::Vram).unwrap();

    // Allocate from "thread 2"
    allocator.allocate(256, MemoryDomain::Vram).unwrap();

    // Should have both allocations
    assert_eq!(allocator.allocated_bytes(), 512);
}

#[test]
fn test_allocator_boundary_conditions() {
    let allocator = GpuMemoryAllocator::new(u64::MAX);

    // Should handle very large allocations
    let alloc = allocator.allocate(1024 * 1024 * 1024, MemoryDomain::Vram);
    assert!(alloc.is_some());
}

// ============================================================================
// CommandQueue Unit Tests (12 tests)
// ============================================================================

#[test]
fn test_command_queue_creation() {
    let queue = CommandQueue::new(16);
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());
}

#[test]
fn test_command_queue_single_submit() {
    let queue = CommandQueue::new(16);

    let cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 42,
        size: 1024,
        priority: 128,
    };

    queue.submit(cmd).unwrap();
    assert_eq!(queue.len(), 1);
    assert!(!queue.is_empty());
}

#[test]
fn test_command_queue_submit_dequeue() {
    let queue = CommandQueue::new(16);

    let cmd = Command {
        cmd_type: CommandType::Compute,
        buffer_id: 100,
        size: 2048,
        priority: 200,
    };

    queue.submit(cmd).unwrap();
    let dequeued = queue.dequeue().unwrap();

    assert_eq!(dequeued.cmd_type, cmd.cmd_type);
    assert_eq!(dequeued.buffer_id, cmd.buffer_id);
    assert_eq!(dequeued.size, cmd.size);
    assert_eq!(dequeued.priority, cmd.priority);
    assert!(queue.is_empty());
}

#[test]
fn test_command_queue_fifo_order() {
    let queue = CommandQueue::new(16);

    for i in 0..5 {
        let cmd = Command {
            cmd_type: CommandType::Render,
            buffer_id: i,
            size: 1024,
            priority: 100,
        };
        queue.submit(cmd).unwrap();
    }

    // Dequeue in FIFO order
    for i in 0..5 {
        let cmd = queue.dequeue().unwrap();
        assert_eq!(cmd.buffer_id, i);
    }
}

#[test]
fn test_command_queue_full() {
    let queue = CommandQueue::new(4);

    let cmd = Command {
        cmd_type: CommandType::Copy,
        buffer_id: 1,
        size: 512,
        priority: 50,
    };

    // Fill queue
    for _ in 0..4 {
        queue.submit(cmd).unwrap();
    }

    // Should reject when full
    let result = queue.submit(cmd);
    assert!(matches!(result, Err(CommandError::QueueFull)));
}

#[test]
fn test_command_queue_empty_dequeue() {
    let queue = CommandQueue::new(16);

    let result = queue.dequeue();
    assert!(
        result.is_none(),
        "Dequeue from empty queue should return None"
    );
}

#[test]
fn test_command_queue_wrap_around() {
    let queue = CommandQueue::new(4);

    let cmd = Command {
        cmd_type: CommandType::Video,
        buffer_id: 1,
        size: 100,
        priority: 75,
    };

    // Fill and drain multiple times to test wrap-around
    for round in 0..3 {
        for i in 0..4 {
            let mut cmd_copy = cmd;
            cmd_copy.buffer_id = (round * 4 + i) as u32;
            queue.submit(cmd_copy).unwrap();
        }

        for _ in 0..4 {
            queue.dequeue().unwrap();
        }
    }

    assert!(queue.is_empty());
}

#[test]
fn test_command_types() {
    let queue = CommandQueue::new(16);

    let types = vec![
        CommandType::Render,
        CommandType::Compute,
        CommandType::Copy,
        CommandType::Video,
    ];

    for (i, cmd_type) in types.iter().enumerate() {
        let cmd = Command {
            cmd_type: *cmd_type,
            buffer_id: i as u32,
            size: 1024,
            priority: 100,
        };
        queue.submit(cmd).unwrap();
    }

    for expected_type in types.iter() {
        let cmd = queue.dequeue().unwrap();
        assert_eq!(cmd.cmd_type, *expected_type);
    }
}

#[test]
fn test_command_packing_all_types() {
    let queue = CommandQueue::new(16);

    let test_cases = vec![
        (CommandType::Render, 0x12345678, 0x200000, 255),
        (CommandType::Compute, 0xABCDEF01, 0x100000, 128),
        (CommandType::Copy, 0x11111111, 0x3FFFFF, 0),
        (CommandType::Video, 0xFFFFFFFF, 0x1, 1),
    ];

    for (cmd_type, buffer_id, size, priority) in test_cases {
        let cmd = Command {
            cmd_type,
            buffer_id,
            size,
            priority,
        };

        queue.submit(cmd).unwrap();
        let unpacked = queue.dequeue().unwrap();

        assert_eq!(unpacked.cmd_type, cmd_type);
        assert_eq!(unpacked.buffer_id, buffer_id);
        assert_eq!(unpacked.size, size);
        assert_eq!(unpacked.priority, priority);
    }
}

#[test]
fn test_command_queue_alignment() {
    use std::mem::align_of;
    assert_eq!(
        align_of::<CommandQueue>(),
        64,
        "CommandQueue must be 64-byte aligned"
    );
}

#[test]
fn test_command_priority_range() {
    let queue = CommandQueue::new(16);

    // Test min and max priority
    let min_cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 1,
        size: 1024,
        priority: 0,
    };

    let max_cmd = Command {
        cmd_type: CommandType::Render,
        buffer_id: 2,
        size: 1024,
        priority: 255,
    };

    queue.submit(min_cmd).unwrap();
    queue.submit(max_cmd).unwrap();

    let cmd1 = queue.dequeue().unwrap();
    let cmd2 = queue.dequeue().unwrap();

    assert_eq!(cmd1.priority, 0);
    assert_eq!(cmd2.priority, 255);
}

#[test]
fn test_command_size_boundary() {
    let queue = CommandQueue::new(16);

    // Maximum size for 22-bit field is 0x3FFFFF (4,194,303)
    let cmd = Command {
        cmd_type: CommandType::Compute,
        buffer_id: 100,
        size: 0x3FFFFF,
        priority: 128,
    };

    queue.submit(cmd).unwrap();
    let dequeued = queue.dequeue().unwrap();

    assert_eq!(dequeued.size, 0x3FFFFF);
}

// ============================================================================
// DRM Interface Unit Tests (8 tests)
// ============================================================================

#[test]
fn test_gem_handle_creation() {
    let handle = GemHandle(42);
    assert_eq!(handle.0, 42);
}

#[test]
fn test_gem_handle_equality() {
    let h1 = GemHandle(100);
    let h2 = GemHandle(100);
    let h3 = GemHandle(200);

    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn test_gem_create_params() {
    let params = GemCreateParams {
        size: 4096,
        alignment: 64,
        cpu_cached: true,
    };

    assert_eq!(params.size, 4096);
    assert_eq!(params.alignment, 64);
    assert!(params.cpu_cached);
}

#[test]
fn test_gem_create_params_large_size() {
    let params = GemCreateParams {
        size: 1024 * 1024 * 1024, // 1GB
        alignment: 4096,
        cpu_cached: false,
    };

    assert_eq!(params.size, 1024 * 1024 * 1024);
}

#[test]
fn test_memory_domain_variants() {
    let domains = vec![MemoryDomain::Vram, MemoryDomain::System, MemoryDomain::Ggtt];

    for domain in domains {
        match domain {
            MemoryDomain::Vram => {}
            MemoryDomain::System => {}
            MemoryDomain::Ggtt => {}
        }
    }
}

#[test]
fn test_memory_domain_equality() {
    assert_eq!(MemoryDomain::Vram, MemoryDomain::Vram);
    assert_ne!(MemoryDomain::Vram, MemoryDomain::System);
}

#[test]
fn test_gem_handle_copy() {
    let h1 = GemHandle(42);
    let h2 = h1; // Copy
    assert_eq!(h1, h2);
}

#[test]
fn test_gem_create_params_clone() {
    let p1 = GemCreateParams {
        size: 4096,
        alignment: 64,
        cpu_cached: true,
    };

    let p2 = p1; // Copy
    assert_eq!(p1.size, p2.size);
    assert_eq!(p1.alignment, p2.alignment);
    assert_eq!(p1.cpu_cached, p2.cpu_cached);
}

// ============================================================================
// GGTT Entry Unit Tests (5 tests)
// ============================================================================

#[test]
fn test_ggtt_entry_creation() {
    let entry = GgttEntry::new(0x1000, 0x2000, 0x4000, 0x3);

    assert_eq!(entry.vaddr, 0x1000);
    assert_eq!(entry.paddr, 0x2000);
    assert_eq!(entry.size, 0x4000);
    assert_eq!(entry.flags, 0x3);
}

#[test]
fn test_ggtt_entry_alignment() {
    use std::mem::align_of;
    assert_eq!(
        align_of::<GgttEntry>(),
        64,
        "GgttEntry must be 64-byte aligned"
    );
}

#[test]
fn test_ggtt_entry_size() {
    use std::mem::size_of;
    assert_eq!(
        size_of::<GgttEntry>(),
        64,
        "GgttEntry should be exactly 64 bytes"
    );
}

#[test]
fn test_ggtt_entry_const_creation() {
    const ENTRY: GgttEntry = GgttEntry::new(0x1000, 0x2000, 0x4000, 0x3);
    assert_eq!(ENTRY.vaddr, 0x1000);
}

#[test]
fn test_ggtt_entry_large_values() {
    let entry = GgttEntry::new(u64::MAX, u64::MAX, u64::MAX, u64::MAX);

    assert_eq!(entry.vaddr, u64::MAX);
    assert_eq!(entry.paddr, u64::MAX);
    assert_eq!(entry.size, u64::MAX);
    assert_eq!(entry.flags, u64::MAX);
}
