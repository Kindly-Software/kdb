//! Integration tests for KgpuCommandEncoderCapsule
//!
//! Tests the type-state command encoder from gpu::kgpu::command module.

use atomic_capsule::gpu::kgpu::{
    KgpuCommandEncoderCapsule, Empty, Recording, Finished,
    CommandType, CommandSlot, CommandError, MAX_COMMANDS,
};

// ============================================================================
// Size and Alignment Tests
// ============================================================================

#[test]
fn test_encoder_size_is_512_bytes() {
    assert_eq!(
        core::mem::size_of::<KgpuCommandEncoderCapsule<Empty>>(),
        512,
        "Empty encoder must be exactly 512 bytes"
    );
    assert_eq!(
        core::mem::size_of::<KgpuCommandEncoderCapsule<Recording>>(),
        512,
        "Recording encoder must be exactly 512 bytes"
    );
    assert_eq!(
        core::mem::size_of::<KgpuCommandEncoderCapsule<Finished>>(),
        512,
        "Finished encoder must be exactly 512 bytes"
    );
}

#[test]
fn test_encoder_alignment_is_512_bytes() {
    assert_eq!(
        core::mem::align_of::<KgpuCommandEncoderCapsule<Empty>>(),
        512,
        "Empty encoder must have 512-byte alignment"
    );
    assert_eq!(
        core::mem::align_of::<KgpuCommandEncoderCapsule<Recording>>(),
        512,
        "Recording encoder must have 512-byte alignment"
    );
    assert_eq!(
        core::mem::align_of::<KgpuCommandEncoderCapsule<Finished>>(),
        512,
        "Finished encoder must have 512-byte alignment"
    );
}

#[test]
fn test_command_slot_size_is_16_bytes() {
    assert_eq!(
        core::mem::size_of::<CommandSlot>(),
        16,
        "CommandSlot must be exactly 16 bytes"
    );
}

// ============================================================================
// Type-State Transition Tests
// ============================================================================

#[test]
fn test_empty_to_recording_transition() {
    let empty: KgpuCommandEncoderCapsule<Empty> = KgpuCommandEncoderCapsule::new();
    let recording: KgpuCommandEncoderCapsule<Recording> = empty.begin();
    // If this compiles, the type-state transition works
    let _ = recording;
}

#[test]
fn test_recording_to_finished_transition() {
    let empty = KgpuCommandEncoderCapsule::new();
    let recording = empty.begin();
    let finished: KgpuCommandEncoderCapsule<Finished> = recording.finish();
    // If this compiles, the type-state transition works
    let _ = finished;
}

#[test]
fn test_full_lifecycle() {
    // Empty -> Recording -> Finished
    let encoder = KgpuCommandEncoderCapsule::new();
    assert_eq!(encoder.generation(), 0);

    let encoder = encoder.begin();
    assert_eq!(encoder.generation(), 1);

    let encoder = encoder.finish();
    assert_eq!(encoder.generation(), 2);
}

// ============================================================================
// Command Recording Tests
// ============================================================================

#[test]
fn test_record_single_command() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    encoder.copy_buffer_to_buffer(0, 1024, 4096).unwrap();

    let encoder = encoder.finish();
    assert_eq!(encoder.command_count(), 1);

    let cmd = encoder.get(0).unwrap();
    assert_eq!(cmd.command_type(), CommandType::CopyBufferToBuffer);
}

#[test]
fn test_record_multiple_commands() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    encoder.set_pipeline(42).unwrap();
    encoder.set_bind_group(0, 100).unwrap();
    encoder.draw(36, 1, 0, 0).unwrap();

    let encoder = encoder.finish();
    assert_eq!(encoder.command_count(), 3);

    // Verify command types in order
    let commands: Vec<_> = encoder.iter().map(|c| c.command_type()).collect();
    assert_eq!(commands[0], CommandType::SetPipeline);
    assert_eq!(commands[1], CommandType::SetBindGroup);
    assert_eq!(commands[2], CommandType::Draw);
}

#[test]
fn test_buffer_full_error() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    // Fill buffer
    for _ in 0..MAX_COMMANDS {
        encoder.set_pipeline(0).unwrap();
    }

    // Next record should fail
    let result = encoder.set_pipeline(0);
    assert_eq!(result, Err(CommandError::BufferFull));
}

// ============================================================================
// Finished State Tests
// ============================================================================

#[test]
fn test_finished_command_count() {
    let encoder = KgpuCommandEncoderCapsule::new().begin();
    let encoder = encoder.finish();

    assert_eq!(encoder.command_count(), 0);
    assert!(encoder.is_empty());
}

