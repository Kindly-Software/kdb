//! B32-Compliant Benchmark: Slot-Based Lockfree vs Current RwLock+HashMap Implementation
//!
//! **Framework**: B32 (32 benchmarking guidelines + 50 hardware reality checks)
//! **Baseline**: CURRENT BudgetRegistry (RwLock<HashMap> + lockfree capsules)
//! **Comparison**: HYPOTHETICAL slot-based pure lockfree (AtomicPtr array)
//!
//! ## Architecture Comparison
//!
//! ### Current: RwLock<HashMap<BudgetId, Arc<RequestCapsule128>>>
//! - Fast path (99%): Read lock + HashMap lookup → Arc<Capsule> → lockfree atomic CAS
//! - Slow path (1%): Write lock + HashMap insert
//! - Performance: ~80ns deduction (read lock + hash + atomic CAS)
//!
//! ### Hypothetical: Vec<AtomicPtr<RequestCapsule128>> (Pure Lockfree)
//! - Fixed-size array of slots (e.g., 10,000 slots)
//! - AtomicPtr::load() → lockfree atomic CAS
//! - No locks whatsoever
//! - Performance: ~50ns deduction (atomic load + atomic CAS)
//!
//! ## Expected Results (B32 Reality Checks)
//!
//! | Operation | Slot-Based | Current (RwLock) | Speedup | Reality Check |
//! |-----------|-----------|------------------|---------|---------------|
//! | Single allocation | ~50ns | ~80ns | 1.6× | K2: Eliminate read lock (25ns) |
//! | Single deduction | ~50ns | ~80ns | 1.6× | K2: AtomicPtr load vs RwLock read |
//! | Single read | ~30ns | ~50ns | 1.7× | K2: Direct atomic vs lock+hash |
//! | Concurrent (4T) | ~70ns | ~120ns | 1.7× | K12: No lock contention |
//! | Circuit breaker check | ~5ns | ~5ns | 1.0× | K2: Both are atomic loads |
//!
//! **B32 K27 Reality**: 1.5-2× speedup is REALISTIC (not 10×!)
//! - Eliminating read lock saves ~25ns per operation
//! - HashMap lookup ~10-20ns vs array indexing ~5ns
//! - Tradeoff: Fixed capacity vs dynamic growth
//!
//! ## B32 Compliance
//!
//! - **B1: Fair Baseline**: Current production implementation (NOT strawman)
//! - **B2: Statistical Rigor**: 95% CI, 1000+ samples, Criterion default
//! - **B3: Realistic Workloads**: Production budget patterns
//! - **B4: Contention Scenarios**: 1/4/8 thread scaling tests
//! - **B5: Full Disclosure**: Complete methodology documentation

