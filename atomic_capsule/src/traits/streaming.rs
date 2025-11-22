//! Tier 5: Streaming Capsule Trait
//!
//! **Continuous windowed computation with O(1) updates**
//!
//! ## UCE33 Q10: Tier 5 (Streaming)
//!
//! Streaming capsules provide windowed aggregation:
//! - **Window sizes**: 60s (60K samples @ 1KHz), 1h (3.6M samples @ 1KHz)
//! - **Update latency**: <1ms per window
//! - **Memory**: O(window_size)
//! - **Use cases**: Moving averages, anomaly detection, real-time monitoring
//!
//! ## Performance Characteristics
//!
//! ```text
//! Naive moving average: O(window_size) per update (recompute sum)
//! Clever moving average: O(1) per update (incremental: sum += new - old)
//!
//! Example (60s window @ 1KHz):
//! - Window size: 60,000 samples
//! - Naive: 60μs per update (60K additions)
//! - Clever: <10ns per update (2 operations: add + subtract)
//! - Speedup: 6,000× via algorithmic optimization
//! ```
//!
//! ## Memory Layout
//!
//! ```text
//! [ Stream Header (128B) ][ Ring Buffer (W × Item) ]
//! ├─ head: AtomicUsize (64B aligned)
//! ├─ tail: AtomicUsize
//! ├─ aggregate: AtomicU64 (running sum for O(1) updates)
//! ├─ generation: AtomicU64 (window consistency)
//! └─ _padding: [u8]
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

/// Tier 5: Streaming Capsule Trait
///
/// Provides windowed streaming computation with configurable latency.
///
/// ## UCE33 Framework Compliance
///
/// - **Q10 (Tier Selection)**: Tier 5 for streaming/windowing
/// - **Q13 (Resources)**: Variable memory (window size dependent)
/// - **Q15 (Scaling)**: O(1) to O(window_size) depending on algorithm
/// - **Q17 (Interface)**: `push`/`window_state`/`aggregate` for streaming
/// - **Q33 (Verification)**: Use `verify_capsule_properties!` for 128B alignment
///
/// ## Safety Requirements
///
/// - **Alignment**: 128-byte (dual cache line for head/tail)
/// - **Atomicity**: Generation counters detect window corruption
/// - **Bounds**: Ring buffer wrap-around handled correctly
///
/// ## Example (Moving Average)
///
/// ```rust,ignore
/// use atomic_capsule::{StreamingCapsule, verify_capsule_properties};
///
/// #[repr(C, align(128))]
/// struct MovingAverageCapsule<const W: usize> {
///     ring_buffer: [f64; W],
///     head: AtomicUsize,
///     tail: AtomicUsize,
///     sum_fixed: AtomicU64,  // Q16.16 fixed-point sum
///     generation: AtomicU64,
///     _padding: [u8; 48],
/// }
///
/// verify_capsule_properties!(MovingAverageCapsule::<1000>, 128, core::mem::size_of::<MovingAverageCapsule<1000>>());
///
/// impl<const W: usize> StreamingCapsule for MovingAverageCapsule<W> {
///     type Input = f64;
///     type Output = f64;
///     const WINDOW_SIZE: usize = W;
///
///     fn push(&mut self, sample: Self::Input) {
///         let head = self.head.load(Ordering::Acquire);
///         let old_sample = self.ring_buffer[head % W];
///
///         // O(1) update: remove old, add new
///         let old_fixed = (old_sample * 65536.0) as u64;
///         let new_fixed = (sample * 65536.0) as u64;
///         self.sum_fixed.fetch_sub(old_fixed, Ordering::AcqRel);
///         self.sum_fixed.fetch_add(new_fixed, Ordering::AcqRel);
///
///         // Update ring buffer
///         self.ring_buffer[head % W] = sample;
///         self.head.store((head + 1) % W, Ordering::Release);
///         self.generation.fetch_add(1, Ordering::Release);
///     }
///
///     fn aggregate(&self) -> Self::Output {
///         let sum_fixed = self.sum_fixed.load(Ordering::Acquire);
///         (sum_fixed as f64) / 65536.0 / (W as f64)
///     }
/// }
/// ```
pub trait StreamingCapsule: super::ComputationalCapsule {
    /// Input sample type
    type Input: Copy;

    /// Output aggregate type
    type Output: Copy;

    /// Window size (number of samples)
    const WINDOW_SIZE: usize;

    /// Push new sample into window
    ///
    /// ## Performance
    ///
    /// - **Latency**: <1ms per window update
    /// - **Algorithm**: O(1) for moving average/sum
    /// - **Algorithm**: O(log W) for moving median (heap-based)
    /// - **Algorithm**: O(W) for moving median (naive sort)
    fn push(&mut self, sample: Self::Input);

    /// Get current window aggregate
    ///
    /// Thread-safe read via atomic operations.
    ///
    /// ## Performance
    ///
    /// - **Latency**: <100ns (atomic load + divide)
    fn aggregate(&self) -> Self::Output;

    /// Get window size
    fn window_size(&self) -> usize {
        Self::WINDOW_SIZE
    }

    /// Check if window is full
    fn is_full(&self) -> bool {
        self.count() >= Self::WINDOW_SIZE
    }

    /// Get current sample count
    ///
    /// Returns min(samples_pushed, WINDOW_SIZE).
    fn count(&self) -> usize {
        let gen = self.stream_generation().load(Ordering::Acquire);
        core::cmp::min(gen as usize, Self::WINDOW_SIZE)
    }

    /// Get window generation (total samples pushed)
    ///
    /// # Safety
    ///
    /// Implementers must provide a valid AtomicU64 reference.
    fn stream_generation(&self) -> &AtomicU64;
}

/// Streaming operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamError {
    /// Window full (no more space)
    Full,
    /// Window corruption detected
    Corruption,
    /// Invalid sample
    Invalid,
}

#[cfg(feature = "std")]
impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::Full => write!(f, "window full"),
            StreamError::Corruption => write!(f, "window corruption"),
            StreamError::Invalid => write!(f, "invalid sample"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for StreamError {}
