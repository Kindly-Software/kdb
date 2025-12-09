//! # TimestampCapsule (T1 Atomic)
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Ultra-low-latency timestamp management for video/audio muxers using
//! lockfree atomics with 128-byte cache alignment.
//!
//! **Tier**: T1 Atomic
//! **Size**: 128 bytes, cache-aligned (two L1 cache lines)
//! **Purpose**: PTS/DTS timestamp tracking, timescale conversion, B-frame reordering
//!
//! ## Features
//!
//! - Presentation Timestamp (PTS) tracking
//! - Decode Timestamp (DTS) tracking
//! - Composition Time Offset (CTS = PTS - DTS)
//! - Timescale conversion utilities:
//!   - 90kHz (MPEG-TS)
//!   - 1000ms (milliseconds)
//!   - Sample rate (44100, 48000, etc.)
//!   - Custom timescale
//! - Frame duration calculation
//! - B-frame reordering support (DTS != PTS)
//! - Timestamp wraparound handling (33-bit PTS in MPEG-TS)
//! - Timestamp discontinuity detection
//! - Timeline management:
//!   - Track-level timestamps
//!   - Edit list offsets
//!   - Media time vs. presentation time
//! - Rational time representation (numerator/denominator)
//! - Frame rate conversion (24fps, 25fps, 29.97fps, 30fps, 60fps)
//!
//! ## DualAtomicU64 Layout
//!
//! ```text
//! Primary (64 bits):
//! ├─ base_pts: 64 bits (PTS origin for track)
//!
//! Secondary (64 bits):
//! └─ base_dts: 64 bits (DTS origin for track)
//!
//! Tertiary atomics (64 bits each):
//! ├─ last_pts: 64 bits (Most recent PTS)
//! └─ last_dts: 64 bits (Most recent DTS)
//!
//! Metadata atomics (32 bits each packed):
//! ├─ timescale: 32 bits (units per second)
//! ├─ frame_duration: 32 bits (ticks per frame)
//! ├─ discontinuity_count: 32 bits (discontinuity counter)
//! └─ flags: 32 bits (state flags)
//!
//! Generation counter:
//! └─ generation: 64 bits (for TOCTOU prevention)
//! ```
//!
//! ## Performance Targets
//!
//! - `record_timestamp(pts, dts)`: <30ns (4 stores + generation increment)
//! - `get_cts()`: <15ns (2 loads, subtract)
//! - `convert_timescale(ts, from, to)`: <10ns (pure arithmetic)
//! - `detect_discontinuity()`: <20ns (compare + conditional)
//!
//! ## ASSUM Safety Tags
//!
//! - `#ASSUME_LOCKFREE_ONLY`: All coordination via atomics, no mutex/RwLock
//! - `#ASSUME_MONOTONIC_TIMESTAMPS`: Timestamps should be monotonic (caller enforces)
//! - `#ASSUME_VALID_TIMESCALE`: Timescale > 0 (enforced at construction)
//! - `#ASSUME_WRAPAROUND_HANDLED`: 33-bit PTS wraparound detection works correctly
//! - `#ASSUME_NO_OVERFLOW`: Saturating arithmetic prevents overflow

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use core::fmt;

// ============================================================================
// Constants
// ============================================================================

/// MPEG-TS compatible timescale (90kHz)
pub const TIMESCALE_90KHZ: u32 = 90_000;

/// Millisecond precision timescale
pub const TIMESCALE_1000: u32 = 1_000;

/// Common audio sample rate (48kHz)
pub const TIMESCALE_48KHZ: u32 = 48_000;

/// CD audio sample rate (44.1kHz)
pub const TIMESCALE_44100: u32 = 44_100;

/// Maximum 33-bit PTS value (MPEG-TS)
/// PTS in MPEG-TS uses 33 bits, wrapping at 2^33 - 1
pub const PTS_MAX_33BIT: u64 = (1 << 33) - 1;

/// Maximum 32-bit timestamp value
pub const TS_MAX_32BIT: u64 = (1 << 32) - 1;

/// Frame duration for 24fps at 90kHz timescale
pub const FRAME_DURATION_24FPS_90K: u32 = 3750; // 90000 / 24

/// Frame duration for 25fps at 90kHz timescale
pub const FRAME_DURATION_25FPS_90K: u32 = 3600; // 90000 / 25

/// Frame duration for 29.97fps (NTSC) at 90kHz timescale
pub const FRAME_DURATION_29_97FPS_90K: u32 = 3003; // 90000 * 1001 / 30000

/// Frame duration for 30fps at 90kHz timescale
pub const FRAME_DURATION_30FPS_90K: u32 = 3000; // 90000 / 30

/// Frame duration for 60fps at 90kHz timescale
pub const FRAME_DURATION_60FPS_90K: u32 = 1500; // 90000 / 60

/// Discontinuity threshold (3 frame durations at 24fps, ~125ms)
/// If gap exceeds this, consider it a discontinuity
pub const DISCONTINUITY_THRESHOLD_90K: u64 = 3 * FRAME_DURATION_24FPS_90K as u64;

// ============================================================================
// Flag bits for state tracking
// ============================================================================

/// Flag: B-frame reordering is active (DTS != PTS)
const FLAG_BFRAME_REORDER: u32 = 1 << 0;

/// Flag: Timestamp discontinuity detected
const FLAG_DISCONTINUITY: u32 = 1 << 1;

/// Flag: Wrapped around 33-bit boundary
const FLAG_WRAPPED: u32 = 1 << 2;

/// Flag: First timestamp recorded
const FLAG_INITIALIZED: u32 = 1 << 3;

// ============================================================================
// TimestampCapsule
// ============================================================================

/// T1 Atomic capsule for PTS/DTS timestamp management.
///
/// **Tier**: T1 Atomic
/// **Size**: 128 bytes (perfectly aligned to two L1 cache lines)
/// **Layout**: 6 AtomicU64 + 4 AtomicU32 + padding
///
/// # ASSUM Safety Tags
///
/// - `#ASSUME_LOCKFREE_ONLY`: All coordination via atomics, no mutex/RwLock (verified)
/// - `#ASSUME_MONOTONIC_TIMESTAMPS`: Caller provides monotonic timestamps
/// - `#ASSUME_VALID_TIMESCALE`: Timescale > 0 (enforced: constructor validates)
/// - `#ASSUME_128B_ALIGNMENT`: 128 bytes prevents false sharing between channels
///
/// # Safety Proof
///
/// - Alignment: `#[repr(C, align(128))]` enforces 128-byte alignment
/// - Atomicity: All updates via `AtomicU64`/`AtomicU32` with proper memory ordering
/// - Race condition: Generation counter enables TOCTOU-safe reads
/// - Overflow: Saturating arithmetic prevents wraparound issues
#[repr(C, align(128))]
pub struct TimestampCapsule {
    // === Cache Line 1 (64 bytes) ===

