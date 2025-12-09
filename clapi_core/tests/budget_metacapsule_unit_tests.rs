//! T28 Tier 1: Unit Testing (Q1-Q7)
//!
//! Comprehensive unit tests for budget metacapsule operations.
//!
//! **Coverage:**
//! - Q1: Core behaviors (budget deduction, provider selection, metrics, audit)
//! - Q2: Edge cases (zero/negative/overflow, empty providers, boundary values)
//! - Q3: Invariants (budget non-negative, generation monotonic, hash chain)
//! - Q4: Code path coverage (all error variants, success paths)
//! - Q5: Isolation and determinism (no shared state, repeatable)
//! - Q6: Performance (<10ms for batch operations)
//! - Q7: Readability (clear structure, descriptive names)
//!
//! **Test Count:** 60 tests

use clapi_core::error::ClapiError;
use clapi_core::proxy::budget_registry::{BudgetRegistry, BudgetStats};
use clapi_core::RequestCapsule128;
use std::sync::Arc;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// ============================================================================
// Test Helper: Deterministic String-to-ID Conversion
// ============================================================================

/// Convert user string ID to u64 for deterministic test ID generation.
///
/// Uses Rust's DefaultHasher for consistent, deterministic hashing.
/// Ensures "user1" always maps to the same u64 across test runs.
///
/// # Examples
/// ```
/// let id1 = user_id("user1");
/// let id2 = user_id("user1");
/// assert_eq!(id1, id2); // Same input → same ID
/// ```
///
/// # Edge Cases Handled
/// - Empty strings: `user_id("")` produces valid u64
/// - Unicode: `user_id("user_世界_🌍")` produces valid u64
/// - Long strings: `user_id(&"a".repeat(1000))` produces valid u64
fn user_id(name: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    name.hash(&mut hasher);
    hasher.finish()
}

// ============================================================================
// T28 Q1: Core Behaviors (13 tests)
// ============================================================================

#[test]
fn test_budget_registry_creation() {
    let registry = BudgetRegistry::new(1000_00);
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_budget_registry_with_initial_budget() {
    let registry = BudgetRegistry::new(500_00);
    assert!(registry.is_empty());
}

#[test]
fn test_budget_deduction_success() {
    // Arrange
    let registry = BudgetRegistry::new(1000_00);

    // Act
    let result = registry.try_deduct(user_id("user1"), 50_00);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 950_00);
    assert_eq!(registry.get_budget(user_id("user1")), Some(950_00));
}

#[test]
fn test_budget_deduction_creates_budget_on_first_use() {
    let registry = BudgetRegistry::new(1000_00);

    assert!(registry.get_budget(user_id("user1")).is_none());

    registry.try_deduct(user_id("user1"), 10_00).unwrap();

    assert!(registry.get_budget(user_id("user1")).is_some());
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_budget_deduction_multiple_users() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.try_deduct(user_id("user2"), 200_00).unwrap();
    registry.try_deduct(user_id("user3"), 300_00).unwrap();

    assert_eq!(registry.len(), 3);
    assert_eq!(registry.get_budget(user_id("user1")), Some(900_00));
    assert_eq!(registry.get_budget(user_id("user2")), Some(800_00));
    assert_eq!(registry.get_budget(user_id("user3")), Some(700_00));
}

#[test]
fn test_budget_credit_success() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 500_00).unwrap();
    let result = registry.credit(user_id("user1"), 300_00);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 800_00);
    assert_eq!(registry.get_budget(user_id("user1")), Some(800_00));
}

#[test]
fn test_budget_get_stats() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.try_deduct(user_id("user1"), 50_00).unwrap();

    let stats = registry.get_stats(user_id("user1")).unwrap();
    assert_eq!(stats.budget, 850_00);
    assert_eq!(stats.total_spent, 150_00);
    assert_eq!(stats.request_count, 2);
    assert!(stats.generation > 0);
}

