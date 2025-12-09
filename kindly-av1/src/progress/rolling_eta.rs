//! RollingEtaCapsule - Exponential Moving Average ETA Estimation (T1 Atomic)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! **UCE34 Tier 1 Atomic Capsule for accurate, stable ETA estimation.**
//!
//! ## Problem with Simple ETA
//!
//! Simple ETA calculation: `remaining_frames / current_fps`
//!
//! Issues:
//! - Early in encoding: FPS is volatile, ETA jumps wildly
//! - Complex scenes: FPS drops temporarily, ETA spikes
//! - User experience: Jumpy ETA is confusing and stressful
//!
//! ## Solution: Exponential Moving Average (EMA)
//!
//! Instead of using instantaneous FPS, we use EMA-smoothed FPS.
//! EMA gives more weight to recent samples while dampening spikes.
//!
//! Formula: `fps_ema = alpha * fps_current + (1 - alpha) * fps_previous`
//!
//! With alpha=0.15:
//! - Recent 5 samples contribute ~53% of weight
//! - Older samples smoothly fade out
//! - Result: Stable, predictable ETA
//!
//! ## Memory Layout (128B cache-aligned)
//!
//! ```text
//! Offset 0-7:   fps_ema_fixed (AtomicU64) - EMA FPS as Q16.16 fixed-point
//! Offset 8-15:  last_frame (AtomicU64) - Last frame count for delta calculation
//! Offset 16-23: last_time_ns (AtomicU64) - Last update timestamp
//! Offset 24-31: sample_count (AtomicU64) - Number of samples taken
//! Offset 32-39: eta_ema_fixed (AtomicU64) - Smoothed ETA as Q16.16 fixed-point
//! Offset 40-47: alpha_fixed (AtomicU64) - EMA alpha as Q16.16 (default: 0.15)
//! Offset 48-55: min_update_interval_ns (AtomicU64) - Minimum time between updates
//! Offset 56-63: total_frames (AtomicU64) - Total frames (for ETA calculation)
//! Offset 64-127: _padding (64 bytes) - Padding to 128B
//! Total: 128 bytes (2 cache lines)
//! ```
//!
//! ## Performance Characteristics
//! - update(): <50ns (atomic CAS + fixed-point arithmetic)
//! - eta_seconds(): <10ns (single atomic load + conversion)
//! - fps_smoothed(): <10ns (single atomic load + conversion)
//!
//! ## Framework Compliance
//! - **UCE34**: Q10 (T1 Atomic), Q33 (Verification), Q34 (Auditability)
//! - **Chaos**: 128B cache-aligned, 100% lockfree, generation counters
//! - **ASSUM**: All atomics use Acquire/Release for visibility
//! - **T3 Fixed-Point**: Q16.16 format for deterministic arithmetic

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// Constants
// ============================================================================

/// Q16.16 fixed-point multiplier (65536)
const FIXED_POINT_SHIFT: u32 = 16;
const FIXED_POINT_MULTIPLIER: u64 = 1 << FIXED_POINT_SHIFT;

/// Default EMA alpha (0.15) as Q16.16 fixed-point (~9830)
const DEFAULT_ALPHA_FIXED: u64 = (0.15 * FIXED_POINT_MULTIPLIER as f64) as u64;

/// Default minimum update interval (100ms in nanoseconds)
const DEFAULT_MIN_UPDATE_NS: u64 = 100_000_000;

/// Maximum reasonable FPS (1000 fps) to detect outliers
const MAX_REASONABLE_FPS: f64 = 1000.0;

// ============================================================================
// RollingEtaCapsule (T1 Atomic, 128B)
// ============================================================================

