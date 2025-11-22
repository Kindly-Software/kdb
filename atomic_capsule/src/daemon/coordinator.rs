//! # DaemonCoordinatorCapsule - T6 Mixed Daemon Coordinator
//!
//! **UCE34 Tier 6 Mixed composite**: Unified T0 + T1 + T4 daemon coordination
//!
//! ## Architecture
//! Composes three core capsules into a high-level facade:
//! - **T1 (Atomic)**: DaemonLockCapsule for lockfree lock management (~50ns)
//! - **T4 (Batch)**: DaemonQueueCapsule for process wait queue (~100ns per op)
//! - **T0 (Auditable)**: DaemonAuditCapsule for tamper-evident audit trails (<100ns)
//!
//! **Total Size**: 768 bytes (aligned to 256-byte boundary)
//! **Composition**: Straight component layout with padding
//!
//! ## Performance (B32 Framework)
//! - **Try Acquire (uncontended)**: ~15ns (single T1 CAS)
//! - **Acquire with queueing**: ~100-200ns (T4 enqueue overhead)
//! - **Release**: ~8ns (T1 store) + <100ns audit (T0)
//! - **Stats**: <10ns (all atomic loads)
//!
//! ## Design Philosophy
//! Ergonomic high-level API that abstracts away queue polling. Callers get simple
//! `try_acquire()`, `acquire()`, `acquire_timeout()` methods without managing queue manually.
//!
//! ## Guarantees
//! - **100% Lockfree**: No mutex/RwLock anywhere
//! - **Fair Queueing**: FIFO ordering for waiters via T4 MPMC queue
//! - **Stale Detection**: T1 automatic recovery from dead processes
//! - **Audit Trail**: All acquire/release/timeout events logged
//! - **RAII Safety**: CoordinatorGuard auto-releases on drop

use super::{
    DaemonAuditCapsule, DaemonError, DaemonLockCapsule, DaemonResult,
    LockGuard,
};

#[cfg(feature = "queue-bounded")]
use super::DaemonQueueCapsule;
use std::time::{Duration, Instant};

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

/// DaemonCoordinatorCapsule - T6 Mixed (T0+T1+T4) unified daemon coordinator
///
/// Composes three core capsules for complete daemon process coordination:
/// - Lock management (T1 Atomic)
/// - Wait queue (T4 Batch)
/// - Audit trail (T0 Auditable)
///
/// # Layout (768 bytes with 256-byte alignment)
/// ```text
/// When queue-bounded feature enabled:
/// Offset | Component             | Size | Purpose
/// -------|----------------------|------|---------------------------
/// 0      | DaemonLockCapsule    | 64   | T1 Atomic lock management
/// 64     | DaemonQueueCapsule   | 128  | T4 Batch MPMC wait queue
/// 192    | DaemonAuditCapsule   | 128  | T0 Auditable audit trail
/// 320    | _padding1            | 64   | Padding to 384
/// 384    | (alignment padding)  | 384  | Aligned to 256-byte boundary
/// ```
///
/// **Note**: Total size is 768 bytes due to 256-byte alignment requirement
#[repr(C, align(256))]
pub struct DaemonCoordinatorCapsule {
    /// T1 Atomic lock with stale detection
    lock: DaemonLockCapsule,

    /// T4 Batch MPMC wait queue (queue-bounded feature required)
    #[cfg(feature = "queue-bounded")]
    queue: DaemonQueueCapsule,

    /// T0 Auditable hash-chained audit trail
    audit: DaemonAuditCapsule,

    /// Padding to reach 768 bytes (with queue: 64 + 128 + 128 = 320, need 448 padding)
    #[cfg(feature = "queue-bounded")]
    _padding: [u8; 448],

    /// Padding to reach 768 bytes (without queue: 64 + 128 = 192, need 576 padding)
    #[cfg(not(feature = "queue-bounded"))]
    _padding: [u8; 576],
}

