//! CrossQueueSyncCapsule Tests (T28 5-tier coverage)
//!
//! Tests organized by T28 framework tiers:
//! - **Q1-Q7 (Unit)**: Basic operations, edge cases, error handling
//! - **Q8-Q14 (Property)**: Invariants, state machine properties
//! - **Q15-Q21 (Integration)**: Multi-queue pipelines, ownership transfer
//! - **Q22-Q28 (Production)**: Stress tests, concurrent access
//! - **Q29-Q35 (Determinism)**: Reproducibility, timing analysis

use atomic_capsule::gpu::kgpu_driver::{
    CrossQueueSyncCapsule, CrossQueueType as QueueType,
};

// ============================================================================
// Q1-Q7: Unit Tests (Basic Operations)
// ============================================================================

#[test]
fn test_unit_new_capsule() {
    let sync = CrossQueueSyncCapsule::new();

    // All queues should start at timeline 0
    for queue in QueueType::ALL_QUEUES {
        assert_eq!(sync.get_queue_timeline(*queue), 0, "Queue {:?} should start at timeline 0", queue);
    }

    // All queues should be ready (no dependencies)
    for queue in QueueType::ALL_QUEUES {
        assert!(sync.is_ready(*queue), "Queue {:?} should be ready initially", queue);
    }
}

#[test]
fn test_unit_signal_timeline_basic() {
    let sync = CrossQueueSyncCapsule::new();

    // Signal Graphics queue to timeline 42
    sync.signal_queue(QueueType::Graphics, 42).unwrap();
    assert_eq!(sync.get_queue_timeline(QueueType::Graphics), 42);

    // Other queues should still be at 0
    assert_eq!(sync.get_queue_timeline(QueueType::Compute), 0);
    assert_eq!(sync.get_queue_timeline(QueueType::Transfer), 0);
}

#[test]
fn test_unit_signal_timeline_monotonic() {
    let sync = CrossQueueSyncCapsule::new();

    // Signal timeline 100
    sync.signal_queue(QueueType::Graphics, 100).unwrap();

    // Try to signal lower value (should fail)
    let result = sync.signal_queue(QueueType::Graphics, 50);
    assert!(result.is_err(), "Timeline should be strictly increasing");

    // Try to signal same value (should fail)
    let result = sync.signal_queue(QueueType::Graphics, 100);
    assert!(result.is_err(), "Timeline should be strictly increasing");

    // Signal higher value (should succeed)
    sync.signal_queue(QueueType::Graphics, 150).unwrap();
    assert_eq!(sync.get_queue_timeline(QueueType::Graphics), 150);
}

#[test]
fn test_unit_add_dependency_basic() {
    let sync = CrossQueueSyncCapsule::new();

    // Add dependency: Compute waits on Transfer
    sync.add_dependency(QueueType::Compute, QueueType::Transfer, 100).unwrap();

    // Compute should NOT be ready (Transfer timeline is 0 < 100)
    assert!(!sync.is_ready(QueueType::Compute));

    // Signal Transfer to 100
    sync.signal_queue(QueueType::Transfer, 100).unwrap();

    // Now Compute should be ready
    assert!(sync.is_ready(QueueType::Compute));
}

#[test]
fn test_unit_wait_for_queue() {
    let sync = CrossQueueSyncCapsule::new();

    // Signal Transfer to 150
    sync.signal_queue(QueueType::Transfer, 150).unwrap();

    // Check various wait values
    assert!(sync.wait_for_queue(QueueType::Transfer, 100));
    assert!(sync.wait_for_queue(QueueType::Transfer, 150));
    assert!(!sync.wait_for_queue(QueueType::Transfer, 200));
}

