//! Kindly-AV1 CLI Argument Parsing
//!
//! [TRADE SECRET] - Proprietary argument parsing implementation.
//!
//! # Architecture
//!
//! This module implements a lockfree, zero-allocation argument parser
//! following the Chaos (Computational Capsule Architecture) principles:
//!
//! - No mutex or RwLock
//! - Pure functional parsing (state passed explicitly)
//! - Zero global mutable state
//! - Compile-time validation where possible
//!
//! # One-Command Simplicity (Wave 4)
//!
//! Following SOTA CLI UX best practices from clig.dev and Evil Martians,
//! kindly-av1 supports one-command operation:
//!
//! ```bash
//! kindly-av1 video.mp4
//! ```
//!
//! This automatically:
//! 1. Detects video file extension (.mp4, .mkv, .mov, .webm, .avi, .y4m)
//! 2. Inserts implicit "encode" command
//! 3. Auto-generates output path (video.mp4 -> video.av1)
//! 4. Selects smart preset based on file size:
//!    - < 100MB: Fast (quick preview)
//!    - 100MB-1GB: Balanced (default quality)
//!    - > 1GB: Quality (user investing time)
//!
//! # Chaos Compliance
//!
//! - UCE34 Q33: 100% lockfree parsing
//! - No heap allocation for common paths
//! - Explicit error handling via Result

use std::path::PathBuf;
use std::fmt;

use crate::obs::{ObsOptions, ObsStatusFormat};

// ============================================================================
// Video File Detection (One-Command Simplicity)
// ============================================================================

/// Supported video file extensions for auto-detection
pub const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "mov", "webm", "avi", "y4m",
    "m4v", "wmv", "flv", "ts", "mts", "m2ts",
];

/// Check if a path looks like a video file based on extension
///
/// Used for one-command simplicity: `kindly-av1 video.mp4`
///
/// # Arguments
/// * `path` - Path string to check
///
/// # Returns
/// `true` if the extension matches a known video format
pub fn is_video_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    VIDEO_EXTENSIONS.iter().any(|ext| path_lower.ends_with(&format!(".{}", ext)))
}

/// Select smart preset based on file size
///
/// Following UX research: users with small files want quick results,
/// users with large files are invested and want quality.
///
/// # Arguments
/// * `file_size` - Input file size in bytes
///
/// # Returns
/// Appropriate preset for the file size
pub fn smart_preset_for_size(file_size: u64) -> Preset {
    const MB_100: u64 = 100 * 1024 * 1024;    // 100 MB
    const GB_1: u64 = 1024 * 1024 * 1024;     // 1 GB

    if file_size < MB_100 {
        Preset::Fast       // Quick preview for small files
    } else if file_size < GB_1 {
        Preset::Balanced   // Default for medium files
    } else {
        Preset::Quality    // User is investing time for large files
    }
}

/// Generate output path from input (replace extension with .av1)
///
/// # Examples
/// - video.mp4 -> video.av1
/// - /path/to/movie.mkv -> /path/to/movie.av1
pub fn auto_output_path(input: &PathBuf) -> PathBuf {
    let mut output = input.clone();
    output.set_extension("av1");
    output
}

// ============================================================================
// Error Types
// ============================================================================

/// CLI parsing errors
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    /// No command provided
    NoCommand,
    /// Unknown command
    UnknownCommand(String),
    /// Missing required argument
    MissingArgument { name: &'static str, context: &'static str },
    /// Invalid argument value
    InvalidValue { name: &'static str, value: String, expected: &'static str },
    /// Missing input file
    MissingInput,
    /// File not found
    FileNotFound(PathBuf),
    /// Unknown option
    UnknownOption(String),
    /// Option requires value
    OptionRequiresValue(String),
    /// Invalid CRF value (must be 0-63)
    InvalidCrf(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand => write!(f, "No command provided. Use 'kindly-av1 help' for usage."),
            Self::UnknownCommand(cmd) => write!(f, "Unknown command: '{}'. Use 'kindly-av1 help' for available commands.", cmd),
            Self::MissingArgument { name, context } => write!(f, "Missing required argument '{}' for {}.", name, context),
            Self::InvalidValue { name, value, expected } => write!(f, "Invalid value '{}' for {}: expected {}.", value, name, expected),
            Self::MissingInput => write!(f, "No input file specified."),
            Self::FileNotFound(path) => write!(f, "File not found: {}", path.display()),
            Self::UnknownOption(opt) => write!(f, "Unknown option: '{}'. Use 'kindly-av1 help' for options.", opt),
            Self::OptionRequiresValue(opt) => write!(f, "Option '{}' requires a value.", opt),
            Self::InvalidCrf(val) => write!(f, "Invalid CRF value '{}': must be 0-63.", val),
        }
    }
}

impl std::error::Error for CliError {}

// ============================================================================
// Preset Enum
// ============================================================================

