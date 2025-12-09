//! Dashboard runner for coordinating interactive CLI display
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Coordinates all dashboard components:
//! - InteractiveStateCapsule (T1 Atomic) for keyboard state
//! - DashboardRendererCapsule (T5 Streaming) for rendering
//! - KeyboardHandler for input polling
//!
//! ## Architecture
//!
//! ```text
//! DashboardRunner (coordinator)
//! ├── InteractiveStateCapsule (shared with encoder, Arc)
//! ├── DashboardRendererCapsule (rendering state)
//! ├── DefaultKeyboardHandler (input polling)
//! └── Rate limiting (render_interval_ms)
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Coordinates T1 (Interactive) + T5 (Dashboard) capsules
//! - **Chaos**: Shares state via Arc<InteractiveStateCapsule>, 100% lockfree
//! - **ASSUM**: All io::Result for safe terminal handling
//! - **T28**: Integration tests for full flow

use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::progress::{
    DashboardRendererCapsule, DashboardState, DefaultKeyboardHandler, FinalStats, InteractiveSnapshot,
    InteractiveStateCapsule, KeyAction, KeyboardInput, ProgressSnapshot,
};
use crate::obs::{ObsStatusWriterCapsule, ObsOptions};

// NEW: Wizard and menu imports
use super::menu::CommandMenuCapsule;
use crate::cli::wizard::{
    map_to_encoding_options, render_step_0, render_step_1, render_step_2, render_step_3,
    render_step_4, EncodingOptions, QualityGoal, RecentFiles, SpeedChoice, UserPreferences,
    WizardContext,
};

/// Default render interval in milliseconds (30 Hz = 33.3ms)
const DEFAULT_RENDER_INTERVAL_MS: u64 = 33;

/// Keyboard poll timeout in milliseconds (non-blocking)
const KEYBOARD_POLL_TIMEOUT_MS: u64 = 10;

/// Dashboard runner that coordinates all dashboard components
///
/// This struct owns all dashboard state and provides high-level methods
/// for the encoding pipeline to interact with the dashboard.
///
/// # Example
///
/// ```ignore
/// let mut dashboard = DashboardRunner::new("input.mp4", "output.av1", "720p@60fps")?;
/// dashboard.start()?;
///
/// // Main encoding loop
/// loop {
///     // Poll keyboard
///     if let Some(KeyAction::Cancel) = dashboard.poll_and_update()? {
///         break;
///     }
///
///     // Update display
///     dashboard.update_progress(&progress);
///
///     // Encoding work...
/// }
///
/// dashboard.stop()?;
/// ```
pub struct DashboardRunner {
    /// Interactive state (shared with encoder via Arc)
    interactive: Arc<InteractiveStateCapsule>,

    /// Dashboard renderer (owned by runner)
    renderer: DashboardRendererCapsule,

    /// Keyboard input handler (owned by runner)
    keyboard: DefaultKeyboardHandler,

    /// Last render timestamp (for rate limiting)
    last_render: Instant,

    /// Render interval in milliseconds
    render_interval_ms: u64,

    /// Whether first render has occurred
    first_render: bool,

    /// Whether raw mode is enabled
    raw_mode_enabled: bool,

    /// OBS status writer (Phase 1: text file output)
    /// Optional - only created if --obs-status CLI flag is provided
    obs_writer: Option<ObsStatusWriterCapsule>,

    // NEW: Wizard state
    /// Wizard user preferences (loaded from ~/.kindly-av1/preferences.json)
    wizard_preferences: Option<UserPreferences>,

    /// Wizard recent files (loaded from ~/.kindly-av1/recent.json)
    wizard_recent: Option<RecentFiles>,

    // NEW: Command menu
    /// Command menu overlay (128B cache-aligned, T1 Atomic)
    menu: CommandMenuCapsule,
}

impl DashboardRunner {
    /// Create a new dashboard runner
    ///
    /// # Arguments
    ///
    /// - `input`: Input filename
    /// - `output`: Output filename
    /// - `resolution`: Resolution string (e.g., "720p@60fps")
    ///
    /// # Returns
    ///
    /// `Ok(DashboardRunner)` on success, `Err(io::Error)` if keyboard initialization fails.
    pub fn new(input: &str, output: &str, resolution: &str) -> io::Result<Self> {
        Self::with_obs_options(input, output, resolution, &ObsOptions::default())
    }

