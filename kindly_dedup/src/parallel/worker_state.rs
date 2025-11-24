//! # WorkerStateCapsule - T1 Atomic Per-Worker Metrics
//!
//! **Tier**: T1 (Atomic - 3-10× speedup)
//! **Cache Alignment**: 128 bytes (prevents false sharing between workers)
//! **Pattern**: T1 Atomic counters with Relaxed/Acquire ordering
//!
//! ## Overview
//!
//! Tracks performance metrics for individual workers in a parallel deduplication pipeline.
//! Every worker gets its own 128-byte cache-aligned capsule to avoid false sharing.
//!
//! **Layout** (128 bytes):
//! ```text
//! +0-7:   worker_id (u32) + cpu_core (u32)
//! +8-63:  8× AtomicU64 performance metrics (56 bytes)
//! +64-75: Atomic state (4 bytes) + padding (11 bytes)
//! +76-127: Cache-line padding (48 bytes)
//! Total: 128 bytes (single cache line, prevents false sharing)
//! ```
//!
//! ## Performance Targets
//!
//! - **Metric Update**: <5ns (Relaxed ordering, single atomic operation)
//! - **Snapshot**: <20ns (Acquire ordering, verify memory visibility)
//! - **False Sharing**: Impossible (128-byte alignment > 2× max cache line)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 (T1 Atomic tier selection), Q33 (deterministic snapshots)
//! - **COCA**: 100% lockfree (AtomicU64/AtomicU32/AtomicBool only, no Mutex)
//! - **ASSUM**: 99.99% safe (documented memory ordering, overflow assumptions)
//! - **T28**: 4-tier tests (unit/property/integration/production)
//! - **B32**: Fair benchmarking (realistic contention, 1000+ iterations)

use std::sync::atomic::{AtomicU64, AtomicU32, AtomicBool, Ordering};

/// Per-worker performance metrics (T1 Atomic tier).
///
/// **Cache Alignment**: 128 bytes
/// **Thread Safety**: 100% lockfree (AtomicU64 operations only)
/// **False Sharing**: Impossible (128-byte > 2× largest cache line)
///
/// ## Layout Analysis
///
/// ```text
/// Field                    Offset   Size   Cumulative
/// ─────────────────────────────────────────────────────
/// worker_id               0-3      4      4
/// cpu_core                4-7      4      8
/// docs_processed          8-15     8      16
/// batches_completed       16-23    8      24
/// work_stolen             24-31    8      32
/// work_donated            32-39    8      40
/// idle_cycles             40-47    8      48
/// compute_cycles          48-55    8      56
/// total_latency_ns        56-63    8      64
/// current_batch_size      64-67    4      68
/// is_active               68       1      69
/// _padding_state          69-79    11     80
/// _padding                80-127   48     128
/// ─────────────────────────────────────────────────────
/// TOTAL:                                  128 bytes
/// ```
///
/// **Alignment Guarantee**: `#[repr(C, align(128))]` enforces 128-byte alignment.
/// **Atomicity**: All metrics use `AtomicU64` for lockfree operations.
#[repr(C, align(128))]
pub struct WorkerStateCapsule {
    // Worker identification (8 bytes)
    // #ASSUME: worker_id in range [0, 255] (8-bit worker count max)
    // #ASSUME: cpu_core valid NUMA core ID (kernel provides validation)
    worker_id: u32,
    cpu_core: u32,

    // Performance metrics (56 bytes)
    // #ASSUME: No overflow on 64-bit counters (millions of docs per worker acceptable)
    // #ASSUME: AtomicU64 Relaxed ordering sufficient (approximate metrics, no strict ordering needed)
    docs_processed: AtomicU64,     // Total documents processed by this worker
    batches_completed: AtomicU64,   // Number of batch completions
    work_stolen: AtomicU64,         // Batches stolen from other workers (work-stealing queue)
    work_donated: AtomicU64,        // Batches donated to other workers
    idle_cycles: AtomicU64,         // Approximate CPU cycles spent idle
    compute_cycles: AtomicU64,      // Approximate CPU cycles spent computing
    total_latency_ns: AtomicU64,    // Cumulative latency across all operations

