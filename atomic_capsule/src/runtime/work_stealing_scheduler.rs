//! WorkStealingScheduler - T4 Batch Tier Task Distribution
//!
//! **UCE34 Q10 Tier Selection**: T4 Batch (parallel task distribution with NUMA awareness)
//!
//! Lockfree work-stealing scheduler that coordinates multiple WorkerDeque instances:
//! - NUMA-aware task placement
//! - Batch scheduling for throughput
//! - Adaptive steal patterns for load balancing
//!
//! # Architecture
//!
//! - Each worker owns a Chase-Lev deque (local push/pop, remote steal)
//! - Scheduler coordinates task placement and steal patterns
//! - Generation counters prevent ABA during steals
//!
//! # Performance Targets (B32 Framework)
//!
//! - schedule_task: <50ns (round-robin + push)
//! - batch_schedule: <30ns per task amortized
//! - steal_work: <100ns (CAS on victim's top)
//!
//! # Safety (ASSUM Framework - 99.5%+)
//!
//! - #ASSUME_CHASE_LEV_CORRECT: Algorithm from 2005 paper
//! - #VERIFY_CHASE_LEV_CORRECT: SeqCst fence at steal
//! - #ASSUME_NUMA_HINT: NUMA hints improve but don't guarantee locality

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::worker_deque::{WorkerDeque, PopResult, StealResult};

// ============================================================================
// SCHEDULER STATE
// ============================================================================

/// Scheduler operational state
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerState {
    /// Scheduler is active and accepting tasks
    Active = 0,
    /// Scheduler is draining (no new tasks)
    Draining = 1,
    /// Scheduler is shut down
    Shutdown = 2,
}

impl SchedulerState {
    #[inline]
    pub const fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(SchedulerState::Active),
            1 => Some(SchedulerState::Draining),
            2 => Some(SchedulerState::Shutdown),
            _ => None,
        }
    }
}

// ============================================================================
// SCHEDULER STATISTICS
// ============================================================================

/// Scheduler statistics snapshot
#[derive(Debug, Clone, Copy, Default)]
pub struct SchedulerStats {
    /// Total tasks scheduled
    pub scheduled: u64,
    /// Tasks stolen (work-stealing)
    pub stolen: u64,
    /// Failed steal attempts
    pub steal_failures: u64,
    /// Batch operations
    pub batches: u64,
}

// ============================================================================
// STEAL PATTERN
// ============================================================================

/// Work-stealing pattern selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StealPattern {
    /// Round-robin victim selection
    RoundRobin,
    /// Random victim selection
    Random,
    /// NUMA-aware (prefer same socket)
    NumaAware,
    /// Adaptive based on load
    Adaptive,
}

impl Default for StealPattern {
    fn default() -> Self {
        StealPattern::RoundRobin
    }
}

// ============================================================================
// WORK STEALING SCHEDULER
// ============================================================================

/// WorkStealingScheduler - T4 Batch Tier Task Distribution
///
/// # Memory Layout (256B, 64B aligned)
///
/// ```text
/// Offset 0-7:     state (AtomicU64: state + generation)
/// Offset 8-15:    scheduled_count (AtomicU64)
/// Offset 16-23:   stolen_count (AtomicU64)
/// Offset 24-31:   steal_fail_count (AtomicU64)
/// Offset 32-39:   batch_count (AtomicU64)
/// Offset 40-47:   next_worker (AtomicUsize: round-robin counter)
/// Offset 48-55:   steal_hint (AtomicUsize: last successful steal target)
/// Offset 56-63:   num_workers (usize)
/// Offset 64-127:  cache line padding
/// Offset 128-255: workers pointer + pattern + padding
/// ```
#[repr(C, align(64))]
pub struct WorkStealingScheduler {
    // === Cache Line 1: Hot Path ===
    /// State (low 32 bits) + generation (high 32 bits)
    state: AtomicU64,
    /// Total scheduled tasks
    scheduled_count: AtomicU64,
    /// Total stolen tasks
    stolen_count: AtomicU64,
    /// Failed steal attempts
    steal_fail_count: AtomicU64,
    /// Batch operation count
    batch_count: AtomicU64,
    /// Round-robin worker counter
    next_worker: AtomicUsize,
    /// Hint for steal target
    steal_hint: AtomicUsize,
    /// Number of workers
    num_workers: usize,

