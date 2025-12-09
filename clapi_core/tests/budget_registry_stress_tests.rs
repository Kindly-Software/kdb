//! T28 Tier 3: Stress Testing (Q15-Q21)
//!
//! High-load stress tests for budget slot capsule operations.
//!
//! **Coverage:**
//! - Q15: Integration points (full allocation/deallocation cycles)
//! - Q16: Error propagation (capacity limits, invalid operations)
//! - Q17: Performance under stress (10K+ operations)
//! - Q18: Production load simulation (concurrent hammering)
//! - Q19: Recovery scenarios (deallocation after exhaustion)
//! - Q20: Boundary conditions (capacity limits)
//! - Q21: Sustained load (memory stability)
//!
//! **Test Count:** 15 stress tests

use clapi_core::capsules::{BudgetMetaCapsule, MAX_BUDGET_SLOTS};
use clapi_core::error::ClapiError;
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// T28 Q15-Q17: Integration & Performance (5 tests)
// ============================================================================

#[test]
fn test_1m_allocations_full_capacity() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();

    // Act: Fill to MAX_BUDGET_SLOTS
    for i in 0..MAX_BUDGET_SLOTS {
        let result = meta.allocate(budget_id, 100_00);
        assert!(result.is_ok(), "Allocation {} failed", i);
    }

    // Assert: All slots filled
    assert_eq!(meta.slot_count(), MAX_BUDGET_SLOTS);

    // Assert: Next allocation fails gracefully
    let result = meta.allocate(budget_id, 100_00);
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::SlotsExhausted { .. })));
}

#[test]
fn test_allocation_deallocation_cycles() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();
    let batch_size = 1000;
    let cycles = 10;

    // Act: 10 cycles of allocate → deallocate
    for cycle in 0..cycles {
        // Allocate batch
        let mut slot_ids = Vec::new();
        for _ in 0..batch_size {
            let slot_id = meta.allocate(budget_id, 100_00).unwrap();
            slot_ids.push(slot_id);
        }

        assert_eq!(
            meta.slot_count(),
            batch_size,
            "Cycle {}: Slot count mismatch after allocation",
            cycle
        );

        // Deallocate batch
        for slot_id in slot_ids {
            meta.deallocate(slot_id).unwrap();
        }

        assert_eq!(
            meta.slot_count(),
            0,
            "Cycle {}: Slot count mismatch after deallocation",
            cycle
        );
    }

    // Assert: Metacapsule stable after 10 cycles
    assert_eq!(meta.slot_count(), 0);
    assert!(meta.generation() > (cycles * batch_size * 2) as u64); // Allocate + deallocate
}