#[test]
fn test_finished_commands_slice() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    encoder.set_pipeline(1).unwrap();
    encoder.set_pipeline(2).unwrap();
    encoder.set_pipeline(3).unwrap();

    let encoder = encoder.finish();

    let commands = encoder.commands();
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].param2, 1);
    assert_eq!(commands[1].param2, 2);
    assert_eq!(commands[2].param2, 3);
}

// ============================================================================
// Batch ID and Generation Tests
// ============================================================================

#[test]
fn test_batch_id() {
    let encoder = KgpuCommandEncoderCapsule::<Empty>::with_batch_id(12345);
    assert_eq!(encoder.batch_id(), 12345);

    let encoder = encoder.begin();
    assert_eq!(encoder.batch_id(), 12345);

    let encoder = encoder.finish();
    assert_eq!(encoder.batch_id(), 12345);
}

#[test]
fn test_generation_increments() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let gen0 = encoder.generation();

    let encoder = encoder.begin();
    let gen1 = encoder.generation();
    assert_eq!(gen1, gen0 + 1);

    let encoder = encoder.finish();
    let gen2 = encoder.generation();
    assert_eq!(gen2, gen1 + 1);
}

// ============================================================================
// Command Type Tests
// ============================================================================

#[test]
fn test_command_type_from_u8() {
    assert_eq!(CommandType::from_u8(0), CommandType::Noop);
    assert_eq!(CommandType::from_u8(1), CommandType::CopyBufferToBuffer);
    assert_eq!(CommandType::from_u8(11), CommandType::Draw);
    assert_eq!(CommandType::from_u8(14), CommandType::Dispatch);
    assert_eq!(CommandType::from_u8(22), CommandType::PopDebugGroup);

    // Invalid values map to Noop
    assert_eq!(CommandType::from_u8(23), CommandType::Noop);
    assert_eq!(CommandType::from_u8(255), CommandType::Noop);
}

// ============================================================================
// Draw Command Parameter Verification
// ============================================================================

#[test]
fn test_draw_parameters() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    encoder.draw(36, 10, 100, 5).unwrap();

    let encoder = encoder.finish();
    let cmd = encoder.get(0).unwrap();

    assert_eq!(cmd.command_type(), CommandType::Draw);
    assert_eq!(cmd.param2, 36); // vertex_count

    // Unpack data field
    let data = cmd.data;
    let first_instance = (data >> 48) as u32;
    let instance_count = ((data >> 32) & 0xFFFF) as u32;
    let first_vertex = (data & 0xFFFF_FFFF) as u32;

    assert_eq!(first_instance, 5);
    assert_eq!(instance_count, 10);
    assert_eq!(first_vertex, 100);
}

// ============================================================================
// Dispatch Command Parameter Verification
// ============================================================================

#[test]
fn test_dispatch_parameters() {
    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();

    encoder.dispatch(64, 32, 16).unwrap();

    let encoder = encoder.finish();
    let cmd = encoder.get(0).unwrap();

    assert_eq!(cmd.command_type(), CommandType::Dispatch);
    assert_eq!(cmd.param2, 64); // x

    // Unpack y and z from data
    let y = (cmd.data & 0xFFFF) as u32;
    let z = ((cmd.data >> 16) & 0xFFFF) as u32;

    assert_eq!(y, 32);
    assert_eq!(z, 16);
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

#[test]
fn test_send_sync_bounds() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<KgpuCommandEncoderCapsule<Empty>>();
    assert_send_sync::<KgpuCommandEncoderCapsule<Recording>>();
    assert_send_sync::<KgpuCommandEncoderCapsule<Finished>>();
}

#[test]
fn test_concurrent_reads_finished() {
    use std::sync::Arc;
    use std::thread;

    let encoder = KgpuCommandEncoderCapsule::new();
    let mut encoder = encoder.begin();
    encoder.set_pipeline(1).unwrap();
    encoder.draw(36, 1, 0, 0).unwrap();
    let encoder: Arc<KgpuCommandEncoderCapsule<Finished>> = Arc::new(encoder.finish());

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let enc: Arc<KgpuCommandEncoderCapsule<Finished>> = Arc::clone(&encoder);
            thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = enc.command_count();
                    let _ = enc.generation();
                    let _ = enc.batch_id();
                    let _ = enc.commands();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Verify state unchanged
    assert_eq!(encoder.command_count(), 2);
}