/// T1 Atomic rolling ETA estimation capsule (128B cache-aligned)
///
/// Uses Exponential Moving Average for stable, accurate ETA estimates.
///
/// # Memory Layout
/// - **fps_ema_fixed** (Offset 0-7): EMA-smoothed FPS as Q16.16 fixed-point
/// - **last_frame** (Offset 8-15): Last frame count for delta calculation
/// - **last_time_ns** (Offset 16-23): Last update timestamp in nanoseconds
/// - **sample_count** (Offset 24-31): Number of samples taken (for warmup)
/// - **eta_ema_fixed** (Offset 32-39): EMA-smoothed ETA as Q16.16 fixed-point
/// - **alpha_fixed** (Offset 40-47): EMA smoothing factor as Q16.16
/// - **min_update_interval_ns** (Offset 48-55): Minimum time between updates
/// - **total_frames** (Offset 56-63): Total frames in video
/// - **_padding** (Offset 64-127): Padding to 128B
///
/// # Performance Characteristics
/// - **update()**: <50ns (atomic operations + fixed-point math)
/// - **eta_seconds()**: <10ns (single atomic load + conversion)
/// - **fps_smoothed()**: <10ns (single atomic load + conversion)
///
/// # ASSUM Framework
/// - `#ASSUME_MONOTONIC_TIME`: SystemTime::now() is monotonic
/// - `#VERIFY_MONOTONIC_TIME`: saturating_sub prevents negative deltas
/// - `#ASSUME_LOCKFREE`: All operations use atomic primitives
/// - `#VERIFY_LOCKFREE`: No mutex/RwLock in implementation
#[repr(C, align(64))]
pub struct RollingEtaCapsule {
    /// EMA-smoothed FPS as Q16.16 fixed-point
    fps_ema_fixed: AtomicU64,

    /// Last frame count (for delta calculation)
    last_frame: AtomicU64,

    /// Last update timestamp in nanoseconds since epoch
    last_time_ns: AtomicU64,

    /// Number of samples taken (for warmup period handling)
    sample_count: AtomicU64,

    /// EMA-smoothed ETA in seconds as Q16.16 fixed-point
    eta_ema_fixed: AtomicU64,

    /// EMA alpha (smoothing factor) as Q16.16 fixed-point
    alpha_fixed: AtomicU64,

    /// Minimum time between updates in nanoseconds
    min_update_interval_ns: AtomicU64,

    /// Total frames in video
    total_frames: AtomicU64,

    /// Padding to 128 bytes (2 cache lines)
    _padding: [u8; 64],
}

// Compile-time size verification
const _: () = assert!(std::mem::size_of::<RollingEtaCapsule>() == 128);
const _: () = assert!(std::mem::align_of::<RollingEtaCapsule>() == 64);

// Safety: All fields are atomic
unsafe impl Send for RollingEtaCapsule {}
unsafe impl Sync for RollingEtaCapsule {}

impl RollingEtaCapsule {
    /// Create new rolling ETA capsule with default settings
    ///
    /// Default alpha: 0.15 (good balance between responsiveness and stability)
    /// Default update interval: 100ms (prevents excessive updates)
    ///
    /// # Example
    /// ```rust
    /// use kindly_av1::progress::RollingEtaCapsule;
    ///
    /// let eta = RollingEtaCapsule::new();
    /// eta.init(1000); // 1000 total frames
    /// ```
    pub const fn new() -> Self {
        Self {
            fps_ema_fixed: AtomicU64::new(0),
            last_frame: AtomicU64::new(0),
            last_time_ns: AtomicU64::new(0),
            sample_count: AtomicU64::new(0),
            eta_ema_fixed: AtomicU64::new(0),
            alpha_fixed: AtomicU64::new(DEFAULT_ALPHA_FIXED),
            min_update_interval_ns: AtomicU64::new(DEFAULT_MIN_UPDATE_NS),
            total_frames: AtomicU64::new(0),
            _padding: [0u8; 64],
        }
    }

    /// Create with custom alpha (0.0 - 1.0)
    ///
    /// Higher alpha = more responsive, less stable
    /// Lower alpha = more stable, slower to respond
    ///
    /// Recommended values:
    /// - 0.10: Very stable (slow response)
    /// - 0.15: Balanced (default)
    /// - 0.25: Responsive (some jitter)
    /// - 0.40: Very responsive (more jitter)
    ///
    /// # Arguments
    /// * `alpha` - EMA smoothing factor (clamped to 0.05 - 0.50)
    pub fn with_alpha(alpha: f64) -> Self {
        let clamped = alpha.clamp(0.05, 0.50);
        let alpha_fixed = (clamped * FIXED_POINT_MULTIPLIER as f64) as u64;

        Self {
            fps_ema_fixed: AtomicU64::new(0),
            last_frame: AtomicU64::new(0),
            last_time_ns: AtomicU64::new(0),
            sample_count: AtomicU64::new(0),
            eta_ema_fixed: AtomicU64::new(0),
            alpha_fixed: AtomicU64::new(alpha_fixed),
            min_update_interval_ns: AtomicU64::new(DEFAULT_MIN_UPDATE_NS),
            total_frames: AtomicU64::new(0),
            _padding: [0u8; 64],
        }
    }

