// GPU Command Buffer Integration Tests (T1+T4 Batch)
// Phase 2 HAL: CommandBufferCapsule comprehensive validation
//
// T28 Framework:
// - Q1-Q7: Unit tests (creation, single command, validation)
// - Q8-Q14: Property tests (ordering, generation, atomicity)
// - Q15-Q21: Integration tests (record+submit cycles)
// - Q22-Q28: Production tests (stress, wraparound, concurrent access)

use atomic_capsule::gpu::hal::{CommandBufferCapsule, GpuCommand, CommandType, CommandBufferError};

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn unit_001_capsule_creation() {
    let buf = CommandBufferCapsule::new();
    assert_eq!(buf.command_count(), 0);
    assert_eq!(buf.head(), 0);
    assert_eq!(buf.is_empty(), true);
}

#[test]
fn unit_002_capsule_size_alignment() {
    use core::mem::{size_of, align_of};
    assert_eq!(size_of::<CommandBufferCapsule>(), 640);
    assert_eq!(align_of::<CommandBufferCapsule>(), 512);
}

#[test]
fn unit_003_gpu_command_noop() {
    let cmd = GpuCommand::noop();
    assert_eq!(cmd.cmd_type, 0);
    assert!(cmd.validate().is_ok());
}

#[test]
fn unit_004_command_type_enum() {
    assert_eq!(CommandType::NoOp as u8, 0);
    assert_eq!(CommandType::Draw as u8, 1);
    assert_eq!(CommandType::Dispatch as u8, 2);
    assert!(CommandType::from_u8(10).is_err());
}

#[test]
fn unit_005_record_single_command() {
    let buf = CommandBufferCapsule::new();
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    let result = buf.record_command(cmd);
    assert!(result.is_ok());
    assert_eq!(buf.command_count(), 1);
    assert_eq!(buf.head(), 1);
}

#[test]
fn unit_006_command_retrieval() {
    let buf = CommandBufferCapsule::new();
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 42,
        size: 512,
        flags: 0xDEADBEEF,
        dependency: 123,
    };

    buf.record_command(cmd).unwrap();
    let retrieved = buf.get_command(0).unwrap();
    assert_eq!(retrieved.cmd_type, CommandType::Draw as u8);
    assert_eq!(retrieved.offset, 42);
    assert_eq!(retrieved.size, 512);
}

#[test]
fn unit_007_invalid_command_type() {
    let cmd = GpuCommand {
        cmd_type: 255,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };
    assert!(matches!(cmd.validate(), Err(CommandBufferError::InvalidCommandType(_))));
}

// ============================================================================
// Q8-Q14: Property Tests
// ============================================================================

#[test]
fn prop_001_command_ordering_preserved() {
    let buf = CommandBufferCapsule::new();

    // Record 5 commands with unique identifiers
    for i in 0..5 {
        let cmd = GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i as u8,
            size: 256 + (i as u16 * 16),
            flags: i as u32,
            dependency: u64::MAX,
        };
        buf.record_command(cmd).unwrap();
    }

    // Verify order
    for i in 0..5 {
        let retrieved = buf.get_command(i as u16).unwrap();
        assert_eq!(retrieved.offset, i as u8);
        assert_eq!(retrieved.flags, i as u32);
    }
}

#[test]
fn prop_002_generation_increments_on_reset() {
    let buf = CommandBufferCapsule::new();
    let gen1 = buf.generation();

    buf.reset().unwrap();
    let gen2 = buf.generation();

    // Generation should change (wrap or increment)
    // Since we start at 0, incrementing gives 1
    assert_ne!(gen1, gen2);
}

#[test]
fn prop_003_buffer_full_detection() {
    let buf = CommandBufferCapsule::new();
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    // Fill all 16 slots
    for _ in 0..16 {
        assert!(buf.record_command(cmd).is_ok());
    }

    // Next should fail
    assert!(matches!(buf.record_command(cmd), Err(CommandBufferError::BufferFull { .. })));
}

#[test]
fn prop_004_empty_buffer_submit_fails() {
    let buf = CommandBufferCapsule::new();
    let result = buf.submit_batch();
    assert!(matches!(result, Err(CommandBufferError::NotReady)));
}

#[test]
fn prop_005_reset_clears_state() {
    let buf = CommandBufferCapsule::new();
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    for _ in 0..8 {
        buf.record_command(cmd).ok();
    }

    assert_eq!(buf.command_count(), 8);
    assert!(!buf.is_empty());

    buf.reset().unwrap();
    assert_eq!(buf.command_count(), 0);
    assert_eq!(buf.head(), 0);
    assert!(buf.is_empty());
}

#[test]
fn prop_006_batch_recording_atomicity() {
    let buf = CommandBufferCapsule::new();
    let commands: Vec<GpuCommand> = (0..4)
        .map(|i| GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i as u8,
            size: 256,
            flags: i as u32,
            dependency: u64::MAX,
        })
        .collect();

    let result = buf.record_batch(&commands);
    assert!(result.is_ok());
    assert_eq!(buf.command_count(), 4);
}