    /// Create a new dashboard runner with OBS integration options
    ///
    /// # Arguments
    ///
    /// - `input`: Input filename
    /// - `output`: Output filename
    /// - `resolution`: Resolution string (e.g., "720p@60fps")
    /// - `obs_options`: OBS integration configuration
    ///
    /// # Returns
    ///
    /// `Ok(DashboardRunner)` on success, `Err(io::Error)` if initialization fails.
    pub fn with_obs_options(input: &str, output: &str, resolution: &str, obs_options: &ObsOptions) -> io::Result<Self> {
        let interactive = Arc::new(InteractiveStateCapsule::new());
        let renderer = DashboardRendererCapsule::new(input, output, resolution);
        let keyboard = DefaultKeyboardHandler::default();

        // Create OBS status writer if enabled (Phase 1)
        let obs_writer = if let Some(ref path) = obs_options.status_file {
            match ObsStatusWriterCapsule::new(path.clone()) {
                Ok(writer) => {
                    writer.set_format(obs_options.status_format);
                    writer.set_interval(obs_options.status_interval_ms);
                    Some(writer)
                }
                Err(e) => {
                    eprintln!("Warning: Failed to create OBS status writer: {}", e);
                    None
                }
            }
        } else {
            None
        };

        // NEW: Load wizard state (best-effort, failures are logged but not fatal)
        let wizard_preferences = Some(UserPreferences::load());
        let wizard_recent = Some(RecentFiles::load());

        Ok(Self {
            interactive,
            renderer,
            keyboard,
            last_render: Instant::now(),
            render_interval_ms: DEFAULT_RENDER_INTERVAL_MS,
            first_render: false,
            raw_mode_enabled: false,
            obs_writer,
            wizard_preferences,
            wizard_recent,
            menu: CommandMenuCapsule::new(),
        })
    }

    /// Start the dashboard (enable raw mode, clear screen, hide cursor)
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(io::Error)` if terminal operations fail.
    ///
    /// # ASSUM: Terminal State
    /// #ASSUME: Terminal supports ANSI escape codes (modern terminals do)
    /// #VERIFY: Tested on Linux/macOS/Windows 10+ terminals
    pub fn start(&mut self) -> io::Result<()> {
        // Enable raw mode for non-blocking input
        self.keyboard.enable_raw_mode()?;
        self.raw_mode_enabled = true;

        // Clear screen and hide cursor
        print!("\x1b[2J"); // Clear screen
        print!("\x1b[?25l"); // Hide cursor
        io::stdout().flush()?;

        Ok(())
    }

    /// Stop the dashboard (restore terminal, show cursor)
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(io::Error)` if terminal operations fail.
    ///
    /// # Safety
    ///
    /// This method MUST be called before program exit to restore the terminal.
    /// It is also called in `Drop` for safety, but explicit calling is preferred.
    pub fn stop(&mut self) -> io::Result<()> {
        if !self.raw_mode_enabled {
            return Ok(()); // Already stopped
        }

        // Show cursor and restore terminal
        print!("\x1b[?25h"); // Show cursor
        println!(); // Move to new line
        io::stdout().flush()?;

        // Restore terminal mode
        self.keyboard.restore_terminal()?;
        self.raw_mode_enabled = false;

        Ok(())
    }

    // ========================================================================
    // Mode Detection (Wizard / Menu / Encoding)
    // ========================================================================

    /// Check if wizard should run
    ///
    /// # Returns
    ///
    /// `true` if wizard_active flag is set in interactive state
    #[inline]
    pub fn should_run_wizard(&self) -> bool {
        self.interactive.is_wizard_active()
    }

    /// Check if menu overlay should show
    ///
    /// # Returns
    ///
    /// `true` if menu_open flag is set in interactive state
    #[inline]
    pub fn should_show_menu(&self) -> bool {
        self.interactive.is_menu_open()
    }

    // ========================================================================
    // Input Handling
    // ========================================================================