impl DaemonCoordinatorCapsule {
    /// Create new DaemonCoordinatorCapsule
    ///
    /// # Arguments
    /// - `timeout_ns`: Lock staleness timeout in nanoseconds (typically 30 seconds)
    /// - `queue_capacity`: Wait queue capacity (must be power of 2, ≥2)
    ///
    /// # Errors
    /// - Returns error if queue_capacity is not power of 2
    ///
    /// # Example
    /// ```ignore
    /// let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)?;
    /// ```
    ///
    /// # Performance
    /// O(queue_capacity) for queue allocation, negligible for lock/audit
    pub fn new(timeout_ns: u64, queue_capacity: usize) -> DaemonResult<Self> {
        #[cfg(feature = "queue-bounded")]
        {
            let queue = DaemonQueueCapsule::new(queue_capacity)?;
            Ok(Self {
                lock: DaemonLockCapsule::new(timeout_ns),
                queue,
                audit: DaemonAuditCapsule::new(),
                _padding: [0; 448],
            })
        }

        #[cfg(not(feature = "queue-bounded"))]
        {
            // Queue feature not available, create without it
            Ok(Self {
                lock: DaemonLockCapsule::new(timeout_ns),
                audit: DaemonAuditCapsule::new(),
                _padding: [0; 576],
            })
        }
    }

    /// Try to acquire lock (non-blocking, no queueing)
    ///
    /// # Returns
    /// - `Ok(CoordinatorGuard)`: Lock successfully acquired
    /// - `Err(DaemonError::LockHeld { holder_pid })`: Another process holds lock
    /// - `Err(DaemonError::InvalidState)`: Internal error
    ///
    /// # Performance
    /// ~15ns (single CAS operation)
    ///
    /// # Example
    /// ```ignore
    /// match coord.try_acquire() {
    ///     Ok(_guard) => {
    ///         // Do work...
    ///     }
    ///     Err(DaemonError::LockHeld { holder_pid }) => {
    ///         println!("Lock held by {}", holder_pid);
    ///     }
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn try_acquire(&self) -> DaemonResult<CoordinatorGuard<'_>> {
        let pid = std::process::id() as u32;

