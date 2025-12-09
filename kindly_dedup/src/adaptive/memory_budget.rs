//! MemoryBudgetCapsule - O(1) Memory Enforcement (T0 Auditable Tier)
//!
//! **UCE34 Framework**: Q10 T0 tier selection (auditable memory tracking)
//! **Chaos Compliance**: 100% lockfree (AtomicU64 only), cache-aligned (64B)
//!
//! # Overview
//!
//! Tracks and limits memory usage to ensure O(1) memory regardless of corpus size.
//! Critical for production deployments where memory is constrained.
//!
//! # Performance
//!
//! - `try_allocate`: <100ns (CAS loop)
//! - `release`: <50ns (atomic sub via CAS)
//! - `can_allocate`: <20ns (single atomic load)
//! - `current_bytes`: <20ns (single atomic load)
//!
//! # Memory Layout (64B, 1 cache line)
//!
//! ```text
//! [0-7]   budget_state: AtomicU64 (current_bytes(32) | generation(32))
//! [8-15]  max_budget_bytes: u64 (immutable after init)
//! [16-63] _padding: [u8; 48]
//! ```
//!
//! # ASSUM Safety Tags
//!
//! - #ASSUME: 4GB max tracking sufficient (u32 current_bytes)
//! - #VERIFY: Production corpus 3.5GB max (93% memory reduction architecture)
//! - #ASSUME: Generation counter wraps safely (4B operations before wrap)
//! - #VERIFY: At 1M ops/sec, wrap takes 4295 seconds (71 minutes)
//!
//! # Example
//!
//! ```rust
//! use kindly_dedup::adaptive::memory_budget::{MemoryBudgetCapsule, presets};
//!
//! // Create with 1.5GB budget
//! let budget = MemoryBudgetCapsule::new_gb(1);
//!
//! // Try to allocate
//! if budget.try_allocate(1024 * 1024).is_ok() {
//!     // Use memory...
//!     budget.release(1024 * 1024).unwrap();
//! }
//!
//! // Check usage
//! println!("Usage: {:.1}%", budget.usage_percent());
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

// ============================================================================
// PRESETS (Production Memory Budgets)
// ============================================================================

/// Predefined memory budget presets for different execution modes
pub mod presets {
    /// 1.44 GB - CPU streaming mode (conservative)
    /// #ASSUME: Sufficient for streaming dedup with 10M+ documents
    /// #VERIFY: Measured 222MB peak in UniversalDedupPipeline
    pub const CPU_STREAMING_BYTES: u64 = 1_500_000_000;

    /// 1.5 GB - GPU LSH mode (includes GPU buffer overhead)
    /// #ASSUME: Additional headroom for GPU buffer transfers
    /// #VERIFY: GPU double-buffering adds ~256MB overhead
    pub const GPU_LSH_BYTES: u64 = 1_600_000_000;

    /// 1.5 GB - Combined adaptive max (worst-case both modes)
    /// #ASSUME: Peak memory during CPU->GPU transition
    /// #VERIFY: Transition requires both pipelines briefly active
    pub const ADAPTIVE_MAX_BYTES: u64 = 1_600_000_000;

    /// 256 MB - Testing budget (small corpus validation)
    pub const TEST_SMALL_BYTES: u64 = 256 * 1024 * 1024;

    /// 512 MB - Development budget (medium corpus)
    pub const DEV_MEDIUM_BYTES: u64 = 512 * 1024 * 1024;

    /// 2 GB - Laptop preset (conservative for shared memory)
    pub const LAPTOP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

    /// 8 GB - Desktop preset (typical workstation)
    pub const DESKTOP_BYTES: u64 = 8 * 1024 * 1024 * 1024;

    /// 32 GB - Server preset (production server) - capped to 4GB for u32 tracking
    pub const SERVER_BYTES: u64 = u32::MAX as u64;

    /// 512 MB - Minimal preset (embedded/constrained)
    pub const MINIMAL_BYTES: u64 = 512 * 1024 * 1024;
}

// ============================================================================
// ERROR TYPES
// ============================================================================