use clapi_core::proxy::BudgetRegistry;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use std::sync::atomic::{AtomicPtr, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Import capsule for hypothetical slot-based implementation
use clapi_core::capsules::RequestCapsule128;

// ============================================================================
// Hypothetical: Slot-Based Pure Lockfree Budget Registry
// ============================================================================

/// Slot-based budget registry using AtomicPtr for 100% lockfree access
///
/// **Architecture**:
/// - Fixed-size array of AtomicPtr<RequestCapsule128>
/// - BudgetId maps directly to slot index (BudgetId % CAPACITY)
/// - No locks, no HashMap, just atomic pointer operations
///
/// **Advantages**:
/// - No read lock overhead (~25ns savings)
/// - No hash computation (~10-20ns savings)
/// - Direct array indexing (~5ns)
///
/// **Disadvantages**:
/// - Fixed capacity (cannot grow)
/// - Hash collisions require chaining or linear probing
/// - Memory overhead for sparse BudgetId space
///
/// **Expected Performance**:
/// - Allocation: ~50ns (vs ~80ns current)
/// - Deduction: ~50ns (vs ~80ns current)
/// - Read: ~30ns (vs ~50ns current)
/// - Speedup: 1.5-2× (B32 K27: realistic range)
struct SlotBasedBudgetRegistry {
    /// Fixed-size array of budget slots
    slots: Vec<AtomicPtr<RequestCapsule128>>,

    /// Default budget for new users
    default_budget: i64,

    /// Generation counter for slot allocation
    allocation_gen: AtomicU64,
}

impl SlotBasedBudgetRegistry {
    /// Create new slot-based registry with fixed capacity
    ///
    /// # Arguments
    /// - `capacity`: Maximum number of budget slots (e.g., 10,000)
    /// - `default_budget`: Default budget for new users (cents)
    fn new(capacity: usize, default_budget: i64) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(AtomicPtr::new(std::ptr::null_mut()));
        }

        Self {
            slots,
            default_budget,
            allocation_gen: AtomicU64::new(0),
        }
    }

    /// Get slot index for budget ID (simple modulo mapping)
    #[inline]
    fn slot_index(&self, budget_id: u64) -> usize {
        (budget_id as usize) % self.slots.len()
    }

    /// Get or allocate budget capsule (100% lockfree)
    ///
    /// # Performance
    /// - Fast path: AtomicPtr::load() (~5ns)
    /// - Slow path: Allocate + CAS (~50ns)
    ///
    /// # Safety
    /// - #ASSUME: AtomicPtr::compare_exchange prevents double allocation
    /// - #VERIFY: Only one thread allocates per slot
    fn get_or_allocate(&self, budget_id: u64) -> *const RequestCapsule128 {
        let idx = self.slot_index(budget_id);
        let slot = &self.slots[idx];

        // Fast path: Load existing capsule
        let ptr = slot.load(Ordering::Acquire);
        if !ptr.is_null() {
            return ptr;
        }

        // Slow path: Allocate new capsule
        let new_capsule = Box::into_raw(Box::new(RequestCapsule128::new(self.default_budget)));

        match slot.compare_exchange(
            std::ptr::null_mut(),
            new_capsule,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                self.allocation_gen.fetch_add(1, Ordering::Relaxed);
                new_capsule
            }
            Err(existing) => {
                // Another thread won the race - free our allocation
                unsafe {
                    let _ = Box::from_raw(new_capsule);
                }
                existing
            }
        }
    }

    /// Try to deduct from budget (100% lockfree)
    ///
    /// # Performance
    /// - Expected: ~50ns (AtomicPtr load + RequestCapsule128::try_deduct)
    /// - vs Current: ~80ns (read lock + hash + try_deduct)
    /// - Speedup: ~1.6× (B32 K27: realistic)
    fn try_deduct(&self, budget_id: u64, amount: i64) -> Result<i64, ()> {
        let capsule_ptr = self.get_or_allocate(budget_id);

        unsafe { (*capsule_ptr).try_deduct(amount).map_err(|_| ()) }
    }

    /// Get current budget (100% lockfree read)
    ///
    /// # Performance
    /// - Expected: ~30ns (AtomicPtr load + atomic load)
    /// - vs Current: ~50ns (read lock + hash + atomic load)
    /// - Speedup: ~1.7× (B32 K27: realistic)
    #[inline]
    fn get_budget(&self, budget_id: u64) -> Option<i64> {
        let idx = self.slot_index(budget_id);
        let ptr = self.slots[idx].load(Ordering::Acquire);

        if ptr.is_null() {
            None
        } else {
            unsafe { Some((*ptr).budget()) }
        }
    }

    /// Allows operation (circuit breaker check)
    ///
    /// # Performance
    /// - Expected: ~5ns (single atomic load)
    /// - vs Current: ~5ns (both are atomic loads)
    /// - Speedup: ~1.0× (no difference)
    #[inline]
    fn allows_operation(&self, budget_id: u64) -> bool {
        self.get_budget(budget_id).unwrap_or(self.default_budget) > 0
    }
}

impl Drop for SlotBasedBudgetRegistry {
    fn drop(&mut self) {
        // Clean up all allocated capsules
        for slot in &self.slots {
            let ptr = slot.load(Ordering::Acquire);
            if !ptr.is_null() {
                unsafe {
                    let _ = Box::from_raw(ptr);
                }
            }
        }
    }
}

// ============================================================================
// B2: Benchmark 1 - Single-Threaded Allocation
// ============================================================================