    // === Cache Line 2 ===
    _padding_hot: [u8; 64],

    // === Cache Line 3: Cold Path ===
    /// Worker deques (owned)
    workers: Option<Box<[WorkerDeque]>>,
    /// Steal pattern
    pattern: StealPattern,
    /// NUMA node mapping (optional)
    numa_mapping: Option<Box<[u8]>>,
    /// Padding to 256B
    _padding_cold: [u8; 32],
}

// Compile-time verification
const _: () = {
    assert!(core::mem::size_of::<WorkStealingScheduler>() <= 256);
    assert!(core::mem::align_of::<WorkStealingScheduler>() >= 64);
};

// SAFETY: WorkStealingScheduler uses atomic operations for all shared state
unsafe impl Send for WorkStealingScheduler {}
unsafe impl Sync for WorkStealingScheduler {}

/// Result type for scheduler operations
pub type SchedulerResult<T> = Result<T, SchedulerError>;

/// Error type for scheduler operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerError {
    /// Scheduler not initialized
    NotInitialized,
    /// Scheduler is shutting down
    ShuttingDown,
    /// Worker queue full
    QueueFull,
    /// Invalid worker ID
    InvalidWorker,
    /// No work available
    NoWork,
}

impl core::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotInitialized => write!(f, "scheduler not initialized"),
            Self::ShuttingDown => write!(f, "scheduler is shutting down"),
            Self::QueueFull => write!(f, "worker queue full"),
            Self::InvalidWorker => write!(f, "invalid worker ID"),
            Self::NoWork => write!(f, "no work available"),
        }
    }
}

impl std::error::Error for SchedulerError {}

impl WorkStealingScheduler {
    /// Create new scheduler with specified worker count
    ///
    /// # Arguments
    ///
    /// * `num_workers` - Number of worker deques to create
    /// * `pattern` - Work-stealing pattern to use
    ///
    /// # Performance
    ///
    /// - Time: O(num_workers) for initialization
    /// - Memory: ~256B per worker deque
    pub fn new(num_workers: usize, pattern: StealPattern) -> SchedulerResult<Self> {
        if num_workers == 0 {
            return Err(SchedulerError::NotInitialized);
        }

        // Initialize worker deques
        let mut workers_vec = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            workers_vec.push(WorkerDeque::new());
        }
        let workers = workers_vec.into_boxed_slice();

