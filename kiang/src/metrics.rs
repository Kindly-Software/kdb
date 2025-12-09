//! GPU Performance Metrics
//!
//! Atomic metrics collection for GPU performance monitoring following
//! lockfree coordination patterns.

use std::sync::atomic::{AtomicU64, Ordering};

/// GPU performance metrics (lockfree atomic counters)
#[repr(C, align(64))]
pub struct GpuMetrics {
    /// Total frames rendered
    frames_rendered: AtomicU64,
    /// Total commands submitted
    commands_submitted: AtomicU64,
    /// Total GPU errors encountered
    errors: AtomicU64,
    /// Total bytes allocated
    bytes_allocated: AtomicU64,
    _pad: [u8; 32], // Padding to 64 bytes
}

impl Default for GpuMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl GpuMetrics {
    /// Create new metrics collector
    pub const fn new() -> Self {
        Self {
            frames_rendered: AtomicU64::new(0),
            commands_submitted: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            bytes_allocated: AtomicU64::new(0),
            _pad: [0; 32],
        }
    }

    /// Increment frame count (atomic)
    pub fn inc_frames(&self) {
        self.frames_rendered.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment command count (atomic)
    pub fn inc_commands(&self, count: u64) {
        self.commands_submitted.fetch_add(count, Ordering::Relaxed);
    }

    /// Increment error count (atomic)
    pub fn inc_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Update allocated bytes (atomic)
    pub fn set_allocated(&self, bytes: u64) {
        self.bytes_allocated.store(bytes, Ordering::Relaxed);
    }

    /// Get snapshot of metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            frames_rendered: self.frames_rendered.load(Ordering::Relaxed),
            commands_submitted: self.commands_submitted.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            bytes_allocated: self.bytes_allocated.load(Ordering::Relaxed),
        }
    }

    /// Get error rate (errors per 1000 commands)
    pub fn error_rate(&self) -> u16 {
        let commands = self.commands_submitted.load(Ordering::Relaxed);
        let errors = self.errors.load(Ordering::Relaxed);

        if commands > 0 {
            // Return as errors per 1000 commands for integer representation
            ((errors * 1000) / commands).min(u16::MAX as u64) as u16
        } else {
            0
        }
    }

    /// Get memory usage percentage (0-100)
    pub fn memory_usage_pct(&self) -> u8 {
        let allocated = self.bytes_allocated.load(Ordering::Relaxed);
        // Assume 8GB total VRAM for Intel Arc A750/A770
        const TOTAL_VRAM: u64 = 8 * 1024 * 1024 * 1024;

        ((allocated * 100) / TOTAL_VRAM).min(100) as u8
    }
}

/// Snapshot of GPU metrics
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    /// Total frames rendered
    pub frames_rendered: u64,
    /// Total commands submitted
    pub commands_submitted: u64,
    /// Total errors
    pub errors: u64,
    /// Current allocated bytes
    pub bytes_allocated: u64,
}

impl MetricsSnapshot {
    /// Calculate commands per frame
    pub fn commands_per_frame(&self) -> f64 {
        if self.frames_rendered > 0 {
            self.commands_submitted as f64 / self.frames_rendered as f64
        } else {
            0.0
        }
    }

    /// Calculate error rate (errors per command)
    pub fn error_rate(&self) -> f64 {
        if self.commands_submitted > 0 {
            self.errors as f64 / self.commands_submitted as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_basic() {
        let metrics = GpuMetrics::new();

        metrics.inc_frames();
        metrics.inc_commands(10);
        metrics.inc_errors();
        metrics.set_allocated(1024 * 1024);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.frames_rendered, 1);
        assert_eq!(snapshot.commands_submitted, 10);
        assert_eq!(snapshot.errors, 1);
        assert_eq!(snapshot.bytes_allocated, 1024 * 1024);
    }

    #[test]
    fn test_metrics_calculations() {
        let metrics = GpuMetrics::new();

        metrics.inc_frames();
        metrics.inc_frames();
        metrics.inc_commands(20);
        metrics.inc_errors();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.commands_per_frame(), 10.0);
        assert_eq!(snapshot.error_rate(), 0.05);
    }
}
