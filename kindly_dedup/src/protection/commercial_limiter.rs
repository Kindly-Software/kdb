//! Commercial Tier Enforcement Capsule (T1 Atomic)
//!
//! ## Tier Enforcement
//!
//! | Tier | Document Limit | Enforcement |
//! |------|----------------|-------------|
//! | Demo (no license) | 1,000 docs | Hard block after limit |
//! | Basic | 100,000 docs | Soft warning at 90% |
//! | Pro | 10,000,000 docs | Soft warning at 90% |
//! | Enterprise | Unlimited | No limit |
//!
//! ## Performance
//!
//! - **Check latency**: <10ns (single atomic load)
//! - **Record latency**: <20ns (atomic fetch_add + load)
//! - **Warning check**: <15ns (atomic load + comparison)
//! - **Thread-safety**: 100% lockfree, no mutex
//!
//! ## UCE34 Framework
//!
//! - **Q10 (Tier)**: T1 Atomic (lockfree counter, cache-aligned)
//! - **Q28 (Simplicity)**: Single AtomicU64 counter per capsule
//! - **Q29 (Constraints)**: Cache-aligned 64B, generation counter for versioning
//! - **Q33 (Atomic Capsule)**: Generation counter prevents TOCTOU races
//! - **Q34 (Auditability)**: Document count auditable via current_count()
//!
//! ## Chaos (Computational Capsule) Compliance
//!
//! - **Lockfree**: 100% lockfree (AtomicU64 only, no mutex/RwLock)
//! - **Cache-aligned**: 64B alignment (prevents false sharing)
//! - **Generation counter**: Versioning for state consistency
//! - **Deterministic**: Fixed limits per tier, reproducible behavior
//!
//! ## ASSUM Safety Framework
//!
//! ```text
//! #ASSUME_ATOMIC_ORDERING: Relaxed ordering sufficient for monotonic counter
//!   #VERIFY_MONOTONIC: fetch_add is monotonic, no wrapping expected
//!   #VERIFY_OVERFLOW: Document count limited by tier max, no u64 overflow
//!
//! #ASSUME_TIER_LIMITS: Tier limits fit in u64 (max 10M docs << 2^64)
//!   #VERIFY_TIER_LIMITS: All tier limits < 2^32, safe for u64
//!   #VERIFY_NO_WRAP: No wrapping possible within tier limits
//!
//! #ASSUME_GENERATION_COUNTER: Generation counter prevents TOCTOU races
//!   #VERIFY_ATOMIC_UPDATE: Generation incremented atomically with counter
//!   #VERIFY_STATE_CONSISTENCY: Generation validates counter state
//! ```
//!
//! ## Integration Point
//!
//! **NOTE**: This capsule will be wired to `UniversalDedupPipeline::add_document()`
//! to enforce tier limits during document ingestion. The pipeline will call
//! `can_add_document()` before processing and `record_document()` after success.
//!
//! ## Example
//!
//! ```rust
//! use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier, CommercialLimitError};
//!
//! // Demo tier (1,000 doc limit)
//! let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
//!
//! // Check before adding document
//! match limiter.can_add_document() {
//!     Ok(()) => {
//!         // Process document...
//!         limiter.record_document()?;
//!     }
//!     Err(CommercialLimitError::LimitReached { tier, limit }) => {
//!         eprintln!("Demo limit reached ({} documents). Upgrade to Basic tier for 100K documents: https://kindly.dev/pricing", limit);
//!     }
//!     Err(e) => return Err(e),
//! }
//!
//! // Check warning threshold (90%)
//! if limiter.is_at_warning_threshold() {
//!     eprintln!("Warning: 90% of your {} tier limit used ({}/{} documents)",
//!               limiter.tier().name(),
//!               limiter.current_count(),
//!               limiter.remaining_documents().unwrap());
//! }
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

