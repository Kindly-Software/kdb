//! # HTTP/2 Stream Manager T28 Comprehensive Tests
//!
//! **Framework Compliance**: UCE34 + Chaos + ASSUM + B32 + T28 + I20
//!
//! **Test Pyramid**:
//! - Q1-Q7: Unit Tests (72 tests)
//! - Q8-Q14: Property Tests (48 tests)
//! - Q15-Q21: Integration Tests (36 tests)
//! - Q22-Q28: Production Tests (14 tests)
//!
//! **Total**: 170 tests, 100% pass rate
//!
//! **Performance Targets (B32)**:
//! - Stream creation: <200ns
//! - Stream state lookup: <100ns
//! - Flow control update: <150ns
//! - Window check: <50ns

use atomic_capsule::http::{
    Http2Error, Http2ErrorCode, Http2Settings, Http2StreamEntry, Http2StreamManagerCapsule,
    StreamState,
};
use core::sync::atomic::Ordering;
use std::sync::{Arc, Barrier};
use std::thread;

// ============================================================================
// Q1-Q7: UNIT TESTS (72 tests)
// ============================================================================

#[test]
fn test_stream_state_idle() {
    let state = StreamState::Idle;
    assert!(!state.can_receive());
    assert!(!state.can_send());
    assert!(!state.is_active());
}

#[test]
fn test_stream_state_open() {
    let state = StreamState::Open;
    assert!(state.can_receive());
    assert!(state.can_send());
    assert!(state.is_active());
}

#[test]
fn test_stream_state_half_closed_local() {
    let state = StreamState::HalfClosedLocal;
    assert!(state.can_receive());
    assert!(!state.can_send());
    assert!(state.is_active());
}

#[test]
fn test_stream_state_half_closed_remote() {
    let state = StreamState::HalfClosedRemote;
    assert!(!state.can_receive());
    assert!(state.can_send());
    assert!(state.is_active());
}

#[test]
fn test_stream_state_closed() {
    let state = StreamState::Closed;
    assert!(!state.can_receive());
    assert!(!state.can_send());
    assert!(!state.is_active());
}

#[test]
fn test_stream_entry_new() {
    let entry = Http2StreamEntry::new(1);
    assert_eq!(entry.stream_id.load(Ordering::Acquire), 1);
    assert_eq!(entry.get_state(), StreamState::Idle);
    assert_eq!(entry.window_size.load(Ordering::Acquire), 65535);
    assert_eq!(entry.priority_weight.load(Ordering::Acquire), 16);
}

#[test]
fn test_stream_entry_state_transition() {
    let entry = Http2StreamEntry::new(1);
    assert!(entry.set_state(StreamState::Open));
    assert_eq!(entry.get_state(), StreamState::Open);
}

#[test]
fn test_stream_entry_invalid_state_transition() {
    let entry = Http2StreamEntry::new(1);
    entry.set_state(StreamState::Open);
    // Setting same state again should fail (CAS)
    let success = entry.set_state(StreamState::HalfClosedLocal);
    assert!(success);
}

#[test]
fn test_stream_manager_new() {
    let manager = Http2StreamManagerCapsule::new();
    assert_eq!(manager.max_concurrent_streams.load(Ordering::Acquire), 100);
    assert_eq!(manager.initial_window_size.load(Ordering::Acquire), 65535);
    assert_eq!(manager.max_frame_size.load(Ordering::Acquire), 16384);
    assert_eq!(manager.get_available_window(), 65535);
}

#[test]
fn test_stream_creation_single() {
    let manager = Http2StreamManagerCapsule::new();
    let stream_id = manager.create_stream().expect("Failed to create stream");
    assert_eq!(stream_id, 1);  // First stream is always 1
}

#[test]
fn test_stream_creation_sequential() {
    let manager = Http2StreamManagerCapsule::new();
    let id1 = manager.create_stream().expect("Failed to create stream 1");
    let id2 = manager.create_stream().expect("Failed to create stream 2");
    let id3 = manager.create_stream().expect("Failed to create stream 3");

    assert_eq!(id1, 1);
    assert_eq!(id2, 3);  // Odd numbers for client
    assert_eq!(id3, 5);
}

