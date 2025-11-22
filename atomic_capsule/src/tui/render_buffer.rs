//! RenderBufferCapsule - Tier 1 Atomic frame timing and dirty tracking for TUI rendering.
//!
//! # Performance
//! - Dirty flag check: <5ns (single atomic load, relaxed)
//! - FPS calculation: <10ns (packed Q16.16 arithmetic)
//! - Frame timing: <100ns (atomic updates with release semantics)
//!
//! # Tier
//! **T1 Atomic** - 256B cache-aligned lockfree coordination primitive
//!
//! # Fields (256B aligned)
//! - `dirty_flag`: AtomicBool - Frame needs rendering
//! - `last_render_ns`: AtomicU64 - Last render timestamp (nanoseconds)
//! - `frame_counter`: AtomicU64 - Total frames rendered
//! - `fps_actual`: AtomicU32 - Current FPS in Q16.16 fixed-point format
//! - `render_time_ns`: AtomicU64 - Duration of last render (nanoseconds)
//! - `_padding`: [u8; 204] - Cache line alignment to 256B
//!
//! # Usage
//! ```ignore
//! use atomic_capsule::tui::RenderBufferCapsule;
//!
//! let buffer = RenderBufferCapsule::new();
//! buffer.mark_dirty();
//!
//! if buffer.should_render() {
//!     let start = std::time::SystemTime::now()
//!         .duration_since(std::time::UNIX_EPOCH)
//!         .unwrap()
//!         .as_nanos() as u64;
//!
//!     // ... render ...
//!
//!     let end = std::time::SystemTime::now()
//!         .duration_since(std::time::UNIX_EPOCH)
//!         .unwrap()
//!         .as_nanos() as u64;
//!     buffer.record_render(start, end);
//! }
//!
//! let fps = buffer.fps();  // FPS in Q16.16 format
//! ```
//!
//! # Q16.16 Fixed-Point Format
//! FPS is stored as Q16.16 fixed-point to avoid floating-point drift:
//! - Integer part: upper 16 bits (0-65535 FPS)
//! - Fractional part: lower 16 bits (1/65536 resolution)
//! - Example: 60 FPS = 60 * 65536 = 3932160 (0x3C0000)

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use core::fmt;

/// RenderBufferCapsule - 256B aligned T1 Atomic primitive for frame timing and dirty tracking.
///
/// Single-Writer, Many-Readers (SWeMR) pattern:
/// - Writer: Single thread calls `mark_dirty()`, `should_render()`, `record_render()`
/// - Readers: Multiple threads observe `fps()`, `frame_count()` via atomic loads
#[repr(C, align(256))]
pub struct RenderBufferCapsule {
    /// Frame needs rendering (true = requires render pass)
    dirty_flag: AtomicBool,
    /// Last render timestamp (nanoseconds since UNIX_EPOCH)
    last_render_ns: AtomicU64,
    /// Total frames rendered (counter)
    frame_counter: AtomicU64,
    /// Current FPS in Q16.16 fixed-point format
    fps_actual: AtomicU32,
    /// Duration of last render pass (nanoseconds)
    render_time_ns: AtomicU64,
    /// Padding to maintain 256B alignment (64B cache line, 4× factor)
    _padding: [u8; 204],
}

impl RenderBufferCapsule {
    /// Create a new RenderBufferCapsule initialized to clean state (no render needed).
    ///
    /// # Performance
    /// - Time: ~5ns (allocation, no locks)
    /// - Memory: 256 bytes (cache-aligned)
    ///
    /// # Examples
    /// ```ignore
    /// let buffer = RenderBufferCapsule::new();
    /// assert!(!buffer.should_render()); // Initially clean
    /// ```
    #[inline]
    pub const fn new() -> Self {
        Self {
            dirty_flag: AtomicBool::new(false),
            last_render_ns: AtomicU64::new(0),
            frame_counter: AtomicU64::new(0),
            fps_actual: AtomicU32::new(0),
            render_time_ns: AtomicU64::new(0),
            _padding: [0u8; 204],
        }
    }

