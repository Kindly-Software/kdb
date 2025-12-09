//! Kindly-AV1 Branding Module
//!
//! [TRADE SECRET] - Proprietary brand identity for Kindly-AV1.
//!
//! This module defines the visual identity of the Kindly-AV1 encoder,
//! including colors, emojis, and formatted output functions.
//!
//! # Brand Guidelines
//!
//! - Primary color: Byzantine Royal Purple (#9932CC / ANSI 129)
//! - Accent color: Gold (#FFD700 / ANSI 220)
//! - Emojis: Purple heart, sparkles for positive; cross for errors
//! - Tone: Professional yet friendly, "kindly" in spirit

use std::io::{self, Write};

// ============================================================================
// ANSI Color Constants
// ============================================================================

/// Primary brand color - Byzantine Royal Purple
pub const PURPLE: &str = "\x1b[38;5;129m";

/// Accent color - Gold for highlights
pub const GOLD: &str = "\x1b[38;5;220m";

/// Secondary purple - lighter shade for progress bars
pub const LIGHT_PURPLE: &str = "\x1b[38;5;141m";

/// Success green
pub const GREEN: &str = "\x1b[38;5;46m";

/// Error red
pub const RED: &str = "\x1b[38;5;196m";

/// Warning yellow
pub const YELLOW: &str = "\x1b[38;5;226m";

/// Dim text for less important info
pub const DIM: &str = "\x1b[2m";

/// Bold text for emphasis
pub const BOLD: &str = "\x1b[1m";

/// Reset all formatting
pub const RESET: &str = "\x1b[0m";

/// Underline for links/paths
pub const UNDERLINE: &str = "\x1b[4m";

// ============================================================================
// Brand Emojis
// ============================================================================

/// Purple heart - primary brand emoji
pub const HEART: &str = "\u{1F49C}";

/// Sparkles - success/magic
pub const SPARK: &str = "\u{2728}";

/// Film camera - encoding
pub const FILM: &str = "\u{1F3AC}";

/// Rocket - performance/speed
pub const ROCKET: &str = "\u{1F680}";

/// Check mark - success
pub const CHECK: &str = "\u{2705}";

/// Cross mark - error
pub const CROSS: &str = "\u{274C}";

/// Chart - info/stats
pub const INFO: &str = "\u{1F4CA}";

/// Question mark - help
pub const HELP: &str = "\u{2753}";

/// Gear - processing
pub const GEAR: &str = "\u{2699}\u{FE0F}";

/// Clock - timing
pub const CLOCK: &str = "\u{23F1}\u{FE0F}";

/// Lightning - GPU acceleration
pub const LIGHTNING: &str = "\u{26A1}";

/// Folder - file operations
pub const FOLDER: &str = "\u{1F4C1}";

// ============================================================================
// Product Information
// ============================================================================

/// Product version from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Product name
pub const NAME: &str = "Kindly-AV1";

/// Product tagline
pub const TAGLINE: &str = "GPU-Accelerated AV1 Encoder";

/// Copyright notice
pub const COPYRIGHT: &str = "Kindly Technologies";

// ============================================================================
// Spinner Animation
// ============================================================================

/// Braille spinner animation frames for smooth rotation
pub const SPINNER: &[&str] = &[
    "\u{280B}", // ⠋
    "\u{2819}", // ⠙
    "\u{2839}", // ⠹
    "\u{2838}", // ⠸
    "\u{283C}", // ⠼
    "\u{2834}", // ⠴
    "\u{2826}", // ⠦
    "\u{2827}", // ⠧
    "\u{2807}", // ⠇
    "\u{280F}", // ⠏
];

/// Progress bar characters
pub const BAR_FULL: &str = "\u{2588}";   // █
pub const BAR_EMPTY: &str = "\u{2591}";  // ░
pub const BAR_HALF: &str = "\u{2592}";   // ▒

// ============================================================================
// Display Functions
// ============================================================================

/// Global color state (thread-local to avoid mutex)
#[derive(Clone, Copy)]
pub struct ColorConfig {
    /// Whether colors are enabled
    pub enabled: bool,
}

impl Default for ColorConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Get color code if colors are enabled
#[inline]
fn color<'a>(config: &ColorConfig, code: &'a str) -> &'a str {
    if config.enabled { code } else { "" }
}

/// Print the branded header
///
/// Displays:
/// ```text
/// 💜✨ Kindly-AV1 v1.0.0
///    GPU-Accelerated AV1 Encoder
/// ```
pub fn print_header() {
    print_header_with_config(&ColorConfig::default());
}

/// Print header with custom color configuration
pub fn print_header_with_config(config: &ColorConfig) {
    let purple = color(config, PURPLE);
    let gold = color(config, GOLD);
    let bold = color(config, BOLD);
    let dim = color(config, DIM);
    let reset = color(config, RESET);

    println!(
        "{}{} {} {}{}{} v{}{}",
        purple, HEART, SPARK, bold, NAME, reset, VERSION, reset
    );
    println!(
        "{}   {}{}{}",
        dim, gold, TAGLINE, reset
    );
    println!();
}