        Ok(Self {
            state: AtomicU64::new(SchedulerState::Active as u64),
            scheduled_count: AtomicU64::new(0),
            stolen_count: AtomicU64::new(0),
            steal_fail_count: AtomicU64::new(0),
            batch_count: AtomicU64::new(0),
            next_worker: AtomicUsize::new(0),
            steal_hint: AtomicUsize::new(0),
            num_workers,
            _padding_hot: [0u8; 64],
            workers: Some(workers),
            pattern,
            numa_mapping: None,
            _padding_cold: [0u8; 32],
        })
    }

    /// Create scheduler with default round-robin pattern
    pub fn with_round_robin(num_workers: usize) -> SchedulerResult<Self> {
        Self::new(num_workers, StealPattern::RoundRobin)
    }

    /// Set NUMA node mapping for workers
    ///
    /// # Arguments
    ///
    /// * `mapping` - Slice where mapping[worker_id] = numa_node
    pub fn set_numa_mapping(&mut self, mapping: &[u8]) -> SchedulerResult<()> {
        if mapping.len() != self.num_workers {
            return Err(SchedulerError::InvalidWorker);
        }

        self.numa_mapping = Some(mapping.to_vec().into_boxed_slice());
        Ok(())
    }

    // ========================================================================
    // SCHEDULING
    // ========================================================================

    /// Schedule a single task to a worker
    ///
    /// # Arguments
    ///
    /// * `task_index` - Task slot index to schedule
    ///
    /// # Returns
    ///
    /// Worker ID where task was scheduled
    ///
    /// # Performance (B32 Target)
    ///
    /// - Time: <50ns (round-robin + push)
    pub fn schedule(&self, task_index: u32) -> SchedulerResult<usize> {
        if !self.is_active() {
            return Err(SchedulerError::ShuttingDown);
        }

        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        // Select worker (round-robin)
        let worker_id = self.next_worker.fetch_add(1, Ordering::Relaxed) % self.num_workers;

        // Try to push to selected worker
        if workers[worker_id].push(task_index) {
            self.scheduled_count.fetch_add(1, Ordering::Relaxed);
            return Ok(worker_id);
        }

        // Worker full, try others
        for i in 1..self.num_workers {
            let target = (worker_id + i) % self.num_workers;
            if workers[target].push(task_index) {
                self.scheduled_count.fetch_add(1, Ordering::Relaxed);
                return Ok(target);
            }
        }

        Err(SchedulerError::QueueFull)
    }

    /// Schedule task to specific worker (NUMA-aware)
    ///
    /// # Arguments
    ///
    /// * `task_index` - Task slot index to schedule
    /// * `preferred_worker` - Preferred worker ID
    ///
    /// # Performance
    ///
    /// - Time: <30ns (direct push)
    pub fn schedule_to(&self, task_index: u32, preferred_worker: usize) -> SchedulerResult<usize> {
        if !self.is_active() {
            return Err(SchedulerError::ShuttingDown);
        }

        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        if preferred_worker >= workers.len() {
            return Err(SchedulerError::InvalidWorker);
        }

        // Try preferred worker first
        if workers[preferred_worker].push(task_index) {
            self.scheduled_count.fetch_add(1, Ordering::Relaxed);
            return Ok(preferred_worker);
        }

        // Fallback to round-robin
        self.schedule(task_index)
    }

    /// Schedule a batch of tasks
    ///
    /// # Arguments
    ///
    /// * `tasks` - Slice of task indices to schedule
    ///
    /// # Returns
    ///
    /// Number of tasks successfully scheduled
    ///
    /// # Performance (B32 Target)
    ///
    /// - Time: <30ns per task amortized
    pub fn schedule_batch(&self, tasks: &[u32]) -> SchedulerResult<usize> {
        if !self.is_active() {
            return Err(SchedulerError::ShuttingDown);
        }

        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        let mut scheduled = 0;
        let mut worker_id = self.next_worker.load(Ordering::Relaxed);

        for &task_index in tasks {
            // Distribute across workers
            let target = worker_id % self.num_workers;

            if workers[target].push(task_index) {
                scheduled += 1;
                worker_id = worker_id.wrapping_add(1);
            } else {
                // Try next worker
                for i in 1..self.num_workers {
                    let alt = (target + i) % self.num_workers;
                    if workers[alt].push(task_index) {
                        scheduled += 1;
                        break;
                    }
                }
            }
        }

        // Update counters
        self.next_worker.store(worker_id % self.num_workers, Ordering::Relaxed);
        self.scheduled_count.fetch_add(scheduled as u64, Ordering::Relaxed);
        self.batch_count.fetch_add(1, Ordering::Relaxed);

        Ok(scheduled)
    }

    // ========================================================================
    // WORK STEALING
    // ========================================================================

    /// Pop task from worker's local queue
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Worker ID to pop from
    ///
    /// # Performance
    ///
    /// - Time: <30ns (local LIFO pop)
    pub fn pop_local(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        if worker_id >= workers.len() {
            return Err(SchedulerError::InvalidWorker);
        }

        match workers[worker_id].pop() {
            PopResult::Success(task) => Ok(Some(task)),
            PopResult::Empty => Ok(None),
        }
    }

    /// Steal work from another worker
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Current worker (will not steal from self)
    ///
    /// # Returns
    ///
    /// Stolen task index, or None if no work available
    ///
    /// # Performance (B32 Target)
    ///
    /// - Time: <100ns (CAS on victim's top)
    pub fn steal_from_other(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        if worker_id >= workers.len() {
            return Err(SchedulerError::InvalidWorker);
        }

        match self.pattern {
            StealPattern::RoundRobin => self.steal_round_robin(worker_id),
            StealPattern::Random => self.steal_random(worker_id),
            StealPattern::NumaAware => self.steal_numa_aware(worker_id),
            StealPattern::Adaptive => self.steal_adaptive(worker_id),
        }
    }

    /// Round-robin steal pattern
    fn steal_round_robin(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;
        let hint = self.steal_hint.load(Ordering::Relaxed);

        for i in 0..self.num_workers {
            let victim = (hint + i) % self.num_workers;
            if victim == worker_id {
                continue;
            }

            match workers[victim].steal() {
                StealResult::Success(task) => {
                    self.stolen_count.fetch_add(1, Ordering::Relaxed);
                    self.steal_hint.store((victim + 1) % self.num_workers, Ordering::Relaxed);
                    return Ok(Some(task));
                }
                StealResult::Empty => continue,
                StealResult::Retry => {
                    self.steal_fail_count.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// Random steal pattern (uses xorshift for speed)
    fn steal_random(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        // Simple xorshift for pseudo-random
        let mut seed = (worker_id as u64).wrapping_mul(0x9E3779B97F4A7C15);
        seed ^= seed >> 12;
        seed ^= seed << 25;
        seed ^= seed >> 27;

        let start = (seed as usize) % self.num_workers;

        for i in 0..self.num_workers {
            let victim = (start + i) % self.num_workers;
            if victim == worker_id {
                continue;
            }

            match workers[victim].steal() {
                StealResult::Success(task) => {
                    self.stolen_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(task));
                }
                StealResult::Empty => continue,
                StealResult::Retry => {
                    self.steal_fail_count.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            }
        }

        Ok(None)
    }

    /// NUMA-aware steal pattern (prefer same socket)
    fn steal_numa_aware(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        // Get worker's NUMA node
        let my_node = self.numa_mapping
            .as_ref()
            .map(|m| m.get(worker_id).copied().unwrap_or(0))
            .unwrap_or(0);

        // First pass: same NUMA node
        for victim in 0..self.num_workers {
            if victim == worker_id {
                continue;
            }

            let victim_node = self.numa_mapping
                .as_ref()
                .map(|m| m.get(victim).copied().unwrap_or(0))
                .unwrap_or(0);

            if victim_node != my_node {
                continue; // Skip different NUMA nodes first pass
            }

            match workers[victim].steal() {
                StealResult::Success(task) => {
                    self.stolen_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(task));
                }
                StealResult::Empty => continue,
                StealResult::Retry => continue,
            }
        }

        // Second pass: any NUMA node
        self.steal_round_robin(worker_id)
    }

    /// Adaptive steal pattern (uses hint + load sensing)
    fn steal_adaptive(&self, worker_id: usize) -> SchedulerResult<Option<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;
        let hint = self.steal_hint.load(Ordering::Relaxed);

        // Try hint first
        if hint != worker_id && hint < workers.len() {
            match workers[hint].steal() {
                StealResult::Success(task) => {
                    self.stolen_count.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(task));
                }
                StealResult::Empty | StealResult::Retry => {}
            }
        }

        // Find busiest worker
        let mut busiest = 0;
        let mut max_len = 0;

        for i in 0..self.num_workers {
            if i == worker_id {
                continue;
            }
            let len = workers[i].len();
            if len > max_len {
                max_len = len;
                busiest = i;
            }
        }

        if max_len == 0 {
            return Ok(None);
        }

        // Steal from busiest
        match workers[busiest].steal() {
            StealResult::Success(task) => {
                self.stolen_count.fetch_add(1, Ordering::Relaxed);
                self.steal_hint.store(busiest, Ordering::Relaxed);
                Ok(Some(task))
            }
            StealResult::Empty => Ok(None),
            StealResult::Retry => {
                self.steal_fail_count.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
        }
    }

    /// Steal batch of tasks (for bulk rebalancing)
    pub fn steal_batch(&self, worker_id: usize, max_count: usize) -> SchedulerResult<Vec<u32>> {
        let workers = self.workers.as_ref().ok_or(SchedulerError::NotInitialized)?;

        if worker_id >= workers.len() {
            return Err(SchedulerError::InvalidWorker);
        }

        let mut stolen = Vec::with_capacity(max_count.min(16));
        let hint = self.steal_hint.load(Ordering::Relaxed);

        for i in 0..self.num_workers {
            let victim = (hint + i) % self.num_workers;
            if victim == worker_id {
                continue;
            }

            let batch = workers[victim].steal_batch(max_count - stolen.len());
            stolen.extend(batch);

            if stolen.len() >= max_count {
                break;
            }
        }

        if !stolen.is_empty() {
            self.stolen_count.fetch_add(stolen.len() as u64, Ordering::Relaxed);
        }

        Ok(stolen)
    }

    // ========================================================================
    // STATE MANAGEMENT
    // ========================================================================

    /// Get current scheduler state
    #[inline]
    pub fn state(&self) -> SchedulerState {
        let packed = self.state.load(Ordering::Acquire);
        SchedulerState::from_u32((packed & 0xFFFFFFFF) as u32)
            .unwrap_or(SchedulerState::Shutdown)
    }

    /// Check if scheduler is active
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state() == SchedulerState::Active
    }

    /// Start draining (no new tasks)
    pub fn start_drain(&self) -> bool {
        let current = self.state.load(Ordering::Acquire);
        let current_state = (current & 0xFFFFFFFF) as u32;

        if current_state != SchedulerState::Active as u32 {
            return false;
        }

        let new = ((current >> 32) + 1) << 32 | (SchedulerState::Draining as u64);
        self.state.compare_exchange(current, new, Ordering::AcqRel, Ordering::Acquire).is_ok()
    }

    /// Shutdown scheduler
    pub fn shutdown(&self) {
        let current = self.state.load(Ordering::Acquire);
        let gen = (current >> 32) + 1;
        let new = (gen << 32) | (SchedulerState::Shutdown as u64);
        self.state.store(new, Ordering::Release);
    }

    // ========================================================================
    // STATISTICS
    // ========================================================================

    /// Get scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        SchedulerStats {
            scheduled: self.scheduled_count.load(Ordering::Relaxed),
            stolen: self.stolen_count.load(Ordering::Relaxed),
            steal_failures: self.steal_fail_count.load(Ordering::Relaxed),
            batches: self.batch_count.load(Ordering::Relaxed),
        }
    }

    /// Get number of workers
    #[inline]
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Get total pending tasks across all workers
    pub fn pending_count(&self) -> usize {
        self.workers
            .as_ref()
            .map(|w| w.iter().map(|d| d.len()).sum())
            .unwrap_or(0)
    }

    /// Get worker queue lengths
    pub fn worker_loads(&self) -> Vec<usize> {
        self.workers
            .as_ref()
            .map(|w| w.iter().map(|d| d.len()).collect())
            .unwrap_or_default()
    }
}