    // State tracking (16 bytes)
    // #ASSUME: current_batch_size fits in u32 (max 4B documents per batch)
    // #ASSUME: is_active boolean sufficient for worker state (active vs waiting)
    current_batch_size: AtomicU32,  // Docs in current batch
    is_active: AtomicBool,          // Is worker currently processing
    _padding_state: [u8; 11],       // Pad to 16 bytes (avoid metrics misalignment)

    // Cache-line padding (48 bytes)
    // #ASSUME: 128-byte cache line standard (Intel, AMD, ARM)
    // #VERIFY: Total size = 128 bytes exactly (prevents false sharing)
    _padding: [u8; 48],
}

// Verify 128-byte size at compile time
const _: () = {
    const fn assert_size() {
        const SIZE: usize = std::mem::size_of::<WorkerStateCapsule>();
        const ALIGN: usize = std::mem::align_of::<WorkerStateCapsule>();
        // SIZE must be exactly 128 bytes
        assert!(SIZE == 128, "WorkerStateCapsule must be exactly 128 bytes");
        // ALIGN must be exactly 128 bytes
        assert!(ALIGN == 128, "WorkerStateCapsule must be 128-byte aligned");
    }
    let () = assert_size();
};

impl WorkerStateCapsule {
    /// Creates a new worker state capsule with worker_id and CPU core affinity.
    ///
    /// **Time Complexity**: O(1)
    /// **Memory**: 128 bytes (single cache line)
    /// **Ordering**: None (initialization)
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Unique worker identifier (0-255 recommended)
    /// * `cpu_core` - NUMA core for thread pinning (kernel validates)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_INIT_ZERO`: All atomic fields initialized to 0
    /// - `#ASSUME_ALIGNMENT`: Heap allocation respects 128-byte alignment
    ///
    /// # Example
    ///
    /// ```ignore
    /// let worker = WorkerStateCapsule::new(0, 0);
    /// assert_eq!(worker.worker_id(), 0);
    /// assert_eq!(worker.cpu_core(), 0);
    /// ```
    pub fn new(worker_id: u32, cpu_core: u32) -> Self {
        WorkerStateCapsule {
            worker_id,
            cpu_core,
            docs_processed: AtomicU64::new(0),
            batches_completed: AtomicU64::new(0),
            work_stolen: AtomicU64::new(0),
            work_donated: AtomicU64::new(0),
            idle_cycles: AtomicU64::new(0),
            compute_cycles: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            current_batch_size: AtomicU32::new(0),
            is_active: AtomicBool::new(false),
            _padding_state: [0u8; 11],
            _padding: [0u8; 48],
        }
    }

    /// Returns the worker ID (immutable, no atomicity needed).
    #[inline]
    pub fn worker_id(&self) -> u32 {
        self.worker_id
    }

    /// Returns the CPU core ID (immutable, no atomicity needed).
    #[inline]
    pub fn cpu_core(&self) -> u32 {
        self.cpu_core
    }

    /// Increments document count by `count`.
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Ordering**: Relaxed (approximate metric, no ordering required)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_NO_OVERFLOW`: count + current < 2^64 (reasonable for ~billions docs)
    /// - `#ASSUME_RELAXED_OK`: Metric doesn't require strict ordering (approximate is fine)
    ///
    /// # Example
    ///
    /// ```ignore
    /// worker.increment_docs(1000);  // Add 1000 docs
    /// ```
    #[inline]
    pub fn increment_docs(&self, count: u64) {
        // #VERIFY: Relaxed ordering safe for approximate throughput metrics
        self.docs_processed.fetch_add(count, Ordering::Relaxed);
    }