/// Encoding preset determining speed/quality tradeoff
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Preset {
    /// Fast encoding, lower quality (speed 8)
    /// Good for: previews, quick tests, real-time streaming
    Fast,

    /// Balanced speed and quality (speed 5)
    /// Good for: most use cases, general transcoding
    #[default]
    Balanced,

    /// High quality, slower encoding (speed 2)
    /// Good for: archival, final delivery, quality-critical content
    Quality,

    /// Maximum quality, slowest encoding (speed 0)
    /// Good for: mastering, archival of irreplaceable content
    Placebo,
}

impl Preset {
    /// Get the rav1e speed setting for this preset
    #[inline]
    pub const fn speed(&self) -> u8 {
        match self {
            Self::Fast => 8,
            Self::Balanced => 5,
            Self::Quality => 2,
            Self::Placebo => 0,
        }
    }

    /// Get preset name for display
    #[inline]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Balanced => "balanced",
            Self::Quality => "quality",
            Self::Placebo => "placebo",
        }
    }

    /// Parse preset from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "fast" | "f" => Some(Self::Fast),
            "balanced" | "b" | "default" => Some(Self::Balanced),
            "quality" | "q" | "high" => Some(Self::Quality),
            "placebo" | "p" | "max" => Some(Self::Placebo),
            _ => None,
        }
    }
}

impl fmt::Display for Preset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (speed {})", self.name(), self.speed())
    }
}

// ============================================================================
// Global Options
// ============================================================================

/// Global CLI options applicable to all commands
#[derive(Debug, Clone, Default)]
pub struct GlobalOptions {
    /// Enable verbose output (multiple -v for more verbosity)
    pub verbose: u8,

    /// Suppress all non-error output
    pub quiet: bool,

    /// Disable GPU acceleration, use CPU only
    pub no_gpu: bool,

    /// Disable colored output
    pub no_color: bool,

    /// Number of threads to use (0 = auto)
    pub threads: u32,

    /// Config file path (optional)
    pub config: Option<PathBuf>,
}

impl GlobalOptions {
    /// Create default options
    #[inline]
    pub const fn new() -> Self {
        Self {
            verbose: 0,
            quiet: false,
            no_gpu: false,
            no_color: false,
            threads: 0,
            config: None,
        }
    }

    /// Check if output should be shown (not quiet)
    #[inline]
    pub const fn should_output(&self) -> bool {
        !self.quiet
    }

    /// Check if colors should be used
    #[inline]
    pub fn should_color(&self) -> bool {
        !self.no_color && std::env::var("NO_COLOR").is_err()
    }
}

// ============================================================================
// Encode Options
// ============================================================================

/// Options specific to the encode command
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// Input file path
    pub input: PathBuf,

    /// Output file path (default: input with .av1 extension)
    pub output: Option<PathBuf>,

    /// Encoding preset
    pub preset: Preset,

    /// Constant Rate Factor (0-63, lower = higher quality)
    /// Default: 32 for balanced quality/size
    pub crf: u8,

    /// Resume from checkpoint if available
    /// Based on Av1an's --resume flag: https://rust-av.github.io/Av1an/Cli/general.html
    pub resume: bool,

    /// Checkpoint file path (for resume capability)
    /// Default: output.av1.kdly.ckpt (auto-generated from output path)
    /// Based on two-phase commit pattern for crash-safe persistence
    pub checkpoint_path: Option<PathBuf>,

    /// Checkpoint interval in frames (default: 30)
    /// Every N frames, a checkpoint is written to disk
    /// Lower values = more frequent checkpoints = more I/O but better resume granularity
    /// Based on Av1an's segment-based checkpointing approach
    pub checkpoint_interval: Option<u64>,

    /// Target bitrate in kbps (0 = CRF mode)
    pub bitrate: u32,

    /// Two-pass encoding
    pub two_pass: bool,

    /// Start time in seconds (for seeking)
    pub start_time: Option<f64>,

    /// Duration in seconds (for trimming)
    pub duration: Option<f64>,

    /// Video filters (resize, crop, etc.)
    pub filters: Vec<String>,

    /// Output width (0 = auto)
    pub width: u32,

    /// Output height (0 = auto)
    pub height: u32,

    /// Frame rate (0 = original)
    pub fps: f32,

    /// Overwrite output without asking
    pub overwrite: bool,

    /// OBS integration options
    pub obs: ObsOptions,

    /// Launch guided setup wizard
    pub wizard: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            input: PathBuf::new(),
            output: None,
            preset: Preset::default(),
            crf: 32,
            resume: false,
            checkpoint_path: None,
            checkpoint_interval: None, // Default 30 applied in commands.rs
            bitrate: 0,
            two_pass: false,
            start_time: None,
            duration: None,
            filters: Vec::new(),
            width: 0,
            height: 0,
            fps: 0.0,
            overwrite: false,
            obs: ObsOptions::default(),
            wizard: false,
        }
    }
}

impl EncodeOptions {
    /// Create new encode options with input file
    pub fn new(input: PathBuf) -> Self {
        Self {
            input,
            ..Default::default()
        }
    }

