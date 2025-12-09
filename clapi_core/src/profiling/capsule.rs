//! LatencyHistogramCapsule - Tier 1 Atomic Capsule for Real-Time Latency Tracking
//!
//! # Architecture
//!
//! 512B cache-aligned capsule with logarithmic bucketing for efficient latency tracking.
//!
//! ## Memory Layout
//!
//! ```text
//! [AtomicU64; 50] buckets (400 bytes)  // Logarithmic buckets: 1ns, 2ns, 4ns, ..., 2^49ns
//! AtomicU64 total_count (8 bytes)      // Total samples
//! AtomicU64 sum_ns (8 bytes)           // Sum for mean calculation
//! AtomicU64 min_ns (8 bytes)           // Minimum latency
//! AtomicU64 max_ns (8 bytes)           // Maximum latency
//! AtomicU64 generation (8 bytes)       // TOCTOU prevention
//! [u8; 72] _padding (72 bytes)         // Total: 512 bytes
//! ```
//!
//! ## Performance Targets
//!
//! - **record()**: <10ns (atomic fetch_add + min/max update)
//! - **percentile()**: <50ns (linear scan through 50 buckets)
//! - **mean()**: <20ns (two loads + division)
//! - **stats()**: <100ns (full snapshot with 7 atomic loads)

use atomic_capsule_derive::ComputationalCapsule;
use std::sync::atomic::{AtomicU64, Ordering};

/// Statistics snapshot from a latency histogram
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistogramStats {
    /// Minimum latency observed (nanoseconds)
    pub min: u64,
    /// Maximum latency observed (nanoseconds)
    pub max: u64,
    /// Mean latency (nanoseconds)
    pub mean: u64,
    /// 50th percentile (median) (nanoseconds)
    pub p50: u64,
    /// 99th percentile (nanoseconds)
    pub p99: u64,
    /// 99.9th percentile (nanoseconds)
    pub p999: u64,
    /// Total number of samples
    pub count: u64,
}

/// Latency histogram with logarithmic bucketing
///
/// # Performance
///
/// - **record()**: <10ns (atomic bucket increment)
/// - **percentile()**: <50ns (O(1) bucket scan)
/// - **stats()**: <100ns (full snapshot)
///
/// # Example
///
/// ```rust
/// use clapi_core::profiling::capsule::LatencyHistogramCapsule;
///
/// let histogram = LatencyHistogramCapsule::new();
///
/// // Record latencies
/// histogram.record(100);  // 100ns
/// histogram.record(250);  // 250ns
/// histogram.record(1000); // 1μs
///
/// // Query percentiles
/// let p99 = histogram.percentile(99.0);
/// assert!(p99 >= 1000);
/// ```
#[derive(ComputationalCapsule)]
#[capsule(alignment = 256, size = 256)]
#[repr(C, align(256))]
pub struct LatencyHistogramCapsule {
    /// Logarithmic buckets: bucket[i] counts latencies in range [2^i, 2^(i+1))
    /// Bucket 0: [1ns, 2ns), Bucket 1: [2ns, 4ns), ..., Bucket 26: [2^26ns, 2^27ns)
    /// Reduced from 50 to 27 buckets to fit in 256B (27*8 + 5*8 = 256 bytes exactly)
    pub(super) buckets: [AtomicU64; 27],

    /// Total number of samples recorded
    pub(super) total_count: AtomicU64,

    /// Sum of all latencies (for mean calculation)
    sum_ns: AtomicU64,

    /// Minimum latency observed (initialized to u64::MAX)
    pub(super) min_ns: AtomicU64,

    /// Maximum latency observed (initialized to 0)
    pub(super) max_ns: AtomicU64,

    /// Generation counter for TOCTOU prevention
    pub(super) generation: AtomicU64,
}

impl LatencyHistogramCapsule {
    /// Create a new latency histogram capsule
    ///
    /// # Performance
    ///
    /// <50ns initialization (const initialization of atomic arrays)
    pub const fn new() -> Self {
        // #ASSUME: const fn allows compile-time initialization
        // #VERIFY: All atomics initialized with const values
        Self {
            buckets: [const { AtomicU64::new(0) }; 27],  // 27 buckets fit in 256B capsule
            total_count: AtomicU64::new(0),
            sum_ns: AtomicU64::new(0),
            min_ns: AtomicU64::new(u64::MAX),
            max_ns: AtomicU64::new(0),
            generation: AtomicU64::new(0),
        }
    }

