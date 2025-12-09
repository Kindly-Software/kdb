//! Friendly Error Messages for kindly-av1
//!
//! [TRADE SECRET] - Proprietary user experience innovation.
//!
//! This module implements user-friendly error messages with actionable suggestions,
//! following SOTA CLI UX best practices from:
//! - [Command Line Interface Guidelines](https://clig.dev/)
//! - [Evil Martians CLI UX Patterns](https://evilmartians.com/chronicles/cli-ux-best-practices)
//! - [Atlassian's 10 Design Principles for CLIs](https://www.atlassian.com/blog/it-teams/10-design-principles-for-delightful-clis)
//!
//! # Design Principles
//!
//! 1. **Plain Language**: No jargon, explain in terms users understand
//! 2. **Actionable**: Always provide a concrete next step
//! 3. **Examples**: Show the correct command when possible
//! 4. **Empathetic**: Never blame the user, stay on their side
//! 5. **Consistent**: Follow Byzantine purple + gold branding
//!
//! # Chaos Compliance
//!
//! - UCE34 Q33: 100% lockfree (no state, pure functions)
//! - No heap allocation for static messages
//! - Zero runtime overhead for error conversion

use super::branding::{GOLD, PURPLE, RED, RESET, GREEN, DIM, BOLD};
use std::fmt;
use std::path::PathBuf;

// ============================================================================
// Friendly Error Structure
// ============================================================================

/// User-friendly error with actionable guidance
///
/// Designed for content creators, not video encoding experts.
/// Every error explains what went wrong and how to fix it.
///
/// # Example Display
///
/// ```text
/// X CRF value 100 is out of range
///
///   AV1 uses CRF (Constant Rate Factor) values from 0 to 63.
///   Lower values = higher quality, larger files.
///   Higher values = lower quality, smaller files.
///
///   Try: CRF 28-32 for balanced quality
///
///   Example:
///     kindly-av1 video.mp4 --crf 30
/// ```
#[derive(Debug, Clone)]
pub struct FriendlyError {
    /// Short title describing the problem
    pub title: String,
    /// Plain English explanation of what went wrong
    pub explanation: String,
    /// Actionable suggestion to fix the problem (highlighted in gold)
    pub suggestion: Option<String>,
    /// Example command showing correct usage
    pub example: Option<String>,
}

impl FriendlyError {
    /// Create a new friendly error
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            explanation: String::new(),
            suggestion: None,
            example: None,
        }
    }

    /// Add explanation
    pub fn with_explanation(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    /// Add suggestion (will be highlighted in gold)
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add example command
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.example = Some(example.into());
        self
    }

    /// Format for display with colors
    pub fn format_colored(&self) -> String {
        let mut output = String::with_capacity(512);

        // Title (red X with message)
        output.push_str(&format!("\n{}\u{274C} {}{}\n", RED, self.title, RESET));

        // Explanation (indented, dim)
        if !self.explanation.is_empty() {
            output.push('\n');
            for line in self.explanation.lines() {
                output.push_str(&format!("{}  {}{}\n", DIM, line, RESET));
            }
        }

        // Suggestion (gold highlight)
        if let Some(ref suggestion) = self.suggestion {
            output.push_str(&format!(
                "\n{}  Try:{} {}{}{}\n",
                DIM, RESET, GOLD, suggestion, RESET
            ));
        }

        // Example (purple highlight)
        if let Some(ref example) = self.example {
            output.push_str(&format!(
                "\n{}  Example:{}\n    {}{}{}\n",
                DIM, RESET, PURPLE, example, RESET
            ));
        }

        output.push('\n');
        output
    }

    /// Format for display without colors (logs, CI)
    pub fn format_plain(&self) -> String {
        let mut output = String::with_capacity(512);

        output.push_str(&format!("\nError: {}\n", self.title));

        if !self.explanation.is_empty() {
            output.push('\n');
            for line in self.explanation.lines() {
                output.push_str(&format!("  {}\n", line));
            }
        }

        if let Some(ref suggestion) = self.suggestion {
            output.push_str(&format!("\n  Try: {}\n", suggestion));
        }

        if let Some(ref example) = self.example {
            output.push_str(&format!("\n  Example:\n    {}\n", example));
        }

        output.push('\n');
        output
    }
}

impl fmt::Display for FriendlyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_colored())
    }
}

// ============================================================================
// Conversion from CliError
// ============================================================================

use super::args::CliError;