#[test]
fn test_unit_transfer_ownership_basic() {
    let sync = CrossQueueSyncCapsule::new();
    let resource_id = 0xDEADBEEFu64;

    // Transfer ownership from Transfer to Graphics
    sync.transfer_ownership(resource_id, QueueType::Transfer, QueueType::Graphics).unwrap();

    // Check ownership via snapshot
    let snapshot = sync.snapshot();
    assert_eq!(snapshot.queue_states[QueueType::Transfer.to_index()].owner_resource, 0);
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, resource_id);
}

#[test]
fn test_unit_transfer_ownership_invalid_resource() {
    let sync = CrossQueueSyncCapsule::new();

    // Try to transfer resource_id=0 (should fail)
    let result = sync.transfer_ownership(0, QueueType::Transfer, QueueType::Graphics);
    assert!(result.is_err(), "Resource ID 0 should be invalid");
}

#[test]
fn test_unit_clear_dependencies() {
    let sync = CrossQueueSyncCapsule::new();

    // Add multiple dependencies
    sync.add_dependency(QueueType::Graphics, QueueType::Transfer, 100).unwrap();
    sync.add_dependency(QueueType::Graphics, QueueType::Compute, 100).unwrap();

    // Clear dependencies
    sync.clear_dependencies(QueueType::Graphics).unwrap();

    // Graphics should now be ready (no dependencies)
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_unit_snapshot() {
    let sync = CrossQueueSyncCapsule::new();

    // Set up some state
    sync.signal_queue(QueueType::Graphics, 42).unwrap();
    sync.signal_queue(QueueType::Compute, 100).unwrap();
    sync.add_dependency(QueueType::Compute, QueueType::Transfer, 50).unwrap();

    // Get snapshot
    let snapshot = sync.snapshot();

    // Verify snapshot data
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].timeline_value, 42);
    assert_eq!(snapshot.queue_states[QueueType::Compute.to_index()].timeline_value, 100);
    assert_eq!(snapshot.queue_states[QueueType::Compute.to_index()].pending_value, 50);
}

// ============================================================================
// Q8-Q14: Property Tests (Invariants)
// ============================================================================

#[test]
fn test_property_timeline_monotonic_invariant() {
    let sync = CrossQueueSyncCapsule::new();

    // Signal timeline in increasing order
    for i in 1..=100 {
        sync.signal_queue(QueueType::Graphics, i).unwrap();
        assert_eq!(sync.get_queue_timeline(QueueType::Graphics), i);
    }
}

#[test]
fn test_property_dependency_transitivity() {
    let sync = CrossQueueSyncCapsule::new();

    // Setup: Graphics depends on Compute, Compute depends on Transfer
    sync.add_dependency(QueueType::Graphics, QueueType::Compute, 100).unwrap();
    sync.add_dependency(QueueType::Compute, QueueType::Transfer, 100).unwrap();

    // Initially, neither Graphics nor Compute should be ready
    assert!(!sync.is_ready(QueueType::Graphics));
    assert!(!sync.is_ready(QueueType::Compute));

    // Signal Transfer to 100
    sync.signal_queue(QueueType::Transfer, 100).unwrap();

    // Now Compute should be ready, but Graphics still waiting on Compute
    assert!(sync.is_ready(QueueType::Compute));
    assert!(!sync.is_ready(QueueType::Graphics));

    // Signal Compute to 100
    sync.signal_queue(QueueType::Compute, 100).unwrap();

    // Now Graphics should also be ready
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_property_multiple_dependencies_all_must_satisfy() {
    let sync = CrossQueueSyncCapsule::new();

    // Graphics depends on both Transfer and Compute
    sync.add_dependency(QueueType::Graphics, QueueType::Transfer, 100).unwrap();
    sync.add_dependency(QueueType::Graphics, QueueType::Compute, 100).unwrap();

    // Neither satisfied
    assert!(!sync.is_ready(QueueType::Graphics));

    // Only Transfer satisfied
    sync.signal_queue(QueueType::Transfer, 100).unwrap();
    assert!(!sync.is_ready(QueueType::Graphics));

    // Only Compute satisfied (reset Transfer)
    let sync2 = CrossQueueSyncCapsule::new();
    sync2.add_dependency(QueueType::Graphics, QueueType::Transfer, 100).unwrap();
    sync2.add_dependency(QueueType::Graphics, QueueType::Compute, 100).unwrap();
    sync2.signal_queue(QueueType::Compute, 100).unwrap();
    assert!(!sync2.is_ready(QueueType::Graphics));

    // Both satisfied
    sync.signal_queue(QueueType::Compute, 100).unwrap();
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_property_ownership_exclusivity() {
    let sync = CrossQueueSyncCapsule::new();
    let resource_id = 0xCAFEBABEu64;

    // Transfer ownership to Graphics
    sync.transfer_ownership(resource_id, QueueType::Transfer, QueueType::Graphics).unwrap();

    // Transfer should no longer own the resource
    let snapshot = sync.snapshot();
    assert_eq!(snapshot.queue_states[QueueType::Transfer.to_index()].owner_resource, 0);
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, resource_id);

    // Transfer ownership again to Compute
    sync.transfer_ownership(resource_id, QueueType::Graphics, QueueType::Compute).unwrap();

    // Graphics should no longer own the resource
    let snapshot = sync.snapshot();
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, 0);
    assert_eq!(snapshot.queue_states[QueueType::Compute.to_index()].owner_resource, resource_id);
}

