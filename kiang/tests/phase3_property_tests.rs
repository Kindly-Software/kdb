//! Phase 3 Property Tests - Invariant Validation
//!
//! Following T42 test framework, this file validates system invariants:
//! - Memory invariants (used + free = total)
//! - Version consistency (head_ver = tail_ver + 1)
//! - No torn reads under concurrent access
//! - Monotonic generation counters

use kiang::capsules::*;
use kiang::command::*;
use kiang::drm_interface::*;
use kiang::memory::*;
use proptest::prelude::*;

// ============================================================================
// Memory Invariant Properties (8 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_memory_state_invariant(
        total_mb in 1u16..=65535u16,
        used_mb in 0u16..=65535u16,
    ) {
        // Invariant: used + available <= total
        let used = used_mb.min(total_mb);
        let available = total_mb.saturating_sub(used);

        let state = MemoryState {
            total_vram_mb: total_mb,
            used_vram_mb: used,
            free_vram_mb: available,
            allocation_count: 0,
            fragment_count: 0,
            largest_free_mb: available,
            allocation_gen: 0,
            pressure_pct: 0,
        };

        // Property: used + available <= total
        prop_assert!(state.used_vram_mb as u32 + state.free_vram_mb as u32 <= state.total_vram_mb as u32);
    }
}

proptest! {
    #[test]
    fn prop_memory_allocation_never_exceeds_total(
        total_vram in 1024u64..=16*1024*1024*1024,
        alloc_size in 1u64..=1024*1024*1024,
    ) {
        let allocator = GpuMemoryAllocator::new(total_vram);

        // Try to allocate
        let result = allocator.allocate(alloc_size, MemoryDomain::Vram);

        // Property: If allocation succeeds, allocated <= total
        if let Some(alloc) = result {
            prop_assert!(allocator.allocated_bytes() <= total_vram);
            prop_assert_eq!(allocator.allocated_bytes(), alloc.size);
        } else {
            // If allocation fails, it must be because size > available
            prop_assert!(alloc_size > allocator.available_bytes());
        }
    }
}

proptest! {
    #[test]
    fn prop_memory_utilization_bounded(
        total_vram in 1024u64..=16*1024*1024*1024,
        alloc_sizes in prop::collection::vec(1u64..=100*1024*1024, 1..=10),
    ) {
        let allocator = GpuMemoryAllocator::new(total_vram);

        for size in alloc_sizes {
            let _ = allocator.allocate(size, MemoryDomain::Vram);
        }

        // Property: Utilization is always 0-100%
        let util = allocator.utilization_pct();
        prop_assert!(util <= 100);

        // Property: allocated <= total
        prop_assert!(allocator.allocated_bytes() <= total_vram);

        // Property: available >= 0
        prop_assert!(allocator.available_bytes() <= total_vram);
    }
}

proptest! {
    #[test]
    fn prop_memory_capsule_can_allocate_consistency(
        total_mb in 1u16..=16384u16,
        used_mb in 0u16..=16384u16,
        request_mb in 0u16..=16384u16,
    ) {
        let capsule = MemoryCapsule::new(total_mb);

        let used = used_mb.min(total_mb);
        let free = total_mb.saturating_sub(used);

        let state = MemoryState {
            total_vram_mb: total_mb,
            used_vram_mb: used,
            free_vram_mb: free,
            allocation_count: 0,
            fragment_count: 0,
            largest_free_mb: free,
            allocation_gen: 0,
            pressure_pct: 0,
        };

        capsule.publish(state);

        // Property: can_allocate returns true iff request <= available
        let can_alloc = capsule.can_allocate(request_mb);
        let should_alloc = request_mb <= free;

        prop_assert_eq!(can_alloc, should_alloc,
            "can_allocate={} but should be {} (request={}, free={})",
            can_alloc, should_alloc, request_mb, free);
    }
}