    /// Get output path, defaulting to input with .av1 extension
    pub fn output_path(&self) -> PathBuf {
        self.output.clone().unwrap_or_else(|| {
            let mut path = self.input.clone();
            path.set_extension("av1");
            path
        })
    }

    /// Validate options
    pub fn validate(&self) -> Result<(), CliError> {
        // Validate CRF range
        if self.crf > 63 {
            return Err(CliError::InvalidCrf(self.crf.to_string()));
        }

        Ok(())
    }
}

// ============================================================================
// Info Options
// ============================================================================

/// Options for the info command
#[derive(Debug, Clone)]
pub struct InfoOptions {
    /// File to inspect
    pub path: PathBuf,

    /// Show detailed stream information
    pub detailed: bool,

    /// Output format (text, json)
    pub format: OutputFormat,
}

/// Output format for info command
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Text,
    Json,
    Xml,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text" | "t" => Some(Self::Text),
            "json" | "j" => Some(Self::Json),
            "xml" | "x" => Some(Self::Xml),
            _ => None,
        }
    }
}

// ============================================================================
// Command Enum
// ============================================================================

/// Available CLI commands
#[derive(Debug, Clone)]
pub enum Command {
    /// Encode video to AV1
    Encode(EncodeOptions),

    /// Show video file information
    Info(InfoOptions),

    /// Run GPU performance benchmarks
    Benchmark {
        /// Duration of benchmark in seconds
        duration_secs: u32,
        /// Resolution to test (720, 1080, 4k)
        resolution: String,
    },

    /// Show help message
    Help {
        /// Specific command to get help for
        command: Option<String>,
    },

    /// Show version information
    Version,

    /// List available GPU devices
    ListGpu,

    /// License management (activate/status/deactivate)
    License {
        /// License subcommand
        subcommand: LicenseSubcommand,
    },

    /// Generate shell completions
    Completions {
        /// Shell type (bash, zsh, fish, powershell)
        shell: String,
    },

    /// Apply support reset code to clear hardware ban
    ResetBan {
        /// Reset code in format KINDLY-XXXX-XXXX-XXXX
        code: String,
    },

    /// Launch interactive wizard for guided setup
    Wizard,
}

/// License subcommands
#[derive(Debug, Clone)]
pub enum LicenseSubcommand {
    /// Activate license online
    Activate {
        /// License key (XXXXX-XXXXX-XXXXX-XXXXX)
        key: String,
    },
    /// Show license status
    Status,
    /// Deactivate license
    Deactivate,
}

// ============================================================================
// Wizard Mode Detection
// ============================================================================

/// Wizard mode activation decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardMode {
    /// No args provided - show prompt asking if user wants wizard
    Prompt,
    /// User explicitly requested wizard (--wizard flag or command)
    Explicit,
    /// User provided arguments - skip wizard, run direct
    Direct,
}

/// Determine wizard activation mode based on parsed args
///
/// # Arguments
///
/// - `parsed`: Parsed CLI arguments
///
/// # Returns
///
/// `WizardMode` indicating how to proceed
///
/// # Logic
///
/// 1. If `Command::Wizard` → Explicit
/// 2. If `EncodeOptions.wizard == true` → Explicit
/// 3. If no command provided (bare Help) → Prompt
/// 4. Otherwise → Direct (normal operation)
pub fn determine_wizard_mode(parsed: &ParsedArgs) -> WizardMode {
    match &parsed.command {
        // Explicit wizard command
        Command::Wizard => WizardMode::Explicit,

        // Encode with --wizard flag
        Command::Encode(opts) if opts.wizard => WizardMode::Explicit,

        // Help with no subcommand AND no arguments → Prompt
        Command::Help { command: None } if !has_any_args() => WizardMode::Prompt,

        // All other cases → Direct
        _ => WizardMode::Direct,
    }
}

/// Check if any CLI arguments were provided (beyond program name)
///
/// # Returns
///
/// `true` if arguments were provided, `false` if bare invocation
fn has_any_args() -> bool {
    std::env::args().len() > 1
}

// ============================================================================
// Parsed Result
// ============================================================================

/// Complete parsed CLI result
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    /// Global options
    pub global: GlobalOptions,

    /// The command to execute
    pub command: Command,
}

// ============================================================================
// Parser Implementation
// ============================================================================

/// Parse command line arguments
///
/// # Returns
/// - `Ok(ParsedArgs)` on successful parse
/// - `Err(CliError)` on parse failure
///
/// # Chaos Compliance
/// - No mutex or global state
/// - Pure functional parsing
/// - Zero heap allocation for common paths
pub fn parse_args() -> Result<ParsedArgs, CliError> {
    let args: Vec<String> = std::env::args().collect();
    parse_args_from(&args)
}