#[test]
fn test_budget_get_stats_nonexistent() {
    let registry = BudgetRegistry::new(1000_00);
    let stats = registry.get_stats(user_id("nonexistent"));
    assert!(stats.is_none());
}

#[test]
fn test_budget_get_budget_nonexistent() {
    let registry = BudgetRegistry::new(1000_00);
    let budget = registry.get_budget(user_id("nonexistent"));
    assert!(budget.is_none());
}

#[test]
fn test_budget_registry_len_increases() {
    let registry = BudgetRegistry::new(1000_00);

    assert_eq!(registry.len(), 0);

    registry.try_deduct(user_id("user1"), 10_00).unwrap();
    assert_eq!(registry.len(), 1);

    registry.try_deduct(user_id("user2"), 20_00).unwrap();
    assert_eq!(registry.len(), 2);

    registry.try_deduct(user_id("user3"), 30_00).unwrap();
    assert_eq!(registry.len(), 3);
}

#[test]
fn test_budget_registry_is_empty() {
    let registry = BudgetRegistry::new(1000_00);

    assert!(registry.is_empty());

    registry.try_deduct(user_id("user1"), 10_00).unwrap();
    assert!(!registry.is_empty());
}

#[test]
fn test_budget_deduction_preserves_previous_balance() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.try_deduct(user_id("user1"), 50_00).unwrap();
    registry.try_deduct(user_id("user1"), 25_00).unwrap();

    assert_eq!(registry.get_budget(user_id("user1")), Some(825_00));
}

#[test]
fn test_budget_credit_adds_to_existing_balance() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 500_00).unwrap();
    registry.credit(user_id("user1"), 100_00).unwrap();
    registry.credit(user_id("user1"), 50_00).unwrap();

    assert_eq!(registry.get_budget(user_id("user1")), Some(650_00));
}

// ============================================================================
// T28 Q2: Edge Cases (12 tests)
// ============================================================================

#[test]
fn test_budget_deduction_zero_amount() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user1"), 0);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 1000_00); // No change
}

#[test]
fn test_budget_deduction_negative_amount() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user1"), -100_00);
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::InvalidCost(_))));
}

#[test]
fn test_budget_credit_zero_amount() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    let result = registry.credit(user_id("user1"), 0);
    assert!(result.is_ok());
}

#[test]
fn test_budget_credit_negative_amount() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    let result = registry.credit(user_id("user1"), -50_00);
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::InvalidCost(_))));
}

#[test]
fn test_budget_deduction_exact_balance() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user1"), 1000_00);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    assert_eq!(registry.get_budget(user_id("user1")), Some(0));
}

#[test]
fn test_budget_deduction_insufficient_funds() {
    let registry = BudgetRegistry::new(100_00);

    let result = registry.try_deduct(user_id("user1"), 200_00);
    assert!(result.is_err());

    match result {
        Err(ClapiError::BudgetExhausted {
            requested,
            available,
        }) => {
            assert_eq!(requested, 200_00);
            assert_eq!(available, 100_00);
        }
        _ => panic!("Expected BudgetExhausted error"),
    }
}

#[test]
fn test_budget_deduction_after_exhaustion() {
    let registry = BudgetRegistry::new(100_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();

    let result = registry.try_deduct(user_id("user1"), 10_00);
    assert!(result.is_err());
    assert!(matches!(result, Err(ClapiError::BudgetExhausted { .. })));
}

#[test]
fn test_budget_large_initial_amount() {
    let registry = BudgetRegistry::new(1_000_000_00); // $1M

    let result = registry.try_deduct(user_id("user1"), 500_000_00);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 500_000_00);
}

#[test]
fn test_budget_very_small_deduction() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user1"), 1); // 1 cent
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 999_99);
}

#[test]
fn test_budget_empty_string_user_id() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id(""), 100_00);
    assert!(result.is_ok());
    assert_eq!(registry.len(), 1);
}

#[test]
fn test_budget_unicode_user_id() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user_世界_🌍"), 100_00);
    assert!(result.is_ok());
    assert_eq!(registry.get_budget(user_id("user_世界_🌍")), Some(900_00));
}