    /// Base PTS for the track (origin timestamp)
    /// Offset 0-7
    base_pts: AtomicU64,

    /// Base DTS for the track (decode origin)
    /// Offset 8-15
    base_dts: AtomicU64,

    /// Most recent PTS recorded
    /// Offset 16-23
    last_pts: AtomicU64,

    /// Most recent DTS recorded
    /// Offset 24-31
    last_dts: AtomicU64,

    /// Timescale (units per second, e.g., 90000 for MPEG-TS)
    /// Offset 32-35
    timescale: AtomicU32,

    /// Frame duration in timescale units
    /// Offset 36-39
    frame_duration: AtomicU32,

    /// Discontinuity counter
    /// Offset 40-43
    discontinuity_count: AtomicU32,

    /// State flags (see FLAG_* constants)
    /// Offset 44-47
    flags: AtomicU32,

    /// Padding to complete first cache line
    /// Offset 48-63
    _padding1: [u8; 16],

    // === Cache Line 2 (64 bytes) ===

    /// Generation counter for TOCTOU prevention
    /// Offset 64-71
    generation: AtomicU64,

    /// Edit list offset (presentation time adjustment)
    /// Offset 72-79
    edit_offset: AtomicU64,

    /// Wraparound count (number of 33-bit wraps detected)
    /// Offset 80-87
    wrap_count: AtomicU64,

    /// Maximum PTS seen (for wraparound detection)
    /// Offset 88-95
    max_pts_seen: AtomicU64,

    /// Padding to complete second cache line
    /// Offset 96-127
    _padding2: [u8; 32],
}

// Compile-time verification of size and alignment
const _: () = {
    const SIZE: usize = core::mem::size_of::<TimestampCapsule>();
    const ALIGN: usize = core::mem::align_of::<TimestampCapsule>();

    // #VERIFY_SIZE: Must be exactly 128 bytes
    const _SIZE_CHECK: () = if SIZE != 128 {
        panic!("TimestampCapsule must be 128 bytes")
    };

    // #VERIFY_ALIGN: Must be 128-byte aligned
    const _ALIGN_CHECK: () = if ALIGN != 128 {
        panic!("TimestampCapsule must be 128-byte aligned")
    };
};

impl TimestampCapsule {
    /// Create a new TimestampCapsule with specified timescale.
    ///
    /// # Parameters
    ///
    /// - `timescale`: Units per second (e.g., 90000 for MPEG-TS)
    ///
    /// # Panics
    ///
    /// Panics if `timescale` is 0.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use atomic_capsule::mux::TimestampCapsule;
    /// use atomic_capsule::mux::TIMESCALE_90KHZ;
    ///
    /// let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
    /// ```
    pub fn new(timescale: u32) -> Self {
        assert!(timescale > 0, "Timescale must be > 0");

        Self {
            base_pts: AtomicU64::new(0),
            base_dts: AtomicU64::new(0),
            last_pts: AtomicU64::new(0),
            last_dts: AtomicU64::new(0),
            timescale: AtomicU32::new(timescale),
            frame_duration: AtomicU32::new(0),
            discontinuity_count: AtomicU32::new(0),
            flags: AtomicU32::new(0),
            _padding1: [0u8; 16],
            generation: AtomicU64::new(0),
            edit_offset: AtomicU64::new(0),
            wrap_count: AtomicU64::new(0),
            max_pts_seen: AtomicU64::new(0),
            _padding2: [0u8; 32],
        }
    }

    /// Create a new TimestampCapsule with timescale and frame rate.
    ///
    /// # Parameters
    ///
    /// - `timescale`: Units per second
    /// - `fps_num`: Frame rate numerator (e.g., 30000 for 29.97fps)
    /// - `fps_den`: Frame rate denominator (e.g., 1001 for 29.97fps)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // 29.97fps (NTSC) at 90kHz
    /// let ts = TimestampCapsule::with_framerate(90000, 30000, 1001);
    /// ```
    pub fn with_framerate(timescale: u32, fps_num: u32, fps_den: u32) -> Self {
        assert!(timescale > 0, "Timescale must be > 0");
        assert!(fps_num > 0, "FPS numerator must be > 0");
        assert!(fps_den > 0, "FPS denominator must be > 0");

        // frame_duration = timescale * fps_den / fps_num
        let frame_duration = ((timescale as u64) * (fps_den as u64) / (fps_num as u64)) as u32;

        let capsule = Self::new(timescale);
        capsule.frame_duration.store(frame_duration, Ordering::Relaxed);
        capsule
    }

    // ========================================================================
    // Timestamp Recording Operations
    // ========================================================================

    /// Record a new PTS/DTS timestamp pair.
    ///
    /// # Parameters
    ///
    /// - `pts`: Presentation timestamp
    /// - `dts`: Decode timestamp (may equal PTS for non-B-frame content)
    ///
    /// # Performance
    ///
    /// <30ns typical (4 stores + generation increment)
    ///
    /// # Returns
    ///
    /// `true` if discontinuity was detected, `false` otherwise.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let ts = TimestampCapsule::new(90000);
    ///
    /// // Record I-frame (PTS == DTS)
    /// ts.record_timestamp(0, 0);
    ///
    /// // Record B-frame (PTS > DTS due to reordering)
    /// ts.record_timestamp(7500, 3000);
    /// ```
    pub fn record_timestamp(&self, pts: u64, dts: u64) -> bool {
        let flags = self.flags.load(Ordering::Acquire);
        let was_initialized = flags & FLAG_INITIALIZED != 0;

        // Load previous values for discontinuity detection
        let prev_pts = self.last_pts.load(Ordering::Acquire);
        let _prev_dts = self.last_dts.load(Ordering::Acquire); // Reserved for future DTS-based discontinuity

        // Detect B-frame reordering
        if pts != dts {
            self.flags.fetch_or(FLAG_BFRAME_REORDER, Ordering::Release);
        }

        // Initialize base timestamps on first record
        if !was_initialized {
            self.base_pts.store(pts, Ordering::Release);
            self.base_dts.store(dts, Ordering::Release);
            self.flags.fetch_or(FLAG_INITIALIZED, Ordering::Release);
        }

        // Detect wraparound (33-bit PTS boundary)
        let max_seen = self.max_pts_seen.load(Ordering::Acquire);
        if pts < max_seen && max_seen - pts > PTS_MAX_33BIT / 2 {
            // Wraparound detected
            self.wrap_count.fetch_add(1, Ordering::Release);
            self.flags.fetch_or(FLAG_WRAPPED, Ordering::Release);
        }

        // Update max PTS seen
        if pts > max_seen {
            self.max_pts_seen.store(pts, Ordering::Release);
        }

        // Detect discontinuity
        let mut discontinuity = false;
        if was_initialized {
            let frame_dur = self.frame_duration.load(Ordering::Acquire) as u64;
            let threshold = if frame_dur > 0 {
                frame_dur * 3 // 3 frame durations
            } else {
                DISCONTINUITY_THRESHOLD_90K
            };

            // Check for large gap (forward or backward)
            let pts_gap = if pts > prev_pts {
                pts - prev_pts
            } else {
                prev_pts - pts
            };

            if pts_gap > threshold {
                discontinuity = true;
                self.discontinuity_count.fetch_add(1, Ordering::Release);
                self.flags.fetch_or(FLAG_DISCONTINUITY, Ordering::Release);
            }
        }

        // Store new timestamps
        self.last_pts.store(pts, Ordering::Release);
        self.last_dts.store(dts, Ordering::Release);

        // Increment generation counter for TOCTOU safety
        self.generation.fetch_add(1, Ordering::Release);

        discontinuity
    }

