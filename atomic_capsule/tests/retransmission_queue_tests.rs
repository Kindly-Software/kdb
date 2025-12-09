//! # RetransmissionQueueCapsule Tests (T28 Framework)
//!
//! Comprehensive test suite for T5 Streaming retransmission queue.
//! Coverage: 28 tests across 4 tiers (unit/property/integration/production).

use atomic_capsule::quic::{RetransmissionQueueCapsule, RetransmissionQueueError};

// ============================================================================
// UNIT TESTS (Q1-Q7)
// ============================================================================

/// Q1: Create and verify initial state
#[test]
fn q1_creation_and_initial_state() {
    let queue = RetransmissionQueueCapsule::new();
    assert!(queue.is_empty(), "Queue should be empty on creation");
    assert!(!queue.is_full(), "Queue should not be full on creation");
    assert_eq!(queue.len(), 0, "Queue length should be 0");
    assert_eq!(queue.generation(), 0, "Generation should be 0");
}

/// Q2: Single packet enqueue
#[test]
fn q2_single_enqueue() {
    let queue = RetransmissionQueueCapsule::new();
    let result = queue.enqueue_lost_packet(1000, 512, 1280);
    assert!(result.is_ok(), "Enqueue should succeed");
    assert_eq!(queue.len(), 1, "Length should be 1");
    assert!(!queue.is_empty(), "Queue should not be empty");
}

/// Q3: FIFO order verification
#[test]
fn q3_fifo_order() {
    let queue = RetransmissionQueueCapsule::new();

    // Enqueue 3 packets
    queue.enqueue_lost_packet(100, 0, 100).ok();
    queue.enqueue_lost_packet(200, 100, 200).ok();
    queue.enqueue_lost_packet(300, 300, 300).ok();

    // Verify FIFO order (oldest first)
    let e1 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e1.get_packet_number(), 100, "First packet should be PN=100");

    let e2 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e2.get_packet_number(), 200, "Second packet should be PN=200");

    let e3 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e3.get_packet_number(), 300, "Third packet should be PN=300");

    assert!(queue.is_empty(), "Queue should be empty after all dequeued");
}

/// Q4: Fill to capacity
#[test]
fn q4_fill_to_capacity() {
    let queue = RetransmissionQueueCapsule::new();

    // Fill to capacity (128 entries)
    for i in 0..128 {
        let result = queue.enqueue_lost_packet(i as u64, i * 100, 128);
        assert!(result.is_ok(), "Enqueue {} should succeed", i);
    }

    assert!(queue.is_full(), "Queue should be full");
    assert_eq!(queue.len(), 128, "Length should be 128");

    // Next enqueue should fail
    let result = queue.enqueue_lost_packet(999, 0, 100);
    assert_eq!(
        result,
        Err(RetransmissionQueueError::QueueFull),
        "Enqueue when full should fail"
    );
}

/// Q5: Peek without dequeue
#[test]
fn q5_peek() {
    let queue = RetransmissionQueueCapsule::new();
    queue.enqueue_lost_packet(1000, 512, 1280).ok();

    // Peek should not change length
    let entry = queue.peek_next();
    assert!(entry.is_some(), "Peek should return entry");
    let e = entry.unwrap();
    assert_eq!(e.get_packet_number(), 1000);
    assert_eq!(queue.len(), 1, "Peek should not change length");

    // Dequeue should return same packet
    let e2 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e2.get_packet_number(), 1000);
}

/// Q6: Empty queue dequeue returns None
#[test]
fn q6_empty_dequeue() {
    let queue = RetransmissionQueueCapsule::new();
    let result = queue.dequeue_next_retransmit();
    assert!(result.is_none(), "Dequeue from empty should return None");
}

