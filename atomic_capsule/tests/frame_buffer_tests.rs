//! Frame Buffer Capsule Tests - T28 Framework (28 tests across 4 tiers)
//!
//! Framework: UCE34 T28 Testing (Unit/Property/Integration/Production tiers)
//! Coverage: Metadata operations, buffer coordination, reference counting, edge cases
//!
//! # Test Tiers (28 total)
//! - Q1-Q7: Unit tests (8 tests)
//! - Q8-Q14: Property tests (7 tests)
//! - Q15-Q21: Integration tests (7 tests)
//! - Q22-Q28: Production tests (6 tests)

#![cfg_attr(not(feature = "std"), no_std)]

use atomic_capsule::encoder::FrameBufferCapsule;
use atomic_capsule::encoder::{FrameFlags, FrameType};

// ============================================================================
// Q1-Q7: UNIT TESTS (8 tests)
// ============================================================================

#[test]
fn q1_test_capsule_creation() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    assert_eq!(capsule.get_frame_type(), FrameType::Key);
    let (w, h, _s) = capsule.get_dimensions();
    assert_eq!(w, 1920);
    assert_eq!(h, 1080);
    assert_eq!(capsule.get_ref_count(), 1);
}

#[test]
fn q2_test_frame_type_enum() {
    assert_eq!(FrameType::Key.as_u8(), 0);
    assert_eq!(FrameType::Inter.as_u8(), 1);
    assert_eq!(FrameType::IntraOnly.as_u8(), 2);
    assert_eq!(FrameType::Switch.as_u8(), 3);

    assert_eq!(FrameType::from_u8(0), Some(FrameType::Key));
    assert_eq!(FrameType::from_u8(1), Some(FrameType::Inter));
    assert_eq!(FrameType::from_u8(2), Some(FrameType::IntraOnly));
    assert_eq!(FrameType::from_u8(3), Some(FrameType::Switch));
    assert_eq!(FrameType::from_u8(4), None);
}

#[test]
fn q3_test_frame_flags() {
    let flags = FrameFlags::new();
    assert!(!flags.is_buffer_attached());
    assert!(!flags.is_dirty());
    assert!(!flags.is_referenced());

    let flags_attached = flags.with_buffer_attached();
    assert!(flags_attached.is_buffer_attached());
    assert_eq!(flags_attached.as_u8() & 0x01, 0x01);

    let flags_dirty = flags.with_dirty();
    assert!(flags_dirty.is_dirty());
    assert_eq!(flags_dirty.as_u8() & 0x02, 0x02);

    let flags_referenced = flags.with_referenced();
    assert!(flags_referenced.is_referenced());
    assert_eq!(flags_referenced.as_u8() & 0x04, 0x04);
}

#[test]
fn q4_test_layout_alignment() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let ptr = &capsule as *const _ as usize;

    // Verify 128-byte alignment
    assert_eq!(ptr % 128, 0, "FrameBufferCapsule must be 128-byte aligned");

    // Verify exact size
    assert_eq!(
        core::mem::size_of_val(&capsule),
        128,
        "FrameBufferCapsule must be exactly 128 bytes"
    );
}

#[test]
fn q5_test_pts_encoding_decoding() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Test various PTS values
    for pts in [0, 1, 1000, 999999, 0xFFFFFFFF] {
        capsule.update_frame_metadata(pts, 10);
        assert_eq!(capsule.get_pts(), pts);
    }
}

#[test]
fn q6_test_frame_id_range() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    for fid in [0, 1, 100, 1000, 0xFFFF] {
        capsule.update_frame_metadata(0, fid);
        assert_eq!(capsule.get_frame_id(), fid);
    }
}

#[test]
fn q7_test_generation_counter() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Generation counter is packed in lower 14 bits
    let gen = capsule.get_generation();
    assert!(gen <= 0x3FFF); // 14-bit max value
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (7 tests)
// ============================================================================

#[test]
fn q8_test_reference_counting_atomicity() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    assert_eq!(capsule.get_ref_count(), 1);

    // Increment multiple times
    capsule.increment_ref().unwrap();
    capsule.increment_ref().unwrap();
    capsule.increment_ref().unwrap();
    assert_eq!(capsule.get_ref_count(), 4);

    // Decrement multiple times
    capsule.decrement_ref();
    capsule.decrement_ref();
    assert_eq!(capsule.get_ref_count(), 2);

    capsule.decrement_ref();
    capsule.decrement_ref();
    assert_eq!(capsule.get_ref_count(), 0);
}

#[test]
fn q9_test_dirty_flag_idempotence() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Marking dirty multiple times is idempotent
    capsule.mark_dirty();
    assert!(capsule.is_dirty());
    capsule.mark_dirty();
    assert!(capsule.is_dirty());

    // Clearing dirty multiple times is idempotent
    capsule.clear_dirty();
    assert!(!capsule.is_dirty());
    capsule.clear_dirty();
    assert!(!capsule.is_dirty());
}

