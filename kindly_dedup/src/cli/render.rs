//! Screen rendering utilities for CLI TUI
//!
//! Provides rendering functions for the Byzantine Purple/Gold themed interface.
//! Uses box_drawing module and Colorize trait from terminal utilities.
//!
//! ## RenderBufferCapsule Integration
//!
//! This module integrates with the global `RenderBufferCapsule` for frame timing,
//! dirty flag tracking, and 60 FPS control. All rendering functions work with the
//! frame scheduling system.
//!
//! **Tier**: T1 Atomic (lockfree, <100ns per operation)
//!
//! **Usage**:
//! ```ignore
//! use kindly_dedup::cli::render_buffer::*;
//!
//! // Mark frame dirty when state changes
//! mark_frame_dirty();
//!
//! // Check if rendering needed (FPS control)
//! if should_render_frame() {
//!     let start = get_nanos();
//!
//!     // Render UI components
//!     println!("{}", render_progress_bar(75, 40));
//!
//!     let end = get_nanos();
//!     record_render_time(start, end);
//!     clear_frame_dirty();
//! }
//!
//! // Monitor performance
//! let fps = get_current_fps_float();
//! let frame_count = get_frame_count();
//! ```

use crate::cli::render_buffer::{
    clear_frame_dirty, get_current_fps_float, get_frame_count, get_nanos, mark_frame_dirty, record_render_time,
    should_render_frame,
};
use crate::utils::terminal::{
    box_drawing, emoji, format_duration, format_number, format_size, format_timestamp, Color, Colorize,
};

/// Render a progress bar (percent complete, width in chars)
///
/// Returns a colored progress bar with percentage.
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_progress_bar;
///
/// let bar = render_progress_bar(75, 40);
/// println!("{}", bar);
/// // Output: [████████████████████░░░░░░░░░░░░░░░░░░░░░░] 75%
/// ```
#[inline]
pub fn render_progress_bar(percent: u8, width: usize) -> String {
    let filled = (percent as usize * width / 100).min(width);
    let percent = percent.min(100);

    let mut bar = String::from("[");

    // Filled portion (gold)
    for _ in 0..filled {
        bar.push_str(&"█".byzantine_gold());
    }

    // Empty portion (dim)
    for _ in filled..width {
        bar.push_str(&"░".dim());
    }

    bar.push_str(&format!("] {}%", percent));
    bar
}

/// Render metric display with value and label
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_metric;
///
/// let metric = render_metric("Throughput", "373K docs/sec", Color::ByzantineGold);
/// println!("{}", metric);
/// // Output: Throughput: 373K docs/sec (in gold)
/// ```
#[inline]
pub fn render_metric(label: &str, value: &str, color: Color) -> String {
    format!("{}: {}", label, value.color(color))
}

/// Render status indicator with emoji
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_status;
/// use kindly_dedup::utils::terminal::emoji;
///
/// let status = render_status("Processing", emoji::status::PENDING);
/// println!("{}", status);
/// // Output: ⏳ Processing
/// ```
#[inline]
pub fn render_status(text: &str, emoji_char: &str) -> String {
    format!("{} {}", emoji_char, text)
}

/// Render section header with Byzantine branding
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_header;
///
/// let header = render_header("Deduplication Pipeline", 50);
/// println!("{}", header);
/// ```
#[inline]
pub fn render_header(title: &str, width: usize) -> String {
    let padded_title = format!(" {} ", title.byzantine_purple().bold());
    let padding_left = (width.saturating_sub(padded_title.len())) / 2;
    let padding_right = width.saturating_sub(padded_title.len()) - padding_left;

    format!(
        "{}{}{}{}{}",
        box_drawing::HEAVY_LEFT_TEE,
        " ".repeat(padding_left),
        padded_title,
        " ".repeat(padding_right),
        box_drawing::HEAVY_RIGHT_TEE
    )
}

/// Render performance metrics summary
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_metrics_summary;
///
/// let summary = render_metrics_summary(
///     373_000,  // throughput (docs/sec)
///     2.7,      // latency (µs)
///     85,       // percent complete
///     150,      // estimated seconds remaining
/// );
/// ```
#[inline]
pub fn render_metrics_summary(throughput: u64, latency_us: f64, percent: u8, eta_secs: u64) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "  {} Throughput: {}\n",
        emoji::performance::ROCKET,
        format_number(throughput).byzantine_gold()
    ));

    output.push_str(&format!("  {} Latency: {:.2}µs\n", emoji::time::TIMER, latency_us));

    output.push_str(&format!(
        "  {} Progress: {}\n",
        emoji::data::CHART,
        render_progress_bar(percent, 30)
    ));

    output.push_str(&format!(
        "  {} ETA: {}\n",
        emoji::time::ALARM,
        format_duration(eta_secs as f64)
    ));

    output
}

