//! Encode command implementation with dashboard integration
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! This module implements the encode command with full dashboard integration.
//! It coordinates the DashboardRunner with the encoding pipeline.
//!
//! ## Architecture
//!
//! ```text
//! run_encode()
//! ├── DashboardRunner (coordinates UI)
//! │   ├── InteractiveStateCapsule (shared with encoder)
//! │   ├── DashboardRendererCapsule (rendering)
//! │   └── DefaultKeyboardHandler (input)
//! └── Encoding loop
//!     ├── Poll keyboard (non-blocking)
//!     ├── Handle pause/resume
//!     ├── Check cancellation
//!     ├── Encode batch
//!     └── Update progress
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Coordinate T1+T5 capsules via DashboardRunner
//! - **Chaos**: Share state via Arc<InteractiveStateCapsule>, lockfree
//! - **ASSUM**: All io::Result for safe terminal handling
//! - **T28**: Integration tests for full encoding flow

use std::io::{self, Result};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::progress::{DashboardRunner, FinalStats, KeyAction, ProgressSnapshot};

/// Encode command arguments
///
/// This struct holds all configuration for an encoding session.
/// It is typically populated from CLI arguments via clap.
#[derive(Debug, Clone)]
pub struct EncodeArgs {
    /// Input video file path
    pub input: String,
    /// Output AV1 file path
    pub output: String,
    /// Encoding preset (ultrafast/superfast/veryfast/faster/fast/medium/slow/slower/veryslow)
    pub preset: String,
    /// Constant Rate Factor (0-63, lower = better quality)
    pub crf: u8,
    /// GPU acceleration enabled
    pub gpu_enabled: bool,
    /// Optional checkpoint file for resume
    pub checkpoint: Option<String>,
    /// Resume from checkpoint
    pub resume: bool,
}

impl Default for EncodeArgs {
    fn default() -> Self {
        Self {
            input: String::new(),
            output: String::new(),
            preset: "medium".to_string(),
            crf: 28,
            gpu_enabled: true,
            checkpoint: None,
            resume: false,
        }
    }
}

/// Run the encode command with dashboard
///
/// This is the main entry point for the encode command.
/// It creates the dashboard runner and executes the encoding loop.
///
/// # Arguments
///
/// - `args`: Encode command arguments
///
/// # Returns
///
/// `Ok(())` on successful encoding, `Err(io::Error)` on failure.
///
/// # Example
///
/// ```ignore
/// let args = EncodeArgs {
///     input: "input.mp4".to_string(),
///     output: "output.av1".to_string(),
///     preset: "medium".to_string(),
///     crf: 28,
///     gpu_enabled: true,
///     checkpoint: None,
///     resume: false,
/// };
///
/// run_encode(args)?;
/// ```
pub fn run_encode(args: EncodeArgs) -> Result<()> {
    // TODO: Get actual resolution from input file
    let resolution = format!("{}@{}", "720p", "60fps");

    // Create dashboard runner
    let mut dashboard = DashboardRunner::new(&args.input, &args.output, &resolution)?;
    dashboard.start()?;

    // Share interactive state with encoder (for future integration)
    let _interactive = dashboard.interactive_state();

    // Main encoding loop
    let result = encode_with_dashboard(&mut dashboard, &args);

    // Always stop dashboard before returning (even on error)
    dashboard.stop()?;

    result
}