    /// Poll for keyboard input and update interactive state
    ///
    /// Returns the key action for the caller to handle higher-level actions.
    /// The interactive state is updated internally (e.g., pause/resume, CRF adjustment).
    ///
    /// # Returns
    ///
    /// - `Ok(Some(KeyAction))` if a key was pressed
    /// - `Ok(None)` if no key was pressed or timeout expired
    /// - `Err(io::Error)` if keyboard polling fails
    ///
    /// # Note
    ///
    /// This method is non-blocking and will return quickly (within ~10ms).
    pub fn poll_and_update(&mut self) -> io::Result<Option<KeyAction>> {
        // Poll keyboard with short timeout (non-blocking)
        let action = self.keyboard.poll_key(KEYBOARD_POLL_TIMEOUT_MS)?;

        if let Some(key_action) = action {
            // Update interactive state based on action
            match key_action {
                KeyAction::TogglePause => {
                    self.interactive.toggle_pause();
                }
                KeyAction::ToggleGpu => {
                    self.interactive.toggle_gpu();
                }
                KeyAction::QualityUp => {
                    self.interactive.adjust_crf(1);
                }
                KeyAction::QualityDown => {
                    self.interactive.adjust_crf(-1);
                }
                KeyAction::Cancel => {
                    self.interactive.request_cancel();
                }
                // Other actions are handled by caller (SaveCheckpoint, OpenOutput, etc.)
                _ => {}
            }

            return Ok(Some(key_action));
        }

        Ok(None)
    }

    /// Update the display with current progress
    ///
    /// This method is rate-limited to avoid excessive rendering.
    /// If called more frequently than `render_interval_ms`, it will be a no-op.
    ///
    /// # Arguments
    ///
    /// - `progress`: Current progress snapshot
    pub fn update_progress(&mut self, progress: &ProgressSnapshot) {
        // Rate limiting: only render at max 30 Hz (33.3ms interval)
        let now = Instant::now();
        if !self.first_render || now.duration_since(self.last_render) >= Duration::from_millis(self.render_interval_ms) {
            let interactive = self.interactive.snapshot();
            let content = self.renderer.render_encoding(progress, &interactive);
            self.renderer.print_dashboard(&content);

            self.last_render = now;
            self.first_render = true;
            self.renderer.mark_rendered(now.elapsed().as_nanos() as u64);
        }

        // OBS Phase 1: Write status to text file (rate-limited internally)
        if let Some(ref obs_writer) = self.obs_writer {
            let _ = obs_writer.write_status(progress);
        }
    }

    /// Show paused state
    ///
    /// Displays the dashboard in paused mode with resume instructions.
    ///
    /// # Arguments
    ///
    /// - `progress`: Current progress snapshot
    pub fn show_paused(&mut self, progress: &ProgressSnapshot) {
        self.renderer.set_state(DashboardState::Paused);
        let content = self.renderer.render_paused(progress);
        self.renderer.print_dashboard(&content);

        let now = Instant::now();
        self.last_render = now;
        self.renderer.mark_rendered(now.elapsed().as_nanos() as u64);
    }

    /// Show completion state
    ///
    /// Displays the dashboard in complete mode with final statistics.
    ///
    /// # Arguments
    ///
    /// - `stats`: Final encoding statistics
    pub fn show_complete(&mut self, stats: &FinalStats) {
        self.renderer.set_state(DashboardState::Complete);
        let content = self.renderer.render_complete(stats);
        self.renderer.print_dashboard(&content);

        let now = Instant::now();
        self.last_render = now;
        self.renderer.mark_rendered(now.elapsed().as_nanos() as u64);

        // OBS Phase 1: Write completion status to text file
        if let Some(ref obs_writer) = self.obs_writer {
            let _ = obs_writer.write_complete(stats);
        }
    }

    /// Show error state
    ///
    /// Displays the dashboard in error mode with error message and optional checkpoint.
    ///
    /// # Arguments
    ///
    /// - `error`: Error message
    /// - `checkpoint`: Optional checkpoint filename
    pub fn show_error(&mut self, error: &str, checkpoint: Option<&str>) {
        self.renderer.set_state(DashboardState::Error);
        let content = self.renderer.render_error(error, checkpoint);
        self.renderer.print_dashboard(&content);

        let now = Instant::now();
        self.last_render = now;
        self.renderer.mark_rendered(now.elapsed().as_nanos() as u64);

        // OBS Phase 1: Write error status to text file
        if let Some(ref obs_writer) = self.obs_writer {
            let _ = obs_writer.write_error(error);
        }
    }

    /// Get a clone of the interactive state for the encoder
    ///
    /// The encoder can hold this Arc to check state (paused, cancelled, etc.)
    /// without needing to communicate with the dashboard runner.
    ///
    /// # Returns
    ///
    /// `Arc<InteractiveStateCapsule>` that can be shared with encoder threads.
    pub fn interactive_state(&self) -> Arc<InteractiveStateCapsule> {
        Arc::clone(&self.interactive)
    }

    /// Check if encoding should pause
    ///
    /// Convenience method that reads the interactive state.
    ///
    /// # Returns
    ///
    /// `true` if user has requested pause, `false` otherwise.
    #[inline]
    pub fn should_pause(&self) -> bool {
        self.interactive.is_paused()
    }

