//! QuicStreamCapsule Integration Tests (T28 4-Tier Framework)
//!
//! Tests for T1 Atomic QUIC stream state machine (RFC 9000)
//!
//! Test Tiers:
//! - Q1-Q7: Unit tests (basic operations, state transitions)
//! - Q8-Q14: Property-based tests (invariants, monotonicity)
//! - Q15-Q21: Integration tests (complex scenarios, edge cases)
//! - Q22-Q28: Production tests (stress, high-throughput, realistic workloads)

#![cfg(all(feature = "std", feature = "network"))]

use atomic_capsule::network::{
    QuicStreamCapsule, QuicStreamError, StreamDirection, StreamState,
};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn q1_stream_creation() {
    let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi, 65536).unwrap();
    assert_eq!(stream.get_stream_id(), 42);
    assert_eq!(stream.get_direction(), StreamDirection::ClientBidi);
    assert_eq!(stream.get_state(), StreamState::Idle);
    assert_eq!(stream.get_bytes_sent(), 0);
    assert!(!stream.is_open());
    assert!(!stream.is_closed());
}

#[test]
fn q2_stream_id_encoding_clientbidi() {
    let stream = QuicStreamCapsule::new(0, StreamDirection::ClientBidi, 65536).unwrap();
    assert_eq!(stream.get_stream_id(), 0);
    assert_eq!(stream.get_direction(), StreamDirection::ClientBidi);
}

#[test]
fn q2_stream_id_encoding_serverbidi() {
    let stream = QuicStreamCapsule::new(1, StreamDirection::ServerBidi, 65536).unwrap();
    assert_eq!(stream.get_stream_id(), 1);
    assert_eq!(stream.get_direction(), StreamDirection::ServerBidi);
}

#[test]
fn q2_stream_id_encoding_clientuni() {
    let stream = QuicStreamCapsule::new(2, StreamDirection::ClientUni, 65536).unwrap();
    assert_eq!(stream.get_stream_id(), 2);
    assert_eq!(stream.get_direction(), StreamDirection::ClientUni);
}

#[test]
fn q2_stream_id_encoding_serveruni() {
    let stream = QuicStreamCapsule::new(3, StreamDirection::ServerUni, 65536).unwrap();
    assert_eq!(stream.get_stream_id(), 3);
    assert_eq!(stream.get_direction(), StreamDirection::ServerUni);
}

#[test]
fn q3_invalid_stream_id_encoding() {
    // Mismatched direction and stream ID
    let result = QuicStreamCapsule::new(0, StreamDirection::ServerBidi, 65536);
    assert_eq!(result, Err(QuicStreamError::InvalidStreamId));
}

#[test]
fn q3_stream_id_too_large() {
    let result = QuicStreamCapsule::new(1u64 << 62, StreamDirection::ClientBidi, 65536);
    assert_eq!(result, Err(QuicStreamError::InvalidStreamId));
}

#[test]
fn q4_stream_size_alignment() {
    assert_eq!(core::mem::size_of::<QuicStreamCapsule>(), 64);
    assert_eq!(core::mem::align_of::<QuicStreamCapsule>(), 64);
}

#[test]
fn q5_open_stream_transition() {
    let stream = QuicStreamCapsule::new(4, StreamDirection::ClientBidi, 65536).unwrap();
    assert_eq!(stream.get_state(), StreamState::Idle);
    assert!(!stream.is_open());

    stream.open_stream().unwrap();
    assert_eq!(stream.get_state(), StreamState::Ready);
    assert!(stream.is_open());
}