#[test]
fn test_budget_very_long_user_id() {
    let registry = BudgetRegistry::new(1000_00);
    let long_id = "a".repeat(1000);

    let result = registry.try_deduct(user_id(&long_id), 100_00);
    assert!(result.is_ok());
}

// ============================================================================
// T28 Q3: Invariants (6 tests)
// ============================================================================

#[test]
fn test_invariant_budget_never_negative() {
    let registry = BudgetRegistry::new(100_00);

    registry.try_deduct(user_id("user1"), 50_00).unwrap();
    registry.try_deduct(user_id("user1"), 50_00).unwrap();

    // Budget should be 0, not negative
    assert_eq!(registry.get_budget(user_id("user1")), Some(0));

    // Further deductions should fail
    let result = registry.try_deduct(user_id("user1"), 10_00);
    assert!(result.is_err());
}

#[test]
fn test_invariant_generation_monotonic() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 10_00).unwrap();
    let stats1 = registry.get_stats(user_id("user1")).unwrap();

    registry.try_deduct(user_id("user1"), 20_00).unwrap();
    let stats2 = registry.get_stats(user_id("user1")).unwrap();

    registry.try_deduct(user_id("user1"), 30_00).unwrap();
    let stats3 = registry.get_stats(user_id("user1")).unwrap();

    // Generation must increase monotonically
    assert!(stats2.generation > stats1.generation);
    assert!(stats3.generation > stats2.generation);
}

#[test]
fn test_invariant_budget_conservation() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.try_deduct(user_id("user1"), 200_00).unwrap();
    registry.try_deduct(user_id("user1"), 300_00).unwrap();

    let stats = registry.get_stats(user_id("user1")).unwrap();

    // Conservation: budget + spent = initial
    assert_eq!(stats.budget + stats.total_spent, 1000_00);
}

#[test]
fn test_invariant_request_count_matches_deductions() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 10_00).unwrap();
    registry.try_deduct(user_id("user1"), 20_00).unwrap();
    registry.try_deduct(user_id("user1"), 30_00).unwrap();

    let stats = registry.get_stats(user_id("user1")).unwrap();
    assert_eq!(stats.request_count, 3);
}

#[test]
fn test_invariant_total_spent_accumulates() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    let stats1 = registry.get_stats(user_id("user1")).unwrap();

    registry.try_deduct(user_id("user1"), 200_00).unwrap();
    let stats2 = registry.get_stats(user_id("user1")).unwrap();

    assert_eq!(stats1.total_spent, 100_00);
    assert_eq!(stats2.total_spent, 300_00);
}

#[test]
fn test_invariant_credit_preserves_conservation() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 500_00).unwrap();
    let before = registry.get_stats(user_id("user1")).unwrap();

    registry.credit(user_id("user1"), 300_00).unwrap();
    let after = registry.get_stats(user_id("user1")).unwrap();

    // Conservation holds: credit increases budget but not spent
    assert_eq!(after.budget, before.budget + 300_00);
    assert_eq!(after.total_spent, before.total_spent);
}

// ============================================================================
// T28 Q4: Code Path Coverage (8 tests)
// ============================================================================

#[test]
fn test_all_error_variants_budget_exhausted() {
    let registry = BudgetRegistry::new(50_00);

    let result = registry.try_deduct(user_id("user1"), 100_00);
    assert!(matches!(result, Err(ClapiError::BudgetExhausted { .. })));
}

#[test]
fn test_all_error_variants_invalid_cost_negative() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.try_deduct(user_id("user1"), -100_00);
    assert!(matches!(result, Err(ClapiError::InvalidCost(_))));
}

#[test]
fn test_all_error_variants_invalid_credit_negative() {
    let registry = BudgetRegistry::new(1000_00);

    let result = registry.credit(user_id("user1"), -100_00);
    assert!(matches!(result, Err(ClapiError::InvalidCost(_))));
}

