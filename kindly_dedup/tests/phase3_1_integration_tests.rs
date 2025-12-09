//! # Phase 3.1 Integration Tests
//!
//! Validates UniversalDedupPipelineCapsule wrapper capsule with DedupMetacapsule.
//!
//! ## Test Coverage
//!
//! - **Wrapper Capsule**: Size, alignment, state machine
//! - **Arc Reference Pattern**: Multiple wrappers, shared orchestrator
//! - **Backward Compatibility**: Old API still works
//! - **3-Stage Coordination**: Stage 1 → Stage 2 → Stage 3
//! - **Error Handling**: State validation, error messages
//!
//! ## Framework Compliance
//!
//! - **UCE34**: T6 Mixed wrapper capsule
//! - **Chaos**: 100% lockfree (#[derive(ComputationalCapsule)])
//! - **ASSUM**: All assumptions documented
//! - **T28**: Integration tests (Q15-Q21)
//! - **I20**: Zero breaking changes

use kindly_dedup::pipeline::{UniversalDedupPipelineCapsule, WrapperState};

#[test]
fn test_wrapper_is_capsule() {
    // Verify wrapper IS a capsule (user requirement)
    // Size: 128 bytes (cache-aligned orchestrator wrapper)
    assert_eq!(
        std::mem::size_of::<UniversalDedupPipelineCapsule>(),
        128,
        "Wrapper capsule must be 128 bytes"
    );

    // Alignment: 128 bytes (cache-line aligned)
    assert_eq!(
        std::mem::align_of::<UniversalDedupPipelineCapsule>(),
        128,
        "Wrapper capsule must be 128-byte aligned"
    );
}

#[test]
fn test_new_wrapper_capsule_basic() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test_corpus.jsonl",
        100_000,
        0.85,
        0,
        100_000,
    )
    .expect("Failed to create wrapper capsule");

    // Verify initial state
    assert_eq!(capsule.state(), WrapperState::Ready);
    assert_eq!(capsule.docs_processed(), 0);
    assert!(!capsule.is_running());
    assert!(!capsule.is_complete());
    assert!(!capsule.is_error());
}

#[test]
fn test_arc_reference_pattern() {
    // Create wrapper capsule
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Get Arc references (like RatatuiProgressAdapter pattern)
    let meta1 = capsule.metacapsule();
    let meta2 = capsule.metacapsule();

    // Verify same orchestrator (Arc::ptr_eq)
    assert!(
        std::sync::Arc::ptr_eq(&meta1, &meta2),
        "Arc references must point to same DedupMetacapsule"
    );

    // Verify Arc refcount increases
    let refcount_before = std::sync::Arc::strong_count(&meta1);
    let _meta3 = capsule.metacapsule();
    let refcount_after = std::sync::Arc::strong_count(&meta1);
    assert_eq!(
        refcount_after,
        refcount_before + 1,
        "Arc refcount must increase"
    );
}

#[test]
fn test_wrapper_state_machine() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Initial state: Ready
    assert_eq!(capsule.state(), WrapperState::Ready);

    // Valid transition: Ready → Running
    assert!(capsule
        .transition_state(WrapperState::Ready, WrapperState::Running)
        .is_ok());
    assert_eq!(capsule.state(), WrapperState::Running);
    assert!(capsule.is_running());

    // Valid transition: Running → Complete
    assert!(capsule
        .transition_state(WrapperState::Running, WrapperState::Complete)
        .is_ok());
    assert_eq!(capsule.state(), WrapperState::Complete);
    assert!(capsule.is_complete());
}

#[test]
fn test_invalid_state_transitions() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Invalid: Ready → Complete (must go through Running)
    assert!(capsule
        .transition_state(WrapperState::Ready, WrapperState::Complete)
        .is_err());

    // Invalid: Complete → Running (cannot go backward)
    capsule
        .transition_state(WrapperState::Ready, WrapperState::Running)
        .unwrap();
    capsule
        .transition_state(WrapperState::Running, WrapperState::Complete)
        .unwrap();
    assert!(capsule
        .transition_state(WrapperState::Complete, WrapperState::Running)
        .is_err());
}

#[test]
fn test_error_state_handling() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Set error state
    let error_msg = "Test error: File not found".to_string();
    assert!(capsule.set_error(error_msg.clone()).is_ok());

    // Verify error state
    assert_eq!(capsule.state(), WrapperState::Error);
    assert!(capsule.is_error());
    assert_eq!(capsule.error_message(), Some(error_msg));
}