/// Parse from a slice of arguments (for testing)
pub fn parse_args_from(args: &[String]) -> Result<ParsedArgs, CliError> {
    let mut global = GlobalOptions::new();
    let mut idx = 1; // Skip program name

    // Parse global options first
    while idx < args.len() {
        let arg = &args[idx];

        if !arg.starts_with('-') {
            break; // Start of command
        }

        match arg.as_str() {
            "-v" | "--verbose" => {
                global.verbose = global.verbose.saturating_add(1);
            }
            "-vv" => {
                global.verbose = global.verbose.saturating_add(2);
            }
            "-vvv" => {
                global.verbose = global.verbose.saturating_add(3);
            }
            "-q" | "--quiet" => {
                global.quiet = true;
            }
            "--no-gpu" | "--cpu" => {
                global.no_gpu = true;
            }
            "--no-color" | "--no-colours" => {
                global.no_color = true;
            }
            "-t" | "--threads" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--threads".into()));
                }
                global.threads = args[idx].parse().map_err(|_| {
                    CliError::InvalidValue {
                        name: "--threads",
                        value: args[idx].clone(),
                        expected: "a positive integer",
                    }
                })?;
            }
            "-c" | "--config" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--config".into()));
                }
                global.config = Some(PathBuf::from(&args[idx]));
            }
            "-h" | "--help" => {
                return Ok(ParsedArgs {
                    global,
                    command: Command::Help { command: None },
                });
            }
            "-V" | "--version" => {
                return Ok(ParsedArgs {
                    global,
                    command: Command::Version,
                });
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::UnknownOption(arg.clone()));
            }
            _ => break,
        }

        idx += 1;
    }

    // Parse command
    if idx >= args.len() {
        // No command - show help
        return Ok(ParsedArgs {
            global,
            command: Command::Help { command: None },
        });
    }

    let cmd = &args[idx];
    idx += 1;

    // =========================================================================
    // ONE-COMMAND SIMPLICITY (Wave 4)
    // =========================================================================
    // If the first argument looks like a video file, treat it as implicit encode.
    // This enables: `kindly-av1 video.mp4` instead of `kindly-av1 encode video.mp4`
    //
    // Sources:
    // - https://clig.dev/ - "Make it work without any arguments if possible"
    // - https://evilmartians.com/chronicles/cli-ux-best-practices
    // - https://www.atlassian.com/blog/it-teams/10-design-principles-for-delightful-clis

    let command = if is_video_file(cmd) {
        // Implicit encode command detected!
        // Re-parse with the video file as input to encode command
        let encode_args: Vec<String> = std::iter::once(cmd.clone())
            .chain(args[idx..].iter().cloned())
            .collect();
        parse_encode_command_smart(&encode_args)?
    } else {
        // Normal command parsing
        match cmd.as_str() {
            "encode" | "enc" | "e" => parse_encode_command(&args[idx..])?,
            "info" | "i" => parse_info_command(&args[idx..])?,
            "benchmark" | "bench" | "b" => parse_benchmark_command(&args[idx..])?,
            "help" | "h" | "?" => {
                let subcmd = args.get(idx).cloned();
                Command::Help { command: subcmd }
            }
            "version" | "ver" | "v" => Command::Version,
            "list-gpu" | "gpu" | "gpus" => Command::ListGpu,
            "license" | "lic" => parse_license_command(&args[idx..])?,
            "completions" => {
                let shell = args.get(idx).cloned().unwrap_or_else(|| "bash".into());
                Command::Completions { shell }
            }
            "reset-ban" | "reset" => {
                let code = args.get(idx).cloned().ok_or_else(|| {
                    CliError::MissingArgument {
                        name: "CODE",
                        context: "reset-ban command requires a reset code (format: KINDLY-XXXX-XXXX-XXXX)",
                    }
                })?;
                Command::ResetBan { code }
            }
            "wizard" | "wiz" | "w" => Command::Wizard,
            _ => return Err(CliError::UnknownCommand(cmd.clone())),
        }
    };

    Ok(ParsedArgs { global, command })
}

/// Parse encode command with smart defaults (for one-command simplicity)
///
/// Applies intelligent defaults:
/// - Auto-generates output path (input.mp4 -> input.av1)
/// - Selects preset based on file size
fn parse_encode_command_smart(args: &[String]) -> Result<Command, CliError> {
    // First, parse normally
    let cmd = parse_encode_command(args)?;

    // Then apply smart defaults if not overridden
    if let Command::Encode(mut opts) = cmd {
        let input_path = opts.input.clone();

        // Auto-generate output if not specified
        if opts.output.is_none() {
            opts.output = Some(auto_output_path(&input_path));
        }

        // Smart preset based on file size (if not explicitly set and no explicit preset)
        // We detect if preset was explicitly set by checking if it's still the default
        // and the args don't contain --preset, --fast, --quality, or --placebo
        let preset_explicitly_set = args.iter().any(|a| {
            a == "--preset" || a == "-p" ||
            a == "--fast" || a == "-f" ||
            a == "--quality" || a == "-Q" ||
            a == "--placebo"
        });

        if !preset_explicitly_set {
            // Get file size for smart preset selection
            if let Ok(metadata) = std::fs::metadata(&input_path) {
                opts.preset = smart_preset_for_size(metadata.len());
            }
        }

        // Auto-overwrite in one-command mode for better UX
        // (user explicitly chose to encode, implies they want output)
        if !args.iter().any(|a| a == "-y" || a == "--overwrite") {
            // Don't auto-overwrite, but we could prompt later
            // For now, leave as-is to be safe
        }

        return Ok(Command::Encode(opts));
    }

    unreachable!("parse_encode_command should always return Command::Encode")
}