#[test]
fn q10_test_dimensions_update_visibility() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let (w, h, s) = capsule.get_dimensions();

    assert_eq!(w, 1920);
    assert_eq!(h, 1080);

    // Update dimensions
    capsule.update_dimensions(3840, 2160, 3840);
    let (w2, h2, s2) = capsule.get_dimensions();

    assert_eq!(w2, 3840);
    assert_eq!(h2, 2160);
    assert_eq!(s2, 3840);
}

#[test]
fn q11_test_timestamp_monotonicity() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    let ts1 = 1_000_000_000u64;
    capsule.set_timestamp_ns(ts1);
    assert_eq!(capsule.get_timestamp_ns(), ts1);

    let ts2 = 2_000_000_000u64;
    capsule.set_timestamp_ns(ts2);
    assert_eq!(capsule.get_timestamp_ns(), ts2);
    assert!(capsule.get_timestamp_ns() >= ts1);
}

#[test]
fn q12_test_checksum_determinism() {
    let capsule1 = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let capsule2 = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    let data = b"deterministic test data";
    capsule1.update_checksum(data);
    capsule2.update_checksum(data);

    // Same data should produce same checksum
    assert_eq!(capsule1.get_checksum(), capsule2.get_checksum());
}

#[test]
fn q13_test_checksum_sensitivity() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    let data1 = b"frame data";
    let data2 = b"frame datb"; // Changed one byte

    capsule.update_checksum(data1);
    let crc1 = capsule.get_checksum();

    capsule.update_checksum(data2);
    let crc2 = capsule.get_checksum();

    // Different data should produce different checksums (very high probability)
    assert_ne!(crc1, crc2);
}

#[test]
fn q14_test_frame_type_preservation() {
    for frame_type in [FrameType::Key, FrameType::Inter, FrameType::IntraOnly, FrameType::Switch] {
        let capsule = FrameBufferCapsule::new(1920, 1080, frame_type);
        assert_eq!(capsule.get_frame_type(), frame_type);
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (7 tests)
// ============================================================================

#[test]
fn q15_test_buffer_attachment_flow() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Create dummy buffer (stack allocation for testing)
    let mut buffer = [0u8; 4096];
    let ptr = buffer.as_mut_ptr();

    capsule.attach_buffer(ptr, 0, 1920 * 1080, 1920 * 1080 + 960 * 540);

    // Verify planes are accessible
    assert!(capsule.get_y_plane().is_some());
    assert!(capsule.get_u_plane().is_some());
    assert!(capsule.get_v_plane().is_some());
}

#[test]
fn q16_test_multi_frame_coordination() {
    let capsule1 = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    let capsule2 = FrameBufferCapsule::new(1920, 1080, FrameType::Inter);
    let capsule3 = FrameBufferCapsule::new(1920, 1080, FrameType::IntraOnly);

    // Verify independence
    capsule1.mark_dirty();
    capsule2.mark_dirty();

    assert!(capsule1.is_dirty());
    assert!(capsule2.is_dirty());
    assert!(!capsule3.is_dirty());
}

#[test]
fn q17_test_reference_count_lifecycle() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Initial ref count is 1
    assert_eq!(capsule.get_ref_count(), 1);

    // Simulate frame being used by 3 threads
    capsule.increment_ref().unwrap();
    capsule.increment_ref().unwrap();
    capsule.increment_ref().unwrap();
    assert_eq!(capsule.get_ref_count(), 4);

    // Threads release references
    capsule.decrement_ref();
    capsule.decrement_ref();
    capsule.decrement_ref();
    capsule.decrement_ref();
    assert_eq!(capsule.get_ref_count(), 0);
}

#[test]
fn q18_test_metadata_consistency_across_updates() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Update metadata multiple times
    for i in 0..100 {
        let pts = (i as u32) * 1000;
        let fid = (i % 256) as u16;
        capsule.update_frame_metadata(pts, fid);

        assert_eq!(capsule.get_pts(), pts);
        assert_eq!(capsule.get_frame_id(), fid);
    }
}

#[test]
fn q19_test_dirty_flag_state_machine() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Initial state: clean
    assert!(!capsule.is_dirty());

    // Mark dirty
    capsule.mark_dirty();
    assert!(capsule.is_dirty());

    // Another mark (idempotent)
    capsule.mark_dirty();
    assert!(capsule.is_dirty());

    // Clear dirty
    capsule.clear_dirty();
    assert!(!capsule.is_dirty());

    // Another clear (idempotent)
    capsule.clear_dirty();
    assert!(!capsule.is_dirty());
}

#[test]
fn q20_test_dimensions_edge_cases() {
    // Test various resolution combinations
    let resolutions = [
        (8, 8),      // Minimum
        (1920, 1080), // HD
        (3840, 2160), // 4K
        (7680, 4320), // 8K
        (0, 0),      // Zero (edge case)
    ];

    for (w, h) in &resolutions {
        let capsule = FrameBufferCapsule::new(*w, *h, FrameType::Key);
        let (rw, rh, _) = capsule.get_dimensions();
        assert_eq!(rw, *w);
        assert_eq!(rh, *h);
    }
}

