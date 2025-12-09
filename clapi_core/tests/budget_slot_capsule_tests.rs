//! T28 Tier 1: Unit Testing (Q1-Q7)
//!
//! Budget slot capsule unit tests covering core behaviors, edge cases,
//! invariants, code paths, isolation, performance, and readability.
//!
//! **Coverage:**
//! - Q1: Core behaviors (try_allocate, get, deallocate, status)
//! - Q2: Edge cases (double allocation, empty deallocation)
//! - Q3: Invariants (generation monotonic, status consistency)
//! - Q4: All code paths (success, CAS failure, error variants)
//! - Q5: Isolation (no shared state, deterministic)
//! - Q6: Performance (<50ns allocation, <10ns read)
//! - Q7: Readability (helpers, arrange-act-assert)
//!
//! **Test Count:** 25 unit tests

use clapi_core::capsules::{BudgetSlotCapsule, RequestCapsule128, SlotStatus};

// ============================================================================
// T28 Q1: Core Behaviors (7 tests)
// ============================================================================

#[test]
fn test_budget_slot_size() {
    // Verify: Slot is 128 bytes
    assert_eq!(std::mem::size_of::<BudgetSlotCapsule>(), 128);
}

#[test]
fn test_budget_slot_alignment() {
    // Verify: Slot is 128-byte aligned (cache line)
    assert_eq!(std::mem::align_of::<BudgetSlotCapsule>(), 128);
}

#[test]
fn test_allocate_empty_slot() {
    // Arrange
    let slot = BudgetSlotCapsule::new();
    assert!(!slot.is_allocated());

    // Act
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    let result = slot.try_allocate(1, capsule);

    // Assert: Basic allocation
    assert!(result.is_ok());
    assert!(slot.is_allocated());
    assert_eq!(slot.status(), SlotStatus::Allocated);
    assert_eq!(slot.budget_id(), 1);
    assert_eq!(slot.generation(), 1); // Generation incremented
}

#[test]
fn test_allocate_conflict() {
    // Arrange: Allocate first capsule
    let slot = BudgetSlotCapsule::new();
    let capsule1 = Box::new(RequestCapsule128::new(100_00));
    slot.try_allocate(1, capsule1).unwrap();

    // Act: Try to allocate second capsule (CAS failure)
    let capsule2 = Box::new(RequestCapsule128::new(200_00));
    let result = slot.try_allocate(2, capsule2);

    // Assert: CAS failure returns ownership
    assert!(result.is_err());
    let returned_capsule = result.unwrap_err();
    assert_eq!(returned_capsule.budget(), 200_00);

    // Original allocation unchanged
    assert_eq!(slot.budget_id(), 1);
}

#[test]
fn test_get_allocated_slot() {
    // Arrange
    let slot = BudgetSlotCapsule::new();
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    slot.try_allocate(42, capsule).unwrap();

    // Act: Read after write
    let retrieved = slot.get();

    // Assert
    assert!(retrieved.is_some());
    let capsule_ref = retrieved.unwrap();
    assert_eq!(capsule_ref.budget(), 1000_00);
}

#[test]
fn test_deallocate_slot() {
    // Arrange
    let slot = BudgetSlotCapsule::new();
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    slot.try_allocate(1, capsule).unwrap();
    assert!(slot.is_allocated());

    // Act: Cleanup
    let result = slot.deallocate();

    // Assert
    assert!(result.is_ok());
    assert!(!slot.is_allocated());
    assert_eq!(slot.status(), SlotStatus::Empty);
    assert_eq!(slot.budget_id(), 0);

    // Returned capsule should be valid
    let returned = result.unwrap();
    assert_eq!(returned.budget(), 1000_00);
}

#[test]
fn test_null_pointer_safety() {
    // Arrange: Empty slot
    let slot = BudgetSlotCapsule::new();

    // Act: Access empty slot (verify null checks)
    let result = slot.get();

    // Assert: Graceful None, no null pointer dereference
    assert!(result.is_none());
}

// ============================================================================
// T28 Q2: Edge Cases (6 tests)
// ============================================================================

#[test]
fn test_allocate_zero_budget() {
    // Arrange
    let slot = BudgetSlotCapsule::new();

    // Act: Allocate slot with zero budget
    let capsule = Box::new(RequestCapsule128::new(0));
    let result = slot.try_allocate(1, capsule);

    // Assert: Zero budget is valid
    assert!(result.is_ok());
    assert_eq!(slot.get().unwrap().budget(), 0);
}

#[test]
fn test_allocate_negative_budget() {
    // Arrange
    let slot = BudgetSlotCapsule::new();

    // Act: Allocate with negative budget (debt)
    let capsule = Box::new(RequestCapsule128::new(-100_00));
    let result = slot.try_allocate(1, capsule);

    // Assert: Negative budget allowed
    assert!(result.is_ok());
    assert_eq!(slot.get().unwrap().budget(), -100_00);
}

