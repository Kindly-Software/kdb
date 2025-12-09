//! Stress Test Harness Capsule (T1 Atomic tier)
//!
//! ## Purpose
//! Lockfree metrics collection for stress testing without observer overhead.
//! 100% atomic operations - zero mutex/RwLock in measurement path.
//!
//! ## Performance Targets
//! - Metric recording: <10ns (single atomic op)
//! - Histogram update: <20ns (bucket selection + atomic increment)
//! - RSS tracking: <5ns (atomic CAS)
//! - Summary generation: <100ns (9 atomic loads)
//!
//! ## Memory Layout (256B aligned)
//! ```text
//! [0-7]     total_ops: AtomicU64
//! [8-15]    success_ops: AtomicU64
//! [16-23]   failed_ops: AtomicU64
//! [24-31]   total_latency_ns: AtomicU64
//! [32-39]   error_count: AtomicU64
//! [40-47]   peak_rss_bytes: AtomicU64
//! [48]      stop_flag: AtomicBool
//! [56-63]   start_ts_ns: AtomicU64
//! [64-255]  _padding: [u8; 192]
//! ```

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Lockfree stress test harness (T1 Atomic tier)
///
/// # Safety
/// - #ASSUME: All atomic operations use correct memory ordering
/// - #VERIFY: Property tests validate concurrent access
/// - #ASSUME: RSS updates via CAS prevent lost writes
/// - #VERIFY: Unit tests validate peak RSS tracking
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct StressTestHarness {
    /// Total operations attempted (lockfree counter)
    total_ops: AtomicU64,

    /// Successful operations (lockfree counter)
    success_ops: AtomicU64,

    /// Failed operations (lockfree counter)
    failed_ops: AtomicU64,

    /// Total latency in nanoseconds (lockfree accumulator)
    total_latency_ns: AtomicU64,

    /// Error counter (lockfree)
    error_count: AtomicU64,

    /// Peak RSS (lockfree, updated via CAS)
    peak_rss_bytes: AtomicU64,

    /// Stop flag for coordinated shutdown
    stop_flag: AtomicBool,

    /// Padding to align AtomicBool
    _padding1: [u8; 7],

    /// Start timestamp (epoch nanos)
    start_ts_ns: AtomicU64,

    /// Padding to 256 bytes
    _padding: [u8; 184],
}