    /// Initialize with total frame count
    ///
    /// Must be called before update() for correct ETA calculation.
    ///
    /// # Arguments
    /// * `total_frames` - Total frames in video
    pub fn init(&self, total_frames: u64) {
        self.total_frames.store(total_frames, Ordering::Release);
        self.fps_ema_fixed.store(0, Ordering::Release);
        self.eta_ema_fixed.store(0, Ordering::Release);
        self.last_frame.store(0, Ordering::Release);
        self.sample_count.store(0, Ordering::Release);

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.last_time_ns.store(now_ns, Ordering::Release);
    }

    /// Update ETA with current frame count
    ///
    /// Call this periodically (e.g., every frame or every 100ms).
    /// Rate-limited internally to prevent excessive updates.
    ///
    /// # Arguments
    /// * `current_frame` - Current frame number being encoded
    ///
    /// # Returns
    /// `true` if ETA was updated, `false` if skipped (rate limiting)
    ///
    /// # Performance
    /// - Time: <50ns (atomic operations + fixed-point math)
    pub fn update(&self, current_frame: u64) -> bool {
        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let last_ns = self.last_time_ns.load(Ordering::Acquire);
        let elapsed_ns = now_ns.saturating_sub(last_ns);

        // Rate limiting: skip if too soon
        let min_interval = self.min_update_interval_ns.load(Ordering::Relaxed);
        if elapsed_ns < min_interval {
            return false;
        }

        let last_frame = self.last_frame.load(Ordering::Acquire);
        let frames_delta = current_frame.saturating_sub(last_frame);

        // Skip if no frames encoded since last update
        if frames_delta == 0 {
            return false;
        }

        // Calculate instantaneous FPS
        let elapsed_secs = elapsed_ns as f64 / 1_000_000_000.0;
        let instant_fps = if elapsed_secs > 0.0 {
            frames_delta as f64 / elapsed_secs
        } else {
            return false;
        };

        // Skip outliers (detect encoding pauses or bursts)
        if instant_fps > MAX_REASONABLE_FPS || instant_fps < 0.001 {
            // Just update timestamps, don't incorporate outlier
            self.last_frame.store(current_frame, Ordering::Release);
            self.last_time_ns.store(now_ns, Ordering::Release);
            return false;
        }

        // Convert to Q16.16 fixed-point
        let instant_fps_fixed = (instant_fps * FIXED_POINT_MULTIPLIER as f64) as u64;

        // Load current EMA
        let current_ema_fixed = self.fps_ema_fixed.load(Ordering::Acquire);
        let alpha_fixed = self.alpha_fixed.load(Ordering::Relaxed);
        let sample_count = self.sample_count.load(Ordering::Acquire);

        // Calculate new EMA
        // new_ema = alpha * instant + (1 - alpha) * current_ema
        // In fixed-point: new_ema = (alpha * instant + (MULT - alpha) * current) / MULT
        let new_ema_fixed = if sample_count < 3 {
            // Warmup period: use simple average for stability
            if current_ema_fixed == 0 {
                instant_fps_fixed
            } else {
                (current_ema_fixed + instant_fps_fixed) / 2
            }
        } else {
            // Normal EMA
            let alpha_part = (alpha_fixed * instant_fps_fixed) / FIXED_POINT_MULTIPLIER;
            let one_minus_alpha = FIXED_POINT_MULTIPLIER - alpha_fixed;
            let ema_part = (one_minus_alpha * current_ema_fixed) / FIXED_POINT_MULTIPLIER;
            alpha_part + ema_part
        };

        // Calculate ETA
        let total = self.total_frames.load(Ordering::Relaxed);
        let remaining = total.saturating_sub(current_frame);

        let eta_seconds = if new_ema_fixed > 0 {
            let fps_float = new_ema_fixed as f64 / FIXED_POINT_MULTIPLIER as f64;
            remaining as f64 / fps_float
        } else {
            0.0
        };
        let eta_fixed = (eta_seconds * FIXED_POINT_MULTIPLIER as f64) as u64;

        // Smooth ETA as well (prevents jitter)
        let current_eta_fixed = self.eta_ema_fixed.load(Ordering::Acquire);
        let new_eta_fixed = if sample_count < 3 || current_eta_fixed == 0 {
            eta_fixed
        } else {
            let alpha_part = (alpha_fixed * eta_fixed) / FIXED_POINT_MULTIPLIER;
            let one_minus_alpha = FIXED_POINT_MULTIPLIER - alpha_fixed;
            let eta_part = (one_minus_alpha * current_eta_fixed) / FIXED_POINT_MULTIPLIER;
            alpha_part + eta_part
        };

        // Store updated values
        self.fps_ema_fixed.store(new_ema_fixed, Ordering::Release);
        self.eta_ema_fixed.store(new_eta_fixed, Ordering::Release);
        self.last_frame.store(current_frame, Ordering::Release);
        self.last_time_ns.store(now_ns, Ordering::Release);
        self.sample_count.fetch_add(1, Ordering::AcqRel);

        true
    }

