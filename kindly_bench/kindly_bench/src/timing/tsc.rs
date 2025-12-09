//! TSC (Time Stamp Counter) timing for cycle-accurate measurements
//!
//! Uses RDTSC instruction with serialization for accurate timing.
//! Calibrates TSC frequency and overhead for conversion to nanoseconds.

use super::Timer;

/// TSC timer for cycle-accurate timing (x86_64 only)
#[derive(Debug, Clone, Copy)]
pub struct TscTimer {
    /// TSC frequency in Hz (calibrated on creation)
    frequency_hz: u64,
    /// Overhead in cycles (calibrated on creation)
    overhead_cycles: u64,
}

impl TscTimer {
    /// Create a new TSC timer with calibration
    pub fn new() -> Self {
        Self {
            frequency_hz: Self::calibrate_frequency(),
            overhead_cycles: Self::calibrate_overhead(),
        }
    }

    /// Calibrate TSC frequency (Hz) using std::time::Instant
    fn calibrate_frequency() -> u64 {
        #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
        {
            use std::time::Instant;
            use std::thread::sleep;
            use std::time::Duration;

            // Sample TSC at two points with known time interval
            let start_tsc = unsafe { Self::rdtsc() };
            let start_time = Instant::now();

            // Wait 10ms for reasonable accuracy
            sleep(Duration::from_millis(10));

            let end_tsc = unsafe { Self::rdtsc() };
            let end_time = Instant::now();

            let elapsed_ns = end_time.duration_since(start_time).as_nanos() as u64;
            let elapsed_cycles = end_tsc - start_tsc;

            // Calculate frequency: cycles / time = cycles/ns * 1e9 = cycles/second
            (elapsed_cycles * 1_000_000_000) / elapsed_ns
        }

        #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
        {
            // Fallback for non-x86_64 or when tsc-timing not enabled
            2_500_000_000 // Assume 2.5 GHz
        }
    }

    /// Calibrate TSC overhead (cycles) by measuring empty loop
    fn calibrate_overhead() -> u64 {
        #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
        {
            let mut min_overhead = u64::MAX;

            // Run 100 iterations to find minimum overhead
            for _ in 0..100 {
                let start = unsafe { Self::rdtsc() };
                let end = unsafe { Self::rdtsc() };
                let overhead = end.saturating_sub(start);
                min_overhead = min_overhead.min(overhead);
            }

            min_overhead
        }

        #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
        {
            0 // No overhead for fallback
        }
    }

    /// Read TSC (Time Stamp Counter) with serialization
    ///
    /// Uses RDTSCP + LFENCE for accurate measurements:
    /// - RDTSCP: Read TSC with serialization after (prevents speculative execution)
    /// - LFENCE: Memory fence before (prevents instruction reordering)
    #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
    #[inline(always)]
    #[allow(dead_code)]
    unsafe fn rdtsc() -> u64 {
        use core::arch::x86_64::{_mm_lfence, _rdtsc};

        _mm_lfence(); // Serialize before
        let tsc = _rdtsc();
        _mm_lfence(); // Serialize after

        tsc
    }

    #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
    #[inline(always)]
    unsafe fn rdtsc() -> u64 {
        0 // Fallback for non-x86_64
    }

    /// Convert cycles to nanoseconds
    #[inline]
    pub fn cycles_to_ns(&self, cycles: u64) -> u64 {
        if self.frequency_hz == 0 {
            return 0;
        }
        // cycles * 1e9 / frequency_hz
        (cycles * 1_000_000_000) / self.frequency_hz
    }
}

impl Timer for TscTimer {
    type Duration = u64; // Cycles

    #[inline(always)]
    fn start(&self) -> Self::Duration {
        #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
        {
            unsafe { Self::rdtsc() }
        }

        #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
        {
            0
        }
    }

    #[inline(always)]
    fn end(&self) -> Self::Duration {
        #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
        {
            unsafe { Self::rdtsc() }
        }

        #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
        {
            0
        }
    }

    #[inline]
    fn elapsed_ns(&self, start: Self::Duration, end: Self::Duration) -> u64 {
        let cycles = end.saturating_sub(start).saturating_sub(self.overhead_cycles);
        self.cycles_to_ns(cycles)
    }
}

impl Default for TscTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Instant timer (portable fallback)
#[derive(Debug, Clone, Copy)]
pub struct InstantTimer;

impl Timer for InstantTimer {
    type Duration = std::time::Instant;

    #[inline(always)]
    fn start(&self) -> Self::Duration {
        std::time::Instant::now()
    }

    #[inline(always)]
    fn end(&self) -> Self::Duration {
        std::time::Instant::now()
    }

    #[inline]
    fn elapsed_ns(&self, start: Self::Duration, end: Self::Duration) -> u64 {
        end.duration_since(start).as_nanos() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsc_timer_creation() {
        let timer = TscTimer::new();
        assert!(timer.frequency_hz > 0, "TSC frequency should be calibrated");
    }

    #[test]
    fn test_instant_timer() {
        let timer = InstantTimer;
        let start = timer.start();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let end = timer.end();
        let elapsed = timer.elapsed_ns(start, end);

        // Should be at least 1ms (1,000,000 ns)
        assert!(elapsed >= 1_000_000, "Elapsed time should be at least 1ms: {}", elapsed);
    }

    #[test]
    #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
    fn test_tsc_measurement() {
        let timer = TscTimer::new();
        let start = timer.start();

        // Do some work
        let mut sum = 0u64;
        for i in 0..1000 {
            sum = sum.wrapping_add(i);
        }

        let end = timer.end();
        let elapsed = timer.elapsed_ns(start, end);

        // Prevent optimization
        assert!(sum > 0);

        // Should take some time (but less than 1ms)
        assert!(elapsed > 0 && elapsed < 1_000_000, "Elapsed: {} ns", elapsed);
    }
}