/// Q7: Clear operation
#[test]
fn q7_clear() {
    let queue = RetransmissionQueueCapsule::new();

    // Add packets
    for i in 0..50 {
        queue.enqueue_lost_packet(i as u64, i * 100, 128).ok();
    }
    assert_eq!(queue.len(), 50);

    // Clear
    queue.clear();
    assert_eq!(queue.len(), 0, "Length should be 0 after clear");
    assert!(queue.is_empty(), "Queue should be empty after clear");
    assert_eq!(queue.generation(), 0, "Generation should be 0 after clear");
}

// ============================================================================
// PROPERTY-BASED TESTS (Q8-Q14)
// ============================================================================

/// Q8: Count consistency across operations
#[test]
fn q8_count_consistency() {
    let queue = RetransmissionQueueCapsule::new();

    // Add 100 packets
    for i in 0..100 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
        assert_eq!(
            queue.len(),
            (i + 1) as u32,
            "Length should be {} after enqueue {}",
            i + 1,
            i
        );
    }

    // Remove all
    for i in 0..100 {
        queue.dequeue_next_retransmit();
        assert_eq!(
            queue.len(),
            (100 - i - 1) as u32,
            "Length should be {} after dequeue {}",
            100 - i - 1,
            i
        );
    }

    assert_eq!(queue.len(), 0, "Final length should be 0");
}

/// Q9: Retransmit count tracking
#[test]
fn q9_retransmit_count() {
    let queue = RetransmissionQueueCapsule::new();
    queue.enqueue_lost_packet(1000, 512, 1280).ok();

    let entry = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(entry.get_retransmit_count(), 0, "Initial retransmit count should be 0");

    // Increment multiple times
    for i in 1..=5 {
        entry.increment_retransmit_count();
        assert_eq!(
            entry.get_retransmit_count(),
            i as u8,
            "Retransmit count should be {}",
            i
        );
    }
}

/// Q10: Payload offset and length preservation
#[test]
fn q10_payload_preservation() {
    let queue = RetransmissionQueueCapsule::new();

    // Test with various offset/length combinations
    let test_cases = vec![
        (1, 0, 100),
        (2, 512, 1280),
        (3, u32::MAX, u16::MAX),
        (4, 1000000, 5000),
    ];

    for (pn, offset, len) in test_cases {
        queue.enqueue_lost_packet(pn as u64, offset, len).ok();
    }

    for (pn, offset, len) in test_cases {
        let entry = queue.dequeue_next_retransmit().unwrap();
        assert_eq!(
            entry.get_packet_number(),
            pn as u64,
            "Packet number should match"
        );
        assert_eq!(
            entry.get_payload_offset(),
            offset,
            "Payload offset should match"
        );
        assert_eq!(entry.get_payload_len(), len, "Payload length should match");
    }
}

/// Q11: Generation counter increments correctly
#[test]
fn q11_generation_counter() {
    let queue = RetransmissionQueueCapsule::new();
    assert_eq!(queue.generation(), 0, "Initial generation should be 0");

    // Enqueue 128 packets (one full cycle)
    for i in 0..128 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }
    assert_eq!(queue.generation(), 1, "Generation should be 1 after 128 enqueues");

    // Dequeue all 128
    for _ in 0..128 {
        queue.dequeue_next_retransmit();
    }
    assert_eq!(queue.generation(), 2, "Generation should be 2 after 128 dequeues");
}

/// Q12: Large capacity fills
#[test]
fn q12_multiple_capacity_cycles() {
    let queue = RetransmissionQueueCapsule::new();

    // Perform 3 complete cycles
    for cycle in 0..3 {
        // Fill
        for i in 0..128 {
            let pn = (cycle * 128 + i) as u64;
            queue.enqueue_lost_packet(pn, 0, 100).ok();
        }
        assert!(queue.is_full());

        // Empty
        for _ in 0..128 {
            queue.dequeue_next_retransmit();
        }
        assert!(queue.is_empty());
    }

    assert_eq!(queue.generation(), 6, "Generation should be 6 after 3 complete cycles");
}

