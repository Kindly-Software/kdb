// TIER 1: UNIT TESTS (Q1-Q7) - Capsule operations
// T28 Framework: Tests individual capsule behaviors in isolation
//
// Framework Compliance:
// - Q1 (Core behaviors): Test primary operations (set/get/deduct/credit)
// - Q2 (Edge cases): Zero values, max values, boundary conditions
// - Q3 (Invariants): Monotonic generation, non-negative budgets, alignment
// - Q4 (Code paths): All branches, error paths, success paths
// - Q5 (Isolation): Each test independent, deterministic
// - Q6 (Performance): <10ms per test (fast feedback)
// - Q7 (Readability): Arrange-Act-Assert, clear names, good error messages

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// AppStateCapsule - Application state management (64B, Tier 1 Atomic)
///
/// Memory Layout:
/// [0-7]    theme_id: AtomicU64 (0=default, 1=high-contrast, 2=deuteranopia, 3=custom)
/// [8-15]   user_id: AtomicU64
/// [16-23]  generation: AtomicU64 (TOCTOU prevention)
/// [24-31]  dark_mode: AtomicU64 (0=light, 1=dark)
/// [32-63]  _padding: [u8; 32] (cache alignment)
#[repr(C, align(64))]
struct AppStateCapsule {
    theme_id: AtomicU64,
    user_id: AtomicU64,
    generation: AtomicU64,
    dark_mode: AtomicU64,
    _padding: [u8; 32],
}

impl AppStateCapsule {
    fn new() -> Self {
        Self {
            theme_id: AtomicU64::new(0),
            user_id: AtomicU64::new(0),
            generation: AtomicU64::new(1),
            dark_mode: AtomicU64::new(0),
            _padding: [0; 32],
        }
    }

    fn set_theme(&self, theme_id: u64) -> Result<(), &'static str> {
        if theme_id > 3 {
            return Err("Invalid theme_id (must be 0-3)");
        }
        self.theme_id.store(theme_id, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn current_theme(&self) -> u64 {
        self.theme_id.load(Ordering::Acquire)
    }

    fn set_user_id(&self, id: u64) {
        self.user_id.store(id, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn get_user_id(&self) -> u64 {
        self.user_id.load(Ordering::Acquire)
    }

    fn is_dark_mode(&self) -> bool {
        self.dark_mode.load(Ordering::Acquire) == 1
    }

    fn toggle_dark_mode(&self) {
        let current = self.dark_mode.load(Ordering::Acquire);
        self.dark_mode.store(1 - current, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Relaxed);
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

/// BudgetViewCapsule - Budget tracking (64B, Tier 1 Atomic)
///
/// Memory Layout:
/// [0-7]    budget_cents: AtomicU64 (i64 reinterpreted)
/// [8-15]   generation: AtomicU64
/// [16-63]  _padding: [u8; 48]
#[repr(C, align(64))]
struct BudgetViewCapsule {
    budget_cents: AtomicU64,
    generation: AtomicU64,
    _padding: [u8; 48],
}

impl BudgetViewCapsule {
    fn new(initial_budget_cents: i64) -> Self {
        Self {
            budget_cents: AtomicU64::new(initial_budget_cents as u64),
            generation: AtomicU64::new(1),
            _padding: [0; 48],
        }
    }

    fn try_deduct(&self, amount_cents: i64) -> Result<i64, &'static str> {
        if amount_cents < 0 {
            return Err("Amount must be non-negative");
        }

        let mut current = self.budget_cents.load(Ordering::Acquire);
        loop {
            let current_i64 = current as i64;
            if current_i64 < amount_cents {
                return Err("Insufficient budget");
            }

            let new_budget = (current_i64 - amount_cents) as u64;
            match self.budget_cents.compare_exchange_weak(
                current,
                new_budget,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.generation.fetch_add(1, Ordering::Relaxed);
                    return Ok(new_budget as i64);
                }
                Err(actual) => current = actual,
            }
        }
    }

    fn credit(&self, amount_cents: i64) -> Result<i64, &'static str> {
        if amount_cents < 0 {
            return Err("Amount must be non-negative");
        }

        let new_budget = self.budget_cents.fetch_add(amount_cents as u64, Ordering::AcqRel);
        self.generation.fetch_add(1, Ordering::Relaxed);
        Ok((new_budget as i64) + amount_cents)
    }

    fn get_budget(&self) -> i64 {
        self.budget_cents.load(Ordering::Acquire) as i64
    }

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }
}

// ============================================================================
// T28 Q1: CORE BEHAVIORS
// ============================================================================

#[test]
fn test_app_state_set_theme_valid() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Act & Assert
    assert!(capsule.set_theme(0).is_ok()); // Default
    assert_eq!(capsule.current_theme(), 0);