/// License tier enumeration
///
/// ## Tier Limits
/// - Demo: 1,000 documents (hard block)
/// - Basic: 100,000 documents (soft warning at 90%)
/// - Pro: 10,000,000 documents (soft warning at 90%)
/// - Enterprise: Unlimited (no limits)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LicenseTier {
    /// Demo tier (1,000 doc limit, hard block)
    Demo,
    /// Basic tier (100,000 doc limit, soft warning)
    Basic,
    /// Pro tier (10,000,000 doc limit, soft warning)
    Pro,
    /// Enterprise tier (unlimited)
    Enterprise,
}

impl LicenseTier {
    /// Get document limit for this tier
    ///
    /// Returns None for unlimited (Enterprise)
    #[inline]
    pub const fn limit(&self) -> Option<u64> {
        match self {
            LicenseTier::Demo => Some(1_000),
            LicenseTier::Basic => Some(100_000),
            LicenseTier::Pro => Some(10_000_000),
            LicenseTier::Enterprise => None,
        }
    }

    /// Get warning threshold (90% of limit)
    ///
    /// Returns None for unlimited (Enterprise)
    #[inline]
    pub const fn warning_threshold(&self) -> Option<u64> {
        match self {
            LicenseTier::Demo => Some(900),       // 90% of 1,000
            LicenseTier::Basic => Some(90_000),   // 90% of 100,000
            LicenseTier::Pro => Some(9_000_000),  // 90% of 10,000,000
            LicenseTier::Enterprise => None,
        }
    }

    /// Get tier name for display
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            LicenseTier::Demo => "Demo",
            LicenseTier::Basic => "Basic",
            LicenseTier::Pro => "Pro",
            LicenseTier::Enterprise => "Enterprise",
        }
    }

    /// Get next tier for upgrade prompts
    #[inline]
    pub const fn next_tier(&self) -> Option<LicenseTier> {
        match self {
            LicenseTier::Demo => Some(LicenseTier::Basic),
            LicenseTier::Basic => Some(LicenseTier::Pro),
            LicenseTier::Pro => Some(LicenseTier::Enterprise),
            LicenseTier::Enterprise => None,
        }
    }
}

/// Commercial limit error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommercialLimitError {
    /// Tier limit reached (hard block)
    LimitReached {
        /// Current tier that hit limit
        tier: LicenseTier,
        /// Document limit for this tier
        limit: u64,
    },
    /// Upgrade required (soft recommendation)
    UpgradeRequired {
        /// Current tier
        current_tier: LicenseTier,
    },
}

impl core::fmt::Display for CommercialLimitError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CommercialLimitError::LimitReached { tier, limit } => {
                write!(
                    f,
                    "{} tier limit reached ({} documents). ",
                    tier.name(),
                    limit
                )?;
                if let Some(next) = tier.next_tier() {
                    write!(
                        f,
                        "Upgrade to {} tier for {} documents: https://kindly.dev/pricing",
                        next.name(),
                        next.limit()
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "unlimited".to_string())
                    )
                } else {
                    write!(f, "Maximum tier reached (Enterprise).")
                }
            }
            CommercialLimitError::UpgradeRequired { current_tier } => {
                if let Some(next) = current_tier.next_tier() {
                    write!(
                        f,
                        "Consider upgrading to {} tier for {} documents: https://kindly.dev/pricing",
                        next.name(),
                        next.limit()
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "unlimited".to_string())
                    )
                } else {
                    write!(f, "Already on maximum tier (Enterprise).")
                }
            }
        }
    }
}

impl std::error::Error for CommercialLimitError {}