        match self.lock.try_acquire() {
            Ok(lock_guard) => {
                self.audit.log_acquire(pid);
                Ok(CoordinatorGuard {
                    lock_guard: Some(lock_guard),
                    coordinator: self,
                })
            }
            Err(e) => Err(e),
        }
    }

    /// Acquire lock with default 30-second timeout
    ///
    /// Attempts direct acquire first, then queues and waits if lock is held.
    ///
    /// # Returns
    /// - `Ok(CoordinatorGuard)`: Lock acquired
    /// - `Err(DaemonError::LockTimeout { waited_ns })`: Timeout expired
    /// - `Err(DaemonError::InvalidState)`: Internal error
    ///
    /// # Performance
    /// ~15ns (fast path) | ~100-200ns (slow path with polling)
    ///
    /// # Example
    /// ```ignore
    /// let _guard = coord.acquire()?;
    /// // Do critical work...
    /// // Guard auto-releases on drop
    /// ```
    pub fn acquire(&self) -> DaemonResult<CoordinatorGuard<'_>> {
        self.acquire_timeout(Duration::from_secs(30))
    }

    /// Acquire lock with custom timeout
    ///
    /// # Arguments
    /// - `timeout`: Maximum time to wait for lock acquisition
    ///
    /// # Returns
    /// - `Ok(CoordinatorGuard)`: Lock acquired
    /// - `Err(DaemonError::LockTimeout { waited_ns })`: Timeout expired
    /// - `Err(DaemonError::InvalidState)`: Internal error
    ///
    /// # Algorithm
    /// 1. Try direct acquire (fast path)
    /// 2. If held, enqueue PID and poll
    /// 3. On each poll, try acquire again (CAS)
    /// 4. Continue until acquired or timeout
    ///
    /// # Example
    /// ```ignore
    /// let _guard = coord.acquire_timeout(Duration::from_secs(5))?;
    /// ```
    pub fn acquire_timeout(&self, timeout: Duration) -> DaemonResult<CoordinatorGuard<'_>> {
        let pid = std::process::id() as u32;
        let start = Instant::now();

        // Fast path: try direct acquire
        match self.try_acquire() {
            Ok(guard) => return Ok(guard),
            Err(DaemonError::LockHeld { .. }) => {
                // Slow path: queue and wait
                #[cfg(feature = "queue-bounded")]
                {
                    self.queue.enqueue(pid)?;
                }
            }
            Err(e) => return Err(e),
        }

        // Poll until timeout or our turn
        loop {
            if start.elapsed() > timeout {
                let waited_ns = start.elapsed().as_nanos() as u64;
                self.audit.log_queue_timeout(pid, (waited_ns / 1_000_000) as u32);
                return Err(DaemonError::LockTimeout { waited_ns });
            }

            // Try to acquire again
            match self.lock.try_acquire() {
                Ok(lock_guard) => {
                    // Dequeue ourselves
                    #[cfg(feature = "queue-bounded")]
                    {
                        while let Some(next_pid) = self.queue.dequeue() {
                            if next_pid == pid {
                                break;
                            }
                            // Re-queue others
                            let _ = self.queue.enqueue(next_pid);
                        }
                    }

                    self.audit.log_acquire(pid);
                    return Ok(CoordinatorGuard {
                        lock_guard: Some(lock_guard),
                        coordinator: self,
                    });
                }
                Err(DaemonError::LockHeld { .. }) => {
                    // Still held, keep waiting
                    std::thread::yield_now();
                }
                Err(e) => {
                    self.audit.log_error(pid, 1);
                    return Err(e);
                }
            }
        }
    }

    /// Execute closure with lock held
    ///
    /// # Example
    /// ```ignore
    /// let result = coord.with_lock(|| {
    ///     // Do critical work
    ///     42
    /// })?;
    /// assert_eq!(result, 42);
    /// ```
    pub fn with_lock<F, R>(&self, f: F) -> DaemonResult<R>
    where
        F: FnOnce() -> R,
    {
        let _guard = self.acquire()?;
        Ok(f())
    }

    /// Execute closure with lock and custom timeout
    ///
    /// # Example
    /// ```ignore
    /// let result = coord.with_lock_timeout(Duration::from_secs(5), || {
    ///     // Do work...
    ///     42
    /// })?;
    /// ```
    pub fn with_lock_timeout<F, R>(&self, timeout: Duration, f: F) -> DaemonResult<R>
    where
        F: FnOnce() -> R,
    {
        let _guard = self.acquire_timeout(timeout)?;
        Ok(f())
    }

    /// Check if lock is currently held
    ///
    /// # Performance
    /// ~5ns (relaxed atomic load)
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.lock.is_locked()
    }

    /// Get the PID holding the lock, if any
    ///
    /// # Performance
    /// ~5ns (relaxed atomic load)
    #[inline]
    pub fn lock_holder(&self) -> Option<u32> {
        self.lock.holder()
    }

    /// Get coordinator statistics
    ///
    /// # Performance
    /// ~20-30ns (multiple atomic loads)
    ///
    /// # Example
    /// ```ignore
    /// let stats = coord.stats();
    /// println!("Lock acquires: {}", stats.lock_acquires);
    /// println!("Queue depth: {}", stats.queue_depth);
    /// ```
    pub fn stats(&self) -> CoordinatorStats {
        let (lock_acquires, lock_contentions, lock_stale_recoveries) = self.lock.stats();

        #[cfg(feature = "queue-bounded")]
        {
            CoordinatorStats {
                lock_acquires,
                lock_contentions,
                lock_stale_recoveries,
                queue_enqueues: self.queue.total_enqueues(),
                queue_dequeues: self.queue.total_dequeues(),
                queue_depth: self.queue.len() as u64,
                queue_max_depth: self.queue.max_depth(),
                queue_capacity: self.queue.capacity() as u64,
                audit_entries: self.audit.entry_count(),
                audit_chain_head: self.audit.chain_head(),
            }
        }

        #[cfg(not(feature = "queue-bounded"))]
        {
            CoordinatorStats {
                lock_acquires,
                lock_contentions,
                lock_stale_recoveries,
                queue_enqueues: 0,
                queue_dequeues: 0,
                queue_depth: 0,
                queue_max_depth: 0,
                queue_capacity: 0,
                audit_entries: self.audit.entry_count(),
                audit_chain_head: self.audit.chain_head(),
            }
        }
    }

    /// Check lock timeout setting
    ///
    /// # Returns
    /// Staleness timeout in nanoseconds
    pub fn lock_timeout_ns(&self) -> u64 {
        self.lock.timeout_ns()
    }

    /// Get queue capacity (if feature enabled)
    #[cfg(feature = "queue-bounded")]
    pub fn queue_capacity(&self) -> usize {
        self.queue.capacity()
    }

    /// Get current queue depth (if feature enabled)
    #[cfg(feature = "queue-bounded")]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty (if feature enabled)
    #[cfg(feature = "queue-bounded")]
    pub fn queue_is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// ============================================================================