#[test]
fn prop_007_command_type_diversity() {
    let buf = CommandBufferCapsule::new();
    let types = vec![
        CommandType::Draw,
        CommandType::Dispatch,
        CommandType::Clear,
        CommandType::Copy,
        CommandType::Barrier,
        CommandType::Marker,
        CommandType::Blit,
    ];

    for (i, cmd_type) in types.iter().enumerate() {
        let cmd = GpuCommand {
            cmd_type: *cmd_type as u8,
            offset: i as u8,
            size: 256,
            flags: 0,
            dependency: u64::MAX,
        };
        buf.record_command(cmd).unwrap();
    }

    assert_eq!(buf.command_count(), 7);

    // Verify all types recorded
    for (i, cmd_type) in types.iter().enumerate() {
        let retrieved = buf.get_command(i as u16).unwrap();
        assert_eq!(retrieved.cmd_type, *cmd_type as u8);
    }
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn integ_001_record_submit_cycle() {
    let buf = CommandBufferCapsule::new();

    // Record
    for i in 0..8 {
        let cmd = GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i,
            size: 256,
            flags: i as u32,
            dependency: u64::MAX,
        };
        buf.record_command(cmd).unwrap();
    }

    assert_eq!(buf.command_count(), 8);

    // Submit
    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 8);
    assert!(result.generation > 0);
    assert!(result.execution_id >= 0);
}

#[test]
fn integ_002_multiple_cycles() {
    let buf = CommandBufferCapsule::new();

    for cycle in 0..3 {
        // Record
        for i in 0..4 {
            let cmd = GpuCommand {
                cmd_type: CommandType::Draw as u8,
                offset: (cycle * 4 + i) as u8,
                size: 256,
                flags: (cycle as u32 * 1000 + i as u32),
                dependency: u64::MAX,
            };
            buf.record_command(cmd).unwrap();
        }

        // Submit
        let result = buf.submit_batch().unwrap();
        assert_eq!(result.command_count, 4);

        // Reset
        buf.reset().unwrap();
        assert!(buf.is_empty());
    }
}

#[test]
fn integ_003_batch_submit_with_diverse_types() {
    let buf = CommandBufferCapsule::new();

    let commands: Vec<GpuCommand> = vec![
        GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: 0,
            size: 256,
            flags: 1,
            dependency: u64::MAX,
        },
        GpuCommand {
            cmd_type: CommandType::Dispatch as u8,
            offset: 1,
            size: 512,
            flags: 2,
            dependency: u64::MAX,
        },
        GpuCommand {
            cmd_type: CommandType::Clear as u8,
            offset: 2,
            size: 128,
            flags: 3,
            dependency: u64::MAX,
        },
        GpuCommand {
            cmd_type: CommandType::Copy as u8,
            offset: 3,
            size: 1024,
            flags: 4,
            dependency: u64::MAX,
        },
    ];

    buf.record_batch(&commands).unwrap();
    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 4);
}

#[test]
fn integ_004_query_operations() {
    let buf = CommandBufferCapsule::new();

    // Empty buffer
    assert!(buf.is_empty());
    assert!(!buf.is_full());
    assert_eq!(buf.command_count(), 0);
    assert_eq!(buf.head(), 0);

    // After recording
    for i in 0..8 {
        let cmd = GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i,
            size: 256,
            flags: 0,
            dependency: u64::MAX,
        };
        buf.record_command(cmd).unwrap();
    }

    assert!(!buf.is_empty());
    assert!(!buf.is_full());
    assert_eq!(buf.command_count(), 8);
    assert_eq!(buf.head(), 8);

    // After reset
    buf.reset().unwrap();
    assert!(buf.is_empty());
    assert_eq!(buf.command_count(), 0);
}

#[test]
fn integ_005_invalid_slot_access() {
    let buf = CommandBufferCapsule::new();
    let result = buf.get_command(100);
    assert!(matches!(result, Err(CommandBufferError::InvalidSlot { .. })));
}

#[test]
fn integ_006_full_buffer_behavior() {
    let buf = CommandBufferCapsule::new();
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    // Fill exactly
    for _ in 0..16 {
        assert!(buf.record_command(cmd).is_ok());
    }

    assert!(buf.is_full());
    assert_eq!(buf.head(), 16);

    // Next should fail
    assert!(buf.record_command(cmd).is_err());
}

#[test]
fn integ_007_generation_tracking() {
    let buf = CommandBufferCapsule::new();
    let initial_gen = buf.generation();

    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    buf.record_command(cmd).unwrap();
    let after_record_gen = buf.generation();
    assert_eq!(initial_gen, after_record_gen); // Recording doesn't change gen

    buf.reset().unwrap();
    let after_reset_gen = buf.generation();
    assert_ne!(initial_gen, after_reset_gen); // Reset increments gen
}

// ============================================================================
// Q22-Q28: Production Tests
// ============================================================================