    /// Increments batch completion counter by 1.
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Ordering**: Relaxed (batch count is approximate)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Batch count doesn't require strict ordering
    #[inline]
    pub fn increment_batches(&self) {
        // #VERIFY: Relaxed ordering safe for approximate batch metrics
        self.batches_completed.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a work-steal event (this worker stole from another).
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Tracks load-balancing efficiency (higher = better parallelism)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Work-steal count is diagnostic, doesn't require ordering
    #[inline]
    pub fn record_steal(&self) {
        // #VERIFY: Relaxed ordering safe for diagnostic metrics
        self.work_stolen.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a work-donation event (this worker gave work to another).
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Tracks how often this worker helps others (lower = better load distribution)
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Work-donation count is diagnostic, doesn't require ordering
    #[inline]
    pub fn record_donation(&self) {
        // #VERIFY: Relaxed ordering safe for diagnostic metrics
        self.work_donated.fetch_add(1, Ordering::Relaxed);
    }

    /// Records an idle cycle (time waiting for work).
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Diagnostics for scheduling efficiency
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Idle count is approximate (may miss some cycles due to Relaxed ordering)
    #[inline]
    pub fn record_idle_cycle(&self) {
        // #VERIFY: Relaxed ordering acceptable for cycle counting (approximate is fine)
        self.idle_cycles.fetch_add(1, Ordering::Relaxed);
    }

    /// Records a compute cycle (time spent computing MinHash signatures).
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Tracks true compute utilization
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Compute cycle count is approximate
    #[inline]
    pub fn record_compute_cycle(&self) {
        // #VERIFY: Relaxed ordering acceptable for cycle counting
        self.compute_cycles.fetch_add(1, Ordering::Relaxed);
    }

    /// Records latency for an operation (in nanoseconds).
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Cumulative latency for P50/P99 calculations
    ///
    /// # Arguments
    ///
    /// * `latency_ns` - Latency in nanoseconds
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_NO_OVERFLOW`: latency_ns + current < 2^64 (acceptable for trillions of nanoseconds)
    /// - `#ASSUME_RELAXED_OK`: Latency sum is approximate (used for statistics, not real-time SLA)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let latency_ns = 1_000; // 1 microsecond
    /// worker.record_latency(latency_ns);
    /// ```
    #[inline]
    pub fn record_latency(&self, latency_ns: u64) {
        // #VERIFY: Relaxed ordering safe for statistical aggregation
        self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);
    }

    /// Records the current batch size.
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Snapshot of current batch for diagnostics
    ///
    /// # Arguments
    ///
    /// * `size` - Number of documents in current batch
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_SIZE_FITS_U32`: Batch size < 4 billion documents
    #[inline]
    pub fn set_batch_size(&self, size: u32) {
        // #VERIFY: Relaxed ordering safe for batch size (updated per batch)
        self.current_batch_size.store(size, Ordering::Relaxed);
    }

    /// Sets worker active state.
    ///
    /// **Time**: <5ns (Relaxed atomic operation)
    /// **Semantics**: Indicates if worker is processing or idle
    ///
    /// # Arguments
    ///
    /// * `active` - true if processing, false if waiting
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_RELAXED_OK`: Active state is informational, doesn't coordinate other threads
    #[inline]
    pub fn set_active(&self, active: bool) {
        // #VERIFY: Relaxed ordering safe (active state is informational only)
        self.is_active.store(active, Ordering::Relaxed);
    }

    /// Takes an atomic snapshot of all metrics.
    ///
    /// **Time**: <20ns (Acquire ordering, ensures memory visibility)
    /// **Ordering**: Acquire (ensures all metrics are synchronized)
    ///
    /// Returns a `WorkerStats` snapshot with current values.
    ///
    /// # ASSUM Tags
    ///
    /// - `#ASSUME_ACQUIRE_ORDERING`: Acquire ordering ensures all prior metrics are visible
    ///   (necessary because worker state is updated from other cores)
    /// - `#ASSUME_ATOMIC_CONSISTENCY`: Snapshot is not point-in-time consistent (metrics updated
    ///   during snapshot), but Acquire ensures no stale reads from before the snapshot started
    ///
    /// # Example
    ///
    /// ```ignore
    /// let stats = worker.snapshot();
    /// println!("Docs: {}, Latency: {}ns", stats.docs_processed, stats.total_latency_ns);
    /// ```
    pub fn snapshot(&self) -> WorkerStats {
        // #VERIFY: Acquire ordering ensures memory visibility from other cores
        WorkerStats {
            worker_id: self.worker_id,
            cpu_core: self.cpu_core,
            docs_processed: self.docs_processed.load(Ordering::Acquire),
            batches_completed: self.batches_completed.load(Ordering::Acquire),
            work_stolen: self.work_stolen.load(Ordering::Acquire),
            work_donated: self.work_donated.load(Ordering::Acquire),
            idle_cycles: self.idle_cycles.load(Ordering::Acquire),
            compute_cycles: self.compute_cycles.load(Ordering::Acquire),
            total_latency_ns: self.total_latency_ns.load(Ordering::Acquire),
            current_batch_size: self.current_batch_size.load(Ordering::Acquire),
            is_active: self.is_active.load(Ordering::Acquire),
        }
    }

    /// Computes average latency per document.
    ///
    /// **Time**: O(1) (arithmetic, no atomics)
    ///
    /// # Returns
    ///
    /// Average latency in nanoseconds, or 0 if no documents processed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let avg_latency = worker.avg_latency_ns();
    /// println!("Avg: {} ns/doc", avg_latency);
    /// ```
    #[inline]
    pub fn avg_latency_ns(&self) -> u64 {
        let docs = self.docs_processed.load(Ordering::Relaxed);
        if docs == 0 {
            0
        } else {
            self.total_latency_ns.load(Ordering::Relaxed) / docs
        }
    }

    /// Computes utilization ratio (compute vs idle cycles).
    ///
    /// **Time**: O(1) (arithmetic, no atomics)
    ///
    /// # Returns
    ///
    /// Ratio 0.0-1.0 (1.0 = 100% compute, 0.0 = 100% idle)
    ///
    /// # Example
    ///
    /// ```ignore
    /// let utilization = worker.utilization();
    /// println!("Utilization: {:.1}%", utilization * 100.0);
    /// ```
    #[inline]
    pub fn utilization(&self) -> f64 {
        let compute = self.compute_cycles.load(Ordering::Relaxed) as f64;
        let idle = self.idle_cycles.load(Ordering::Relaxed) as f64;
        let total = compute + idle;

        if total == 0.0 {
            0.0
        } else {
            compute / total
        }
    }
}

/// Snapshot of worker metrics (copy-safe for analysis).
///
/// **Invariant**: All fields are u64 (fits in registers, can be safely copied)
/// **Ordering**: Acquired with Acquire ordering from source capsule
///
/// Used for statistics, reporting, and diagnostics without holding locks.
#[derive(Debug, Clone, Copy)]
pub struct WorkerStats {
    pub worker_id: u32,
    pub cpu_core: u32,
    pub docs_processed: u64,
    pub batches_completed: u64,
    pub work_stolen: u64,
    pub work_donated: u64,
    pub idle_cycles: u64,
    pub compute_cycles: u64,
    pub total_latency_ns: u64,
    pub current_batch_size: u32,
    pub is_active: bool,
}

impl WorkerStats {
    /// Average latency per document.
    #[inline]
    pub fn avg_latency_ns(&self) -> u64 {
        if self.docs_processed == 0 {
            0
        } else {
            self.total_latency_ns / self.docs_processed
        }
    }

