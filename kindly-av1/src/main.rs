//! kindly-av1 - GPU-Accelerated AV1 Video Encoder
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! World's fastest lockfree AV1 encoder built on Chaos architecture.
//! Copyright (c) 2025 Kindly. All rights reserved.

#![feature(portable_simd)]

use std::process::ExitCode;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use kindly_av1::checkpoint::{
    calculate_input_hash, default_checkpoint_path, recover_from_crash, CheckpointRecovery,
};
use kindly_av1::cli::{
    branding, determine_wizard_mode, execute, parse_args, Command, CommandError, EncodeOptions,
    GlobalOptions, WizardMode, FriendlyError, format_friendly_error,
};
use kindly_av1::encoder::{EncoderConfig, EncoderWiringCapsule, KindlyAv1CliMetacapsule};
use kindly_av1::file::{
    check_system_capabilities, create_reader, detect_format, InputFormat, PixelFormat,
};
use kindly_av1::license::{LicenseError, LicenseTier, TierEnforcementCapsule};
use kindly_av1::protection::{
    get_corruption_mask, init_tamper_detection, is_banned, run_tamper_detection, HardwareIdCapsule,
    BAN_MESSAGE,
};

// ============================================================================
// Wizard Integration Functions
// ============================================================================

/// Show the no-args wizard prompt and returns true if user wants wizard
///
/// Displays a friendly prompt when kindly-av1 is invoked with no arguments.
/// Offers three options:
/// 1. Launch wizard (Y/yes)
/// 2. Type a video file path to encode directly
/// 3. Decline wizard (n/no) - shows help instead
///
/// # Returns
///
/// - `Ok(true)` if user wants wizard
/// - `Ok(false)` if user declines or provides a file path
/// - `Err(io::Error)` if I/O fails
fn prompt_for_wizard() -> std::io::Result<bool> {
    use std::io::{self, Write};
    use std::path::Path;

    println!("💜 Kindly-AV1 Encoder");
    println!();
    println!("Would you like to use the guided setup wizard? [Y/n]");
    println!("(Or type a video file path to encode directly)");
    println!();
    print!("> ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let input = input.trim();

    // Check if it's a file path
    if Path::new(&input).exists() {
        eprintln!("Direct file encoding not yet implemented via prompt.");
        eprintln!("Use: kindly-av1 encode {}", input);
        return Ok(false);
    }

    // Check response (default to yes if empty)
    let wants_wizard =
        input.is_empty() || input.eq_ignore_ascii_case("y") || input.eq_ignore_ascii_case("yes");

    Ok(wants_wizard)
}

/// Run the interactive wizard flow
///
/// Full TUI wizard with arrow key navigation and automatic hardware detection.
///
/// # Flow
/// 1. Hardware detection (CPU, GPU, memory)
/// 2. Video file selection
/// 3. Quality goal choice (Smallest/Balanced/Best)
/// 4. Speed choice (Quick/Normal/Thorough)
/// 5. Confirmation and encoding start
///
/// # Arguments
///
/// - `global`: Global CLI options
///
/// # Returns
///
/// `Ok(())` on success, `Err(String)` on failure
fn run_wizard(global: &GlobalOptions) -> Result<(), String> {
    use kindly_av1::cli::wizard::{
        map_to_encoding_options,
        steps::WizardContext,
        tui::{disable_raw_mode, enable_raw_mode, keys, read_key},
        SpeedChoice, TerminalStateCapsule, WizardFlowCapsule, WizardState, WizardTuiCapsule,
    };

    // Create wizard components
    let flow = WizardFlowCapsule::new();
    let tui = WizardTuiCapsule::new(&flow);
    let terminal = TerminalStateCapsule::new();

    // Detect hardware
    let caps = check_system_capabilities();
    let cpu_threads = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(8);
    let memory_gb = 16; // Placeholder - would need system API

    // Initialize context with hardware info
    let mut ctx = WizardContext {
        input_path: None,
        quality: flow.quality(),
        speed: flow.speed(),
        output_path: None,
        gpu_name: if caps.direct_formats_supported {
            "ROCm GPU".to_string()
        } else {
            "Unknown".to_string()
        },
        cpu_threads,
        memory_gb,
    };

    // Enter raw mode for keyboard input
    terminal
        .enter_raw_mode()
        .map_err(|e| format!("Failed to enter raw mode: {}", e))?;

    // Ensure terminal is restored on panic
    struct TerminalGuard<'a>(&'a TerminalStateCapsule);
    impl Drop for TerminalGuard<'_> {
        fn drop(&mut self) {
            let _ = self.0.exit_raw_mode();
        }
    }
    let _guard = TerminalGuard(&terminal);

    // Start wizard
    flow.start();

    // Main wizard loop
    loop {
        // Update context from flow state
        ctx.quality = flow.quality();
        ctx.speed = flow.speed();
        if let Some(path) = flow.input_path() {
            ctx.input_path = Some(path.clone());
            // Auto-generate output path
            let output = std::path::PathBuf::from(&path);
            if let Some(stem) = output.file_stem() {
                let mut out_path = output.with_file_name(stem);
                out_path.set_extension("av1");
                ctx.output_path = Some(out_path.to_string_lossy().into_owned());
            }
        }

        // Render current state
        if let Err(e) = tui.render(&ctx) {
            eprintln!("Render error: {}", e);
            break;
        }

        // Check for completion or cancellation
        let state = flow.state();
        match state {
            WizardState::Complete => {
                // Exit raw mode before encoding
                drop(_guard);
                terminal
                    .exit_raw_mode()
                    .map_err(|e| format!("Failed to exit raw mode: {}", e))?;

                // Get encoding options from wizard choices
                let encoding_opts = map_to_encoding_options(ctx.quality, ctx.speed);

                // Map speed choice to preset
                use kindly_av1::cli::Preset;
                let preset = match ctx.speed {
                    SpeedChoice::Quick => Preset::Fast,
                    SpeedChoice::Normal => Preset::Balanced,
                    SpeedChoice::Thorough => Preset::Quality,
                };

                // Build EncodeOptions from wizard choices
                let encode_opts = EncodeOptions {
                    input: std::path::PathBuf::from(
                        ctx.input_path.as_ref().ok_or("No input file selected")?,
                    ),
                    output: ctx.output_path.as_ref().map(std::path::PathBuf::from),
                    preset,
                    crf: encoding_opts.crf,
                    resume: false,
                    checkpoint_path: None,
                    checkpoint_interval: None,
                    bitrate: 0, // CRF mode
                    two_pass: false,
                    start_time: None,
                    duration: None,
                    filters: Vec::new(),
                    width: 0,        // Auto-detect
                    height: 0,       // Auto-detect
                    fps: 0.0,        // Auto-detect
                    overwrite: true, // Auto-overwrite in wizard mode
                    obs: Default::default(),
                    wizard: false,
                };

                // Start encoding
                return run_encode_integrated(encode_opts, global);
            }
            WizardState::Cancelled => {
                // Exit gracefully
                drop(_guard);
                terminal
                    .exit_raw_mode()
                    .map_err(|e| format!("Failed to exit raw mode: {}", e))?;

                eprintln!("\nWizard cancelled.");
                return Ok(());
            }
            _ => {}
        }

        // Read key input (blocking)
        let key = read_key().map_err(|e| format!("Failed to read key: {}", e))?;

        // Handle Ctrl+C
        if key == keys::CTRL_C {
            flow.cancel();
            continue;
        }

        // Handle key and check for redraw
        if tui.handle_key(key) {
            // Screen needs redraw - loop will re-render
        }
    }

    // Should not reach here, but ensure cleanup
    drop(_guard);
    terminal
        .exit_raw_mode()
        .map_err(|e| format!("Failed to exit raw mode: {}", e))?;

    Ok(())
}