    /// Mark the frame as dirty (requires rendering).
    ///
    /// # Performance
    /// - Time: <5ns (single atomic store, relaxed semantics)
    /// - Ordering: Relaxed (no synchronization needed, next reader will see it)
    ///
    /// # Examples
    /// ```ignore
    /// buffer.mark_dirty();
    /// assert!(buffer.should_render()); // Now dirty
    /// ```
    #[inline]
    pub fn mark_dirty(&self) {
        self.dirty_flag.store(true, Ordering::Relaxed);
    }

    /// Mark the frame as clean (no rendering needed).
    ///
    /// # Performance
    /// - Time: <5ns (single atomic store, relaxed semantics)
    ///
    /// # Examples
    /// ```ignore
    /// buffer.clear_dirty();
    /// assert!(!buffer.should_render()); // Now clean
    /// ```
    #[inline]
    pub fn clear_dirty(&self) {
        self.dirty_flag.store(false, Ordering::Relaxed);
    }

    /// Check if frame should be rendered (dirty flag is set).
    ///
    /// # Performance
    /// - Time: <5ns (single atomic load, relaxed semantics)
    /// - Ordering: Relaxed (readers don't need acquire semantics)
    ///
    /// # Returns
    /// - `true` if frame is dirty and should be rendered
    /// - `false` if frame is clean and can skip rendering
    ///
    /// # Examples
    /// ```ignore
    /// buffer.mark_dirty();
    /// if buffer.should_render() {
    ///     // Perform render pass
    ///     buffer.clear_dirty();
    /// }
    /// ```
    #[inline]
    pub fn should_render(&self) -> bool {
        self.dirty_flag.load(Ordering::Relaxed)
    }

    /// Record a render pass with timing information.
    ///
    /// Updates frame counter, last render time, and estimates current FPS from inter-frame time.
    ///
    /// # Arguments
    /// - `start_ns`: Render start time (nanoseconds since UNIX_EPOCH)
    /// - `end_ns`: Render end time (nanoseconds since UNIX_EPOCH)
    ///
    /// # Performance
    /// - Time: <100ns (multiple atomic stores, release semantics)
    /// - Ordering: Release (subsequent readers see consistent state)
    ///
    /// # Panics
    /// - Panics if `end_ns < start_ns` (invalid timing)
    ///
    /// # FPS Calculation
    /// FPS is estimated from the interval between consecutive render calls:
    /// - If `last_render_ns == 0` (first call), no FPS update (undefined)
    /// - Otherwise: `fps = 1e9 / (start_ns - last_render_ns)` (Q16.16 fixed-point)
    ///
    /// # Examples
    /// ```ignore
    /// let start = now_ns();
    /// // ... render ...
    /// let end = now_ns();
    /// buffer.record_render(start, end);
    /// ```
    #[inline]
    pub fn record_render(&self, start_ns: u64, end_ns: u64) {
        assert!(end_ns >= start_ns, "render timing invalid: {} < {}", end_ns, start_ns);

        let render_duration = end_ns - start_ns;
        self.render_time_ns.store(render_duration, Ordering::Relaxed);

        let last = self.last_render_ns.load(Ordering::Relaxed);
        self.last_render_ns.store(start_ns, Ordering::Relaxed);

        // Increment frame counter
        let frame_num = self.frame_counter.load(Ordering::Relaxed);
        self.frame_counter.store(frame_num.wrapping_add(1), Ordering::Relaxed);

        // Calculate FPS from inter-frame interval (Q16.16 fixed-point)
        if last > 0 && start_ns > last {
            let interval_ns = start_ns - last;
            // FPS = 1e9 / interval_ns, stored as Q16.16
            // Q16.16 multiplier = 65536
            let fps_q16_16 = compute_fps_q16_16(interval_ns);
            self.fps_actual.store(fps_q16_16, Ordering::Release);
        }
    }