#[test]
fn test_high_contention_100_threads() {
    let budget_id = 1u64;
    // Arrange
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 100;
    let ops_per_thread = 100;

    // Act: 100 threads × 100 operations
    let start = std::time::Instant::now();

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    let mut meta = m.lock().unwrap();
                    let _ = meta.allocate(budget_id, 100_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();

    // Assert: All operations completed
    let meta = meta.lock().unwrap();
    assert_eq!(meta.slot_count(), num_threads * ops_per_thread);

    // Assert: Reasonable throughput
    let ops_per_sec = (num_threads * ops_per_thread) as f64 / elapsed.as_secs_f64();
    assert!(
        ops_per_sec > 10_000.0,
        "Throughput too low: {:.0} ops/s",
        ops_per_sec
    );
}

#[test]
fn test_mixed_workload_allocate_deallocate_get() {
    let budget_id = 1u64;
    // Arrange
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 50;

    // Act: Mixed operations
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let m = Arc::clone(&meta);
            thread::spawn(move || {
                match i % 3 {
                    0 => {
                        // Allocate
                        for _ in 0..100 {
                            let mut meta = m.lock().unwrap();
                            let _ = meta.allocate(budget_id, 100_00);
                        }
                    }
                    1 => {
                        // Get (read-heavy)
                        for slot_id in 0..1000 {
                            let meta = m.lock().unwrap();
                            let _ = meta.get(slot_id);
                        }
                    }
                    _ => {
                        // Allocate then deallocate
                        for _ in 0..50 {
                            let mut meta = m.lock().unwrap();
                            if let Ok((slot_id, _)) = meta.allocate(budget_id, 100_00) {
                                drop(meta);
                                thread::yield_now();
                                let mut meta = m.lock().unwrap();
                                let _ = meta.deallocate(slot_id);
                            }
                        }
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: System stable after mixed workload
    let meta = meta.lock().unwrap();
    assert!(meta.slot_count() <= MAX_BUDGET_SLOTS);
}

#[test]
fn test_sequential_vs_random_access_performance() {
    let budget_id = 1u64;
    // Arrange: Allocate 10K slots
    let mut meta = BudgetMetaCapsule::new();
    for _ in 0..10_000 {
        meta.allocate(budget_id, 100_00).unwrap();
    }

    // Act: Sequential access
    let start = std::time::Instant::now();
    for slot_id in 0..10_000 {
        let _ = meta.get(slot_id).unwrap();
    }
    let seq_elapsed = start.elapsed();

    // Act: Random access (with pseudo-random pattern)
    let start = std::time::Instant::now();
    for i in 0..10_000 {
        let slot_id = (i * 7919) % 10_000; // Prime modulo for distribution
        let _ = meta.get(slot_id).unwrap();
    }
    let rand_elapsed = start.elapsed();

    // Assert: Both patterns complete quickly
    assert!(
        seq_elapsed.as_millis() < 50,
        "Sequential access too slow: {}ms",
        seq_elapsed.as_millis()
    );
    assert!(
        rand_elapsed.as_millis() < 100,
        "Random access too slow: {}ms",
        rand_elapsed.as_millis()
    );
}

// ============================================================================
// T28 Q18-Q19: Production Load & Recovery (4 tests)
// ============================================================================

#[test]
fn test_rapid_allocation_to_capacity() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();
    let target = 100_000; // 10% of capacity

    // Act: Rapid allocation
    let start = std::time::Instant::now();
    for _ in 0..target {
        meta.allocate(budget_id, 100_00).unwrap();
    }
    let elapsed = start.elapsed();

    // Assert: Fast allocation rate
    let ops_per_sec = target as f64 / elapsed.as_secs_f64();
    assert!(
        ops_per_sec > 100_000.0,
        "Allocation rate too low: {:.0} ops/s",
        ops_per_sec
    );

    assert_eq!(meta.slot_count(), target);
}

#[test]
fn test_exhaustion_then_recovery() {
    let budget_id = 1u64;
    // Arrange: Fill to capacity
    let mut meta = BudgetMetaCapsule::new();
    let initial_allocations = 1000;

    for _ in 0..initial_allocations {
        meta.allocate(budget_id, 100_00).unwrap();
    }

    // Act: Deallocate half to create space
    for slot_id in 0..500 {
        meta.deallocate(slot_id).unwrap();
    }

    assert_eq!(meta.slot_count(), 500);

    // Act: Allocate again (recovery)
    for _ in 0..500 {
        meta.allocate(budget_id, 200_00).unwrap();
    }

    // Assert: System recovered
    assert_eq!(meta.slot_count(), 1000);
}

#[test]
fn test_concurrent_allocation_with_backpressure() {
    let budget_id = 1u64;
    // Arrange: Start near capacity
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));

    // Pre-allocate slots to create contention
    {
        let mut meta = meta.lock().unwrap();
        for _ in 0..10_000 {
            meta.allocate(budget_id, 100_00).unwrap();
        }
    }

    let num_threads = 20;

    // Act: Concurrent allocations under backpressure
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            thread::spawn(move || {
                let mut successful = 0;
                for _ in 0..100 {
                    let mut meta = m.lock().unwrap();
                    if meta.allocate(budget_id, 100_00).is_ok() {
                        successful += 1;
                    }
                }
                successful
            })
        })
        .collect();

    let total_success: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();

    // Assert: Some allocations succeeded despite backpressure
    assert!(total_success > 0, "No allocations succeeded");
}

#[test]
fn test_memory_stability_under_churn() {
    let budget_id = 1u64;
    // Arrange
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let duration = std::time::Duration::from_secs(2);
    let start = std::time::Instant::now();

    // Act: Continuous churn for 2 seconds
    let handle = {
        let m = Arc::clone(&meta);
        thread::spawn(move || {
            let mut slot_ids = Vec::new();
            let mut operations = 0;

            while std::time::Instant::now() - start < duration {
                let mut meta = m.lock().unwrap();

                // Allocate
                if let Ok((slot_id, _)) = meta.allocate(budget_id, 100_00) {
                    slot_ids.push(slot_id);
                    operations += 1;

                    // Periodically deallocate
                    if slot_ids.len() > 100 {
                        if let Some(old_slot) = slot_ids.pop() {
                            let _ = meta.deallocate(old_slot);
                            operations += 1;
                        }
                    }
                }
            }

            operations
        })
    };

    let operations = handle.join().unwrap();

    // Assert: High operation count (sustained throughput)
    assert!(
        operations > 1000,
        "Low operation count: {} ops in 2s",
        operations
    );

    // Assert: Memory stable (slot count bounded)
    let meta = meta.lock().unwrap();
    assert!(meta.slot_count() <= 200); // Bounded by churn logic
}

