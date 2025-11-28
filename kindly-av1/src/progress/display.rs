//! Progress display with Kindly branding
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Real-time encoding progress display with branded TUI output.
//!
//! ## Features
//!
//! - Animated spinner with braille characters
//! - Colored progress bar with Unicode blocks
//! - Real-time FPS, ETA, and compression stats
//! - Completion summary with detailed metrics
//!
//! ## Branding
//!
//! - Primary: Byzantine Royal Purple (#9B59B6)
//! - Accent: Gold (#F1C40F)
//! - Emojis: Purple heart, sparkles, film camera
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier (display state)
//! - **COCA**: Lockfree progress reading, stateless display

use super::capsule::{ProgressCapsule, ProgressSnapshot};
use crate::cli::branding::{
    self, BAR_EMPTY, BAR_FULL, BOLD, CHECK, CROSS, DIM, FILM, GEAR, GOLD, GREEN, HEART, INFO,
    LIGHT_PURPLE, LIGHTNING, PURPLE, RED, RESET, SPARK, SPINNER,
};
// NOTE: EncodingStats removed with encoder module - using placeholder for wizard tests
// use crate::encoder::EncodingStats;
use std::io::{self, Write};
use std::time::Instant;

/// Placeholder for EncodingStats (encoder module removed)
#[allow(dead_code)]
struct EncodingStats {
    frames_encoded: u64,
    duration_ms: u64,
    average_fps: f64,
    total_bytes: u64,
    compression_ratio: f64,
    quality_metrics: String,
    psnr_average: Option<f64>,
    ssim_average: Option<f64>,
    gpu_utilization: Option<f64>,
}

/// Display configuration
///
/// Controls appearance and update frequency of progress display.
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    /// Enable colored output (ANSI codes)
    pub color: bool,
    /// Progress bar width in characters
    pub bar_width: usize,
    /// Minimum time between display updates (milliseconds)
    pub update_interval_ms: u64,
    /// Show verbose statistics
    pub verbose: bool,
    /// Show animated spinner
    pub show_spinner: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            color: true,
            bar_width: 40,
            update_interval_ms: 100,
            verbose: false,
            show_spinner: true,
        }
    }
}

impl DisplayConfig {
    /// Create config for non-interactive (e.g., CI) environments
    pub fn non_interactive() -> Self {
        Self {
            color: false,
            bar_width: 30,
            update_interval_ms: 1000, // Less frequent updates
            verbose: false,
            show_spinner: false,
        }
    }

    /// Create config for verbose output
    pub fn verbose() -> Self {
        Self {
            verbose: true,
            ..Default::default()
        }
    }
}

/// Video information for display header
///
/// Contains metadata about the input video for header display.
#[derive(Debug, Clone, Default)]
pub struct VideoInfo {
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels
    pub height: u32,
    /// Frame rate (fps)
    pub frame_rate: f64,
    /// Total frame count
    pub frame_count: u64,
    /// Video duration in seconds
    pub duration_secs: f64,
    /// Input file size in bytes
    pub file_size: u64,
    /// Codec/format name
    pub codec: Option<String>,
    /// Bit depth (8, 10, 12)
    pub bit_depth: u8,
}

impl VideoInfo {
    /// Create VideoInfo from basic parameters
    pub fn new(width: u32, height: u32, frame_rate: f64, frame_count: u64) -> Self {
        let duration_secs = if frame_rate > 0.0 {
            frame_count as f64 / frame_rate
        } else {
            0.0
        };

        Self {
            width,
            height,
            frame_rate,
            frame_count,
            duration_secs,
            file_size: 0,
            codec: None,
            bit_depth: 8,
        }
    }

    /// Format resolution as string (e.g., "1920x1080")
    pub fn resolution(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    /// Format duration as human-readable string
    pub fn format_duration(&self) -> String {
        let total_secs = self.duration_secs as u64;
        let hours = total_secs / 3600;
        let minutes = (total_secs % 3600) / 60;
        let seconds = total_secs % 60;

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }
}

/// Progress display manager
///
/// Handles all terminal output for encoding progress.
/// Thread-safe when reading from shared ProgressCapsule.
pub struct ProgressDisplay {
    config: DisplayConfig,
    last_update: Instant,
    spinner_frame: usize,
    header_printed: bool,
}

impl ProgressDisplay {
    /// Create new display with configuration
    pub fn new(config: DisplayConfig) -> Self {
        Self {
            config,
            last_update: Instant::now(),
            spinner_frame: 0,
            header_printed: false,
        }
    }

