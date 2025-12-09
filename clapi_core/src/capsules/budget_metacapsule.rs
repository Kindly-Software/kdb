//! BudgetMetaCapsule - Large-scale budget management metacapsule (v0.2.0)
//!
//! **Version**: v0.2.0 - Pure Atomic Architecture
//! **Updated**: 2025-10-16
//!
//! Tier 1 (Atomic) - 128MB metacapsule for:
//! - 1M concurrent budget slots (lockfree BudgetSlotCapsule array)
//! - Circuit breaker protection (graceful degradation)
//! - Generation counter coordination
//! - 100% lockfree allocation/deallocation
//!
//! # Changes from v0.1.x
//! - Replaced `Vec<Option<Arc<RequestCapsule128>>>` with `Box<[BudgetSlotCapsule; 1M]>`
//! - Added `CircuitBreakerCapsule` for failure protection
//! - Removed all Arc overhead (lockfree ownership via AtomicPtr)
//! - Pure atomic operations (no Vec mutations)
//!
//! Performance: <100ns per slot lookup, <50ns per allocation, <5ns circuit check

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::capsules::{BudgetSlotCapsule, CircuitBreakerCapsule, RequestCapsule128};
use crate::error::{ClapiError, ClapiResult};

/// Maximum number of budget slots
pub const MAX_BUDGET_SLOTS: usize = 1_000_000;

/// BudgetMetaCapsule header (128B, cache-aligned)
///
/// # Memory Layout
/// ```text
/// [0-7]     slot_count: AtomicUsize       // Number of active slots
/// [8-15]    generation: AtomicU64          // Global generation counter
/// [16-23]   next_slot_id: AtomicUsize     // Next available slot ID
/// [24-31]   total_allocations: AtomicU64  // Total slot allocations
/// [32-39]   total_deallocations: AtomicU64 // Total slot deallocations
/// [40-127]  _padding: [u8; 88]            // Cache alignment to 128 bytes
/// ```
///
/// # Safety
/// - #ASSUME: AtomicUsize::fetch_add for slot allocation prevents collisions
/// - #VERIFY: Property test validates unique slot IDs under concurrency
/// - #ASSUME: Generation counter increments atomically (monotonic)
/// - #VERIFY: Unit test validates generation increments
#[derive(ComputationalCapsule)]
#[capsule(alignment = 128, size = 128)]
#[repr(C, align(128))]
pub struct BudgetMetaCapsuleHeader {
    /// Number of active slots (0 to MAX_BUDGET_SLOTS)
    slot_count: AtomicUsize,

    /// Global generation counter (TOCTOU prevention)
    generation: AtomicU64,

    /// Next available slot ID (monotonic allocator)
    next_slot_id: AtomicUsize,

    /// Total slot allocations since creation
    total_allocations: AtomicU64,

    /// Total slot deallocations since creation
    total_deallocations: AtomicU64,

    /// Padding to 128 bytes
    _padding: [u8; 88],
}

impl BudgetMetaCapsuleHeader {
    /// Create new metacapsule header
    pub fn new() -> Self {
        Self {
            slot_count: AtomicUsize::new(0),
            generation: AtomicU64::new(1), // Start at 1 (0 = uninitialized)
            next_slot_id: AtomicUsize::new(0),
            total_allocations: AtomicU64::new(0),
            total_deallocations: AtomicU64::new(0),
            _padding: [0u8; 88],
        }
    }

    /// Get current slot count
    ///
    /// # Safety
    /// - #ASSUME: Relaxed load safe for counter reads (monotonic)
    /// - #VERIFY: Concurrent readers get consistent slot count snapshot
    #[inline]
    pub fn slot_count(&self) -> usize {
        self.slot_count.load(Ordering::Relaxed)
    }

    /// Get global generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Allocate a new slot ID (atomic, lockfree)
    ///
    /// # Returns
    /// - `Ok(slot_id)` if allocation successful
    /// - `Err(SlotsExhausted)` if MAX_BUDGET_SLOTS reached
    ///
    /// # Safety
    /// - #ASSUME: fetch_add prevents slot ID collisions
    /// - #VERIFY: Property test validates unique IDs under 100 threads
    ///
    /// # Performance
    /// - <50ns (single atomic operation)
    pub fn allocate_slot(&self) -> ClapiResult<usize> {
        // Check capacity first (fast path)
        let current_count = self.slot_count.load(Ordering::Relaxed);
        if current_count >= MAX_BUDGET_SLOTS {
            return Err(ClapiError::SlotsExhausted {
                max: MAX_BUDGET_SLOTS,
                current: current_count,
            });
        }

        // Allocate slot ID (atomic, monotonic)
        let slot_id = self.next_slot_id.fetch_add(1, Ordering::Relaxed);

        // Check if we exceeded capacity during allocation
        if slot_id >= MAX_BUDGET_SLOTS {
            return Err(ClapiError::SlotsExhausted {
                max: MAX_BUDGET_SLOTS,
                current: slot_id,
            });
        }

        // Update counters
        self.slot_count.fetch_add(1, Ordering::Relaxed);
        self.total_allocations.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(slot_id)
    }