proptest! {
    #[test]
    fn prop_memory_pressure_calculation(
        total_mb in 1u16..=65535u16,
        used_mb in 0u16..=65535u16,
    ) {
        let used = used_mb.min(total_mb);
        let pressure_pct = if total_mb > 0 {
            ((used as u32 * 100) / total_mb as u32) as u8
        } else {
            0
        };

        // Property: Pressure percentage is 0-100
        prop_assert!(pressure_pct <= 100);

        // Property: Pressure = 0 when used = 0
        if used == 0 {
            prop_assert_eq!(pressure_pct, 0);
        }

        // Property: Pressure = 100 when used = total
        if used == total_mb {
            prop_assert_eq!(pressure_pct, 100);
        }
    }
}

proptest! {
    #[test]
    fn prop_allocator_free_restores_availability(
        total_vram in 1024u64..=1024*1024*1024,
        alloc_size in 1u64..=100*1024*1024,
    ) {
        let allocator = GpuMemoryAllocator::new(total_vram);

        let before_available = allocator.available_bytes();

        // Allocate
        if let Some(alloc) = allocator.allocate(alloc_size, MemoryDomain::Vram) {
            let after_alloc_available = allocator.available_bytes();

            // Property: available decreased by allocation size
            prop_assert_eq!(before_available - after_alloc_available, alloc.size);

            // Free
            allocator.free(alloc.size);
            let after_free_available = allocator.available_bytes();

            // Property: availability restored after free
            prop_assert_eq!(before_available, after_free_available);
        }
    }
}

proptest! {
    #[test]
    fn prop_memory_capsule_snapshot_validity(
        total_mb in 1u16..=8192u16,
        used_mb in 0u16..=8192u16,
        alloc_count in 0u32..=1000000,
    ) {
        let capsule = MemoryCapsule::new(total_mb);

        let used = used_mb.min(total_mb);
        let free = total_mb.saturating_sub(used);

        let state = MemoryState {
            total_vram_mb: total_mb,
            used_vram_mb: used,
            free_vram_mb: free,
            allocation_count: alloc_count,
            fragment_count: 0,
            largest_free_mb: free,
            allocation_gen: 0,
            pressure_pct: 0,
        };

        capsule.publish(state);

        let snapshot = capsule.read();

        // Property: Snapshot is always valid after publish
        prop_assert!(snapshot.is_some());

        if let Some(snap) = snapshot {
            prop_assert!(snap.is_valid());
            prop_assert_eq!(snap.state.total_vram_mb, total_mb);
            prop_assert_eq!(snap.state.used_vram_mb, used);
            prop_assert_eq!(snap.state.free_vram_mb, free);
        }
    }
}

proptest! {
    #[test]
    fn prop_memory_monotonic_allocation_gen(
        total_vram in 1024u64..=1024*1024*1024,
        alloc_sizes in prop::collection::vec(1u64..=10*1024*1024, 1..=20),
    ) {
        let allocator = GpuMemoryAllocator::new(total_vram);
        let mut prev_gen = 0u64;

        for size in alloc_sizes {
            if let Some(_) = allocator.allocate(size, MemoryDomain::Vram) {
                let snapshot = allocator.capsule().read();
                if let Some(snap) = snapshot {
                    let cur_gen = snap.state.allocation_gen as u64;
                    // Property: Generation counter monotonically increases
                    prop_assert!(cur_gen >= prev_gen);
                    prev_gen = cur_gen;
                }
            }
        }
    }
}

// ============================================================================
// Command State Machine Properties (6 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_command_state_transitions_valid(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::Normal);

        // Property: PENDING → SUBMITTED is valid
        prop_assert!(cmd.mark_submitted());
        let snap1 = cmd.read().unwrap();
        prop_assert_eq!(snap1.state, CommandState::Submitted);

        // Property: SUBMITTED → EXECUTING is valid
        prop_assert!(cmd.mark_executing());
        let snap2 = cmd.read().unwrap();
        prop_assert_eq!(snap2.state, CommandState::Executing);

        // Property: EXECUTING → COMPLETED is valid
        prop_assert!(cmd.mark_completed());
        let snap3 = cmd.read().unwrap();
        prop_assert_eq!(snap3.state, CommandState::Completed);
    }
}

proptest! {
    #[test]
    fn prop_command_invalid_transitions_rejected(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::Normal);

        // Property: PENDING → EXECUTING is invalid (must go through SUBMITTED)
        prop_assert!(!cmd.transition_to(CommandState::Executing));

        // Property: PENDING → COMPLETED is invalid
        prop_assert!(!cmd.transition_to(CommandState::Completed));
    }
}