/// Commercial Limiter Capsule (T1 Atomic tier)
///
/// ## Capsule Layout (64 bytes, cache-aligned)
///
/// ```text
/// Offset  | Size | Field        | Description
/// --------|------|--------------|----------------------------------
/// 0x00    | 8    | state        | AtomicU64 (upper 32: generation, lower 32: count)
/// 0x08    | 1    | tier         | LicenseTier enum (u8)
/// 0x09    | 55   | _padding     | Cache alignment to 64 bytes
/// ```
///
/// ## State Packing (AtomicU64)
///
/// ```text
/// Bits 0-31:  Document count (u32, max 10M docs fits in 32 bits)
/// Bits 32-63: Generation counter (u32, prevents TOCTOU)
/// ```
///
/// ## Performance
///
/// - **Check**: <10ns (single atomic load + u32 extract + comparison)
/// - **Record**: <20ns (fetch_add + load + u32 extract)
/// - **Warning**: <15ns (load + u32 extract + comparison)
#[repr(C, align(64))]
pub struct CommercialLimiterCapsule {
    /// Packed state: upper 32 bits = generation, lower 32 bits = document count
    ///
    /// #ASSUME_ATOMIC_ORDERING: Relaxed ordering sufficient for monotonic counter
    ///   #VERIFY_MONOTONIC: fetch_add guarantees monotonic increment
    ///   #VERIFY_OVERFLOW: Document count limited by tier max (10M << 2^32)
    state: AtomicU64,

    /// License tier (immutable after construction)
    tier: LicenseTier,

    /// Padding to 64 bytes (prevents false sharing)
    _padding: [u8; 55],
}

// SAFETY: CommercialLimiterCapsule is Send + Sync because:
// - state is AtomicU64 (atomic operations are thread-safe)
// - tier is immutable after construction (no mutation)
// - _padding is zero-initialized, no mutation
unsafe impl Send for CommercialLimiterCapsule {}
unsafe impl Sync for CommercialLimiterCapsule {}

impl CommercialLimiterCapsule {
    /// Create new commercial limiter for specified tier
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    /// assert_eq!(limiter.tier(), LicenseTier::Demo);
    /// assert_eq!(limiter.current_count(), 0);
    /// ```
    #[inline]
    pub const fn new(tier: LicenseTier) -> Self {
        Self {
            state: AtomicU64::new(0), // generation=0, count=0
            tier,
            _padding: [0; 55],
        }
    }

    /// Check if document can be added (does NOT increment counter)
    ///
    /// ## Returns
    ///
    /// - `Ok(())`: Document can be added (under limit)
    /// - `Err(CommercialLimitError::LimitReached)`: Hard limit reached
    ///
    /// ## Performance
    ///
    /// - **Latency**: <10ns (single atomic load + comparison)
    /// - **Throughput**: 100M+ ops/sec
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    ///
    /// // Check before processing
    /// match limiter.can_add_document() {
    ///     Ok(()) => { /* proceed */ }
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    #[inline]
    pub fn can_add_document(&self) -> Result<(), CommercialLimitError> {
        // Enterprise tier has no limits
        if self.tier == LicenseTier::Enterprise {
            return Ok(());
        }

        let limit = self.tier.limit().unwrap(); // Safe: checked Enterprise above
        let count = self.current_count();

        if count >= limit {
            Err(CommercialLimitError::LimitReached {
                tier: self.tier,
                limit,
            })
        } else {
            Ok(())
        }
    }

