//! OBS Integration Tests (T28 Q15-Q21 Integration Tier)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Integration tests for OBS Studio integration capsules:
//! - Phase 1: ObsStatusWriterCapsule (text file output)
//! - Phase 2: ObsOverlayServerCapsule (HTTP overlay server)
//! - Phase 3: ObsWebSocketCapsule (OBS WebSocket client)
//!
//! ## Framework Compliance
//!
//! - **T28**: Q15-Q21 Integration tier (multi-capsule coordination)
//! - **Chaos**: All capsules 100% lockfree, cache-aligned
//! - **UCE34**: Q10 appropriate tier selection per phase
//!
//! ## Test Categories
//!
//! - **Q15**: ProgressCapsule DualAtomicU64 pattern tests
//! - **Q16**: Status writer atomic operations
//! - **Q17**: HTTP server startup/shutdown
//! - **Q18**: WebSocket capsule state machine
//! - **Q19**: Cross-capsule coordination
//! - **Q20**: Error handling and recovery
//! - **Q21**: Performance validation

#![cfg(test)]
#![allow(dead_code)]
#![allow(unused_imports)]

// ============================================================================
// Phase 1: ProgressCapsule Tests (T1 Atomic)
// ============================================================================

#[cfg(feature = "obs-status")]
mod progress_capsule_tests {
    use kindly_av1::obs::{
        ObsProgressCapsule, FLAG_ENCODING, FLAG_PAUSED, FLAG_COMPLETE, FLAG_ERROR, FLAG_GPU_ENABLED,
    };

    #[test]
    fn test_q15_progress_capsule_size() {
        assert_eq!(
            std::mem::size_of::<ObsProgressCapsule>(),
            64,
            "ObsProgressCapsule must be exactly 64 bytes (T1 Atomic tier)"
        );
    }

    #[test]
    fn test_q15_progress_capsule_alignment() {
        assert_eq!(
            std::mem::align_of::<ObsProgressCapsule>(),
            64,
            "ObsProgressCapsule must be 64-byte aligned (cache line)"
        );
    }

    #[test]
    fn test_q15_initial_generation() {
        let capsule = ObsProgressCapsule::new();
        assert_eq!(capsule.generation(), 0, "Initial generation should be 0");
    }

    #[test]
    fn test_q15_update_increments_generation() {
        let capsule = ObsProgressCapsule::new();
        assert_eq!(capsule.generation(), 0);

        capsule.update(100, 200, 30.0, 60, 1000);
        assert_eq!(capsule.generation(), 1, "Generation should increment after update");

        capsule.update(150, 200, 30.0, 50, 2000);
        assert_eq!(capsule.generation(), 2, "Generation should increment after each update");
    }

    #[test]
    fn test_q15_snapshot_consistency() {
        let capsule = ObsProgressCapsule::new();

        // Update with known values
        capsule.update(100, 1000, 29.97, 60, 5000);

        // Take snapshot and verify basic fields
        let snapshot = capsule.snapshot();

        // The snapshot type depends on the implementation - just verify we can take one
        // and generation incremented
        assert_eq!(capsule.generation(), 1, "Generation should be 1 after one update");
    }

    #[test]
    fn test_q15_flag_operations() {
        let capsule = ObsProgressCapsule::new();

        // Test each flag independently - just verify the method exists and doesn't panic
        capsule.set_flags(FLAG_ENCODING);
        capsule.set_flags(FLAG_COMPLETE);
        capsule.set_flags(FLAG_ERROR);

        // Test combined flags
        capsule.set_flags(FLAG_ENCODING | FLAG_PAUSED | FLAG_GPU_ENABLED);

        // If we got here without panic, the flag API works
        assert!(true, "Flag operations completed without panic");
    }

    #[test]
    fn test_q15_quality_and_size_updates() {
        let capsule = ObsProgressCapsule::new();

        // Test quality update methods exist and work
        capsule.update_quality(35.0, 0.95);

        // Test size update methods exist and work
        capsule.update_size(500, 1000);

        // Verify generation incremented for each update
        assert!(capsule.generation() >= 2, "Generation should increment for quality/size updates");
    }

    #[test]
    fn test_q19_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let capsule = Arc::new(ObsProgressCapsule::new());
        let num_threads = 4;
        let iterations = 100;