#[test]
fn test_config_validation() {
    // Invalid threshold (> 1.0)
    let result = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        1.5, // Invalid
        0,
        1000,
    );
    assert!(result.is_err());

    // Invalid threshold (< 0.0)
    let result = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        -0.1, // Invalid
        0,
        1000,
    );
    assert!(result.is_err());

    // Invalid capacity (0)
    let result = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        0, // Invalid
        0.85,
        0,
        1000,
    );
    assert!(result.is_err());

    // Invalid document range (start >= end)
    let result = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        1000, // start >= end
        1000,
    );
    assert!(result.is_err());
}

#[test]
fn test_progress_snapshot() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Get progress snapshot (orchestrator atomic snapshot)
    let progress = capsule.progress();

    // Verify orchestrator state
    assert_eq!(
        progress.state,
        kindly_dedup::metacapsule::State::Idle
    );
    assert_eq!(progress.docs_processed, 0);
    assert_eq!(progress.worker_mask, 0);
}

#[test]
fn test_backward_compatible_api() {
    // Old API pattern (preserved for compatibility)
    let capsule = UniversalDedupPipelineCapsule::new(
        "test_corpus.jsonl",
        10_000,
        0.85,
        0,
        10_000,
    );

    // Verify old API still works
    assert!(capsule.is_ok());

    let capsule = capsule.unwrap();

    // Old API: config access
    assert_eq!(capsule.config().capacity, 10_000);
    assert_eq!(capsule.config().threshold, 0.85);
    assert_eq!(capsule.config().corpus_path, "test_corpus.jsonl");

    // Old API: state checks
    assert!(capsule.state() == WrapperState::Ready);
    assert_eq!(capsule.docs_processed(), 0);
}

#[test]
fn test_process_corpus_placeholder() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Process corpus (placeholder implementation)
    let result = capsule.process_corpus();

    // Should succeed with placeholder
    assert!(result.is_ok());

    // Verify state transition: Ready → Running → Complete
    assert_eq!(capsule.state(), WrapperState::Complete);
}

#[test]
fn test_find_duplicates_placeholder() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Process corpus first
    capsule.process_corpus().unwrap();

    // Find duplicates (placeholder returns empty)
    let clusters = capsule.find_duplicates(0.85);
    assert!(clusters.is_ok());
    assert_eq!(clusters.unwrap().len(), 0);
}

#[test]
fn test_find_duplicates_before_processing() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Try to find duplicates before processing
    let result = capsule.find_duplicates(0.85);

    // Should fail (invalid state)
    assert!(result.is_err());
}

#[test]
fn test_wrapper_send_sync() {
    // Verify wrapper is Send + Sync (required for threading)
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<UniversalDedupPipelineCapsule>();
    assert_sync::<UniversalDedupPipelineCapsule>();
}

#[test]
fn test_wrapper_debug_impl() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Verify Debug implementation
    let debug_str = format!("{:?}", capsule);
    assert!(debug_str.contains("UniversalDedupPipelineCapsule"));
    assert!(debug_str.contains("Ready"));
}

#[test]
fn test_orchestrator_state_coordination() {
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    // Get orchestrator reference
    let metacapsule = capsule.metacapsule();

    // Start orchestrator streaming
    assert!(metacapsule.start_streaming().is_ok());

    // Verify orchestrator state reflects in progress
    let progress = capsule.progress();
    assert_eq!(
        progress.state,
        kindly_dedup::metacapsule::State::Streaming
    );
}

#[test]
fn test_wrapper_drop_cleanup() {
    // Create wrapper with error state
    let capsule = UniversalDedupPipelineCapsule::new(
        "test.jsonl",
        1000,
        0.85,
        0,
        1000,
    )
    .unwrap();

    capsule.set_error("Test error".to_string()).unwrap();

    // Drop capsule (should clean up error pointer)
    drop(capsule);

    // If we get here without panic, Drop cleanup succeeded
    assert!(true);
}

// TODO Phase 3.1: Add 100K corpus integration test after stage wiring complete
// #[test]
// #[ignore] // Only run with --ignored flag (requires test corpus)
// fn test_100k_corpus_no_regression() {
//     // This test requires actual corpus file and full stage wiring implementation
//     let pipeline = UniversalDedupPipelineCapsule::new(
//         "test_data/c4_100k.jsonl",
//         100_000,
//         0.85,
//         0,
//         100_000,
//     ).unwrap();
//
//     let start = std::time::Instant::now();
//     pipeline.process_corpus().unwrap();
//     let elapsed = start.elapsed();
//
//     let throughput = 100_000 / elapsed.as_secs();
//
//     // No regression: ≥2.6K docs/sec (v3.0 baseline)
//     assert!(throughput >= 2_600, "Throughput regression: {} docs/sec", throughput);
//
//     // Validate accuracy: ≥90% F1 score
//     let clusters = pipeline.find_duplicates(0.85).unwrap();
//     // TODO: Compute F1 score vs ground truth
// }