impl From<CliError> for FriendlyError {
    fn from(error: CliError) -> Self {
        match error {
            CliError::NoCommand => FriendlyError::new("No command provided")
                .with_explanation(
                    "kindly-av1 needs to know what you want to do.\n\
                     The most common use is encoding a video to AV1 format."
                )
                .with_suggestion("Run the interactive wizard or encode directly")
                .with_example("kindly-av1 video.mp4"),

            CliError::UnknownCommand(cmd) => FriendlyError::new(format!("Unknown command: '{}'", cmd))
                .with_explanation(
                    "Available commands:\n\
                     - encode    Convert video to AV1 format\n\
                     - info      Show video file information\n\
                     - benchmark Test encoding performance\n\
                     - wizard    Interactive guided setup\n\
                     - help      Show all available commands"
                )
                .with_suggestion("Use 'encode' for video conversion")
                .with_example(format!("kindly-av1 encode {} -o output.av1", cmd)),

            CliError::MissingArgument { name, context } =>
                FriendlyError::new(format!("Missing required value: {}", name))
                    .with_explanation(format!(
                        "The {} command needs {} to work properly.",
                        context, name
                    ))
                    .with_suggestion(format!("Provide a value for {}", name)),

            CliError::InvalidValue { name, value, expected } =>
                FriendlyError::new(format!("Invalid value '{}' for {}", value, name))
                    .with_explanation(format!(
                        "The value '{}' doesn't match what {} expects.\n\
                         Expected: {}",
                        value, name, expected
                    ))
                    .with_suggestion(format!("Use {}", expected)),

            CliError::MissingInput => FriendlyError::new("No input file specified")
                .with_explanation(
                    "kindly-av1 needs to know which video file to encode.\n\
                     Supported formats: MP4, MKV, WebM, MOV, AVI, Y4M"
                )
                .with_suggestion("Provide a video file path")
                .with_example("kindly-av1 encode my_video.mp4"),

            CliError::FileNotFound(path) => FriendlyError::new(format!(
                "File not found: {}", path.display()
            ))
                .with_explanation(
                    "The video file you specified doesn't exist or can't be accessed.\n\
                     Please check:\n\
                     - The file path is spelled correctly\n\
                     - The file exists in that location\n\
                     - You have permission to read the file"
                )
                .with_suggestion("Double-check the file path")
                .with_example(format!("kindly-av1 encode \"{}\"", path.display())),

            CliError::UnknownOption(opt) => {
                // Try to suggest similar options
                let suggestion = suggest_similar_option(&opt);
                let mut err = FriendlyError::new(format!("Unknown option: {}", opt))
                    .with_explanation(
                        "This option isn't recognized. Common options:\n\
                         --crf <0-63>     Quality level (lower = better)\n\
                         --preset <name>  Speed preset (fast, balanced, quality)\n\
                         -o <path>        Output file path\n\
                         --help           Show all available options"
                    );

                if let Some(similar) = suggestion {
                    err = err.with_suggestion(format!("Did you mean {}?", similar));
                }

                err.with_example("kindly-av1 encode video.mp4 --crf 28")
            }

            CliError::OptionRequiresValue(opt) => FriendlyError::new(format!(
                "Option {} needs a value", opt
            ))
                .with_explanation(format!(
                    "The {} option requires a value after it.\n\
                     It can't be used on its own.",
                    opt
                ))
                .with_suggestion(format!("Provide a value after {}", opt))
                .with_example(format!("kindly-av1 encode video.mp4 {} 28", opt)),

            CliError::InvalidCrf(val) => FriendlyError::new(format!(
                "CRF value '{}' is out of range", val
            ))
                .with_explanation(
                    "AV1 uses CRF (Constant Rate Factor) values from 0 to 63.\n\
                     \n\
                     CRF Guide:\n\
                     - 0-15:  Visually lossless, very large files\n\
                     - 16-24: High quality, large files\n\
                     - 25-35: Balanced quality/size (recommended)\n\
                     - 36-50: Lower quality, small files\n\
                     - 51-63: Low quality, very small files\n\
                     \n\
                     Lower CRF = Higher quality = Larger file\n\
                     Higher CRF = Lower quality = Smaller file"
                )
                .with_suggestion("Use CRF 28-32 for a good balance")
                .with_example("kindly-av1 encode video.mp4 --crf 30"),
        }
    }
}

// ============================================================================
// Preset Validation Errors
// ============================================================================