/// Encode with dashboard integration (skeleton implementation)
///
/// This function demonstrates the integration pattern between the dashboard
/// and encoding pipeline. The actual encoding logic will be integrated later.
///
/// # Arguments
///
/// - `dashboard`: Dashboard runner (mutable for updates)
/// - `args`: Encode command arguments
///
/// # Returns
///
/// `Ok(())` on successful encoding, `Err(io::Error)` on failure.
fn encode_with_dashboard(dashboard: &mut DashboardRunner, args: &EncodeArgs) -> Result<()> {
    let start = Instant::now();
    let mut frames_encoded: u64 = 0;
    let total_frames: u64 = 2400; // TODO: Get from input file metadata

    // Simulate encoding loop (SKELETON - real encoder integration later)
    loop {
        // Poll keyboard (non-blocking, <10ms)
        if let Some(action) = dashboard.poll_and_update()? {
            match action {
                KeyAction::Cancel => {
                    // User requested cancellation
                    eprintln!("Encoding cancelled by user");
                    return Ok(());
                }
                KeyAction::SaveCheckpoint if dashboard.should_pause() => {
                    // Save checkpoint when paused
                    if let Some(checkpoint_path) = &args.checkpoint {
                        eprintln!("Saving checkpoint to: {}", checkpoint_path);
                        // TODO: checkpoint.save(checkpoint_path)?;
                    }
                }
                _ => {} // Other actions handled internally by dashboard
            }
        }

        // Handle pause state
        if dashboard.should_pause() {
            let progress = make_progress_snapshot(frames_encoded, total_frames, start.elapsed());
            dashboard.show_paused(&progress);
            std::thread::sleep(Duration::from_millis(50));
            continue;
        }

        // Check cancellation flag
        if dashboard.should_cancel() {
            eprintln!("Encoding cancelled");
            break;
        }

        // Encode next batch
        // TODO: Actually encode frames using KindlyAv1CliMetacapsule
        // For now, simulate encoding with sleep
        frames_encoded += 10; // Simulate 10 frames per batch
        std::thread::sleep(Duration::from_millis(10)); // Simulate encoding work

        // Update progress display
        let progress = make_progress_snapshot(frames_encoded, total_frames, start.elapsed());
        dashboard.update_progress(&progress);

        // Check if encoding is complete
        if frames_encoded >= total_frames {
            let stats = make_final_stats(frames_encoded, start.elapsed());
            dashboard.show_complete(&stats);

            // Wait for user to exit
            loop {
                if let Some(KeyAction::Exit) = dashboard.poll_and_update()? {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            break;
        }
    }

    Ok(())
}

/// Create progress snapshot from current state
///
/// # Arguments
///
/// - `frames`: Frames encoded so far
/// - `total`: Total frame count
/// - `elapsed`: Elapsed time since encoding started
///
/// # Returns
///
/// `ProgressSnapshot` with calculated metrics.
fn make_progress_snapshot(frames: u64, total: u64, elapsed: Duration) -> ProgressSnapshot {
    let secs = elapsed.as_secs_f64();
    let fps = if secs > 0.0 {
        frames as f64 / secs
    } else {
        0.0
    };
    let remaining = total.saturating_sub(frames);
    let eta = if fps > 0.0 {
        remaining as f64 / fps
    } else {
        0.0
    };

    ProgressSnapshot {
        frames_encoded: frames,
        total_frames: total,
        fps,
        eta_seconds: eta,
        psnr: 42.1,           // TODO: Real metrics from encoder
        ssim: 0.987,          // TODO: Real metrics from encoder
        bitrate_mbps: 2.4,    // TODO: Real metrics from encoder
        gpu_percent: 94,      // TODO: Real GPU utilization
        bytes_written: frames * 1000, // Estimate (1KB per frame)
        input_size: total * 2000,     // Estimate (2KB per frame)
    }
}

/// Create final statistics from encoding session
///
/// # Arguments
///
/// - `frames`: Total frames encoded
/// - `elapsed`: Total encoding time
///
/// # Returns
///
/// `FinalStats` with encoding summary.
fn make_final_stats(frames: u64, elapsed: Duration) -> FinalStats {
    let duration_secs = elapsed.as_secs_f64();
    let avg_fps = if duration_secs > 0.0 {
        frames as f64 / duration_secs
    } else {
        0.0
    };

    FinalStats {
        total_frames: frames,
        duration_seconds: duration_secs,
        avg_fps,
        avg_psnr: 42.1,          // TODO: Real average from encoder
        avg_ssim: 0.987,         // TODO: Real average from encoder
        compression_ratio: 2.85, // TODO: Real ratio from actual sizes
        input_size: frames * 2000,
        output_size: frames * 700,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_args_default() {
        let args = EncodeArgs::default();
        assert_eq!(args.preset, "medium");
        assert_eq!(args.crf, 28);
        assert_eq!(args.gpu_enabled, true);
        assert_eq!(args.resume, false);
    }

    #[test]
    fn test_make_progress_snapshot() {
        let snapshot = make_progress_snapshot(100, 1000, Duration::from_secs(10));

        assert_eq!(snapshot.frames_encoded, 100);
        assert_eq!(snapshot.total_frames, 1000);
        assert!((snapshot.fps - 10.0).abs() < 0.1);
        assert!((snapshot.eta_seconds - 90.0).abs() < 1.0);
    }

    #[test]
    fn test_make_progress_snapshot_zero_elapsed() {
        let snapshot = make_progress_snapshot(0, 1000, Duration::from_secs(0));

        assert_eq!(snapshot.frames_encoded, 0);
        assert_eq!(snapshot.total_frames, 1000);
        assert_eq!(snapshot.fps, 0.0);
        assert_eq!(snapshot.eta_seconds, 0.0);
    }

    #[test]
    fn test_make_final_stats() {
        let stats = make_final_stats(1000, Duration::from_secs(100));

        assert_eq!(stats.total_frames, 1000);
        assert!((stats.duration_seconds - 100.0).abs() < 0.1);
        assert!((stats.avg_fps - 10.0).abs() < 0.1);
        assert_eq!(stats.input_size, 2_000_000);
        assert_eq!(stats.output_size, 700_000);
    }

    #[test]
    fn test_make_final_stats_zero_duration() {
        let stats = make_final_stats(100, Duration::from_secs(0));

        assert_eq!(stats.total_frames, 100);
        assert_eq!(stats.duration_seconds, 0.0);
        assert_eq!(stats.avg_fps, 0.0); // Avoid division by zero
    }
}