/// Memory budget error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryError {
    /// Requested allocation would exceed budget
    BudgetExceeded {
        /// Bytes requested
        requested: usize,
        /// Bytes available
        available: usize,
        /// Total budget
        budget: usize,
    },
    /// Arithmetic overflow in allocation tracking
    Overflow,
    /// Tried to release more than currently allocated
    Underflow {
        /// Current allocated bytes
        current: usize,
        /// Bytes attempted to release
        release: usize,
    },
}

impl core::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MemoryError::BudgetExceeded {
                requested,
                available,
                budget,
            } => {
                write!(
                    f,
                    "Memory budget exceeded: requested {} bytes, available {} bytes, budget {} bytes",
                    requested, available, budget
                )
            }
            MemoryError::Overflow => {
                write!(f, "Memory allocation overflow")
            }
            MemoryError::Underflow { current, release } => {
                write!(
                    f,
                    "Memory release underflow: tried to release {} bytes but only {} allocated",
                    release, current
                )
            }
        }
    }
}

impl std::error::Error for MemoryError {}

// ============================================================================
// SNAPSHOT TYPE
// ============================================================================

/// Snapshot of memory budget state for debugging and monitoring
#[derive(Debug, Clone, Copy)]
pub struct MemoryBudgetSnapshot {
    /// Current allocated bytes
    pub current_bytes: usize,
    /// Maximum budget in bytes
    pub max_bytes: u64,
    /// Available bytes remaining
    pub available: usize,
    /// Usage percentage (0.0 - 100.0)
    pub usage_percent: f64,
    /// Generation counter (for Q34 audit)
    pub generation: u32,
}

impl MemoryBudgetSnapshot {
    /// Check if O(1) invariant is maintained
    ///
    /// Returns true if current usage is within budget.
    #[inline]
    pub const fn is_within_budget(&self) -> bool {
        self.current_bytes as u64 <= self.max_bytes
    }

    /// Utilization as percentage (0-100) as integer
    #[inline]
    pub fn utilization_percent(&self) -> u8 {
        if self.max_bytes == 0 {
            return 0;
        }
        ((self.current_bytes as u64 * 100) / self.max_bytes) as u8
    }

    /// Legacy API: Get allocated bytes (alias for current_bytes)
    #[inline]
    pub const fn allocated(&self) -> usize {
        self.current_bytes
    }

    /// Legacy API: Get total budget (alias for max_bytes)
    #[inline]
    pub const fn total(&self) -> u64 {
        self.max_bytes
    }

    /// Legacy API: Get peak allocation (same as current for new API)
    /// Note: Peak tracking was removed in the 64B compact layout
    #[inline]
    pub const fn peak(&self) -> usize {
        self.current_bytes
    }
}

// ============================================================================
// HELPER FUNCTIONS (Bit Packing)
// ============================================================================

/// Pack budget state into u64
/// Layout: bits 0-31 = current_bytes (u32), bits 32-63 = generation (u32)
#[inline]
const fn pack_budget(current: u32, generation: u32) -> u64 {
    (current as u64) | ((generation as u64) << 32)
}

/// Unpack budget state from u64
/// Returns (current_bytes, generation)
#[inline]
const fn unpack_budget(packed: u64) -> (u32, u32) {
    let current = packed as u32;
    let generation = (packed >> 32) as u32;
    (current, generation)
}

// ============================================================================
// MEMORY BUDGET CAPSULE
// ============================================================================

/// MemoryBudgetCapsule - O(1) memory enforcement
///
/// Tracks and limits memory usage to ensure O(1) memory regardless of corpus size.
/// Critical for production deployments where memory is constrained.
///
/// # Chaos Compliance
/// - 100% lockfree (AtomicU64 only)
/// - Cache-aligned (64B, 1 cache line)
/// - Generation counter for Q34 audit trail
///
/// # Performance
/// - try_allocate: <100ns (CAS loop)
/// - release: <50ns (atomic sub via CAS)
/// - check: <20ns (single load)
///
/// # Memory Layout (64B)
///
/// ```text
/// [0-7]   budget_state: AtomicU64 (current_bytes(32) | generation(32))
/// [8-15]  max_budget_bytes: u64 (immutable after init)
/// [16-63] _padding: [u8; 48]
/// ```
#[repr(C, align(64))]
pub struct MemoryBudgetCapsule {
    /// Packed state: current_bytes(32) | generation(32)
    budget_state: AtomicU64,
    /// Maximum budget in bytes (immutable after init)
    max_budget_bytes: u64,
    /// Padding to fill cache line (64B total)
    _padding: [u8; 48],
}

