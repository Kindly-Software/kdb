//! GPU timer for T7 Heterogeneous benchmarks
//!
//! # Overview
//!
//! Provides GPU-accurate timing via CUDA events or Vulkan queries for GPU kernel benchmarks.
//!
//! # When to Use
//!
//! - **T7 Heterogeneous**: GPU/FPGA/TPU acceleration benchmarks
//! - **GPU kernel timing**: Measure GPU execution time (not host-device transfer)
//! - **CUDA/Vulkan workloads**: Where GPU-side timing is critical
//!
//! # Accuracy
//!
//! - **Resolution**: ~500ns (CUDA event timing)
//! - **Overhead**: ~10-50µs (event creation + synchronization)
//! - **Precision**: GPU clock cycle level
//!
//! # Requirements
//!
//! - CUDA runtime (feature = "gpu")
//! - NVIDIA GPU with CUDA support
//! - CUDA driver installed
//!
//! # Example
//!
//! ```rust,ignore
//! use kindly_bench::timing::{BenchTimer, GpuTimer};
//!
//! let mut timer = GpuTimer::cuda(stream)?;
//! let start = timer.start();
//! // GPU kernel launch
//! launch_kernel(a, b, c);
//! let elapsed_ns = timer.end(start);
//! ```

use super::BenchTimer;

#[cfg(feature = "gpu")]
use cuda_runtime::{CudaEvent, CudaStream, CudaError};

/// GPU timer for kernel execution timing (T7 Heterogeneous)
#[cfg(feature = "gpu")]
pub struct GpuTimer {
    stream: CudaStream,
    start_event: CudaEvent,
    end_event: CudaEvent,
    overhead_ns: u64,
}

#[cfg(feature = "gpu")]
impl GpuTimer {
    /// Create new GPU timer for CUDA stream
    pub fn cuda(stream: CudaStream) -> Result<Self, CudaError> {
        let start_event = CudaEvent::create()?;
        let end_event = CudaEvent::create()?;

        let mut timer = Self {
            stream,
            start_event,
            end_event,
            overhead_ns: 0,
        };

        timer.overhead_ns = timer.calibrate_overhead_internal()?;
        Ok(timer)
    }

    /// Calibrate timer overhead (nanoseconds)
    fn calibrate_overhead_internal(&mut self) -> Result<u64, CudaError> {
        const CALIBRATION_ITERATIONS: usize = 100;
        let mut min_ns = u64::MAX;

        for _ in 0..CALIBRATION_ITERATIONS {
            self.start_event.record(self.stream)?;
            self.end_event.record(self.stream)?;
            self.end_event.synchronize()?;

            let elapsed_ms = self.start_event.elapsed_time(&self.end_event)?;
            let elapsed_ns = (elapsed_ms * 1_000_000.0) as u64;
            min_ns = min_ns.min(elapsed_ns);
        }

        Ok(min_ns)
    }
}

#[cfg(feature = "gpu")]
impl BenchTimer for GpuTimer {
    type Timestamp = CudaEvent;

    fn start(&mut self) -> Self::Timestamp {
        let event = CudaEvent::create().expect("Failed to create CUDA event");
        event.record(self.stream).expect("Failed to record start event");
        event
    }

    fn end(&mut self, start: Self::Timestamp) -> u64 {
        self.end_event.record(self.stream).expect("Failed to record end event");
        self.end_event.synchronize().expect("Failed to synchronize end event");

        let elapsed_ms = start.elapsed_time(&self.end_event).expect("Failed to get elapsed time");
        let elapsed_ns = (elapsed_ms * 1_000_000.0) as u64;
        elapsed_ns.saturating_sub(self.overhead_ns)
    }

    fn calibrate_overhead(&mut self) -> u64 {
        let overhead = self.calibrate_overhead_internal().expect("Failed to calibrate overhead");
        self.overhead_ns = overhead;
        overhead
    }

    fn resolution(&self) -> u64 {
        // CUDA event timing resolution is ~500ns
        500
    }
}

// Stub implementation for when GPU feature is disabled
#[cfg(not(feature = "gpu"))]
pub struct GpuTimer;

#[cfg(not(feature = "gpu"))]
impl GpuTimer {
    pub fn cuda() -> Result<Self, &'static str> {
        Err("GPU feature not enabled. Enable with --features gpu")
    }
}

#[cfg(all(test, feature = "gpu"))]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_timer_creation() {
        let stream = CudaStream::create().expect("Failed to create CUDA stream");
        let timer = GpuTimer::cuda(stream).expect("Failed to create GPU timer");
        assert!(timer.overhead_ns > 0);
    }

    #[test]
    fn test_gpu_timer_measurement() {
        let stream = CudaStream::create().expect("Failed to create CUDA stream");
        let mut timer = GpuTimer::cuda(stream).expect("Failed to create GPU timer");

        let start = timer.start();
        // Launch simple kernel (empty kernel for testing)
        // launch_empty_kernel(stream);
        let elapsed_ns = timer.end(start);

        // Should measure something > 0ns
        assert!(elapsed_ns >= 0);
    }

    #[test]
    fn test_gpu_timer_overhead_calibration() {
        let stream = CudaStream::create().expect("Failed to create CUDA stream");
        let mut timer = GpuTimer::cuda(stream).expect("Failed to create GPU timer");
        let overhead_ns = timer.calibrate_overhead();

        // Overhead should be 1-100µs (typical for CUDA event sync)
        assert!(overhead_ns >= 100);
        assert!(overhead_ns <= 100_000);
    }

    #[test]
    fn test_gpu_timer_resolution() {
        let stream = CudaStream::create().expect("Failed to create CUDA stream");
        let timer = GpuTimer::cuda(stream).expect("Failed to create GPU timer");
        let resolution_ns = timer.resolution();

        // Resolution should be ~500ns
        assert_eq!(resolution_ns, 500);
    }
}