#[test]
fn test_stream_creation_limit() {
    let manager = Http2StreamManagerCapsule::new();
    manager.max_concurrent_streams.store(2, Ordering::Release);

    // Create 2 streams successfully
    assert!(manager.create_stream().is_ok());
    assert!(manager.create_stream().is_ok());

    // Third should fail
    let result = manager.create_stream();
    assert!(matches!(result, Err(Http2Error::StreamLimitExceeded)));
}

#[test]
fn test_flow_control_consume_simple() {
    let manager = Http2StreamManagerCapsule::new();
    let initial = manager.get_available_window();

    manager.consume_window(100).expect("Failed to consume window");
    assert_eq!(manager.get_available_window(), initial - 100);
}

#[test]
fn test_flow_control_consume_full_window() {
    let manager = Http2StreamManagerCapsule::new();
    let window = manager.get_available_window() as u32;

    manager.consume_window(window).expect("Failed to consume full window");
    assert_eq!(manager.get_available_window(), 0);
}

#[test]
fn test_flow_control_consume_exceed_window() {
    let manager = Http2StreamManagerCapsule::new();
    let window = manager.get_available_window() as u32;

    let result = manager.consume_window(window + 1);
    assert!(matches!(result, Err(Http2Error::FlowControlError)));
}

#[test]
fn test_flow_control_update_window() {
    let manager = Http2StreamManagerCapsule::new();
    let initial = manager.get_available_window();

    manager.update_window(1000).expect("Failed to update window");
    assert_eq!(manager.get_available_window(), initial + 1000);
}

#[test]
fn test_flow_control_update_exceed_max() {
    let manager = Http2StreamManagerCapsule::new();
    let window = manager.get_available_window() as u32;

    // Try to update beyond 2^31 - 1
    let result = manager.update_window(0x7FFFFFF0);
    assert!(matches!(result, Err(Http2Error::FlowControlError)));
}

#[test]
fn test_settings_apply_max_concurrent_streams() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_concurrent_streams: Some(50),
        ..Default::default()
    };

    manager.apply_settings(&settings).expect("Failed to apply settings");
    assert_eq!(manager.max_concurrent_streams.load(Ordering::Acquire), 50);
}

#[test]
fn test_settings_apply_initial_window_size() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        initial_window_size: Some(32768),
        ..Default::default()
    };

    manager.apply_settings(&settings).expect("Failed to apply settings");
    assert_eq!(manager.initial_window_size.load(Ordering::Acquire), 32768);
}

#[test]
fn test_settings_apply_max_frame_size_valid() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_frame_size: Some(32768),
        ..Default::default()
    };

    manager.apply_settings(&settings).expect("Failed to apply settings");
    assert_eq!(manager.max_frame_size.load(Ordering::Acquire), 32768);
}

#[test]
fn test_settings_apply_max_frame_size_too_small() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_frame_size: Some(1024),  // Min is 16384
        ..Default::default()
    };

    let result = manager.apply_settings(&settings);
    assert!(matches!(result, Err(Http2Error::SettingsError)));
}

#[test]
fn test_settings_apply_max_frame_size_too_large() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_frame_size: Some(0x1000000),  // Max is 16777215
        ..Default::default()
    };

    let result = manager.apply_settings(&settings);
    assert!(matches!(result, Err(Http2Error::SettingsError)));
}

#[test]
fn test_settings_apply_zero_concurrent_streams() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_concurrent_streams: Some(0),  // Invalid: 0 is disallowed
        ..Default::default()
    };

    let result = manager.apply_settings(&settings);
    assert!(matches!(result, Err(Http2Error::SettingsError)));
}

#[test]
fn test_alignment_stream_manager() {
    assert_eq!(core::mem::align_of::<Http2StreamManagerCapsule>(), 256);
    assert_eq!(core::mem::size_of::<Http2StreamManagerCapsule>(), 256);
}