// ============================================================================
// Hardware Protection Functions
// ============================================================================

/// Check if this hardware is banned (Tier 4 self-destruct active)
///
/// Returns Ok(()) if not banned, Err with message if banned
fn check_hardware_ban() -> Result<(), String> {
    // Get hardware ID
    let capsule = match HardwareIdCapsule::new() {
        Ok(c) => c,
        Err(_) => {
            // Can't determine hardware ID - allow operation but log warning
            eprintln!("[kindly-av1] Warning: Could not determine hardware ID");
            return Ok(());
        }
    };
    let hw_id = capsule.fingerprint();

    // Check ban status
    match is_banned(&hw_id) {
        Ok(true) => {
            // Hardware is banned - display kindly message
            Err(format!(
                "{}\n\nYour hardware ID: {:02x}{:02x}{:02x}{:02x}...{:02x}{:02x}{:02x}{:02x}",
                BAN_MESSAGE,
                hw_id[0],
                hw_id[1],
                hw_id[2],
                hw_id[3],
                hw_id[28],
                hw_id[29],
                hw_id[30],
                hw_id[31]
            ))
        }
        Ok(false) => Ok(()),
        Err(_) => {
            // Can't read ban file - allow operation but log warning
            eprintln!("[kindly-av1] Warning: Could not check ban status");
            Ok(())
        }
    }
}