/// Print a branded success message
///
/// # Example
/// ```text
/// ✅ Encoding complete! Output: video.av1
/// ```
pub fn print_success(msg: &str) {
    print_success_with_config(msg, &ColorConfig::default());
}

/// Print success with custom color configuration
pub fn print_success_with_config(msg: &str, config: &ColorConfig) {
    let green = color(config, GREEN);
    let reset = color(config, RESET);

    println!("{}{} {}{}", green, CHECK, msg, reset);
}

/// Print a branded error message
///
/// # Example
/// ```text
/// ❌ Error: File not found: video.mp4
/// ```
pub fn print_error(msg: &str) {
    print_error_with_config(msg, &ColorConfig::default());
}

/// Print error with custom color configuration
pub fn print_error_with_config(msg: &str, config: &ColorConfig) {
    let red = color(config, RED);
    let reset = color(config, RESET);

    eprintln!("{}{} {}{}", red, CROSS, msg, reset);
}

/// Print a branded warning message
///
/// # Example
/// ```text
/// ⚠️ Warning: GPU not detected, using CPU fallback
/// ```
pub fn print_warning(msg: &str) {
    print_warning_with_config(msg, &ColorConfig::default());
}

/// Print warning with custom color configuration
pub fn print_warning_with_config(msg: &str, config: &ColorConfig) {
    let yellow = color(config, YELLOW);
    let reset = color(config, RESET);

    eprintln!("{}\u{26A0}\u{FE0F} Warning: {}{}", yellow, msg, reset);
}

/// Print a branded info message
///
/// # Example
/// ```text
/// 📊 Video: 1920x1080, 60fps, 10:32 duration
/// ```
pub fn print_info(msg: &str) {
    print_info_with_config(msg, &ColorConfig::default());
}

/// Print info with custom color configuration
pub fn print_info_with_config(msg: &str, config: &ColorConfig) {
    let purple = color(config, LIGHT_PURPLE);
    let reset = color(config, RESET);

    println!("{}{} {}{}", purple, INFO, msg, reset);
}

/// Print encoding progress with styled progress bar
///
/// # Arguments
/// * `current` - Current frame number
/// * `total` - Total frame count
/// * `fps` - Current encoding frames per second
/// * `eta_secs` - Estimated time remaining in seconds
///
/// # Display
/// ```text
/// 🎬 Encoding: video.mp4
/// [████████████░░░░░░░░] 60% | 45 fps | ETA: 2m 30s
/// ```
pub fn print_progress(current: u64, total: u64, fps: f64, eta_secs: u64) {
    print_progress_with_config(current, total, fps, eta_secs, &ColorConfig::default());
}

/// Print progress with custom color configuration
pub fn print_progress_with_config(
    current: u64,
    total: u64,
    fps: f64,
    eta_secs: u64,
    config: &ColorConfig,
) {
    let purple = color(config, PURPLE);
    let light = color(config, LIGHT_PURPLE);
    let dim = color(config, DIM);
    let reset = color(config, RESET);

    // Calculate percentage and bar width
    let percentage = if total > 0 {
        ((current as f64 / total as f64) * 100.0).min(100.0) as u8
    } else {
        0
    };

    const BAR_WIDTH: usize = 30;
    let filled = (percentage as usize * BAR_WIDTH) / 100;
    let empty = BAR_WIDTH - filled;

    // Format ETA
    let eta_str = format_duration(eta_secs);

    // Build progress bar
    let bar: String = std::iter::repeat(BAR_FULL)
        .take(filled)
        .chain(std::iter::repeat(BAR_EMPTY).take(empty))
        .collect();

    // Print with carriage return for in-place updates
    print!(
        "\r{}[{}{}{}{}] {}{}%{} | {}{:.1} fps{} | {}ETA: {}{}",
        purple,
        light,
        bar,
        purple,
        reset,
        purple,
        percentage,
        reset,
        dim,
        fps,
        reset,
        dim,
        eta_str,
        reset
    );

    // Flush to ensure immediate display
    let _ = io::stdout().flush();
}

/// Print progress with filename header
pub fn print_progress_with_file(
    filename: &str,
    current: u64,
    total: u64,
    fps: f64,
    eta_secs: u64,
) {
    print_progress_with_file_config(filename, current, total, fps, eta_secs, &ColorConfig::default());
}

/// Print progress with filename and custom config
pub fn print_progress_with_file_config(
    filename: &str,
    current: u64,
    total: u64,
    fps: f64,
    eta_secs: u64,
    config: &ColorConfig,
) {
    let purple = color(config, PURPLE);
    let reset = color(config, RESET);

    // Clear line and print filename on first line
    print!("\x1b[2K"); // Clear line
    println!("{}{} Encoding: {}{}", purple, FILM, filename, reset);

    // Print progress bar
    print_progress_with_config(current, total, fps, eta_secs, config);
}