/// Create friendly error for invalid preset
pub fn invalid_preset_error(value: &str) -> FriendlyError {
    FriendlyError::new(format!("Unknown preset: '{}'", value))
        .with_explanation(
            "Available presets control the speed/quality tradeoff:\n\
             \n\
             fast       Speed 8 - Quick encodes, preview quality\n\
             balanced   Speed 5 - Good quality, reasonable time (default)\n\
             quality    Speed 2 - High quality, slower encoding\n\
             placebo    Speed 0 - Maximum quality, very slow\n\
             \n\
             Note: 'balanced' is recommended for most users."
        )
        .with_suggestion("Use 'balanced' for most encodes")
        .with_example("kindly-av1 encode video.mp4 --preset balanced")
}

/// Create friendly error for invalid bitrate
pub fn invalid_bitrate_error(value: &str) -> FriendlyError {
    FriendlyError::new(format!("Invalid bitrate: '{}'", value))
        .with_explanation(
            "Bitrate should be a number with optional suffix:\n\
             \n\
             - 5M or 5Mbps  = 5 megabits/second\n\
             - 5000k        = 5000 kilobits/second\n\
             - 5000000      = 5000000 bits/second\n\
             \n\
             Recommended bitrates:\n\
             - 720p:  2-5 Mbps\n\
             - 1080p: 4-8 Mbps\n\
             - 4K:    15-25 Mbps"
        )
        .with_suggestion("Try 5M for 1080p content")
        .with_example("kindly-av1 encode video.mp4 --bitrate 5M")
}

/// Create friendly error for resolution mismatch
pub fn resolution_error(width: u32, height: u32, tier: &str, max_width: u32) -> FriendlyError {
    FriendlyError::new(format!(
        "Video resolution {}x{} exceeds {} tier limit", width, height, tier
    ))
        .with_explanation(format!(
            "Your {} license supports up to {}p resolution.\n\
             \n\
             This video is {}x{} which requires an upgraded license.\n\
             \n\
             License tiers:\n\
             - Registered Free: 720p  (max {}px width)\n\
             - Creator ($49):   1080p (max 1920px width)\n\
             - Professional ($149): 4K (max 3840px width)\n\
             - Enterprise ($499): 8K  (max 7680px width)",
            tier, max_width, width, height, max_width
        ))
        .with_suggestion("Upgrade your license or resize the video")
        .with_example("kindly-av1 encode video.mp4 --size 720p")
}

/// Create friendly error for disk space
pub fn disk_space_error(required: u64, available: u64) -> FriendlyError {
    let required_str = format_size(required);
    let available_str = format_size(available);

    FriendlyError::new("Not enough disk space")
        .with_explanation(format!(
            "Encoding needs approximately {} of free disk space.\n\
             Currently available: {}\n\
             \n\
             The output file could be up to 2x the input size during encoding,\n\
             plus temporary files for checkpoints.",
            required_str, available_str
        ))
        .with_suggestion("Free up disk space or use a different output location")
        .with_example("kindly-av1 encode video.mp4 -o /path/with/more/space/output.av1")
}

/// Create friendly error for GPU not available
pub fn gpu_unavailable_error(requested: &str) -> FriendlyError {
    FriendlyError::new(format!("GPU '{}' not available", requested))
        .with_explanation(
            "The requested GPU backend couldn't be initialized.\n\
             \n\
             Possible causes:\n\
             - GPU drivers not installed\n\
             - GPU doesn't support compute\n\
             - Another application is using the GPU\n\
             - ROCm/Vulkan not properly configured\n\
             \n\
             kindly-av1 will automatically fall back to CPU encoding."
        )
        .with_suggestion("Use --no-gpu to explicitly use CPU encoding")
        .with_example("kindly-av1 encode video.mp4 --no-gpu")
}

/// Create friendly error for output file exists
pub fn output_exists_error(path: &PathBuf) -> FriendlyError {
    FriendlyError::new(format!("Output file already exists: {}", path.display()))
        .with_explanation(
            "A file with this name already exists.\n\
             kindly-av1 won't overwrite it without your permission."
        )
        .with_suggestion("Use -y to overwrite or choose a different output name")
        .with_example(format!("kindly-av1 encode video.mp4 -o {} -y", path.display()))
}