/// Benchmark 1: Single-threaded slot allocation
///
/// **Expected**: Slot-based ~50ns, Current ~80ns (1.6× speedup)
/// **Reality Check (K2)**: AtomicPtr CAS ~15ns + capsule overhead vs read lock ~25ns
fn bench_slot_allocate(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_single_allocate");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    // Slot-based implementation
    group.bench_function("slot_based", |b| {
        let registry = SlotBasedBudgetRegistry::new(10_000, 1_000_000_00);
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let budget_id = counter % 100;
            black_box(registry.try_deduct(budget_id, 1))
        });
    });

    // Current implementation (RwLock + HashMap)
    group.bench_function("current_rwlock", |b| {
        let registry = BudgetRegistry::new(1_000_000_00);
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let budget_id = counter % 100;
            black_box(registry.try_deduct(budget_id, 1))
        });
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 2 - Single-Threaded Deduction
// ============================================================================

/// Benchmark 2: Single-threaded deduction (hot path)
///
/// **Expected**: Slot-based ~50ns, Current ~80ns (1.6× speedup)
/// **Reality Check (K2)**: Eliminate read lock overhead
fn bench_slot_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_single_deduct");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    let budget_ids: Vec<u64> = (0..100).collect();

    // Slot-based implementation
    group.bench_function("slot_based", |b| {
        let registry = SlotBasedBudgetRegistry::new(10_000, 1_000_000_00);
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            black_box(registry.try_deduct(budget_id, 100_00))
        });
    });

    // Current implementation
    group.bench_function("current_rwlock", |b| {
        let registry = BudgetRegistry::new(1_000_000_00);
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            black_box(registry.try_deduct(budget_id, 100_00))
        });
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 3 - Single-Threaded Read
// ============================================================================

/// Benchmark 3: Single-threaded read (budget query)
///
/// **Expected**: Slot-based ~30ns, Current ~50ns (1.7× speedup)
/// **Reality Check (K2)**: Direct atomic load vs read lock + hash lookup
fn bench_slot_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_single_read");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    let budget_ids: Vec<u64> = (0..100).collect();

    // Slot-based implementation
    group.bench_function("slot_based", |b| {
        let registry = SlotBasedBudgetRegistry::new(10_000, 1_000_000_00);
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            black_box(registry.get_budget(budget_id))
        });
    });

    // Current implementation
    group.bench_function("current_rwlock", |b| {
        let registry = BudgetRegistry::new(1_000_000_00);
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            black_box(registry.get_budget(budget_id))
        });
    });

    group.finish();
}

// ============================================================================
// B2: Benchmark 4 - Circuit Breaker Check
// ============================================================================