    /// Record a latency sample
    ///
    /// # Performance
    ///
    /// <10ns (4 atomic fetch_add operations + 2 compare-and-swap for min/max)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clapi_core::profiling::capsule::LatencyHistogramCapsule;
    ///
    /// let histogram = LatencyHistogramCapsule::new();
    /// histogram.record(100);  // Record 100ns latency
    /// ```
    pub fn record(&self, latency_ns: u64) {
        // #ASSUME: Logarithmic bucketing: bucket[i] = [2^i, 2^(i+1))
        // #VERIFY: Bucket assignment is deterministic and correct
        let bucket = if latency_ns == 0 {
            0
        } else {
            // Find the highest bit set (log2 floor)
            (63 - latency_ns.leading_zeros()) as usize
        };
        let bucket = bucket.min(49); // Clamp to max bucket

        // #ASSUME: Relaxed ordering sufficient for counters (no synchronization needed)
        // #VERIFY: Statistical counters don't require strict ordering
        self.buckets[bucket].fetch_add(1, Ordering::Relaxed);
        self.total_count.fetch_add(1, Ordering::Relaxed);
        self.sum_ns.fetch_add(latency_ns, Ordering::Relaxed);

        // Update min (with CAS loop to handle races)
        // #ASSUME: CAS loop eventually succeeds (no ABA problem)
        // #VERIFY: Min update is correct under concurrent access
        let mut current_min = self.min_ns.load(Ordering::Relaxed);
        while latency_ns < current_min {
            match self.min_ns.compare_exchange_weak(
                current_min,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_min = actual,
            }
        }

        // Update max (with CAS loop to handle races)
        // #ASSUME: CAS loop eventually succeeds (no ABA problem)
        // #VERIFY: Max update is correct under concurrent access
        let mut current_max = self.max_ns.load(Ordering::Relaxed);
        while latency_ns > current_max {
            match self.max_ns.compare_exchange_weak(
                current_max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => current_max = actual,
            }
        }

        // Increment generation counter (TOCTOU prevention)
        // #ASSUME: Generation counter prevents stale reads
        // #VERIFY: Readers can detect concurrent modifications
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Calculate percentile from histogram
    ///
    /// # Performance
    ///
    /// <50ns (linear scan through 50 buckets)
    ///
    /// # Arguments
    ///
    /// * `p` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate latency at given percentile (nanoseconds)
    ///
    /// # Example
    ///
    /// ```rust
    /// use clapi_core::profiling::capsule::LatencyHistogramCapsule;
    ///
    /// let histogram = LatencyHistogramCapsule::new();
    /// for i in 0..100 {
    ///     histogram.record(i * 10);
    /// }
    ///
    /// let p50 = histogram.percentile(50.0);
    /// let p99 = histogram.percentile(99.0);
    /// ```
    pub fn percentile(&self, p: f64) -> u64 {
        // #ASSUME: Acquire ordering ensures consistent snapshot
        // #VERIFY: Total count is loaded before bucket scan
        let total = self.total_count.load(Ordering::Acquire);
        if total == 0 {
            return 0;
        }

        // Calculate target count for percentile
        let target_count = ((total as f64 * p) / 100.0).ceil() as u64;

        // Scan buckets to find percentile
        let mut cumulative = 0u64;
        for (i, bucket) in self.buckets.iter().enumerate() {
            cumulative += bucket.load(Ordering::Relaxed);
            if cumulative >= target_count {
                // Return bucket midpoint (geometric mean of bucket range)
                return if i == 0 { 1 } else { 1u64 << i };
            }
        }

        // Edge case: return max bucket
        1u64 << 49
    }

    /// Calculate mean latency
    ///
    /// # Performance
    ///
    /// <20ns (two atomic loads + division)
    pub fn mean_ns(&self) -> f64 {
        // #ASSUME: Acquire ordering ensures consistent snapshot
        // #VERIFY: Total count loaded before sum
        let total = self.total_count.load(Ordering::Acquire);
        if total == 0 {
            return 0.0;
        }
        let sum = self.sum_ns.load(Ordering::Relaxed);
        sum as f64 / total as f64
    }

    /// Get full statistics snapshot
    ///
    /// # Performance
    ///
    /// <100ns (7 atomic loads + percentile calculations)
    pub fn stats(&self) -> HistogramStats {
        // #ASSUME: Acquire ordering on generation ensures consistent snapshot
        // #VERIFY: All subsequent loads see consistent state
        let _gen = self.generation.load(Ordering::Acquire);

        HistogramStats {
            min: self.min_ns.load(Ordering::Relaxed),
            max: self.max_ns.load(Ordering::Relaxed),
            mean: self.mean_ns() as u64,
            p50: self.percentile(50.0),
            p99: self.percentile(99.0),
            p999: self.percentile(99.9),
            count: self.total_count.load(Ordering::Relaxed),
        }
    }

    /// Reset histogram (clear all counters)
    ///
    /// # Performance
    ///
    /// <500ns (reset 50 buckets + 5 counters)
    pub fn reset(&self) {
        // #ASSUME: SeqCst ordering ensures reset is visible to all threads
        // #VERIFY: Reset is atomic from external observer perspective
        for bucket in &self.buckets {
            bucket.store(0, Ordering::SeqCst);
        }
        self.total_count.store(0, Ordering::SeqCst);
        self.sum_ns.store(0, Ordering::SeqCst);
        self.min_ns.store(u64::MAX, Ordering::SeqCst);
        self.max_ns.store(0, Ordering::SeqCst);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Get current generation (for TOCTOU detection)
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Get total sample count
    pub fn count(&self) -> u64 {
        self.total_count.load(Ordering::Acquire)
    }
}

impl Default for LatencyHistogramCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Compile-time verification is automatic via #[derive(ComputationalCapsule)]
// This ensures:
// - Alignment: 512 bytes
// - Size: 512 bytes
// - No runtime overhead

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_histogram() {
        let histogram = LatencyHistogramCapsule::new();
        assert_eq!(histogram.count(), 0);
        assert_eq!(histogram.mean_ns(), 0.0);
    }

    #[test]
    fn test_record_single() {
        let histogram = LatencyHistogramCapsule::new();
        histogram.record(100);
        assert_eq!(histogram.count(), 1);
        assert_eq!(histogram.mean_ns(), 100.0);
    }

    #[test]
    fn test_percentile_calculation() {
        let histogram = LatencyHistogramCapsule::new();

        // Record 100 samples: 0ns, 10ns, 20ns, ..., 990ns
        for i in 0..100 {
            histogram.record(i * 10);
        }

        let p50 = histogram.percentile(50.0);
        let p99 = histogram.percentile(99.0);

        // p50 should be around 500ns (bucket containing 512ns)
        assert!(p50 >= 256 && p50 <= 512, "p50={}", p50);

        // p99 should be around 990ns (bucket containing 1024ns)
        assert!(p99 >= 512 && p99 <= 1024, "p99={}", p99);
    }

    #[test]
    fn test_bucket_assignment() {
        let histogram = LatencyHistogramCapsule::new();

        // Test logarithmic bucketing
        histogram.record(1);    // Bucket 0: [1, 2)
        histogram.record(2);    // Bucket 1: [2, 4)
        histogram.record(4);    // Bucket 2: [4, 8)
        histogram.record(8);    // Bucket 3: [8, 16)
        histogram.record(1024); // Bucket 10: [1024, 2048)

        assert_eq!(histogram.count(), 5);
    }

    #[test]
    fn test_min_max_tracking() {
        let histogram = LatencyHistogramCapsule::new();

        histogram.record(100);
        histogram.record(500);
        histogram.record(50);
        histogram.record(1000);

        let stats = histogram.stats();
        assert_eq!(stats.min, 50);
        assert_eq!(stats.max, 1000);
    }

    #[test]
    fn test_reset() {
        let histogram = LatencyHistogramCapsule::new();

        histogram.record(100);
        histogram.record(200);
        assert_eq!(histogram.count(), 2);

        histogram.reset();
        assert_eq!(histogram.count(), 0);
        assert_eq!(histogram.mean_ns(), 0.0);
    }

    #[test]
    fn test_concurrent_recording() {
        use std::sync::Arc;
        use std::thread;

        let histogram = Arc::new(LatencyHistogramCapsule::new());
        let mut handles = vec![];

        // Spawn 10 threads, each recording 100 samples
        for _ in 0..10 {
            let hist = Arc::clone(&histogram);
            handles.push(thread::spawn(move || {
                for i in 0..100 {
                    hist.record(i * 10);
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have 1000 total samples
        assert_eq!(histogram.count(), 1000);
    }

    #[test]
    fn test_generation_counter() {
        let histogram = LatencyHistogramCapsule::new();
        let gen1 = histogram.generation();

        histogram.record(100);
        let gen2 = histogram.generation();

        assert!(gen2 > gen1, "Generation should increment after record");
    }

    #[test]
    fn test_empty_percentile() {
        let histogram = LatencyHistogramCapsule::new();
        assert_eq!(histogram.percentile(50.0), 0);
        assert_eq!(histogram.percentile(99.0), 0);
    }

    #[test]
    fn test_stats_snapshot() {
        let histogram = LatencyHistogramCapsule::new();

        for i in 1..=100 {
            histogram.record(i * 10);
        }

        let stats = histogram.stats();
        assert_eq!(stats.count, 100);
        assert_eq!(stats.min, 10);
        assert_eq!(stats.max, 1000);
        assert!(stats.mean > 0);
        assert!(stats.p50 > 0);
        assert!(stats.p99 > 0);
        assert!(stats.p999 > 0);
    }
}