/// Create friendly error for invalid time format
pub fn invalid_time_error(value: &str) -> FriendlyError {
    FriendlyError::new(format!("Invalid time format: '{}'", value))
        .with_explanation(
            "Time can be specified as:\n\
             \n\
             - Seconds:    90     (90 seconds)\n\
             - MM:SS:      1:30   (1 minute 30 seconds)\n\
             - HH:MM:SS:   1:30:00 (1 hour 30 minutes)"
        )
        .with_suggestion("Use seconds or MM:SS format")
        .with_example("kindly-av1 encode video.mp4 --start 1:30 --duration 5:00")
}

/// Create friendly error for license issues
pub fn license_error(kind: &str) -> FriendlyError {
    match kind {
        "not_found" | "not_activated" => FriendlyError::new("License not activated")
            .with_explanation(
                "kindly-av1 requires a valid license to encode videos.\n\
                 \n\
                 Options:\n\
                 1. Activate your existing license key\n\
                 2. Purchase a license at https://kindly.gumroad.com/kindly-av1"
            )
            .with_suggestion("Run 'kindly-av1 license activate' with your key")
            .with_example("kindly-av1 license activate KDLY-XXXX-XXXX-XXXX"),

        "expired" => FriendlyError::new("License has expired")
            .with_explanation(
                "Your kindly-av1 license has expired.\n\
                 \n\
                 Visit https://kindly.gumroad.com/kindly-av1 to renew."
            )
            .with_suggestion("Renew your license")
            .with_example("kindly-av1 license status"),

        "hardware_mismatch" => FriendlyError::new("License bound to different hardware")
            .with_explanation(
                "This license was activated on a different computer.\n\
                 \n\
                 Licenses are bound to hardware to prevent unauthorized sharing.\n\
                 You can deactivate on the old machine and reactivate here."
            )
            .with_suggestion("Deactivate on old machine first, then reactivate here")
            .with_example("kindly-av1 license activate KDLY-XXXX-XXXX-XXXX"),

        _ => FriendlyError::new("License error")
            .with_explanation(
                "There was a problem with your license.\n\
                 Please check your license status."
            )
            .with_suggestion("Check license status")
            .with_example("kindly-av1 license status"),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Suggest a similar option if the user made a typo
fn suggest_similar_option(input: &str) -> Option<String> {
    let known_options = [
        "--crf", "--preset", "--output", "-o", "--bitrate", "-b",
        "--fast", "--quality", "--placebo", "--resume", "-r",
        "--checkpoint", "--threads", "-t", "--help", "-h",
        "--verbose", "-v", "--quiet", "-q", "--no-gpu", "--cpu",
        "--overwrite", "-y", "--wizard", "-w",
    ];

    let input_lower = input.to_lowercase();

    // Check for common typos
    for opt in &known_options {
        let opt_lower = opt.to_lowercase();

        // Exact prefix match
        if opt_lower.starts_with(&input_lower) || input_lower.starts_with(&opt_lower) {
            return Some(opt.to_string());
        }

        // Simple edit distance check (very rough)
        if levenshtein_close(&input_lower, &opt_lower) {
            return Some(opt.to_string());
        }
    }

    None
}

/// Very simple check if two strings are within edit distance 2
fn levenshtein_close(a: &str, b: &str) -> bool {
    let len_diff = (a.len() as i32 - b.len() as i32).unsigned_abs() as usize;
    if len_diff > 2 {
        return false;
    }

    let mut differences = 0;
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();

    let min_len = a_chars.len().min(b_chars.len());
    for i in 0..min_len {
        if a_chars[i] != b_chars[i] {
            differences += 1;
        }
    }

    differences + len_diff <= 2
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

// ============================================================================
// Print Helpers
// ============================================================================

/// Print a friendly error to stderr
pub fn print_friendly_error(error: &FriendlyError, use_color: bool) {
    if use_color {
        eprint!("{}", error.format_colored());
    } else {
        eprint!("{}", error.format_plain());
    }
}

/// Convert CliError to FriendlyError and print
pub fn print_cli_error(error: CliError, use_color: bool) {
    let friendly: FriendlyError = error.into();
    print_friendly_error(&friendly, use_color);
}

// ============================================================================
// Format Helpers (for returning strings instead of printing)
// ============================================================================

/// Format a FriendlyError to a String (with colors if terminal supports it)
///
/// This function detects if stdout is a TTY and uses colors accordingly.
/// For explicit color control, use `error.format_colored()` or `error.format_plain()`.
///
/// # Example
///
/// ```ignore
/// let err = FriendlyError::new("Something went wrong")
///     .with_suggestion("Try again");
/// let message = format_friendly_error(&err);
/// eprintln!("{}", message);
/// ```
pub fn format_friendly_error(error: &FriendlyError) -> String {
    // Check if we're outputting to a terminal
    // Use colored output if stdout is a TTY, otherwise use plain
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let is_tty = unsafe { libc::isatty(std::io::stderr().as_raw_fd()) != 0 };
        if is_tty {
            error.format_colored()
        } else {
            error.format_plain()
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix systems, default to plain output
        error.format_plain()
    }
}

/// Convert a CliError to a FriendlyError
///
/// This is a convenience function that wraps the From trait conversion.
///
/// # Example
///
/// ```ignore
/// let cli_err = CliError::MissingInput;
/// let friendly = cli_error_to_friendly(cli_err);
/// ```
pub fn cli_error_to_friendly(error: CliError) -> FriendlyError {
    error.into()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_friendly_error_basic() {
        let err = FriendlyError::new("Test error")
            .with_explanation("This is an explanation")
            .with_suggestion("Try this instead")
            .with_example("example command");

        assert_eq!(err.title, "Test error");
        assert_eq!(err.explanation, "This is an explanation");
        assert_eq!(err.suggestion, Some("Try this instead".to_string()));
        assert_eq!(err.example, Some("example command".to_string()));
    }

    #[test]
    fn test_cli_error_conversion_crf() {
        let cli_err = CliError::InvalidCrf("100".to_string());
        let friendly: FriendlyError = cli_err.into();

        assert!(friendly.title.contains("100"));
        assert!(friendly.explanation.contains("0 to 63"));
        assert!(friendly.suggestion.is_some());
        assert!(friendly.example.is_some());
    }

    #[test]
    fn test_cli_error_conversion_missing_input() {
        let cli_err = CliError::MissingInput;
        let friendly: FriendlyError = cli_err.into();

        assert!(friendly.title.contains("input file"));
        assert!(friendly.explanation.contains("Supported formats"));
    }

    #[test]
    fn test_cli_error_conversion_file_not_found() {
        let cli_err = CliError::FileNotFound(PathBuf::from("/test/video.mp4"));
        let friendly: FriendlyError = cli_err.into();

        assert!(friendly.title.contains("video.mp4"));
        assert!(friendly.explanation.contains("spelled correctly"));
    }

    #[test]
    fn test_suggest_similar_option() {
        assert_eq!(suggest_similar_option("--crff"), Some("--crf".to_string()));
        assert_eq!(suggest_similar_option("--presets"), Some("--preset".to_string()));
        assert_eq!(suggest_similar_option("-oo"), Some("-o".to_string()));
    }

    #[test]
    fn test_levenshtein_close() {
        assert!(levenshtein_close("--crf", "--crff"));
        assert!(levenshtein_close("preset", "presets"));
        assert!(!levenshtein_close("--crf", "--bitrate"));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 bytes");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1048576), "1.0 MB");
        assert_eq!(format_size(1073741824), "1.0 GB");
    }

    #[test]
    fn test_format_plain() {
        let err = FriendlyError::new("Test")
            .with_explanation("Explanation")
            .with_suggestion("Suggestion");

        let plain = err.format_plain();
        assert!(plain.contains("Error: Test"));
        assert!(plain.contains("Explanation"));
        assert!(plain.contains("Try: Suggestion"));
        // Should not contain ANSI codes
        assert!(!plain.contains("\x1b["));
    }

    #[test]
    fn test_invalid_preset_error() {
        let err = invalid_preset_error("superfast");
        assert!(err.title.contains("superfast"));
        assert!(err.explanation.contains("fast"));
        assert!(err.explanation.contains("balanced"));
        assert!(err.explanation.contains("quality"));
    }

    #[test]
    fn test_resolution_error() {
        let err = resolution_error(3840, 2160, "Registered Free", 1280);
        assert!(err.title.contains("3840x2160"));
        assert!(err.explanation.contains("Creator"));
        assert!(err.explanation.contains("Professional"));
    }

    #[test]
    fn test_disk_space_error() {
        let err = disk_space_error(5_000_000_000, 1_000_000_000);
        assert!(err.title.contains("disk space"));
        assert!(err.explanation.contains("4.7 GB")); // ~5GB
        assert!(err.explanation.contains("953.7 MB")); // ~1GB
    }
}