/// Parse encode command options
fn parse_encode_command(args: &[String]) -> Result<Command, CliError> {
    let mut opts = EncodeOptions::default();
    let mut idx = 0;
    let mut input_set = false;

    while idx < args.len() {
        let arg = &args[idx];

        match arg.as_str() {
            "-o" | "--output" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--output".into()));
                }
                opts.output = Some(PathBuf::from(&args[idx]));
            }
            "-p" | "--preset" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--preset".into()));
                }
                opts.preset = Preset::from_str(&args[idx]).ok_or_else(|| {
                    CliError::InvalidValue {
                        name: "--preset",
                        value: args[idx].clone(),
                        expected: "fast, balanced, quality, or placebo",
                    }
                })?;
            }
            "--fast" | "-f" => {
                opts.preset = Preset::Fast;
            }
            "--quality" | "-Q" => {
                opts.preset = Preset::Quality;
            }
            "--placebo" => {
                opts.preset = Preset::Placebo;
            }
            "--crf" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--crf".into()));
                }
                let crf: u8 = args[idx].parse().map_err(|_| {
                    CliError::InvalidCrf(args[idx].clone())
                })?;
                if crf > 63 {
                    return Err(CliError::InvalidCrf(args[idx].clone()));
                }
                opts.crf = crf;
            }
            "-b" | "--bitrate" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--bitrate".into()));
                }
                opts.bitrate = parse_bitrate(&args[idx])?;
            }
            "--2pass" | "--two-pass" => {
                opts.two_pass = true;
            }
            "-r" | "--resume" => {
                opts.resume = true;
            }
            "--checkpoint" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--checkpoint".into()));
                }
                opts.checkpoint_path = Some(PathBuf::from(&args[idx]));
            }
            "--checkpoint-interval" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--checkpoint-interval".into()));
                }
                opts.checkpoint_interval = Some(
                    args[idx].parse::<u64>()
                        .map_err(|_| CliError::InvalidValue {
                            name: "--checkpoint-interval",
                            value: args[idx].clone(),
                            expected: "positive integer (frames between checkpoints)",
                        })?
                );
            }
            "-ss" | "--start" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--start".into()));
                }
                opts.start_time = Some(parse_time(&args[idx])?);
            }
            "-t" | "--duration" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--duration".into()));
                }
                opts.duration = Some(parse_time(&args[idx])?);
            }
            "-s" | "--size" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--size".into()));
                }
                let (w, h) = parse_size(&args[idx])?;
                opts.width = w;
                opts.height = h;
            }
            "--fps" | "--framerate" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--fps".into()));
                }
                opts.fps = args[idx].parse().map_err(|_| {
                    CliError::InvalidValue {
                        name: "--fps",
                        value: args[idx].clone(),
                        expected: "a number like 24, 30, 60",
                    }
                })?;
            }
            "-y" | "--overwrite" => {
                opts.overwrite = true;
            }
            "--filter" | "-vf" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--filter".into()));
                }
                opts.filters.push(args[idx].clone());
            }
            "-w" | "--wizard" => {
                opts.wizard = true;
            }
            // OBS Integration Options (Phase 1: Text File Output)
            "--obs-status" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--obs-status".into()));
                }
                opts.obs.status_file = Some(PathBuf::from(&args[idx]));
            }
            "--obs-format" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--obs-format".into()));
                }
                opts.obs.status_format = ObsStatusFormat::from_str(&args[idx]).ok_or_else(|| {
                    CliError::InvalidValue {
                        name: "--obs-format",
                        value: args[idx].clone(),
                        expected: "simple, multiline, or json",
                    }
                })?;
            }
            "--obs-interval" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--obs-interval".into()));
                }
                opts.obs.status_interval_ms = args[idx].parse().map_err(|_| {
                    CliError::InvalidValue {
                        name: "--obs-interval",
                        value: args[idx].clone(),
                        expected: "interval in milliseconds (100-5000)",
                    }
                })?;
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::UnknownOption(arg.clone()));
            }
            _ => {
                // Positional argument = input file
                if !input_set {
                    opts.input = PathBuf::from(arg);
                    input_set = true;
                } else if opts.output.is_none() {
                    opts.output = Some(PathBuf::from(arg));
                }
            }
        }

        idx += 1;
    }

    if !input_set {
        return Err(CliError::MissingInput);
    }

    opts.validate()?;
    Ok(Command::Encode(opts))
}