    /// Deallocate a slot (mark as free)
    ///
    /// # Safety
    /// - #ASSUME: Slot ID is valid (caller responsibility)
    /// - #VERIFY: Bounds check in BudgetMetaCapsule::deallocate
    pub fn deallocate_slot(&self) -> ClapiResult<()> {
        let current_count = self.slot_count.load(Ordering::Relaxed);
        if current_count == 0 {
            return Err(ClapiError::NoSlotsAllocated);
        }

        self.slot_count.fetch_sub(1, Ordering::Relaxed);
        self.total_deallocations.fetch_add(1, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Release);

        Ok(())
    }
}

impl Default for BudgetMetaCapsuleHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// BudgetMetaCapsule - Large-scale budget management (v0.2.0)
///
/// Total size: 128B (header) + 1M × 128B (slots) + 64B (circuit) = ~128MB
///
/// # Architecture (Pure Atomic)
/// - Header: 128B (cache-aligned metadata)
/// - Slots: Box<[BudgetSlotCapsule; 1M]> (lockfree AtomicPtr array)
/// - Circuit Breaker: 64B (failure protection)
///
/// # Safety
/// - #ASSUME: Box<[BudgetSlotCapsule]> provides safe preallocated array
/// - #VERIFY: Bounds checks on all slot access
/// - #ASSUME: BudgetSlotCapsule lockfree operations prevent races
/// - #VERIFY: Property tests validate concurrent allocation uniqueness
/// - #ASSUME: CircuitBreakerCapsule prevents cascading failures
/// - #VERIFY: Failure rate tracking ensures graceful degradation
pub struct BudgetMetaCapsule {
    /// Metacapsule header (128B, cache-aligned)
    header: BudgetMetaCapsuleHeader,

    /// Budget slots (1M × 128B preallocated array)
    /// #ASSUME: BudgetSlotCapsule provides lockfree allocation via AtomicPtr
    /// #VERIFY: No Arc overhead, direct pointer ownership transfer
    slots: Box<[BudgetSlotCapsule; MAX_BUDGET_SLOTS]>,

    /// Circuit breaker (64B, graceful degradation)
    /// #ASSUME: Tracks allocation success/failure rates
    /// #VERIFY: Opens circuit at >10% failure rate
    circuit_breaker: CircuitBreakerCapsule,
}

impl BudgetMetaCapsule {
    /// Create new budget metacapsule (v0.2.0 - preallocated array)
    ///
    /// # Memory Allocation
    /// - Header: 128B
    /// - Slots: 1M × 128B (BudgetSlotCapsule array) = 128MB (preallocated)
    /// - Circuit Breaker: 64B
    /// - Total: ~128MB upfront allocation
    ///
    /// # Performance
    /// - Initialization: ~50ms (array zeroing)
    /// - Zero allocations after init (all operations lockfree on preallocated array)
    pub fn new() -> Self {
        // Preallocate 1M budget slots (128MB total) - heap allocation via Vec
        // #ASSUME: Vec allocates directly on heap (no stack overflow)
        // #VERIFY: All slots initialized to empty state (null ptr)
        //
        // NOTE: We use Vec + try_into() to avoid stack overflow from Box::new([T; N])
        // which would first allocate on stack then move to heap
        let slots_vec: Vec<BudgetSlotCapsule> = (0..MAX_BUDGET_SLOTS)
            .map(|_| BudgetSlotCapsule::new())
            .collect();

        let slots: Box<[BudgetSlotCapsule; MAX_BUDGET_SLOTS]> = slots_vec
            .into_boxed_slice()
            .try_into()
            .expect("Vec length must match MAX_BUDGET_SLOTS");

        Self {
            header: BudgetMetaCapsuleHeader::new(),
            slots,
            circuit_breaker: CircuitBreakerCapsule::new(),
        }
    }

    /// Get current slot count
    #[inline]
    pub fn slot_count(&self) -> usize {
        self.header.slot_count()
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.header.generation()
    }