impl Default for WorkStealingScheduler {
    fn default() -> Self {
        let num_workers = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(4);
        Self::new(num_workers, StealPattern::RoundRobin)
            .expect("Failed to create WorkStealingScheduler")
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();
        assert_eq!(scheduler.num_workers(), 4);
        assert!(scheduler.is_active());
    }

    #[test]
    fn test_schedule_single() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        let worker = scheduler.schedule(1).unwrap();
        assert!(worker < 4);
        assert_eq!(scheduler.stats().scheduled, 1);
    }

    #[test]
    fn test_schedule_round_robin() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        let mut workers = Vec::new();
        for i in 0..8 {
            let worker = scheduler.schedule(i).unwrap();
            workers.push(worker);
        }

        // Should distribute 0,1,2,3,0,1,2,3
        for i in 0..8 {
            assert_eq!(workers[i], i % 4);
        }
    }

    #[test]
    fn test_schedule_batch() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        let tasks: Vec<u32> = (0..20).collect();
        let scheduled = scheduler.schedule_batch(&tasks).unwrap();

        assert_eq!(scheduled, 20);
        assert_eq!(scheduler.stats().scheduled, 20);
        assert_eq!(scheduler.stats().batches, 1);
    }

    #[test]
    fn test_pop_local() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        scheduler.schedule_to(42, 0).unwrap();

        let task = scheduler.pop_local(0).unwrap();
        assert_eq!(task, Some(42));

        let empty = scheduler.pop_local(0).unwrap();
        assert_eq!(empty, None);
    }

    #[test]
    fn test_steal_from_other() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        // Schedule tasks to worker 0
        for i in 0..5 {
            scheduler.schedule_to(i, 0).unwrap();
        }

        // Worker 1 steals from worker 0
        let stolen = scheduler.steal_from_other(1).unwrap();
        assert!(stolen.is_some());
        assert_eq!(scheduler.stats().stolen, 1);
    }

    #[test]
    fn test_steal_batch() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        // Schedule many tasks to worker 0
        for i in 0..10 {
            scheduler.schedule_to(i, 0).unwrap();
        }

        // Worker 1 batch steals
        let stolen = scheduler.steal_batch(1, 5).unwrap();
        assert!(stolen.len() <= 5);
        assert!(!stolen.is_empty());
    }

    #[test]
    fn test_shutdown() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        assert!(scheduler.is_active());

        scheduler.start_drain();
        assert_eq!(scheduler.state(), SchedulerState::Draining);

        scheduler.shutdown();
        assert_eq!(scheduler.state(), SchedulerState::Shutdown);
    }

    #[test]
    fn test_concurrent_schedule_steal() {
        let scheduler = Arc::new(WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap());

        let handles: Vec<_> = (0..4).map(|worker_id| {
            let s = Arc::clone(&scheduler);
            thread::spawn(move || {
                // Each worker schedules and steals
                for i in 0..100 {
                    let _ = s.schedule_to(i, worker_id);

                    // Try to steal
                    let _ = s.steal_from_other(worker_id);

                    // Pop local
                    let _ = s.pop_local(worker_id);
                }
            })
        }).collect();

        for h in handles {
            h.join().unwrap();
        }

        let stats = scheduler.stats();
        assert!(stats.scheduled > 0);
    }

    #[test]
    fn test_worker_loads() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::RoundRobin).unwrap();

        // Schedule different amounts to each worker
        for _ in 0..10 { scheduler.schedule_to(1, 0).unwrap(); }
        for _ in 0..5 { scheduler.schedule_to(1, 1).unwrap(); }

        let loads = scheduler.worker_loads();
        assert_eq!(loads[0], 10);
        assert_eq!(loads[1], 5);
        assert_eq!(loads[2], 0);
        assert_eq!(loads[3], 0);
    }

    #[test]
    fn test_adaptive_steal() {
        let scheduler = WorkStealingScheduler::new(4, StealPattern::Adaptive).unwrap();

        // Load up worker 2 heavily
        for i in 0..20 {
            scheduler.schedule_to(i, 2).unwrap();
        }

        // Worker 0 should preferentially steal from worker 2
        let stolen = scheduler.steal_from_other(0).unwrap();
        assert!(stolen.is_some());
    }
}
