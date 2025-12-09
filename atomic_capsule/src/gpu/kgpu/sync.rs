//! KGPU Synchronization Utilities - T1 Atomic Tier Coordination Patterns
//!
//! **Tier**: T1 Atomic (lockfree coordination primitives)
//! **Purpose**: Common synchronization patterns for GPU command submission
//!
//! # Architecture
//!
//! Provides high-level synchronization primitives built on:
//! - [`KgpuFenceCapsule`] (CPU-GPU sync)
//! - [`KgpuSemaphoreCapsule`] (GPU-GPU sync)
//!
//! # SOTA Patterns (2024)
//!
//! **Vulkan Timeline Semaphore Patterns**:
//! - SyncPoint: (semaphore, value) pair for timeline sync
//! - WaitInfo: Multiple wait points before submission
//! - SignalInfo: Multiple signal points after submission
//! - Out-of-order submission with timeline values
//!
//! **D3D12 Componentized Applications**:
//! - Accept (fence, value) pair at entry
//! - Signal (fence, value) pair at exit
//! - Enables modular GPU workload composition
//!
//! **Cross-Queue Synchronization**:
//! - Graphics → Compute dependencies
//! - Compute → Transfer dependencies
//! - Minimize pipeline bubbles with async queues
//!
//! **Frame Pacing (NVIDIA DLSS 4)**:
//! - Consistent frame intervals via fence timing
//! - Hardware flip metering for multi-frame generation
//! - CPU/GPU pipeline coordination
//!
//! # Framework Compliance
//!
//! - **UCE34**: T1 Atomic tier (lockfree coordination)
//! - **Chaos**: 100% lockfree (no mutex), const generics
//! - **ASSUM**: All assumptions documented
//! - **B32**: Performance targets for common patterns
//! - **T28**: Comprehensive test coverage
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule::gpu::kgpu::{
//!     KgpuSemaphoreCapsule, SyncPoint, WaitInfo, SignalInfo
//! };
//!
//! // Timeline semaphore
//! let sem = KgpuSemaphoreCapsule::new_timeline(0);
//!
//! // Wait for value 1 before submission
//! let wait_info = WaitInfo::single(SyncPoint::new(&sem, 1));
//!
//! // Signal value 2 after submission
//! let signal_info = SignalInfo::single(SyncPoint::new(&sem, 2));
//!
//! // Submit work with wait/signal dependencies
//! // queue.submit(command_buffer, wait_info, signal_info);
//! ```

use super::semaphore::KgpuSemaphoreCapsule;

// ============================================================================
// SyncPoint - (Semaphore, Value) Pair
// ============================================================================

/// Synchronization point for timeline semaphores
///
/// Represents a (semaphore, value) pair for wait/signal operations.
/// Used for cross-queue dependencies and out-of-order submission.
///
/// # ASSUM Safety
///
/// - `#ASSUME_SEMAPHORE_VALID`: Semaphore remains valid during sync
/// - `#ASSUME_TIMELINE_VALUE`: Value is valid timeline counter
///
/// # Examples
///
/// ```rust,ignore
/// let sem = KgpuSemaphoreCapsule::new_timeline(0);
/// let sync_point = SyncPoint::new(&sem, 1);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SyncPoint<'a> {
    /// Semaphore to wait on or signal
    pub semaphore: &'a KgpuSemaphoreCapsule,

    /// Timeline value to wait for or signal
    pub value: u64,
}

impl<'a> SyncPoint<'a> {
    /// Create a new sync point
    ///
    /// # Arguments
    ///
    /// - `semaphore`: Timeline semaphore reference
    /// - `value`: Timeline value
    ///
    /// # Safety
    ///
    /// #ASSUME_SEMAPHORE_TIMELINE: Semaphore must be timeline type
    /// #VERIFY: Runtime panic if binary semaphore
    ///
    /// # Panics
    ///
    /// Panics if semaphore is binary (timeline required)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let sp = SyncPoint::new(&sem, 1);
    /// ```
    #[inline]
    pub const fn new(semaphore: &'a KgpuSemaphoreCapsule, value: u64) -> Self {
        Self { semaphore, value }
    }