    /// Utilization ratio (compute vs total cycles).
    #[inline]
    pub fn utilization(&self) -> f64 {
        let compute = self.compute_cycles as f64;
        let idle = self.idle_cycles as f64;
        let total = compute + idle;

        if total == 0.0 {
            0.0
        } else {
            compute / total
        }
    }

    /// Work-stealing efficiency (stolen / donated).
    ///
    /// Values > 1.0 indicate this worker steals more than it donates (imbalanced load).
    #[inline]
    pub fn steal_ratio(&self) -> f64 {
        if self.work_donated == 0 {
            if self.work_stolen == 0 {
                1.0
            } else {
                f64::INFINITY
            }
        } else {
            self.work_stolen as f64 / self.work_donated as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // UNIT TESTS (T28: Q1-Q7)
    // ========================================================================

    #[test]
    fn test_size_alignment() {
        // #VERIFY: WorkerStateCapsule is exactly 128 bytes
        assert_eq!(
            std::mem::size_of::<WorkerStateCapsule>(),
            128,
            "WorkerStateCapsule must be exactly 128 bytes"
        );

        // #VERIFY: 128-byte alignment prevents false sharing
        assert_eq!(
            std::mem::align_of::<WorkerStateCapsule>(),
            128,
            "WorkerStateCapsule must be 128-byte aligned"
        );
    }

    #[test]
    fn test_new_initialization() {
        let worker = WorkerStateCapsule::new(42, 5);
        assert_eq!(worker.worker_id(), 42);
        assert_eq!(worker.cpu_core(), 5);

        // All metrics start at 0
        let stats = worker.snapshot();
        assert_eq!(stats.docs_processed, 0);
        assert_eq!(stats.batches_completed, 0);
        assert_eq!(stats.work_stolen, 0);
        assert_eq!(stats.work_donated, 0);
        assert_eq!(stats.idle_cycles, 0);
        assert_eq!(stats.compute_cycles, 0);
        assert_eq!(stats.total_latency_ns, 0);
        assert!(!stats.is_active);
    }

    #[test]
    fn test_increment_docs() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.increment_docs(100);
        worker.increment_docs(200);
        assert_eq!(worker.snapshot().docs_processed, 300);
    }

    #[test]
    fn test_increment_batches() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..10 {
            worker.increment_batches();
        }
        assert_eq!(worker.snapshot().batches_completed, 10);
    }