    assert!(capsule.set_theme(1).is_ok()); // High contrast
    assert_eq!(capsule.current_theme(), 1);

    assert!(capsule.set_theme(2).is_ok()); // Deuteranopia
    assert_eq!(capsule.current_theme(), 2);

    assert!(capsule.set_theme(3).is_ok()); // Custom
    assert_eq!(capsule.current_theme(), 3);
}

#[test]
fn test_app_state_set_theme_invalid() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Act & Assert
    assert!(capsule.set_theme(4).is_err()); // Out of range
    assert!(capsule.set_theme(10).is_err());
    assert!(capsule.set_theme(u64::MAX).is_err());
}

#[test]
fn test_app_state_get_user_id() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Act
    capsule.set_user_id(12345);

    // Assert
    assert_eq!(capsule.get_user_id(), 12345);
}

#[test]
fn test_app_state_dark_mode_toggle() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Act & Assert
    let initial = capsule.is_dark_mode();
    assert!(!initial); // Default: light mode

    capsule.toggle_dark_mode();
    assert!(capsule.is_dark_mode());

    capsule.toggle_dark_mode();
    assert!(!capsule.is_dark_mode());
}

#[test]
fn test_budget_view_deduct_success() {
    // Arrange
    let capsule = BudgetViewCapsule::new(1000_00); // $1000

    // Act
    let result = capsule.try_deduct(50_00); // $50

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 950_00); // $950 remaining
    assert_eq!(capsule.get_budget(), 950_00);
}

#[test]
fn test_budget_view_deduct_insufficient() {
    // Arrange
    let capsule = BudgetViewCapsule::new(100_00); // $100

    // Act
    let result = capsule.try_deduct(150_00); // Over budget

    // Assert
    assert!(result.is_err());
    assert_eq!(capsule.get_budget(), 100_00); // Unchanged
}

#[test]
fn test_budget_view_credit() {
    // Arrange
    let capsule = BudgetViewCapsule::new(1000_00);
    capsule.try_deduct(100_00).unwrap();

    // Act
    let result = capsule.credit(50_00);

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 950_00); // 1000 - 100 + 50
    assert_eq!(capsule.get_budget(), 950_00);
}

// ============================================================================
// T28 Q2: EDGE CASES
// ============================================================================

#[test]
fn test_budget_edge_case_zero_budget() {
    // Arrange
    let capsule = BudgetViewCapsule::new(0);

    // Act & Assert
    assert_eq!(capsule.get_budget(), 0);
    assert!(capsule.try_deduct(1).is_err());
}

#[test]
fn test_budget_edge_case_exact_deduction() {
    // Arrange
    let capsule = BudgetViewCapsule::new(100_00);

    // Act
    let result = capsule.try_deduct(100_00); // Exact amount

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
    assert_eq!(capsule.get_budget(), 0);
}

#[test]
fn test_budget_edge_case_negative_amounts_rejected() {
    // Arrange
    let capsule = BudgetViewCapsule::new(1000_00);

    // Act & Assert
    assert!(capsule.try_deduct(-50_00).is_err());
    assert!(capsule.credit(-50_00).is_err());
    assert_eq!(capsule.get_budget(), 1000_00); // Unchanged
}