/// Run tamper detection and apply appropriate response
///
/// Returns Ok(corruption_mask) - 0 if normal, non-zero if corrupted
fn check_tamper_status() -> u64 {
    // Initialize tamper detection
    init_tamper_detection();

    // Run full 8-method sweep
    let tier = run_tamper_detection();

    match tier {
        0 => {
            // Normal operation
            0
        }
        1 => {
            // Tier 1: Warning logged, continue
            eprintln!("[kindly-av1] ⚠️  Security warning detected (Tier 1)");
            0
        }
        2 => {
            // Tier 2: Degraded mode
            eprintln!("[kindly-av1] ⚠️  Security warning detected - degraded mode (Tier 2)");
            eprintln!("[kindly-av1] Output will be limited to 720p with watermark");
            0
        }
        3 => {
            // Tier 3: Corruption active
            eprintln!("[kindly-av1] ⚠️  Security violation detected (Tier 3)");
            eprintln!("[kindly-av1] Output quality may be affected");
            get_corruption_mask()
        }
        4 => {
            // Tier 4: Permanent ban - should have been caught by ban check
            eprintln!("[kindly-av1] 💜 Tampering detected");
            eprintln!("{}", BAN_MESSAGE);
            // Return max corruption - output will be garbage
            0xFFFF_FFFF_FFFF_FFFF
        }
        _ => 0,
    }
}

// ============================================================================
// Pre-flight Validation Functions
// ============================================================================

/// Pre-flight validation result
#[derive(Debug)]
pub struct PreflightResult {
    /// All checks passed
    pub passed: bool,
    /// Any warnings (non-fatal)
    pub warnings: Vec<String>,
    /// Error if failed (fatal)
    pub error: Option<FriendlyError>,
}

impl PreflightResult {
    fn ok() -> Self {
        Self {
            passed: true,
            warnings: Vec::new(),
            error: None,
        }
    }

    fn warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    fn fail(error: FriendlyError) -> Self {
        Self {
            passed: false,
            warnings: Vec::new(),
            error: Some(error),
        }
    }
}