/// Format duration in human-readable format
///
/// # Examples
/// - 65 -> "1m 5s"
/// - 3661 -> "1h 1m 1s"
/// - 45 -> "45s"
fn format_duration(seconds: u64) -> String {
    if seconds >= 3600 {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        let secs = seconds % 60;
        format!("{}h {}m {}s", hours, mins, secs)
    } else if seconds >= 60 {
        let mins = seconds / 60;
        let secs = seconds % 60;
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", seconds)
    }
}

/// Print section divider
pub fn print_divider() {
    print_divider_with_config(&ColorConfig::default());
}

/// Print divider with custom config
pub fn print_divider_with_config(config: &ColorConfig) {
    let dim = color(config, DIM);
    let reset = color(config, RESET);

    println!("{}────────────────────────────────────────{}", dim, reset);
}

/// Print encoding summary after completion
pub fn print_summary(
    input_size: u64,
    output_size: u64,
    duration_secs: f64,
    avg_fps: f64,
) {
    print_summary_with_config(input_size, output_size, duration_secs, avg_fps, &ColorConfig::default());
}

/// Print summary with custom config
pub fn print_summary_with_config(
    input_size: u64,
    output_size: u64,
    duration_secs: f64,
    avg_fps: f64,
    config: &ColorConfig,
) {
    let purple = color(config, PURPLE);
    let gold = color(config, GOLD);
    let green = color(config, GREEN);
    let dim = color(config, DIM);
    let reset = color(config, RESET);

    println!();
    print_divider_with_config(config);
    println!(
        "{}{} {} Encoding Complete! {}{}",
        purple, HEART, SPARK, HEART, reset
    );
    print_divider_with_config(config);

    // Calculate compression ratio
    let ratio = if output_size > 0 {
        input_size as f64 / output_size as f64
    } else {
        0.0
    };

    // Format sizes
    let input_str = format_size(input_size);
    let output_str = format_size(output_size);

    println!(
        "{}Input:{}      {} {}({}){}",
        dim, reset, input_str, dim, format_size_bytes(input_size), reset
    );
    println!(
        "{}Output:{}     {} {}({}){}",
        dim, reset, output_str, dim, format_size_bytes(output_size), reset
    );
    println!(
        "{}Ratio:{}      {}{:.2}x{} compression",
        dim, reset, gold, ratio, reset
    );
    println!(
        "{}Time:{}       {}",
        dim, reset, format_duration(duration_secs as u64)
    );
    println!(
        "{}Avg FPS:{}    {}{:.1}{}",
        dim, reset, green, avg_fps, reset
    );
    print_divider_with_config(config);
}

/// Format file size in human-readable format
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format file size in bytes with comma separators
fn format_size_bytes(bytes: u64) -> String {
    let s = bytes.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect::<String>() + " bytes"
}

/// Get spinner frame for animation
///
/// # Arguments
/// * `tick` - Animation tick counter (will be wrapped)
///
/// # Returns
/// The appropriate spinner frame string
#[inline]
pub fn get_spinner_frame(tick: usize) -> &'static str {
    SPINNER[tick % SPINNER.len()]
}

/// Print spinner with message
pub fn print_spinner(tick: usize, msg: &str) {
    print_spinner_with_config(tick, msg, &ColorConfig::default());
}

/// Print spinner with custom config
pub fn print_spinner_with_config(tick: usize, msg: &str, config: &ColorConfig) {
    let purple = color(config, PURPLE);
    let reset = color(config, RESET);
    let frame = get_spinner_frame(tick);

    print!("\r{}{}{} {}", purple, frame, reset, msg);
    let _ = io::stdout().flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(0), "0s");
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(1048576), "1.00 MB");
        assert_eq!(format_size(1073741824), "1.00 GB");
    }

    #[test]
    fn test_spinner_frames() {
        assert_eq!(SPINNER.len(), 10);
        assert_eq!(get_spinner_frame(0), SPINNER[0]);
        assert_eq!(get_spinner_frame(10), SPINNER[0]); // Wraps
        assert_eq!(get_spinner_frame(15), SPINNER[5]);
    }

    #[test]
    fn test_color_config_disabled() {
        let config = ColorConfig { enabled: false };
        assert_eq!(color(&config, PURPLE), "");
        assert_eq!(color(&config, RESET), "");
    }

    #[test]
    fn test_color_config_enabled() {
        let config = ColorConfig { enabled: true };
        assert_eq!(color(&config, PURPLE), PURPLE);
        assert_eq!(color(&config, RESET), RESET);
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size_bytes(1000), "1,000 bytes");
        assert_eq!(format_size_bytes(1000000), "1,000,000 bytes");
        assert_eq!(format_size_bytes(123), "123 bytes");
    }
}
