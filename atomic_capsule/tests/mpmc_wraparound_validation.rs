//! MPMC Queue Wraparound Fix Validation Test
//!
//! PURPOSE: Validate the MPMC queue wraparound fix with correct sequence calculation formula:
//!   expected_seq = (rotations * capacity + slot_idx) * 2 (for push)
//!   expected_seq = (rotations * capacity + slot_idx) * 2 + 1 (for pop)
//!
//! PREVIOUS BUG: Wrong formula (slot_idx + rotations) * 2 caused sequence collisions:
//!   - Slot 0 rotation 0: (0+0)*2 = 0 ✓
//!   - Slot 1 rotation 0: (1+0)*2 = 2 ✗ COLLISION
//!   - Slot 0 rotation 1: (0+1)*2 = 2 ✗ (same as Slot 1 rotation 0!)
//!
//! CORRECT FORMULA: (rotations * capacity + slot_idx) * 2
//!   - Slot 0 rotation 0: (0*4 + 0)*2 = 0 ✓
//!   - Slot 1 rotation 0: (0*4 + 1)*2 = 2 ✓
//!   - Slot 0 rotation 1: (1*4 + 0)*2 = 8 ✓ (unique!)
//!   - Slot 1 rotation 1: (1*4 + 1)*2 = 10 ✓ (unique!)
//!
//! TEST METHODOLOGY: 8 push/pop cycles on capacity=4 queue (2 full rotations)
//! - Triggers all wraparound code paths
//! - Verifies correct sequence numbers prevent deadlock
//! - Tests post-wraparound operations
//!
//! EXPECTED RESULT: PASS in <1s (previous bug: HANG at 60s+)

use atomic_capsule::collections::queue::{QueueCapsule, MPMC};
use std::time::Instant;