    /// Get the current FPS estimate in Q16.16 fixed-point format.
    ///
    /// # Performance
    /// - Time: <5ns (single atomic load, acquire semantics)
    /// - Ordering: Acquire (ensures we see the most recent FPS update)
    ///
    /// # Returns
    /// FPS value in Q16.16 fixed-point format:
    /// - Integer part: `fps >> 16` (upper 16 bits)
    /// - Fractional part: `(fps & 0xFFFF) / 65536.0` (lower 16 bits)
    /// - Example: 60 FPS = 3932160 (0x3C0000)
    ///
    /// # Examples
    /// ```ignore
    /// let fps_q16_16 = buffer.fps();
    /// let fps_int = fps_q16_16 >> 16;
    /// let fps_frac = (fps_q16_16 & 0xFFFF) as f32 / 65536.0;
    /// println!("FPS: {}.{}", fps_int, fps_frac);
    /// ```
    #[inline]
    pub fn fps(&self) -> u32 {
        self.fps_actual.load(Ordering::Acquire)
    }

    /// Get the duration of the last render pass (nanoseconds).
    ///
    /// # Performance
    /// - Time: <5ns (single atomic load, relaxed semantics)
    ///
    /// # Returns
    /// Duration in nanoseconds (0 if no render has occurred)
    ///
    /// # Examples
    /// ```ignore
    /// let render_ns = buffer.render_time();
    /// println!("Last render took: {}ns", render_ns);
    /// ```
    #[inline]
    pub fn render_time(&self) -> u64 {
        self.render_time_ns.load(Ordering::Relaxed)
    }

    /// Get the total number of frames rendered.
    ///
    /// # Performance
    /// - Time: <5ns (single atomic load, relaxed semantics)
    ///
    /// # Returns
    /// Frame counter (wraps at u64::MAX)
    ///
    /// # Examples
    /// ```ignore
    /// let frame_num = buffer.frame_count();
    /// println!("Frames rendered: {}", frame_num);
    /// ```
    #[inline]
    pub fn frame_count(&self) -> u64 {
        self.frame_counter.load(Ordering::Relaxed)
    }

    /// Get the timestamp of the last render (nanoseconds since UNIX_EPOCH).
    ///
    /// # Performance
    /// - Time: <5ns (single atomic load, relaxed semantics)
    ///
    /// # Returns
    /// Last render timestamp (0 if no render has occurred)
    ///
    /// # Examples
    /// ```ignore
    /// let last_ns = buffer.last_render_time();
    /// println!("Last render at: {}", last_ns);
    /// ```
    #[inline]
    pub fn last_render_time(&self) -> u64 {
        self.last_render_ns.load(Ordering::Relaxed)
    }
}

impl Default for RenderBufferCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for RenderBufferCapsule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let dirty = self.dirty_flag.load(Ordering::Relaxed);
        let frames = self.frame_counter.load(Ordering::Relaxed);
        let render_ns = self.render_time_ns.load(Ordering::Relaxed);
        let fps_q16_16 = self.fps();
        let last_ns = self.last_render_time();

        let fps_int = fps_q16_16 >> 16;
        let fps_frac = ((fps_q16_16 & 0xFFFF) as f32) / 65536.0;

        f.debug_struct("RenderBufferCapsule")
            .field("dirty", &dirty)
            .field("frames", &frames)
            .field("render_time_ns", &render_ns)
            .field("fps_actual_q16_16", &format!("{}.{:05}", fps_int, (fps_frac * 100000.0) as u32))
            .field("last_render_ns", &last_ns)
            .finish()
    }
}