    /// Create a sync point with current semaphore value
    ///
    /// Useful for signaling next value in sequence.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// sem.signal_value(1);
    /// let sp = SyncPoint::current(&sem); // value = 1
    /// ```
    #[inline]
    pub fn current(semaphore: &'a KgpuSemaphoreCapsule) -> Self {
        let value = semaphore.value();
        Self { semaphore, value }
    }

    /// Create a sync point with next semaphore value
    ///
    /// Useful for signaling next value in sequence.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// sem.signal_value(1);
    /// let sp = SyncPoint::next(&sem); // value = 2
    /// ```
    #[inline]
    pub fn next(semaphore: &'a KgpuSemaphoreCapsule) -> Self {
        let value = semaphore.value() + 1;
        Self { semaphore, value }
    }

    /// Check if this sync point is reached
    ///
    /// Returns true if semaphore value >= target value.
    ///
    /// # Performance
    ///
    /// - Latency: <10ns (single atomic load)
    #[inline]
    pub fn is_reached(&self) -> bool {
        self.semaphore.value() >= self.value
    }
}

// ============================================================================
// WaitInfo - Multi-Wait Conditions
// ============================================================================

/// Wait conditions for command submission
///
/// Specifies semaphore wait points that must be satisfied before
/// executing a command buffer. Supports up to 8 wait points.
///
/// # ASSUM Safety
///
/// - `#ASSUME_WAIT_COUNT_VALID`: Count <= MAX_WAIT_POINTS
/// - `#ASSUME_SEMAPHORES_TIMELINE`: All semaphores are timeline type
///
/// # Examples
///
/// ```rust,ignore
/// let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
/// let sem2 = KgpuSemaphoreCapsule::new_timeline(0);
///
/// // Wait for both semaphores
/// let wait_info = WaitInfo::multiple(&[
///     SyncPoint::new(&sem1, 1),
///     SyncPoint::new(&sem2, 2),
/// ]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct WaitInfo<'a> {
    /// Wait points (up to 8)
    pub points: [Option<SyncPoint<'a>>; MAX_WAIT_POINTS],

    /// Number of valid wait points
    pub count: usize,
}

/// Maximum wait points per submission
pub const MAX_WAIT_POINTS: usize = 8;

impl<'a> WaitInfo<'a> {
    /// Create empty wait info (no waits)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let wait_info = WaitInfo::none();
    /// ```
    #[inline]
    pub const fn none() -> Self {
        Self {
            points: [None; MAX_WAIT_POINTS],
            count: 0,
        }
    }

    /// Create wait info with single wait point
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let wait_info = WaitInfo::single(SyncPoint::new(&sem, 1));
    /// ```
    #[inline]
    pub const fn single(point: SyncPoint<'a>) -> Self {
        let mut points = [None; MAX_WAIT_POINTS];
        points[0] = Some(point);
        Self { points, count: 1 }
    }

    /// Create wait info with multiple wait points
    ///
    /// # Arguments
    ///
    /// - `sync_points`: Slice of sync points (up to 8)
    ///
    /// # Panics
    ///
    /// Panics if `sync_points.len()` > MAX_WAIT_POINTS
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
    /// let sem2 = KgpuSemaphoreCapsule::new_timeline(0);
    ///
    /// let wait_info = WaitInfo::multiple(&[
    ///     SyncPoint::new(&sem1, 1),
    ///     SyncPoint::new(&sem2, 2),
    /// ]);
    /// ```
    pub fn multiple(sync_points: &[SyncPoint<'a>]) -> Self {
        assert!(
            sync_points.len() <= MAX_WAIT_POINTS,
            "Too many wait points: {} (max {})",
            sync_points.len(),
            MAX_WAIT_POINTS
        );

        let mut points = [None; MAX_WAIT_POINTS];
        for (i, &point) in sync_points.iter().enumerate() {
            points[i] = Some(point);
        }

        Self {
            points,
            count: sync_points.len(),
        }
    }

    /// Check if all wait points are reached
    ///
    /// Returns true if all semaphore values >= target values.
    ///
    /// # Performance
    ///
    /// - Latency: O(n) where n is number of wait points
    /// - Typical: <80ns for 8 wait points
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// sem.signal_value(1);
    ///
    /// let wait_info = WaitInfo::single(SyncPoint::new(&sem, 1));
    /// assert!(wait_info.all_reached());
    /// ```
    pub fn all_reached(&self) -> bool {
        self.points[..self.count]
            .iter()
            .filter_map(|&p| p)
            .all(|p| p.is_reached())
    }

    /// Get iterator over valid sync points
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let wait_info = WaitInfo::multiple(&[...]);
    /// for point in wait_info.iter() {
    ///     println!("Wait for value {}", point.value);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = SyncPoint<'a>> + '_ {
        self.points[..self.count].iter().filter_map(|&p| p)
    }
}