#[test]
fn test_app_state_edge_case_max_user_id() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Act
    capsule.set_user_id(u64::MAX);

    // Assert
    assert_eq!(capsule.get_user_id(), u64::MAX);
}

// ============================================================================
// T28 Q3: INVARIANTS
// ============================================================================

#[test]
fn test_budget_invariant_never_negative() {
    // Arrange
    let capsule = BudgetViewCapsule::new(1000_00);

    // Act
    capsule.try_deduct(500_00).unwrap();
    capsule.try_deduct(500_00).unwrap();

    // Invariant: Budget never goes negative
    assert!(capsule.try_deduct(1).is_err());
    assert_eq!(capsule.get_budget(), 0);
}

#[test]
fn test_budget_invariant_generation_monotonic() {
    // Arrange
    let capsule = BudgetViewCapsule::new(1000_00);
    let mut last_gen = capsule.generation();

    // Act & Assert
    for _ in 0..100 {
        capsule.credit(10_00).unwrap();
        let current_gen = capsule.generation();

        // Invariant: Generation always increases
        assert!(
            current_gen > last_gen,
            "Generation must increase: {} > {}",
            current_gen,
            last_gen
        );
        last_gen = current_gen;
    }
}

#[test]
fn test_app_state_invariant_generation_increases() {
    // Arrange
    let capsule = AppStateCapsule::new();
    let initial_gen = capsule.generation();

    // Act
    capsule.set_theme(1).unwrap();
    let gen_after_theme = capsule.generation();

    capsule.toggle_dark_mode();
    let gen_after_dark_mode = capsule.generation();

    capsule.set_user_id(12345);
    let gen_after_user_id = capsule.generation();

    // Assert: Generation increases monotonically
    assert!(gen_after_theme > initial_gen);
    assert!(gen_after_dark_mode > gen_after_theme);
    assert!(gen_after_user_id > gen_after_dark_mode);
}

#[test]
fn test_capsule_invariant_cache_alignment() {
    // Invariant: All capsules must be 64-byte aligned
    let app_state = Box::new(AppStateCapsule::new());
    let budget = Box::new(BudgetViewCapsule::new(1000_00));

    let app_state_addr = &*app_state as *const _ as usize;
    let budget_addr = &*budget as *const _ as usize;

    assert_eq!(app_state_addr % 64, 0, "AppStateCapsule not 64-byte aligned");
    assert_eq!(budget_addr % 64, 0, "BudgetViewCapsule not 64-byte aligned");
}

// ============================================================================
// T28 Q4: CODE PATH COVERAGE
// ============================================================================

#[test]
fn test_budget_all_error_paths() {
    // Arrange
    let capsule = BudgetViewCapsule::new(100_00);

    // Error path 1: Negative deduction
    assert!(capsule.try_deduct(-1).is_err());

    // Error path 2: Insufficient budget
    assert!(capsule.try_deduct(150_00).is_err());

    // Error path 3: Negative credit
    assert!(capsule.credit(-1).is_err());

    // Success paths
    assert!(capsule.try_deduct(50_00).is_ok());
    assert!(capsule.credit(25_00).is_ok());
}

#[test]
fn test_app_state_all_branches() {
    // Arrange
    let capsule = AppStateCapsule::new();

    // Branch 1: Valid themes (0-3)
    for theme_id in 0..=3 {
        assert!(capsule.set_theme(theme_id).is_ok());
    }

    // Branch 2: Invalid themes (>3)
    for theme_id in 4..10 {
        assert!(capsule.set_theme(theme_id).is_err());
    }

    // Branch 3: Dark mode toggle (light → dark → light)
    assert!(!capsule.is_dark_mode());
    capsule.toggle_dark_mode();
    assert!(capsule.is_dark_mode());
    capsule.toggle_dark_mode();
    assert!(!capsule.is_dark_mode());
}