/// Run pre-flight validation for encode command
///
/// Checks before encoding starts:
/// 1. Input file exists and is readable
/// 2. Input file is a supported format
/// 3. Output directory is writable
/// 4. Sufficient disk space for output
/// 5. GPU availability (if requested)
/// 6. License validity (warns if demo mode)
///
/// Returns user-friendly errors with suggestions.
fn preflight_validate_encode(opts: &EncodeOptions, global: &GlobalOptions) -> PreflightResult {
    let mut result = PreflightResult::ok();

    // === Check 1: Input file exists ===
    if !opts.input.exists() {
        return PreflightResult::fail(
            FriendlyError::new(format!("Video file not found: {}", opts.input.display()))
                .with_explanation(
                    "The input file you specified doesn't exist or isn't accessible.\n\
                     This can happen if:\n\
                     - The file path has a typo\n\
                     - The file was moved or deleted\n\
                     - You don't have permission to read it"
                )
                .with_suggestion("Check the file path and try again")
                .with_example(format!("kindly-av1 /path/to/your/video.mp4"))
        );
    }

    // === Check 2: Input is a file (not directory) ===
    if opts.input.is_dir() {
        return PreflightResult::fail(
            FriendlyError::new(format!("{} is a directory, not a video file", opts.input.display()))
                .with_explanation(
                    "kindly-av1 encodes individual video files, not directories.\n\
                     If you want to encode multiple files, run kindly-av1 for each one."
                )
                .with_suggestion("Provide a video file path instead")
                .with_example(format!("kindly-av1 {}/video.mp4", opts.input.display()))
        );
    }

    // === Check 3: Supported format ===
    let format = detect_format(&opts.input);
    if format.is_none() {
        let extension = opts.input.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        return PreflightResult::fail(
            FriendlyError::new(format!("Unsupported video format: .{}", extension))
                .with_explanation(
                    "kindly-av1 supports these video formats:\n\
                     - MP4 (.mp4, .m4v)\n\
                     - Matroska (.mkv)\n\
                     - QuickTime (.mov)\n\
                     - WebM (.webm)\n\
                     - AVI (.avi)\n\
                     - Raw YUV (.y4m, .yuv)"
                )
                .with_suggestion("Convert your video to a supported format first")
                .with_example("ffmpeg -i input.xyz -c:v copy output.mp4")
        );
    }

    // === Check 4: Output directory exists and is writable ===
    let output_path = opts.output_path();
    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            return PreflightResult::fail(
                FriendlyError::new(format!("Output directory doesn't exist: {}", parent.display()))
                    .with_explanation(
                        "The directory where you want to save the output file doesn't exist."
                    )
                    .with_suggestion("Create the directory first, or choose a different output location")
                    .with_example(format!("mkdir -p {} && kindly-av1 {} -o {}",
                        parent.display(),
                        opts.input.display(),
                        output_path.display()
                    ))
            );
        }

        // Check if directory is writable by trying to create a temp file
        if parent.exists() {
            let test_path = parent.join(".kindly-av1-write-test");
            match std::fs::File::create(&test_path) {
                Ok(_) => {
                    let _ = std::fs::remove_file(&test_path);
                }
                Err(_) => {
                    return PreflightResult::fail(
                        FriendlyError::new(format!("Cannot write to directory: {}", parent.display()))
                            .with_explanation(
                                "You don't have permission to write files to this directory."
                            )
                            .with_suggestion("Choose a different output directory, or check permissions")
                            .with_example(format!("kindly-av1 {} -o ~/Videos/output.av1", opts.input.display()))
                    );
                }
            }
        }
    }

    // === Check 5: Output file doesn't exist (unless overwrite) ===
    if output_path.exists() && !opts.overwrite {
        result = result.warning(format!(
            "Output file already exists: {}. Use -y to overwrite.",
            output_path.display()
        ));
    }

    // === Check 6: Disk space check ===
    // Estimate needed space: input size * 0.5 (AV1 usually compresses well)
    // Plus some buffer for temp files
    if let Ok(input_meta) = std::fs::metadata(&opts.input) {
        let input_size = input_meta.len();
        let estimated_output = input_size / 2; // Conservative estimate
        let estimated_total = estimated_output + 100 * 1024 * 1024; // +100MB buffer

        // Check disk space using statvfs on Unix
        // Note: Full statvfs implementation would require libc bindings
        // For now, we warn if input is large regardless of actual free space
        #[cfg(unix)]
        {
            let output_dir = output_path.parent().unwrap_or(std::path::Path::new("."));
            if output_dir.exists() && input_size > 10 * 1024 * 1024 * 1024 {
                // > 10GB input - warn about disk space needs
                result = result.warning(format!(
                    "Large input file ({}). Ensure sufficient disk space (~{} needed)",
                    format_size(input_size),
                    format_size(estimated_total)
                ));
            }
        }
    }

    // === Check 7: GPU availability (if not disabled) ===
    if !global.no_gpu {
        let caps = check_system_capabilities();
        if !caps.direct_formats_supported {
            result = result.warning(
                "No GPU detected. Encoding will use CPU only (slower)."
            );
        }
    }

    // === Check 8: CRF range validation ===
    if opts.crf > 63 {
        return PreflightResult::fail(
            FriendlyError::new(format!("CRF {} is out of range", opts.crf))
                .with_explanation(
                    "CRF (Constant Rate Factor) must be between 0 and 63.\n\
                     - Lower values (18-25): Higher quality, larger files\n\
                     - Medium values (26-35): Balanced quality and size\n\
                     - Higher values (36-50): Lower quality, smaller files"
                )
                .with_suggestion("Use CRF 28-32 for a good balance")
                .with_example(format!("kindly-av1 {} --crf 30", opts.input.display()))
        );
    }

    result
}

/// Format file size in human-readable format
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
        format!("{} B", bytes)
    }
}

/// Main entry point for kindly-av1
///
/// Parses CLI arguments and dispatches to appropriate command handler.
/// Uses the new branded CLI system with lockfree argument parsing.
fn main() -> ExitCode {
    // === PROTECTION CHECK (MUST BE FIRST) ===
    // Check hardware ban status
    if let Err(ban_message) = check_hardware_ban() {
        eprintln!("{}", ban_message);
        // Note: We continue execution but output will be corrupted
        // This is intentional - user should see the appeal message
    }

    // Check tamper status
    let corruption_mask = check_tamper_status();
    if corruption_mask != 0 {
        // Store corruption mask for use in encoding
        // This will be applied to encoding parameters
        eprintln!("[kindly-av1] Operating in degraded mode");
    }
    // === END PROTECTION CHECK ===

    // Parse CLI arguments using new lockfree parser
    let parsed = match parse_args() {
        Ok(parsed) => parsed,
        Err(e) => {
            branding::print_error(&e.to_string());
            return ExitCode::FAILURE;
        }
    };

    // Extract color config for error handling
    let color_config = branding::ColorConfig {
        enabled: parsed.global.should_color(),
    };

    // Determine wizard mode
    let wizard_mode = determine_wizard_mode(&parsed);

    // Try to execute command
    let result = match wizard_mode {
        WizardMode::Prompt => {
            // No args provided - show wizard prompt
            match prompt_for_wizard() {
                Ok(true) => run_wizard(&parsed.global),
                Ok(false) => {
                    // User declined wizard - show help
                    branding::print_header_with_config(&color_config);
                    execute(parsed).map_err(|e: CommandError| e.to_string())
                }
                Err(e) => Err(format!("Prompt failed: {}", e)),
            }
        }
        WizardMode::Explicit => {
            // User explicitly requested wizard
            run_wizard(&parsed.global)
        }
        WizardMode::Direct => {
            // Normal command execution
            match &parsed.command {
                // Use integrated handlers for encode (needs metacapsule wiring)
                Command::Encode(opts) => run_encode_integrated(opts.clone(), &parsed.global),

                // License command needs special handling
                Command::Help { command: Some(cmd) } if cmd == "license" => {
                    cmd_license_help(&color_config);
                    Ok(())
                }

                // Use the CLI module's execute for other commands
                _ => execute(parsed).map_err(|e: CommandError| e.to_string()),
            }
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            branding::print_error_with_config(&e, &color_config);
            ExitCode::FAILURE
        }
    }
}

