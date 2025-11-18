//! RenderBufferCapsule integration for kindly_dedup CLI rendering system
//!
//! Provides global frame timing, dirty flag tracking, and 60 FPS control for the deduplication pipeline TUI.
//!
//! ## Architecture
//!
//! **Tier**: T1 Atomic (256B cache-aligned, lockfree)
//!
//! **Components**:
//! - Global `RenderBufferCapsule` for frame timing and dirty tracking
//! - Integration with animation scheduler for coordinated rendering
//! - FPS measurement and control (8-60 FPS configurable)
//!
//! ## Performance Targets
//! - Dirty flag check: <5ns (single atomic load)
//! - FPS calculation: <10ns (Q16.16 arithmetic)
//! - Frame timing: <100ns total overhead
//!
//! ## Usage
//!
//! ```ignore
//! use kindly_dedup::cli::render_buffer::get_render_buffer;
//! use std::time::{SystemTime, UNIX_EPOCH};
//!
//! let buffer = get_render_buffer();
//!
//! // Mark frame as dirty (state changed)
//! buffer.mark_dirty();
//!
//! // Check if rendering needed
//! if buffer.should_render() {
//!     let start = get_nanos();
//!     // ... render frame ...
//!     let end = get_nanos();
//!
//!     buffer.record_render(start, end);
//!     buffer.clear_dirty();
//! }
//!
//! // Get current FPS (Q16.16 format)
//! let fps_q16_16 = buffer.fps();
//! let fps_int = fps_q16_16 >> 16;
//! println!("Current FPS: {}", fps_int);
//! ```

use atomic_capsule::tui::RenderBufferCapsule;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Global render buffer instance (lazy-initialized, thread-safe)
static RENDER_BUFFER: OnceLock<Arc<RenderBufferCapsule>> = OnceLock::new();

/// Get the global render buffer capsule
///
/// Returns a reference to the shared, globally-accessible render buffer.
/// Thread-safe: Multiple threads can read/write simultaneously (SWeMR pattern).
///
/// # Performance
/// - First call: ~10μs (allocation + initialization)
/// - Subsequent calls: <1ns (OnceLock cached access)
///
/// # Examples
/// ```ignore
/// let buffer = get_render_buffer();
/// buffer.mark_dirty();
/// ```
#[inline]
pub fn get_render_buffer() -> Arc<RenderBufferCapsule> {
    RENDER_BUFFER
        .get_or_init(|| Arc::new(RenderBufferCapsule::new()))
        .clone()
}

/// Get current time as nanoseconds since UNIX_EPOCH
///
/// Used for precise frame timing measurement.
///
/// # Panics
/// Panics if system time cannot be determined (very rare).
///
/// # Performance
/// ~20ns per call (SystemTime::now() overhead)
#[inline]
pub fn get_nanos() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time has gone backwards")
        .as_nanos() as u64
}

/// Mark the current frame as needing rendering
///
/// Should be called whenever state changes (document processed, metrics updated, etc.)
///
/// # Performance
/// <5ns (single atomic store, relaxed semantics)
///
/// # Examples
/// ```ignore
/// document_processed();
/// mark_frame_dirty();  // Trigger re-render
/// ```
#[inline]
pub fn mark_frame_dirty() {
    get_render_buffer().mark_dirty();
}

/// Check if frame should be rendered
///
/// Returns true if dirty flag is set (frame needs rendering).
/// Call after checking FPS throttle.
///
/// # Performance
/// <5ns (single atomic load, relaxed semantics)
///
/// # Examples
/// ```ignore
/// if should_render_frame() {
///     // Perform render pass
/// }
/// ```
#[inline]
pub fn should_render_frame() -> bool {
    get_render_buffer().should_render()
}

/// Record a completed render pass
///
/// Updates frame counter and estimates current FPS from inter-frame time.
/// Call after rendering completes to update timing metrics.
///
/// # Arguments
/// - `start_ns`: Render start time (nanoseconds since UNIX_EPOCH)
/// - `end_ns`: Render end time (nanoseconds since UNIX_EPOCH)
///
/// # Performance
/// <100ns (multiple atomic stores with release semantics)
///
/// # Panics
/// Panics if `end_ns < start_ns` (invalid timing).
///
/// # Examples
/// ```ignore
/// let start = get_nanos();
/// render_frame();
/// let end = get_nanos();
/// record_render_time(start, end);
/// ```
#[inline]
pub fn record_render_time(start_ns: u64, end_ns: u64) {
    get_render_buffer().record_render(start_ns, end_ns);
}