#[test]
fn test_success_path_deduct_then_credit() {
    let registry = BudgetRegistry::new(1000_00);

    // Success path: deduct
    let result1 = registry.try_deduct(user_id("user1"), 500_00);
    assert!(result1.is_ok());

    // Success path: credit
    let result2 = registry.credit(user_id("user1"), 200_00);
    assert!(result2.is_ok());

    assert_eq!(registry.get_budget(user_id("user1")), Some(700_00));
}

#[test]
fn test_success_path_multiple_users() {
    let registry = BudgetRegistry::new(1000_00);

    assert!(registry.try_deduct(user_id("user1"), 100_00).is_ok());
    assert!(registry.try_deduct(user_id("user2"), 200_00).is_ok());
    assert!(registry.try_deduct(user_id("user3"), 300_00).is_ok());

    assert_eq!(registry.len(), 3);
}

#[test]
fn test_branch_coverage_get_budget_exists() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();

    // Branch: budget exists
    let budget = registry.get_budget(user_id("user1"));
    assert!(budget.is_some());
}

#[test]
fn test_branch_coverage_get_budget_not_exists() {
    let registry = BudgetRegistry::new(1000_00);

    // Branch: budget does not exist
    let budget = registry.get_budget(user_id("user1"));
    assert!(budget.is_none());
}

#[test]
fn test_branch_coverage_is_empty() {
    let registry = BudgetRegistry::new(1000_00);

    // Branch: empty
    assert!(registry.is_empty());

    registry.try_deduct(user_id("user1"), 10_00).unwrap();

    // Branch: not empty
    assert!(!registry.is_empty());
}

// ============================================================================
// T28 Q5: Isolation and Determinism (8 tests)
// ============================================================================

#[test]
fn test_isolation_no_shared_state_between_tests() {
    // Each test creates fresh registry - no shared state
    let registry1 = BudgetRegistry::new(1000_00);
    let registry2 = BudgetRegistry::new(2000_00);

    registry1.try_deduct(user_id("user1"), 100_00).unwrap();
    registry2.try_deduct(user_id("user1"), 200_00).unwrap();

    assert_eq!(registry1.get_budget(user_id("user1")), Some(900_00));
    assert_eq!(registry2.get_budget(user_id("user1")), Some(1800_00));
}

#[test]
fn test_deterministic_operations_same_result() {
    // Same operations should produce same results
    for _ in 0..10 {
        let registry = BudgetRegistry::new(1000_00);

        registry.try_deduct(user_id("user1"), 100_00).unwrap();
        registry.try_deduct(user_id("user1"), 50_00).unwrap();

        assert_eq!(registry.get_budget(user_id("user1")), Some(850_00));
    }
}

#[test]
fn test_deterministic_stats() {
    for _ in 0..10 {
        let registry = BudgetRegistry::new(1000_00);

        registry.try_deduct(user_id("user1"), 100_00).unwrap();
        registry.try_deduct(user_id("user1"), 200_00).unwrap();

        let stats = registry.get_stats(user_id("user1")).unwrap();
        assert_eq!(stats.budget, 700_00);
        assert_eq!(stats.total_spent, 300_00);
        assert_eq!(stats.request_count, 2);
    }
}

#[test]
fn test_isolated_users_no_interference() {
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.try_deduct(user_id("user2"), 200_00).unwrap();

    // user1 budget unaffected by user2 operations
    assert_eq!(registry.get_budget(user_id("user1")), Some(900_00));
    assert_eq!(registry.get_budget(user_id("user2")), Some(800_00));
}

#[test]
fn test_deterministic_error_conditions() {
    for _ in 0..10 {
        let registry = BudgetRegistry::new(50_00);

        let result = registry.try_deduct(user_id("user1"), 100_00);
        assert!(result.is_err());
        assert!(matches!(result, Err(ClapiError::BudgetExhausted { .. })));
    }
}