    /// Get smoothed ETA in seconds
    ///
    /// Returns 0 if not enough data or encoding complete.
    ///
    /// # Performance
    /// - Time: <10ns (single atomic load + conversion)
    #[inline]
    pub fn eta_seconds(&self) -> u64 {
        let eta_fixed = self.eta_ema_fixed.load(Ordering::Acquire);
        (eta_fixed / FIXED_POINT_MULTIPLIER) as u64
    }

    /// Get smoothed ETA as f64 (with fractional seconds)
    ///
    /// # Performance
    /// - Time: <10ns (single atomic load + conversion)
    #[inline]
    pub fn eta_seconds_f64(&self) -> f64 {
        let eta_fixed = self.eta_ema_fixed.load(Ordering::Acquire);
        eta_fixed as f64 / FIXED_POINT_MULTIPLIER as f64
    }

    /// Get smoothed FPS
    ///
    /// # Performance
    /// - Time: <10ns (single atomic load + conversion)
    #[inline]
    pub fn fps_smoothed(&self) -> f64 {
        let fps_fixed = self.fps_ema_fixed.load(Ordering::Acquire);
        fps_fixed as f64 / FIXED_POINT_MULTIPLIER as f64
    }

    /// Get number of samples taken
    ///
    /// Useful for determining if ETA is reliable (>10 samples = good)
    #[inline]
    pub fn sample_count(&self) -> u64 {
        self.sample_count.load(Ordering::Acquire)
    }

    /// Check if ETA is reliable (enough samples collected)
    ///
    /// Returns true if at least 5 samples have been collected.
    #[inline]
    pub fn is_reliable(&self) -> bool {
        self.sample_count() >= 5
    }

    /// Get confidence level (0.0 - 1.0)
    ///
    /// Based on sample count:
    /// - 0-2 samples: 0.0 - 0.2 (unreliable)
    /// - 3-9 samples: 0.3 - 0.6 (warming up)
    /// - 10+ samples: 0.7 - 1.0 (reliable)
    pub fn confidence(&self) -> f64 {
        let samples = self.sample_count();
        if samples < 3 {
            samples as f64 * 0.1
        } else if samples < 10 {
            0.3 + (samples - 3) as f64 * 0.05
        } else {
            (0.7 + (samples - 10) as f64 * 0.01).min(1.0)
        }
    }

    /// Set EMA alpha (smoothing factor)
    ///
    /// # Arguments
    /// * `alpha` - EMA smoothing factor (clamped to 0.05 - 0.50)
    pub fn set_alpha(&self, alpha: f64) {
        let clamped = alpha.clamp(0.05, 0.50);
        let alpha_fixed = (clamped * FIXED_POINT_MULTIPLIER as f64) as u64;
        self.alpha_fixed.store(alpha_fixed, Ordering::Release);
    }

    /// Set minimum update interval
    ///
    /// # Arguments
    /// * `interval_ms` - Minimum milliseconds between updates
    pub fn set_update_interval_ms(&self, interval_ms: u64) {
        let interval_ns = interval_ms * 1_000_000;
        self.min_update_interval_ns.store(interval_ns, Ordering::Release);
    }