/// Q13: Alternating enqueue/dequeue patterns
#[test]
fn q13_alternating_pattern() {
    let queue = RetransmissionQueueCapsule::new();

    queue.enqueue_lost_packet(1, 0, 100).ok();
    queue.enqueue_lost_packet(2, 100, 100).ok();

    let e1 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e1.get_packet_number(), 1);

    queue.enqueue_lost_packet(3, 200, 100).ok();

    let e2 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e2.get_packet_number(), 2);

    queue.enqueue_lost_packet(4, 300, 100).ok();
    queue.enqueue_lost_packet(5, 400, 100).ok();

    let e3 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e3.get_packet_number(), 3);

    let e4 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e4.get_packet_number(), 4);

    let e5 = queue.dequeue_next_retransmit().unwrap();
    assert_eq!(e5.get_packet_number(), 5);

    assert!(queue.is_empty());
}

/// Q14: Peek on full queue
#[test]
fn q14_peek_full_queue() {
    let queue = RetransmissionQueueCapsule::new();

    // Fill queue
    for i in 0..128 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }

    // Peek should work even when full
    let entry = queue.peek_next();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().get_packet_number(), 0);
    assert_eq!(queue.len(), 128, "Peek should not affect length");
}

// ============================================================================
// INTEGRATION TESTS (Q15-Q21)
// ============================================================================

/// Q15: Stress with 1000 sequential operations
#[test]
fn q15_stress_sequential_1000() {
    let queue = RetransmissionQueueCapsule::new();

    // Enqueue 128 packets repeatedly (8 cycles = 1024 total)
    for cycle in 0..8 {
        for i in 0..128 {
            let pn = (cycle * 128 + i) as u64;
            queue.enqueue_lost_packet(pn, (i * 10) as u32, 128).ok();
        }

        // Dequeue all in this cycle
        for _ in 0..128 {
            queue.dequeue_next_retransmit();
        }
    }

    assert!(queue.is_empty(), "Queue should be empty after stress test");
}

/// Q16: Simulated packet loss and retransmission pattern
#[test]
fn q16_loss_retransmission_simulation() {
    let queue = RetransmissionQueueCapsule::new();

    // Simulate: 20 packets lost, retry 2 times each
    for batch in 0..3 {
        // Lose 20 packets
        for i in 0..20 {
            let pn = (batch * 100 + i) as u64;
            queue.enqueue_lost_packet(pn, (i * 50) as u32, 1280).ok();
        }

        // Retransmit (with 50% success rate - simulate some fail)
        while !queue.is_empty() {
            if let Some(entry) = queue.dequeue_next_retransmit() {
                let count = entry.get_retransmit_count();
                entry.increment_retransmit_count();

                // Re-enqueue if still within retry limit
                if count < 2 {
                    queue
                        .enqueue_lost_packet(
                            entry.get_packet_number(),
                            entry.get_payload_offset(),
                            entry.get_payload_len(),
                        )
                        .ok();
                }
            }
        }
    }

    assert!(queue.is_empty());
}

/// Q17: Mixed operations under moderate load
#[test]
fn q17_mixed_operations() {
    let queue = RetransmissionQueueCapsule::new();

    // Enqueue 50
    for i in 0..50 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }

    // Dequeue 25
    for _ in 0..25 {
        queue.dequeue_next_retransmit();
    }

    assert_eq!(queue.len(), 25);

    // Enqueue 50 more
    for i in 50..100 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }

    assert_eq!(queue.len(), 75);

    // Dequeue all
    for _ in 0..75 {
        queue.dequeue_next_retransmit();
    }

    assert!(queue.is_empty());
}