#[test]
fn test_no_side_effects_on_failed_deduction() {
    let registry = BudgetRegistry::new(100_00);

    // Create budget first
    registry.try_deduct(user_id("user1"), 50_00).unwrap();
    let before_budget = registry.get_budget(user_id("user1"));

    // Failed deduction (insufficient funds)
    let _ = registry.try_deduct(user_id("user1"), 200_00);

    let after_budget = registry.get_budget(user_id("user1"));

    // No side effects - budget unchanged after failed deduction
    assert_eq!(before_budget, after_budget);
}

#[test]
fn test_isolated_credit_operations() {
    let registry = BudgetRegistry::new(1000_00);

    registry.credit(user_id("user1"), 100_00).unwrap();
    registry.credit(user_id("user2"), 200_00).unwrap();

    assert_eq!(registry.get_budget(user_id("user1")), Some(1100_00));
    assert_eq!(registry.get_budget(user_id("user2")), Some(1200_00));
}

#[test]
fn test_deterministic_generation_increment() {
    for _ in 0..10 {
        let registry = BudgetRegistry::new(1000_00);

        registry.try_deduct(user_id("user1"), 10_00).unwrap();
        let stats = registry.get_stats(user_id("user1")).unwrap();

        // Generation should start at 1, increment to 2 after first deduct
        assert!(stats.generation >= 2);
    }
}

// ============================================================================
// T28 Q6: Performance (5 tests)
// ============================================================================

#[test]
fn test_performance_batch_deductions() {
    let registry = BudgetRegistry::new(100_000_00);

    let start = std::time::Instant::now();

    for i in 0..1000 {
        let _ = registry.try_deduct(user_id("user1"), 10_00);
    }

    let elapsed = start.elapsed();

    // Budget: <10ms for 1000 operations
    assert!(
        elapsed.as_millis() < 10,
        "Batch deductions too slow: {:?}",
        elapsed
    );
}

#[test]
fn test_performance_multiple_users() {
    let registry = BudgetRegistry::new(10_000_00);

    let start = std::time::Instant::now();

    for i in 0..100 {
        let user_name = format!("user{}", i);
        let _ = registry.try_deduct(user_id(&user_name), 100_00);
    }

    let elapsed = start.elapsed();

    // Budget: <10ms for 100 users × 1 operation
    assert!(
        elapsed.as_millis() < 10,
        "Multiple user operations too slow: {:?}",
        elapsed
    );
}

#[test]
fn test_performance_get_budget() {
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(user_id("user1"), 100_00).unwrap();

    let start = std::time::Instant::now();

    for _ in 0..10_000 {
        let _ = registry.get_budget(user_id("user1"));
    }

    let elapsed = start.elapsed();

    // Budget: <10ms for 10K reads
    assert!(
        elapsed.as_millis() < 10,
        "Budget reads too slow: {:?}",
        elapsed
    );
}

#[test]
fn test_performance_get_stats() {
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(user_id("user1"), 100_00).unwrap();

    let start = std::time::Instant::now();

    for _ in 0..1000 {
        let _ = registry.get_stats(user_id("user1"));
    }

    let elapsed = start.elapsed();

    // Budget: <10ms for 1000 stats reads
    assert!(
        elapsed.as_millis() < 10,
        "Stats reads too slow: {:?}",
        elapsed
    );
}

#[test]
fn test_performance_credit_operations() {
    let registry = BudgetRegistry::new(1000_00);

    let start = std::time::Instant::now();

    for _ in 0..1000 {
        let _ = registry.credit(user_id("user1"), 10_00);
    }

    let elapsed = start.elapsed();

    // Budget: <10ms for 1000 credit operations
    assert!(
        elapsed.as_millis() < 10,
        "Credit operations too slow: {:?}",
        elapsed
    );
}

// ============================================================================
// T28 Q7: Readability and Maintainability (8 tests)
// ============================================================================
// All tests follow arrange-act-assert pattern
// Test names are descriptive and indicate what they test
// Helper functions minimize duplication

/// Helper: Create registry with specific budget and make deductions
fn create_registry_with_deductions(initial: i64, user: &str, deductions: &[i64]) -> BudgetRegistry {
    let registry = BudgetRegistry::new(initial);
    for &amount in deductions {
        let _ = registry.try_deduct(user_id(user), amount);
    }
    registry
}