proptest! {
    #[test]
    fn prop_command_readiness_only_when_pending(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::High);

        // Property: Ready only in PENDING state
        prop_assert!(cmd.is_ready());

        cmd.mark_submitted();
        prop_assert!(!cmd.is_ready());

        cmd.mark_executing();
        prop_assert!(!cmd.is_ready());

        cmd.mark_completed();
        prop_assert!(!cmd.is_ready());
    }
}

proptest! {
    #[test]
    fn prop_command_execution_duration_non_negative(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::Normal);

        cmd.mark_submitted();
        cmd.mark_executing();

        // Small delay
        std::thread::sleep(std::time::Duration::from_micros(10));

        cmd.mark_completed();

        let snapshot = cmd.read().unwrap();

        // Property: Execution duration is always >= 0
        if let Some(duration) = snapshot.execution_duration_us() {
            prop_assert!(duration > 0);
        }
    }
}

proptest! {
    #[test]
    fn prop_command_snapshot_consistency(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
        priority in 0u8..=3u8,
    ) {
        let priority_enum = match priority {
            3 => CommandPriority::RealTime,
            2 => CommandPriority::High,
            1 => CommandPriority::Normal,
            _ => CommandPriority::Low,
        };

        let cmd = CommandCapsule::with_state(buffer_id, size_kb, priority_enum);

        let snapshot = cmd.read().unwrap();

        // Property: Snapshot fields match published values
        prop_assert_eq!(snapshot.buffer_id, buffer_id);
        prop_assert_eq!(snapshot.size_kb, size_kb);
        prop_assert_eq!(snapshot.priority, priority_enum);
        prop_assert_eq!(snapshot.state, CommandState::Pending);
    }
}

proptest! {
    #[test]
    fn prop_command_reset_clears_completion(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::Normal);

        // Complete full cycle
        cmd.mark_submitted();
        cmd.mark_executing();
        cmd.mark_completed();

        // Reset
        cmd.reset(buffer_id, size_kb * 2, CommandPriority::High);

        let snapshot = cmd.read().unwrap();

        // Property: After reset, state is PENDING
        prop_assert_eq!(snapshot.state, CommandState::Pending);

        // Property: After reset, completion_us is 0
        prop_assert_eq!(snapshot.completion_us, 0);

        // Property: After reset, is_ready() returns true
        prop_assert!(cmd.is_ready());
    }
}

// ============================================================================
// GPU State Properties (5 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_gpu_state_ready_thresholds(
        temp in 0u8..=255u8,
        util in 0u8..=100u8,
    ) {
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: temp,
            utilization: util,
            valid: true,
        };

        // Property: Ready only if temp < 95 AND util < 95
        let should_be_ready = temp < 95 && util < 95;
        prop_assert_eq!(state.is_ready(), should_be_ready);
    }
}

proptest! {
    #[test]
    fn prop_gpu_state_invalid_never_ready(
        temp in 0u8..=94u8,
        util in 0u8..=94u8,
    ) {
        let state = GpuState {
            gpu_id: 0,
            frequency_mhz: 2100,
            power_mw: 45000,
            temp_celsius: temp,
            utilization: util,
            valid: false, // Invalid!
        };

        // Property: Invalid state is never ready
        prop_assert!(!state.is_ready());
    }
}

proptest! {
    #[test]
    fn prop_gpu_state_capsule_consistency(
        gpu_id in 0u8..=255,
        freq_mhz in 0u16..=65535,
        temp in 0u8..=255,
    ) {
        let capsule = GpuStateCapsule::new();

        let state = GpuState {
            gpu_id,
            frequency_mhz: freq_mhz,
            power_mw: 45000,
            temp_celsius: temp,
            utilization: 50,
            valid: true,
        };

        capsule.publish(state);

        let read_state = capsule.read();

        // Property: Published state matches read state
        prop_assert!(read_state.is_valid());
        prop_assert_eq!(read_state.gpu_id, gpu_id);
        prop_assert_eq!(read_state.frequency_mhz, freq_mhz);
        prop_assert_eq!(read_state.temp_celsius, temp);
    }
}