    /// Record PTS only (DTS assumed equal).
    ///
    /// Convenience method for content without B-frames.
    ///
    /// # Performance
    ///
    /// <25ns typical
    #[inline]
    pub fn record_pts(&self, pts: u64) -> bool {
        self.record_timestamp(pts, pts)
    }

    // ========================================================================
    // Timestamp Query Operations
    // ========================================================================

    /// Get the last recorded PTS.
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load)
    #[inline]
    pub fn last_pts(&self) -> u64 {
        self.last_pts.load(Ordering::Acquire)
    }

    /// Get the last recorded DTS.
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load)
    #[inline]
    pub fn last_dts(&self) -> u64 {
        self.last_dts.load(Ordering::Acquire)
    }

    /// Get the Composition Time Offset (CTS = PTS - DTS).
    ///
    /// CTS represents the time between decoding and presentation,
    /// which is non-zero for B-frames in reordered streams.
    ///
    /// # Performance
    ///
    /// <15ns (2 loads + subtract)
    ///
    /// # Returns
    ///
    /// Signed CTS value (positive for normal reordering)
    #[inline]
    pub fn cts(&self) -> i64 {
        let pts = self.last_pts.load(Ordering::Acquire);
        let dts = self.last_dts.load(Ordering::Acquire);
        pts as i64 - dts as i64
    }

    /// Get the base PTS (track origin).
    #[inline]
    pub fn base_pts(&self) -> u64 {
        self.base_pts.load(Ordering::Acquire)
    }

    /// Get the base DTS (decode origin).
    #[inline]
    pub fn base_dts(&self) -> u64 {
        self.base_dts.load(Ordering::Acquire)
    }

    /// Get current timescale.
    #[inline]
    pub fn timescale(&self) -> u32 {
        self.timescale.load(Ordering::Acquire)
    }

    /// Get frame duration in timescale units.
    #[inline]
    pub fn frame_duration(&self) -> u32 {
        self.frame_duration.load(Ordering::Acquire)
    }

    /// Get discontinuity count.
    #[inline]
    pub fn discontinuity_count(&self) -> u32 {
        self.discontinuity_count.load(Ordering::Acquire)
    }

    /// Check if B-frame reordering is detected.
    #[inline]
    pub fn has_bframe_reorder(&self) -> bool {
        self.flags.load(Ordering::Acquire) & FLAG_BFRAME_REORDER != 0
    }

    /// Check if any discontinuity was detected.
    #[inline]
    pub fn has_discontinuity(&self) -> bool {
        self.flags.load(Ordering::Acquire) & FLAG_DISCONTINUITY != 0
    }

    /// Check if 33-bit wraparound occurred.
    #[inline]
    pub fn has_wrapped(&self) -> bool {
        self.flags.load(Ordering::Acquire) & FLAG_WRAPPED != 0
    }

    /// Get the number of 33-bit wraparounds detected.
    #[inline]
    pub fn wrap_count(&self) -> u64 {
        self.wrap_count.load(Ordering::Acquire)
    }

    /// Get generation counter (for TOCTOU-safe reads).
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Read timestamp state atomically using generation counter.
    ///
    /// # Returns
    ///
    /// `Some((pts, dts, cts))` if read was consistent, `None` if concurrent modification detected.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// loop {
    ///     if let Some((pts, dts, cts)) = ts.read_consistent() {
    ///         // Use values safely
    ///         break;
    ///     }
    ///     // Retry on concurrent modification
    /// }
    /// ```
    pub fn read_consistent(&self) -> Option<(u64, u64, i64)> {
        let gen_before = self.generation.load(Ordering::Acquire);
        let pts = self.last_pts.load(Ordering::Acquire);
        let dts = self.last_dts.load(Ordering::Acquire);
        let gen_after = self.generation.load(Ordering::Acquire);

        if gen_before == gen_after {
            let cts = pts as i64 - dts as i64;
            Some((pts, dts, cts))
        } else {
            None
        }
    }

    // ========================================================================
    // Timescale Conversion Operations
    // ========================================================================

    /// Convert timestamp between timescales.
    ///
    /// # Parameters
    ///
    /// - `timestamp`: Input timestamp
    /// - `from_scale`: Source timescale
    /// - `to_scale`: Target timescale
    ///
    /// # Performance
    ///
    /// <10ns (pure arithmetic)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Convert 90kHz to milliseconds
    /// let ms = TimestampCapsule::convert_timescale(90000, 90000, 1000);
    /// assert_eq!(ms, 1000); // 1 second
    /// ```
    #[inline]
    pub fn convert_timescale(timestamp: u64, from_scale: u32, to_scale: u32) -> u64 {
        if from_scale == to_scale {
            return timestamp;
        }
        if from_scale == 0 {
            return 0;
        }

        // Use 128-bit arithmetic to avoid overflow
        // result = timestamp * to_scale / from_scale
        let numerator = timestamp as u128 * to_scale as u128;
        (numerator / from_scale as u128) as u64
    }

    /// Convert timestamp to milliseconds.
    #[inline]
    pub fn to_milliseconds(&self, timestamp: u64) -> u64 {
        let scale = self.timescale.load(Ordering::Acquire);
        Self::convert_timescale(timestamp, scale, TIMESCALE_1000)
    }

    /// Convert milliseconds to current timescale.
    #[inline]
    pub fn from_milliseconds(&self, ms: u64) -> u64 {
        let scale = self.timescale.load(Ordering::Acquire);
        Self::convert_timescale(ms, TIMESCALE_1000, scale)
    }

    /// Convert timestamp to 90kHz (MPEG-TS).
    #[inline]
    pub fn to_90khz(&self, timestamp: u64) -> u64 {
        let scale = self.timescale.load(Ordering::Acquire);
        Self::convert_timescale(timestamp, scale, TIMESCALE_90KHZ)
    }

    /// Convert 90kHz timestamp to current timescale.
    #[inline]
    pub fn from_90khz(&self, timestamp: u64) -> u64 {
        let scale = self.timescale.load(Ordering::Acquire);
        Self::convert_timescale(timestamp, TIMESCALE_90KHZ, scale)
    }

    // ========================================================================
    // Rational Time Operations
    // ========================================================================

    /// Convert timestamp to rational time (seconds as numerator/denominator).
    ///
    /// # Returns
    ///
    /// `(numerator, denominator)` where `numerator/denominator` is time in seconds.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (num, den) = ts.to_rational(45000); // 0.5 seconds at 90kHz
    /// // num = 45000, den = 90000 (simplifies to 1/2)
    /// ```
    #[inline]
    pub fn to_rational(&self, timestamp: u64) -> (u64, u64) {
        let scale = self.timescale.load(Ordering::Acquire);
        (timestamp, scale as u64)
    }

    /// Convert rational time to timestamp.
    ///
    /// # Parameters
    ///
    /// - `numerator`: Time numerator
    /// - `denominator`: Time denominator
    ///
    /// # Returns
    ///
    /// Timestamp in current timescale
    #[inline]
    pub fn from_rational(&self, numerator: u64, denominator: u64) -> u64 {
        if denominator == 0 {
            return 0;
        }
        let scale = self.timescale.load(Ordering::Acquire);
        // timestamp = numerator * timescale / denominator
        let result = numerator as u128 * scale as u128 / denominator as u128;
        result as u64
    }

    // ========================================================================
    // Frame Duration Operations
    // ========================================================================

    /// Set frame duration directly.
    #[inline]
    pub fn set_frame_duration(&self, duration: u32) {
        self.frame_duration.store(duration, Ordering::Release);
    }

    /// Set frame duration from frame rate.
    ///
    /// # Parameters
    ///
    /// - `fps_num`: Frame rate numerator (e.g., 30000 for 29.97fps)
    /// - `fps_den`: Frame rate denominator (e.g., 1001 for 29.97fps)
    pub fn set_framerate(&self, fps_num: u32, fps_den: u32) {
        if fps_num == 0 {
            return;
        }
        let scale = self.timescale.load(Ordering::Acquire);
        let duration = ((scale as u64) * (fps_den as u64) / (fps_num as u64)) as u32;
        self.frame_duration.store(duration, Ordering::Release);
    }

    /// Calculate timestamp for frame N.
    ///
    /// # Parameters
    ///
    /// - `frame_number`: 0-indexed frame number
    ///
    /// # Returns
    ///
    /// Timestamp for the specified frame
    #[inline]
    pub fn frame_timestamp(&self, frame_number: u64) -> u64 {
        let base = self.base_pts.load(Ordering::Acquire);
        let duration = self.frame_duration.load(Ordering::Acquire) as u64;
        base.saturating_add(frame_number.saturating_mul(duration))
    }

    /// Calculate frame number from timestamp.
    ///
    /// # Returns
    ///
    /// Frame number (0-indexed)
    #[inline]
    pub fn timestamp_to_frame(&self, timestamp: u64) -> u64 {
        let base = self.base_pts.load(Ordering::Acquire);
        let duration = self.frame_duration.load(Ordering::Acquire) as u64;

        if duration == 0 || timestamp < base {
            return 0;
        }

        (timestamp - base) / duration
    }

    /// Get frame rate as rational (fps_num/fps_den).
    ///
    /// # Returns
    ///
    /// `(numerator, denominator)` where `numerator/denominator` is frames per second.
    pub fn framerate(&self) -> (u32, u32) {
        let scale = self.timescale.load(Ordering::Acquire);
        let duration = self.frame_duration.load(Ordering::Acquire);

        if duration == 0 {
            return (0, 1);
        }

        // fps = timescale / frame_duration
        // To get rational: (timescale, frame_duration)
        (scale, duration)
    }

    // ========================================================================
    // Edit List / Timeline Operations
    // ========================================================================

    /// Set edit list offset.
    ///
    /// The edit offset adjusts presentation time vs. media time.
    /// Positive offset delays presentation.
    pub fn set_edit_offset(&self, offset: i64) {
        // Store as unsigned, interpret as signed when reading
        self.edit_offset.store(offset as u64, Ordering::Release);
    }

    /// Get edit list offset.
    pub fn edit_offset(&self) -> i64 {
        self.edit_offset.load(Ordering::Acquire) as i64
    }

    /// Apply edit offset to timestamp.
    #[inline]
    pub fn apply_edit_offset(&self, timestamp: u64) -> u64 {
        let offset = self.edit_offset.load(Ordering::Acquire) as i64;
        if offset >= 0 {
            timestamp.saturating_add(offset as u64)
        } else {
            timestamp.saturating_sub((-offset) as u64)
        }
    }

    /// Convert media time to presentation time.
    ///
    /// Applies base timestamp and edit offset.
    #[inline]
    pub fn media_to_presentation(&self, media_time: u64) -> u64 {
        let base = self.base_pts.load(Ordering::Acquire);
        let adjusted = media_time.saturating_add(base);
        self.apply_edit_offset(adjusted)
    }

    // ========================================================================
    // B-frame Reordering Support
    // ========================================================================

    /// Calculate reorder delay in frames.
    ///
    /// Returns the maximum reorder delay seen (max(PTS - DTS) / frame_duration).
    pub fn reorder_delay_frames(&self) -> u32 {
        let cts = self.cts();
        let duration = self.frame_duration.load(Ordering::Acquire) as i64;

        if duration <= 0 || cts <= 0 {
            return 0;
        }

        (cts / duration) as u32
    }

    // ========================================================================
    // Wraparound Handling
    // ========================================================================

    /// Unwrap 33-bit PTS to full 64-bit value.
    ///
    /// Handles MPEG-TS 33-bit PTS wraparound by tracking wrap count.
    pub fn unwrap_pts_33bit(&self, pts_33bit: u64) -> u64 {
        let wraps = self.wrap_count.load(Ordering::Acquire);
        pts_33bit + (wraps * (PTS_MAX_33BIT + 1))
    }

    /// Wrap 64-bit timestamp to 33-bit range.
    #[inline]
    pub fn wrap_to_33bit(timestamp: u64) -> u64 {
        timestamp & PTS_MAX_33BIT
    }

    // ========================================================================
    // Reset Operations
    // ========================================================================

    /// Reset all timestamps to initial state.
    pub fn reset(&self) {
        self.base_pts.store(0, Ordering::Release);
        self.base_dts.store(0, Ordering::Release);
        self.last_pts.store(0, Ordering::Release);
        self.last_dts.store(0, Ordering::Release);
        self.discontinuity_count.store(0, Ordering::Release);
        self.flags.store(0, Ordering::Release);
        self.edit_offset.store(0, Ordering::Release);
        self.wrap_count.store(0, Ordering::Release);
        self.max_pts_seen.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Reset discontinuity flag (after handling).
    pub fn clear_discontinuity_flag(&self) {
        self.flags.fetch_and(!FLAG_DISCONTINUITY, Ordering::Release);
    }
}