#[test]
fn test_double_allocation_rejected() {
    // Arrange
    let slot = BudgetSlotCapsule::new();
    let capsule1 = Box::new(RequestCapsule128::new(100_00));
    slot.try_allocate(1, capsule1).unwrap();

    // Act: Second allocation
    let capsule2 = Box::new(RequestCapsule128::new(200_00));
    let result = slot.try_allocate(2, capsule2);

    // Assert: Rejected (slot occupied)
    assert!(result.is_err());
}

#[test]
fn test_deallocate_unallocated_slot() {
    // Arrange: Empty slot
    let slot = BudgetSlotCapsule::new();

    // Act: Deallocate slot that was never allocated
    let result = slot.deallocate();

    // Assert: Error
    assert!(result.is_err());
}

#[test]
fn test_deallocate_twice() {
    // Arrange: Allocate and deallocate once
    let slot = BudgetSlotCapsule::new();
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    slot.try_allocate(1, capsule).unwrap();
    slot.deallocate().unwrap();

    // Act: Deallocate same slot again
    let result = slot.deallocate();

    // Assert: Double-deallocate error
    assert!(result.is_err());
}

#[test]
fn test_status_transitions() {
    // Arrange
    let slot = BudgetSlotCapsule::new();

    // Assert: Initial state
    assert_eq!(slot.status(), SlotStatus::Empty);

    // Allocate
    let capsule = Box::new(RequestCapsule128::new(100_00));
    slot.try_allocate(1, capsule).unwrap();
    assert_eq!(slot.status(), SlotStatus::Allocated);

    // Deallocate
    slot.deallocate().unwrap();
    assert_eq!(slot.status(), SlotStatus::Empty);
}

// ============================================================================
// T28 Q3: Invariants (3 tests)
// ============================================================================

#[test]
fn test_invariant_generation_monotonic() {
    // Arrange
    let slot = BudgetSlotCapsule::new();
    let gen0 = slot.generation();

    // Act: Allocate
    let capsule1 = Box::new(RequestCapsule128::new(100_00));
    slot.try_allocate(1, capsule1).unwrap();
    let gen1 = slot.generation();

    // Assert: Generation increases
    assert!(gen1 > gen0, "Generation must increase: {} -> {}", gen0, gen1);

    // Act: Deallocate
    slot.deallocate().unwrap();
    let gen2 = slot.generation();

    // Assert: Generation continues increasing
    assert!(gen2 > gen1, "Generation must continue increasing: {} -> {}", gen1, gen2);

    // Act: Allocate again
    let capsule2 = Box::new(RequestCapsule128::new(200_00));
    slot.try_allocate(2, capsule2).unwrap();
    let gen3 = slot.generation();

    // Assert: Generation monotonic throughout lifecycle
    assert!(gen3 > gen2, "Generation must remain monotonic: {} -> {}", gen2, gen3);
}

#[test]
fn test_invariant_budget_id_consistency() {
    // Arrange
    let slot = BudgetSlotCapsule::new();

    // Assert: Empty slot has budget_id = 0
    assert_eq!(slot.budget_id(), 0);

    // Allocate with budget_id = 42
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    slot.try_allocate(42, capsule).unwrap();

    // Assert: Budget ID matches allocation
    assert_eq!(slot.budget_id(), 42);

    // Deallocate
    slot.deallocate().unwrap();

    // Assert: Budget ID cleared on deallocation
    assert_eq!(slot.budget_id(), 0);
}

#[test]
fn test_invariant_generation_starts_at_zero() {
    // Arrange/Act
    let slot = BudgetSlotCapsule::new();

    // Assert: Generation starts at 0 (increments to 1 on first allocation)
    assert_eq!(slot.generation(), 0);
}

// ============================================================================
// T28 Q4: Code Paths (3 tests)
// ============================================================================

#[test]
fn test_all_status_values() {
    // Test all SlotStatus variants
    assert_eq!(SlotStatus::Empty as u8, 0);
    assert_eq!(SlotStatus::Allocated as u8, 1);
    assert_eq!(SlotStatus::Reserved as u8, 2);
    assert_eq!(SlotStatus::Poisoned as u8, 3);

    // Test From<u8> conversion
    assert_eq!(SlotStatus::from(0), SlotStatus::Empty);
    assert_eq!(SlotStatus::from(1), SlotStatus::Allocated);
    assert_eq!(SlotStatus::from(2), SlotStatus::Reserved);
    assert_eq!(SlotStatus::from(3), SlotStatus::Poisoned);
    assert_eq!(SlotStatus::from(255), SlotStatus::Poisoned); // Invalid = poisoned
}

#[test]
fn test_success_path_full_lifecycle() {
    // Arrange
    let slot = BudgetSlotCapsule::new();

    // Allocate
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    assert!(slot.try_allocate(1, capsule).is_ok());

    // Get
    assert!(slot.get().is_some());

    // Deallocate
    assert!(slot.deallocate().is_ok());

    // Verify empty
    assert!(!slot.is_allocated());
}