/// Clear the dirty flag (mark frame as clean)
///
/// Call after rendering to prevent unnecessary re-renders.
///
/// # Performance
/// <5ns (single atomic store, relaxed semantics)
///
/// # Examples
/// ```ignore
/// if should_render_frame() {
///     render_frame();
///     clear_frame_dirty();
/// }
/// ```
#[inline]
pub fn clear_frame_dirty() {
    get_render_buffer().clear_dirty();
}

/// Get the duration of the last render pass (nanoseconds)
///
/// # Performance
/// <5ns (single atomic load, relaxed semantics)
///
/// # Returns
/// Duration in nanoseconds (0 if no render has occurred)
///
/// # Examples
/// ```ignore
/// let render_ns = get_last_render_time();
/// if render_ns > 100_000 {  // > 100µs
///     eprintln!("Slow render: {}ns", render_ns);
/// }
/// ```
#[inline]
pub fn get_last_render_time() -> u64 {
    get_render_buffer().render_time()
}

/// Get the total number of frames rendered
///
/// # Performance
/// <5ns (single atomic load, relaxed semantics)
///
/// # Returns
/// Frame counter (wraps at u64::MAX after ~584 billion frames)
///
/// # Examples
/// ```ignore
/// let frame_num = get_frame_count();
/// println!("Total frames: {}", frame_num);
/// ```
#[inline]
pub fn get_frame_count() -> u64 {
    get_render_buffer().frame_count()
}

/// Get the current FPS estimate (Q16.16 fixed-point format)
///
/// FPS is estimated from the interval between consecutive render calls.
///
/// # Performance
/// <5ns (single atomic load, acquire semantics)
///
/// # Returns
/// FPS in Q16.16 fixed-point format:
/// - Integer part: `fps >> 16` (upper 16 bits, 0-65535)
/// - Fractional part: `(fps & 0xFFFF) / 65536.0` (lower 16 bits)
/// - Example: 60 FPS = 3932160 (0x3C0000)
///
/// # Examples
/// ```ignore
/// let fps_q16_16 = get_current_fps();
/// let fps_int = fps_q16_16 >> 16;
/// let fps_frac = ((fps_q16_16 & 0xFFFF) as f32) / 65536.0;
/// println!("FPS: {}.{:05}", fps_int, (fps_frac * 100000.0) as u32);
/// ```
#[inline]
pub fn get_current_fps() -> u32 {
    get_render_buffer().fps()
}

/// Get the current FPS as a floating-point value
///
/// Convenience function that converts Q16.16 format to f64.
///
/// # Performance
/// ~10ns (atomic load + conversion)
///
/// # Returns
/// FPS as f64 (e.g., 60.0 for 60 FPS)
///
/// # Examples
/// ```ignore
/// let fps = get_current_fps_float();
/// if fps < 50.0 {
///     eprintln!("Low FPS: {:.1}", fps);
/// }
/// ```
#[inline]
pub fn get_current_fps_float() -> f64 {
    let fps_q16_16 = get_current_fps();
    let int_part = (fps_q16_16 >> 16) as f64;
    let frac_part = ((fps_q16_16 & 0xFFFF) as f64) / 65536.0;
    int_part + frac_part
}

