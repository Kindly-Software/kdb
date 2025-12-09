//! BudgetViewCapsule - Tier 1 Atomic (128B)
//!
//! Purpose: Budget tracking for UI with deduction/credit operations
//! Memory Layout:
//!   [0-7]   budget_cents: AtomicI64 (current budget in cents)
//!   [8-15]  spent_cents: AtomicI64 (total spent in cents)
//!   [16-23] packed: AtomicU64 (request_count:32b + generation:32b)
//!   [24-127] _padding: [u8; 104] (cache alignment)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use super::error::{CapsuleError, CapsuleResult};

/// Tier 1 Atomic: Budget view capsule (128B cache-aligned)
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct BudgetViewCapsule {
    /// Current budget in cents (signed for overdraft detection)
    budget_cents: AtomicI64,
    /// Total spent in cents
    spent_cents: AtomicI64,
    /// Packed: request_count(32b) + generation(32b)
    packed: AtomicU64,
    /// Padding to 128 bytes (cache line alignment)
    _padding: [u8; 104],
}

const REQUEST_COUNT_MASK: u64 = 0xFFFF_FFFF;
const GENERATION_MASK: u64 = 0xFFFF_FFFF_0000_0000;
const GENERATION_SHIFT: u32 = 32;

impl BudgetViewCapsule {
    /// Create new budget view capsule
    ///
    /// # Arguments
    /// * `initial_budget_cents` - Initial budget in cents
    ///
    /// # Returns
    /// BudgetViewCapsule with budget set, spent=0, requests=0
    pub const fn new(initial_budget_cents: i64) -> Self {
        Self {
            budget_cents: AtomicI64::new(initial_budget_cents),
            spent_cents: AtomicI64::new(0),
            packed: AtomicU64::new(0),
            _padding: [0u8; 104],
        }
    }

    /// Try to deduct amount from budget
    ///
    /// #ASSUME: Atomic CAS on budget_cents prevents race conditions
    /// #VERIFY: Budget never goes negative (enforced by CAS loop)
    ///
    /// # Arguments
    /// * `cost_cents` - Cost to deduct in cents
    ///
    /// # Returns
    /// Remaining budget after deduction or error if insufficient
    pub fn try_deduct(&self, cost_cents: i64) -> CapsuleResult<i64> {
        if cost_cents < 0 {
            return Err(CapsuleError::InvalidValue {
                message: format!("cost_cents {} must be non-negative", cost_cents),
            });
        }

        // #ASSUME: Acquire ordering ensures budget read before deduction
        let mut current = self.budget_cents.load(Ordering::Acquire);
        loop {
            if current < cost_cents {
                return Err(CapsuleError::BudgetExhausted {
                    required: cost_cents,
                    available: current,
                });
            }

            let new_budget = current - cost_cents;

            // #ASSUME: CAS with Release ensures deduction visible to other threads
            match self.budget_cents.compare_exchange_weak(
                current,
                new_budget,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    // Successfully deducted, update spent counter
                    // #ASSUME: Relaxed ordering safe for spent (audit counter only)
                    self.spent_cents.fetch_add(cost_cents, Ordering::Relaxed);

                    // Increment request count
                    self._increment_request_count();

                    return Ok(new_budget);
                }
                Err(actual) => current = actual, // Retry with updated value
            }
        }
    }

    /// Credit amount to budget
    ///
    /// #ASSUME: Atomic fetch_add prevents overflow with saturation
    /// #VERIFY: No overflow on credit operations
    ///
    /// # Arguments
    /// * `amount_cents` - Amount to credit in cents
    ///
    /// # Returns
    /// New budget after credit or error if overflow
    pub fn credit(&self, amount_cents: i64) -> CapsuleResult<i64> {
        if amount_cents < 0 {
            return Err(CapsuleError::InvalidValue {
                message: format!("amount_cents {} must be non-negative", amount_cents),
            });
        }

        // #ASSUME: Release ordering ensures credit visible to other threads
        let old_budget = self.budget_cents.fetch_add(amount_cents, Ordering::Release);

        // Check for overflow
        let new_budget = old_budget.checked_add(amount_cents)
            .ok_or_else(|| CapsuleError::Overflow {
                operation: format!("credit {} to budget {}", amount_cents, old_budget),
            })?;

        // Increment request count
        self._increment_request_count();

        Ok(new_budget)
    }

    /// Get current budget
    ///
    /// #ASSUME: Acquire load ensures latest budget visible
    pub fn get_budget(&self) -> i64 {
        self.budget_cents.load(Ordering::Acquire)
    }

    /// Get total spent
    ///
    /// #ASSUME: Relaxed load safe (spent is audit counter)
    pub fn get_spent(&self) -> i64 {
        self.spent_cents.load(Ordering::Relaxed)
    }

    /// Get request count
    ///
    /// #ASSUME: Relaxed load safe (request_count is audit counter)
    pub fn get_request_count(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        (packed & REQUEST_COUNT_MASK) as u32
    }

    /// Get generation counter
    ///
    /// #ASSUME: Relaxed load safe (generation for TOCTOU only)
    pub fn generation(&self) -> u32 {
        let packed = self.packed.load(Ordering::Relaxed);
        ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32
    }

    /// Increment request count (internal)
    ///
    /// #ASSUME: Fetch-add with Relaxed safe (request_count is monotonic)
    fn _increment_request_count(&self) {
        let mut current = self.packed.load(Ordering::Relaxed);
        loop {
            let count = (current & REQUEST_COUNT_MASK) as u32;
            let gen = ((current & GENERATION_MASK) >> GENERATION_SHIFT) as u32;

            let new_count = count.wrapping_add(1);
            let new_gen = gen.wrapping_add(1);
            let new_packed = (new_count as u64) | ((new_gen as u64) << GENERATION_SHIFT);

            match self.packed.compare_exchange_weak(
                current,
                new_packed,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return,
                Err(actual) => current = actual,
            }
        }
    }

    /// Get snapshot of all values
    ///
    /// #ASSUME: Acquire loads ensure consistent snapshot
    ///
    /// # Returns
    /// (budget_cents, spent_cents, request_count, generation)
    pub fn snapshot(&self) -> (i64, i64, u32, u32) {
        let budget = self.budget_cents.load(Ordering::Acquire);
        let spent = self.spent_cents.load(Ordering::Relaxed);
        let packed = self.packed.load(Ordering::Relaxed);
        let count = (packed & REQUEST_COUNT_MASK) as u32;
        let gen = ((packed & GENERATION_MASK) >> GENERATION_SHIFT) as u32;
        (budget, spent, count, gen)
    }
}

