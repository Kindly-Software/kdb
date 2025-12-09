//! T5 Streaming Tier Validation
//!
//! **Purpose**: Validate kindly_bench framework for T5 Streaming tier
//!
//! **Pattern**: Incremental O(1) update vs Batch O(n) rebuild
//!
//! **Example**: Rolling window metrics tracker
//! - **T5 Streaming**: Cached sum (O(1) - constant time regardless of window size)
//! - **Batch Rebuild**: Sum all slots (O(n) - linear time with window size)
//!
//! **Expected Behavior**: Speedup should scale linearly with window size
//! - Window 10: ~10× speedup
//! - Window 100: ~100× speedup
//! - Window 1000: ~1000× speedup
//!
//! **UCE34 Framework**:
//! - Q10: Tier T5 Streaming (O(1) incremental)
//! - Q30: Performance 10-1000× (data-dependent scaling)
//! - Q31: Simplicity - Clear O(1) vs O(n) comparison
//! - Q33: Validation - Multiple window sizes prove scaling

use std::sync::atomic::{AtomicU32, Ordering};
use kindly_bench::{BenchmarkConfig, run_benchmark, Classification, PerformanceTier};

/// T5 Streaming: Rolling window with cached total (O(1))
#[repr(C, align(128))]
pub struct StreamingMetricsOptimized {
    /// Ring buffer of request counts
    window: Vec<AtomicU32>,

    /// Current position in ring buffer
    head: AtomicU32,

    /// Cached total (O(1) access)
    total: AtomicU32,
}

impl StreamingMetricsOptimized {
    pub fn new(window_size: usize) -> Self {
        Self {
            window: (0..window_size).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicU32::new(0),
            total: AtomicU32::new(0),
        }
    }

    pub fn record_request(&self) {
        let head = self.head.load(Ordering::Relaxed) as usize % self.window.len();
        self.window[head].fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn advance_window(&self) {
        let old_head = self.head.fetch_add(1, Ordering::Release) as usize % self.window.len();
        let new_head = (old_head + 1) % self.window.len();

        // Evict old value from total (O(1))
        let evicted = self.window[new_head].swap(0, Ordering::Relaxed);
        self.total.fetch_sub(evicted, Ordering::Relaxed);
    }

    /// O(1) - Uses cached total
    pub fn total_requests_streaming(&self) -> u32 {
        self.total.load(Ordering::Relaxed)
    }

    /// O(n) - Sums all slots (baseline for comparison)
    pub fn total_requests_batch_rebuild(&self) -> u32 {
        self.window
            .iter()
            .map(|slot| slot.load(Ordering::Relaxed))
            .sum()
    }
}

/// Baseline: Batch rebuild (O(n) - sum all slots every time)
#[repr(C, align(128))]
pub struct BatchRebuildMetrics {
    window: Vec<AtomicU32>,
    head: AtomicU32,
}

impl BatchRebuildMetrics {
    pub fn new(window_size: usize) -> Self {
        Self {
            window: (0..window_size).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicU32::new(0),
        }
    }

    pub fn record_request(&self) {
        let head = self.head.load(Ordering::Relaxed) as usize % self.window.len();
        self.window[head].fetch_add(1, Ordering::Relaxed);
    }

    pub fn advance_window(&self) {
        let old_head = self.head.fetch_add(1, Ordering::Release) as usize % self.window.len();
        let new_head = (old_head + 1) % self.window.len();
        self.window[new_head].store(0, Ordering::Relaxed);
    }

    /// O(n) - Always sums all slots
    pub fn total_requests(&self) -> u32 {
        self.window
            .iter()
            .map(|slot| slot.load(Ordering::Relaxed))
            .sum()
    }
}

fn main() {
    println!("=== T5 Streaming Tier Validation ===\n");
    println!("Testing: Rolling window metrics (cached sum vs batch rebuild)\n");

    // Test multiple window sizes to show O(1) vs O(n) scaling
    let window_sizes = [10, 100, 1000, 10000];

    let mut results = Vec::new();

    for window_size in window_sizes {
        println!("\n--- Window Size: {} ---", window_size);

        // Setup: Pre-populate with data
        let streaming = StreamingMetricsOptimized::new(window_size);
        let batch = BatchRebuildMetrics::new(window_size);

        // Add some data to each slot
        for i in 0..window_size {
            streaming.record_request();
            batch.record_request();

            if i > 0 && i % 10 == 0 {
                streaming.advance_window();
                batch.advance_window();
            }
        }

        // Run benchmark
        let config = BenchmarkConfig::new(
            format!("T5_Streaming_Window_{}", window_size),
            "T5-Streaming",
            "BatchRebuild",
        )
        .iterations(10000)
        .warmup(100);

        // Optimized: O(1) streaming total
        let optimized = || {
            std::hint::black_box(streaming.total_requests_streaming());
        };

        // Baseline: O(n) batch rebuild
        let baseline = || {
            std::hint::black_box(batch.total_requests());
        };

        // Run benchmark (this will print results and save XML)
        run_benchmark(config, optimized, baseline);

        // Store result for analysis
        // Note: In a real scenario, we'd parse the XML output or add a return value
        // For now, we'll just demonstrate the API usage
        results.push(window_size);
    }

    println!("\n=== Scaling Analysis ===\n");
    println!("Expected behavior:");
    println!("  - Streaming time: O(1) - constant regardless of window size");
    println!("  - Batch rebuild time: O(n) - linear with window size");
    println!("  - Speedup: Should increase linearly with window size");
    println!("\nExpected tier classification:");
    println!("  - Window 10: ~10× speedup (SUSPICIOUS - requires validation)");
    println!("  - Window 100: ~100× speedup (SUSPICIOUS - requires validation)");
    println!("  - Window 1000: ~1000× speedup (SUSPICIOUS - requires validation)");
    println!("  - Window 10000: ~10000× speedup (SUSPICIOUS - requires validation)");
    println!("\nNote: T5 Streaming often produces SUSPICIOUS results due to massive O(1) vs O(n) advantage.");
    println!("This is expected and demonstrates the power of incremental algorithms.");
    println!("\nXML results saved for each window size.");
}
