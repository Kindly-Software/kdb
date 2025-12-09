//! Instant timer for T8 Network benchmarks
//!
//! # Overview
//!
//! Provides wall-clock timing via `std::time::Instant` for network/distributed benchmarks.
//!
//! # When to Use
//!
//! - **T8 Network**: Distributed coordination (multi-node benchmarks)
//! - **Wall-clock measurements**: When cycle-accurate TSC isn't appropriate
//! - **Long-running tasks**: Where microsecond precision is sufficient
//!
//! # Accuracy
//!
//! - **Resolution**: ~100ns-1µs (platform-dependent)
//! - **Overhead**: ~20-50ns (Instant::now() call)
//! - **Precision**: Microsecond level
//!
//! # Trade-offs
//!
//! - **Pros**: Works across all platforms, handles long durations, monotonic
//! - **Cons**: Lower precision than TSC, higher overhead, affected by system load

use super::BenchTimer;
use std::time::Instant;

/// Instant timer for wall-clock measurements (T8 Network)
pub struct InstantTimer {
    overhead_ns: u64,
}

impl InstantTimer {
    /// Create new Instant timer with calibrated overhead
    pub fn new() -> Self {
        let mut timer = Self { overhead_ns: 0 };
        timer.overhead_ns = timer.calibrate_overhead_internal();
        timer
    }

    /// Calibrate timer overhead (nanoseconds)
    fn calibrate_overhead_internal(&self) -> u64 {
        const CALIBRATION_ITERATIONS: usize = 1000;
        let mut min_ns = u64::MAX;

        for _ in 0..CALIBRATION_ITERATIONS {
            let start = Instant::now();
            let end = Instant::now();
            let elapsed_ns = end.duration_since(start).as_nanos() as u64;
            min_ns = min_ns.min(elapsed_ns);
        }

        min_ns
    }
}

impl Default for InstantTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTimer for InstantTimer {
    type Timestamp = Instant;

    #[inline]
    fn start(&mut self) -> Self::Timestamp {
        Instant::now()
    }

    #[inline]
    fn end(&mut self, start: Self::Timestamp) -> u64 {
        let end = Instant::now();
        let elapsed_ns = end.duration_since(start).as_nanos() as u64;
        elapsed_ns.saturating_sub(self.overhead_ns)
    }

    fn calibrate_overhead(&mut self) -> u64 {
        let overhead = self.calibrate_overhead_internal();
        self.overhead_ns = overhead;
        overhead
    }

    fn resolution(&self) -> u64 {
        // Instant resolution is platform-dependent
        // Typically 100ns-1µs
        100  // Conservative estimate: 100ns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_instant_timer_creation() {
        let timer = InstantTimer::new();
        assert!(timer.overhead_ns > 0);
    }

    #[test]
    fn test_instant_timer_measurement() {
        let mut timer = InstantTimer::new();
        let start = timer.start();

        // Sleep for 1ms
        thread::sleep(Duration::from_millis(1));

        let elapsed_ns = timer.end(start);

        // Should measure ~1ms (1,000,000ns)
        // Allow ±20% tolerance for scheduling jitter
        assert!(elapsed_ns >= 800_000);
        assert!(elapsed_ns <= 1_200_000);
    }

    #[test]
    fn test_instant_timer_overhead_calibration() {
        let mut timer = InstantTimer::new();
        let overhead_ns = timer.calibrate_overhead();

        // Overhead should be 10-100ns (typical for Instant::now())
        assert!(overhead_ns >= 5);
        assert!(overhead_ns <= 200);
    }

    #[test]
    fn test_instant_timer_resolution() {
        let timer = InstantTimer::new();
        let resolution_ns = timer.resolution();

        // Resolution should be 100ns-1µs
        assert!(resolution_ns >= 10);
        assert!(resolution_ns <= 10_000);
    }

    #[test]
    fn test_instant_timer_monotonic() {
        let mut timer = InstantTimer::new();

        // Multiple measurements should be monotonic
        let mut prev_end = 0u64;
        for _ in 0..100 {
            let start = timer.start();
            thread::sleep(Duration::from_micros(10));
            let end = timer.end(start);
            assert!(end >= prev_end);
            prev_end = end;
        }
    }
}