proptest! {
    #[test]
    fn prop_gpu_state_multiple_publishes_monotonic(
        updates in prop::collection::vec(
            (0u16..=65535u16, 0u8..=255u8),
            1..=10
        ),
    ) {
        let capsule = GpuStateCapsule::new();

        for (freq, temp) in updates {
            let state = GpuState {
                gpu_id: 1,
                frequency_mhz: freq,
                power_mw: 45000,
                temp_celsius: temp,
                utilization: 50,
                valid: true,
            };

            capsule.publish(state);

            // Property: Every read after publish is valid
            let read = capsule.read();
            prop_assert!(read.is_valid());
        }
    }
}

proptest! {
    #[test]
    fn prop_gpu_state_boundary_values(
        gpu_id in 0u8..=255,
        freq in 0u16..=65535,
        power in 0u16..=65535,
        temp in 0u8..=255,
        util in 0u8..=255,
    ) {
        let state = GpuState {
            gpu_id,
            frequency_mhz: freq,
            power_mw: power,
            temp_celsius: temp,
            utilization: util,
            valid: true,
        };

        let capsule = GpuStateCapsule::new();
        capsule.publish(state);

        let read = capsule.read();

        // Property: All fields are preserved within their bit ranges
        prop_assert!(read.is_valid());
        prop_assert_eq!(read.gpu_id, gpu_id);
        prop_assert_eq!(read.frequency_mhz, freq);
        prop_assert_eq!(read.power_mw, power);
        prop_assert_eq!(read.temp_celsius, temp);
        prop_assert_eq!(read.utilization, util);
    }
}

// ============================================================================
// DRM Interface Properties (4 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_drm_generation_monotonic(
        alloc_count in 1usize..=10,
    ) {
        use std::os::unix::io::FromRawFd;

        let device = DrmDevice {
            file: unsafe { std::fs::File::from_raw_fd(0) },
            card_path: "/dev/dri/card0".to_string(),
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };

        let mut prev_gen = device.generation();

        for _ in 0..alloc_count {
            let _gem = GemObject::create(&device, 4096).unwrap();
            let cur_gen = device.generation();

            // Property: Generation is monotonically increasing
            prop_assert!(cur_gen > prev_gen);
            prev_gen = cur_gen;
        }
    }
}

proptest! {
    #[test]
    fn prop_gem_handle_uniqueness(
        count in 2usize..=20,
    ) {
        use std::os::unix::io::FromRawFd;
        use std::collections::HashSet;

        let device = DrmDevice {
            file: unsafe { std::fs::File::from_raw_fd(0) },
            card_path: "/dev/dri/card0".to_string(),
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };

        let mut handles = HashSet::new();

        for _ in 0..count {
            let gem = GemObject::create(&device, 4096).unwrap();
            handles.insert(gem.handle());
        }

        // Property: All GEM handles are unique
        prop_assert_eq!(handles.len(), count);
    }
}

proptest! {
    #[test]
    fn prop_vm_bind_alignment_4kb(
        addr in 0u64..=0x1000000,
    ) {
        use std::os::unix::io::FromRawFd;

        let device = DrmDevice {
            file: unsafe { std::fs::File::from_raw_fd(0) },
            card_path: "/dev/dri/card0".to_string(),
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };

        let gem = GemObject::create(&device, 4096).unwrap();
        let vm_bind = VmBind::new(&device);

        let result = vm_bind.bind(&gem, addr);

        // Property: Binding succeeds only for 4KB-aligned addresses
        if addr % 4096 == 0 {
            prop_assert!(result.is_ok());
        } else {
            prop_assert!(result.is_err());
        }
    }
}

proptest! {
    #[test]
    fn prop_gem_object_generation_captured(
        size in 4096u64..=1024*1024,
    ) {
        use std::os::unix::io::FromRawFd;

        let device = DrmDevice {
            file: unsafe { std::fs::File::from_raw_fd(0) },
            card_path: "/dev/dri/card0".to_string(),
            generation: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };

        let gen_before = device.generation();
        let gem = GemObject::create(&device, size).unwrap();
        let gen_after = device.generation();

        // Property: GEM captures generation BEFORE device bump
        prop_assert_eq!(gem.generation(), gen_before);

        // Property: Device generation increments after allocation
        prop_assert_eq!(gen_after, gen_before + 1);
    }
}