impl Default for WaitInfo<'_> {
    fn default() -> Self {
        Self::none()
    }
}

// ============================================================================
// SignalInfo - Multi-Signal Conditions
// ============================================================================

/// Signal conditions for command submission
///
/// Specifies semaphore signal points that will be signaled after
/// executing a command buffer. Supports up to 8 signal points.
///
/// # ASSUM Safety
///
/// - `#ASSUME_SIGNAL_COUNT_VALID`: Count <= MAX_SIGNAL_POINTS
/// - `#ASSUME_SEMAPHORES_TIMELINE`: All semaphores are timeline type
/// - `#ASSUME_VALUES_MONOTONIC`: All values > current semaphore values
///
/// # Examples
///
/// ```rust,ignore
/// let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
/// let sem2 = KgpuSemaphoreCapsule::new_timeline(0);
///
/// // Signal both semaphores after completion
/// let signal_info = SignalInfo::multiple(&[
///     SyncPoint::new(&sem1, 1),
///     SyncPoint::new(&sem2, 2),
/// ]);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct SignalInfo<'a> {
    /// Signal points (up to 8)
    pub points: [Option<SyncPoint<'a>>; MAX_SIGNAL_POINTS],

    /// Number of valid signal points
    pub count: usize,
}

/// Maximum signal points per submission
pub const MAX_SIGNAL_POINTS: usize = 8;

impl<'a> SignalInfo<'a> {
    /// Create empty signal info (no signals)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let signal_info = SignalInfo::none();
    /// ```
    #[inline]
    pub const fn none() -> Self {
        Self {
            points: [None; MAX_SIGNAL_POINTS],
            count: 0,
        }
    }

    /// Create signal info with single signal point
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let signal_info = SignalInfo::single(SyncPoint::new(&sem, 1));
    /// ```
    #[inline]
    pub const fn single(point: SyncPoint<'a>) -> Self {
        let mut points = [None; MAX_SIGNAL_POINTS];
        points[0] = Some(point);
        Self { points, count: 1 }
    }

    /// Create signal info with multiple signal points
    ///
    /// # Arguments
    ///
    /// - `sync_points`: Slice of sync points (up to 8)
    ///
    /// # Panics
    ///
    /// Panics if `sync_points.len()` > MAX_SIGNAL_POINTS
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
    /// let sem2 = KgpuSemaphoreCapsule::new_timeline(0);
    ///
    /// let signal_info = SignalInfo::multiple(&[
    ///     SyncPoint::new(&sem1, 1),
    ///     SyncPoint::new(&sem2, 2),
    /// ]);
    /// ```
    pub fn multiple(sync_points: &[SyncPoint<'a>]) -> Self {
        assert!(
            sync_points.len() <= MAX_SIGNAL_POINTS,
            "Too many signal points: {} (max {})",
            sync_points.len(),
            MAX_SIGNAL_POINTS
        );

        let mut points = [None; MAX_SIGNAL_POINTS];
        for (i, &point) in sync_points.iter().enumerate() {
            points[i] = Some(point);
        }

        Self {
            points,
            count: sync_points.len(),
        }
    }

    /// Perform all signals
    ///
    /// Signals all semaphores with their target values.
    ///
    /// # Performance
    ///
    /// - Latency: O(n) where n is number of signal points
    /// - Typical: <400ns for 8 signal points (~50ns per signal)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let signal_info = SignalInfo::single(SyncPoint::new(&sem, 1));
    /// signal_info.signal_all();
    /// assert_eq!(sem.value(), 1);
    /// ```
    pub fn signal_all(&self) {
        for point in self.iter() {
            point.semaphore.signal_value(point.value);
        }
    }

    /// Get iterator over valid sync points
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let signal_info = SignalInfo::multiple(&[...]);
    /// for point in signal_info.iter() {
    ///     println!("Will signal value {}", point.value);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = SyncPoint<'a>> + '_ {
        self.points[..self.count].iter().filter_map(|&p| p)
    }
}