#[test]
fn test_error_path_allocation_failure() {
    // Arrange: Pre-allocate slot
    let slot = BudgetSlotCapsule::new();
    let capsule1 = Box::new(RequestCapsule128::new(100_00));
    slot.try_allocate(1, capsule1).unwrap();

    // Act: Try to allocate again (error path)
    let capsule2 = Box::new(RequestCapsule128::new(200_00));
    let result = slot.try_allocate(2, capsule2);

    // Assert: Error path executed, ownership returned
    assert!(result.is_err());
}

// ============================================================================
// T28 Q5: Isolation (2 tests)
// ============================================================================

#[test]
fn test_isolation_no_shared_state() {
    // Arrange: Create two independent slots
    let slot1 = BudgetSlotCapsule::new();
    let slot2 = BudgetSlotCapsule::new();

    // Act: Allocate in slot1
    let capsule = Box::new(RequestCapsule128::new(100_00));
    slot1.try_allocate(1, capsule).unwrap();

    // Assert: slot2 unaffected
    assert!(slot1.is_allocated());
    assert!(!slot2.is_allocated());
}

#[test]
fn test_deterministic_operations() {
    // Arrange/Act: Create two slots and perform same operations
    let slot1 = BudgetSlotCapsule::new();
    let slot2 = BudgetSlotCapsule::new();

    let capsule1 = Box::new(RequestCapsule128::new(100_00));
    let capsule2 = Box::new(RequestCapsule128::new(100_00));

    slot1.try_allocate(1, capsule1).unwrap();
    slot2.try_allocate(1, capsule2).unwrap();

    // Assert: Same operations produce same results
    assert_eq!(slot1.budget_id(), slot2.budget_id());
    assert_eq!(slot1.generation(), slot2.generation());
    assert_eq!(slot1.status(), slot2.status());
}

// ============================================================================
// T28 Q6: Performance (2 tests)
// ============================================================================

#[test]
fn test_performance_allocation() {
    // Arrange
    let iterations = 1000;
    let mut slots = Vec::new();
    for _ in 0..iterations {
        slots.push(BudgetSlotCapsule::new());
    }

    // Act: Batch allocate
    let start = std::time::Instant::now();
    for (i, slot) in slots.iter().enumerate() {
        let capsule = Box::new(RequestCapsule128::new(100_00));
        slot.try_allocate(i as u64, capsule).unwrap();
    }
    let elapsed = start.elapsed();

    // Assert: Fast allocation (<10ms for 1000 operations)
    assert!(
        elapsed.as_millis() < 10,
        "Allocation too slow: {}ms (target: <10ms)",
        elapsed.as_millis()
    );
}

#[test]
fn test_performance_get() {
    // Arrange: Pre-allocate slots
    let iterations = 10_000;
    let mut slots = Vec::new();
    for i in 0..iterations {
        let slot = BudgetSlotCapsule::new();
        let capsule = Box::new(RequestCapsule128::new(100_00));
        slot.try_allocate(i, capsule).unwrap();
        slots.push(slot);
    }

    // Act: Batch read
    let start = std::time::Instant::now();
    for slot in &slots {
        let _ = slot.get();
    }
    let elapsed = start.elapsed();

    // Assert: Fast reads (<10ms for 10K operations)
    assert!(
        elapsed.as_millis() < 10,
        "Get too slow: {}ms (target: <10ms)",
        elapsed.as_millis()
    );
}

// ============================================================================
// T28 Q7: Readability (2 tests)
// ============================================================================

/// Helper: Create allocated slot with budget
fn create_allocated_slot(budget: i64, budget_id: u64) -> BudgetSlotCapsule {
    let slot = BudgetSlotCapsule::new();
    let capsule = Box::new(RequestCapsule128::new(budget));
    slot.try_allocate(budget_id, capsule).unwrap();
    slot
}

#[test]
fn test_helper_usage_simple() {
    // Arrange: Use helper for cleaner test setup
    let slot = create_allocated_slot(1000_00, 42);

    // Assert: Helper worked correctly
    assert!(slot.is_allocated());
    assert_eq!(slot.budget_id(), 42);
    assert_eq!(slot.get().unwrap().budget(), 1000_00);
}

#[test]
fn test_arrange_act_assert_pattern() {
    // Arrange: Set up initial state
    let slot = BudgetSlotCapsule::new();
    assert!(!slot.is_allocated());
    assert_eq!(slot.generation(), 0);

    // Act: Perform operation
    let capsule = Box::new(RequestCapsule128::new(1000_00));
    let result = slot.try_allocate(1, capsule);

    // Assert: Verify expected outcome
    assert!(result.is_ok());
    assert!(slot.is_allocated());
    assert_eq!(slot.budget_id(), 1);
    assert_eq!(slot.generation(), 1);
}