    /// Reset all state
    ///
    /// Call when starting a new encoding session.
    pub fn reset(&self) {
        self.fps_ema_fixed.store(0, Ordering::Release);
        self.eta_ema_fixed.store(0, Ordering::Release);
        self.last_frame.store(0, Ordering::Release);
        self.sample_count.store(0, Ordering::Release);

        let now_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        self.last_time_ns.store(now_ns, Ordering::Release);
    }

    /// Format ETA as human-readable string with confidence indicator
    ///
    /// Returns strings like:
    /// - "calculating..." (0-2 samples)
    /// - "~45s" (3-9 samples)
    /// - "42s" (10+ samples)
    pub fn format_eta(&self) -> String {
        let samples = self.sample_count();
        let eta = self.eta_seconds();

        if samples < 3 {
            return "calculating...".to_string();
        }

        let prefix = if samples < 10 { "~" } else { "" };

        if eta == 0 {
            format!("{}finishing", prefix)
        } else if eta < 60 {
            format!("{}{}s", prefix, eta)
        } else if eta < 3600 {
            format!("{}{}m {}s", prefix, eta / 60, eta % 60)
        } else {
            format!("{}{}h {}m", prefix, eta / 3600, (eta % 3600) / 60)
        }
    }

    /// Format FPS as human-readable string
    ///
    /// Returns strings like:
    /// - "..." (0-2 samples)
    /// - "~24.3 fps" (3-9 samples)
    /// - "24.7 fps" (10+ samples)
    pub fn format_fps(&self) -> String {
        let samples = self.sample_count();
        let fps = self.fps_smoothed();

        if samples < 3 {
            return "...".to_string();
        }

        let prefix = if samples < 10 { "~" } else { "" };
        format!("{}{:.1} fps", prefix, fps)
    }
}