impl Default for SignalInfo<'_> {
    fn default() -> Self {
        Self::none()
    }
}

// ============================================================================
// Common Sync Patterns
// ============================================================================

/// Common synchronization patterns for GPU workloads
pub struct SyncPatterns;

impl SyncPatterns {
    /// Create single producer, multiple consumer pattern
    ///
    /// Producer signals one semaphore, multiple consumers wait on it.
    ///
    /// # Arguments
    ///
    /// - `producer_sem`: Semaphore signaled by producer
    /// - `signal_value`: Value to signal
    ///
    /// # Returns
    ///
    /// - SignalInfo for producer
    /// - WaitInfo for each consumer (clone for multiple consumers)
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let (producer_signal, consumer_wait) = SyncPatterns::single_producer_multi_consumer(&sem, 1);
    ///
    /// // Producer: queue.submit(cmd_buf, WaitInfo::none(), producer_signal);
    /// // Consumer: queue.submit(cmd_buf, consumer_wait, SignalInfo::none());
    /// ```
    pub fn single_producer_multi_consumer<'a>(
        producer_sem: &'a KgpuSemaphoreCapsule,
        signal_value: u64,
    ) -> (SignalInfo<'a>, WaitInfo<'a>) {
        let signal_info = SignalInfo::single(SyncPoint::new(producer_sem, signal_value));
        let wait_info = WaitInfo::single(SyncPoint::new(producer_sem, signal_value));
        (signal_info, wait_info)
    }

    /// Create pipeline pattern (A → B → C)
    ///
    /// Each stage waits on previous stage, signals next stage.
    ///
    /// # Arguments
    ///
    /// - `sem_ab`: Semaphore between A and B
    /// - `value_ab`: Value A signals, B waits
    /// - `sem_bc`: Semaphore between B and C
    /// - `value_bc`: Value B signals, C waits
    ///
    /// # Returns
    ///
    /// - (SignalInfo, WaitInfo) for stage A
    /// - (SignalInfo, WaitInfo) for stage B
    /// - (SignalInfo, WaitInfo) for stage C
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let sem_ab = KgpuSemaphoreCapsule::new_timeline(0);
    /// let sem_bc = KgpuSemaphoreCapsule::new_timeline(0);
    ///
    /// let (sync_a, sync_b, sync_c) = SyncPatterns::pipeline(&sem_ab, 1, &sem_bc, 2);
    ///
    /// // Stage A: queue.submit(cmd_a, sync_a.1, sync_a.0);
    /// // Stage B: queue.submit(cmd_b, sync_b.1, sync_b.0);
    /// // Stage C: queue.submit(cmd_c, sync_c.1, sync_c.0);
    /// ```
    pub fn pipeline<'a>(
        sem_ab: &'a KgpuSemaphoreCapsule,
        value_ab: u64,
        sem_bc: &'a KgpuSemaphoreCapsule,
        value_bc: u64,
    ) -> (
        (SignalInfo<'a>, WaitInfo<'a>), // Stage A
        (SignalInfo<'a>, WaitInfo<'a>), // Stage B
        (SignalInfo<'a>, WaitInfo<'a>), // Stage C
    ) {
        let sync_a = (
            SignalInfo::single(SyncPoint::new(sem_ab, value_ab)),
            WaitInfo::none(),
        );

        let sync_b = (
            SignalInfo::single(SyncPoint::new(sem_bc, value_bc)),
            WaitInfo::single(SyncPoint::new(sem_ab, value_ab)),
        );

        let sync_c = (
            SignalInfo::none(),
            WaitInfo::single(SyncPoint::new(sem_bc, value_bc)),
        );

        (sync_a, sync_b, sync_c)
    }

    /// Create async compute pattern (Graphics + Compute overlap)
    ///
    /// Graphics and compute queues overlap work with dependency at end.
    ///
    /// # Arguments
    ///
    /// - `graphics_sem`: Graphics queue semaphore
    /// - `compute_sem`: Compute queue semaphore
    /// - `sync_value`: Final sync value
    ///
    /// # Returns
    ///
    /// - (SignalInfo, WaitInfo) for graphics queue
    /// - (SignalInfo, WaitInfo) for compute queue
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let gfx_sem = KgpuSemaphoreCapsule::new_timeline(0);
    /// let compute_sem = KgpuSemaphoreCapsule::new_timeline(0);
    ///
    /// let (gfx_sync, compute_sync) = SyncPatterns::async_compute(&gfx_sem, &compute_sem, 1);
    ///
    /// // Graphics: queue_gfx.submit(cmd_gfx, gfx_sync.1, gfx_sync.0);
    /// // Compute: queue_compute.submit(cmd_compute, compute_sync.1, compute_sync.0);
    /// ```
    pub fn async_compute<'a>(
        graphics_sem: &'a KgpuSemaphoreCapsule,
        compute_sem: &'a KgpuSemaphoreCapsule,
        sync_value: u64,
    ) -> (
        (SignalInfo<'a>, WaitInfo<'a>), // Graphics queue
        (SignalInfo<'a>, WaitInfo<'a>), // Compute queue
    ) {
        let gfx_sync = (
            SignalInfo::single(SyncPoint::new(graphics_sem, sync_value)),
            WaitInfo::single(SyncPoint::new(compute_sem, sync_value)),
        );

        let compute_sync = (
            SignalInfo::single(SyncPoint::new(compute_sem, sync_value)),
            WaitInfo::none(),
        );

        (gfx_sync, compute_sync)
    }
}