#[test]
fn test_property_generation_counter_increments() {
    let sync = CrossQueueSyncCapsule::new();

    let initial_gen = sync.snapshot().queue_states[QueueType::Graphics.to_index()].generation;

    // Signal should increment generation
    sync.signal_queue(QueueType::Graphics, 10).unwrap();
    let gen1 = sync.snapshot().queue_states[QueueType::Graphics.to_index()].generation;
    assert!(gen1 > initial_gen, "Generation should increment on signal");

    // Add dependency should increment generation
    sync.add_dependency(QueueType::Graphics, QueueType::Transfer, 10).unwrap();
    let gen2 = sync.snapshot().queue_states[QueueType::Graphics.to_index()].generation;
    assert!(gen2 > gen1, "Generation should increment on add_dependency");

    // Transfer ownership should increment generation
    sync.transfer_ownership(0x1234, QueueType::Graphics, QueueType::Compute).unwrap();
    let gen3 = sync.snapshot().queue_states[QueueType::Graphics.to_index()].generation;
    assert!(gen3 > gen2, "Generation should increment on transfer_ownership");
}

// ============================================================================
// Q15-Q21: Integration Tests (Multi-Queue Pipelines)
// ============================================================================

#[test]
fn test_integration_graphics_pipeline() {
    // Typical graphics pipeline: Transfer → Graphics → Present
    let sync = CrossQueueSyncCapsule::new();

    // Stage 1: Transfer uploads texture data
    sync.signal_queue(QueueType::Transfer, 1).unwrap();

    // Stage 2: Graphics waits for Transfer, then renders
    sync.add_dependency(QueueType::Graphics, QueueType::Transfer, 1).unwrap();
    assert!(sync.is_ready(QueueType::Graphics), "Graphics should be ready after Transfer");

    sync.signal_queue(QueueType::Graphics, 2).unwrap();

    // Stage 3: Present waits for Graphics (using Sparse queue as proxy for Present)
    sync.add_dependency(QueueType::Sparse, QueueType::Graphics, 2).unwrap();
    assert!(sync.is_ready(QueueType::Sparse), "Present should be ready after Graphics");
}

#[test]
fn test_integration_async_compute_pipeline() {
    // Async compute pipeline: Graphics + Compute run in parallel, then join
    let sync = CrossQueueSyncCapsule::new();

    // Stage 1: Graphics and Compute start independently
    sync.signal_queue(QueueType::Graphics, 10).unwrap();
    sync.signal_queue(QueueType::Compute, 5).unwrap();

    // Stage 2: Transfer waits for both Graphics and Compute
    sync.add_dependency(QueueType::Transfer, QueueType::Graphics, 10).unwrap();
    sync.add_dependency(QueueType::Transfer, QueueType::Compute, 5).unwrap();

    assert!(sync.is_ready(QueueType::Transfer), "Transfer should be ready after both Graphics and Compute");
}