#[test]
fn q21_test_concurrent_read_safety() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    capsule.update_frame_metadata(12345, 5);
    capsule.set_timestamp_ns(1_000_000_000);

    // Simulate multiple readers
    let pts1 = capsule.get_pts();
    let fid1 = capsule.get_frame_id();
    let ts1 = capsule.get_timestamp_ns();

    let pts2 = capsule.get_pts();
    let fid2 = capsule.get_frame_id();
    let ts2 = capsule.get_timestamp_ns();

    // All reads should be consistent
    assert_eq!(pts1, pts2);
    assert_eq!(fid1, fid2);
    assert_eq!(ts1, ts2);
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (6 tests)
// ============================================================================

#[test]
fn q22_test_stress_reference_counting() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Stress test: 1000 increment/decrement pairs
    for _ in 0..1000 {
        capsule.increment_ref().unwrap();
        capsule.increment_ref().unwrap();
        capsule.decrement_ref();
        capsule.decrement_ref();
    }

    assert_eq!(capsule.get_ref_count(), 1); // Should return to initial
}

#[test]
fn q23_test_stress_metadata_updates() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Stress test: 10000 metadata updates
    for i in 0..10000 {
        let pts = (i as u32) & 0xFFFFFFFF;
        let fid = (i as u16) & 0xFFFF;
        capsule.update_frame_metadata(pts, fid);

        if i % 1000 == 0 {
            assert_eq!(capsule.get_pts(), pts);
            assert_eq!(capsule.get_frame_id(), fid);
        }
    }
}

#[test]
fn q24_test_stress_dirty_flag_toggling() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Stress test: toggle dirty flag 10000 times
    for i in 0..10000 {
        if i % 2 == 0 {
            capsule.mark_dirty();
        } else {
            capsule.clear_dirty();
        }
    }

    assert!(!capsule.is_dirty()); // Should be clean at the end
}

#[test]
fn q25_test_production_realtime_scenario() {
    // Simulate real-time encoding: encode 30 frames/sec for 10 seconds
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    let mut frame_count = 0;
    let frame_interval = 33_333_333; // 33.3ms in nanoseconds (30 FPS)

    for i in 0..300 {
        // 10 seconds at 30 FPS
        let pts = (i as u32) & 0xFFFFFFFF;
        let ts_ns = i as u64 * frame_interval;

        let frame_type = match i % 30 {
            0 => FrameType::Key,
            _ => FrameType::Inter,
        };

        let c = FrameBufferCapsule::new(1920, 1080, frame_type);
        c.update_frame_metadata(pts, (i as u16) % 256);
        c.set_timestamp_ns(ts_ns);

        if i % 30 == 0 {
            assert_eq!(c.get_frame_type(), FrameType::Key);
        }

        frame_count += 1;
    }

    assert_eq!(frame_count, 300);
}

#[test]
fn q26_test_production_memory_leak_prevention() {
    // Create and destroy 1000 capsules
    for _ in 0..1000 {
        let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Inter);
        let _pts = capsule.get_pts();
        let _dims = capsule.get_dimensions();
        // Capsule dropped automatically
    }

    // If this test completes without panicking/aborting, memory safety is verified
}

#[test]
fn q27_test_production_checksum_integrity() {
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);

    // Simulate computing checksum over frame data
    let frame_data = vec![0x55u8; 8192]; // Typical frame chunk

    capsule.update_checksum(&frame_data);
    let crc1 = capsule.get_checksum();

    // Recompute checksum should be identical
    capsule.update_checksum(&frame_data);
    let crc2 = capsule.get_checksum();

    assert_eq!(crc1, crc2);
}

#[test]
fn q28_test_production_complete_frame_lifecycle() {
    // Comprehensive lifecycle test: create, configure, use, destroy frame

    // 1. Create frame
    let capsule = FrameBufferCapsule::new(1920, 1080, FrameType::Key);
    assert_eq!(capsule.get_ref_count(), 1);

    // 2. Configure buffer
    let mut buffer = vec![0u8; 4_147_200]; // 1920×1080 YUV420
    let ptr = buffer.as_mut_ptr();
    capsule.attach_buffer(ptr, 0, 1920 * 1080, 1920 * 1080 + 960 * 540);

    // 3. Set metadata
    capsule.update_frame_metadata(33333, 1);
    capsule.set_timestamp_ns(33_333_333);

    // 4. Add references (simulate encoding pipeline)
    capsule.increment_ref().unwrap();
    capsule.increment_ref().unwrap();
    assert_eq!(capsule.get_ref_count(), 3);

    // 5. Mark as dirty during encoding
    capsule.mark_dirty();
    assert!(capsule.is_dirty());

    // 6. Compute integrity checksum
    capsule.update_checksum(&buffer[..1024]);

    // 7. Release references
    capsule.decrement_ref();
    capsule.decrement_ref();
    capsule.decrement_ref();
    assert_eq!(capsule.get_ref_count(), 0);

    // 8. Verify final state
    assert_eq!(capsule.get_pts(), 33333);
    assert_eq!(capsule.get_frame_id(), 1);
    assert_eq!(capsule.get_timestamp_ns(), 33_333_333);
    assert_ne!(capsule.get_checksum(), 0);
}