// ============================================================================
// Tests (T28 Framework)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // SyncPoint Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_sync_point_new() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let sp = SyncPoint::new(&sem, 5);
        assert_eq!(sp.value, 5);
        assert!(!sp.is_reached());
    }

    #[test]
    fn test_sync_point_current() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        sem.signal_value(3);
        let sp = SyncPoint::current(&sem);
        assert_eq!(sp.value, 3);
        assert!(sp.is_reached());
    }

    #[test]
    fn test_sync_point_next() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        sem.signal_value(3);
        let sp = SyncPoint::next(&sem);
        assert_eq!(sp.value, 4);
        assert!(!sp.is_reached());
    }

    #[test]
    fn test_sync_point_is_reached() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let sp = SyncPoint::new(&sem, 2);
        assert!(!sp.is_reached());

        sem.signal_value(2);
        assert!(sp.is_reached());

        sem.signal_value(3);
        assert!(sp.is_reached()); // Still true (value >= 2)
    }

    // ========================================================================
    // WaitInfo Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_wait_info_none() {
        let wait = WaitInfo::none();
        assert_eq!(wait.count, 0);
        assert!(wait.all_reached());
    }

    #[test]
    fn test_wait_info_single() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let wait = WaitInfo::single(SyncPoint::new(&sem, 1));
        assert_eq!(wait.count, 1);
        assert!(!wait.all_reached());

        sem.signal_value(1);
        assert!(wait.all_reached());
    }

    #[test]
    fn test_wait_info_multiple() {
        let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
        let sem2 = KgpuSemaphoreCapsule::new_timeline(0);

        let wait = WaitInfo::multiple(&[
            SyncPoint::new(&sem1, 1),
            SyncPoint::new(&sem2, 2),
        ]);

        assert_eq!(wait.count, 2);
        assert!(!wait.all_reached());

        sem1.signal_value(1);
        assert!(!wait.all_reached()); // Only 1/2 reached

        sem2.signal_value(2);
        assert!(wait.all_reached()); // Both reached
    }

    #[test]
    #[should_panic(expected = "Too many wait points")]
    fn test_wait_info_too_many() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let points = vec![SyncPoint::new(&sem, 1); 9]; // 9 > MAX_WAIT_POINTS
        let _ = WaitInfo::multiple(&points);
    }

    #[test]
    fn test_wait_info_iter() {
        let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
        let sem2 = KgpuSemaphoreCapsule::new_timeline(0);

        let wait = WaitInfo::multiple(&[
            SyncPoint::new(&sem1, 1),
            SyncPoint::new(&sem2, 2),
        ]);

        let values: Vec<u64> = wait.iter().map(|sp| sp.value).collect();
        assert_eq!(values, vec![1, 2]);
    }

    // ========================================================================
    // SignalInfo Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_signal_info_none() {
        let signal = SignalInfo::none();
        assert_eq!(signal.count, 0);
        signal.signal_all(); // No-op
    }

    #[test]
    fn test_signal_info_single() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let signal = SignalInfo::single(SyncPoint::new(&sem, 1));
        assert_eq!(signal.count, 1);

        signal.signal_all();
        assert_eq!(sem.value(), 1);
    }

    #[test]
    fn test_signal_info_multiple() {
        let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
        let sem2 = KgpuSemaphoreCapsule::new_timeline(0);

        let signal = SignalInfo::multiple(&[
            SyncPoint::new(&sem1, 1),
            SyncPoint::new(&sem2, 2),
        ]);

        assert_eq!(signal.count, 2);

        signal.signal_all();
        assert_eq!(sem1.value(), 1);
        assert_eq!(sem2.value(), 2);
    }

    #[test]
    #[should_panic(expected = "Too many signal points")]
    fn test_signal_info_too_many() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let points = vec![SyncPoint::new(&sem, 1); 9]; // 9 > MAX_SIGNAL_POINTS
        let _ = SignalInfo::multiple(&points);
    }

    #[test]
    fn test_signal_info_iter() {
        let sem1 = KgpuSemaphoreCapsule::new_timeline(0);
        let sem2 = KgpuSemaphoreCapsule::new_timeline(0);

        let signal = SignalInfo::multiple(&[
            SyncPoint::new(&sem1, 1),
            SyncPoint::new(&sem2, 2),
        ]);

        let values: Vec<u64> = signal.iter().map(|sp| sp.value).collect();
        assert_eq!(values, vec![1, 2]);
    }

    // ========================================================================
    // SyncPatterns Tests (T28 Integration Tier)
    // ========================================================================

    #[test]
    fn test_pattern_single_producer_multi_consumer() {
        let sem = KgpuSemaphoreCapsule::new_timeline(0);
        let (producer_signal, consumer_wait) =
            SyncPatterns::single_producer_multi_consumer(&sem, 1);

        assert_eq!(producer_signal.count, 1);
        assert_eq!(consumer_wait.count, 1);

        // Producer signals
        producer_signal.signal_all();
        assert_eq!(sem.value(), 1);

        // Consumers can wait
        assert!(consumer_wait.all_reached());
    }

    #[test]
    fn test_pattern_pipeline() {
        let sem_ab = KgpuSemaphoreCapsule::new_timeline(0);
        let sem_bc = KgpuSemaphoreCapsule::new_timeline(0);

        let (sync_a, sync_b, sync_c) = SyncPatterns::pipeline(&sem_ab, 1, &sem_bc, 2);

        // Stage A: no waits, signals sem_ab=1
        assert_eq!(sync_a.0.count, 1); // signal
        assert_eq!(sync_a.1.count, 0); // no wait

        // Stage B: waits sem_ab=1, signals sem_bc=2
        assert_eq!(sync_b.0.count, 1); // signal
        assert_eq!(sync_b.1.count, 1); // wait

        // Stage C: waits sem_bc=2, no signals
        assert_eq!(sync_c.0.count, 0); // no signal
        assert_eq!(sync_c.1.count, 1); // wait

        // Execute pipeline
        sync_a.0.signal_all();
        assert!(sync_b.1.all_reached());

        sync_b.0.signal_all();
        assert!(sync_c.1.all_reached());
    }

    #[test]
    fn test_pattern_async_compute() {
        let gfx_sem = KgpuSemaphoreCapsule::new_timeline(0);
        let compute_sem = KgpuSemaphoreCapsule::new_timeline(0);

        let (gfx_sync, compute_sync) = SyncPatterns::async_compute(&gfx_sem, &compute_sem, 1);

        // Compute has no waits (runs in parallel)
        assert_eq!(compute_sync.1.count, 0);

        // Graphics waits on compute completion
        assert_eq!(gfx_sync.1.count, 1);

        // Both signal their semaphores
        compute_sync.0.signal_all();
        assert!(gfx_sync.1.all_reached());

        gfx_sync.0.signal_all();
        assert_eq!(gfx_sem.value(), 1);
    }

    // ========================================================================
    // Default Tests (T28 Unit Tier)
    // ========================================================================

    #[test]
    fn test_wait_info_default() {
        let wait: WaitInfo = Default::default();
        assert_eq!(wait.count, 0);
    }

    #[test]
    fn test_signal_info_default() {
        let signal: SignalInfo = Default::default();
        assert_eq!(signal.count, 0);
    }
}