impl StressTestHarness {
    /// Create new harness
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_ops: AtomicU64::new(0),
            success_ops: AtomicU64::new(0),
            failed_ops: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            peak_rss_bytes: AtomicU64::new(0),
            stop_flag: AtomicBool::new(false),
            _padding1: [0u8; 7],
            start_ts_ns: AtomicU64::new(0),
            _padding: [0u8; 184],
        })
    }

    /// Start measurement (record start timestamp)
    #[inline(always)]
    pub fn start(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        self.start_ts_ns.store(now, Ordering::Release);
    }

    /// Signal coordinated stop
    #[inline(always)]
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Release);
    }

    /// Check if should stop
    #[inline(always)]
    pub fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::Acquire)
    }

    /// Record operation result (<10ns)
    ///
    /// # Performance
    /// - Fast path (success): 2 atomic increments + 1 atomic add = <10ns
    /// - Slow path (failure): 2 atomic increments = <5ns
    #[inline(always)]
    pub fn record_op(&self, success: bool, latency_ns: u64) {
        self.total_ops.fetch_add(1, Ordering::Relaxed);
        if success {
            self.success_ops.fetch_add(1, Ordering::Relaxed);
            self.total_latency_ns.fetch_add(latency_ns, Ordering::Relaxed);
        } else {
            self.failed_ops.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Record error (<5ns)
    #[inline(always)]
    pub fn record_error(&self) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Update peak RSS via CAS (<5ns fast path, <50ns slow path)
    ///
    /// # Algorithm
    /// - Load current peak
    /// - If new value > peak, CAS to update
    /// - Retry on CAS failure (contention)
    pub fn update_peak_rss(&self, current_rss: u64) {
        let mut peak = self.peak_rss_bytes.load(Ordering::Relaxed);
        while current_rss > peak {
            match self.peak_rss_bytes.compare_exchange_weak(
                peak,
                current_rss,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => peak = x,
            }
        }
    }

    /// Generate summary (<100ns)
    ///
    /// # Performance
    /// - 8 atomic loads (Acquire) = ~80ns
    /// - Arithmetic operations = ~20ns
    /// - Total: <100ns
    pub fn summary(&self) -> StressTestSummary {
        let total = self.total_ops.load(Ordering::Acquire);
        let success = self.success_ops.load(Ordering::Acquire);
        let failed = self.failed_ops.load(Ordering::Acquire);
        let total_latency = self.total_latency_ns.load(Ordering::Acquire);
        let errors = self.error_count.load(Ordering::Acquire);
        let peak_rss = self.peak_rss_bytes.load(Ordering::Acquire);

        let avg_latency_ns = if success > 0 {
            total_latency / success
        } else {
            0
        };

        let start_ns = self.start_ts_ns.load(Ordering::Acquire);
        let elapsed_ns = (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64)
            .saturating_sub(start_ns);

        let throughput = if elapsed_ns > 0 {
            success as f64 / (elapsed_ns as f64 / 1e9)
        } else {
            0.0
        };

        StressTestSummary {
            total_ops: total,
            success_ops: success,
            failed_ops: failed,
            error_count: errors,
            avg_latency_ns,
            throughput_ops_per_sec: throughput,
            peak_rss_bytes: peak_rss,
            elapsed_secs: elapsed_ns as f64 / 1e9,
        }
    }

    /// Get total operations
    #[inline(always)]
    pub fn total_ops(&self) -> u64 {
        self.total_ops.load(Ordering::Relaxed)
    }

    /// Get successful operations
    #[inline(always)]
    pub fn success_ops(&self) -> u64 {
        self.success_ops.load(Ordering::Relaxed)
    }

    /// Get failed operations
    #[inline(always)]
    pub fn failed_ops(&self) -> u64 {
        self.failed_ops.load(Ordering::Relaxed)
    }
}

impl Default for StressTestHarness {
    fn default() -> Self {
        Self {
            total_ops: AtomicU64::new(0),
            success_ops: AtomicU64::new(0),
            failed_ops: AtomicU64::new(0),
            total_latency_ns: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            peak_rss_bytes: AtomicU64::new(0),
            stop_flag: AtomicBool::new(false),
            _padding1: [0u8; 7],
            start_ts_ns: AtomicU64::new(0),
            _padding: [0u8; 184],
        }
    }
}

/// Summary of stress test results
#[derive(Debug, Clone)]
pub struct StressTestSummary {
    /// Total operations attempted
    pub total_ops: u64,

    /// Successful operations
    pub success_ops: u64,

    /// Failed operations
    pub failed_ops: u64,

    /// Total errors encountered
    pub error_count: u64,

    /// Average latency (nanoseconds)
    pub avg_latency_ns: u64,

    /// Throughput (operations per second)
    pub throughput_ops_per_sec: f64,

    /// Peak RSS (bytes)
    pub peak_rss_bytes: u64,

    /// Elapsed time (seconds)
    pub elapsed_secs: f64,
}

// ============================================================================
// Latency Histogram (Lockfree, T4 Batch tier)
// ============================================================================

/// Lockfree latency histogram with power-of-2 buckets
///
/// # Buckets
/// - [0]: <100ns
/// - [1]: 100ns-1μs
/// - [2]: 1μs-10μs
/// - [3]: 10μs-100μs
/// - [4]: 100μs-1ms
/// - [5]: 1ms-10ms
/// - [6]: 10ms-100ms
/// - [7]: 100ms-1s
/// - [8]: ≥1s
pub struct LatencyHistogram {
    /// Lockfree buckets (9 power-of-2 ranges)
    buckets: [AtomicU64; 9],
}

impl LatencyHistogram {
    /// Create new histogram
    pub fn new() -> Self {
        Self {
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    /// Record latency sample (<20ns)
    ///
    /// # Performance
    /// - Bucket selection: <5ns (branch-free)
    /// - Atomic increment: <10ns
    /// - Total: <20ns
    #[inline(always)]
    pub fn record(&self, latency_ns: u64) {
        let bucket_idx = match latency_ns {
            0..=99 => 0,
            100..=999 => 1,
            1_000..=9_999 => 2,
            10_000..=99_999 => 3,
            100_000..=999_999 => 4,
            1_000_000..=9_999_999 => 5,
            10_000_000..=99_999_999 => 6,
            100_000_000..=999_999_999 => 7,
            _ => 8,
        };
        self.buckets[bucket_idx].fetch_add(1, Ordering::Relaxed);
    }

    /// Calculate percentile (<200ns)
    ///
    /// # Algorithm
    /// - Sum all buckets to get total count
    /// - Calculate target count (total × percentile)
    /// - Walk buckets until cumulative >= target
    /// - Return bucket upper bound
    pub fn percentile(&self, p: f64) -> u64 {
        let total: u64 = self.buckets.iter().map(|b| b.load(Ordering::Relaxed)).sum();

        if total == 0 {
            return 0;
        }

        let target = (total as f64 * p) as u64;
        let mut cumulative = 0u64;

        let bucket_bounds = [
            100,
            1_000,
            10_000,
            100_000,
            1_000_000,
            10_000_000,
            100_000_000,
            1_000_000_000,
            u64::MAX,
        ];

        for (i, &bound) in bucket_bounds.iter().enumerate() {
            cumulative += self.buckets[i].load(Ordering::Relaxed);
            if cumulative >= target {
                return bound;
            }
        }

        u64::MAX
    }

    /// Generate summary with common percentiles
    pub fn summary(&self) -> HistogramSummary {
        HistogramSummary {
            p50: self.percentile(0.50),
            p99: self.percentile(0.99),
            p99_9: self.percentile(0.999),
            p99_99: self.percentile(0.9999),
        }
    }

    /// Get total sample count
    pub fn total(&self) -> u64 {
        self.buckets.iter().map(|b| b.load(Ordering::Relaxed)).sum()
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

/// Histogram summary (common percentiles)
#[derive(Debug, Clone, Copy)]
pub struct HistogramSummary {
    /// 50th percentile (median)
    pub p50: u64,

    /// 99th percentile
    pub p99: u64,

    /// 99.9th percentile
    pub p99_9: u64,

    /// 99.99th percentile
    pub p99_99: u64,
}

// ============================================================================
// Helper Utilities
// ============================================================================

/// Get current RSS (resident set size) in bytes
///
/// # Platform Support
/// - Linux: Reads /proc/self/status
/// - Other: Returns 0 (fallback)
pub fn get_current_rss_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let Ok(kb) = parts[1].parse::<u64>() {
                            return kb * 1024;
                        }
                    }
                }
            }
        }
    }
    0 // Fallback if not available
}