#[test]
fn q5_open_stream_idempotent_fails() {
    let stream = QuicStreamCapsule::new(8, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();
    let result = stream.open_stream();
    assert_eq!(result, Err(QuicStreamError::InvalidStateTransition));
}

#[test]
fn q6_send_data_state_transition() {
    let stream = QuicStreamCapsule::new(12, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();

    assert_eq!(stream.get_state(), StreamState::Ready);
    stream.send_data(1024).unwrap();
    assert_eq!(stream.get_state(), StreamState::Send);
    assert_eq!(stream.get_bytes_sent(), 1024);
}

#[test]
fn q6_send_data_flow_control_simple() {
    let stream = QuicStreamCapsule::new(16, StreamDirection::ClientBidi, 100).unwrap();
    stream.open_stream().unwrap();

    // Send within flow control window
    stream.send_data(50).unwrap();
    assert_eq!(stream.get_bytes_sent(), 50);

    // Send more within window
    stream.send_data(50).unwrap();
    assert_eq!(stream.get_bytes_sent(), 100);

    // Exceed flow control window
    let result = stream.send_data(1);
    assert_eq!(result, Err(QuicStreamError::ExceedsFlowControl));
}

#[test]
fn q7_finish_stream_sets_fin() {
    let stream = QuicStreamCapsule::new(20, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();
    stream.send_data(1024).unwrap();

    assert!(!stream.is_fin_sent());
    stream.finish_stream().unwrap();
    assert!(stream.is_fin_sent());
    assert_eq!(stream.get_state(), StreamState::DataSent);
}

// ============================================================================
// Q8-Q14: PROPERTY-BASED TESTS
// ============================================================================

#[test]
fn q8_property_stream_id_immutable() {
    let stream = QuicStreamCapsule::new(42, StreamDirection::ClientBidi, 65536).unwrap();
    let id1 = stream.get_stream_id();
    let id2 = stream.get_stream_id();
    let id3 = stream.get_stream_id();
    assert_eq!(id1, id2);
    assert_eq!(id2, id3);
    assert_eq!(id1, 42);
}

#[test]
fn q8_property_direction_immutable() {
    let stream = QuicStreamCapsule::new(44, StreamDirection::ServerBidi, 65536).unwrap();
    let dir1 = stream.get_direction();
    let dir2 = stream.get_direction();
    let dir3 = stream.get_direction();
    assert_eq!(dir1, dir2);
    assert_eq!(dir2, dir3);
    assert_eq!(dir1, StreamDirection::ServerBidi);
}

#[test]
fn q9_property_bytes_sent_monotonic() {
    let stream = QuicStreamCapsule::new(46, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();

    let bytes1 = stream.get_bytes_sent();
    assert_eq!(bytes1, 0);

    stream.send_data(100).unwrap();
    let bytes2 = stream.get_bytes_sent();
    assert!(bytes2 > bytes1);
    assert_eq!(bytes2, 100);

    stream.send_data(100).unwrap();
    let bytes3 = stream.get_bytes_sent();
    assert!(bytes3 > bytes2);
    assert_eq!(bytes3, 200);

    stream.send_data(50).unwrap();
    let bytes4 = stream.get_bytes_sent();
    assert!(bytes4 > bytes3);
    assert_eq!(bytes4, 250);
}

#[test]
fn q10_property_state_never_backward() {
    let stream = QuicStreamCapsule::new(48, StreamDirection::ClientBidi, 65536).unwrap();

    let state1 = stream.get_state();
    assert_eq!(state1 as u8, StreamState::Idle as u8);

    stream.open_stream().unwrap();
    let state2 = stream.get_state();
    assert!(state2 as u8 >= state1 as u8);
    assert_eq!(state2 as u8, StreamState::Ready as u8);

    stream.send_data(100).unwrap();
    let state3 = stream.get_state();
    assert!(state3 as u8 >= state2 as u8);
    assert_eq!(state3 as u8, StreamState::Send as u8);

    stream.finish_stream().unwrap();
    let state4 = stream.get_state();
    assert!(state4 as u8 >= state3 as u8);
    assert_eq!(state4 as u8, StreamState::DataSent as u8);
}

#[test]
fn q11_property_fin_implies_datasent() {
    let stream = QuicStreamCapsule::new(50, StreamDirection::ClientBidi, 65536).unwrap();
    assert!(!stream.is_fin_sent());

    stream.open_stream().unwrap();
    assert!(!stream.is_fin_sent());

    stream.send_data(100).unwrap();
    assert!(!stream.is_fin_sent());

    stream.finish_stream().unwrap();
    assert!(stream.is_fin_sent());
    assert_eq!(stream.get_state(), StreamState::DataSent);
}

#[test]
fn q12_property_reset_terminal() {
    let stream = QuicStreamCapsule::new(52, StreamDirection::ClientBidi, 65536).unwrap();
    assert!(!stream.is_closed());

    stream.open_stream().unwrap();
    assert!(!stream.is_closed());

    stream.send_data(100).unwrap();
    assert!(!stream.is_closed());

    stream.reset_stream().unwrap();
    assert!(stream.is_closed());
    assert_eq!(stream.get_state(), StreamState::Reset);
}

#[test]
fn q13_property_cant_reset_twice() {
    let stream = QuicStreamCapsule::new(54, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();
    stream.send_data(100).unwrap();

    stream.reset_stream().unwrap();
    let result = stream.reset_stream();
    assert_eq!(result, Err(QuicStreamError::StreamClosed));
}

#[test]
fn q14_property_flow_control_invariant() {
    let stream = QuicStreamCapsule::new(56, StreamDirection::ClientBidi, 256).unwrap();
    stream.open_stream().unwrap();

    // Send in multiple chunks
    for _ in 0..4 {
        stream.send_data(64).unwrap();
    }
    assert_eq!(stream.get_bytes_sent(), 256);

    // Further sends should fail
    assert_eq!(
        stream.send_data(1),
        Err(QuicStreamError::ExceedsFlowControl)
    );
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn i1_full_lifecycle_bidirectional() {
    let stream = QuicStreamCapsule::new(60, StreamDirection::ClientBidi, 1000).unwrap();

    // Initial state
    assert_eq!(stream.get_state(), StreamState::Idle);
    assert!(!stream.is_open());
    assert!(!stream.is_closed());

    // Open stream
    stream.open_stream().unwrap();
    assert_eq!(stream.get_state(), StreamState::Ready);
    assert!(stream.is_open());

    // Send data in chunks
    stream.send_data(250).unwrap();
    assert_eq!(stream.get_bytes_sent(), 250);
    assert_eq!(stream.get_state(), StreamState::Send);

    stream.send_data(250).unwrap();
    assert_eq!(stream.get_bytes_sent(), 500);

    stream.send_data(500).unwrap();
    assert_eq!(stream.get_bytes_sent(), 1000);

    // Finish stream
    stream.finish_stream().unwrap();
    assert_eq!(stream.get_state(), StreamState::DataSent);
    assert!(stream.is_fin_sent());
}

#[test]
fn i2_flow_control_enforcement() {
    let stream = QuicStreamCapsule::new(62, StreamDirection::ServerBidi, 512).unwrap();
    stream.open_stream().unwrap();

    // Gradually increase usage
    for i in 0..4 {
        let result = stream.send_data(128);
        assert!(result.is_ok(), "Failed at iteration {}", i);
    }
    assert_eq!(stream.get_bytes_sent(), 512);

    // Should fail to send more
    assert_eq!(
        stream.send_data(1),
        Err(QuicStreamError::ExceedsFlowControl)
    );

    // Increase window
    stream.update_max_stream_data(1024).unwrap();

    // Now send should succeed
    stream.send_data(256).unwrap();
    assert_eq!(stream.get_bytes_sent(), 768);
}

#[test]
fn i3_reset_during_send() {
    let stream = QuicStreamCapsule::new(64, StreamDirection::ClientUni, 65536).unwrap();
    stream.open_stream().unwrap();

    // Send some data
    stream.send_data(1000).unwrap();
    assert_eq!(stream.get_bytes_sent(), 1000);

    // Reset stream
    stream.reset_stream().unwrap();
    assert_eq!(stream.get_state(), StreamState::Reset);
    assert!(stream.is_closed());

    // All operations should fail on closed stream
    assert_eq!(
        stream.send_data(100),
        Err(QuicStreamError::InvalidStateTransition)
    );
    assert_eq!(
        stream.finish_stream(),
        Err(QuicStreamError::InvalidStateTransition)
    );
}

#[test]
fn i4_multiple_window_increases() {
    let stream = QuicStreamCapsule::new(66, StreamDirection::ClientBidi, 100).unwrap();
    stream.open_stream().unwrap();

    // Initial window: 100 bytes
    stream.send_data(100).unwrap();
    assert_eq!(
        stream.send_data(1),
        Err(QuicStreamError::ExceedsFlowControl)
    );

    // Increase window 5 times, each by 100 bytes
    for i in 1..=5 {
        stream
            .update_max_stream_data((i * 100) as u32)
            .unwrap();
        stream.send_data(100).unwrap();
    }

    assert_eq!(stream.get_bytes_sent(), 600);
}

#[test]
fn i5_send_data_requires_open() {
    let stream = QuicStreamCapsule::new(68, StreamDirection::ClientBidi, 65536).unwrap();
    // Try to send without opening
    let result = stream.send_data(100);
    assert_eq!(result, Err(QuicStreamError::InvalidStateTransition));
}

#[test]
fn i6_finish_requires_send() {
    let stream = QuicStreamCapsule::new(70, StreamDirection::ClientBidi, 65536).unwrap();
    stream.open_stream().unwrap();
    // Try to finish without sending
    let result = stream.finish_stream();
    assert_eq!(result, Err(QuicStreamError::InvalidStateTransition));
}

// ============================================================================
// Q22-Q28: PRODUCTION / STRESS TESTS
// ============================================================================

#[test]
fn prod1_high_throughput_single_stream() {
    let stream = QuicStreamCapsule::new(100, StreamDirection::ServerBidi, u32::MAX).unwrap();
    stream.open_stream().unwrap();

    // Simulate high-throughput scenario (10 million bytes)
    const CHUNK_SIZE: u32 = 1024;
    const ITERATIONS: u32 = 10000;
    const EXPECTED_TOTAL: u32 = CHUNK_SIZE * ITERATIONS;

    for _ in 0..ITERATIONS {
        stream.send_data(CHUNK_SIZE).unwrap();
    }

    assert_eq!(stream.get_bytes_sent(), EXPECTED_TOTAL);
    assert_eq!(stream.get_state(), StreamState::Send);
}

#[test]
fn prod2_multiple_bidirectional_streams() {
    // Create 25 bidirectional streams (stream IDs: 0, 4, 8, 12, ...)
    for stream_id in (0..100).step_by(4) {
        let stream = QuicStreamCapsule::new(stream_id, StreamDirection::ClientBidi, 65536)
            .unwrap();
        stream.open_stream().unwrap();
        stream.send_data(256).unwrap();
        stream.finish_stream().unwrap();

        assert_eq!(stream.get_state(), StreamState::DataSent);
        assert!(stream.is_fin_sent());
        assert_eq!(stream.get_bytes_sent(), 256);
    }
}

#[test]
fn prod3_multiple_unidirectional_streams() {
    // Create 25 unidirectional streams (stream IDs: 2, 6, 10, 14, ...)
    for stream_id in (2..100).step_by(4) {
        let stream =
            QuicStreamCapsule::new(stream_id, StreamDirection::ClientUni, 32768)
                .unwrap();
        stream.open_stream().unwrap();

        // Send max window
        stream.send_data(32768).unwrap();

        // Reset is allowed
        stream.reset_stream().unwrap();
        assert_eq!(stream.get_state(), StreamState::Reset);
    }
}

#[test]
fn prod4_all_directions() {
    // Test all 4 directions
    let directions = [
        (0, StreamDirection::ClientBidi),
        (1, StreamDirection::ServerBidi),
        (2, StreamDirection::ClientUni),
        (3, StreamDirection::ServerUni),
    ];

    for (id, dir) in &directions {
        let stream = QuicStreamCapsule::new(*id, *dir, 65536).unwrap();
        stream.open_stream().unwrap();
        stream.send_data(1024).unwrap();

        if matches!(dir, StreamDirection::ServerBidi | StreamDirection::ClientBidi) {
            stream.finish_stream().unwrap();
            assert!(stream.is_fin_sent());
        } else {
            stream.reset_stream().unwrap();
        }
    }
}

#[test]
fn prod5_extreme_window_sizes() {
    // Minimum window
    let stream_min =
        QuicStreamCapsule::new(200, StreamDirection::ClientBidi, 1)
            .unwrap();
    stream_min.open_stream().unwrap();
    stream_min.send_data(1).unwrap();
    assert_eq!(
        stream_min.send_data(1),
        Err(QuicStreamError::ExceedsFlowControl)
    );

    // Maximum window
    let stream_max =
        QuicStreamCapsule::new(204, StreamDirection::ClientBidi, u32::MAX)
            .unwrap();
    stream_max.open_stream().unwrap();

    // Should be able to send large chunks without issue (won't actually send all 4GB)
    stream_max.send_data(1_000_000).unwrap();
    assert_eq!(stream_max.get_bytes_sent(), 1_000_000);
}