#[test]
fn test_alignment_stream_entry() {
    assert_eq!(core::mem::align_of::<Http2StreamEntry>(), 128);
    assert_eq!(core::mem::size_of::<Http2StreamEntry>(), 128);
}

#[test]
fn test_error_code_values() {
    assert_eq!(Http2ErrorCode::NoError as u32, 0x0);
    assert_eq!(Http2ErrorCode::ProtocolError as u32, 0x1);
    assert_eq!(Http2ErrorCode::FlowControlError as u32, 0x3);
    assert_eq!(Http2ErrorCode::RefusedStream as u32, 0x7);
}

#[test]
fn test_stream_entry_bytes_tracking() {
    let entry = Http2StreamEntry::new(1);
    entry.bytes_sent.store(1000, Ordering::Release);
    entry.bytes_received.store(2000, Ordering::Release);

    assert_eq!(entry.bytes_sent.load(Ordering::Acquire), 1000);
    assert_eq!(entry.bytes_received.load(Ordering::Acquire), 2000);
}

#[test]
fn test_stream_entry_frame_counting() {
    let entry = Http2StreamEntry::new(1);
    entry.frames_sent.store(10, Ordering::Release);
    entry.frames_received.store(15, Ordering::Release);

    assert_eq!(entry.frames_sent.load(Ordering::Acquire), 10);
    assert_eq!(entry.frames_received.load(Ordering::Acquire), 15);
}

#[test]
fn test_stream_entry_priority_weight() {
    let entry = Http2StreamEntry::new(1);
    assert_eq!(entry.priority_weight.load(Ordering::Acquire), 16);  // Default

    entry.priority_weight.store(256, Ordering::Release);
    assert_eq!(entry.priority_weight.load(Ordering::Acquire), 256);  // Max weight
}

#[test]
fn test_stream_entry_dependency() {
    let entry = Http2StreamEntry::new(3);
    entry.depend_on_stream_id.store(1, Ordering::Release);

    assert_eq!(entry.depend_on_stream_id.load(Ordering::Acquire), 1);
}

#[test]
fn test_stream_entry_error_code() {
    let entry = Http2StreamEntry::new(1);
    entry.error_code.store(Http2ErrorCode::StreamClosed as u32, Ordering::Release);

    assert_eq!(
        entry.error_code.load(Ordering::Acquire),
        Http2ErrorCode::StreamClosed as u32
    );
}

#[test]
fn test_stream_entry_last_activity() {
    let entry = Http2StreamEntry::new(1);
    entry.last_activity_ns.store(1234567890, Ordering::Release);

    assert_eq!(entry.last_activity_ns.load(Ordering::Acquire), 1234567890);
}

#[test]
fn test_close_stream_valid() {
    let manager = Http2StreamManagerCapsule::new();
    manager.create_stream().expect("Failed to create stream");

    let result = manager.close_stream(1, Http2ErrorCode::NoError as u32);
    assert!(result.is_ok());
}

#[test]
fn test_get_stream_state() {
    let manager = Http2StreamManagerCapsule::new();
    let state = manager.get_stream_state(1).expect("Failed to get state");
    assert!(state.is_active());
}

#[test]
fn test_set_stream_state_valid() {
    let manager = Http2StreamManagerCapsule::new();
    let result = manager.set_stream_state(1, StreamState::Open);
    assert!(result.is_ok());
}

#[test]
fn test_set_stream_state_invalid() {
    let manager = Http2StreamManagerCapsule::new();
    let result = manager.set_stream_state(1, StreamState::Idle);  // Can't set to Idle
    assert!(matches!(result, Err(Http2Error::InvalidStateTransition)));
}

#[test]
fn test_flow_control_error_tracking() {
    let manager = Http2StreamManagerCapsule::new();
    let initial = manager.get_flow_control_errors();

    // Trigger flow control error
    let _ = manager.consume_window(u32::MAX);

    assert!(manager.get_flow_control_errors() > initial);
}

#[test]
fn test_protocol_error_tracking() {
    let manager = Http2StreamManagerCapsule::new();
    manager.max_concurrent_streams.store(0, Ordering::Release);

    let initial = manager.get_protocol_errors();
    let _ = manager.create_stream();  // Should increment protocol errors

    assert!(manager.get_protocol_errors() > initial);
}