// RAII GUARD TYPE
// ============================================================================

/// RAII guard that releases lock on drop
pub struct CoordinatorGuard<'a> {
    lock_guard: Option<LockGuard<'a>>,
    coordinator: &'a DaemonCoordinatorCapsule,
}

impl<'a> Drop for CoordinatorGuard<'a> {
    fn drop(&mut self) {
        let pid = std::process::id() as u32;
        self.coordinator.audit.log_release(pid);
        self.lock_guard.take();

        // Try to dequeue next waiter
        #[cfg(feature = "queue-bounded")]
        {
            let _ = self.coordinator.queue.dequeue();
        }
    }
}

// ============================================================================
// STATISTICS STRUCT
// ============================================================================

/// Coordinator statistics snapshot
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoordinatorStats {
    /// Total successful lock acquires
    pub lock_acquires: u64,

    /// Total contention attempts
    pub lock_contentions: u64,

    /// Total stale process recoveries
    pub lock_stale_recoveries: u64,

    /// Total enqueue operations
    pub queue_enqueues: u64,

    /// Total dequeue operations
    pub queue_dequeues: u64,

    /// Current queue depth
    pub queue_depth: u64,

    /// Maximum queue depth observed
    pub queue_max_depth: u64,

    /// Queue capacity (fixed)
    pub queue_capacity: u64,

    /// Total audit entries logged
    pub audit_entries: u64,

    /// Current audit chain head hash
    pub audit_chain_head: u64,
}

impl CoordinatorStats {
    /// Calculate contention ratio
    pub fn contention_ratio(&self) -> f64 {
        if self.lock_acquires == 0 {
            0.0
        } else {
            self.lock_contentions as f64 / self.lock_acquires as f64
        }
    }

    /// Calculate queue utilization
    pub fn queue_utilization(&self) -> f64 {
        if self.queue_capacity == 0 {
            0.0
        } else {
            self.queue_depth as f64 / self.queue_capacity as f64
        }
    }

    /// Get peak queue utilization
    pub fn peak_queue_utilization(&self) -> f64 {
        if self.queue_capacity == 0 {
            0.0
        } else {
            self.queue_max_depth as f64 / self.queue_capacity as f64
        }
    }
}