#[test]
fn test_integration_video_encode_pipeline() {
    // Video encode pipeline: Transfer → VideoDec → VideoEnc → Transfer
    let sync = CrossQueueSyncCapsule::new();

    // Stage 1: Transfer uploads video frame
    sync.signal_queue(QueueType::Transfer, 1).unwrap();

    // Stage 2: VideoDec waits for Transfer
    sync.add_dependency(QueueType::VideoDec, QueueType::Transfer, 1).unwrap();
    assert!(sync.is_ready(QueueType::VideoDec));
    sync.signal_queue(QueueType::VideoDec, 2).unwrap();

    // Stage 3: VideoEnc waits for VideoDec
    sync.add_dependency(QueueType::VideoEnc, QueueType::VideoDec, 2).unwrap();
    assert!(sync.is_ready(QueueType::VideoEnc));
    sync.signal_queue(QueueType::VideoEnc, 3).unwrap();

    // Stage 4: Transfer downloads encoded frame (waits for VideoEnc)
    // Use a second Transfer submission by advancing timeline
    sync.add_dependency(QueueType::Transfer, QueueType::VideoEnc, 3).unwrap();
    assert!(sync.is_ready(QueueType::Transfer));
}

#[test]
fn test_integration_ownership_transfer_pipeline() {
    let sync = CrossQueueSyncCapsule::new();
    let buffer_id = 0x1234u64;

    // Stage 1: Transfer queue uploads data to buffer
    sync.signal_queue(QueueType::Transfer, 10).unwrap();

    // Stage 2: Transfer ownership to Compute queue
    sync.transfer_ownership(buffer_id, QueueType::Transfer, QueueType::Compute).unwrap();

    // Compute waits for Transfer timeline
    sync.add_dependency(QueueType::Compute, QueueType::Transfer, 10).unwrap();
    assert!(sync.is_ready(QueueType::Compute));

    // Stage 3: Compute processes buffer
    sync.signal_queue(QueueType::Compute, 20).unwrap();

    // Stage 4: Transfer ownership to Graphics queue
    sync.transfer_ownership(buffer_id, QueueType::Compute, QueueType::Graphics).unwrap();

    // Graphics waits for Compute timeline
    sync.add_dependency(QueueType::Graphics, QueueType::Compute, 20).unwrap();
    assert!(sync.is_ready(QueueType::Graphics));

    // Verify final ownership
    let snapshot = sync.snapshot();
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, buffer_id);
}

#[test]
fn test_integration_multi_resource_ownership() {
    let sync = CrossQueueSyncCapsule::new();
    let buffer_a = 0xAAAAu64;
    let buffer_b = 0xBBBBu64;

    // Step 1: Transfer queue gets ownership of buffer_a
    sync.transfer_ownership(buffer_a, QueueType::Compute, QueueType::Transfer).unwrap();

    // Step 2: Graphics queue gets ownership of buffer_a from Transfer
    sync.transfer_ownership(buffer_a, QueueType::Transfer, QueueType::Graphics).unwrap();

    // Step 3: Compute queue gets ownership of buffer_b
    sync.transfer_ownership(buffer_b, QueueType::Transfer, QueueType::Compute).unwrap();

    // Step 4: Transfer queue gets ownership of buffer_b from Compute
    sync.transfer_ownership(buffer_b, QueueType::Compute, QueueType::Transfer).unwrap();

    // Verify final ownership
    let snapshot = sync.snapshot();
    // Graphics owns buffer_a
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, buffer_a);
    // Transfer owns buffer_b
    assert_eq!(snapshot.queue_states[QueueType::Transfer.to_index()].owner_resource, buffer_b);
    // Compute owns nothing (released buffer_b)
    assert_eq!(snapshot.queue_states[QueueType::Compute.to_index()].owner_resource, 0);
}