    /// Check if colors are enabled
    #[inline]
    fn color_enabled(&self) -> bool {
        self.config.color
    }

    /// Get color code if colors are enabled
    #[inline]
    fn color<'a>(&self, code: &'a str) -> &'a str {
        if self.color_enabled() {
            code
        } else {
            ""
        }
    }

    /// Print encoding header with Kindly branding
    ///
    /// Displays:
    /// ```text
    /// 💜 Kindly-AV1 Encoder
    /// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
    ///
    /// 🎬 Input:  video.mp4
    /// ✨ Output: video.av1
    /// 📊 1920×1080 @ 24.00 fps, 1440 frames
    /// ```
    pub fn print_header(&mut self, input: &str, output: &str, info: &VideoInfo) {
        if self.header_printed {
            return;
        }
        self.header_printed = true;

        let purple = self.color(PURPLE);
        let gold = self.color(GOLD);
        let bold = self.color(BOLD);
        let dim = self.color(DIM);
        let reset = self.color(RESET);

        // Brand header
        println!();
        println!(
            "{}{}{} Kindly-AV1 Encoder{}",
            bold, purple, HEART, reset
        );
        println!(
            "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            dim, reset
        );
        println!();

        // File info
        println!(
            "{} Input:  {}{}{}{}",
            FILM, bold, input, reset, reset
        );
        println!(
            "{} Output: {}{}{}{}",
            SPARK, bold, output, reset, reset
        );

        // Video info
        let duration_str = info.format_duration();
        println!(
            "{} {}{}×{}{} @ {}{:.2}{} fps, {}{}{} frames ({})",
            INFO,
            gold,
            info.width,
            info.height,
            reset,
            gold,
            info.frame_rate,
            reset,
            bold,
            info.frame_count,
            reset,
            duration_str,
        );

        if self.config.verbose {
            if let Some(ref codec) = info.codec {
                println!(
                    "   {}Codec: {}, {}bit{}",
                    dim, codec, info.bit_depth, reset
                );
            }
            if info.file_size > 0 {
                println!(
                    "   {}Size: {}{}",
                    dim,
                    format_size(info.file_size),
                    reset
                );
            }
        }

        println!();
    }

    /// Update progress display (call periodically)
    ///
    /// Rate-limited by `update_interval_ms` to prevent flickering.
    /// Use force=true to bypass rate limiting.
    pub fn update(&mut self, progress: &ProgressCapsule) {
        self.update_with_force(progress, false);
    }

    /// Update progress display with optional force flag
    pub fn update_with_force(&mut self, progress: &ProgressCapsule, force: bool) {
        // Rate limit updates
        if !force {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_update);
            if elapsed.as_millis() < self.config.update_interval_ms as u128 {
                return;
            }
            self.last_update = now;
        }

        let snap = progress.snapshot();
        self.render_progress(&snap);
    }

    /// Update from snapshot
    pub fn update_snapshot(&mut self, snap: &ProgressSnapshot) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update);
        if elapsed.as_millis() < self.config.update_interval_ms as u128 {
            return;
        }
        self.last_update = now;
        self.render_progress(snap);
    }

    /// Render progress bar and stats
    fn render_progress(&mut self, snap: &ProgressSnapshot) {
        let purple = self.color(PURPLE);
        let light_purple = self.color(LIGHT_PURPLE);
        let gold = self.color(GOLD);
        let bold = self.color(BOLD);
        let dim = self.color(DIM);
        let reset = self.color(RESET);

        // Clear line
        print!("\r\x1b[K");

        if self.color_enabled() && self.config.show_spinner {
            // Animated spinner
            let spinner = SPINNER[self.spinner_frame % SPINNER.len()];
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            print!("{}{}{} ", purple, spinner, reset);
        }

        // Progress bar
        let pct = snap.progress;
        let filled = (pct * self.config.bar_width as f64) as usize;
        let empty = self.config.bar_width.saturating_sub(filled);

        print!("{}[", purple);
        print!(
            "{}{}",
            light_purple,
            BAR_FULL.repeat(filled)
        );
        print!(
            "{}{}{}",
            dim,
            BAR_EMPTY.repeat(empty),
            reset
        );
        print!("{}]{} ", purple, reset);

        // Percentage
        print!("{}{:>5.1}%{} ", bold, pct * 100.0, reset);

        // Stats separator
        print!("{}│{} ", gold, reset);

        // FPS
        print!("{:.1} fps ", snap.fps);

        // ETA
        print!("{}│{} ", gold, reset);
        print!("ETA: {} ", format_eta(snap.eta_seconds));

        // Compression ratio (if bytes written)
        if snap.compression_ratio > 0.0 {
            print!("{}│{} ", gold, reset);
            print!("{:.1}:1 ", snap.compression_ratio);
        }

        // Flush immediately
        let _ = io::stdout().flush();
    }

    /// Print completion summary
    ///
    /// Displays detailed statistics after encoding completes.
    /// The progress capsule is available for additional real-time data if needed.
    pub fn print_summary(&self, _progress: &ProgressCapsule, stats: &EncodingStats) {
        // Move to new line after progress bar
        println!();
        println!();

        let _purple = self.color(PURPLE); // Reserved for future branding
        let gold = self.color(GOLD);
        let green = self.color(GREEN);
        let bold = self.color(BOLD);
        let dim = self.color(DIM);
        let reset = self.color(RESET);

        // Success header
        println!(
            "{}{}{} Encoding Complete!{}",
            bold, green, CHECK, reset
        );
        println!(
            "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            dim, reset
        );

        // Statistics
        println!(
            "  {}Frames:{}      {}",
            bold, reset, stats.frames_encoded
        );
        println!(
            "  {}Time:{}        {}",
            bold,
            reset,
            format_duration(stats.duration_ms)
        );
        println!(
            "  {}Avg FPS:{}     {}{:.2}{}",
            bold, reset, green, stats.average_fps, reset
        );
        println!(
            "  {}Output:{}      {}",
            bold,
            reset,
            format_size(stats.total_bytes)
        );
        println!(
            "  {}Compression:{} {}{:.1}:1{}",
            bold, reset, gold, stats.compression_ratio, reset
        );

        // Quality metrics if available
        if let Some(psnr) = stats.psnr_average {
            println!(
                "  {}PSNR:{}        {:.2} dB",
                bold, reset, psnr
            );
        }
        if let Some(ssim) = stats.ssim_average {
            println!(
                "  {}SSIM:{}        {:.4}",
                bold, reset, ssim
            );
        }

        // GPU utilization if available
        if let Some(gpu) = stats.gpu_utilization {
            println!(
                "  {}GPU:{}         {} {:.1}%",
                bold, reset, LIGHTNING, gpu
            );
        }

        println!(
            "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            dim, reset
        );
        println!();
    }

    /// Print completion summary from progress capsule only
    pub fn print_summary_simple(&self, progress: &ProgressCapsule) {
        let snap = progress.snapshot();

        println!();
        println!();

        let green = self.color(GREEN);
        let gold = self.color(GOLD);
        let bold = self.color(BOLD);
        let dim = self.color(DIM);
        let reset = self.color(RESET);

        println!(
            "{}{}{} Encoding Complete!{}",
            bold, green, CHECK, reset
        );
        println!(
            "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            dim, reset
        );
        println!(
            "  {}Frames:{}      {}",
            bold, reset, snap.current
        );
        println!(
            "  {}Time:{}        {}",
            bold,
            reset,
            format_duration(snap.elapsed_ms)
        );
        println!(
            "  {}Avg FPS:{}     {}{:.2}{}",
            bold, reset, green, snap.fps, reset
        );
        println!(
            "  {}Output:{}      {}",
            bold,
            reset,
            format_size(snap.bytes_written)
        );
        if snap.compression_ratio > 0.0 {
            println!(
                "  {}Compression:{} {}{:.1}:1{}",
                bold, reset, gold, snap.compression_ratio, reset
            );
        }
        println!(
            "{}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━{}",
            dim, reset
        );
        println!();
    }

    /// Print error message with Kindly branding
    pub fn print_error(&self, error: &str) {
        let red = self.color(RED);
        let bold = self.color(BOLD);
        let reset = self.color(RESET);

        // Clear progress line if any
        print!("\r\x1b[K");

        eprintln!();
        eprintln!(
            "{}{}{} Error:{} {}",
            bold, red, CROSS, reset, error
        );
    }

    /// Print warning message
    pub fn print_warning(&self, warning: &str) {
        branding::print_warning(warning);
    }

    /// Print info message
    pub fn print_info(&self, msg: &str) {
        branding::print_info(msg);
    }

    /// Clear the progress line
    pub fn clear_line(&self) {
        print!("\r\x1b[K");
        let _ = io::stdout().flush();
    }

    /// Print a processing/status message with spinner
    pub fn print_status(&mut self, msg: &str) {
        let purple = self.color(PURPLE);
        let reset = self.color(RESET);

        print!("\r\x1b[K");

        if self.color_enabled() && self.config.show_spinner {
            let spinner = SPINNER[self.spinner_frame % SPINNER.len()];
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
            print!("{}{}{} {}", purple, spinner, reset, msg);
        } else {
            print!("{} {}", GEAR, msg);
        }

        let _ = io::stdout().flush();
    }

    /// Finalize display (ensure cursor on new line)
    pub fn finish(&self) {
        println!();
    }
}