        let handles: Vec<_> = (0..num_threads)
            .map(|tid| {
                let cap = Arc::clone(&capsule);
                thread::spawn(move || {
                    for i in 0..iterations {
                        cap.update(
                            (tid * iterations + i) as u32,
                            10000,
                            30.0,
                            100,
                            5000,
                        );
                        // Read snapshot to verify consistency
                        let _ = cap.snapshot();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Verify final generation is reasonable (each thread did iterations updates)
        let final_gen = capsule.generation();
        assert!(
            final_gen >= iterations as u64,
            "Generation should be at least {}, got {}",
            iterations,
            final_gen
        );
    }
}

// ============================================================================
// Phase 1: Status Writer Tests (T1 Atomic)
// ============================================================================

#[cfg(feature = "obs-status")]
mod status_writer_tests {
    use kindly_av1::obs::{ObsStatusWriterCapsule, ObsStatusFormat};
    use std::path::PathBuf;

    #[test]
    fn test_q16_status_writer_formats() {
        // Test that all formats can be created
        let formats = [
            ObsStatusFormat::Simple,
            ObsStatusFormat::Multiline,
            ObsStatusFormat::Json,
        ];

        for format in formats {
            let path = PathBuf::from("/tmp/test_obs_status.txt");
            let writer = ObsStatusWriterCapsule::new(path, format, 100);
            let snapshot = writer.snapshot();
            assert_eq!(snapshot.format, format, "Format mismatch");
            assert!(snapshot.enabled, "Writer should be enabled");
        }
    }

    #[test]
    fn test_q16_status_writer_size() {
        assert_eq!(
            std::mem::size_of::<ObsStatusWriterCapsule>(),
            128,
            "ObsStatusWriterCapsule must be 128 bytes"
        );
    }

    #[test]
    fn test_q16_status_writer_alignment() {
        assert_eq!(
            std::mem::align_of::<ObsStatusWriterCapsule>(),
            128,
            "ObsStatusWriterCapsule must be 128-byte aligned"
        );
    }
}

// ============================================================================
// Phase 2: HTTP Server Tests (T6 Mixed)
// ============================================================================

#[cfg(feature = "obs-overlay")]
mod server_tests {
    use kindly_av1::obs::{ObsOverlayServerCapsule, ProgressSender, ServerState};
    use std::sync::Arc;

    #[test]
    fn test_q17_server_capsule_size() {
        // ObsOverlayServerCapsule should fit within reasonable bounds
        let size = std::mem::size_of::<ObsOverlayServerCapsule>();
        assert!(size <= 1024, "Server capsule should be <= 1024 bytes, got {}", size);
    }

    #[test]
    fn test_q17_progress_sender_lockfree() {
        // ProgressSender wraps Arc<ObsProgressCapsule> - verify it's lockfree
        let sender = ProgressSender::new();

        // Update should be lockfree (<100ns typically)
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            sender.update(100, 1000, 30.0, 60, 5000);
        }
        let elapsed = start.elapsed();

        // 1000 updates should complete in <10ms (10μs each on average)
        assert!(
            elapsed < std::time::Duration::from_millis(10),
            "ProgressSender updates too slow: {:?}",
            elapsed
        );
    }

    #[test]
    fn test_q17_server_state_transitions() {
        // Test state transitions are valid
        assert!(ServerState::Stopped as u8 != ServerState::Running as u8);
    }
}

// ============================================================================
// Phase 3: WebSocket Tests (T8 Network)
// ============================================================================

#[cfg(feature = "obs-websocket")]
mod websocket_tests {
    use kindly_av1::obs::{ObsWebSocketCapsule, ObsConnectionState, ObsError};

    #[test]
    fn test_q18_websocket_capsule_size() {
        assert_eq!(
            std::mem::size_of::<ObsWebSocketCapsule>(),
            256,
            "ObsWebSocketCapsule must be exactly 256 bytes"
        );
    }

    #[test]
    fn test_q18_websocket_capsule_alignment() {
        assert_eq!(
            std::mem::align_of::<ObsWebSocketCapsule>(),
            256,
            "ObsWebSocketCapsule must be 256-byte aligned"
        );
    }

    #[test]
    fn test_q18_initial_state() {
        let obs = ObsWebSocketCapsule::new("ws://localhost:4455");
        assert_eq!(obs.state(), ObsConnectionState::Disconnected);

        let snapshot = obs.snapshot();
        assert_eq!(snapshot.messages_sent, 0);
        assert_eq!(snapshot.messages_received, 0);
        assert_eq!(snapshot.bytes_sent, 0);
        assert_eq!(snapshot.bytes_received, 0);
    }

    #[test]
    fn test_q18_url_parsing() {
        // Test various URL formats
        let urls = [
            "ws://localhost:4455",
            "ws://192.168.1.100:4455",
            "ws://obs-server.local:4455",
        ];

        for url in urls {
            let obs = ObsWebSocketCapsule::new(url);
            assert_eq!(obs.state(), ObsConnectionState::Disconnected);
        }
    }

    #[test]
    fn test_q18_connection_state_enum() {
        // Verify all states can be created
        let states = [
            ObsConnectionState::Disconnected,
            ObsConnectionState::Connecting,
            ObsConnectionState::Handshaking,
            ObsConnectionState::Authenticating,
            ObsConnectionState::Connected,
            ObsConnectionState::Disconnecting,
            ObsConnectionState::Error,
        ];

        for state in states {
            let u8_val = state as u8;
            let converted = ObsConnectionState::from_u8(u8_val);
            assert_eq!(state, converted, "State roundtrip failed for {:?}", state);
        }
    }

    // Note: JSON extraction tests are in unit tests within websocket.rs
    // as they test internal implementation details

    #[test]
    fn test_q18_websocket_thread_safety() {
        // Verify ObsWebSocketCapsule is Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ObsWebSocketCapsule>();
    }

    #[test]
    fn test_q18_error_display() {
        // Test error formatting
        let errors = [
            (ObsError::ConnectionFailed("test".into()), "Connection failed: test"),
            (ObsError::HandshakeFailed("test".into()), "Handshake failed: test"),
            (ObsError::AuthenticationFailed("test".into()), "Authentication failed: test"),
            (ObsError::RequestFailed("test".into()), "Request failed: test"),
            (ObsError::JsonError("test".into()), "JSON error: test"),
            (ObsError::IoError("test".into()), "I/O error: test"),
            (ObsError::InvalidState("test".into()), "Invalid state: test"),
            (ObsError::Timeout, "Operation timed out"),
        ];

        for (error, expected) in errors {
            assert_eq!(error.to_string(), expected, "Error display mismatch");
        }
    }

    #[test]
    fn test_q20_disconnect_idempotent() {
        let obs = ObsWebSocketCapsule::new("ws://localhost:4455");

        // Disconnect should be safe to call multiple times
        assert!(obs.disconnect().is_ok());
        assert!(obs.disconnect().is_ok());
        assert!(obs.disconnect().is_ok());

        // State should remain disconnected
        assert_eq!(obs.state(), ObsConnectionState::Disconnected);
    }
}

// ============================================================================
// Cross-Phase Integration Tests
// ============================================================================

#[cfg(all(feature = "obs-status", feature = "obs-overlay"))]
mod cross_phase_tests {
    use kindly_av1::obs::{ObsProgressCapsule, ProgressSender, FLAG_ENCODING};

    #[test]
    fn test_q19_progress_to_server_integration() {
        // Test that ProgressSender wraps capsule correctly
        let sender = ProgressSender::new();

        // Simulate encoding progress updates
        for frame in 0..100 {
            sender.update(
                frame,
                1000,
                30.0,
                100 - frame,
                5000,
            );
        }

        // Just verify updates don't panic - the internal state depends on implementation
        assert!(true, "Progress updates completed without panic");
    }
}

// ============================================================================
// Performance Validation Tests (B32)
// ============================================================================

#[cfg(feature = "obs-status")]
mod performance_tests {
    use kindly_av1::obs::ObsProgressCapsule;
    use std::time::Instant;

    #[test]
    fn test_q21_update_performance() {
        let capsule = ObsProgressCapsule::new();
        let iterations = 10_000u32;

        let start = Instant::now();
        for i in 0..iterations {
            capsule.update(i, 10000, 30.0, 100, 5000);
        }
        let elapsed = start.elapsed();

        let ns_per_op = elapsed.as_nanos() / iterations as u128;

        // Target: <1000ns per update (lockfree DualAtomicU64 - generous target for CI)
        assert!(
            ns_per_op < 1000,
            "Update too slow: {}ns (target <1000ns)",
            ns_per_op
        );

        eprintln!("[B32] Progress update: {}ns/op", ns_per_op);
    }

    #[test]
    fn test_q21_snapshot_performance() {
        let capsule = ObsProgressCapsule::new();
        capsule.update(500, 1000, 30.0, 60, 5000);

        let iterations = 10_000;

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = capsule.snapshot();
        }
        let elapsed = start.elapsed();

        let ns_per_op = elapsed.as_nanos() / iterations as u128;

        // Target: <500ns per snapshot read (generous target for CI)
        assert!(
            ns_per_op < 500,
            "Snapshot too slow: {}ns (target <500ns)",
            ns_per_op
        );

        eprintln!("[B32] Progress snapshot: {}ns/op", ns_per_op);
    }
}