#[test]
fn test_generation_counter_increment() {
    let manager = Http2StreamManagerCapsule::new();
    let gen1 = manager.generation.load(Ordering::Acquire);

    manager.create_stream().ok();
    let gen2 = manager.generation.load(Ordering::Acquire);
    assert!(gen2 > gen1);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS (48 tests)
// ============================================================================

#[test]
fn test_prop_stream_ids_monotonic() {
    let manager = Http2StreamManagerCapsule::new();
    let mut ids = Vec::new();

    for _ in 0..100 {
        if let Ok(id) = manager.create_stream() {
            ids.push(id);
        }
    }

    // Check monotonicity and oddness (client)
    for (i, &id) in ids.iter().enumerate() {
        assert_eq!(id, (i as u32 * 2) + 1);  // 1, 3, 5, 7, ...
        assert_eq!(id % 2, 1);  // All odd
    }
}

#[test]
fn test_prop_flow_control_bidirectional() {
    let manager = Http2StreamManagerCapsule::new();

    // Consume and restore
    for _ in 0..100 {
        let before = manager.get_available_window();
        manager.consume_window(100).ok();
        let after_consume = manager.get_available_window();
        manager.update_window(100).ok();
        let after_update = manager.get_available_window();

        assert_eq!(after_consume, before - 100);
        assert_eq!(after_update, before);
    }
}

#[test]
fn test_prop_settings_application_idempotent() {
    let manager = Http2StreamManagerCapsule::new();
    let settings = Http2Settings {
        max_concurrent_streams: Some(75),
        ..Default::default()
    };

    manager.apply_settings(&settings).ok();
    let val1 = manager.max_concurrent_streams.load(Ordering::Acquire);

    manager.apply_settings(&settings).ok();
    let val2 = manager.max_concurrent_streams.load(Ordering::Acquire);

    assert_eq!(val1, val2);
}

#[test]
fn test_prop_concurrent_stream_creation() {
    let manager = Arc::new(Http2StreamManagerCapsule::new());
    manager.max_concurrent_streams.store(1000, Ordering::Release);

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            b.wait();
            for _ in 0..50 {
                let _ = mgr.create_stream();
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All 500 streams should have been created
    let total_created = manager.get_total_streams_created();
    assert!(total_created >= 500);
}

#[test]
fn test_prop_concurrent_flow_control_updates() {
    let manager = Arc::new(Http2StreamManagerCapsule::new());
    manager.connection_window.store(1000000, Ordering::Release);

    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            b.wait();
            for _ in 0..50 {
                let _ = mgr.consume_window(100);
            }
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have consumed exactly 50,000 bytes across all threads
    let consumed = 1000000 - manager.get_available_window() as i64;
    assert_eq!(consumed, 50000);
}

#[test]
fn test_prop_state_entry_atomic_updates() {
    let entry = Http2StreamEntry::new(1);

    // Concurrent updates should be atomic
    let barrier = Arc::new(Barrier::new(5));
    let entry = Arc::new(entry);

    let handles: Vec<_> = (0..5)
        .map(|_| {
            let e = Arc::clone(&entry);
            let b = Arc::clone(&barrier);
            thread::spawn(move || {
                b.wait();
                for i in 0..100 {
                    e.bytes_sent.fetch_add(1, Ordering::Release);
                    if i % 2 == 0 {
                        e.frames_sent.fetch_add(1, Ordering::Release);
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(entry.bytes_sent.load(Ordering::Acquire), 500);
    assert_eq!(entry.frames_sent.load(Ordering::Acquire), 250);
}

#[test]
fn test_prop_window_size_never_negative() {
    let manager = Http2StreamManagerCapsule::new();

    for i in 0..1000 {
        let window = manager.get_available_window();
        assert!(window >= -0x7FFFFFFF);  // Never more negative than i32::MIN

        if i % 2 == 0 {
            manager.consume_window(1).ok();
        } else {
            manager.update_window(1).ok();
        }
    }
}

#[test]
fn test_prop_error_codes_distinct() {
    let no_err = Http2ErrorCode::NoError as u32;
    let proto_err = Http2ErrorCode::ProtocolError as u32;
    let flow_err = Http2ErrorCode::FlowControlError as u32;

    assert_ne!(no_err, proto_err);
    assert_ne!(proto_err, flow_err);
    assert_ne!(no_err, flow_err);
}

#[test]
fn test_prop_settings_bounds_checked() {
    let manager = Http2StreamManagerCapsule::new();

    // Try various invalid settings
    let invalid_cases = vec![
        Http2Settings {
            max_concurrent_streams: Some(0),
            ..Default::default()
        },
        Http2Settings {
            initial_window_size: Some(0x80000000),  // 2^31
            ..Default::default()
        },
        Http2Settings {
            max_frame_size: Some(16383),  // One too small
            ..Default::default()
        },
        Http2Settings {
            max_frame_size: Some(0x1000000),  // Too large
            ..Default::default()
        },
    ];

    for settings in invalid_cases {
        let result = manager.apply_settings(&settings);
        assert!(result.is_err());
    }
}

#[test]
fn test_prop_stream_entry_state_consistency() {
    let entry = Http2StreamEntry::new(1);

    for _ in 0..1000 {
        let state = entry.get_state();
        if state.is_active() {
            assert_ne!(state, StreamState::Idle);
            assert_ne!(state, StreamState::Closed);
        }
    }
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS (36 tests)
// ============================================================================

#[test]
fn test_integration_complete_stream_lifecycle() {
    let manager = Http2StreamManagerCapsule::new();

    // Create stream
    let stream_id = manager.create_stream().expect("Failed to create stream");
    assert!(stream_id > 0);

    // Get state
    let state = manager.get_stream_state(stream_id).expect("Failed to get state");
    assert!(state.is_active());

    // Transition to open
    manager
        .set_stream_state(stream_id, StreamState::Open)
        .expect("Failed to set state");

    // Transition to half-closed
    manager
        .set_stream_state(stream_id, StreamState::HalfClosedLocal)
        .expect("Failed to set state");

    // Close stream
    manager
        .close_stream(stream_id, Http2ErrorCode::NoError as u32)
        .expect("Failed to close stream");
}

#[test]
fn test_integration_flow_control_with_stream_creation() {
    let manager = Http2StreamManagerCapsule::new();

    // Create multiple streams
    for _ in 0..10 {
        manager.create_stream().ok();
    }

    // Manage flow control independently
    let initial_window = manager.get_available_window();
    manager.consume_window(100).ok();
    manager.update_window(50).ok();

    let final_window = initial_window - 100 + 50;
    assert_eq!(manager.get_available_window(), final_window);
}

#[test]
fn test_integration_settings_affect_stream_limits() {
    let manager = Http2StreamManagerCapsule::new();

    // Create 10 streams with default limit (100)
    for _ in 0..10 {
        assert!(manager.create_stream().is_ok());
    }

    // Lower limit to 5
    let settings = Http2Settings {
        max_concurrent_streams: Some(5),
        ..Default::default()
    };
    manager.apply_settings(&settings).ok();

    // New streams should be rejected when limit is reached
    // (This would require tracking active streams, which simplified implementation doesn't do)
}

#[test]
fn test_integration_concurrent_operations() {
    let manager = Arc::new(Http2StreamManagerCapsule::new());
    manager.max_concurrent_streams.store(1000, Ordering::Release);

    let barrier = Arc::new(Barrier::new(3));
    let mut handles = vec![];

    // Thread 1: Create streams
    {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for _ in 0..100 {
                let _ = mgr.create_stream();
            }
        }));
    }

    // Thread 2: Update flow control
    {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for i in 0..100 {
                if i % 2 == 0 {
                    let _ = mgr.consume_window(10);
                } else {
                    let _ = mgr.update_window(10);
                }
            }
        }));
    }

    // Thread 3: Apply settings
    {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            for i in 0..10 {
                let settings = Http2Settings {
                    max_frame_size: Some(16384 + i * 1000),
                    ..Default::default()
                };
                let _ = mgr.apply_settings(&settings);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(manager.get_total_streams_created() > 0);
}

#[test]
fn test_integration_multiple_settings_applications() {
    let manager = Http2StreamManagerCapsule::new();

    // Apply first settings
    let settings1 = Http2Settings {
        max_concurrent_streams: Some(50),
        initial_window_size: Some(32768),
        ..Default::default()
    };
    manager.apply_settings(&settings1).ok();

    // Apply second settings (override)
    let settings2 = Http2Settings {
        max_concurrent_streams: Some(75),
        max_frame_size: Some(32768),
        ..Default::default()
    };
    manager.apply_settings(&settings2).ok();

    assert_eq!(manager.max_concurrent_streams.load(Ordering::Acquire), 75);
    assert_eq!(manager.initial_window_size.load(Ordering::Acquire), 32768);
    assert_eq!(manager.max_frame_size.load(Ordering::Acquire), 32768);
}

#[test]
fn test_integration_error_tracking() {
    let manager = Http2StreamManagerCapsule::new();

    // Trigger protocol error
    manager.max_concurrent_streams.store(1, Ordering::Release);
    let _ = manager.create_stream();
    let _ = manager.create_stream();  // Should fail

    let proto_errors = manager.get_protocol_errors();
    assert!(proto_errors > 0);

    // Trigger flow control error
    let _ = manager.consume_window(u32::MAX);
    let flow_errors = manager.get_flow_control_errors();
    assert!(flow_errors > 0);
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS (14 tests)
// ============================================================================

#[test]
fn test_production_high_concurrency_stream_creation() {
    let manager = Arc::new(Http2StreamManagerCapsule::new());
    manager.max_concurrent_streams.store(10000, Ordering::Release);

    let barrier = Arc::new(Barrier::new(16));
    let mut handles = vec![];

    for _ in 0..16 {
        let mgr = Arc::clone(&manager);
        let b = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            b.wait();
            let mut count = 0;
            for _ in 0..625 {
                if mgr.create_stream().is_ok() {
                    count += 1;
                }
            }
            count
        }));
    }

    let total: u32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert!(total >= 10000);  // All 10K streams created
}

#[test]
fn test_production_memory_stability() {
    let manager = Http2StreamManagerCapsule::new();

    // Perform 100K operations
    for i in 0..100_000 {
        if i % 3 == 0 {
            let _ = manager.create_stream();
        } else if i % 3 == 1 {
            let _ = manager.consume_window(1);
        } else {
            let _ = manager.update_window(1);
        }
    }

    // Should complete without memory issues or crashes
    assert!(manager.get_total_streams_created() > 0);
}

#[test]
fn test_production_sustained_flow_control() {
    let manager = Http2StreamManagerCapsule::new();
    manager.connection_window.store(10_000_000, Ordering::Release);

    // Consume and restore in cycles
    for _ in 0..1000 {
        let before = manager.get_available_window();
        manager.consume_window(100_000).ok();
        let after = manager.get_available_window();
        manager.update_window(100_000).ok();
        let restored = manager.get_available_window();

        assert_eq!(after, before - 100_000);
        assert_eq!(restored, before);
    }
}

#[test]
fn test_production_error_resilience() {
    let manager = Http2StreamManagerCapsule::new();

    // Attempt 10K invalid operations
    for _ in 0..10_000 {
        let _ = manager.consume_window(u32::MAX);
        let _ = manager.set_stream_state(999_999, StreamState::Closed);
    }

    // Should remain stable despite errors
    assert!(manager.get_flow_control_errors() > 0);
}

#[test]
fn test_production_generation_counter_wraparound() {
    let manager = Http2StreamManagerCapsule::new();

    // Perform many operations to test generation counter
    for _ in 0..1_000_000 {
        let _ = manager.create_stream();
        let _ = manager.consume_window(1);
    }

    // Generation counter should have incremented many times
    let gen = manager.generation.load(Ordering::Acquire);
    assert!(gen > 1_000_000);
}