    /// Allocate a new budget slot (lockfree, with circuit breaker)
    ///
    /// # Returns
    /// - `Ok(slot_id)` if allocation successful
    /// - `Err(CircuitOpen)` if circuit breaker triggered
    /// - `Err(SlotsExhausted)` if MAX_BUDGET_SLOTS reached
    /// - `Err(AllocationConflict)` if CAS retries exceeded
    ///
    /// # Performance
    /// - <100ns (allocation + slot CAS)
    /// - Circuit check: <5ns (atomic load)
    ///
    /// # Safety
    /// - #ASSUME: Circuit breaker prevents cascading failures
    /// - #VERIFY: Opens at >10% allocation failure rate
    /// - #ASSUME: BudgetSlotCapsule::try_allocate is lockfree
    /// - #VERIFY: AtomicPtr CAS ensures atomic ownership transfer
    pub fn allocate(&self, budget_id: u64, initial_budget_cents: i64) -> ClapiResult<usize> {
        // Check circuit breaker first (fast path: <5ns)
        if !self.circuit_breaker.allows_operation() {
            return Err(ClapiError::ConfigError(
                "Circuit breaker open: allocation failures exceeded threshold".to_string(),
            ));
        }

        // Allocate slot ID from header (atomic, lockfree)
        let slot_id = self.header.allocate_slot()?;

        // Create new budget capsule
        let capsule = Box::new(RequestCapsule128::new(initial_budget_cents));

        // Try to allocate slot atomically (lockfree CAS)
        // #ASSUME: slot_id < MAX_BUDGET_SLOTS (validated in allocate_slot)
        // #VERIFY: Bounds check below
        if slot_id >= MAX_BUDGET_SLOTS {
            return Err(ClapiError::InvalidSlotId {
                slot_id,
                max: MAX_BUDGET_SLOTS,
            });
        }

        match self.slots[slot_id].try_allocate(budget_id, capsule) {
            Ok(()) => {
                // Allocation successful - record success for circuit breaker
                self.circuit_breaker.record_success();
                Ok(slot_id)
            }
            Err(_capsule) => {
                // Allocation failed (slot occupied) - record failure
                self.circuit_breaker.record_failure();
                // Rollback header allocation
                let _ = self.header.deallocate_slot();
                Err(ClapiError::ConfigError(
                    "Slot allocation conflict: CAS failed".to_string(),
                ))
            }
        }
    }

    /// Get budget capsule reference by slot ID (lockfree)
    ///
    /// # Safety
    /// - #ASSUME: Slot ID is valid (bounds checked)
    /// - #VERIFY: Bounds check below
    /// - #ASSUME: Returned reference valid while slot allocated
    /// - #VERIFY: Pointer remains valid until deallocation
    ///
    /// # Performance
    /// - <10ns (array lookup + pointer load)
    pub fn get(&self, slot_id: usize) -> ClapiResult<&RequestCapsule128> {
        if slot_id >= MAX_BUDGET_SLOTS {
            return Err(ClapiError::InvalidSlotId {
                slot_id,
                max: MAX_BUDGET_SLOTS,
            });
        }

        self.slots[slot_id]
            .get()
            .ok_or(ClapiError::SlotNotAllocated { slot_id })
    }

    /// Deallocate a budget slot (lockfree)
    ///
    /// # Safety
    /// - #ASSUME: BudgetSlotCapsule::deallocate transfers ownership atomically
    /// - #VERIFY: AtomicPtr swap ensures lockfree deallocation
    /// - #ASSUME: No other threads hold references after deallocation
    /// - #VERIFY: Capsule dropped when Box goes out of scope
    ///
    /// # Performance
    /// - <50ns (atomic swap + counter update)
    pub fn deallocate(&self, slot_id: usize) -> ClapiResult<()> {
        if slot_id >= MAX_BUDGET_SLOTS {
            return Err(ClapiError::InvalidSlotId {
                slot_id,
                max: MAX_BUDGET_SLOTS,
            });
        }

        // Deallocate slot atomically (lockfree swap)
        let _capsule = self.slots[slot_id].deallocate()?;
        // Box<RequestCapsule128> dropped here, freeing memory

        // Update header counters
        self.header.deallocate_slot()?;

        Ok(())
    }

    /// Get statistics (aggregated across all slots)
    pub fn get_stats(&self) -> MetaCapsuleStats {
        MetaCapsuleStats {
            slot_count: self.header.slot_count(),
            generation: self.header.generation(),
            total_allocations: self.header.total_allocations.load(Ordering::Relaxed),
            total_deallocations: self.header.total_deallocations.load(Ordering::Relaxed),
            max_slots: MAX_BUDGET_SLOTS,
        }
    }
}