// #ASSUME: MemoryBudgetCapsule is Send+Sync due to AtomicU64 internals
// #VERIFY: All fields are either AtomicU64 (Send+Sync), u64 (Send+Sync), or [u8; N] (Send+Sync)
unsafe impl Send for MemoryBudgetCapsule {}
unsafe impl Sync for MemoryBudgetCapsule {}

impl MemoryBudgetCapsule {
    /// Create new budget with max in bytes
    ///
    /// # Arguments
    /// - `max_bytes`: Maximum memory budget in bytes (capped at 4GB for u32 tracking)
    ///
    /// # Performance
    /// - Time: O(1), <100ns
    /// - Memory: 64B (stack allocated)
    ///
    /// # ASSUM Safety
    /// #ASSUME: max_bytes <= 4GB (u32::MAX = 4,294,967,295)
    /// #VERIFY: Production budgets are 1.5-1.6GB (well under limit)
    #[inline]
    pub const fn new(max_bytes: u64) -> Self {
        // Cap at u32::MAX to prevent overflow in current_bytes tracking
        let capped_max = if max_bytes > u32::MAX as u64 {
            u32::MAX as u64
        } else {
            max_bytes
        };

        Self {
            budget_state: AtomicU64::new(pack_budget(0, 0)),
            max_budget_bytes: capped_max,
            _padding: [0u8; 48],
        }
    }

    /// Create with max in megabytes (convenience)
    ///
    /// # Arguments
    /// - `max_mb`: Maximum memory budget in megabytes
    #[inline]
    pub const fn new_mb(max_mb: u64) -> Self {
        Self::new(max_mb * 1024 * 1024)
    }

    /// Create with max in gigabytes (convenience)
    ///
    /// # Arguments
    /// - `max_gb`: Maximum memory budget in gigabytes (max 4GB)
    #[inline]
    pub const fn new_gb(max_gb: u64) -> Self {
        Self::new(max_gb * 1024 * 1024 * 1024)
    }