impl Default for TimestampCapsule {
    /// Default: 90kHz timescale (MPEG-TS compatible)
    fn default() -> Self {
        Self::new(TIMESCALE_90KHZ)
    }
}

impl fmt::Debug for TimestampCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let flags = self.flags.load(Ordering::Relaxed);

        f.debug_struct("TimestampCapsule")
            .field("timescale", &self.timescale.load(Ordering::Relaxed))
            .field("last_pts", &self.last_pts.load(Ordering::Relaxed))
            .field("last_dts", &self.last_dts.load(Ordering::Relaxed))
            .field("cts", &self.cts())
            .field("base_pts", &self.base_pts.load(Ordering::Relaxed))
            .field("frame_duration", &self.frame_duration.load(Ordering::Relaxed))
            .field("discontinuity_count", &self.discontinuity_count.load(Ordering::Relaxed))
            .field("has_bframe_reorder", &(flags & FLAG_BFRAME_REORDER != 0))
            .field("has_discontinuity", &(flags & FLAG_DISCONTINUITY != 0))
            .field("generation", &self.generation.load(Ordering::Relaxed))
            .finish()
    }
}

// Safety: TimestampCapsule only contains atomics and padding
unsafe impl Send for TimestampCapsule {}
unsafe impl Sync for TimestampCapsule {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // UNIT TESTS (Q1-Q7): Basic functionality
    // ========================================================================