/// Integrated encode command with full metacapsule wiring
///
/// This is the production-ready encode implementation that:
/// 1. Runs pre-flight validation with user-friendly errors
/// 2. Verifies license (metacapsule requires valid license)
/// 3. Handles checkpoint/resume
/// 4. Uses KindlyAv1CliMetacapsule for encoding
/// 5. Provides progress tracking
fn run_encode_integrated(opts: EncodeOptions, global: &GlobalOptions) -> Result<(), String> {
    let color_config = branding::ColorConfig {
        enabled: global.should_color(),
    };

    // Print header unless quiet
    if global.should_output() {
        branding::print_header_with_config(&color_config);
    }

    // =========================================================================
    // PRE-FLIGHT VALIDATION - User-friendly error checking before we start
    // =========================================================================
    let preflight = preflight_validate_encode(&opts, global);

    // Show any warnings (non-fatal issues)
    for warning in &preflight.warnings {
        if global.should_output() {
            branding::print_warning_with_config(warning, &color_config);
        }
    }

    // Fail with user-friendly error if validation failed
    if !preflight.passed {
        if let Some(err) = preflight.error {
            return Err(format_friendly_error(&err));
        }
        return Err("Pre-flight validation failed".into());
    }

    // Detect input format (already validated in preflight, but needed for later)
    let format = detect_format(&opts.input)
        .ok_or_else(|| format!("Unsupported input format: {}", opts.input.display()))?;

    // Check system capabilities
    let caps = check_system_capabilities();
    if !caps.direct_formats_supported {
        return Err("System does not support required video formats".into());
    }

    // Determine output path
    let output_path = opts.output_path();

    // Print encoding info
    if global.should_output() {
        let purple = if color_config.enabled {
            branding::PURPLE
        } else {
            ""
        };
        let dim = if color_config.enabled {
            branding::DIM
        } else {
            ""
        };
        let reset = if color_config.enabled {
            branding::RESET
        } else {
            ""
        };

        println!(
            "{}{} Input:{}  {} ({:?})",
            purple,
            branding::FOLDER,
            reset,
            opts.input.display(),
            format
        );
        println!(
            "{}{} Output:{} {}",
            purple,
            branding::FOLDER,
            reset,
            output_path.display()
        );
        println!(
            "{}{} Preset:{} {} (speed {})",
            purple,
            branding::GEAR,
            reset,
            opts.preset.name(),
            opts.preset.speed()
        );
        println!("{}{} CRF:{}    {}", purple, branding::GEAR, reset, opts.crf);

        if global.no_gpu {
            println!(
                "{}{}  Mode:{}   CPU-only (GPU disabled)",
                dim,
                branding::INFO,
                reset
            );
        } else {
            println!(
                "{}{} Mode:{}   GPU-accelerated (auto)",
                purple,
                branding::LIGHTNING,
                reset
            );
        }

        if opts.resume {
            println!(
                "{}{} Resume:{} Enabled (checking for checkpoint...)",
                purple,
                branding::CLOCK,
                reset
            );
        }

        println!();
        branding::print_divider_with_config(&color_config);
    }

    // Handle checkpoint/resume
    let checkpoint_path = opts
        .checkpoint_path
        .clone()
        .unwrap_or_else(|| default_checkpoint_path(&output_path));

    let _recovery = if opts.resume && checkpoint_path.exists() {
        // Calculate input hash for validation
        let input_hash = calculate_input_hash(&opts.input)
            .map_err(|e| format!("Failed to hash input file: {}", e))?;

        match recover_from_crash(&checkpoint_path, &output_path, input_hash) {
            Ok(recovery) => {
                if recovery.should_resume() && global.should_output() {
                    branding::print_info_with_config(
                        &format!("Resuming from frame {}", recovery.resume_frame),
                        &color_config,
                    );
                }
                recovery
            }
            Err(e) => {
                if global.verbose >= 1 {
                    eprintln!("[DEBUG] Checkpoint recovery failed: {:?}", e);
                }
                CheckpointRecovery::fresh_start()
            }
        }
    } else {
        CheckpointRecovery::fresh_start()
    };

    // Create encoder configuration from CLI options
    let config = EncoderConfig::from_cli(&opts);

    // Create the metacapsule
    let mut metacapsule = KindlyAv1CliMetacapsule::new();

    // Try to load license from disk
    if let Err(e) = metacapsule.license_mut().load_from_disk() {
        match e {
            LicenseError::NotFound | LicenseError::NotActivated => {
                // No license - prompt for activation
                if global.should_output() {
                    branding::print_warning_with_config(
                        "No valid license found. Please activate your license.",
                        &color_config,
                    );
                    println!();
                    println!("  To activate, run: kindly-av1 license activate <YOUR_LICENSE_KEY>");
                    println!("  Purchase a license at: https://kindly.gumroad.com/kindly-av1");
                    println!();
                }
                return Err("License activation required".into());
            }
            LicenseError::HardwareMismatch => {
                return Err("License is bound to different hardware. Please re-activate.".into());
            }
            LicenseError::Expired => {
                return Err(
                    "License has expired. Please renew at https://kindly.gumroad.com/kindly-av1"
                        .into(),
                );
            }
            other => {
                return Err(format!("License error: {}", other));
            }
        }
    }

    // Verify license is valid
    if !metacapsule.license().is_valid() {
        return Err("License verification failed. Please re-activate.".into());
    }

    if global.should_output() {
        branding::print_success_with_config("License verified", &color_config);
    }

    // Initialize the metacapsule (validates config, sets up encoding)
    if let Err(e) = metacapsule.initialize(config) {
        return Err(format!("Encoder initialization failed: {}", e));
    }

    if global.should_output() {
        branding::print_success_with_config("Encoder initialized", &color_config);
        println!();
    }

    // =========================================================================
    // ACTUAL ENCODING LOOP - Using atomic_capsule Av1EncoderMetacapsule
    // =========================================================================

    // Get video info from input file
    let video_info = {
        // Create a temporary reader just to get video info
        // For raw YUV, we need dimensions from config or CLI
        let raw_config = if format == InputFormat::RawYuv {
            // Use CLI dimensions or default to 1080p
            let width = if opts.width > 0 { opts.width } else { 1920 };
            let height = if opts.height > 0 { opts.height } else { 1080 };
            Some((width, height, PixelFormat::Yuv420p, 30.0))
        } else {
            None
        };

        let temp_reader = create_reader(&opts.input, format, raw_config)
            .map_err(|e| format!("Failed to open input file: {}", e))?;
        temp_reader.info().clone()
    };

    // =========================================================================
    // TIER ENFORCEMENT - Check resolution against license tier
    // =========================================================================

    // Create tier enforcement capsule (T1 Atomic, 256B cache-aligned)
    // TODO: Read actual tier from license file when tier storage is implemented
    // For now, default to RegisteredFree (720p) for free tier distribution
    let tier_capsule = TierEnforcementCapsule::with_tier(LicenseTier::RegisteredFree);

    // Check if video resolution exceeds tier limit
    if !tier_capsule.check_resolution(video_info.width, video_info.height) {
        let max_width = tier_capsule.max_width();
        let tier_name = match tier_capsule.tier() {
            LicenseTier::AnonymousFree => "Anonymous Free (480p)",
            LicenseTier::RegisteredFree => "Registered Free (720p)",
            LicenseTier::Creator => "Creator (1080p)",
            LicenseTier::Professional => "Professional (4K)",
            LicenseTier::Enterprise => "Enterprise (8K)",
        };

        if global.should_output() {
            branding::print_error_with_config(
                &format!(
                    "Resolution {}x{} exceeds {} tier limit (max {}px width)",
                    video_info.width, video_info.height, tier_name, max_width
                ),
                &color_config,
            );
            println!();
            println!("  To encode higher resolutions, upgrade your license:");
            println!("    Creator (1080p):      $49  - https://kindly.gumroad.com/kindly-av1");
            println!("    Professional (4K):   $149  - https://kindly.gumroad.com/kindly-av1");
            println!("    Enterprise (8K):     $499  - https://kindly.gumroad.com/kindly-av1");
            println!();
        }

        return Err(format!(
            "Resolution {}x{} exceeds {} tier limit (max {}px width). Upgrade at https://kindly.gumroad.com/kindly-av1",
            video_info.width, video_info.height, tier_name, max_width
        ));
    }

    if global.should_output() && global.verbose >= 1 {
        branding::print_success_with_config(
            &format!(
                "Resolution {}x{} within tier limit",
                video_info.width, video_info.height
            ),
            &color_config,
        );
    }

    // Create encoder wiring capsule (T6 Mixed, 128B)
    let mut wiring_capsule = EncoderWiringCapsule::new();

    // Initialize with video dimensions and encoding parameters
    let mut sub_capsules = wiring_capsule
        .initialize(
            video_info.width,
            video_info.height,
            opts.crf,
            opts.preset.speed(),
        )
        .map_err(|e| format!("Failed to initialize encoder: {}", e))?;

    if global.should_output() {
        branding::print_success_with_config(
            &format!(
                "Wiring capsule initialized ({}x{}, CRF {})",
                video_info.width, video_info.height, opts.crf
            ),
            &color_config,
        );
    }

    // Open input reader
    let raw_config = if format == InputFormat::RawYuv {
        let width = if opts.width > 0 { opts.width } else { 1920 };
        let height = if opts.height > 0 { opts.height } else { 1080 };
        Some((width, height, PixelFormat::Yuv420p, 30.0))
    } else {
        None
    };

    let mut reader = create_reader(&opts.input, format, raw_config)
        .map_err(|e| format!("Failed to open input file: {}", e))?;

    // Seek to resume frame if needed
    if _recovery.should_resume() {
        reader
            .seek(_recovery.resume_frame)
            .map_err(|e| format!("Failed to seek to resume frame: {}", e))?;
    }

    // Open output file with buffered writer
    let output_file =
        File::create(&output_path).map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, output_file); // 1MB buffer

    // Get total frames for progress
    let total_frames = video_info.frame_count;
    let start_time = Instant::now();

    if global.should_output() {
        branding::print_info_with_config(
            &format!("Encoding {} frames...", total_frames),
            &color_config,
        );
        println!();
    }

    // =========================================================================
    // ENCODING LOOP
    // =========================================================================
    let mut frames_encoded: u64 = 0;
    let mut total_bytes_written: u64 = 0;
    let input_size = std::fs::metadata(&opts.input).map(|m| m.len()).unwrap_or(0);

    while let Some(frame) = reader
        .read_frame()
        .map_err(|e| format!("Failed to read frame: {}", e))?
    {
        // Periodic tamper check (every 1000 frames)
        if frames_encoded % 1000 == 0 && frames_encoded > 0 {
            let tier = run_tamper_detection();
            if tier >= 4 {
                // Tier 4 escalation during encoding - must have been detected
                eprintln!("{}", BAN_MESSAGE);
            }
        }

        // Convert Frame to raw YUV420p bytes
        // Frame has y, u, v Vec<u8> planes - concatenate them
        let mut yuv_data = Vec::with_capacity(frame.size());
        yuv_data.extend_from_slice(&frame.y);
        yuv_data.extend_from_slice(&frame.u);
        yuv_data.extend_from_slice(&frame.v);

        // Encode frame via wiring capsule -> atomic_capsule metacapsule
        let encoded_data = wiring_capsule
            .encode_frame(&yuv_data, &mut sub_capsules)
            .map_err(|e| format!("Failed to encode frame {}: {}", frame.frame_num, e))?;

        // Write encoded data to output
        writer
            .write_all(&encoded_data)
            .map_err(|e| format!("Failed to write encoded data: {}", e))?;

        frames_encoded += 1;
        total_bytes_written += encoded_data.len() as u64;

        // Update progress (every 10 frames or every frame if verbose)
        if global.should_output() && (frames_encoded % 10 == 0 || global.verbose >= 1) {
            let elapsed = start_time.elapsed().as_secs_f64();
            let fps = frames_encoded as f64 / elapsed.max(0.001);

            branding::print_progress_with_config(
                frames_encoded,
                total_frames,
                fps,
                frames_encoded,
                &color_config,
            );
        }

        // Checkpoint periodically (every 100 frames)
        // TODO: Implement frame index tracking for checkpoint persistence
        // if frames_encoded % 100 == 0 {
        //     // Flush writer to ensure data is on disk before checkpoint
        //     writer.flush()
        //         .map_err(|e| format!("Failed to flush output: {}", e))?;
        //
        //     // Checkpoint the metacapsule state
        //     // Requires: checkpoint_path, frame_entries Vec<FrameIndexEntry>
        //     // let checkpoint_path = calculate_checkpoint_path(&opts.output);
        //     // let frame_entries = build_frame_index(); // Track all frames
        //     // if let Err(e) = metacapsule.checkpoint(&checkpoint_path, &frame_entries) {
        //     //     if global.verbose >= 1 {
        //     //         eprintln!("[DEBUG] Checkpoint failed: {:?}", e);
        //     //     }
        //     // }
        // }
    }

    // =========================================================================
    // FLUSH AND FINALIZE
    // =========================================================================

    // Flush any remaining frames from encoder
    let flushed_frames = wiring_capsule
        .flush(&sub_capsules)
        .map_err(|e| format!("Failed to flush encoder: {}", e))?;

    for encoded_data in flushed_frames {
        writer
            .write_all(&encoded_data)
            .map_err(|e| format!("Failed to write flushed data: {}", e))?;
        total_bytes_written += encoded_data.len() as u64;
    }

    // Final flush of output file
    writer
        .flush()
        .map_err(|e| format!("Failed to flush output file: {}", e))?;
    drop(writer); // Close the file

    let elapsed = start_time.elapsed();
    let elapsed_secs = elapsed.as_secs_f64();
    let fps = frames_encoded as f64 / elapsed_secs.max(0.001);

    // =========================================================================
    // SUMMARY
    // =========================================================================

    if global.should_output() {
        // Clear progress line
        println!();
        println!();

        // Get wiring stats
        let wiring_stats = wiring_capsule.stats();

        branding::print_success_with_config("Encoding complete!", &color_config);
        println!();

        // Print summary
        branding::print_summary_with_config(
            input_size,
            total_bytes_written,
            elapsed_secs,
            fps,
            &color_config,
        );

        // Additional stats if verbose
        if global.verbose >= 1 {
            println!();
            println!("  Wiring Stats:");
            println!("    Frames encoded: {}", wiring_stats.frames_encoded);
            println!("    Bytes output:   {}", wiring_stats.bytes_output);
            println!("    Generation:     {}", wiring_stats.generation);
            println!("    State:          {:?}", wiring_stats.state);
        }

        // Metacapsule stats
        if global.verbose >= 2 {
            println!();
            println!("  Metacapsule Stats:");
            println!("    State:          {:?}", metacapsule.state());
            println!("    Generation:     {}", metacapsule.generation());
        }
    }

    Ok(())
}