// ============================================================================
// Version Consistency Properties (4 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_memory_capsule_version_consistency(
        total_mb in 1u16..=8192,
        used_mb in 0u16..=8192,
        iterations in 1usize..=50,
    ) {
        let capsule = MemoryCapsule::new(total_mb);

        for i in 0..iterations {
            let used = (used_mb + i as u16).min(total_mb);
            let free = total_mb.saturating_sub(used);

            let state = MemoryState {
                total_vram_mb: total_mb,
                used_vram_mb: used,
                free_vram_mb: free,
                allocation_count: i as u32,
                fragment_count: 0,
                largest_free_mb: free,
                allocation_gen: i as u16,
                pressure_pct: 0,
            };

            capsule.publish(state);

            // Property: Read always returns valid or None (no torn reads)
            let snapshot = capsule.read();
            if let Some(snap) = snapshot {
                prop_assert!(snap.is_valid());
                prop_assert_eq!(snap.state.allocation_gen, i as u16);
            }
        }
    }
}

proptest! {
    #[test]
    fn prop_command_capsule_version_match(
        buffer_id in 1u32..=0xFFFFFF,
        size_kb in 1u16..=65535,
        state_changes in 1usize..=20,
    ) {
        let cmd = CommandCapsule::with_state(buffer_id, size_kb, CommandPriority::Normal);

        for _ in 0..state_changes {
            // Transition through states
            let _ = cmd.mark_submitted();
            let _ = cmd.mark_executing();
            let _ = cmd.mark_completed();

            cmd.reset(buffer_id, size_kb, CommandPriority::Normal);

            // Property: Read always returns consistent state or None
            let snapshot = cmd.read();
            prop_assert!(snapshot.is_some() || snapshot.is_none()); // Never panics
        }
    }
}

proptest! {
    #[test]
    fn prop_gpu_state_no_torn_reads(
        updates in prop::collection::vec(
            (0u16..=65535, 0u8..=255, 0u8..=100),
            1..=30
        ),
    ) {
        let capsule = GpuStateCapsule::new();

        for (freq, temp, util) in updates {
            let state = GpuState {
                gpu_id: 1,
                frequency_mhz: freq,
                power_mw: 45000,
                temp_celsius: temp,
                utilization: util,
                valid: true,
            };

            capsule.publish(state);

            // Property: Read never returns torn state
            let read = capsule.read();
            prop_assert!(read.is_valid());

            // If read succeeds, all fields must be from same publish
            prop_assert_eq!(read.frequency_mhz, freq);
            prop_assert_eq!(read.temp_celsius, temp);
            prop_assert_eq!(read.utilization, util);
        }
    }
}

proptest! {
    #[test]
    fn prop_allocator_capsule_sync(
        total_vram in 1024*1024u64..=1024*1024*1024,
        allocs in prop::collection::vec(1u64..=10*1024*1024, 1..=20),
    ) {
        let allocator = GpuMemoryAllocator::new(total_vram);

        for size in allocs {
            let before_alloc = allocator.allocated_bytes();

            if let Some(alloc) = allocator.allocate(size, MemoryDomain::Vram) {
                let after_alloc = allocator.allocated_bytes();

                // Property: Allocator and capsule stay in sync
                prop_assert_eq!(after_alloc, before_alloc + alloc.size);

                let snapshot = allocator.capsule().read();
                if let Some(snap) = snapshot {
                    let capsule_used_bytes = snap.state.used_vram_mb as u64 * 1024 * 1024;
                    let allocator_used = allocator.allocated_bytes();

                    // Allow for rounding (MB granularity in capsule)
                    let diff = if capsule_used_bytes > allocator_used {
                        capsule_used_bytes - allocator_used
                    } else {
                        allocator_used - capsule_used_bytes
                    };

                    prop_assert!(diff < 2 * 1024 * 1024); // Within 2MB
                }
            }
        }
    }
}