// ============================================================================
// Q22-Q28: Production Tests (Stress & Edge Cases)
// ============================================================================

#[test]
fn test_production_high_timeline_values() {
    let sync = CrossQueueSyncCapsule::new();

    // Test with very high timeline values (near u64 max)
    let high_value = u64::MAX / 2; // Half of u64::MAX

    sync.signal_queue(QueueType::Graphics, high_value).unwrap();
    assert_eq!(sync.get_queue_timeline(QueueType::Graphics), high_value);

    // Advance further
    sync.signal_queue(QueueType::Graphics, high_value + 1000).unwrap();
    assert_eq!(sync.get_queue_timeline(QueueType::Graphics), high_value + 1000);
}

#[test]
fn test_production_all_queues_dependency_chain() {
    let sync = CrossQueueSyncCapsule::new();

    // Create dependency chain: Q0 → Q1 → Q2 → Q3 → Q4 → Q5 → Q6 → Q7
    for i in 0..7 {
        let current = QueueType::from_index(i).unwrap();
        let next = QueueType::from_index(i + 1).unwrap();

        sync.signal_queue(current, (i + 1) as u64 * 10).unwrap();
        sync.add_dependency(next, current, (i + 1) as u64 * 10).unwrap();
    }

    // All queues except Graphics (Q0) should be ready
    for i in 1..8 {
        let queue = QueueType::from_index(i).unwrap();
        assert!(sync.is_ready(queue), "Queue {:?} should be ready", queue);
    }
}

#[test]
fn test_production_rapid_timeline_advancement() {
    let sync = CrossQueueSyncCapsule::new();

    // Rapidly advance timeline 1000 times
    for i in 1..=1000 {
        sync.signal_queue(QueueType::Graphics, i).unwrap();
    }

    assert_eq!(sync.get_queue_timeline(QueueType::Graphics), 1000);
}