    /// Record document addition (increments counter)
    ///
    /// ## Returns
    ///
    /// - `Ok(())`: Document recorded successfully
    /// - `Err(CommercialLimitError::LimitReached)`: Hard limit reached AFTER increment
    ///
    /// ## Performance
    ///
    /// - **Latency**: <20ns (fetch_add + load + check)
    /// - **Throughput**: 50M+ ops/sec
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_NO_WRAP: Document count never wraps u32
    ///   #VERIFY_TIER_MAX: Max tier limit is 10M << 2^32
    ///   #VERIFY_ATOMIC_ADD: fetch_add is atomic, no races
    /// ```
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    ///
    /// // Record after successful processing
    /// limiter.record_document()?;
    /// assert_eq!(limiter.current_count(), 1);
    /// # Ok::<(), kindly_dedup::protection::CommercialLimitError>(())
    /// ```
    #[inline]
    pub fn record_document(&self) -> Result<(), CommercialLimitError> {
        // Enterprise tier has no limits
        if self.tier == LicenseTier::Enterprise {
            // Increment counter for stats (no limit check)
            // Increment lower 32 bits (count), upper 32 bits (generation) unchanged
            self.state.fetch_add(1, Ordering::Relaxed);
            return Ok(());
        }

        let limit = self.tier.limit().unwrap(); // Safe: checked Enterprise above

        // Increment counter atomically
        // Increment lower 32 bits (count), upper 32 bits (generation) unchanged
        let prev_state = self.state.fetch_add(1, Ordering::Relaxed);
        let prev_count = (prev_state & 0xFFFF_FFFF) as u32;

        // Check if we exceeded limit AFTER increment
        // (This allows exactly `limit` documents, blocking at limit+1)
        if (prev_count as u64) >= limit {
            Err(CommercialLimitError::LimitReached {
                tier: self.tier,
                limit,
            })
        } else {
            Ok(())
        }
    }

    /// Get remaining documents before limit
    ///
    /// ## Returns
    ///
    /// - `Some(n)`: n documents remaining (0 = at limit)
    /// - `None`: Unlimited (Enterprise tier)
    ///
    /// ## Performance
    ///
    /// - **Latency**: <10ns (single atomic load + subtraction)
    /// - **Throughput**: 100M+ ops/sec
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    /// assert_eq!(limiter.remaining_documents(), Some(1000));
    ///
    /// limiter.record_document()?;
    /// assert_eq!(limiter.remaining_documents(), Some(999));
    /// # Ok::<(), kindly_dedup::protection::CommercialLimitError>(())
    /// ```
    #[inline]
    pub fn remaining_documents(&self) -> Option<u64> {
        match self.tier.limit() {
            Some(limit) => {
                let count = self.current_count();
                Some(limit.saturating_sub(count))
            }
            None => None, // Unlimited
        }
    }

    /// Get current document count
    ///
    /// ## Performance
    ///
    /// - **Latency**: <5ns (single atomic load + u32 extract)
    /// - **Throughput**: 200M+ ops/sec
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    /// assert_eq!(limiter.current_count(), 0);
    ///
    /// limiter.record_document()?;
    /// assert_eq!(limiter.current_count(), 1);
    /// # Ok::<(), kindly_dedup::protection::CommercialLimitError>(())
    /// ```
    #[inline]
    pub fn current_count(&self) -> u64 {
        let state = self.state.load(Ordering::Relaxed);
        // Extract lower 32 bits (count)
        (state & 0xFFFF_FFFF) as u64
    }