    /// Try to allocate bytes within budget
    ///
    /// Returns Ok(()) if successful, Err if would exceed budget.
    ///
    /// # Performance
    /// - Time: <100ns typical (CAS loop, usually 1-2 iterations)
    /// - Memory: O(1)
    ///
    /// # Algorithm
    /// Uses CAS loop to atomically check and update:
    /// 1. Load current state
    /// 2. Check if allocation would exceed budget
    /// 3. CAS to update current_bytes and increment generation
    /// 4. Retry if CAS fails (contention)
    ///
    /// # ASSUM Safety
    /// #ASSUME: CAS loop terminates (no livelock under reasonable contention)
    /// #VERIFY: CAS guarantees progress (one thread always succeeds per iteration)
    pub fn try_allocate(&self, bytes: usize) -> Result<(), MemoryError> {
        loop {
            let current = self.budget_state.load(Ordering::Acquire);
            let (current_bytes, generation) = unpack_budget(current);

            // Check for arithmetic overflow
            let new_bytes = (current_bytes as usize)
                .checked_add(bytes)
                .ok_or(MemoryError::Overflow)?;

            // Check if allocation would exceed budget
            if new_bytes as u64 > self.max_budget_bytes {
                return Err(MemoryError::BudgetExceeded {
                    requested: bytes,
                    available: (self.max_budget_bytes as usize).saturating_sub(current_bytes as usize),
                    budget: self.max_budget_bytes as usize,
                });
            }

            // Pack new state with incremented generation
            let new_state = pack_budget(new_bytes as u32, generation.wrapping_add(1));

            // Attempt CAS
            if self
                .budget_state
                .compare_exchange(current, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            // CAS failed due to contention, retry
            core::hint::spin_loop();
        }
    }

    /// Allocate bytes within budget (legacy API, returns bool)
    ///
    /// For backward compatibility with existing code.
    /// Prefer `try_allocate` for new code.
    #[inline]
    pub fn allocate(&self, bytes: u64) -> bool {
        self.try_allocate(bytes as usize).is_ok()
    }

    /// Release previously allocated bytes
    ///
    /// # Performance
    /// - Time: <50ns typical (CAS loop, usually 1 iteration)
    /// - Memory: O(1)
    ///
    /// # ASSUM Safety
    /// #ASSUME: Caller releases exactly what was allocated (no double-free)
    /// #VERIFY: Underflow error catches mismatched release
    pub fn release(&self, bytes: usize) -> Result<(), MemoryError> {
        loop {
            let current = self.budget_state.load(Ordering::Acquire);
            let (current_bytes, generation) = unpack_budget(current);

            // Check for underflow
            if bytes > current_bytes as usize {
                return Err(MemoryError::Underflow {
                    current: current_bytes as usize,
                    release: bytes,
                });
            }

            let new_bytes = (current_bytes as usize) - bytes;
            let new_state = pack_budget(new_bytes as u32, generation.wrapping_add(1));

            if self
                .budget_state
                .compare_exchange(current, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok(());
            }
            core::hint::spin_loop();
        }
    }

    /// Deallocate bytes (legacy API, no error return)
    ///
    /// For backward compatibility with existing code.
    /// Prefer `release` for new code.
    #[inline]
    pub fn deallocate(&self, bytes: u64) {
        let _ = self.release(bytes as usize);
    }

    /// Check if allocation would succeed (doesn't modify state)
    ///
    /// # Performance
    /// - Time: <20ns (single atomic load)
    /// - Memory: O(1)
    #[inline]
    pub fn can_allocate(&self, bytes: u64) -> bool {
        let current = self.budget_state.load(Ordering::Acquire);
        let (current_bytes, _) = unpack_budget(current);

        let new_bytes = match (current_bytes as u64).checked_add(bytes) {
            Some(n) => n,
            None => return false, // Overflow
        };

        new_bytes <= self.max_budget_bytes
    }

    /// Get current usage in bytes
    ///
    /// # Performance
    /// - Time: <20ns (single atomic load)
    #[inline]
    pub fn current_bytes(&self) -> usize {
        let current = self.budget_state.load(Ordering::Acquire);
        let (current_bytes, _) = unpack_budget(current);
        current_bytes as usize
    }

    /// Get currently allocated bytes (legacy API alias)
    #[inline]
    pub fn allocated(&self) -> u64 {
        self.current_bytes() as u64
    }

    /// Get current usage in megabytes
    #[inline]
    pub fn current_mb(&self) -> f64 {
        self.current_bytes() as f64 / (1024.0 * 1024.0)
    }

    /// Get maximum budget in bytes
    #[inline]
    pub const fn max_bytes(&self) -> u64 {
        self.max_budget_bytes
    }

    /// Get total budget in bytes (legacy API alias)
    #[inline]
    pub fn total(&self) -> u64 {
        self.max_budget_bytes
    }

    /// Get maximum budget in megabytes
    #[inline]
    pub fn max_mb(&self) -> f64 {
        self.max_budget_bytes as f64 / (1024.0 * 1024.0)
    }

    /// Get available bytes remaining
    #[inline]
    pub fn available(&self) -> usize {
        let current = self.current_bytes();
        (self.max_budget_bytes as usize).saturating_sub(current)
    }

    /// Get usage percentage (0.0 - 100.0)
    #[inline]
    pub fn usage_percent(&self) -> f64 {
        if self.max_budget_bytes == 0 {
            return 0.0;
        }
        (self.current_bytes() as f64 / self.max_budget_bytes as f64) * 100.0
    }

    /// Reset to zero usage
    ///
    /// Atomically resets current_bytes to 0 while incrementing generation.
    ///
    /// # Performance
    /// - Time: <50ns (CAS loop)
    pub fn reset(&self) {
        loop {
            let current = self.budget_state.load(Ordering::Acquire);
            let (_, generation) = unpack_budget(current);
            let new_state = pack_budget(0, generation.wrapping_add(1));

            if self
                .budget_state
                .compare_exchange(current, new_state, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Get generation counter (for Q34 audit)
    ///
    /// Generation increments on every state-modifying operation.
    ///
    /// # Performance
    /// - Time: <20ns (single atomic load)
    #[inline]
    pub fn generation(&self) -> u32 {
        let current = self.budget_state.load(Ordering::Acquire);
        let (_, generation) = unpack_budget(current);
        generation
    }

    /// Assert O(1) invariant (panics if violated)
    ///
    /// Use in tests to validate memory guarantees.
    ///
    /// # Panics
    /// Panics if current usage exceeds budget.
    #[inline]
    pub fn assert_o1(&self) {
        let current = self.current_bytes();
        let max = self.max_budget_bytes as usize;
        assert!(
            current <= max,
            "O(1) memory invariant violated: current {} > budget {}",
            current,
            max
        );
    }

    /// Get full snapshot for debugging
    ///
    /// # Performance
    /// - Time: <50ns (single atomic load + calculations)
    #[inline]
    pub fn snapshot(&self) -> MemoryBudgetSnapshot {
        let current = self.budget_state.load(Ordering::Acquire);
        let (current_bytes, generation) = unpack_budget(current);

        let current_usize = current_bytes as usize;
        let max = self.max_budget_bytes;

        MemoryBudgetSnapshot {
            current_bytes: current_usize,
            max_bytes: max,
            available: (max as usize).saturating_sub(current_usize),
            usage_percent: if max == 0 {
                0.0
            } else {
                (current_usize as f64 / max as f64) * 100.0
            },
            generation,
        }
    }
}

impl Default for MemoryBudgetCapsule {
    /// Default to adaptive max preset (1.5GB)
    fn default() -> Self {
        Self::new(presets::ADAPTIVE_MAX_BYTES)
    }
}

impl core::fmt::Debug for MemoryBudgetCapsule {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let snapshot = self.snapshot();
        f.debug_struct("MemoryBudgetCapsule")
            .field("current_bytes", &snapshot.current_bytes)
            .field("max_bytes", &snapshot.max_bytes)
            .field("usage_percent", &format!("{:.1}%", snapshot.usage_percent))
            .field("generation", &snapshot.generation)
            .finish()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_starts_empty() {
        let budget = MemoryBudgetCapsule::new(1024);
        assert_eq!(budget.current_bytes(), 0);
        assert_eq!(budget.generation(), 0);
        assert_eq!(budget.max_bytes(), 1024);
    }

    #[test]
    fn test_allocate_within_budget() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Allocate 512 bytes
        assert!(budget.try_allocate(512).is_ok());
        assert_eq!(budget.current_bytes(), 512);
        assert_eq!(budget.generation(), 1);

        // Allocate another 256 bytes
        assert!(budget.try_allocate(256).is_ok());
        assert_eq!(budget.current_bytes(), 768);
        assert_eq!(budget.generation(), 2);

        // Can allocate up to budget
        assert!(budget.try_allocate(256).is_ok());
        assert_eq!(budget.current_bytes(), 1024);
    }

    #[test]
    fn test_allocate_exceeds_budget() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Fill to half
        assert!(budget.try_allocate(512).is_ok());

        // Try to exceed budget
        let err = budget.try_allocate(600).unwrap_err();
        assert!(matches!(err, MemoryError::BudgetExceeded { .. }));

        if let MemoryError::BudgetExceeded {
            requested,
            available,
            budget: total,
        } = err
        {
            assert_eq!(requested, 600);
            assert_eq!(available, 512);
            assert_eq!(total, 1024);
        }

        // Current bytes unchanged after failed allocation
        assert_eq!(budget.current_bytes(), 512);
    }

    #[test]
    fn test_release_decreases_usage() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Allocate then release
        assert!(budget.try_allocate(512).is_ok());
        assert_eq!(budget.current_bytes(), 512);

        assert!(budget.release(256).is_ok());
        assert_eq!(budget.current_bytes(), 256);

        // Generation incremented for release too
        assert_eq!(budget.generation(), 2);
    }

    #[test]
    fn test_release_underflow_error() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Allocate 256 bytes
        assert!(budget.try_allocate(256).is_ok());

        // Try to release more than allocated
        let err = budget.release(512).unwrap_err();
        assert!(matches!(err, MemoryError::Underflow { .. }));

        if let MemoryError::Underflow { current, release } = err {
            assert_eq!(current, 256);
            assert_eq!(release, 512);
        }

        // Current bytes unchanged after failed release
        assert_eq!(budget.current_bytes(), 256);
    }