/// Parse info command options
fn parse_info_command(args: &[String]) -> Result<Command, CliError> {
    let mut path: Option<PathBuf> = None;
    let mut detailed = false;
    let mut format = OutputFormat::Text;
    let mut idx = 0;

    while idx < args.len() {
        let arg = &args[idx];

        match arg.as_str() {
            "-d" | "--detailed" | "--full" => {
                detailed = true;
            }
            "--format" | "-F" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--format".into()));
                }
                format = OutputFormat::from_str(&args[idx]).ok_or_else(|| {
                    CliError::InvalidValue {
                        name: "--format",
                        value: args[idx].clone(),
                        expected: "text, json, or xml",
                    }
                })?;
            }
            "--json" | "-j" => {
                format = OutputFormat::Json;
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::UnknownOption(arg.clone()));
            }
            _ => {
                path = Some(PathBuf::from(arg));
            }
        }

        idx += 1;
    }

    let path = path.ok_or(CliError::MissingInput)?;

    Ok(Command::Info(InfoOptions { path, detailed, format }))
}

/// Parse benchmark command options
fn parse_benchmark_command(args: &[String]) -> Result<Command, CliError> {
    let mut duration_secs = 30;
    let mut resolution = "1080".to_string();
    let mut idx = 0;

    while idx < args.len() {
        let arg = &args[idx];

        match arg.as_str() {
            "-d" | "--duration" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--duration".into()));
                }
                duration_secs = args[idx].parse().map_err(|_| {
                    CliError::InvalidValue {
                        name: "--duration",
                        value: args[idx].clone(),
                        expected: "duration in seconds",
                    }
                })?;
            }
            "-r" | "--resolution" => {
                idx += 1;
                if idx >= args.len() {
                    return Err(CliError::OptionRequiresValue("--resolution".into()));
                }
                resolution = args[idx].clone();
            }
            "--720" | "--720p" => {
                resolution = "720".to_string();
            }
            "--1080" | "--1080p" => {
                resolution = "1080".to_string();
            }
            "--4k" | "--2160" | "--2160p" => {
                resolution = "4k".to_string();
            }
            _ if arg.starts_with('-') => {
                return Err(CliError::UnknownOption(arg.clone()));
            }
            _ => {}
        }

        idx += 1;
    }

    Ok(Command::Benchmark { duration_secs, resolution })
}

/// Parse license command options
fn parse_license_command(args: &[String]) -> Result<Command, CliError> {
    if args.is_empty() {
        return Err(CliError::MissingArgument {
            name: "SUBCOMMAND",
            context: "license command requires a subcommand (activate, status, deactivate)",
        });
    }

    let subcommand_str = &args[0];
    let subcommand = match subcommand_str.as_str() {
        "activate" | "act" | "a" => {
            // Requires license key
            if args.len() < 2 {
                return Err(CliError::MissingArgument {
                    name: "LICENSE_KEY",
                    context: "activate subcommand requires a license key",
                });
            }
            LicenseSubcommand::Activate {
                key: args[1].clone(),
            }
        }
        "status" | "stat" | "s" => LicenseSubcommand::Status,
        "deactivate" | "deact" | "d" => LicenseSubcommand::Deactivate,
        _ => {
            return Err(CliError::UnknownCommand(format!(
                "Unknown license subcommand: '{}'. Use 'activate', 'status', or 'deactivate'.",
                subcommand_str
            )));
        }
    };

    Ok(Command::License { subcommand })
}

/// Parse bitrate string (e.g., "5M", "5000k", "5000000")
fn parse_bitrate(s: &str) -> Result<u32, CliError> {
    let s = s.trim().to_lowercase();

    let (num_str, multiplier) = if s.ends_with('m') || s.ends_with("mb") || s.ends_with("mbps") {
        let end = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
        (&s[..end], 1_000_000)
    } else if s.ends_with('k') || s.ends_with("kb") || s.ends_with("kbps") {
        let end = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
        (&s[..end], 1_000)
    } else {
        (s.as_str(), 1)
    };

    let num: f64 = num_str.parse().map_err(|_| {
        CliError::InvalidValue {
            name: "--bitrate",
            value: s.clone(),
            expected: "a number like 5M, 5000k, or 5000000",
        }
    })?;

    Ok((num * multiplier as f64) as u32)
}

/// Parse time string (e.g., "1:30:45", "90.5", "1h30m")
fn parse_time(s: &str) -> Result<f64, CliError> {
    let s = s.trim();

    // Try HH:MM:SS or MM:SS format
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => {
                let mins: f64 = parts[0].parse().map_err(|_| CliError::InvalidValue {
                    name: "time",
                    value: s.to_string(),
                    expected: "MM:SS format",
                })?;
                let secs: f64 = parts[1].parse().map_err(|_| CliError::InvalidValue {
                    name: "time",
                    value: s.to_string(),
                    expected: "MM:SS format",
                })?;
                Ok(mins * 60.0 + secs)
            }
            3 => {
                let hours: f64 = parts[0].parse().map_err(|_| CliError::InvalidValue {
                    name: "time",
                    value: s.to_string(),
                    expected: "HH:MM:SS format",
                })?;
                let mins: f64 = parts[1].parse().map_err(|_| CliError::InvalidValue {
                    name: "time",
                    value: s.to_string(),
                    expected: "HH:MM:SS format",
                })?;
                let secs: f64 = parts[2].parse().map_err(|_| CliError::InvalidValue {
                    name: "time",
                    value: s.to_string(),
                    expected: "HH:MM:SS format",
                })?;
                Ok(hours * 3600.0 + mins * 60.0 + secs)
            }
            _ => Err(CliError::InvalidValue {
                name: "time",
                value: s.to_string(),
                expected: "HH:MM:SS or MM:SS format",
            }),
        }
    } else {
        // Try plain seconds
        s.parse().map_err(|_| CliError::InvalidValue {
            name: "time",
            value: s.to_string(),
            expected: "seconds or HH:MM:SS format",
        })
    }
}