/// Render a simple menu (for non-interactive display)
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_menu;
///
/// let menu = render_menu(&[
///     ("1", "Start Deduplication"),
///     ("2", "Load Corpus"),
///     ("3", "Settings"),
///     ("q", "Quit"),
/// ], Some(0));
/// ```
#[inline]
pub fn render_menu(items: &[(&str, &str)], selected: Option<usize>) -> String {
    let mut output = String::new();

    for (idx, (key, label)) in items.iter().enumerate() {
        let marker = if selected == Some(idx) {
            format!("{}→", emoji::arrows::RIGHT)
        } else {
            "  ".to_string()
        };

        let item = if selected == Some(idx) {
            format!("{} [{}] {}", marker, key.byzantine_gold().bold(), label)
        } else {
            format!("{} [{}] {}", marker, key, label)
        };

        output.push_str(&item);
        output.push('\n');
    }

    output
}

/// Render system resource display
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_resources;
///
/// let resources = render_resources(
///     8,        // cores
///     16,       // total GB RAM
///     8,        // used GB RAM
///     "/tmp/dedup.mmap",  // working directory
/// );
/// ```
#[inline]
pub fn render_resources(cores: u8, total_gb: u64, used_gb: u64, workdir: &str) -> String {
    let mut output = String::new();

    output.push_str(&format!(
        "  {} CPU: {} cores\n",
        emoji::tech::GEAR,
        cores.to_string().byzantine_purple()
    ));

    let percent = if total_gb > 0 {
        ((used_gb as f64 / total_gb as f64) * 100.0) as u8
    } else {
        0
    };

    output.push_str(&format!(
        "  {} Memory: {} / {} GB {}\n",
        emoji::tools::TOOLBOX,
        used_gb.to_string().byzantine_purple(),
        total_gb,
        render_progress_bar(percent, 20)
    ));

    output.push_str(&format!("  {} Working: {}\n", emoji::tools::WRENCH, workdir.dim()));

    output
}

/// Render a simple box border (non-interactive)
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_box;
///
/// let box_output = render_box("Status", 60, 5);
/// println!("{}", box_output);
/// ```
#[inline]
pub fn render_box(title: &str, width: usize, content_lines: usize) -> String {
    box_drawing::draw_heavy_box(width, content_lines, Some(title))
}

/// Render performance metrics from RenderBufferCapsule
///
/// Displays current FPS and frame count from the global render buffer.
/// Call this after recording render times to show performance data.
///
/// ## Example
/// ```rust
/// use kindly_dedup::cli::render::render_frame_metrics;
///
/// let metrics = render_frame_metrics();
/// println!("{}", metrics);
/// // Output: 🎬 60.0 FPS | Frames: 1234
/// ```
#[inline]
pub fn render_frame_metrics() -> String {
    let fps = get_current_fps_float();
    let frames = get_frame_count();

    format!(
        "  {} FPS: {:.1} | {} Frames: {}",
        emoji::performance::ROCKET,
        fps,
        emoji::data::CHART,
        format_number(frames)
    )
}