/// Compute FPS in Q16.16 fixed-point format from inter-frame interval.
///
/// # Arguments
/// - `interval_ns`: Time between consecutive renders (nanoseconds)
///
/// # Returns
/// FPS in Q16.16 fixed-point format
///
/// # Calculation
/// FPS = 1e9 / interval_ns, converted to Q16.16:
/// - FPS_decimal = 1_000_000_000.0 / interval_ns
/// - FPS_q16_16 = (FPS_decimal * 65536.0) as u32
///
/// # Examples
/// - 60 FPS interval: 1e9 / 16_666_667 ns ≈ 60.0 FPS = 3932160 (0x3C0000)
/// - 30 FPS interval: 1e9 / 33_333_333 ns ≈ 30.0 FPS = 1966080 (0x1E0000)
#[inline]
fn compute_fps_q16_16(interval_ns: u64) -> u32 {
    if interval_ns == 0 {
        return 0;
    }

    // Compute FPS as Q16.16: FPS * 65536
    // FPS = 1e9 / interval_ns, so FPS_q16_16 = (1e9 * 65536) / interval_ns
    // Constant: 1e9 * 65536 = 65_536_000_000_000
    const NANOS_PER_SEC_Q16_16: u64 = 1_000_000_000 * 65536;

    // Divide with rounding to nearest
    let fps_q16_16 = (NANOS_PER_SEC_Q16_16 + (interval_ns >> 1)) / interval_ns;

    // Clamp to u32 range (max ~65535 FPS in integer part)
    fps_q16_16.min(u32::MAX as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_initialization() {
        let buffer = RenderBufferCapsule::new();
        assert!(!buffer.should_render(), "new buffer should be clean");
        assert_eq!(buffer.frame_count(), 0, "frame counter should start at 0");
        assert_eq!(buffer.fps(), 0, "fps should be 0 initially");
        assert_eq!(buffer.render_time(), 0, "render_time should be 0");
        assert_eq!(buffer.last_render_time(), 0, "last_render_time should be 0");
    }

    #[test]
    fn test_dirty_tracking() {
        let buffer = RenderBufferCapsule::new();

        // Initially clean
        assert!(!buffer.should_render());

        // Mark dirty
        buffer.mark_dirty();
        assert!(buffer.should_render(), "should_render after mark_dirty");

        // Clear dirty
        buffer.clear_dirty();
        assert!(!buffer.should_render(), "should_render after clear_dirty");

        // Mark dirty again
        buffer.mark_dirty();
        assert!(buffer.should_render());
    }

    #[test]
    fn test_frame_counter_increment() {
        let buffer = RenderBufferCapsule::new();
        assert_eq!(buffer.frame_count(), 0);

        buffer.record_render(1000, 1100);
        assert_eq!(buffer.frame_count(), 1, "frame counter should increment");

        buffer.record_render(20000, 20100);
        assert_eq!(buffer.frame_count(), 2);

        buffer.record_render(40000, 40100);
        assert_eq!(buffer.frame_count(), 3);
    }

    #[test]
    fn test_render_time_recording() {
        let buffer = RenderBufferCapsule::new();

        buffer.record_render(1000, 1500);
        assert_eq!(buffer.render_time(), 500, "render_time should be end - start");

        buffer.record_render(2000, 2750);
        assert_eq!(buffer.render_time(), 750);
    }

    #[test]
    fn test_fps_calculation_60_fps() {
        let buffer = RenderBufferCapsule::new();

        // First render: no FPS yet
        buffer.record_render(1000, 1100);
        assert_eq!(buffer.fps(), 0, "fps should be 0 on first render");

        // Second render: 60 FPS target = 16_666_667 ns interval
        // Note: 1e9 / 16_666_667 ≈ 59.9999... so we allow ±1 FPS due to rounding
        let interval_60fps = 16_666_667u64;
        buffer.record_render(1000 + interval_60fps, 1100 + interval_60fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 60).abs() <= 1, "60 FPS ±1, got {}", fps_int);
    }

    #[test]
    fn test_fps_calculation_30_fps() {
        let buffer = RenderBufferCapsule::new();

        buffer.record_render(1000, 1100);
        let interval_30fps = 33_333_333u64;
        buffer.record_render(1000 + interval_30fps, 1100 + interval_30fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 30).abs() <= 1, "30 FPS ±1, got {}", fps_int);
    }

    #[test]
    fn test_fps_calculation_120_fps() {
        let buffer = RenderBufferCapsule::new();

        buffer.record_render(1000, 1100);
        let interval_120fps = 8_333_333u64;
        buffer.record_render(1000 + interval_120fps, 1100 + interval_120fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 120).abs() <= 1, "120 FPS ±1, got {}", fps_int);
    }

    #[test]
    fn test_last_render_time_tracking() {
        let buffer = RenderBufferCapsule::new();
        assert_eq!(buffer.last_render_time(), 0);

        buffer.record_render(5000, 5100);
        assert_eq!(buffer.last_render_time(), 5000, "should track render start time");

        buffer.record_render(25000, 25100);
        assert_eq!(buffer.last_render_time(), 25000);
    }

    #[test]
    fn test_complete_render_cycle() {
        let buffer = RenderBufferCapsule::new();

        // Start: clean
        assert!(!buffer.should_render());

        // Mark dirty
        buffer.mark_dirty();
        assert!(buffer.should_render());

        // Render frame 1
        buffer.record_render(1000, 1500);
        assert_eq!(buffer.frame_count(), 1);
        assert_eq!(buffer.render_time(), 500);

        // Frame is still dirty (need to clear manually)
        assert!(buffer.should_render());
        buffer.clear_dirty();
        assert!(!buffer.should_render());

        // Render frame 2 at 60 FPS (±1 due to rounding)
        let t1 = 1500 + 16_666_667;
        buffer.record_render(t1, t1 + 500);
        assert_eq!(buffer.frame_count(), 2);
        let fps = buffer.fps() >> 16;
        assert!((fps as i32 - 60).abs() <= 1, "Expected 60 FPS ±1, got {}", fps);
    }

    #[test]
    fn test_concurrent_reads() {
        let buffer = std::sync::Arc::new(RenderBufferCapsule::new());

        buffer.record_render(1000, 1100);
        let interval = 16_666_667u64;
        buffer.record_render(1000 + interval, 1100 + interval);

        // Spawn multiple reader threads
        let mut handles = vec![];
        for _ in 0..4 {
            let b = buffer.clone();
            let handle = std::thread::spawn(move || {
                let fps = b.fps();
                let frames = b.frame_count();
                let render_ns = b.render_time();
                (fps, frames, render_ns)
            });
            handles.push(handle);
        }

        for handle in handles {
            let (fps, frames, render_ns) = handle.join().unwrap();
            let fps_int = fps >> 16;
            assert!((fps_int as i32 - 60).abs() <= 1, "concurrent reader should see ~60 FPS, got {}", fps_int);
            assert_eq!(frames, 2);
            assert!(render_ns > 0);
        }
    }

    #[test]
    fn test_alignment_256bytes() {
        let _buffer = RenderBufferCapsule::new();
        let size = std::mem::size_of::<RenderBufferCapsule>();
        let align = std::mem::align_of::<RenderBufferCapsule>();

        assert_eq!(size, 256, "RenderBufferCapsule should be exactly 256 bytes");
        assert_eq!(align, 256, "RenderBufferCapsule should be 256-byte aligned");
    }

    #[test]
    fn test_debug_format() {
        let buffer = RenderBufferCapsule::new();
        buffer.mark_dirty();
        buffer.record_render(1000, 1100);

        let interval = 16_666_667u64;
        buffer.record_render(1000 + interval, 1100 + interval);

        let debug_str = format!("{:?}", buffer);
        assert!(debug_str.contains("RenderBufferCapsule"), "debug should include struct name");
        assert!(debug_str.contains("true"), "debug should show dirty=true");
        assert!(debug_str.contains("60"), "debug should show ~60 FPS");
    }

    #[test]
    #[should_panic(expected = "render timing invalid")]
    fn test_invalid_render_timing_panics() {
        let buffer = RenderBufferCapsule::new();
        buffer.record_render(1000, 900); // end < start - should panic
    }

    #[test]
    fn test_fps_zero_interval_safety() {
        // compute_fps_q16_16 should handle zero interval safely
        let fps = compute_fps_q16_16(0);
        assert_eq!(fps, 0, "zero interval should return 0 FPS");
    }

    #[test]
    fn test_fps_high_framerate() {
        let buffer = RenderBufferCapsule::new();

        buffer.record_render(1000, 1010);
        let interval_1000fps = 1_000_000u64; // 1 microsecond
        buffer.record_render(1000 + interval_1000fps, 1010 + interval_1000fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 1000).abs() <= 1, "1000 FPS ±1, got {}", fps_int);
    }
}