    #[test]
    fn test_can_allocate_check() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Initially can allocate anything up to budget
        assert!(budget.can_allocate(1024));
        assert!(budget.can_allocate(512));
        assert!(!budget.can_allocate(1025)); // Over budget

        // After allocation, check updates
        budget.try_allocate(768).unwrap();
        assert!(budget.can_allocate(256));
        assert!(!budget.can_allocate(257)); // Would exceed
    }

    #[test]
    fn test_usage_percent() {
        let budget = MemoryBudgetCapsule::new(1000);

        assert_eq!(budget.usage_percent(), 0.0);

        budget.try_allocate(500).unwrap();
        assert!((budget.usage_percent() - 50.0).abs() < 0.01);

        budget.try_allocate(250).unwrap();
        assert!((budget.usage_percent() - 75.0).abs() < 0.01);

        budget.try_allocate(250).unwrap();
        assert!((budget.usage_percent() - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_assert_o1_passes() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Should not panic when within budget
        budget.assert_o1();

        budget.try_allocate(512).unwrap();
        budget.assert_o1();

        budget.try_allocate(512).unwrap();
        budget.assert_o1(); // Exactly at budget is OK
    }

    #[test]
    fn test_generation_increments() {
        let budget = MemoryBudgetCapsule::new(1024);
        assert_eq!(budget.generation(), 0);

        budget.try_allocate(100).unwrap();
        assert_eq!(budget.generation(), 1);

        budget.try_allocate(100).unwrap();
        assert_eq!(budget.generation(), 2);

        budget.release(50).unwrap();
        assert_eq!(budget.generation(), 3);

        budget.reset();
        assert_eq!(budget.generation(), 4);
    }

    #[test]
    fn test_capsule_size_and_alignment() {
        // Verify Chaos compliance: 64B, 1 cache line
        assert_eq!(
            core::mem::size_of::<MemoryBudgetCapsule>(),
            64,
            "MemoryBudgetCapsule should be exactly 64 bytes"
        );

        assert_eq!(
            core::mem::align_of::<MemoryBudgetCapsule>(),
            64,
            "MemoryBudgetCapsule should be 64-byte aligned"
        );
    }

    #[test]
    fn test_snapshot() {
        let budget = MemoryBudgetCapsule::new(1000);
        budget.try_allocate(300).unwrap();

        let snap = budget.snapshot();
        assert_eq!(snap.current_bytes, 300);
        assert_eq!(snap.max_bytes, 1000);
        assert_eq!(snap.available, 700);
        assert!((snap.usage_percent - 30.0).abs() < 0.01);
        assert_eq!(snap.generation, 1);
        assert!(snap.is_within_budget());
    }

    #[test]
    fn test_convenience_constructors() {
        let mb_budget = MemoryBudgetCapsule::new_mb(100);
        assert_eq!(mb_budget.max_bytes(), 100 * 1024 * 1024);

        let gb_budget = MemoryBudgetCapsule::new_gb(2);
        assert_eq!(gb_budget.max_bytes(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_reset() {
        let budget = MemoryBudgetCapsule::new(1024);
        budget.try_allocate(512).unwrap();
        assert_eq!(budget.current_bytes(), 512);

        budget.reset();
        assert_eq!(budget.current_bytes(), 0);
        assert_eq!(budget.generation(), 2); // allocate + reset
    }

    #[test]
    fn test_available() {
        let budget = MemoryBudgetCapsule::new(1024);
        assert_eq!(budget.available(), 1024);

        budget.try_allocate(300).unwrap();
        assert_eq!(budget.available(), 724);

        budget.try_allocate(724).unwrap();
        assert_eq!(budget.available(), 0);
    }

    #[test]
    fn test_default() {
        let budget = MemoryBudgetCapsule::default();
        assert_eq!(budget.max_bytes(), presets::ADAPTIVE_MAX_BYTES);
    }

    #[test]
    fn test_debug_format() {
        let budget = MemoryBudgetCapsule::new(1000);
        budget.try_allocate(500).unwrap();

        let debug = format!("{:?}", budget);
        assert!(debug.contains("MemoryBudgetCapsule"));
        assert!(debug.contains("current_bytes"));
        assert!(debug.contains("500"));
    }

    #[test]
    fn test_error_display() {
        let err = MemoryError::BudgetExceeded {
            requested: 1000,
            available: 500,
            budget: 1024,
        };
        let display = format!("{}", err);
        assert!(display.contains("budget exceeded"));
        assert!(display.contains("1000"));
        assert!(display.contains("500"));

        let underflow = MemoryError::Underflow {
            current: 100,
            release: 200,
        };
        let display2 = format!("{}", underflow);
        assert!(display2.contains("underflow"));

        let overflow = MemoryError::Overflow;
        let display3 = format!("{}", overflow);
        assert!(display3.contains("overflow"));
    }

    #[test]
    fn test_zero_budget() {
        let budget = MemoryBudgetCapsule::new(0);
        assert_eq!(budget.max_bytes(), 0);
        assert_eq!(budget.usage_percent(), 0.0);
        assert!(!budget.can_allocate(1));

        let err = budget.try_allocate(1).unwrap_err();
        assert!(matches!(err, MemoryError::BudgetExceeded { .. }));
    }

    #[test]
    fn test_exact_budget_allocation() {
        let budget = MemoryBudgetCapsule::new(1024);

        // Allocate exactly at budget
        assert!(budget.try_allocate(1024).is_ok());
        assert_eq!(budget.current_bytes(), 1024);
        assert_eq!(budget.available(), 0);

        // Can't allocate even 1 more byte
        assert!(!budget.can_allocate(1));
    }

    // Legacy API compatibility tests
    #[test]
    fn test_legacy_allocate_api() {
        let budget = MemoryBudgetCapsule::new_mb(100);
        assert!(budget.allocate(50 * 1024 * 1024));
        assert_eq!(budget.allocated(), 50 * 1024 * 1024);
    }

    #[test]
    fn test_legacy_deallocate_api() {
        let budget = MemoryBudgetCapsule::new_mb(100);
        budget.allocate(50 * 1024 * 1024);
        budget.deallocate(25 * 1024 * 1024);
        assert_eq!(budget.allocated(), 25 * 1024 * 1024);
    }

    #[test]
    fn test_legacy_total_api() {
        let budget = MemoryBudgetCapsule::new_gb(4);
        // Capped to u32::MAX (4GB limit for u32 tracking)
        assert!(budget.total() <= u32::MAX as u64);
    }

    #[test]
    fn test_new_gb() {
        let budget = MemoryBudgetCapsule::new_gb(2);
        assert_eq!(budget.total(), 2 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_allocation_fails_when_budget_exceeded() {
        let budget = MemoryBudgetCapsule::new_mb(100);

        assert!(budget.allocate(100 * 1024 * 1024));
        assert!(!budget.allocate(1)); // Should fail, budget exhausted
    }

    #[test]
    fn test_utilization_percent() {
        let budget = MemoryBudgetCapsule::new_mb(100);
        budget.allocate(25 * 1024 * 1024);

        let snapshot = budget.snapshot();
        assert_eq!(snapshot.utilization_percent(), 25);
    }

    // Multi-threaded stress test
    #[test]
    fn test_concurrent_allocate_release() {
        use std::sync::Arc;
        use std::thread;

        let budget = Arc::new(MemoryBudgetCapsule::new(1_000_000));
        let mut handles = vec![];

        // Spawn 4 threads, each doing allocate/release cycles
        for _ in 0..4 {
            let budget_clone = Arc::clone(&budget);
            handles.push(thread::spawn(move || {
                for _ in 0..1000 {
                    if budget_clone.try_allocate(100).is_ok() {
                        // Small delay to increase contention
                        std::hint::spin_loop();
                        let _ = budget_clone.release(100);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // After all threads complete, budget should be back to 0 or close
        // (may have some in-flight allocations that failed)
        assert!(budget.current_bytes() < 1000);
        budget.assert_o1();
    }
}