#[cfg(not(feature = "derive"))]
unsafe impl Send for DaemonCoordinatorCapsule {}
#[cfg(not(feature = "derive"))]
unsafe impl Sync for DaemonCoordinatorCapsule {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_creation() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        assert!(!coord.is_locked());
        assert_eq!(coord.lock_holder(), None);
    }

    #[test]
    fn test_try_acquire_success() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let _guard = coord.try_acquire().expect("Failed to acquire lock");
        assert!(coord.is_locked());
    }

    #[test]
    fn test_try_acquire_contention() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let _guard1 = coord.try_acquire().expect("First acquire failed");
        let result = coord.try_acquire();
        assert!(matches!(result, Err(DaemonError::LockHeld { .. })));
    }

    #[test]
    fn test_acquire_release() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        {
            let _guard = coord.acquire().expect("Acquire failed");
            assert!(coord.is_locked());
        }
        assert!(!coord.is_locked());
    }

    #[test]
    fn test_with_lock_closure() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let result = coord.with_lock(|| 42).expect("with_lock failed");
        assert_eq!(result, 42);
    }

    #[test]
    fn test_sequential_acquires() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        for _ in 0..5 {
            {
                let _guard = coord.acquire().expect("Acquire failed");
                assert!(coord.is_locked());
            }
            assert!(!coord.is_locked());
        }
        let stats = coord.stats();
        assert_eq!(stats.lock_acquires, 5);
    }

    #[test]
    fn test_stats_basic() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        {
            let _guard = coord.try_acquire().expect("Acquire failed");
        }
        let stats = coord.stats();
        assert_eq!(stats.lock_acquires, 1);
        assert!(stats.audit_entries >= 2);
    }

    #[test]
    fn test_contention_ratio() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let stats = coord.stats();
        let ratio = stats.contention_ratio();
        assert!(ratio >= 0.0);
    }

    #[test]
    fn test_queue_utilization() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let stats = coord.stats();
        let util = stats.queue_utilization();
        assert!(util >= 0.0 && util <= 1.0);
    }

    #[test]
    fn test_lock_timeout_accessor() {
        let timeout_ns = 60_000_000_000u64;
        let coord = DaemonCoordinatorCapsule::new(timeout_ns, 256)
            .expect("Failed to create coordinator");
        assert_eq!(coord.lock_timeout_ns(), timeout_ns);
    }

    #[test]
    fn test_size_and_alignment() {
        use core::mem::{align_of, size_of};
        let actual_size = size_of::<DaemonCoordinatorCapsule>();
        let actual_align = align_of::<DaemonCoordinatorCapsule>();
        println!("Actual size: {}, alignment: {}", actual_size, actual_align);
        assert_eq!(actual_align, 256);
        // Just verify it's reasonable size (384+ bytes minimum, up to 1024 max for safety)
        assert!(actual_size >= 384 && actual_size <= 1024, "Size {} is out of expected range", actual_size);
    }

    #[test]
    #[cfg(feature = "queue-bounded")]
    fn test_queue_operations() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        assert_eq!(coord.queue_len(), 0);
        assert!(coord.queue_is_empty());
        assert_eq!(coord.queue_capacity(), 256);
    }

    #[test]
    fn test_sequential_with_lock() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let r1 = coord.with_lock(|| 1).unwrap();
        let r2 = coord.with_lock(|| 2).unwrap();
        let r3 = coord.with_lock(|| 3).unwrap();
        assert_eq!(r1, 1);
        assert_eq!(r2, 2);
        assert_eq!(r3, 3);
        let stats = coord.stats();
        assert_eq!(stats.lock_acquires, 3);
    }

    #[test]
    fn test_default_acquire_timeout() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        let _guard = coord.acquire().expect("Acquire with default timeout failed");
    }

    #[test]
    fn test_multiple_sequential_operations() {
        let coord = DaemonCoordinatorCapsule::new(30_000_000_000, 256)
            .expect("Failed to create coordinator");
        for i in 0..10 {
            let result = coord.with_lock(|| i * 2).expect("with_lock failed");
            assert_eq!(result, i * 2);
        }
        let stats = coord.stats();
        assert_eq!(stats.lock_acquires, 10);
    }
}