/// Get the timestamp of the last render (nanoseconds since UNIX_EPOCH)
///
/// # Performance
/// <5ns (single atomic load, relaxed semantics)
///
/// # Returns
/// Last render timestamp (0 if no render has occurred)
///
/// # Examples
/// ```ignore
/// let last_ns = get_last_render_timestamp();
/// let elapsed = get_nanos() - last_ns;
/// println!("Time since last render: {}ns", elapsed);
/// ```
#[inline]
pub fn get_last_render_timestamp() -> u64 {
    get_render_buffer().last_render_time()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_buffer_singleton() {
        let buf1 = get_render_buffer();
        let buf2 = get_render_buffer();

        // Should be the same Arc (singleton pattern)
        assert_eq!(
            Arc::as_ptr(&buf1),
            Arc::as_ptr(&buf2),
            "buffers should be the same singleton"
        );
    }

    #[test]
    fn test_dirty_flag_tracking() {
        let buffer = get_render_buffer();

        // Start clean
        buffer.clear_dirty();
        assert!(!should_render_frame(), "initially clean");

        // Mark dirty
        mark_frame_dirty();
        assert!(should_render_frame(), "should be dirty after mark");

        // Clear dirty
        clear_frame_dirty();
        assert!(!should_render_frame(), "should be clean after clear");
    }

    #[test]
    fn test_frame_counter() {
        let buffer = get_render_buffer();
        let initial_count = buffer.frame_count();

        buffer.record_render(1000, 1100);
        assert_eq!(buffer.frame_count(), initial_count + 1, "should increment");

        buffer.record_render(20000, 20100);
        assert_eq!(buffer.frame_count(), initial_count + 2);
    }

    #[test]
    fn test_render_time_measurement() {
        let buffer = get_render_buffer();

        buffer.record_render(1000, 1500);
        assert_eq!(buffer.render_time(), 500, "should record duration");

        buffer.record_render(2000, 2750);
        assert_eq!(buffer.render_time(), 750);
    }

    #[test]
    fn test_fps_calculation() {
        let buffer = Arc::new(RenderBufferCapsule::new());

        // First render: no FPS
        buffer.record_render(1000, 1100);
        assert_eq!(buffer.fps(), 0, "first render has no FPS");

        // Second render at ~60 FPS interval (16.67ms)
        let interval_60fps = 16_666_667u64;
        buffer.record_render(1000 + interval_60fps, 1100 + interval_60fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 60).abs() <= 1, "should be ~60 FPS, got {}", fps_int);
    }

    #[test]
    fn test_fps_float_conversion() {
        let buffer = Arc::new(RenderBufferCapsule::new());

        buffer.record_render(1000, 1100);
        let interval_60fps = 16_666_667u64;
        buffer.record_render(1000 + interval_60fps, 1100 + interval_60fps);

        let fps_q16_16 = buffer.fps();
        let int_part = (fps_q16_16 >> 16) as f64;
        let frac_part = ((fps_q16_16 & 0xFFFF) as f64) / 65536.0;
        let fps_float = int_part + frac_part;

        assert!((fps_float - 60.0).abs() < 1.0, "should be ~60 FPS float");
    }

    #[test]
    fn test_timestamp_tracking() {
        let buffer = Arc::new(RenderBufferCapsule::new());

        buffer.record_render(5000, 5100);
        assert_eq!(buffer.last_render_time(), 5000);

        buffer.record_render(25000, 25100);
        assert_eq!(buffer.last_render_time(), 25000);
    }

    #[test]
    fn test_helper_functions() {
        let buffer = get_render_buffer();

        // Test mark_frame_dirty
        buffer.clear_dirty();
        mark_frame_dirty();
        assert!(should_render_frame());

        // Test record_render_time
        let start = 1000;
        let end = 1500;
        record_render_time(start, end);
        assert_eq!(get_last_render_time(), 500);

        // Test clear_frame_dirty
        clear_frame_dirty();
        assert!(!should_render_frame());

        // Test getters
        let frame_count = get_frame_count();
        assert!(frame_count > 0);

        let fps = get_current_fps();
        let fps_float = get_current_fps_float();
        // Both should be consistent
        let fps_int_from_q16 = (fps >> 16) as f64;
        assert!((fps_float - fps_int_from_q16).abs() < 1.0);
    }

    #[test]
    fn test_concurrent_reads() {
        let buffer = Arc::new(RenderBufferCapsule::new());

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
            assert!((fps_int as i32 - 60).abs() <= 1);
            assert_eq!(frames, 2);
            assert!(render_ns > 0);
        }
    }

    #[test]
    fn test_complete_render_cycle() {
        let buffer = Arc::new(RenderBufferCapsule::new());

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

        // Render frame 2 at 60 FPS
        let t1 = 1500 + 16_666_667;
        buffer.record_render(t1, t1 + 500);
        assert_eq!(buffer.frame_count(), 2);
        let fps = buffer.fps() >> 16;
        assert!((fps as i32 - 60).abs() <= 1);
    }

    #[test]
    fn test_30_fps_interval() {
        let buffer = Arc::new(RenderBufferCapsule::new());

        buffer.record_render(1000, 1100);
        let interval_30fps = 33_333_333u64;
        buffer.record_render(1000 + interval_30fps, 1100 + interval_30fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 30).abs() <= 1, "30 FPS ±1, got {}", fps_int);
    }

    #[test]
    fn test_120_fps_interval() {
        let buffer = Arc::new(RenderBufferCapsule::new());

        buffer.record_render(1000, 1100);
        let interval_120fps = 8_333_333u64;
        buffer.record_render(1000 + interval_120fps, 1100 + interval_120fps);

        let fps_q16_16 = buffer.fps();
        let fps_int = fps_q16_16 >> 16;

        assert!((fps_int as i32 - 120).abs() <= 1, "120 FPS ±1, got {}", fps_int);
    }
}