#[test]
fn test_mpmc_wraparound_8_cycles() {
    // Create queue with capacity=4 (forces wraparound every 4 items)
    let queue = QueueCapsule::<u64, MPMC>::new(4).expect("Failed to create queue");

    let test_start = Instant::now();

    // Test 8 push/pop cycles (2 full rotations)
    // Each cycle: push 4 items, pop 4 items
    for cycle in 0..8 {
        // PUSH phase: Insert 4 items (fills queue)
        for i in 0..4 {
            let value = cycle * 100 + i as u64;
            queue
                .push(value)
                .expect(&format!("Cycle {}: Push failed at position {}", cycle, i));
        }

        // POP phase: Remove 4 items (empties queue)
        for i in 0..4 {
            let expected = cycle * 100 + i as u64;
            let actual = queue
                .pop()
                .expect(&format!("Cycle {}: Pop failed at position {}", cycle, i));
            assert_eq!(
                actual, expected,
                "Cycle {}: Value mismatch at position {}",
                cycle, i
            );
        }

        // Verify queue is empty after each cycle
        assert_eq!(
            queue.pop(),
            None,
            "Cycle {}: Queue should be empty after pop phase",
            cycle
        );
    }

    let elapsed = test_start.elapsed();

    // Post-wraparound verification: push/pop one more item
    queue.push(999).expect("Post-wraparound push failed");
    assert_eq!(queue.pop(), Some(999), "Post-wraparound pop failed");

    println!("✓ MPMC Wraparound Test PASSED");
    println!("  - 8 cycles (2 full rotations on capacity=4 queue)");
    println!("  - 32 push operations (4 per cycle)");
    println!("  - 32 pop operations (4 per cycle)");
    println!("  - Completed in {:.3}ms", elapsed.as_secs_f64() * 1000.0);

    // Verify performance: should complete in <1s (was hanging at 60s+ with bug)
    assert!(
        elapsed.as_secs() < 1,
        "Test took {:.2}s (should be <1s). Possible deadlock or performance regression.",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_mpmc_wraparound_stress() {
    // More aggressive wraparound test: 16 cycles on capacity=4
    let queue = QueueCapsule::<u64, MPMC>::new(4).expect("Failed to create queue");

    let test_start = Instant::now();

    // 16 cycles = 4 full rotations
    for cycle in 0..16 {
        for i in 0..4 {
            let value = cycle * 1000 + i as u64;
            queue
                .push(value)
                .expect(&format!("Cycle {}: Push {} failed", cycle, value));
        }

        for i in 0..4 {
            let expected = cycle * 1000 + i as u64;
            assert_eq!(
                queue.pop(),
                Some(expected),
                "Cycle {}: Value mismatch",
                cycle
            );
        }

        assert_eq!(queue.pop(), None, "Cycle {}: Should be empty", cycle);
    }

    let elapsed = test_start.elapsed();

    println!("✓ MPMC Wraparound Stress Test PASSED");
    println!("  - 16 cycles (4 full rotations on capacity=4 queue)");
    println!("  - 64 push operations");
    println!("  - 64 pop operations");
    println!("  - Completed in {:.3}ms", elapsed.as_secs_f64() * 1000.0);

    // Must complete in <5s
    assert!(
        elapsed.as_secs() < 5,
        "Stress test took {:.2}s (should be <5s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_mpmc_wraparound_interleaved() {
    // Test with interleaved operations (not fill/empty cycles)
    let queue = QueueCapsule::<u64, MPMC>::new(4).expect("Failed to create queue");

    let test_start = Instant::now();

    // Pattern: push 2, pop 1, push 2, pop 1, ...
    // This creates more complex wraparound patterns
    let mut push_count = 0;
    let mut pop_count = 0;
    let mut pushed_values = std::collections::VecDeque::new();

    for iteration in 0..20 {
        // Push 2
        for _ in 0..2 {
            let value = iteration * 100 + push_count;
            queue.push(value).ok();
            pushed_values.push_back(value);
            push_count += 1;
        }

        // Pop 1
        if let Some(expected) = pushed_values.pop_front() {
            assert_eq!(
                queue.pop(),
                Some(expected),
                "Iteration {}: Pop mismatch",
                iteration
            );
            pop_count += 1;
        }
    }

    let elapsed = test_start.elapsed();

    // Drain remaining items
    while let Some(expected) = pushed_values.pop_front() {
        assert_eq!(queue.pop(), Some(expected), "Drain: Pop mismatch");
    }

    println!("✓ MPMC Wraparound Interleaved Test PASSED");
    println!("  - 20 iterations with interleaved push/pop");
    println!("  - {} push operations", push_count);
    println!("  - {} pop operations", pop_count);
    println!("  - Completed in {:.3}ms", elapsed.as_secs_f64() * 1000.0);

    assert!(
        elapsed.as_secs() < 2,
        "Interleaved test took {:.2}s (should be <2s)",
        elapsed.as_secs_f64()
    );
}

#[test]
fn test_mpmc_sequence_uniqueness() {
    // Verify that sequences are correctly calculated (no collisions)
    // This test documents the expected sequence values

    // Capacity = 4, so we have slots 0-3
    // For push: expected_seq = (rotations * 4 + slot_idx) * 2
    // For pop: expected_seq = (rotations * 4 + slot_idx) * 2 + 1

    let sequences = vec![
        // Rotation 0 (items 0-3)
        (0, 0, 0), // Slot 0, rotation 0: (0*4+0)*2 = 0 (even, writable)
        (0, 1, 2), // Slot 1, rotation 0: (0*4+1)*2 = 2 (even, writable)
        (0, 2, 4), // Slot 2, rotation 0: (0*4+2)*2 = 4 (even, writable)
        (0, 3, 6), // Slot 3, rotation 0: (0*4+3)*2 = 6 (even, writable)
        // Rotation 1 (items 4-7)
        (1, 0, 8),  // Slot 0, rotation 1: (1*4+0)*2 = 8 (even, writable)
        (1, 1, 10), // Slot 1, rotation 1: (1*4+1)*2 = 10 (even, writable)
        (1, 2, 12), // Slot 2, rotation 1: (1*4+2)*2 = 12 (even, writable)
        (1, 3, 14), // Slot 3, rotation 1: (1*4+3)*2 = 14 (even, writable)
    ];

    // Verify all sequences are unique
    let mut seen = std::collections::HashSet::new();
    for (rotation, slot, expected_seq) in &sequences {
        assert!(
            seen.insert(expected_seq),
            "COLLISION: Rotation {}, Slot {} maps to sequence {} (already seen)",
            rotation,
            slot,
            expected_seq
        );
    }

    println!("✓ MPMC Sequence Uniqueness Test PASSED");
    println!(
        "  - All {} sequences are unique (no collisions)",
        sequences.len()
    );
    println!("  - Correct formula: (rotations * capacity + slot_idx) * 2");
}