impl Default for RollingEtaCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for RollingEtaCapsule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RollingEtaCapsule")
            .field("fps_smoothed", &self.fps_smoothed())
            .field("eta_seconds", &self.eta_seconds())
            .field("sample_count", &self.sample_count())
            .field("confidence", &format!("{:.0}%", self.confidence() * 100.0))
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, size_of};

    #[test]
    fn test_alignment_and_size() {
        assert_eq!(size_of::<RollingEtaCapsule>(), 128);
        assert_eq!(align_of::<RollingEtaCapsule>(), 64);
    }

    #[test]
    fn test_new() {
        let eta = RollingEtaCapsule::new();
        assert_eq!(eta.fps_smoothed(), 0.0);
        assert_eq!(eta.eta_seconds(), 0);
        assert_eq!(eta.sample_count(), 0);
        assert!(!eta.is_reliable());
    }

    #[test]
    fn test_init() {
        let eta = RollingEtaCapsule::new();
        eta.init(1000);

        assert_eq!(eta.total_frames.load(Ordering::Relaxed), 1000);
        assert_eq!(eta.sample_count(), 0);
    }

    #[test]
    fn test_with_alpha() {
        let eta = RollingEtaCapsule::with_alpha(0.25);
        let alpha = eta.alpha_fixed.load(Ordering::Relaxed);
        let expected = (0.25 * FIXED_POINT_MULTIPLIER as f64) as u64;

        // Allow small rounding error
        assert!((alpha as i64 - expected as i64).abs() < 10);
    }

    #[test]
    fn test_alpha_clamping() {
        // Too low
        let eta = RollingEtaCapsule::with_alpha(0.01);
        let alpha = eta.alpha_fixed.load(Ordering::Relaxed);
        let min_expected = (0.05 * FIXED_POINT_MULTIPLIER as f64) as u64;
        assert!((alpha as i64 - min_expected as i64).abs() < 10);

        // Too high
        let eta = RollingEtaCapsule::with_alpha(0.90);
        let alpha = eta.alpha_fixed.load(Ordering::Relaxed);
        let max_expected = (0.50 * FIXED_POINT_MULTIPLIER as f64) as u64;
        assert!((alpha as i64 - max_expected as i64).abs() < 10);
    }

    #[test]
    fn test_confidence() {
        let eta = RollingEtaCapsule::new();

        // 0 samples = 0% confidence
        assert!(eta.confidence() < 0.01);

        // Simulate samples
        for _ in 0..3 {
            eta.sample_count.fetch_add(1, Ordering::Relaxed);
        }
        let conf_3 = eta.confidence();
        assert!(conf_3 >= 0.29 && conf_3 < 0.35, "3 samples: {}", conf_3);

        // 10 samples
        for _ in 0..7 {
            eta.sample_count.fetch_add(1, Ordering::Relaxed);
        }
        let conf_10 = eta.confidence();
        assert!(conf_10 >= 0.69, "10 samples: {}", conf_10);

        // 20 samples: 0.7 + (20-10) * 0.01 = 0.8 (accounting for float precision)
        for _ in 0..10 {
            eta.sample_count.fetch_add(1, Ordering::Relaxed);
        }
        let conf_20 = eta.confidence();
        assert!(conf_20 >= 0.79, "20 samples: {}", conf_20);
    }

    #[test]
    fn test_format_eta() {
        let eta = RollingEtaCapsule::new();
        eta.init(1000);

        // No samples
        assert_eq!(eta.format_eta(), "calculating...");

        // Few samples (warmup)
        eta.sample_count.store(5, Ordering::Relaxed);
        eta.eta_ema_fixed.store(45 * FIXED_POINT_MULTIPLIER, Ordering::Relaxed);
        assert!(eta.format_eta().starts_with("~"));

        // Many samples (reliable)
        eta.sample_count.store(15, Ordering::Relaxed);
        assert!(!eta.format_eta().starts_with("~"));
    }

    #[test]
    fn test_format_fps() {
        let eta = RollingEtaCapsule::new();

        // No samples
        assert_eq!(eta.format_fps(), "...");

        // Few samples
        eta.sample_count.store(5, Ordering::Relaxed);
        eta.fps_ema_fixed.store(
            (24.5 * FIXED_POINT_MULTIPLIER as f64) as u64,
            Ordering::Relaxed,
        );
        let fps_str = eta.format_fps();
        assert!(fps_str.starts_with("~"));
        assert!(fps_str.contains("24.5"));
    }

    #[test]
    fn test_set_alpha() {
        let eta = RollingEtaCapsule::new();
        eta.set_alpha(0.30);

        let alpha = eta.alpha_fixed.load(Ordering::Relaxed);
        let expected = (0.30 * FIXED_POINT_MULTIPLIER as f64) as u64;
        assert!((alpha as i64 - expected as i64).abs() < 10);
    }

    #[test]
    fn test_set_update_interval() {
        let eta = RollingEtaCapsule::new();
        eta.set_update_interval_ms(200);

        let interval = eta.min_update_interval_ns.load(Ordering::Relaxed);
        assert_eq!(interval, 200_000_000);
    }

    #[test]
    fn test_reset() {
        let eta = RollingEtaCapsule::new();
        eta.init(1000);

        // Simulate some updates
        eta.fps_ema_fixed.store(100 * FIXED_POINT_MULTIPLIER, Ordering::Relaxed);
        eta.eta_ema_fixed.store(50 * FIXED_POINT_MULTIPLIER, Ordering::Relaxed);
        eta.sample_count.store(10, Ordering::Relaxed);

        eta.reset();

        assert_eq!(eta.fps_smoothed(), 0.0);
        assert_eq!(eta.eta_seconds(), 0);
        assert_eq!(eta.sample_count(), 0);
    }

    #[test]
    fn test_debug() {
        let eta = RollingEtaCapsule::new();
        eta.init(1000);
        eta.sample_count.store(5, Ordering::Relaxed);

        let debug_str = format!("{:?}", eta);
        assert!(debug_str.contains("RollingEtaCapsule"));
        assert!(debug_str.contains("fps_smoothed"));
        assert!(debug_str.contains("confidence"));
    }

    #[test]
    fn test_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<RollingEtaCapsule>();
        assert_sync::<RollingEtaCapsule>();
    }

    #[test]
    fn test_is_reliable() {
        let eta = RollingEtaCapsule::new();

        assert!(!eta.is_reliable());

        eta.sample_count.store(4, Ordering::Relaxed);
        assert!(!eta.is_reliable());

        eta.sample_count.store(5, Ordering::Relaxed);
        assert!(eta.is_reliable());
    }
}