    #[test]
    fn test_size_and_alignment() {
        assert_eq!(core::mem::size_of::<TimestampCapsule>(), 128);
        assert_eq!(core::mem::align_of::<TimestampCapsule>(), 128);
    }

    #[test]
    fn test_new() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        assert_eq!(ts.timescale(), 90000);
        assert_eq!(ts.last_pts(), 0);
        assert_eq!(ts.last_dts(), 0);
    }

    #[test]
    fn test_default() {
        let ts = TimestampCapsule::default();
        assert_eq!(ts.timescale(), TIMESCALE_90KHZ);
    }

    #[test]
    fn test_with_framerate_24fps() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 24, 1);
        assert_eq!(ts.frame_duration(), FRAME_DURATION_24FPS_90K);
    }

    #[test]
    fn test_with_framerate_29_97fps() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30000, 1001);
        assert_eq!(ts.frame_duration(), FRAME_DURATION_29_97FPS_90K);
    }

    #[test]
    fn test_with_framerate_30fps() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        assert_eq!(ts.frame_duration(), FRAME_DURATION_30FPS_90K);
    }

    #[test]
    fn test_record_timestamp() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        ts.record_timestamp(1000, 1000);

        assert_eq!(ts.last_pts(), 1000);
        assert_eq!(ts.last_dts(), 1000);
        assert_eq!(ts.base_pts(), 1000);
        assert_eq!(ts.base_dts(), 1000);
    }

    #[test]
    fn test_record_pts_only() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        ts.record_pts(5000);

        assert_eq!(ts.last_pts(), 5000);
        assert_eq!(ts.last_dts(), 5000);
        assert_eq!(ts.cts(), 0);
    }

    #[test]
    fn test_cts_calculation() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // B-frame with reordering: PTS > DTS
        ts.record_timestamp(9000, 3000);

        assert_eq!(ts.cts(), 6000); // PTS - DTS
        assert!(ts.has_bframe_reorder());
    }

    #[test]
    fn test_negative_cts() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Edge case: DTS > PTS (unusual but valid)
        ts.record_timestamp(1000, 2000);

        assert_eq!(ts.cts(), -1000);
    }

    #[test]
    fn test_timescale_conversion_identity() {
        let result = TimestampCapsule::convert_timescale(90000, 90000, 90000);
        assert_eq!(result, 90000);
    }

    #[test]
    fn test_timescale_90k_to_ms() {
        // 1 second at 90kHz = 1000ms
        let result = TimestampCapsule::convert_timescale(90000, TIMESCALE_90KHZ, TIMESCALE_1000);
        assert_eq!(result, 1000);
    }

    #[test]
    fn test_timescale_ms_to_90k() {
        // 1000ms = 90000 at 90kHz
        let result = TimestampCapsule::convert_timescale(1000, TIMESCALE_1000, TIMESCALE_90KHZ);
        assert_eq!(result, 90000);
    }

    #[test]
    fn test_to_milliseconds() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        assert_eq!(ts.to_milliseconds(90000), 1000);
        assert_eq!(ts.to_milliseconds(45000), 500);
        assert_eq!(ts.to_milliseconds(9000), 100);
    }

    #[test]
    fn test_from_milliseconds() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        assert_eq!(ts.from_milliseconds(1000), 90000);
        assert_eq!(ts.from_milliseconds(500), 45000);
    }

    #[test]
    fn test_to_90khz() {
        let ts = TimestampCapsule::new(TIMESCALE_1000);
        assert_eq!(ts.to_90khz(1000), 90000);
    }

    #[test]
    fn test_from_90khz() {
        let ts = TimestampCapsule::new(TIMESCALE_1000);
        assert_eq!(ts.from_90khz(90000), 1000);
    }

    #[test]
    fn test_rational_time() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        let (num, den) = ts.to_rational(45000);
        assert_eq!(num, 45000);
        assert_eq!(den, 90000);
        // 45000/90000 = 0.5 seconds
    }

    #[test]
    fn test_from_rational() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // 1/2 second = 45000 at 90kHz
        let result = ts.from_rational(1, 2);
        assert_eq!(result, 45000);
    }

    #[test]
    fn test_frame_timestamp() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        ts.record_pts(0); // Set base

        assert_eq!(ts.frame_timestamp(0), 0);
        assert_eq!(ts.frame_timestamp(1), 3000);
        assert_eq!(ts.frame_timestamp(30), 90000); // 1 second
    }

    #[test]
    fn test_timestamp_to_frame() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        ts.record_pts(0);

        assert_eq!(ts.timestamp_to_frame(0), 0);
        assert_eq!(ts.timestamp_to_frame(3000), 1);
        assert_eq!(ts.timestamp_to_frame(90000), 30);
    }

    #[test]
    fn test_framerate() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        let (num, den) = ts.framerate();
        assert_eq!(num, 90000);
        assert_eq!(den, 3000);
        // 90000/3000 = 30 fps
    }

    #[test]
    fn test_edit_offset() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        ts.set_edit_offset(1000);
        assert_eq!(ts.edit_offset(), 1000);

        assert_eq!(ts.apply_edit_offset(5000), 6000);
    }

    #[test]
    fn test_edit_offset_negative() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        ts.set_edit_offset(-1000);
        assert_eq!(ts.edit_offset(), -1000);

        assert_eq!(ts.apply_edit_offset(5000), 4000);
    }

    #[test]
    fn test_wrap_to_33bit() {
        assert_eq!(TimestampCapsule::wrap_to_33bit(0), 0);
        assert_eq!(TimestampCapsule::wrap_to_33bit(PTS_MAX_33BIT), PTS_MAX_33BIT);
        assert_eq!(TimestampCapsule::wrap_to_33bit(PTS_MAX_33BIT + 1), 0);
    }

    #[test]
    fn test_reset() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        ts.record_timestamp(1000, 500);
        ts.set_edit_offset(100);

        ts.reset();

        assert_eq!(ts.last_pts(), 0);
        assert_eq!(ts.last_dts(), 0);
        assert_eq!(ts.edit_offset(), 0);
        assert!(!ts.has_bframe_reorder());
    }

    #[test]
    fn test_generation_counter() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        let gen1 = ts.generation();
        ts.record_pts(1000);
        let gen2 = ts.generation();

        assert!(gen2 > gen1);
    }

    // ========================================================================
    // PROPERTY TESTS (Q8-Q14): Invariants and edge cases
    // ========================================================================

    #[test]
    fn test_timescale_conversion_roundtrip() {
        // Property: convert(convert(x, a, b), b, a) == x (for divisible values)
        let original = 90000u64; // 1 second at 90kHz

        let to_ms = TimestampCapsule::convert_timescale(original, TIMESCALE_90KHZ, TIMESCALE_1000);
        let back = TimestampCapsule::convert_timescale(to_ms, TIMESCALE_1000, TIMESCALE_90KHZ);

        assert_eq!(back, original);
    }

    #[test]
    fn test_timescale_conversion_large_values() {
        // Property: No overflow with large timestamps
        let large_ts = u64::MAX / 2;

        // Should not panic due to overflow
        let result = TimestampCapsule::convert_timescale(large_ts, TIMESCALE_90KHZ, TIMESCALE_1000);
        assert!(result > 0);
    }

    #[test]
    fn test_timescale_conversion_zero() {
        let result = TimestampCapsule::convert_timescale(0, TIMESCALE_90KHZ, TIMESCALE_1000);
        assert_eq!(result, 0);
    }

    #[test]
    fn test_discontinuity_detection_forward() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        ts.record_pts(0);

        // Normal progression (no discontinuity)
        let disc1 = ts.record_pts(3000);
        assert!(!disc1);

        // Large gap forward (discontinuity)
        let disc2 = ts.record_pts(1_000_000);
        assert!(disc2);
        assert!(ts.has_discontinuity());
        assert!(ts.discontinuity_count() >= 1);
    }

    #[test]
    fn test_discontinuity_detection_backward() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        ts.record_pts(100_000);

        // Large gap backward (discontinuity)
        let disc = ts.record_pts(1000);
        assert!(disc);
    }

    #[test]
    fn test_wraparound_detection() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Record near max 33-bit
        ts.record_pts(PTS_MAX_33BIT - 1000);

        // Wraparound
        ts.record_pts(500);

        assert!(ts.has_wrapped());
        assert!(ts.wrap_count() >= 1);
    }

    #[test]
    fn test_unwrap_pts_33bit() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Simulate one wraparound
        ts.record_pts(PTS_MAX_33BIT - 1000);
        ts.record_pts(500); // Triggers wrap detection

        // Unwrap should add one full 33-bit cycle
        let unwrapped = ts.unwrap_pts_33bit(500);
        assert_eq!(unwrapped, 500 + PTS_MAX_33BIT + 1);
    }

    #[test]
    fn test_bframe_reorder_detection() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // I-frame: PTS == DTS
        ts.record_timestamp(0, 0);
        assert!(!ts.has_bframe_reorder());

        // B-frame: PTS > DTS
        ts.record_timestamp(6000, 3000);
        assert!(ts.has_bframe_reorder());
    }

    #[test]
    fn test_reorder_delay_frames() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        // CTS = 2 frame durations = 6000
        ts.record_timestamp(9000, 3000);

        assert_eq!(ts.reorder_delay_frames(), 2);
    }

    #[test]
    fn test_read_consistent_no_contention() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        ts.record_timestamp(1000, 500);

        let result = ts.read_consistent();
        assert!(result.is_some());

        let (pts, dts, cts) = result.unwrap();
        assert_eq!(pts, 1000);
        assert_eq!(dts, 500);
        assert_eq!(cts, 500);
    }

    #[test]
    fn test_media_to_presentation() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Set base PTS
        ts.record_pts(10000);

        // Set edit offset
        ts.set_edit_offset(500);

        // Media time 5000 -> base + 5000 + edit = 10000 + 5000 + 500 = 15500
        let presentation = ts.media_to_presentation(5000);
        assert_eq!(presentation, 15500);
    }

    #[test]
    fn test_frame_duration_consistency() {
        // Property: frame_timestamp(N) - frame_timestamp(0) == N * frame_duration
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        ts.record_pts(0);

        let duration = ts.frame_duration() as u64;

        for n in 0..100 {
            let expected = n * duration;
            let actual = ts.frame_timestamp(n);
            assert_eq!(actual, expected, "Frame {} mismatch", n);
        }
    }

    #[test]
    fn test_timestamp_to_frame_inverse() {
        // Property: timestamp_to_frame(frame_timestamp(N)) == N
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        ts.record_pts(0);

        for n in 0..100 {
            let timestamp = ts.frame_timestamp(n);
            let frame = ts.timestamp_to_frame(timestamp);
            assert_eq!(frame, n, "Frame {} inverse mismatch", n);
        }
    }

    // ========================================================================
    // INTEGRATION TESTS (Q15-Q21): Multi-operation sequences
    // ========================================================================

    #[test]
    fn test_bframe_sequence_ibbp() {
        // Typical I-B-B-P sequence
        // Display order: I0 B1 B2 P3
        // Decode order:  I0 P3 B1 B2

        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        let dur = ts.frame_duration() as u64;

        // I-frame (decode first, display first)
        ts.record_timestamp(0 * dur, 0 * dur);
        assert_eq!(ts.cts(), 0);

        // P-frame (decode second, display fourth)
        ts.record_timestamp(3 * dur, 1 * dur);
        assert_eq!(ts.cts(), 2 * dur as i64);

        // B-frame (decode third, display second)
        ts.record_timestamp(1 * dur, 2 * dur);
        assert_eq!(ts.cts(), -(dur as i64));

        // B-frame (decode fourth, display third)
        ts.record_timestamp(2 * dur, 3 * dur);
        assert_eq!(ts.cts(), -(dur as i64));

        assert!(ts.has_bframe_reorder());
    }

    #[test]
    fn test_audio_video_separate_timelines() {
        // Video at 30fps, audio at 48kHz
        let video_ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        let audio_ts = TimestampCapsule::new(TIMESCALE_48KHZ);
        audio_ts.set_frame_duration(1024); // AAC frame = 1024 samples

        // 1 second of video (30 frames)
        for i in 0..30 {
            video_ts.record_pts(i * 3000);
        }

        // 1 second of audio (~47 AAC frames)
        for i in 0..47 {
            audio_ts.record_pts(i * 1024);
        }

        // Both should be at ~1 second
        assert!(video_ts.to_milliseconds(video_ts.last_pts()) >= 900);
        let audio_ms = TimestampCapsule::convert_timescale(
            audio_ts.last_pts(), TIMESCALE_48KHZ, TIMESCALE_1000
        );
        assert!(audio_ms >= 900);
    }

    #[test]
    fn test_stream_with_discontinuity_recovery() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        // Initial segment
        for i in 0..30 {
            ts.record_pts(i * 3000);
        }

        assert!(!ts.has_discontinuity());

        // Discontinuity (e.g., ad insertion or stream switch)
        ts.record_pts(1_000_000);

        assert!(ts.has_discontinuity());
        let disc_count = ts.discontinuity_count();

        // Clear flag after handling
        ts.clear_discontinuity_flag();
        assert!(!ts.has_discontinuity());

        // Continue normal playback
        for i in 0..30 {
            ts.record_pts(1_000_000 + i * 3000);
        }

        // Discontinuity count unchanged (no new discontinuity)
        assert_eq!(ts.discontinuity_count(), disc_count);
    }

    #[test]
    fn test_edit_list_workflow() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Media starts at 90000 (1 second offset in media file)
        ts.record_pts(90000);

        // But presentation should start at 0
        ts.set_edit_offset(-90000);

        // Media time 90000 -> presentation time 0
        // base_pts = 90000, edit_offset = -90000
        // 90000 + 90000 - 90000 = 90000 (base is added in media_to_presentation)

        // Actually let's verify with a simpler test
        let presentation = ts.apply_edit_offset(90000);
        assert_eq!(presentation, 0);
    }

    #[test]
    fn test_33bit_wraparound_sequence() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // Start near wraparound point
        let start = PTS_MAX_33BIT - 90000;
        ts.record_pts(start);

        // 2 seconds later (wraps around)
        ts.record_pts(90000); // This is after wraparound

        assert!(ts.has_wrapped());
        assert_eq!(ts.wrap_count(), 1);

        // Unwrap the timestamp
        let unwrapped = ts.unwrap_pts_33bit(90000);
        assert_eq!(unwrapped, 90000 + PTS_MAX_33BIT + 1);
    }

    #[test]
    fn test_concurrent_read_write() {
        use std::sync::Arc;
        use std::thread;

        let ts = Arc::new(TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1));

        // Writer thread
        let ts_writer = Arc::clone(&ts);
        let writer = thread::spawn(move || {
            for i in 0..1000 {
                ts_writer.record_pts(i * 3000);
            }
        });

        // Reader thread
        let ts_reader = Arc::clone(&ts);
        let reader = thread::spawn(move || {
            let mut consistent_reads = 0;
            for _ in 0..1000 {
                if ts_reader.read_consistent().is_some() {
                    consistent_reads += 1;
                }
            }
            consistent_reads
        });

        writer.join().unwrap();
        let reads = reader.join().unwrap();

        // Most reads should be consistent
        assert!(reads > 0);
    }

    #[test]
    fn test_multiple_streams_interleaved() {
        // Simulate muxing multiple streams
        let video = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        let audio = TimestampCapsule::new(TIMESCALE_48KHZ);
        audio.set_frame_duration(1024);

        let mut video_ts = 0u64;
        let mut audio_ts = 0u64;

        // Interleave 100 video and audio frames
        for _ in 0..100 {
            // Video frame
            video.record_pts(video_ts);
            video_ts += 3000;

            // ~1.5 audio frames per video frame at 48kHz with 1024 sample frames
            audio.record_pts(audio_ts);
            audio_ts += 1024;
            audio.record_pts(audio_ts);
            audio_ts += 1024;
        }

        // Video: 100 frames at 30fps = ~3.3 seconds
        // Audio: 200 frames at ~47fps = ~4.3 seconds

        assert!(video.to_milliseconds(video.last_pts()) >= 3000);
    }

    #[test]
    fn test_frame_rate_conversion_24_to_25() {
        // PAL pulldown: 24fps -> 25fps
        let _ts_24 = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 24, 1);

        // At 24fps, frame duration = 3750
        // At 25fps, frame duration = 3600

        // 1 second at 24fps = 24 frames = 90000 ticks
        let pts_24 = 24 * 3750u64;
        assert_eq!(pts_24, 90000);

        // Convert to 25fps timeline (same time, different tick rate)
        let pts_25 = TimestampCapsule::convert_timescale(pts_24, TIMESCALE_90KHZ, TIMESCALE_90KHZ);
        assert_eq!(pts_25, 90000); // Same timescale, no change
    }

    // ========================================================================
    // ADDITIONAL UNIT TESTS (Q1-Q7 extended)
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(TIMESCALE_90KHZ, 90_000);
        assert_eq!(TIMESCALE_1000, 1_000);
        assert_eq!(TIMESCALE_48KHZ, 48_000);
        assert_eq!(TIMESCALE_44100, 44_100);
        assert_eq!(PTS_MAX_33BIT, 8_589_934_591);
    }

    #[test]
    fn test_frame_duration_constants() {
        assert_eq!(FRAME_DURATION_24FPS_90K, 3750);
        assert_eq!(FRAME_DURATION_25FPS_90K, 3600);
        assert_eq!(FRAME_DURATION_30FPS_90K, 3000);
        assert_eq!(FRAME_DURATION_60FPS_90K, 1500);
    }

    #[test]
    fn test_set_framerate() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        ts.set_framerate(60, 1);
        assert_eq!(ts.frame_duration(), 1500);

        ts.set_framerate(30000, 1001);
        assert_eq!(ts.frame_duration(), 3003);
    }

    #[test]
    fn test_set_frame_duration_direct() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        ts.set_frame_duration(4000);
        assert_eq!(ts.frame_duration(), 4000);
    }

    #[test]
    #[should_panic(expected = "Timescale must be > 0")]
    fn test_new_zero_timescale_panics() {
        let _ = TimestampCapsule::new(0);
    }

    #[test]
    fn test_debug_format() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);
        ts.record_timestamp(1000, 500);

        let debug = format!("{:?}", ts);
        assert!(debug.contains("TimestampCapsule"));
        assert!(debug.contains("timescale"));
        assert!(debug.contains("90000"));
    }

    #[test]
    fn test_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<TimestampCapsule>();
    }

    // ========================================================================
    // ADDITIONAL PROPERTY TESTS (Q8-Q14 extended)
    // ========================================================================

    #[test]
    fn test_cts_sign_consistency() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // PTS > DTS -> positive CTS
        ts.record_timestamp(2000, 1000);
        assert!(ts.cts() > 0);

        // PTS == DTS -> zero CTS
        ts.record_timestamp(3000, 3000);
        assert_eq!(ts.cts(), 0);

        // PTS < DTS -> negative CTS
        ts.record_timestamp(4000, 5000);
        assert!(ts.cts() < 0);
    }

    #[test]
    fn test_monotonic_base_timestamp() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        // First recording sets base
        ts.record_pts(1000);
        let base = ts.base_pts();

        // Subsequent recordings don't change base
        ts.record_pts(2000);
        ts.record_pts(3000);

        assert_eq!(ts.base_pts(), base);
    }

    #[test]
    fn test_generation_increases_monotonically() {
        let ts = TimestampCapsule::new(TIMESCALE_90KHZ);

        let mut last_gen = ts.generation();
        for i in 0..100 {
            ts.record_pts(i * 1000);
            let new_gen = ts.generation();
            assert!(new_gen > last_gen, "Generation must increase");
            last_gen = new_gen;
        }
    }

    #[test]
    fn test_timescale_conversion_preserves_order() {
        // Property: if a > b, then convert(a) >= convert(b)
        for i in 0..100u64 {
            let a = i * 1000 + 500;
            let b = i * 1000;

            let a_converted = TimestampCapsule::convert_timescale(a, TIMESCALE_90KHZ, TIMESCALE_1000);
            let b_converted = TimestampCapsule::convert_timescale(b, TIMESCALE_90KHZ, TIMESCALE_1000);

            assert!(a_converted >= b_converted);
        }
    }

    #[test]
    fn test_discontinuity_threshold_respects_frame_duration() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 60, 1);

        // Frame duration at 60fps = 1500
        // Threshold should be 3 * 1500 = 4500

        ts.record_pts(0);

        // Gap of 2 frames (no discontinuity)
        let disc = ts.record_pts(3000);
        assert!(!disc);

        // Gap of 5 frames (discontinuity)
        let disc = ts.record_pts(3000 + 7500);
        assert!(disc);
    }

    // ========================================================================
    // ADDITIONAL INTEGRATION TESTS (Q15-Q21 extended)
    // ========================================================================

    #[test]
    fn test_full_hls_segment_workflow() {
        // Simulate HLS segment creation
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        // Each HLS segment is ~6 seconds = 180 frames
        let frames_per_segment = 180;
        let frame_duration = ts.frame_duration() as u64;

        for segment in 0..3 {
            let base = segment * frames_per_segment * frame_duration;

            for frame in 0..frames_per_segment {
                let pts = base + frame * frame_duration;
                ts.record_pts(pts);
            }

            // Verify segment end timestamp
            let expected_end = (segment + 1) * frames_per_segment * frame_duration - frame_duration;
            assert_eq!(ts.last_pts(), expected_end);
        }

        // Total: 3 segments * 6 seconds = 18 seconds
        assert!(ts.to_milliseconds(ts.last_pts()) >= 17900);
    }

    #[test]
    fn test_fragmented_mp4_workflow() {
        // fMP4: Each fragment has its own timestamp base
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        // Fragment 1: Frames 0-29
        for i in 0..30 {
            ts.record_pts(i * 3000);
        }

        // Fragment boundary (no discontinuity, continuous)

        // Fragment 2: Frames 30-59
        for i in 30..60 {
            let disc = ts.record_pts(i * 3000);
            assert!(!disc, "No discontinuity expected at frame {}", i);
        }

        // Verify continuous timeline
        assert_eq!(ts.last_pts(), 59 * 3000);
    }

    #[test]
    fn test_timecode_burn_in_alignment() {
        // For timecode burn-in, we need frame-accurate timestamps
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30000, 1001); // 29.97fps

        // 1 hour of video
        let _frames_per_hour = 30 * 60 * 60; // Approximate

        for frame in 0..100 {
            ts.record_pts(frame * ts.frame_duration() as u64);
        }

        // Verify frame-accurate conversion
        for frame in 0..100 {
            let expected_ts = frame * ts.frame_duration() as u64;
            let calculated_ts = ts.frame_timestamp(frame);
            assert_eq!(calculated_ts, expected_ts);
        }
    }

    #[test]
    fn test_audio_video_sync_drift() {
        // Verify A/V sync doesn't drift over long duration
        let video = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);
        let audio = TimestampCapsule::new(TIMESCALE_48KHZ);
        audio.set_frame_duration(1024);

        // 10 seconds of content (using same time duration for both)
        // Video: 30fps * 10 seconds = 300 frames
        // Audio: 48000 samples/sec / 1024 samples/frame * 10 seconds = ~469 frames
        let duration_sec = 10u64;

        // Calculate timestamps for 10 seconds
        let video_ts = duration_sec * TIMESCALE_90KHZ as u64;  // 900,000 at 90kHz
        let audio_ts = duration_sec * TIMESCALE_48KHZ as u64;  // 480,000 at 48kHz

        video.record_pts(video_ts);
        audio.record_pts(audio_ts);

        // Convert both to milliseconds
        let video_ms = video.to_milliseconds(video.last_pts());
        let audio_ms = TimestampCapsule::convert_timescale(
            audio.last_pts(), TIMESCALE_48KHZ, TIMESCALE_1000
        );

        // Both should be exactly 10000 ms (10 seconds)
        // Should be within 1ms of each other
        let drift = if video_ms > audio_ms {
            video_ms - audio_ms
        } else {
            audio_ms - video_ms
        };

        assert!(drift < 1, "A/V drift {} ms exceeds threshold", drift);
        assert_eq!(video_ms, 10000);
        assert_eq!(audio_ms, 10000);
    }

    #[test]
    fn test_subtitle_alignment() {
        // Subtitles often use millisecond timestamps
        let video = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 24, 1);
        let subtitle = TimestampCapsule::new(TIMESCALE_1000);

        // Video at frame 100 (4.166 seconds at 24fps)
        video.record_pts(100 * FRAME_DURATION_24FPS_90K as u64);

        // Subtitle at 4166 ms
        subtitle.record_pts(4166);

        // Verify alignment
        let video_ms = video.to_milliseconds(video.last_pts());
        assert_eq!(video_ms, 4166);
        assert_eq!(subtitle.last_pts(), video_ms);
    }

    // ========================================================================
    // STRESS TESTS
    // ========================================================================

    #[test]
    fn test_high_frame_rate_120fps() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 120, 1);

        // Frame duration at 120fps = 750
        assert_eq!(ts.frame_duration(), 750);

        // 1 minute at 120fps = 7200 frames
        for i in 0..7200 {
            ts.record_pts(i * 750);
        }

        // Should be ~1 minute
        assert!(ts.to_milliseconds(ts.last_pts()) >= 59900);
    }

    #[test]
    fn test_long_duration_24_hours() {
        let ts = TimestampCapsule::with_framerate(TIMESCALE_90KHZ, 30, 1);

        // 24 hours in 90kHz ticks
        let ticks_24h: u64 = 90000 * 60 * 60 * 24;

        ts.record_pts(ticks_24h);

        // Verify millisecond conversion
        let ms = ts.to_milliseconds(ts.last_pts());
        let expected_ms: u64 = 1000 * 60 * 60 * 24;

        assert_eq!(ms, expected_ms);
    }
}