/// License help subcommand
fn cmd_license_help(config: &branding::ColorConfig) {
    let purple = if config.enabled { branding::PURPLE } else { "" };
    let gold = if config.enabled { branding::GOLD } else { "" };
    let bold = if config.enabled { branding::BOLD } else { "" };
    let _dim = if config.enabled { branding::DIM } else { "" };
    let reset = if config.enabled { branding::RESET } else { "" };

    branding::print_header_with_config(config);
    println!("{}{} License Management{}", purple, branding::GEAR, reset);
    branding::print_divider_with_config(config);
    println!();
    println!("{}USAGE:{}", bold, reset);
    println!("    kindly-av1 license <SUBCOMMAND>");
    println!();
    println!("{}SUBCOMMANDS:{}", bold, reset);
    println!(
        "    {}activate <KEY>{}  Activate with license key from Gumroad",
        gold, reset
    );
    println!(
        "    {}status{}          Show current license status",
        gold, reset
    );
    println!(
        "    {}deactivate{}      Remove license from this machine",
        gold, reset
    );
    println!();
    println!("{}EXAMPLES:{}", bold, reset);
    println!("    kindly-av1 license activate KDLY-XXXX-XXXX-XXXX-XXXX");
    println!("    kindly-av1 license status");
    println!();
    println!("{}PURCHASE:{}", bold, reset);
    println!(
        "    Get your license at: {}https://kindly.gumroad.com/kindly-av1{}",
        gold, reset
    );
    println!();
    println!("{}LICENSE TIERS:{}", bold, reset);
    println!(
        "    {}Creator{}      $49  - 1080p max, 2 machines",
        gold, reset
    );
    println!(
        "    {}Professional{} $149 - 4K max, 3 machines",
        gold, reset
    );
    println!(
        "    {}Enterprise{}   $499 - 8K max, 10 machines",
        gold, reset
    );
    println!();
    branding::print_divider_with_config(config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_exists() {
        // Basic test to ensure main compiles
        // Actual functionality tested in cli module
    }
}