// ============================================================================
// T28 Q20-Q21: Boundary & Sustained Load (6 tests)
// ============================================================================

#[test]
fn test_exact_capacity_boundary() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();

    // Act: Fill exactly to MAX_BUDGET_SLOTS
    for _ in 0..MAX_BUDGET_SLOTS {
        assert!(meta.allocate(budget_id, 100_00).is_ok());
    }

    // Assert: At capacity
    assert_eq!(meta.slot_count(), MAX_BUDGET_SLOTS);

    // Assert: One more allocation fails
    assert!(meta.allocate(budget_id, 100_00).is_err());
}

#[test]
fn test_deallocation_below_zero() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();

    // Act: Try to deallocate when empty
    let result = meta.deallocate(0);

    // Assert: Error (cannot go below zero)
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::NoSlotsAllocated) | Err(ClapiError::SlotNotAllocated { .. })));
}

#[test]
fn test_large_budget_values() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();

    // Act: Allocate with i64::MAX budget
    let result = meta.allocate(i64::MAX);
    assert!(result.is_ok());

    let (slot_id, capsule) = result.unwrap();

    // Assert: Large value preserved
    assert_eq!(capsule.budget(), i64::MAX);
    assert_eq!(meta.get(slot_id).unwrap().budget(), i64::MAX);
}

#[test]
fn test_sustained_get_operations() {
    let budget_id = 1u64;
    // Arrange: Allocate 1000 slots
    let mut meta = BudgetMetaCapsule::new();
    for _ in 0..1000 {
        meta.allocate(budget_id, 100_00).unwrap();
    }

    let meta = Arc::new(meta);
    let duration = std::time::Duration::from_secs(2);
    let start = std::time::Instant::now();

    // Act: Sustained read load
    let handle = {
        let m = Arc::clone(&meta);
        thread::spawn(move || {
            let mut reads = 0;
            while std::time::Instant::now() - start < duration {
                for slot_id in 0..1000 {
                    if m.get(slot_id).is_ok() {
                        reads += 1;
                    }
                }
            }
            reads
        })
    };

    let reads = handle.join().unwrap();

    // Assert: High read throughput
    let reads_per_sec = reads as f64 / 2.0;
    assert!(
        reads_per_sec > 100_000.0,
        "Read throughput too low: {:.0} reads/s",
        reads_per_sec
    );
}

#[test]
fn test_slot_id_boundary_edge_cases() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();

    // Allocate first and last valid slots
    let (first_id, _) = meta.allocate(budget_id, 100_00).unwrap();
    assert_eq!(first_id, 0);

    // Fill to capacity
    for _ in 1..MAX_BUDGET_SLOTS {
        meta.allocate(budget_id, 100_00).unwrap();
    }

    let last_id = MAX_BUDGET_SLOTS - 1;

    // Assert: First and last slots are accessible
    assert!(meta.get(first_id).is_ok());
    assert!(meta.get(last_id).is_ok());

    // Assert: Beyond last is invalid
    assert!(meta.get(MAX_BUDGET_SLOTS).is_err());
}

#[test]
fn test_generation_counter_growth_rate() {
    let budget_id = 1u64;
    // Arrange
    let mut meta = BudgetMetaCapsule::new();
    let start_gen = meta.generation();

    // Act: 1000 allocations + 500 deallocations
    let mut slot_ids = Vec::new();
    for _ in 0..1000 {
        let slot_id = meta.allocate(budget_id, 100_00).unwrap();
        slot_ids.push(slot_id);
    }

    for i in 0..500 {
        meta.deallocate(slot_ids[i]).unwrap();
    }

    let end_gen = meta.generation();

    // Assert: Generation grew by (1000 allocations + 500 deallocations)
    let gen_growth = end_gen - start_gen;
    assert_eq!(gen_growth, 1500, "Generation growth mismatch");
}