/// Main rendering loop with FPS control and dirty flag tracking
///
/// Returns true if frame was rendered, false if skipped (FPS throttle).
/// Call this in the main UI loop to control rendering cadence.
///
/// # Arguments
/// - `render_fn`: Closure that performs the actual rendering
///
/// # Performance
/// <100ns total overhead for frame scheduling
///
/// # Example
/// ```ignore
/// use kindly_dedup::cli::render::render_frame;
///
/// loop {
///     if render_frame(|| {
///         println!("{}", render_progress_bar(50, 40));
///         println!("{}", render_frame_metrics());
///     }) {
///         // Frame was rendered
///     } else {
///         // Frame skipped (FPS throttle)
///     }
/// }
/// ```
#[inline]
pub fn render_frame<F: FnOnce()>(render_fn: F) -> bool {
    if should_render_frame() {
        let start = get_nanos();

        // Perform rendering
        render_fn();

        let end = get_nanos();

        // Record timing and mark clean
        record_render_time(start, end);
        clear_frame_dirty();

        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar() {
        let bar = render_progress_bar(50, 20);
        assert!(bar.contains("50%"));
    }

    #[test]
    fn test_render_metric() {
        let metric = render_metric("Test", "123", Color::Red);
        assert!(metric.contains("Test"));
        assert!(metric.contains("123"));
    }

    #[test]
    fn test_render_status() {
        let status = render_status("Running", "🚀");
        assert!(status.contains("Running"));
        assert!(status.contains("🚀"));
    }

    #[test]
    fn test_render_header() {
        let header = render_header("Test", 40);
        assert!(header.contains("Test"));
    }

    #[test]
    fn test_render_menu() {
        let menu = render_menu(&[("1", "Option 1"), ("2", "Option 2")], Some(0));
        assert!(menu.contains("Option 1"));
        assert!(menu.contains("Option 2"));
    }

    // RenderBufferCapsule integration tests (12 total for integration suite)

    #[test]
    fn test_render_frame_metrics_format() {
        let metrics = render_frame_metrics();
        assert!(metrics.contains("FPS:"));
        assert!(metrics.contains("Frames:"));
    }

    #[test]
    fn test_render_frame_control_dirty_flag() {
        use crate::cli::render_buffer::{clear_frame_dirty, mark_frame_dirty};

        // Ensure clean state
        clear_frame_dirty();

        // Mark dirty and verify rendering happens
        mark_frame_dirty();
        let rendered = render_frame(|| {
            // No-op render function for test
        });

        assert!(rendered, "frame should render when dirty");

        // After render, should be clean
        let rendered_again = render_frame(|| {
            // No-op render function
        });

        // Should be clean now (skipped)
        assert!(!rendered_again, "frame should be skipped when clean");
    }

    #[test]
    fn test_render_frame_timing() {
        use crate::cli::render_buffer::{get_last_render_time, mark_frame_dirty};

        mark_frame_dirty();
        render_frame(|| {
            // Simulate some work
            for _ in 0..100 {
                std::hint::black_box(42);
            }
        });

        let render_time = get_last_render_time();
        assert!(render_time > 0, "render time should be recorded");
    }

    #[test]
    fn test_render_frame_fps_calculation() {
        use crate::cli::render_buffer::{get_current_fps_float, mark_frame_dirty};

        mark_frame_dirty();

        // Render first frame
        let _ = render_frame(|| {});

        // FPS should still be 0 (only one frame so far)
        let fps = get_current_fps_float();
        assert_eq!(fps, 0.0, "FPS should be 0 on first frame");

        // Render second frame immediately
        mark_frame_dirty();
        let _ = render_frame(|| {});

        // FPS should now be very high (immediate second frame)
        let fps = get_current_fps_float();
        assert!(fps > 100.0, "FPS should be very high for back-to-back renders: {}", fps);
    }

    #[test]
    fn test_render_frame_counter_increment() {
        use crate::cli::render_buffer::{get_frame_count, mark_frame_dirty};

        let initial = get_frame_count();

        mark_frame_dirty();
        render_frame(|| {});

        let after_one = get_frame_count();
        assert_eq!(after_one, initial + 1, "frame counter should increment");

        mark_frame_dirty();
        render_frame(|| {});

        let after_two = get_frame_count();
        assert_eq!(after_two, initial + 2, "frame counter should increment again");
    }

    #[test]
    fn test_render_frame_dirty_flag_cleanup() {
        use crate::cli::render_buffer::{mark_frame_dirty, should_render_frame};

        mark_frame_dirty();
        assert!(should_render_frame(), "should be dirty after mark");

        // Render should clean the flag
        render_frame(|| {});

        assert!(!should_render_frame(), "should be clean after render_frame");
    }

    #[test]
    fn test_render_frame_with_closure() {
        use crate::cli::render_buffer::mark_frame_dirty;

        let mut render_called = false;

        mark_frame_dirty();
        render_frame(|| {
            render_called = true;
        });

        assert!(render_called, "render closure should have been called");
    }

    #[test]
    fn test_render_frame_multiple_cycles() {
        use crate::cli::render_buffer::{get_frame_count, mark_frame_dirty};

        let start = get_frame_count();

        for i in 0..5 {
            mark_frame_dirty();
            let rendered = render_frame(|| {});

            assert!(rendered, "frame {} should render", i);
            assert_eq!(
                get_frame_count(),
                start + i as u64 + 1,
                "frame counter should be consistent"
            );
        }
    }

    #[test]
    fn test_render_metrics_with_fps() {
        use crate::cli::render_buffer::{get_current_fps_float, mark_frame_dirty};

        let initial_fps = get_current_fps_float();

        // Render a couple frames
        for _ in 0..3 {
            mark_frame_dirty();
            render_frame(|| {});
        }

        let metrics = render_frame_metrics();

        // Should contain FPS info (even if 0)
        assert!(metrics.contains("FPS:"), "metrics should contain FPS: {}", metrics);

        // Verify FPS is present
        let fps = get_current_fps_float();
        assert!(
            fps >= initial_fps,
            "FPS should be consistent or increase: {} >= {}",
            fps,
            initial_fps
        );
    }
}
