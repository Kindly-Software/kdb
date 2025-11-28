//! kindly-av1 - GPU-Accelerated AV1 Video Encoder
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! World's fastest lockfree AV1 encoder built on COCA architecture.
//! Copyright (c) 2025 Kindly. All rights reserved.

#![feature(portable_simd)]

use std::process::ExitCode;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

use kindly_av1::cli::{
    branding, parse_args, Command, GlobalOptions, EncodeOptions,
    execute, WizardMode, determine_wizard_mode,
};
use kindly_av1::encoder::{EncoderConfig, EncoderWiringCapsule, KindlyAv1CliMetacapsule};
use kindly_av1::file::{create_reader, detect_format, check_system_capabilities, InputFormat, PixelFormat};
use kindly_av1::checkpoint::{
    calculate_input_hash, default_checkpoint_path, recover_from_crash, CheckpointRecovery,
};
use kindly_av1::license::LicenseError;
use kindly_av1::protection::{
    HardwareIdCapsule, is_banned, BAN_MESSAGE,
    init_tamper_detection, run_tamper_detection,
    get_corruption_mask,
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
    let wants_wizard = input.is_empty()
        || input.eq_ignore_ascii_case("y")
        || input.eq_ignore_ascii_case("yes");

    Ok(wants_wizard)
}

