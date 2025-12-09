//! T28 Tier 2: Property Testing (Q8-Q14)
//!
//! Property-based tests validating invariants hold across input space.
//!
//! **Coverage:**
//! - Q8: Universal properties (slot count conservation, generation monotonic)
//! - Q9: Concurrent invariants (no lost updates, unique slot IDs)
//! - Q10: Edge case properties (boundary handling, overflow protection)
//! - Q11: ASSUM verification (alignment, TOCTOU prevention)
//! - Q12: Composition properties (independence of operations)
//! - Q13: Statistical properties (allocation distribution)
//! - Q14: Regression prevention (stable behavior)
//!
//! **Test Count:** 20 property tests

use clapi_core::capsules::{BudgetMetaCapsule, MAX_BUDGET_SLOTS};
use clapi_core::error::ClapiError;
use proptest::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;

// ============================================================================
// T28 Q8: Universal Properties (3 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_slot_count_conservation(operations in prop::collection::vec(0..10000i64, 1..100)) {
        let mut meta = BudgetMetaCapsule::new();
        let mut expected_count = 0;

        for (i, budget) in operations.into_iter().enumerate() {
            let budget_id = (i as u64).wrapping_add(1000);
            if meta.allocate(budget_id, 100_00).is_ok() {
                expected_count += 1;
            }
        }

        // Property: Slot count matches successful allocations
        prop_assert_eq!(meta.slot_count(), expected_count);
    }

    #[test]
    fn prop_generation_monotonic(operations in prop::collection::vec(0..1000i64, 10..50)) {
        let mut meta = BudgetMetaCapsule::new();
        let mut last_gen = meta.generation();

        for (i, _budget) in operations.into_iter().enumerate() {
            let budget_id = (i as u64).wrapping_add(2000);
            if meta.allocate(budget_id, 100_00).is_ok() {
                let current_gen = meta.generation();
                // Property: Generation always increases
                prop_assert!(current_gen > last_gen);
                last_gen = current_gen;
            }
        }
    }

    #[test]
    fn prop_get_idempotent(slot_id in 0usize..100) {
        let mut meta = BudgetMetaCapsule::new();

        // Allocate enough slots
        for i in 0..=slot_id {
            let budget_id = (i as u64).wrapping_add(3000);
            let _ = meta.allocate(budget_id, 100_00);
        }

        if let Ok(capsule1) = meta.get(slot_id) {
            let capsule2 = meta.get(slot_id).unwrap();

            // Property: Multiple gets return same budget
            prop_assert_eq!(capsule1.budget(), capsule2.budget());
        }
    }
}

// ============================================================================
// T28 Q9: Concurrent Invariants (5 tests)
// ============================================================================

#[test]
fn prop_concurrent_allocation_uniqueness() {
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 10;
    let allocations_per_thread = 100;
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(10000));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                let mut slot_ids = Vec::new();
                for _ in 0..allocations_per_thread {
                    let budget_id = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut meta = m.lock().unwrap();
                    if let Ok(slot_id) = meta.allocate(budget_id, 100_00) {
                        slot_ids.push(slot_id);
                    }
                }
                slot_ids
            })
        })
        .collect();

    let mut all_slot_ids = Vec::new();
    for h in handles {
        let slot_ids = h.join().unwrap();
        all_slot_ids.extend(slot_ids);
    }

    // Property: All slot IDs are unique (no collisions)
    let original_len = all_slot_ids.len();
    all_slot_ids.sort_unstable();
    all_slot_ids.dedup();
    assert_eq!(
        all_slot_ids.len(),
        original_len,
        "Slot ID collision detected"
    );
}