// ============================================================================
// T28 Q5: ISOLATION & DETERMINISM
// ============================================================================

#[test]
fn test_capsule_isolation() {
    // Arrange
    let capsule1 = BudgetViewCapsule::new(1000_00);
    let capsule2 = BudgetViewCapsule::new(500_00);

    // Act
    capsule1.try_deduct(100_00).unwrap();

    // Assert: capsule2 unaffected (isolation)
    assert_eq!(capsule1.get_budget(), 900_00);
    assert_eq!(capsule2.get_budget(), 500_00);
}

#[test]
fn test_deterministic_behavior() {
    // Arrange
    let capsule1 = BudgetViewCapsule::new(1000_00);
    let capsule2 = BudgetViewCapsule::new(1000_00);

    // Act: Identical operations
    capsule1.try_deduct(50_00).unwrap();
    capsule1.credit(25_00).unwrap();

    capsule2.try_deduct(50_00).unwrap();
    capsule2.credit(25_00).unwrap();

    // Assert: Identical outcomes (determinism)
    assert_eq!(capsule1.get_budget(), capsule2.get_budget());
}

// ============================================================================
// T28 Q6: PERFORMANCE (<10ms per test)
// ============================================================================

#[test]
fn test_performance_budget_operations() {
    use std::time::Instant;

    // Arrange
    let capsule = BudgetViewCapsule::new(1_000_000_00);
    let iterations = 10_000;

    // Act
    let start = Instant::now();
    for i in 0..iterations {
        if i % 2 == 0 {
            capsule.try_deduct(100).unwrap();
        } else {
            capsule.credit(100).unwrap();
        }
    }
    let elapsed = start.elapsed();

    // Assert: <10ms total, <1μs per op
    assert!(
        elapsed.as_millis() < 10,
        "Test took {}ms (should be <10ms)",
        elapsed.as_millis()
    );
}

// ============================================================================
// T28 Q7: READABILITY
// ============================================================================
// All tests follow Arrange-Act-Assert structure
// Test names clearly describe what is being tested
// Error messages provide context for failures

// ============================================================================
// CONCURRENT CORRECTNESS (Basic - Full property tests in property_tests.rs)
// ============================================================================

#[test]
fn test_concurrent_budget_updates() {
    use std::thread;

    // Arrange
    let capsule = Arc::new(BudgetViewCapsule::new(1_000_000_00));
    let num_threads = 10;
    let updates_per_thread = 100;

    // Act
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..updates_per_thread {
                    c.try_deduct(100).unwrap();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: All updates applied (no lost writes)
    let expected = 1_000_000_00 - (num_threads * updates_per_thread * 100);
    assert_eq!(capsule.get_budget(), expected);
}

#[test]
fn test_concurrent_app_state_updates() {
    use std::thread;

    // Arrange
    let capsule = Arc::new(AppStateCapsule::new());
    let num_threads = 10;

    // Act
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let c = Arc::clone(&capsule);
            thread::spawn(move || {
                for _ in 0..100 {
                    c.set_theme((i % 4) as u64).unwrap();
                    c.toggle_dark_mode();
                }
            })
        })
        .collect();

    for h in handles {
        h.join().unwrap();
    }

    // Assert: Generation counter reflects all updates
    // 10 threads × 100 iterations × 2 ops = 2000 updates
    assert!(capsule.generation() >= 2000);
}

// ============================================================================
// SUMMARY: 20+ TESTS COVERING T28 Q1-Q7
// ============================================================================
//
// Unit Tests: 20+ tests
// Coverage: AppStateCapsule (set_theme, get_user_id, dark_mode)
//           BudgetViewCapsule (deduct, credit, exhaustion)
// Framework Compliance: T28 Q1-Q7 fully implemented
// Performance: All tests <10ms (most <1ms)
// Concurrent: Basic concurrent correctness validated