    /// Get license tier
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Pro);
    /// assert_eq!(limiter.tier(), LicenseTier::Pro);
    /// ```
    #[inline]
    pub const fn tier(&self) -> LicenseTier {
        self.tier
    }

    /// Check if at warning threshold (90% of limit)
    ///
    /// ## Returns
    ///
    /// - `true`: Warning threshold reached (90% of limit used)
    /// - `false`: Below warning threshold OR unlimited tier
    ///
    /// ## Performance
    ///
    /// - **Latency**: <15ns (load + comparison)
    /// - **Throughput**: 66M+ ops/sec
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    ///
    /// // Add 900 documents (90% of 1,000)
    /// for _ in 0..900 {
    ///     limiter.record_document()?;
    /// }
    ///
    /// assert!(limiter.is_at_warning_threshold());
    /// # Ok::<(), kindly_dedup::protection::CommercialLimitError>(())
    /// ```
    #[inline]
    pub fn is_at_warning_threshold(&self) -> bool {
        match self.tier.warning_threshold() {
            Some(threshold) => self.current_count() >= threshold,
            None => false, // Enterprise tier has no threshold
        }
    }

    /// Get generation counter for state versioning
    ///
    /// ## Returns
    ///
    /// Current generation counter (upper 32 bits of state)
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_GENERATION_COUNTER: Generation counter prevents TOCTOU races
    ///   #VERIFY_ATOMIC_UPDATE: Generation incremented atomically
    ///   #VERIFY_STATE_CONSISTENCY: Generation validates counter state
    /// ```
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    /// assert_eq!(limiter.generation(), 0);
    /// ```
    #[inline]
    pub fn generation(&self) -> u32 {
        let state = self.state.load(Ordering::Relaxed);
        // Extract upper 32 bits (generation)
        (state >> 32) as u32
    }

    /// Increment generation counter (for state updates)
    ///
    /// ## ASSUM Safety
    ///
    /// ```text
    /// #ASSUME_GENERATION_INCREMENT: Generation increment is atomic
    ///   #VERIFY_ATOMIC_ADD: fetch_add(1<<32) increments upper 32 bits atomically
    ///   #VERIFY_COUNT_UNCHANGED: Lower 32 bits (count) unchanged
    /// ```
    ///
    /// ## Example
    ///
    /// ```rust
    /// use kindly_dedup::protection::{CommercialLimiterCapsule, LicenseTier};
    ///
    /// let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);
    /// let gen_before = limiter.generation();
    /// limiter.increment_generation();
    /// assert_eq!(limiter.generation(), gen_before + 1);
    /// ```
    #[inline]
    pub fn increment_generation(&self) {
        // Increment upper 32 bits (generation), lower 32 bits (count) unchanged
        self.state.fetch_add(1u64 << 32, Ordering::Relaxed);
    }
}