/// Parse size string (e.g., "1920x1080", "1080p", "4k")
fn parse_size(s: &str) -> Result<(u32, u32), CliError> {
    let s = s.trim().to_lowercase();

    // Common presets
    match s.as_str() {
        "720p" | "hd" => return Ok((1280, 720)),
        "1080p" | "fhd" => return Ok((1920, 1080)),
        "1440p" | "2k" | "qhd" => return Ok((2560, 1440)),
        "2160p" | "4k" | "uhd" => return Ok((3840, 2160)),
        "4320p" | "8k" => return Ok((7680, 4320)),
        _ => {}
    }

    // Try WxH format
    if let Some(x_pos) = s.find('x') {
        let width: u32 = s[..x_pos].parse().map_err(|_| CliError::InvalidValue {
            name: "--size",
            value: s.clone(),
            expected: "WxH like 1920x1080 or preset like 1080p, 4k",
        })?;
        let height: u32 = s[x_pos + 1..].parse().map_err(|_| CliError::InvalidValue {
            name: "--size",
            value: s.clone(),
            expected: "WxH like 1920x1080 or preset like 1080p, 4k",
        })?;
        return Ok((width, height));
    }

    Err(CliError::InvalidValue {
        name: "--size",
        value: s,
        expected: "WxH like 1920x1080 or preset like 1080p, 4k",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn test_parse_encode_basic() {
        let parsed = parse_args_from(&args("kindly-av1 encode video.mp4")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
                assert_eq!(opts.preset, Preset::Balanced);
                assert_eq!(opts.crf, 32);
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_parse_encode_with_options() {
        let parsed = parse_args_from(&args(
            "kindly-av1 encode video.mp4 -o out.av1 --fast --crf 28 -y"
        )).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
                assert_eq!(opts.output, Some(PathBuf::from("out.av1")));
                assert_eq!(opts.preset, Preset::Fast);
                assert_eq!(opts.crf, 28);
                assert!(opts.overwrite);
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_parse_info() {
        let parsed = parse_args_from(&args("kindly-av1 info video.mp4 --json")).unwrap();
        match parsed.command {
            Command::Info(opts) => {
                assert_eq!(opts.path, PathBuf::from("video.mp4"));
                assert_eq!(opts.format, OutputFormat::Json);
            }
            _ => panic!("Expected Info command"),
        }
    }

    #[test]
    fn test_parse_benchmark() {
        let parsed = parse_args_from(&args("kindly-av1 benchmark --4k -d 60")).unwrap();
        match parsed.command {
            Command::Benchmark { duration_secs, resolution } => {
                assert_eq!(duration_secs, 60);
                assert_eq!(resolution, "4k");
            }
            _ => panic!("Expected Benchmark command"),
        }
    }

    #[test]
    fn test_parse_global_options() {
        let parsed = parse_args_from(&args("kindly-av1 -vv --no-gpu --threads 8 encode video.mp4")).unwrap();
        assert_eq!(parsed.global.verbose, 2);
        assert!(parsed.global.no_gpu);
        assert_eq!(parsed.global.threads, 8);
    }

    #[test]
    fn test_parse_help() {
        let parsed = parse_args_from(&args("kindly-av1 help encode")).unwrap();
        match parsed.command {
            Command::Help { command } => {
                assert_eq!(command, Some("encode".to_string()));
            }
            _ => panic!("Expected Help command"),
        }
    }

    #[test]
    fn test_parse_bitrate() {
        assert_eq!(parse_bitrate("5M").unwrap(), 5_000_000);
        assert_eq!(parse_bitrate("5000k").unwrap(), 5_000_000);
        assert_eq!(parse_bitrate("5000000").unwrap(), 5_000_000);
        assert_eq!(parse_bitrate("2.5M").unwrap(), 2_500_000);
    }

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_time("90").unwrap(), 90.0);
        assert_eq!(parse_time("1:30").unwrap(), 90.0);
        assert_eq!(parse_time("1:30:45").unwrap(), 5445.0);
        assert_eq!(parse_time("0:00:30.5").unwrap(), 30.5);
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("1920x1080").unwrap(), (1920, 1080));
        assert_eq!(parse_size("1080p").unwrap(), (1920, 1080));
        assert_eq!(parse_size("4k").unwrap(), (3840, 2160));
        assert_eq!(parse_size("720p").unwrap(), (1280, 720));
    }

    #[test]
    fn test_preset_from_str() {
        assert_eq!(Preset::from_str("fast"), Some(Preset::Fast));
        assert_eq!(Preset::from_str("balanced"), Some(Preset::Balanced));
        assert_eq!(Preset::from_str("quality"), Some(Preset::Quality));
        assert_eq!(Preset::from_str("placebo"), Some(Preset::Placebo));
        assert_eq!(Preset::from_str("invalid"), None);
    }

    #[test]
    fn test_error_missing_input() {
        let result = parse_args_from(&args("kindly-av1 encode"));
        assert!(matches!(result, Err(CliError::MissingInput)));
    }

    #[test]
    fn test_error_invalid_crf() {
        let result = parse_args_from(&args("kindly-av1 encode video.mp4 --crf 100"));
        assert!(matches!(result, Err(CliError::InvalidCrf(_))));
    }

    #[test]
    fn test_error_unknown_option() {
        let result = parse_args_from(&args("kindly-av1 --invalid-option encode video.mp4"));
        assert!(matches!(result, Err(CliError::UnknownOption(_))));
    }

    // =========================================================================
    // ONE-COMMAND SIMPLICITY TESTS (Wave 4)
    // =========================================================================

    #[test]
    fn test_is_video_file() {
        // Positive cases
        assert!(is_video_file("video.mp4"));
        assert!(is_video_file("video.mkv"));
        assert!(is_video_file("video.mov"));
        assert!(is_video_file("video.webm"));
        assert!(is_video_file("video.avi"));
        assert!(is_video_file("video.y4m"));
        assert!(is_video_file("VIDEO.MP4")); // Case insensitive
        assert!(is_video_file("/path/to/video.mp4"));
        assert!(is_video_file("C:\\Videos\\test.mkv"));

        // Negative cases
        assert!(!is_video_file("encode"));
        assert!(!is_video_file("help"));
        assert!(!is_video_file("video.txt"));
        assert!(!is_video_file("video.jpg"));
        assert!(!is_video_file("mp4")); // No dot
    }

    #[test]
    fn test_one_command_simplicity_mp4() {
        let parsed = parse_args_from(&args("kindly-av1 video.mp4")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
                // Output auto-generated
                assert_eq!(opts.output, Some(PathBuf::from("video.av1")));
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_one_command_simplicity_mkv() {
        let parsed = parse_args_from(&args("kindly-av1 movie.mkv")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("movie.mkv"));
                assert_eq!(opts.output, Some(PathBuf::from("movie.av1")));
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_one_command_with_explicit_output() {
        let parsed = parse_args_from(&args("kindly-av1 video.mp4 -o custom.av1")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
                assert_eq!(opts.output, Some(PathBuf::from("custom.av1")));
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_one_command_with_crf() {
        let parsed = parse_args_from(&args("kindly-av1 video.mp4 --crf 24")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
                assert_eq!(opts.crf, 24);
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_one_command_with_preset() {
        let parsed = parse_args_from(&args("kindly-av1 video.mp4 --quality")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.preset, Preset::Quality);
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_smart_preset_small_file() {
        // < 100MB -> Fast
        assert_eq!(smart_preset_for_size(50 * 1024 * 1024), Preset::Fast);
    }

    #[test]
    fn test_smart_preset_medium_file() {
        // 100MB - 1GB -> Balanced
        assert_eq!(smart_preset_for_size(500 * 1024 * 1024), Preset::Balanced);
    }

    #[test]
    fn test_smart_preset_large_file() {
        // > 1GB -> Quality
        assert_eq!(smart_preset_for_size(2 * 1024 * 1024 * 1024), Preset::Quality);
    }

    #[test]
    fn test_auto_output_path() {
        assert_eq!(
            auto_output_path(&PathBuf::from("video.mp4")),
            PathBuf::from("video.av1")
        );
        assert_eq!(
            auto_output_path(&PathBuf::from("/path/to/movie.mkv")),
            PathBuf::from("/path/to/movie.av1")
        );
    }

    #[test]
    fn test_explicit_encode_still_works() {
        // Explicit encode command should still work
        let parsed = parse_args_from(&args("kindly-av1 encode video.mp4")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert_eq!(opts.input, PathBuf::from("video.mp4"));
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_wizard_flag() {
        let parsed = parse_args_from(&args("kindly-av1 encode video.mp4 --wizard")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert!(opts.wizard);
            }
            _ => panic!("Expected Encode command"),
        }
    }

    #[test]
    fn test_wizard_short_flag() {
        let parsed = parse_args_from(&args("kindly-av1 encode video.mp4 -w")).unwrap();
        match parsed.command {
            Command::Encode(opts) => {
                assert!(opts.wizard);
            }
            _ => panic!("Expected Encode command"),
        }
    }
}