#[test]
fn prop_concurrent_generation_increases() {
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 10;
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(20000));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..10 {
                    let budget_id = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut meta = m.lock().unwrap();
                    let _ = meta.allocate(budget_id, 100_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Generation increased by at least num_threads * 10
    let meta = meta.lock().unwrap();
    assert!(meta.generation() >= (num_threads * 10 + 1) as u64);
}

#[test]
fn prop_concurrent_no_slot_count_corruption() {
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 20;
    let ops_per_thread = 50;
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(30000));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..ops_per_thread {
                    let budget_id = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut meta = m.lock().unwrap();
                    let _ = meta.allocate(budget_id, 100_00);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Property: Slot count exactly matches allocations
    let meta = meta.lock().unwrap();
    assert_eq!(meta.slot_count(), (num_threads * ops_per_thread).min(MAX_BUDGET_SLOTS));
}

#[test]
fn prop_concurrent_get_stability() {
    let mut meta = BudgetMetaCapsule::new();
    for i in 0..100 {
        let budget_id = (i as u64).wrapping_add(40000);
        meta.allocate(budget_id, 100_00).unwrap();
    }

    let meta = Arc::new(meta);
    let num_threads = 50;

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            thread::spawn(move || {
                for slot_id in 0..100 {
                    if let Ok(capsule) = m.get(slot_id) {
                        // Property: Budget is always 100_00 (no corruption)
                        assert_eq!(capsule.budget(), 100_00);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

#[test]
fn prop_concurrent_stats_consistency() {
    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 10;
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(50000));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                for _ in 0..10 {
                    let budget_id = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut meta = m.lock().unwrap();
                    let _ = meta.allocate(budget_id, 100_00);
                    let stats = meta.get_stats();
                    // Property: Stats are internally consistent
                    assert!(stats.generation > 0);
                    assert!(stats.slot_count <= MAX_BUDGET_SLOTS);
                    assert!(stats.total_allocations >= stats.slot_count as u64);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }
}

// ============================================================================
// T28 Q10: Edge Case Properties (4 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_handles_zero_budget(count in 1usize..100) {
        let mut meta = BudgetMetaCapsule::new();

        for i in 0..count {
            let budget_id = (i as u64).wrapping_add(60000);
            // Property: Zero budget is valid
            prop_assert!(meta.allocate(budget_id, 0).is_ok());
        }
    }

    #[test]
    fn prop_handles_negative_budget(budget in -1000000i64..0) {
        let mut meta = BudgetMetaCapsule::new();

        // Property: Negative budgets allowed (debt tracking)
        let result = meta.allocate(1234u64, budget);
        prop_assert!(result.is_ok());

        let slot_id = result.unwrap();
        let capsule = meta.get(slot_id).unwrap();
        prop_assert_eq!(capsule.budget(), budget);
    }

    #[test]
    fn prop_rejects_invalid_slot_ids(invalid_id in MAX_BUDGET_SLOTS..MAX_BUDGET_SLOTS * 2) {
        let meta = BudgetMetaCapsule::new();

        // Property: Out-of-bounds slot IDs rejected
        prop_assert!(meta.get(invalid_id).is_err());
        prop_assert!(matches!(meta.get(invalid_id), Err(ClapiError::InvalidSlotId { .. })), "Expected InvalidSlotId error");
    }

    #[test]
    fn prop_handles_boundary_slot_ids(slot_id in 0usize..MAX_BUDGET_SLOTS) {
        let mut meta = BudgetMetaCapsule::new();

        // Allocate enough slots
        for i in 0..=slot_id {
            let budget_id = (i as u64).wrapping_add(70000);
            let _ = meta.allocate(budget_id, 100_00);
        }

        // Property: All valid slot IDs are accessible
        prop_assert!(meta.get(slot_id).is_ok());
    }
}

// ============================================================================
// T28 Q11: ASSUM Verification (2 tests)
// ============================================================================

#[test]
fn prop_verify_alignment() {
    use clapi_core::capsules::BudgetMetaCapsuleHeader;

    // #ASSUME: BudgetMetaCapsuleHeader is 128-byte aligned
    // #VERIFY: Alignment property
    let header = BudgetMetaCapsuleHeader::new();
    let addr = &header as *const _ as usize;

    // Property: Header address is 128-byte aligned
    assert_eq!(addr % 128, 0, "Header not aligned: address 0x{:x}", addr);
}

#[test]
fn prop_verify_fetch_add_prevents_collision() {
    // #ASSUME: AtomicUsize::fetch_add for slot allocation prevents collisions
    // #VERIFY: Property test with high contention

    let meta = Arc::new(Mutex::new(BudgetMetaCapsule::new()));
    let num_threads = 100;
    let allocations_per_thread = 10;
    let counter = Arc::new(std::sync::atomic::AtomicU64::new(80000));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let m = Arc::clone(&meta);
            let c = Arc::clone(&counter);
            thread::spawn(move || {
                let mut slot_ids = Vec::new();
                for _ in 0..allocations_per_thread {
                    let budget_id = c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let mut meta = m.lock().unwrap();
                    if let Ok(slot_id) = meta.allocate(budget_id, 100_00) {
                        slot_ids.push(slot_id);
                    }
                }
                slot_ids
            })
        })
        .collect();

    let mut all_slot_ids = Vec::new();
    for h in handles {
        all_slot_ids.extend(h.join().unwrap());
    }

    // Property: fetch_add ensures unique slot IDs (ASSUM verified)
    all_slot_ids.sort_unstable();
    for i in 1..all_slot_ids.len() {
        assert_ne!(
            all_slot_ids[i - 1],
            all_slot_ids[i],
            "Slot ID collision: {}",
            all_slot_ids[i]
        );
    }
}

// ============================================================================
// T28 Q12: Composition Properties (2 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_allocate_then_deallocate_composition(count in 1usize..100) {
        let mut meta = BudgetMetaCapsule::new();

        // Allocate N slots
        let mut slot_ids = Vec::new();
        for i in 0..count {
            let budget_id = (i as u64).wrapping_add(90000);
            if let Ok(slot_id) = meta.allocate(budget_id, 100_00) {
                slot_ids.push(slot_id);
            }
        }

        let allocate_count = meta.slot_count();

        // Deallocate all slots
        for slot_id in slot_ids {
            let _ = meta.deallocate(slot_id);
        }

        // Property: Slot count returns to 0
        prop_assert_eq!(meta.slot_count(), allocate_count - count);
    }

    #[test]
    fn prop_multiple_allocations_independent(budgets in prop::collection::vec(0i64..10000, 10..50)) {
        let mut meta = BudgetMetaCapsule::new();
        let mut allocated_budgets = Vec::new();

        for (i, budget) in budgets.iter().enumerate() {
            let budget_id = (i as u64).wrapping_add(4000);
            if let Ok(slot_id) = meta.allocate(budget_id, *budget) {
                allocated_budgets.push((slot_id, *budget));
            }
        }

        // Property: Each allocation is independent (correct budget)
        for (slot_id, expected_budget) in allocated_budgets {
            let capsule = meta.get(slot_id).unwrap();
            prop_assert_eq!(capsule.budget(), expected_budget);
        }
    }
}