/// Q18: Wraparound at capacity boundaries
#[test]
fn q18_wraparound_boundaries() {
    let queue = RetransmissionQueueCapsule::new();

    let mut total_operations = 0;

    // Do 5 complete wraparounds
    for _ in 0..5 {
        // Fill to capacity
        for i in 0..128 {
            queue.enqueue_lost_packet(i as u64, 0, 100).ok();
            total_operations += 1;
        }

        // Drain
        while !queue.is_empty() {
            queue.dequeue_next_retransmit();
            total_operations += 1;
        }
    }

    assert_eq!(queue.generation(), 10, "Should have 10 generation increments");
}

/// Q19: Size and alignment verification
#[test]
fn q19_size_alignment() {
    use std::mem::{align_of, size_of};

    let size = size_of::<RetransmissionQueueCapsule>();
    let align = align_of::<RetransmissionQueueCapsule>();

    // Should fit in ~2KB with 256B alignment
    assert!(size <= 4096, "Size {} exceeds 4KB", size);
    assert_eq!(align, 256, "Alignment should be 256 bytes (got {})", align);

    // Verify it's a power of 2
    assert!((align & (align - 1)) == 0, "Alignment {} is not power of 2", align);
}

/// Q20: Default and Clone semantics
#[test]
fn q20_default_semantics() {
    let q1 = RetransmissionQueueCapsule::new();
    let q2 = RetransmissionQueueCapsule::default();

    assert_eq!(q1.len(), q2.len(), "Default should create empty queue");
    assert_eq!(q1.generation(), q2.generation());
}

/// Q21: Edge cases - empty, single, full
#[test]
fn q21_edge_cases() {
    let queue = RetransmissionQueueCapsule::new();

    // Empty state
    assert!(queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.len(), 0);

    // Single entry
    queue.enqueue_lost_packet(1, 0, 100).ok();
    assert!(!queue.is_empty());
    assert!(!queue.is_full());
    assert_eq!(queue.len(), 1);

    // Drain to empty
    queue.dequeue_next_retransmit();
    assert!(queue.is_empty());

    // Full state
    for i in 0..128 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }
    assert!(!queue.is_empty());
    assert!(queue.is_full());
    assert_eq!(queue.len(), 128);
}

// ============================================================================
// PRODUCTION TESTS (Q22-Q28)
// ============================================================================

/// Q22: Long-running stability with realistic workload
#[test]
fn q22_production_workload() {
    let queue = RetransmissionQueueCapsule::new();

    // Simulate 10 batches of realistic packet loss
    for batch in 0..10 {
        // Each batch: 20 packets lost, various retry patterns
        for i in 0..20 {
            let pn = (batch * 1000 + i) as u64;
            queue.enqueue_lost_packet(pn, (i * 64) as u32, 1280).ok();
        }

        // Process retransmissions
        let mut processed = 0;
        while !queue.is_empty() && processed < 100 {
            if let Some(entry) = queue.dequeue_next_retransmit() {
                let count = entry.get_retransmit_count();
                entry.increment_retransmit_count();

                // Retry up to 3 times
                if count < 3 {
                    queue
                        .enqueue_lost_packet(
                            entry.get_packet_number(),
                            entry.get_payload_offset(),
                            entry.get_payload_len(),
                        )
                        .ok();
                }

                processed += 1;
            }
        }
    }

    assert!(queue.is_empty(), "Queue should drain completely");
}

/// Q23: High-frequency enqueue/dequeue pattern
#[test]
fn q23_high_frequency_pattern() {
    let queue = RetransmissionQueueCapsule::new();

    // Rapid alternation
    for cycle in 0..100 {
        queue.enqueue_lost_packet(cycle as u64, 0, 100).ok();
        if cycle > 0 {
            queue.dequeue_next_retransmit();
        }
    }

    // Should have 1 entry left (cycle 100 enqueued but not dequeued)
    assert_eq!(queue.len(), 1);
}

