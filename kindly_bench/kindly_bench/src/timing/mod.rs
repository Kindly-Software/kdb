//! Timing infrastructure for benchmarks
//!
//! Provides high-precision timing using TSC (Time Stamp Counter) for x86_64 platforms.
//! Falls back to std::time::Instant for other platforms or when TSC is unavailable.

pub mod tsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    /// TSC timing (cycle-accurate, x86_64 only)
    Tsc,
    /// Instant timing (std::time::Instant, portable)
    Instant,
}

/// Trait for timing measurements
pub trait Timer {
    type Duration;

    /// Start timing
    fn start(&self) -> Self::Duration;

    /// End timing
    fn end(&self) -> Self::Duration;

    /// Calculate elapsed time in nanoseconds
    fn elapsed_ns(&self, start: Self::Duration, end: Self::Duration) -> u64;
}

/// Select appropriate timer for the platform
pub fn default_timer() -> TimerKind {
    #[cfg(all(target_arch = "x86_64", feature = "tsc-timing"))]
    {
        TimerKind::Tsc
    }

    #[cfg(not(all(target_arch = "x86_64", feature = "tsc-timing")))]
    {
        TimerKind::Instant
    }
}
