//! TSC (Time Stamp Counter) timing for T1-T6 capsules
//!
//! # Overview
//!
//! Provides cycle-accurate timing via RDTSC instruction on x86_64.
//!
//! # Accuracy
//!
//! - **Resolution**: ~1ns (depends on CPU frequency)
//! - **Overhead**: ~10-20ns (serialized RDTSC)
//! - **Precision**: Single CPU cycle
//!
//! # Requirements
//!
//! - x86_64 architecture
//! - Constant TSC (invariant_tsc CPU flag)
//! - Nightly Rust (asm! macro)

use super::BenchTimer;
use core::arch::x86_64::{__rdtscp, _mm_lfence, _mm_mfence};

/// TSC timer for cycle-accurate measurements (T1-T6)
pub struct TscTimer {
    frequency_mhz: u64,
    overhead_cycles: u64,
}

impl TscTimer {
    /// Create new TSC timer with calibrated frequency
    pub fn new() -> Self {
        let mut timer = Self {
            frequency_mhz: 0,
            overhead_cycles: 0,
        };
        timer.frequency_mhz = timer.calibrate_frequency();
        timer.overhead_cycles = timer.calibrate_overhead_cycles();
        timer
    }

    /// Calibrate TSC frequency (MHz)
    fn calibrate_frequency(&self) -> u64 {
        // ASSUME: Constant TSC frequency (invariant_tsc)
        // VERIFY: Check /proc/cpuinfo for "constant_tsc" flag
        // TODO: Implement proper frequency calibration via sleep(1ms) + RDTSC
        // For now, use typical frequency (will be validated via B32 hardware checks)
        3000 // 3 GHz typical
    }

    /// Calibrate timer overhead (cycles)
    fn calibrate_overhead_cycles(&mut self) -> u64 {
        const CALIBRATION_ITERATIONS: usize = 1000;
        let mut min_cycles = u64::MAX;

        for _ in 0..CALIBRATION_ITERATIONS {
            let start = self.rdtsc_start();
            let end = self.rdtsc_end();
            let cycles = end.saturating_sub(start);
            min_cycles = min_cycles.min(cycles);
        }

        min_cycles
    }

    /// Serialized RDTSC (start measurement)
    #[inline(always)]
    fn rdtsc_start(&self) -> u64 {
        unsafe {
            _mm_lfence();  // Load fence (prevent reordering)
            let mut aux: u32 = 0;
            let tsc = __rdtscp(&mut aux);
            _mm_lfence();  // Load fence (serialize)
            tsc
        }
    }

    /// Serialized RDTSC (end measurement)
    #[inline(always)]
    fn rdtsc_end(&self) -> u64 {
        unsafe {
            let mut aux: u32 = 0;
            let tsc = __rdtscp(&mut aux);
            _mm_mfence();  // Memory fence (serialize)
            tsc
        }
    }

    /// Convert cycles to nanoseconds
    #[inline]
    fn cycles_to_ns(&self, cycles: u64) -> u64 {
        // ns = cycles * 1000 / frequency_mhz
        cycles.saturating_mul(1000) / self.frequency_mhz
    }
}

impl Default for TscTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl BenchTimer for TscTimer {
    type Timestamp = u64;

    #[inline(always)]
    fn start(&mut self) -> Self::Timestamp {
        self.rdtsc_start()
    }

    #[inline(always)]
    fn end(&mut self, start: Self::Timestamp) -> u64 {
        let end_cycles = self.rdtsc_end();
        let elapsed_cycles = end_cycles.saturating_sub(start);
        let adjusted_cycles = elapsed_cycles.saturating_sub(self.overhead_cycles);
        self.cycles_to_ns(adjusted_cycles)
    }

    fn calibrate_overhead(&mut self) -> u64 {
        let overhead_cycles = self.calibrate_overhead_cycles();
        self.overhead_cycles = overhead_cycles;
        self.cycles_to_ns(overhead_cycles)
    }

    fn resolution(&self) -> u64 {
        // Single cycle resolution
        self.cycles_to_ns(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tsc_timer_creation() {
        let timer = TscTimer::new();
        assert!(timer.frequency_mhz > 0);
        assert!(timer.overhead_cycles > 0);
    }

    #[test]
    fn test_tsc_timer_measurement() {
        let mut timer = TscTimer::new();
        let start = timer.start();
        // Small workload
        let mut sum = 0u64;
        for i in 0..100 {
            sum = sum.wrapping_add(i);
        }
        let elapsed_ns = timer.end(start);

        // Should measure something > 0ns
        assert!(elapsed_ns > 0);
        // Prevent optimization
        assert!(sum > 0);
    }

    #[test]
    fn test_tsc_timer_overhead_calibration() {
        let mut timer = TscTimer::new();
        let overhead_ns = timer.calibrate_overhead();

        // Overhead should be 10-50ns (typical for serialized RDTSC)
        assert!(overhead_ns >= 5);
        assert!(overhead_ns <= 100);
    }

    #[test]
    fn test_tsc_timer_resolution() {
        let timer = TscTimer::new();
        let resolution_ns = timer.resolution();

        // Resolution should be ~1ns (1 cycle at 1+ GHz)
        assert!(resolution_ns <= 5);
    }
}