/// Deterministic random number generator (Xorshift64)
///
/// # Purpose
/// - Reproducible test sequences (seeded)
/// - Fast generation (<5ns per value)
/// - No external dependencies
pub struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    /// Create RNG with seed
    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.max(1), // Ensure non-zero state
        }
    }

    /// Generate next u64 (<5ns)
    #[inline(always)]
    pub fn next_u64(&mut self) -> u64 {
        // Xorshift64
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Generate value in range [min, max)
    #[inline(always)]
    pub fn gen_range(&mut self, min: u64, max: u64) -> u64 {
        if max <= min {
            return min;
        }
        min + (self.next_u64() % (max - min))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_harness_creation() {
        let harness = StressTestHarness::new();
        assert_eq!(harness.total_ops(), 0);
        assert_eq!(harness.success_ops(), 0);
        assert_eq!(harness.failed_ops(), 0);
    }

    #[test]
    fn test_record_op() {
        let harness = StressTestHarness::new();

        harness.record_op(true, 100);
        assert_eq!(harness.total_ops(), 1);
        assert_eq!(harness.success_ops(), 1);

        harness.record_op(false, 0);
        assert_eq!(harness.total_ops(), 2);
        assert_eq!(harness.failed_ops(), 1);
    }

    #[test]
    fn test_concurrent_record() {
        const THREADS: usize = 10;
        const OPS_PER_THREAD: usize = 1000;

        let harness = StressTestHarness::new();
        let harness_shared = Arc::clone(&harness);

        let handles: Vec<_> = (0..THREADS)
            .map(|_| {
                let h = Arc::clone(&harness_shared);
                thread::spawn(move || {
                    for _ in 0..OPS_PER_THREAD {
                        h.record_op(true, 100);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(harness_shared.total_ops(), (THREADS * OPS_PER_THREAD) as u64);
        assert_eq!(harness_shared.success_ops(), (THREADS * OPS_PER_THREAD) as u64);
    }

    #[test]
    fn test_histogram() {
        let hist = LatencyHistogram::new();

        hist.record(50);      // <100ns
        hist.record(500);     // 100ns-1μs
        hist.record(5_000);   // 1μs-10μs
        hist.record(50_000);  // 10μs-100μs

        assert_eq!(hist.total(), 4);

        let summary = hist.summary();
        assert!(summary.p50 > 0);
    }

    #[test]
    fn test_deterministic_rng() {
        let mut rng1 = DeterministicRng::new(42);
        let mut rng2 = DeterministicRng::new(42);

        // Same seed produces same sequence
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_peak_rss_tracking() {
        let harness = StressTestHarness::new();

        harness.update_peak_rss(1000);
        harness.update_peak_rss(2000);
        harness.update_peak_rss(1500); // Should not update

        let summary = harness.summary();
        assert_eq!(summary.peak_rss_bytes, 2000);
    }
}