impl Default for ProgressDisplay {
    fn default() -> Self {
        Self::new(DisplayConfig::default())
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Format ETA as human-readable string
///
/// # Examples
/// - 0 -> "calculating..."
/// - 45 -> "45s"
/// - 72 -> "1m 12s"
/// - 3672 -> "1h 1m"
fn format_eta(seconds: u64) -> String {
    if seconds == 0 {
        return "calculating...".to_string();
    }
    if seconds == u64::MAX {
        return "unknown".to_string();
    }
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
    }
}

/// Format duration in milliseconds as human-readable string
fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;
    format_eta(seconds)
}

/// Format file size as human-readable string
///
/// # Examples
/// - 500 -> "500 B"
/// - 1536 -> "1.5 KB"
/// - 5242880 -> "5.0 MB"
/// - 1073741824 -> "1.00 GB"
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_config_default() {
        let config = DisplayConfig::default();
        assert!(config.color);
        assert_eq!(config.bar_width, 40);
        assert_eq!(config.update_interval_ms, 100);
        assert!(!config.verbose);
        assert!(config.show_spinner);
    }

    #[test]
    fn test_display_config_non_interactive() {
        let config = DisplayConfig::non_interactive();
        assert!(!config.color);
        assert_eq!(config.update_interval_ms, 1000);
        assert!(!config.show_spinner);
    }

    #[test]
    fn test_video_info_new() {
        let info = VideoInfo::new(1920, 1080, 24.0, 1440);
        assert_eq!(info.width, 1920);
        assert_eq!(info.height, 1080);
        assert_eq!(info.frame_rate, 24.0);
        assert_eq!(info.frame_count, 1440);
        assert!((info.duration_secs - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_video_info_resolution() {
        let info = VideoInfo::new(1920, 1080, 24.0, 100);
        assert_eq!(info.resolution(), "1920x1080");
    }

    #[test]
    fn test_video_info_format_duration() {
        let mut info = VideoInfo::default();

        info.duration_secs = 45.0;
        assert_eq!(info.format_duration(), "0:45");

        info.duration_secs = 125.0;
        assert_eq!(info.format_duration(), "2:05");

        info.duration_secs = 3661.0;
        assert_eq!(info.format_duration(), "1:01:01");
    }

    #[test]
    fn test_format_eta() {
        assert_eq!(format_eta(0), "calculating...");
        assert_eq!(format_eta(45), "45s");
        assert_eq!(format_eta(72), "1m 12s");
        assert_eq!(format_eta(3672), "1h 1m");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "calculating...");
        assert_eq!(format_duration(45000), "45s");
        assert_eq!(format_duration(72000), "1m 12s");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn test_progress_display_creation() {
        let display = ProgressDisplay::new(DisplayConfig::default());
        assert!(display.color_enabled());
        assert!(!display.header_printed);
    }

    #[test]
    fn test_progress_display_color_disabled() {
        let config = DisplayConfig {
            color: false,
            ..Default::default()
        };
        let display = ProgressDisplay::new(config);
        assert!(!display.color_enabled());
        assert_eq!(display.color(PURPLE), "");
    }

    #[test]
    fn test_progress_display_color_enabled() {
        let display = ProgressDisplay::new(DisplayConfig::default());
        assert_eq!(display.color(PURPLE), PURPLE);
    }
}