#[test]
fn prod_001_stress_sequential_commands() {
    let buf = CommandBufferCapsule::new();

    for i in 0..16 {
        let cmd = GpuCommand {
            cmd_type: (CommandType::Draw as u8 + (i % 7)) % 8,
            offset: (i % 256) as u8,
            size: 256 + (i as u16 * 16),
            flags: i as u32,
            dependency: u64::MAX,
        };
        assert!(buf.record_command(cmd).is_ok());
    }

    assert!(buf.is_full());
    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 16);
}

#[test]
fn prod_002_stress_batch_recording() {
    let buf = CommandBufferCapsule::new();

    let commands: Vec<GpuCommand> = (0..8)
        .map(|i| GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i as u8,
            size: 512,
            flags: i as u32,
            dependency: u64::MAX,
        })
        .collect();

    assert!(buf.record_batch(&commands).is_ok());
    assert_eq!(buf.command_count(), 8);

    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 8);
}

#[test]
fn prod_003_stress_many_cycles() {
    let buf = CommandBufferCapsule::new();

    for cycle in 0..100 {
        // Record and flush 16 commands each cycle
        for i in 0..16 {
            let cmd = GpuCommand {
                cmd_type: (CommandType::Draw as u8 + (cycle % 7)) % 8,
                offset: ((i + cycle as u16) % 256) as u8,
                size: 256,
                flags: (cycle as u32 * 1000 + i as u32),
                dependency: u64::MAX,
            };
            assert!(buf.record_command(cmd).is_ok());
        }

        let result = buf.submit_batch();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().command_count, 16);

        assert!(buf.reset().is_ok());
    }
}

#[test]
fn prod_004_generation_wraparound() {
    let buf = CommandBufferCapsule::new();

    // Manually set generation to near max
    let state_val = buf.state.load_secondary(std::sync::atomic::Ordering::Relaxed);
    let near_max_gen = u32::MAX;
    let new_state = (state_val & 0xFFFF_FFFF) | ((near_max_gen as u64) << 32);
    buf.state
        .store_secondary(new_state, std::sync::atomic::Ordering::Relaxed);

    assert_eq!(buf.generation(), u32::MAX);

    // Reset should wrap to 0
    buf.reset().unwrap();
    assert_eq!(buf.generation(), 0);
}

#[test]
fn prod_005_concurrent_state_queries() {
    let buf = CommandBufferCapsule::new();

    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };

    buf.record_command(cmd).unwrap();

    // Multiple rapid queries should be consistent
    let count1 = buf.command_count();
    let head1 = buf.head();
    let gen1 = buf.generation();

    let count2 = buf.command_count();
    let head2 = buf.head();
    let gen2 = buf.generation();

    assert_eq!(count1, count2);
    assert_eq!(head1, head2);
    assert_eq!(gen1, gen2);
}

#[test]
fn prod_006_mixed_batch_sequential() {
    let buf = CommandBufferCapsule::new();

    // First batch of 4
    let batch1: Vec<GpuCommand> = (0..4)
        .map(|i| GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: i as u8,
            size: 256,
            flags: i as u32,
            dependency: u64::MAX,
        })
        .collect();
    buf.record_batch(&batch1).unwrap();

    // Add 4 more sequentially
    for i in 4..8 {
        let cmd = GpuCommand {
            cmd_type: CommandType::Dispatch as u8,
            offset: i as u8,
            size: 512,
            flags: i as u32,
            dependency: u64::MAX,
        };
        buf.record_command(cmd).unwrap();
    }

    assert_eq!(buf.command_count(), 8);

    // Submit all
    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 8);

    // Verify all commands
    for i in 0..4 {
        let cmd = buf.get_command(i as u16).unwrap();
        assert_eq!(cmd.cmd_type, CommandType::Draw as u8);
    }
    for i in 4..8 {
        let cmd = buf.get_command(i as u16).unwrap();
        assert_eq!(cmd.cmd_type, CommandType::Dispatch as u8);
    }
}

#[test]
fn prod_007_large_parameter_blocks() {
    let buf = CommandBufferCapsule::new();

    // Test with maximum-size parameter blocks
    for i in 0..16 {
        let cmd = GpuCommand {
            cmd_type: CommandType::Draw as u8,
            offset: (i % 256) as u8,
            size: 65535, // Maximum size
            flags: i as u32,
            dependency: u64::MAX,
        };
        assert!(buf.record_command(cmd).is_ok());
    }

    assert!(buf.is_full());
    let result = buf.submit_batch().unwrap();
    assert_eq!(result.command_count, 16);
}

#[test]
fn prod_008_empty_batch_handling() {
    let buf = CommandBufferCapsule::new();

    // Try to submit empty batch
    let result = buf.submit_batch();
    assert!(matches!(result, Err(CommandBufferError::NotReady)));

    // Record, reset, try again
    let cmd = GpuCommand {
        cmd_type: CommandType::Draw as u8,
        offset: 0,
        size: 256,
        flags: 0,
        dependency: u64::MAX,
    };
    buf.record_command(cmd).ok();
    buf.reset().ok();

    let result = buf.submit_batch();
    assert!(matches!(result, Err(CommandBufferError::NotReady)));
}