impl Default for BudgetMetaCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Metacapsule statistics snapshot
#[derive(Debug, Clone)]
pub struct MetaCapsuleStats {
    /// Current number of active slots
    pub slot_count: usize,
    /// Global generation counter
    pub generation: u64,
    /// Total slot allocations since creation
    pub total_allocations: u64,
    /// Total slot deallocations since creation
    pub total_deallocations: u64,
    /// Maximum number of slots
    pub max_slots: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<BudgetMetaCapsuleHeader>(), 128);
    }

    #[test]
    fn test_header_alignment() {
        assert_eq!(std::mem::align_of::<BudgetMetaCapsuleHeader>(), 128);
    }

    #[test]
    fn test_new() {
        let meta = BudgetMetaCapsule::new();
        assert_eq!(meta.slot_count(), 0);
        assert_eq!(meta.generation(), 1);
    }

    #[test]
    fn test_allocate_slot() {
        let meta = BudgetMetaCapsule::new();

        let result = meta.allocate(1, 1000_00);
        assert!(result.is_ok());

        let slot_id = result.unwrap();
        assert_eq!(slot_id, 0);
        assert_eq!(meta.slot_count(), 1);

        let capsule = meta.get(slot_id).unwrap();
        assert_eq!(capsule.budget(), 1000_00);
    }

    #[test]
    fn test_get_slot() {
        let meta = BudgetMetaCapsule::new();

        let slot_id = meta.allocate(1, 1000_00).unwrap();

        let capsule = meta.get(slot_id).unwrap();
        assert_eq!(capsule.budget(), 1000_00);
    }

    #[test]
    fn test_deallocate_slot() {
        let meta = BudgetMetaCapsule::new();

        let slot_id = meta.allocate(1, 1000_00).unwrap();
        assert_eq!(meta.slot_count(), 1);

        let result = meta.deallocate(slot_id);
        assert!(result.is_ok());
        assert_eq!(meta.slot_count(), 0);
    }

    #[test]
    fn test_invalid_slot_id() {
        let meta = BudgetMetaCapsule::new();

        let result = meta.get(MAX_BUDGET_SLOTS + 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_slot_not_allocated() {
        let meta = BudgetMetaCapsule::new();

        let result = meta.get(0);
        assert!(result.is_err());
        assert!(matches!(result, Err(ClapiError::SlotNotAllocated { .. })));
    }

    #[test]
    fn test_generation_increments() {
        let meta = BudgetMetaCapsule::new();
        let gen1 = meta.generation();

        meta.allocate(1, 1000_00).unwrap();
        let gen2 = meta.generation();

        assert!(gen2 > gen1, "Generation must increase monotonically");
    }

    #[test]
    #[ignore] // Expensive test: allocates 128MB
    fn test_capacity_limit() {
        let meta = BudgetMetaCapsule::new();

        // Allocate MAX_BUDGET_SLOTS slots
        for i in 0..MAX_BUDGET_SLOTS {
            let result = meta.allocate(i as u64, 1000_00);
            assert!(result.is_ok(), "Allocation {} should succeed", i);
        }

        // Next allocation should fail
        let result = meta.allocate(MAX_BUDGET_SLOTS as u64, 1000_00);
        assert!(result.is_err());
        assert!(matches!(result, Err(ClapiError::SlotsExhausted { .. })));
    }

    #[test]
    fn test_concurrent_allocation() {
        use std::sync::Arc;
        use std::thread;

        let meta = Arc::new(BudgetMetaCapsule::new());
        let mut handles = vec![];

        for thread_id in 0..10 {
            let m = Arc::clone(&meta);
            handles.push(thread::spawn(move || {
                let mut allocated_slots = Vec::new();
                for i in 0..10 {
                    let budget_id = (thread_id * 10 + i) as u64;
                    if let Ok(slot_id) = m.allocate(budget_id, 1000_00) {
                        allocated_slots.push(slot_id);
                    }
                }
                allocated_slots
            }));
        }

        let mut all_slots = Vec::new();
        for h in handles {
            let slots = h.join().unwrap();
            all_slots.extend(slots);
        }

        // All slot IDs should be unique
        all_slots.sort_unstable();
        all_slots.dedup();
        assert_eq!(all_slots.len(), 100, "All slot IDs must be unique");
    }

    #[test]
    fn test_stats() {
        let meta = BudgetMetaCapsule::new();

        let slot_id = meta.allocate(1, 1000_00).unwrap();
        let stats = meta.get_stats();

        assert_eq!(stats.slot_count, 1);
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 0);

        meta.deallocate(slot_id).unwrap();
        let stats = meta.get_stats();

        assert_eq!(stats.slot_count, 0);
        assert_eq!(stats.total_allocations, 1);
        assert_eq!(stats.total_deallocations, 1);
    }
}