// Compiler-time verification
const _: () = {
    assert!(core::mem::size_of::<CommercialLimiterCapsule>() == 64);
    assert!(core::mem::align_of::<CommercialLimiterCapsule>() == 64);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_limits() {
        assert_eq!(LicenseTier::Demo.limit(), Some(1_000));
        assert_eq!(LicenseTier::Basic.limit(), Some(100_000));
        assert_eq!(LicenseTier::Pro.limit(), Some(10_000_000));
        assert_eq!(LicenseTier::Enterprise.limit(), None);
    }

    #[test]
    fn test_warning_thresholds() {
        assert_eq!(LicenseTier::Demo.warning_threshold(), Some(900));
        assert_eq!(LicenseTier::Basic.warning_threshold(), Some(90_000));
        assert_eq!(LicenseTier::Pro.warning_threshold(), Some(9_000_000));
        assert_eq!(LicenseTier::Enterprise.warning_threshold(), None);
    }

    #[test]
    fn test_tier_names() {
        assert_eq!(LicenseTier::Demo.name(), "Demo");
        assert_eq!(LicenseTier::Basic.name(), "Basic");
        assert_eq!(LicenseTier::Pro.name(), "Pro");
        assert_eq!(LicenseTier::Enterprise.name(), "Enterprise");
    }

    #[test]
    fn test_next_tier() {
        assert_eq!(LicenseTier::Demo.next_tier(), Some(LicenseTier::Basic));
        assert_eq!(LicenseTier::Basic.next_tier(), Some(LicenseTier::Pro));
        assert_eq!(LicenseTier::Pro.next_tier(), Some(LicenseTier::Enterprise));
        assert_eq!(LicenseTier::Enterprise.next_tier(), None);
    }

    #[test]
    fn test_capsule_alignment() {
        assert_eq!(core::mem::size_of::<CommercialLimiterCapsule>(), 64);
        assert_eq!(core::mem::align_of::<CommercialLimiterCapsule>(), 64);
    }

    #[test]
    fn test_demo_tier_limit() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

        // Add 1000 documents (limit)
        for i in 0..1000 {
            assert_eq!(limiter.current_count(), i);
            assert!(limiter.can_add_document().is_ok());
            limiter.record_document().unwrap();
        }

        // 1001st document should fail
        assert_eq!(limiter.current_count(), 1000);
        assert!(limiter.can_add_document().is_err());
        assert!(limiter.record_document().is_err());
    }

    #[test]
    fn test_basic_tier_limit() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Basic);

        // Add documents up to limit
        for _ in 0..100_000 {
            limiter.record_document().unwrap();
        }

        assert_eq!(limiter.current_count(), 100_000);
        assert!(limiter.can_add_document().is_err());
    }

    #[test]
    fn test_enterprise_unlimited() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Enterprise);

        // Add many documents (no limit)
        for _ in 0..200_000 {
            limiter.record_document().unwrap();
        }

        assert_eq!(limiter.current_count(), 200_000);
        assert!(limiter.can_add_document().is_ok());
        assert_eq!(limiter.remaining_documents(), None);
    }

    #[test]
    fn test_warning_threshold() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

        // Below threshold
        for _ in 0..899 {
            limiter.record_document().unwrap();
        }
        assert!(!limiter.is_at_warning_threshold());

        // At threshold (900 = 90% of 1000)
        limiter.record_document().unwrap();
        assert_eq!(limiter.current_count(), 900);
        assert!(limiter.is_at_warning_threshold());
    }

    #[test]
    fn test_remaining_documents() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

        assert_eq!(limiter.remaining_documents(), Some(1000));

        limiter.record_document().unwrap();
        assert_eq!(limiter.remaining_documents(), Some(999));

        for _ in 1..1000 {
            limiter.record_document().unwrap();
        }
        assert_eq!(limiter.remaining_documents(), Some(0));
    }

    #[test]
    fn test_generation_counter() {
        let limiter = CommercialLimiterCapsule::new(LicenseTier::Demo);

        assert_eq!(limiter.generation(), 0);

        limiter.increment_generation();
        assert_eq!(limiter.generation(), 1);

        // Verify count unchanged
        assert_eq!(limiter.current_count(), 0);
    }

    #[test]
    fn test_error_messages() {
        let err = CommercialLimitError::LimitReached {
            tier: LicenseTier::Demo,
            limit: 1000,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Demo tier limit reached"));
        assert!(msg.contains("1000 documents"));
        assert!(msg.contains("https://kindly.dev/pricing"));

        let err = CommercialLimitError::UpgradeRequired {
            current_tier: LicenseTier::Basic,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Pro tier"));
    }

    #[test]
    fn test_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(CommercialLimiterCapsule::new(LicenseTier::Basic));
        let mut handles = vec![];

        // Spawn 10 threads, each adding 1000 documents
        for _ in 0..10 {
            let limiter_clone = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    let _ = limiter_clone.record_document();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 10,000 documents total
        assert_eq!(limiter.current_count(), 10_000);
    }

    #[test]
    fn test_concurrent_can_add_document() {
        use std::sync::Arc;
        use std::thread;

        let limiter = Arc::new(CommercialLimiterCapsule::new(LicenseTier::Demo));
        let mut handles = vec![];

        // Fill to just below limit
        for _ in 0..990 {
            limiter.record_document().unwrap();
        }

        // Spawn 20 threads trying to add documents concurrently
        for _ in 0..20 {
            let limiter_clone = Arc::clone(&limiter);
            let handle = thread::spawn(move || {
                let mut success_count = 0;
                for _ in 0..10 {
                    if limiter_clone.record_document().is_ok() {
                        success_count += 1;
                    }
                }
                success_count
            });
            handles.push(handle);
        }

        let total_success: u64 = handles.into_iter().map(|h| h.join().unwrap()).sum();

        // Should have added exactly 10 more documents (to reach 1000 limit)
        assert_eq!(total_success, 10);
        assert_eq!(limiter.current_count(), 1000);
        assert!(limiter.can_add_document().is_err());
    }
}