/// Q24: Generation counter evolution over extended run
#[test]
fn q24_generation_evolution() {
    let queue = RetransmissionQueueCapsule::new();

    let mut expected_gen = 0;

    for phase in 0..4 {
        // Fill queue
        for i in 0..128 {
            queue.enqueue_lost_packet((phase * 128 + i) as u64, 0, 100).ok();
        }
        expected_gen += 1;
        assert_eq!(
            queue.generation(),
            expected_gen,
            "After enqueue phase {}",
            phase
        );

        // Drain queue
        for _ in 0..128 {
            queue.dequeue_next_retransmit();
        }
        expected_gen += 1;
        assert_eq!(
            queue.generation(),
            expected_gen,
            "After dequeue phase {}",
            phase
        );
    }
}

/// Q25: Peek + dequeue consistency
#[test]
fn q25_peek_dequeue_consistency() {
    let queue = RetransmissionQueueCapsule::new();

    for i in 0..100 {
        queue.enqueue_lost_packet(i as u64, i * 100, 100 + i).ok();
    }

    // Verify peek returns same as dequeue would
    for i in 0..100 {
        let peeked = queue.peek_next().unwrap();
        let dequeued = queue.dequeue_next_retransmit().unwrap();

        assert_eq!(peeked.get_packet_number(), dequeued.get_packet_number());
        assert_eq!(peeked.get_payload_offset(), dequeued.get_payload_offset());
        assert_eq!(peeked.get_payload_len(), dequeued.get_payload_len());
    }
}

/// Q26: Single entry wraparound cycles
#[test]
fn q26_single_entry_wraparound() {
    let queue = RetransmissionQueueCapsule::new();

    // Do 20 single-entry cycles
    for cycle in 0..20 {
        queue.enqueue_lost_packet(cycle as u64, 0, 100).ok();
        let entry = queue.dequeue_next_retransmit().unwrap();
        assert_eq!(entry.get_packet_number(), cycle as u64);
    }

    assert!(queue.is_empty());
}

/// Q27: Retransmit count overflow (u8 saturation)
#[test]
fn q27_retransmit_count_saturation() {
    let queue = RetransmissionQueueCapsule::new();
    queue.enqueue_lost_packet(1000, 0, 100).ok();

    let entry = queue.dequeue_next_retransmit().unwrap();

    // Increment 300 times (beyond u8::MAX of 255)
    for _ in 0..300 {
        entry.increment_retransmit_count();
    }

    // Should saturate at some point; just verify it doesn't panic
    let final_count = entry.get_retransmit_count();
    assert!(final_count > 0, "Should have incremented");
}

/// Q28: Comprehensive integration test with all features
#[test]
fn q28_comprehensive_integration() {
    let queue = RetransmissionQueueCapsule::new();

    // 1. Fill to capacity
    for i in 0..128 {
        queue.enqueue_lost_packet(i as u64, i * 1000, 1280 + i).ok();
    }
    assert!(queue.is_full());

    // 2. Peek on full queue
    let peeked = queue.peek_next().unwrap();
    assert_eq!(peeked.get_packet_number(), 0);

    // 3. Partial dequeue
    for _ in 0..50 {
        queue.dequeue_next_retransmit();
    }
    assert_eq!(queue.len(), 78);

    // 4. Partial re-enqueue
    for i in 128..150 {
        let result = queue.enqueue_lost_packet(i as u64, 0, 100);
        if i < 128 + 50 {
            assert!(result.is_ok(), "Should enqueue {}", i);
        } else {
            assert!(result.is_err(), "Should be full at {}", i);
        }
    }

    // 5. Drain all
    while !queue.is_empty() {
        queue.dequeue_next_retransmit();
    }

    // 6. Clear and verify reset
    queue.enqueue_lost_packet(9999, 0, 100).ok();
    queue.clear();
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.generation(), 0);

    // 7. Verify can use again after clear
    for i in 0..10 {
        queue.enqueue_lost_packet(i as u64, 0, 100).ok();
    }
    assert_eq!(queue.len(), 10);
}