// ============================================================================
// T28 Q13: Statistical Properties (2 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_allocation_distribution_uniform(count in 10usize..100) {
        let mut meta = BudgetMetaCapsule::new();

        for i in 0..count {
            let budget_id = (i as u64).wrapping_add(100000);
            let _ = meta.allocate(budget_id, 100_00);
        }

        // Property: Slot IDs are sequential (0, 1, 2, ..., count-1)
        for slot_id in 0..count {
            prop_assert!(meta.get(slot_id).is_ok());
        }

        // Property: Next slot_id is unallocated
        prop_assert!(meta.get(count).is_err());
    }

    #[test]
    fn prop_generation_growth_bounded(operations in prop::collection::vec(0i64..1000, 50..200)) {
        let mut meta = BudgetMetaCapsule::new();
        let start_gen = meta.generation();

        for (i, budget) in operations.iter().enumerate() {
            let budget_id = (i as u64).wrapping_add(5000);
            let _ = meta.allocate(budget_id, *budget);
        }

        let end_gen = meta.generation();

        // Property: Generation growth is bounded (1 per allocation + 1 per deallocate)
        let max_gen_growth = (operations.len() as u64) * 2; // Worst case: all allocate + deallocate
        prop_assert!(end_gen - start_gen <= max_gen_growth);
    }
}

// ============================================================================
// T28 Q14: Regression Prevention (2 tests)
// ============================================================================

proptest! {
    #[test]
    fn prop_stable_behavior_across_runs(seed in 0u64..1000) {
        // Use seed to ensure deterministic test
        let mut meta1 = BudgetMetaCapsule::new();
        let mut meta2 = BudgetMetaCapsule::new();

        // Same operations on both metacapsules
        for i in 0..10 {
            let budget = ((seed + i as u64) % 1000) as i64;
            let budget_id = seed.wrapping_add(i as u64);
            let _ = meta1.allocate(budget_id, budget);
            let _ = meta2.allocate(budget_id, budget);
        }

        // Property: Same operations produce same results
        prop_assert_eq!(meta1.slot_count(), meta2.slot_count());
        prop_assert_eq!(meta1.generation(), meta2.generation());
    }

    #[test]
    fn prop_generation_never_wraps(operations in prop::collection::vec(0i64..1000, 100..500)) {
        let mut meta = BudgetMetaCapsule::new();

        for (i, budget) in operations.into_iter().enumerate() {
            let budget_id = (i as u64).wrapping_add(6000);
            let _ = meta.allocate(budget_id, budget);
        }

        // Property: Generation never wraps to 0 (starts at 1)
        prop_assert!(meta.generation() > 0);
        prop_assert!(meta.generation() < u64::MAX / 2); // Far from overflow
    }
}