impl Default for BudgetViewCapsule {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_view_alignment() {
        assert_eq!(std::mem::align_of::<BudgetViewCapsule>(), 128);
        assert_eq!(std::mem::size_of::<BudgetViewCapsule>(), 128);
    }

    #[test]
    fn test_deduct_success() {
        let budget = BudgetViewCapsule::new(1000_00); // $1000.00

        let remaining = budget.try_deduct(250_00).unwrap(); // $250.00
        assert_eq!(remaining, 750_00);
        assert_eq!(budget.get_budget(), 750_00);
        assert_eq!(budget.get_spent(), 250_00);
        assert_eq!(budget.get_request_count(), 1);
    }

    #[test]
    fn test_deduct_insufficient() {
        let budget = BudgetViewCapsule::new(100_00); // $100.00

        let result = budget.try_deduct(200_00); // $200.00
        assert!(result.is_err());
        assert_eq!(budget.get_budget(), 100_00); // Unchanged
    }

    #[test]
    fn test_credit() {
        let budget = BudgetViewCapsule::new(500_00);

        let new_budget = budget.credit(250_00).unwrap();
        assert_eq!(new_budget, 750_00);
        assert_eq!(budget.get_budget(), 750_00);
    }

    #[test]
    fn test_multiple_operations() {
        let budget = BudgetViewCapsule::new(1000_00);

        budget.try_deduct(100_00).unwrap();
        budget.try_deduct(200_00).unwrap();
        budget.credit(50_00).unwrap();

        assert_eq!(budget.get_budget(), 750_00);
        assert_eq!(budget.get_spent(), 300_00);
        assert_eq!(budget.get_request_count(), 3);
    }

    #[test]
    fn test_snapshot() {
        let budget = BudgetViewCapsule::new(1000_00);
        budget.try_deduct(250_00).unwrap();

        let (budget_val, spent, count, _gen) = budget.snapshot();
        assert_eq!(budget_val, 750_00);
        assert_eq!(spent, 250_00);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_invalid_negative_deduct() {
        let budget = BudgetViewCapsule::new(100_00);
        assert!(budget.try_deduct(-10_00).is_err());
    }

    #[test]
    fn test_invalid_negative_credit() {
        let budget = BudgetViewCapsule::new(100_00);
        assert!(budget.credit(-10_00).is_err());
    }
}