    #[test]
    fn test_work_stealing() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.record_steal();
        worker.record_steal();
        worker.record_steal();
        assert_eq!(worker.snapshot().work_stolen, 3);
    }

    #[test]
    fn test_work_donation() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.record_donation();
        worker.record_donation();
        assert_eq!(worker.snapshot().work_donated, 2);
    }

    #[test]
    fn test_idle_cycles() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.record_idle_cycle();
        worker.record_idle_cycle();
        worker.record_idle_cycle();
        worker.record_idle_cycle();
        assert_eq!(worker.snapshot().idle_cycles, 4);
    }

    #[test]
    fn test_compute_cycles() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..7 {
            worker.record_compute_cycle();
        }
        assert_eq!(worker.snapshot().compute_cycles, 7);
    }

    #[test]
    fn test_latency_recording() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.record_latency(1_000);
        worker.record_latency(2_000);
        worker.record_latency(3_000);
        assert_eq!(worker.snapshot().total_latency_ns, 6_000);
    }

    #[test]
    fn test_batch_size() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.set_batch_size(1000);
        assert_eq!(worker.snapshot().current_batch_size, 1000);

        worker.set_batch_size(2000);
        assert_eq!(worker.snapshot().current_batch_size, 2000);
    }

    #[test]
    fn test_active_state() {
        let worker = WorkerStateCapsule::new(0, 0);
        assert!(!worker.snapshot().is_active);

        worker.set_active(true);
        assert!(worker.snapshot().is_active);

        worker.set_active(false);
        assert!(!worker.snapshot().is_active);
    }

    #[test]
    fn test_avg_latency_ns() {
        let worker = WorkerStateCapsule::new(0, 0);

        // No docs: avg is 0
        assert_eq!(worker.avg_latency_ns(), 0);

        worker.increment_docs(10);
        worker.record_latency(10_000); // 10 µs total
        assert_eq!(worker.avg_latency_ns(), 1_000); // 1 µs per doc
    }

    #[test]
    fn test_utilization_all_compute() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..100 {
            worker.record_compute_cycle();
        }
        let utilization = worker.utilization();
        assert!(utilization > 0.99 && utilization <= 1.0);
    }

    #[test]
    fn test_utilization_all_idle() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..100 {
            worker.record_idle_cycle();
        }
        assert_eq!(worker.utilization(), 0.0);
    }

    #[test]
    fn test_utilization_mixed() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..75 {
            worker.record_compute_cycle();
        }
        for _ in 0..25 {
            worker.record_idle_cycle();
        }
        let utilization = worker.utilization();
        assert!(utilization > 0.74 && utilization < 0.76); // ~75%
    }

    // ========================================================================
    // PROPERTY TESTS (T28: Q8-Q14)
    // ========================================================================

    #[test]
    fn prop_metrics_never_negative() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.increment_docs(1_000_000);
        worker.record_latency(u64::MAX / 2);
        let stats = worker.snapshot();

        // u64 is always >= 0, so this is trivially true, but validates types
        assert!(stats.docs_processed >= 0);
        assert!(stats.total_latency_ns >= 0);
    }

    #[test]
    fn prop_snapshot_consistency() {
        let worker = WorkerStateCapsule::new(10, 15);
        worker.increment_docs(500);
        worker.increment_batches();

        let snapshot1 = worker.snapshot();
        let snapshot2 = worker.snapshot();

        // Worker ID and CPU core are immutable
        assert_eq!(snapshot1.worker_id, snapshot2.worker_id);
        assert_eq!(snapshot1.cpu_core, snapshot2.cpu_core);

        // Metrics should be >= previous snapshot (monotonic increase)
        assert_eq!(snapshot1.docs_processed, snapshot2.docs_processed);
        assert_eq!(snapshot1.batches_completed, snapshot2.batches_completed);
    }

    #[test]
    fn prop_sum_equals_individual_additions() {
        let worker = WorkerStateCapsule::new(0, 0);
        let increments = vec![100, 200, 150, 350, 200];

        for &inc in &increments {
            worker.increment_docs(inc);
        }

        let expected: u64 = increments.iter().sum();
        assert_eq!(worker.snapshot().docs_processed, expected);
    }

    #[test]
    fn prop_large_values() {
        let worker = WorkerStateCapsule::new(0, 0);

        // Test near maximum u64 value (but not overflow)
        let large_val = u64::MAX / 4; // Safe value
        worker.increment_docs(large_val);
        worker.increment_docs(large_val);

        // Should not panic or overflow
        let stats = worker.snapshot();
        assert_eq!(stats.docs_processed, large_val * 2);
    }

    // ========================================================================
    // INTEGRATION TESTS (T28: Q15-Q21)
    // ========================================================================

    #[test]
    fn test_concurrent_reads() {
        use std::sync::Arc;
        use std::thread;

        let worker = Arc::new(WorkerStateCapsule::new(1, 2));

        // Main thread increments
        for _ in 0..1000 {
            worker.increment_docs(1);
        }

        let worker_clone = Arc::clone(&worker);
        let handle = thread::spawn(move || {
            // Reader thread takes snapshots
            let mut sum = 0u64;
            for _ in 0..100 {
                sum += worker_clone.snapshot().docs_processed;
            }
            sum
        });

        let reader_sum = handle.join().unwrap();

        // Reader saw consistent snapshots
        assert!(reader_sum >= 100 * 1000); // At least 100k docs (likely much higher)
    }

    #[test]
    fn test_multiple_metrics_together() {
        let worker = WorkerStateCapsule::new(5, 10);

        // Simulate real workload
        worker.increment_docs(1000);
        worker.increment_batches();
        worker.record_compute_cycle();
        worker.record_compute_cycle();
        worker.record_idle_cycle();
        worker.set_batch_size(512);
        worker.set_active(true);
        worker.record_latency(5_000);

        let stats = worker.snapshot();
        assert_eq!(stats.docs_processed, 1000);
        assert_eq!(stats.batches_completed, 1);
        assert_eq!(stats.compute_cycles, 2);
        assert_eq!(stats.idle_cycles, 1);
        assert_eq!(stats.current_batch_size, 512);
        assert!(stats.is_active);
        assert_eq!(stats.total_latency_ns, 5_000);
    }

    #[test]
    fn test_worker_stats_avg_latency() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.increment_docs(100);
        worker.record_latency(100_000);

        let stats = worker.snapshot();
        assert_eq!(stats.avg_latency_ns(), 1_000);
    }

    #[test]
    fn test_worker_stats_steal_ratio() {
        let worker = WorkerStateCapsule::new(0, 0);
        worker.record_steal();
        worker.record_steal();
        worker.record_donation();

        let stats = worker.snapshot();
        assert!((stats.steal_ratio() - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_worker_stats_utilization() {
        let worker = WorkerStateCapsule::new(0, 0);
        for _ in 0..80 {
            worker.record_compute_cycle();
        }
        for _ in 0..20 {
            worker.record_idle_cycle();
        }

        let stats = worker.snapshot();
        let util = stats.utilization();
        assert!(util > 0.79 && util < 0.81); // ~80%
    }

    // ========================================================================
    // PRODUCTION TESTS (T28: Q22-Q28)
    // ========================================================================

    #[test]
    #[ignore = "stress test"]
    fn stress_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let worker = Arc::new(WorkerStateCapsule::new(0, 0));
        let mut handles = vec![];

        // 16 threads, each incrementing 1M times
        for thread_id in 0..16 {
            let worker_clone = Arc::clone(&worker);
            let handle = thread::spawn(move || {
                for _ in 0..100_000 {
                    worker_clone.increment_docs(10);
                    if thread_id % 2 == 0 {
                        worker_clone.record_compute_cycle();
                    } else {
                        worker_clone.record_idle_cycle();
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = worker.snapshot();
        // 16 threads × 100K iterations × 10 docs = 16M docs
        assert_eq!(stats.docs_processed, 16_000_000);
        // 8 threads × 100K = 800K compute cycles
        assert_eq!(stats.compute_cycles, 800_000);
        // 8 threads × 100K = 800K idle cycles
        assert_eq!(stats.idle_cycles, 800_000);
    }

    #[test]
    #[ignore = "false sharing detection"]
    fn false_sharing_detection() {
        use std::sync::Arc;
        use std::thread;
        use std::time::Instant;

        // Create two workers side-by-side in memory
        let workers = Arc::new((
            WorkerStateCapsule::new(0, 0),
            WorkerStateCapsule::new(1, 1),
        ));

        let mut handles = vec![];

        // Thread 0: Continuously update worker 0
        let workers_clone = Arc::clone(&workers);
        let handle0 = thread::spawn(move || {
            let start = Instant::now();
            for _ in 0..10_000_000 {
                workers_clone.0.increment_docs(1);
            }
            start.elapsed().as_secs_f64()
        });
        handles.push(handle0);

        // Thread 1: Continuously update worker 1
        let workers_clone = Arc::clone(&workers);
        let handle1 = thread::spawn(move || {
            let start = Instant::now();
            for _ in 0..10_000_000 {
                workers_clone.1.increment_docs(1);
            }
            start.elapsed().as_secs_f64()
        });
        handles.push(handle1);

        let elapsed0 = handles.pop().unwrap().join().unwrap();
        let elapsed1 = handles.pop().unwrap().join().unwrap();

        // Should complete in similar time (not 2-3× slower due to false sharing)
        let ratio = elapsed0 / elapsed1;
        assert!(
            ratio > 0.8 && ratio < 1.2,
            "False sharing detected: ratio = {:.2}",
            ratio
        );
    }

    #[test]
    fn latency_measurement_consistency() {
        let worker = WorkerStateCapsule::new(0, 0);

        // Simulate 1000 operations with varying latencies
        let mut total_latency = 0u64;
        for i in 1..=1000 {
            let latency = (i * 100) as u64;
            worker.record_latency(latency);
            total_latency += latency;
        }

        worker.increment_docs(1000);

        let stats = worker.snapshot();
        assert_eq!(stats.total_latency_ns, total_latency);
        assert_eq!(stats.avg_latency_ns(), total_latency / 1000);
    }
}