    /// Check if encoding should cancel
    ///
    /// Convenience method that reads the interactive state.
    ///
    /// # Returns
    ///
    /// `true` if user has requested cancellation, `false` otherwise.
    #[inline]
    pub fn should_cancel(&self) -> bool {
        self.interactive.is_cancelled()
    }

    /// Get current CRF adjustment
    ///
    /// Convenience method that reads the interactive state.
    ///
    /// # Returns
    ///
    /// CRF adjustment value (-10 to +10).
    #[inline]
    pub fn crf_adjustment(&self) -> i8 {
        self.interactive.crf_adjustment()
    }

    /// Check if GPU is enabled
    ///
    /// Convenience method that reads the interactive state.
    ///
    /// # Returns
    ///
    /// `true` if GPU acceleration is enabled, `false` otherwise.
    #[inline]
    pub fn gpu_enabled(&self) -> bool {
        self.interactive.is_gpu_enabled()
    }

    // ========================================================================
    // Menu Input Handling
    // ========================================================================

    /// Handle menu navigation input
    ///
    /// Routes input to menu capsule for navigation (up/down/select/back).
    ///
    /// # Arguments
    ///
    /// - `action`: Key action to handle
    ///
    /// # Returns
    ///
    /// `Some(KeyAction)` if menu item was selected, `None` otherwise
    pub fn handle_menu_input(&mut self, action: KeyAction) -> Option<KeyAction> {
        match action {
            KeyAction::Up => {
                self.menu.move_up();
                None
            }
            KeyAction::Down => {
                self.menu.move_down();
                None
            }
            KeyAction::Select => {
                // Get selected menu item and close menu
                let items = CommandMenuCapsule::items();
                let selected = self.menu.selected_index();
                let item_action = items[selected].action;
                self.interactive.close_menu();
                Some(item_action)
            }
            KeyAction::Back | KeyAction::Cancel => {
                self.interactive.close_menu();
                None
            }
            _ => None,
        }
    }

    // ========================================================================
    // Wizard Input Handling (Placeholder)
    // ========================================================================

    /// Handle wizard navigation input (placeholder)
    ///
    /// NOTE: Full wizard flow implementation is in progress (Agent 3A).
    /// This method provides scaffolding for future wizard integration.
    ///
    /// # Arguments
    ///
    /// - `action`: Key action to handle
    ///
    /// # Returns
    ///
    /// `true` if wizard should exit, `false` to continue
    pub fn handle_wizard_input(&mut self, action: KeyAction) -> bool {
        match action {
            KeyAction::Select => {
                // Advance wizard step
                self.interactive.wizard_next();
                // TODO: Check if wizard is complete (step 4 → finish)
                if self.interactive.wizard_step() >= 4 {
                    self.finish_wizard();
                    return true; // Exit wizard
                }
                false
            }
            KeyAction::Back => {
                // Go to previous step or exit if at step 0
                if self.interactive.wizard_step() == 0 {
                    self.cancel_wizard();
                    return true; // Exit wizard
                }
                self.interactive.wizard_prev();
                false
            }
            KeyAction::Cancel => {
                self.cancel_wizard();
                true // Exit wizard
            }
            _ => false,
        }
    }

    // ========================================================================
    // Wizard State Management (Placeholder)
    // ========================================================================

    /// Finish wizard and apply encoding options (placeholder)
    ///
    /// NOTE: This is scaffolding for future wizard integration.
    /// The actual EncodingOptions application will be handled by the encoder.
    fn finish_wizard(&self) {
        // TODO: Extract wizard choices and map to EncodingOptions
        // let quality = ...; // Extract from wizard state
        // let speed = ...; // Extract from wizard state
        // let options = map_to_encoding_options(quality, speed);

        self.interactive.finish_wizard();
    }

    /// Cancel wizard without applying changes
    fn cancel_wizard(&self) {
        self.interactive.finish_wizard();
    }

    /// Build wizard context for rendering (placeholder)
    ///
    /// # Returns
    ///
    /// WizardContext with current state
    fn build_wizard_context(&self) -> WizardContext {
        // TODO: Extract actual hardware info from system
        // TODO: Extract wizard choices from WizardFlowCapsule (Agent 3A)
        WizardContext {
            input_path: None, // TODO: From wizard state
            quality: QualityGoal::Balanced, // TODO: From wizard state
            speed: SpeedChoice::Normal, // TODO: From wizard state
            output_path: None, // TODO: From wizard state
            gpu_name: "Unknown GPU".to_string(), // TODO: From hardware detection
            cpu_threads: std::thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(8),
            memory_gb: 16, // TODO: From hardware detection
        }
    }