#[test]
fn test_production_many_dependencies() {
    let sync = CrossQueueSyncCapsule::new();

    // Graphics depends on all other 7 queues
    for queue in &QueueType::ALL_QUEUES[1..] {
        sync.add_dependency(QueueType::Graphics, *queue, 100).unwrap();
    }

    // Graphics should NOT be ready
    assert!(!sync.is_ready(QueueType::Graphics));

    // Signal all other queues to 100
    for queue in &QueueType::ALL_QUEUES[1..] {
        sync.signal_queue(*queue, 100).unwrap();
    }

    // Now Graphics should be ready
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_production_repeated_ownership_transfers() {
    let sync = CrossQueueSyncCapsule::new();
    let resource_id = 0xDEADu64;

    // Repeatedly transfer ownership in a cycle: Graphics → Compute → Transfer → Graphics
    for _ in 0..100 {
        sync.transfer_ownership(resource_id, QueueType::Graphics, QueueType::Compute).unwrap();
        sync.transfer_ownership(resource_id, QueueType::Compute, QueueType::Transfer).unwrap();
        sync.transfer_ownership(resource_id, QueueType::Transfer, QueueType::Graphics).unwrap();
    }

    // Verify final state
    let snapshot = sync.snapshot();
    assert_eq!(snapshot.queue_states[QueueType::Graphics.to_index()].owner_resource, resource_id);
}

// ============================================================================
// Q29-Q35: Determinism Tests (Reproducibility)
// ============================================================================

#[test]
fn test_determinism_snapshot_reproducibility() {
    let sync1 = CrossQueueSyncCapsule::new();
    let sync2 = CrossQueueSyncCapsule::new();

    // Apply same operations to both capsules
    for sync in [&sync1, &sync2] {
        sync.signal_queue(QueueType::Graphics, 42).unwrap();
        sync.signal_queue(QueueType::Compute, 100).unwrap();
        sync.add_dependency(QueueType::Graphics, QueueType::Transfer, 50).unwrap();
        sync.transfer_ownership(0x1234, QueueType::Transfer, QueueType::Graphics).unwrap();
    }

    // Snapshots should be identical (except generation counters may differ due to timing)
    let snap1 = sync1.snapshot();
    let snap2 = sync2.snapshot();

    for i in 0..8 {
        assert_eq!(snap1.queue_states[i].timeline_value, snap2.queue_states[i].timeline_value);
        assert_eq!(snap1.queue_states[i].dependency_mask, snap2.queue_states[i].dependency_mask);
        assert_eq!(snap1.queue_states[i].pending_value, snap2.queue_states[i].pending_value);
        assert_eq!(snap1.queue_states[i].owner_resource, snap2.queue_states[i].owner_resource);
    }
}

#[test]
fn test_determinism_queue_type_conversions() {
    // Queue type conversions should be deterministic
    for queue in QueueType::ALL_QUEUES {
        let idx = queue.to_index();
        let recovered = QueueType::from_index(idx).unwrap();
        assert_eq!(*queue, recovered, "Queue type conversion should be deterministic");

        let bit = queue.to_bit();
        assert_eq!(bit, 1u64 << idx, "Queue bit should match index");
    }
}

#[test]
fn test_determinism_capsule_size_and_alignment() {
    // Capsule should always be 1024B, cache-aligned
    assert_eq!(std::mem::size_of::<CrossQueueSyncCapsule>(), 1024);
    assert_eq!(std::mem::align_of::<CrossQueueSyncCapsule>(), 512);
}

// ============================================================================
// Chaos Compliance Tests
// ============================================================================

#[test]
fn test_chaos_alignment() {
    // Verify 512B alignment (8 cache lines × 64B)
    assert_eq!(std::mem::align_of::<CrossQueueSyncCapsule>(), 512);
}

#[test]
fn test_chaos_fixed_size() {
    // Verify exactly 1024B size
    assert_eq!(std::mem::size_of::<CrossQueueSyncCapsule>(), 1024);
}

#[test]
fn test_chaos_lockfree_guarantee() {
    // All operations use AtomicU64/AtomicU32 (lockfree by definition on modern CPUs)
    // This test verifies that operations complete without blocking

    let sync = CrossQueueSyncCapsule::new();

    // These should never block
    sync.signal_queue(QueueType::Graphics, 100).unwrap();
    sync.add_dependency(QueueType::Compute, QueueType::Graphics, 100).unwrap();
    let _ready = sync.is_ready(QueueType::Compute);
    let _timeline = sync.get_queue_timeline(QueueType::Graphics);
    let _snapshot = sync.snapshot();
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_edge_case_dependency_on_self() {
    let sync = CrossQueueSyncCapsule::new();

    // Add dependency on self (should work but be immediately satisfied)
    sync.signal_queue(QueueType::Graphics, 100).unwrap();
    sync.add_dependency(QueueType::Graphics, QueueType::Graphics, 100).unwrap();

    // Should be ready (timeline >= 100)
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_edge_case_clear_nonexistent_dependencies() {
    let sync = CrossQueueSyncCapsule::new();

    // Clear dependencies on a queue that has none
    sync.clear_dependencies(QueueType::Graphics).unwrap();

    // Should still work fine
    assert!(sync.is_ready(QueueType::Graphics));
}

#[test]
fn test_edge_case_timeline_zero() {
    let sync = CrossQueueSyncCapsule::new();

    // All queues start at timeline 0
    for queue in QueueType::ALL_QUEUES {
        assert_eq!(sync.get_queue_timeline(*queue), 0);
    }

    // Waiting for timeline 0 should always succeed
    assert!(sync.wait_for_queue(QueueType::Graphics, 0));
}