/// Benchmark 4: Circuit breaker check (allows_operation)
///
/// **Expected**: Slot-based ~5ns, Current ~5ns (1.0× - no difference)
/// **Reality Check (K2)**: Both are atomic loads
fn bench_circuit_breaker_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("circuit_breaker_check");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(1000);
    group.throughput(Throughput::Elements(1000));

    let budget_ids: Vec<u64> = (0..100).collect();

    // Slot-based implementation
    group.bench_function("slot_based", |b| {
        let registry = SlotBasedBudgetRegistry::new(10_000, 1_000_000_00);
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            black_box(registry.allows_operation(budget_id))
        });
    });

    // Current implementation (would need circuit breaker method)
    group.bench_function("current_rwlock", |b| {
        let registry = BudgetRegistry::new(1_000_000_00);
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        let mut counter = 0usize;
        b.iter(|| {
            counter += 1;
            let budget_id = budget_ids[counter % 100];
            // Simulate circuit breaker: cheap budget check
            black_box(registry.get_budget(budget_id).unwrap_or(0) > 0)
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 5 - Concurrent Allocation (4 Threads)
// ============================================================================

/// Benchmark 5: Concurrent allocation with 4 threads
///
/// **Expected**: Slot-based ~70ns, Current ~120ns (1.7× speedup)
/// **Reality Check (K12)**: Lockfree sweet spot, no read lock contention
fn bench_concurrent_allocation_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_concurrent_allocation_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 1000;

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // Slot-based implementation
    group.bench_function("slot_based_4t", |b| {
        let registry = Arc::new(SlotBasedBudgetRegistry::new(10_000, 1_000_000_00));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = (tid * 1000 + i) % 100;
                            let _ = r.try_deduct(budget_id, 1);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Current implementation
    group.bench_function("current_rwlock_4t", |b| {
        let registry = Arc::new(BudgetRegistry::new(1_000_000_00));
        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = (tid * 1000 + i) % 100;
                            let _ = r.try_deduct(budget_id, 1);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B4: Benchmark 6 - Concurrent Reads (8 Threads)
// ============================================================================

/// Benchmark 6: Concurrent reads with 8 threads
///
/// **Expected**: Slot-based ~40ns, Current ~60ns (1.5× speedup)
/// **Reality Check (K12)**: No read lock contention in slot-based
fn bench_concurrent_reads_8t(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_concurrent_reads_8t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 8;
    let ops_per_thread = 2000;
    let budget_ids: Vec<u64> = (0..100).collect();

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // Slot-based implementation
    group.bench_function("slot_based_8t", |b| {
        let registry = Arc::new(SlotBasedBudgetRegistry::new(10_000, 1_000_000_00));
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    let ids = budget_ids.clone();
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = ids[(tid * 2000 + i) % 100];
                            let _ = r.get_budget(budget_id);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Current implementation
    group.bench_function("current_rwlock_8t", |b| {
        let registry = Arc::new(BudgetRegistry::new(1_000_000_00));
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    let ids = budget_ids.clone();
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = ids[(tid * 2000 + i) % 100];
                            let _ = r.get_budget(budget_id);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B3: Benchmark 7 - Mixed Workload (75% Reads, 25% Writes)
// ============================================================================

/// Benchmark 7: Mixed workload with 75% reads, 25% writes
///
/// **Expected**: Slot-based ~60ns, Current ~90ns (1.5× speedup)
/// **Reality Check (K27)**: Typical optimization for read-heavy workload
fn bench_mixed_workload_4t(c: &mut Criterion) {
    let mut group = c.benchmark_group("slot_mixed_workload_4t");
    group.warm_up_time(Duration::from_secs(3));
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(100);

    let num_threads = 4;
    let ops_per_thread = 2000;
    let budget_ids: Vec<u64> = (0..100).collect();

    group.throughput(Throughput::Elements((num_threads * ops_per_thread) as u64));

    // Slot-based implementation
    group.bench_function("slot_based_mixed", |b| {
        let registry = Arc::new(SlotBasedBudgetRegistry::new(10_000, 100_000_000_00));
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    let ids = budget_ids.clone();
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = ids[(tid * 2000 + i) % 100];

                            // 75% reads, 25% deductions
                            if i % 4 == 0 {
                                let _ = r.try_deduct(budget_id, 50_00);
                            } else {
                                let _ = r.get_budget(budget_id);
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    // Current implementation
    group.bench_function("current_rwlock_mixed", |b| {
        let registry = Arc::new(BudgetRegistry::new(100_000_000_00));
        // Pre-populate
        for &id in &budget_ids {
            registry.try_deduct(id, 1).ok();
        }

        b.iter(|| {
            let handles: Vec<_> = (0..num_threads)
                .map(|tid| {
                    let r = Arc::clone(&registry);
                    let ids = budget_ids.clone();
                    thread::spawn(move || {
                        for i in 0..ops_per_thread {
                            let budget_id = ids[(tid * 2000 + i) % 100];

                            // 75% reads, 25% deductions
                            if i % 4 == 0 {
                                let _ = r.try_deduct(budget_id, 50_00);
                            } else {
                                let _ = r.get_budget(budget_id);
                            }
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// B2: Criterion Configuration (Statistical Rigor)
// ============================================================================

criterion_group! {
    name = benches;
    config = Criterion::default()
        .confidence_level(0.95)      // B2: 95% confidence intervals
        .significance_level(0.05)
        .noise_threshold(0.05);
    targets =
        bench_slot_allocate,
        bench_slot_get,
        bench_slot_read,
        bench_circuit_breaker_check,
        bench_concurrent_allocation_4t,
        bench_concurrent_reads_8t,
        bench_mixed_workload_4t
}

criterion_main!(benches);