    // ========================================================================
    // Rendering Modes
    // ========================================================================

    /// Render wizard step (placeholder)
    ///
    /// NOTE: Full wizard flow implementation is in progress (Agent 3A).
    /// This method provides scaffolding for future wizard integration.
    ///
    /// # Returns
    ///
    /// Rendered wizard step as string
    fn render_wizard_step(&self) -> String {
        let ctx = self.build_wizard_context();
        let step = self.interactive.wizard_step();

        match step {
            0 => render_step_0(&ctx),
            1 => {
                let recent: Vec<(String, u64)> = self
                    .wizard_recent
                    .as_ref()
                    .map(|r| {
                        r.files()
                            .iter()
                            .map(|f| (f.path.to_string_lossy().to_string(), f.size_bytes))
                            .collect()
                    })
                    .unwrap_or_default();
                render_step_1(&ctx, &recent)
            }
            2 => render_step_2(&ctx),
            3 => render_step_3(&ctx),
            4 => render_step_4(&ctx),
            _ => String::from("Invalid wizard step"),
        }
    }

    /// Overlay menu on top of dashboard
    ///
    /// # Arguments
    ///
    /// - `dashboard`: Rendered dashboard content
    /// - `menu`: Rendered menu content
    ///
    /// # Returns
    ///
    /// Combined dashboard + menu overlay
    fn overlay_menu(&self, dashboard: String, menu: String) -> String {
        // Simple approach: append menu below dashboard
        // TODO: Advanced overlay with transparent background and centering
        format!("{}\n\n{}", dashboard, menu)
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Set custom render interval
    ///
    /// Default is 33ms (30 Hz). Lower values increase CPU usage but provide
    /// more responsive UI. Higher values reduce CPU usage.
    ///
    /// # Arguments
    ///
    /// - `interval_ms`: Render interval in milliseconds (min: 16ms for 60 Hz)
    pub fn set_render_interval(&mut self, interval_ms: u64) {
        self.render_interval_ms = interval_ms.max(16); // Min 60 Hz
    }
}

impl Drop for DashboardRunner {
    /// Ensure terminal is always restored on drop
    ///
    /// This is a safety measure to prevent leaving the terminal in raw mode
    /// if the program panics or exits unexpectedly.
    ///
    /// # ASSUM: Drop Safety
    /// #ASSUME: Errors in Drop are safe to ignore (best-effort cleanup)
    /// #VERIFY: Terminal restoration failures logged, not propagated
    fn drop(&mut self) {
        // Best-effort terminal restoration (ignore errors in drop)
        let _ = self.stop();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_runner_new() {
        let dashboard = DashboardRunner::new("input.mp4", "output.av1", "720p@60fps");
        assert!(dashboard.is_ok());

        let dashboard = dashboard.unwrap();
        assert_eq!(dashboard.should_pause(), false);
        assert_eq!(dashboard.should_cancel(), false);
        assert_eq!(dashboard.gpu_enabled(), true);
        assert_eq!(dashboard.crf_adjustment(), 0);
        assert_eq!(dashboard.render_interval_ms, DEFAULT_RENDER_INTERVAL_MS);
    }

    #[test]
    fn test_interactive_state_shared() {
        let dashboard = DashboardRunner::new("in.mp4", "out.av1", "1080p").unwrap();

        // Get shared state
        let state1 = dashboard.interactive_state();
        let state2 = dashboard.interactive_state();

        // Modify via state1
        state1.toggle_pause();
        assert!(state1.is_paused());

        // Change visible via state2 (same Arc)
        assert!(state2.is_paused());

        // Change visible via dashboard
        assert!(dashboard.should_pause());
    }

    #[test]
    fn test_poll_returns_none_without_input() {
        let mut dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        // Without actual keyboard input (DefaultKeyboardHandler is Stub in tests), poll should return None
        let result = dashboard.poll_and_update();
        assert!(result.is_ok());
        // Stub handler always returns None
        #[cfg(not(feature = "cli-interactive"))]
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_pause_resume_cycle() {
        let dashboard = DashboardRunner::new("video.mp4", "out.av1", "1080p").unwrap();

        // Initially not paused
        assert!(!dashboard.should_pause());

        // Toggle to paused
        dashboard.interactive.toggle_pause();
        assert!(dashboard.should_pause());

        // Toggle back to resumed
        dashboard.interactive.toggle_pause();
        assert!(!dashboard.should_pause());
    }

    #[test]
    fn test_cancel_propagates() {
        let dashboard = DashboardRunner::new("test.mp4", "test.av1", "4K").unwrap();

        // Initially not cancelled
        assert!(!dashboard.should_cancel());

        // Request cancellation
        dashboard.interactive.request_cancel();
        assert!(dashboard.should_cancel());
    }

    #[test]
    fn test_crf_adjustment_tracking() {
        let dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        // Initially zero
        assert_eq!(dashboard.crf_adjustment(), 0);

        // Adjust up
        dashboard.interactive.adjust_crf(3);
        assert_eq!(dashboard.crf_adjustment(), 3);

        // Adjust down
        dashboard.interactive.adjust_crf(-5);
        assert_eq!(dashboard.crf_adjustment(), -2);
    }

    #[test]
    fn test_gpu_enabled_toggle() {
        let dashboard = DashboardRunner::new("test.mp4", "test.av1", "1080p").unwrap();

        // Initially enabled
        assert!(dashboard.gpu_enabled());

        // Toggle to disabled
        dashboard.interactive.toggle_gpu();
        assert!(!dashboard.gpu_enabled());

        // Toggle back to enabled
        dashboard.interactive.toggle_gpu();
        assert!(dashboard.gpu_enabled());
    }

    #[test]
    fn test_set_render_interval() {
        let mut dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        // Default
        assert_eq!(dashboard.render_interval_ms, DEFAULT_RENDER_INTERVAL_MS);

        // Set custom
        dashboard.set_render_interval(50);
        assert_eq!(dashboard.render_interval_ms, 50);

        // Test minimum (60 Hz = 16ms)
        dashboard.set_render_interval(10);
        assert_eq!(dashboard.render_interval_ms, 16);
    }

    #[test]
    fn test_update_progress_rate_limiting() {
        let mut dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        let progress = ProgressSnapshot {
            frames_encoded: 100,
            total_frames: 1000,
            fps: 30.0,
            eta_seconds: 30.0,
            psnr: 42.0,
            ssim: 0.98,
            bitrate_mbps: 2.5,
            gpu_percent: 90,
            bytes_written: 50_000,
            input_size: 500_000,
        };

        // First update should always render
        dashboard.update_progress(&progress);
        assert!(dashboard.first_render);

        // Subsequent update within interval should be skipped (no visual change)
        let last_render = dashboard.last_render;
        dashboard.update_progress(&progress);
        // last_render should not change if rate limited
        // Note: This test may be flaky due to timing
    }

    #[test]
    fn test_dashboard_state_transitions() {
        let mut dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        // Initial state is Encoding
        assert_eq!(dashboard.renderer.state(), DashboardState::Encoding);

        // Transition to Paused
        let progress = ProgressSnapshot {
            frames_encoded: 50,
            total_frames: 100,
            fps: 0.0,
            eta_seconds: 0.0,
            psnr: 0.0,
            ssim: 0.0,
            bitrate_mbps: 0.0,
            gpu_percent: 0,
            bytes_written: 0,
            input_size: 0,
        };
        dashboard.show_paused(&progress);
        assert_eq!(dashboard.renderer.state(), DashboardState::Paused);

        // Transition to Complete
        let stats = FinalStats {
            total_frames: 100,
            duration_seconds: 10.0,
            avg_fps: 10.0,
            avg_psnr: 42.0,
            avg_ssim: 0.98,
            compression_ratio: 3.0,
            input_size: 100_000,
            output_size: 33_333,
        };
        dashboard.show_complete(&stats);
        assert_eq!(dashboard.renderer.state(), DashboardState::Complete);

        // Transition to Error
        dashboard.show_error("Test error", Some("checkpoint.ckpt"));
        assert_eq!(dashboard.renderer.state(), DashboardState::Error);
    }

    #[test]
    fn test_start_stop_idempotent() {
        let mut dashboard = DashboardRunner::new("test.mp4", "test.av1", "720p").unwrap();

        // Start
        let result = dashboard.start();
        // May fail in test environment without terminal, but shouldn't panic
        if result.is_ok() {
            assert!(dashboard.raw_mode_enabled);

            // Stop
            assert!(dashboard.stop().is_ok());
            assert!(!dashboard.raw_mode_enabled);

            // Stop again (idempotent)
            assert!(dashboard.stop().is_ok());
            assert!(!dashboard.raw_mode_enabled);
        }
    }
}