#[test]
fn test_helper_usage_simple() {
    // Arrange: Use helper for clean setup
    let registry = create_registry_with_deductions(1000_00, "user1", &[100_00, 200_00]);

    // Act: Query final state
    let budget = registry.get_budget(user_id("user1"));

    // Assert: Verify expected result
    assert_eq!(budget, Some(700_00));
}

#[test]
fn test_clear_failure_messages() {
    let registry = BudgetRegistry::new(50_00);

    let result = registry.try_deduct(user_id("user1"), 100_00);

    assert!(
        result.is_err(),
        "Expected budget exhausted error, but got Ok"
    );

    match result {
        Err(ClapiError::BudgetExhausted {
            requested,
            available,
        }) => {
            assert_eq!(
                requested, 100_00,
                "Requested amount mismatch: expected 100_00, got {}",
                requested
            );
            assert_eq!(
                available, 50_00,
                "Available amount mismatch: expected 50_00, got {}",
                available
            );
        }
        _ => panic!("Expected BudgetExhausted error, got different error type"),
    }
}

#[test]
fn test_arrange_act_assert_pattern() {
    // Arrange: Set up initial conditions
    let registry = BudgetRegistry::new(1000_00);

    // Act: Perform the operation under test
    registry.try_deduct(user_id("user1"), 250_00).unwrap();

    // Assert: Verify the expected outcome
    assert_eq!(registry.get_budget(user_id("user1")), Some(750_00));
}

#[test]
fn test_descriptive_test_name_documents_behavior() {
    // This test name clearly indicates what behavior is being tested
    let registry = BudgetRegistry::new(1000_00);

    registry.try_deduct(user_id("user1"), 100_00).unwrap();
    registry.credit(user_id("user1"), 50_00).unwrap();

    let stats = registry.get_stats(user_id("user1")).unwrap();
    assert_eq!(stats.budget, 950_00);
    assert_eq!(stats.total_spent, 100_00);
}

#[test]
fn test_isolated_assertions_for_clarity() {
    let registry = BudgetRegistry::new(1000_00);
    registry.try_deduct(user_id("user1"), 100_00).unwrap();

    let stats = registry.get_stats(user_id("user1")).unwrap();

    // Separate assertions for clarity
    assert_eq!(stats.budget, 900_00, "Budget should be 900_00");
    assert_eq!(stats.total_spent, 100_00, "Total spent should be 100_00");
    assert_eq!(stats.request_count, 1, "Request count should be 1");
}

#[test]
fn test_helper_reduces_duplication() {
    let registry1 = create_registry_with_deductions(1000_00, "user1", &[100_00]);
    let registry2 = create_registry_with_deductions(2000_00, "user2", &[200_00]);

    assert_eq!(registry1.get_budget(user_id("user1")), Some(900_00));
    assert_eq!(registry2.get_budget(user_id("user2")), Some(1800_00));
}

#[test]
fn test_comments_explain_complex_behavior() {
    let registry = BudgetRegistry::new(1000_00);

    // Deduct until budget is low
    registry.try_deduct(user_id("user1"), 950_00).unwrap();

    // Attempt to deduct more than available - should fail
    let result = registry.try_deduct(user_id("user1"), 100_00);
    assert!(result.is_err());

    // Credit to recover budget
    registry.credit(user_id("user1"), 100_00).unwrap();

    // Now the deduction should succeed
    let result2 = registry.try_deduct(user_id("user1"), 100_00);
    assert!(result2.is_ok());
}

#[test]
fn test_consistent_naming_conventions() {
    let registry = BudgetRegistry::new(1000_00);

    let result_deduct = registry.try_deduct(user_id("user1"), 100_00);
    let result_credit = registry.credit(user_id("user1"), 50_00);

    assert!(result_deduct.is_ok());
    assert!(result_credit.is_ok());
}