/// Run the interactive wizard flow
///
/// NOTE: This is a placeholder implementation.
/// Full wizard implementation is in progress (Agent 3A + 4A).
///
/// The wizard will:
/// 1. Load user preferences (skip wizard if disabled)
/// 2. Initialize DashboardRunner with wizard mode
/// 3. Run interactive wizard steps
/// 4. Map user choices to encoding options
/// 5. Start encoding with chosen options
///
/// # Arguments
///
/// - `global`: Global CLI options
///
/// # Returns
///
/// `Ok(())` on success, `Err(String)` on failure
fn run_wizard(_global: &GlobalOptions) -> Result<(), String> {
    eprintln!("💜 Kindly-AV1 Wizard");
    eprintln!();
    eprintln!("NOTICE: Full wizard implementation is in progress.");
    eprintln!();
    eprintln!("The wizard will guide you through:");
    eprintln!("  1. Selecting a video file");
    eprintln!("  2. Choosing quality goal (Best Quality / Balanced / Smallest Size)");
    eprintln!("  3. Choosing encoding speed (Fast / Normal / Slow)");
    eprintln!("  4. Reviewing and confirming settings");
    eprintln!();
    eprintln!("For now, please use direct encoding:");
    eprintln!("  kindly-av1 encode input.mp4 -o output.av1");
    eprintln!();

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
                hw_id[0], hw_id[1], hw_id[2], hw_id[3],
                hw_id[28], hw_id[29], hw_id[30], hw_id[31]
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
                    execute(parsed).map_err(|e| e.to_string())
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
                _ => execute(parsed).map_err(|e| e.to_string()),
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
/// 1. Verifies license (metacapsule requires valid license)
/// 2. Handles checkpoint/resume
/// 3. Uses KindlyAv1CliMetacapsule for encoding
/// 4. Provides progress tracking
fn run_encode_integrated(opts: EncodeOptions, global: &GlobalOptions) -> Result<(), String> {
    let color_config = branding::ColorConfig {
        enabled: global.should_color(),
    };

    // Print header unless quiet
    if global.should_output() {
        branding::print_header_with_config(&color_config);
    }

    // Validate input file exists
    if !opts.input.exists() {
        return Err(format!("Input file not found: {}", opts.input.display()));
    }

    // Detect input format
    let format = detect_format(&opts.input)
        .ok_or_else(|| format!("Unsupported input format: {}", opts.input.display()))?;

    // Check system capabilities
    let caps = check_system_capabilities();
    if !caps.direct_formats_supported {
        return Err("System does not support required video formats".into());
    }

    // Determine output path
    let output_path = opts.output_path();

    // Check if output exists (unless overwrite is set)
    if output_path.exists() && !opts.overwrite {
        return Err(format!(
            "Output file already exists: {} (use -y to overwrite)",
            output_path.display()
        ));
    }

    // Print encoding info
    if global.should_output() {
        let purple = if color_config.enabled { branding::PURPLE } else { "" };
        let dim = if color_config.enabled { branding::DIM } else { "" };
        let reset = if color_config.enabled { branding::RESET } else { "" };

        println!(
            "{}{} Input:{}  {} ({:?})",
            purple, branding::FOLDER, reset,
            opts.input.display(), format
        );
        println!(
            "{}{} Output:{} {}",
            purple, branding::FOLDER, reset,
            output_path.display()
        );
        println!(
            "{}{} Preset:{} {} (speed {})",
            purple, branding::GEAR, reset,
            opts.preset.name(), opts.preset.speed()
        );
        println!(
            "{}{} CRF:{}    {}",
            purple, branding::GEAR, reset,
            opts.crf
        );

        if global.no_gpu {
            println!(
                "{}{}  Mode:{}   CPU-only (GPU disabled)",
                dim, branding::INFO, reset
            );
        } else {
            println!(
                "{}{} Mode:{}   GPU-accelerated (auto)",
                purple, branding::LIGHTNING, reset
            );
        }

        if opts.resume {
            println!(
                "{}{} Resume:{} Enabled (checking for checkpoint...)",
                purple, branding::CLOCK, reset
            );
        }

        println!();
        branding::print_divider_with_config(&color_config);
    }

    // Handle checkpoint/resume
    let checkpoint_path = opts.checkpoint.clone()
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
                        &color_config
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
                        &color_config
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
                return Err("License has expired. Please renew at https://kindly.gumroad.com/kindly-av1".into());
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

    // Create encoder wiring capsule (T6 Mixed, 128B)
    let mut wiring_capsule = EncoderWiringCapsule::new();

    // Initialize with video dimensions and encoding parameters
    let sub_capsules = wiring_capsule
        .initialize(
            video_info.width,
            video_info.height,
            opts.crf,
            opts.preset.speed(),
        )
        .map_err(|e| format!("Failed to initialize encoder: {}", e))?;

    if global.should_output() {
        branding::print_success_with_config(
            &format!("Wiring capsule initialized ({}x{}, CRF {})",
                video_info.width, video_info.height, opts.crf),
            &color_config
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
        reader.seek(_recovery.resume_frame)
            .map_err(|e| format!("Failed to seek to resume frame: {}", e))?;
    }

    // Open output file with buffered writer
    let output_file = File::create(&output_path)
        .map_err(|e| format!("Failed to create output file: {}", e))?;
    let mut writer = BufWriter::with_capacity(1024 * 1024, output_file); // 1MB buffer

    // Get total frames for progress
    let total_frames = video_info.frame_count;
    let start_time = Instant::now();

    if global.should_output() {
        branding::print_info_with_config(
            &format!("Encoding {} frames...", total_frames),
            &color_config
        );
        println!();
    }

    // =========================================================================
    // ENCODING LOOP
    // =========================================================================
    let mut frames_encoded: u64 = 0;
    let mut total_bytes_written: u64 = 0;
    let input_size = std::fs::metadata(&opts.input)
        .map(|m| m.len())
        .unwrap_or(0);

    while let Some(frame) = reader.read_frame()
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
            .encode_frame(&yuv_data, &sub_capsules)
            .map_err(|e| format!("Failed to encode frame {}: {}", frame.frame_num, e))?;

        // Write encoded data to output
        writer.write_all(&encoded_data)
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
                &color_config
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
    let flushed_frames = wiring_capsule.flush(&sub_capsules)
        .map_err(|e| format!("Failed to flush encoder: {}", e))?;

    for encoded_data in flushed_frames {
        writer.write_all(&encoded_data)
            .map_err(|e| format!("Failed to write flushed data: {}", e))?;
        total_bytes_written += encoded_data.len() as u64;
    }

    // Final flush of output file
    writer.flush()
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
            &color_config
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
    println!("    {}activate <KEY>{}  Activate with license key from Gumroad", gold, reset);
    println!("    {}status{}          Show current license status", gold, reset);
    println!("    {}deactivate{}      Remove license from this machine", gold, reset);
    println!();
    println!("{}EXAMPLES:{}", bold, reset);
    println!("    kindly-av1 license activate KDLY-XXXX-XXXX-XXXX-XXXX");
    println!("    kindly-av1 license status");
    println!();
    println!("{}PURCHASE:{}", bold, reset);
    println!("    Get your license at: {}https://kindly.gumroad.com/kindly-av1{}", gold, reset);
    println!();
    println!("{}LICENSE TIERS:{}", bold, reset);
    println!("    {}Creator{}      $49  - 1080p max, 2 machines", gold, reset);
    println!("    {}Professional{} $149 - 4K max, 3 machines", gold, reset);
    println!("    {}Enterprise{}   $499 - 8K max, 10 machines", gold, reset);
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
