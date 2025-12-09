//! TUI Application State Capsule - Lockfree Event-Driven TUI
//!
//! # UCE34 Framework
//! - Q1-Q9: TUI application state management (event loop + rendering + command palette)
//! - Q10: Tier 1 (Atomic) - Lockfree state coordination
//! - Q11: Rust atomic patterns for event handling
//! - Q12: Nightly N/A (stable atomics sufficient)
//! - Q13-Q28: Event validation, frame time monitoring, input handling
//! - Q31: Simplicity - Single atomic state machine, <16ms frame time
//! - Q33: Validation - #[derive(ComputationalCapsule)] compile-time verification
//! - Q34: Auditability - Command history via InputHandler
//!
//! # ASSUM Framework
//! - #ASSUME: AtomicU8 state machine (8 states max)
//! - #VERIFY: All state transitions are valid (compile-time enum)
//! - #ASSUME: AtomicBool flags independent (no ordering dependencies)
//! - #VERIFY: All atomic operations use appropriate memory ordering
//! - #ASSUME: Event loop is single-threaded (crossterm guarantees sequential delivery)
//! - #VERIFY: Palette/input capsules are lockfree safe (#[derive(ComputationalCapsule)])
//!
//! # Performance Targets
//! - Frame time: <16ms (60 FPS target)
//! - Event processing: <5ms per event
//! - State reads: <5ns (single atomic load, Relaxed ordering)
//! - State updates: <10ns (single atomic store, Release ordering)
//! - Input latency: <1ms (keyboard → buffer update)

use atomic_capsule_derive::ComputationalCapsule;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::task::JoinHandle;

// Import command palette, input handler, content capsule, metrics poller, progress indicator, and output capsule
use crate::tui::palette::CommandPalette;
use crate::tui::input::InputHandler;
use crate::tui::content::DashboardContentCapsule;
use crate::tui::polling::MetricsPoller;
use crate::tui::progress::ProgressIndicatorCapsule;
use crate::tui::help::HelpOverlayCapsule;
use crate::tui::output::CommandOutputCapsule;
use crate::tui::tabs::TabStateCapsule;
use crate::tui::dispatcher::CommandDispatcher;
use crate::tui::palette::format_friendly_error;

/// TUI Application State (8 states, fits in u8)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppState {
    Running = 0,
    Paused = 1,
    Exiting = 2,
    Error = 3,
}

impl From<u8> for AppState {
    fn from(value: u8) -> Self {
        match value {
            0 => AppState::Running,
            1 => AppState::Paused,
            2 => AppState::Exiting,
            3 => AppState::Error,
            _ => AppState::Running, // Safe default
        }
    }
}

/// TUI Application Capsule (T1 Atomic, 64B aligned)
///
/// # Memory Layout
/// ```text
/// Offset | Field              | Size | Alignment
/// -------|-------------------|------|----------
/// 0      | state             | 1    | 1
/// 1      | should_quit       | 1    | 1
/// 2      | should_refresh    | 1    | 1
/// 3-63   | _padding          | 61   | 1 (pad to 64B)
/// ```
///
/// # Chaos Principles
/// - Cache-aligned (64B) - Single cache line access
/// - Atomic state machine - Lockfree state transitions
/// - Zero dependencies - No external UI state libraries
/// - <16ms frame time - 60 FPS rendering target
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
pub struct TuiAppCapsule {
    state: AtomicU8,            // Current application state
    should_quit: AtomicBool,    // Quit requested flag
    should_refresh: AtomicBool, // Refresh requested flag
    _padding: [u8; 61],         // Pad to 64B
}

impl TuiAppCapsule {
    /// Create new TUI application capsule
    ///
    /// # Performance
    /// - <20ns initialization (3 atomic stores)
    /// - Zero allocation
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(AppState::Running as u8),
            should_quit: AtomicBool::new(false),
            should_refresh: AtomicBool::new(true),
            _padding: [0; 61],
        }
    }

    // State management

    /// Get current application state
    ///
    /// # Performance
    /// - <5ns (single atomic load, Relaxed ordering)
    #[inline(always)]
    pub fn state(&self) -> AppState {
        // #ASSUME: Relaxed ordering sufficient (state reads don't require synchronization)
        AppState::from(self.state.load(Ordering::Relaxed))
    }

    /// Set application state
    ///
    /// # Performance
    /// - <10ns (single atomic store, Release ordering)
    #[inline(always)]
    pub fn set_state(&self, new_state: AppState) {
        // #VERIFY: Release ordering ensures state change visible to other threads
        self.state.store(new_state as u8, Ordering::Release);
    }

    /// Check if should quit
    #[inline(always)]
    pub fn should_quit(&self) -> bool {
        self.should_quit.load(Ordering::Relaxed)
    }

    /// Request quit
    #[inline(always)]
    pub fn request_quit(&self) {
        self.should_quit.store(true, Ordering::Release);
        self.set_state(AppState::Exiting);
    }

    /// Check if should refresh
    #[inline(always)]
    pub fn should_refresh(&self) -> bool {
        self.should_refresh.load(Ordering::Relaxed)
    }

    /// Request refresh
    #[inline(always)]
    pub fn request_refresh(&self) {
        self.should_refresh.store(true, Ordering::Release);
    }

    /// Clear refresh flag (called after rendering)
    #[inline(always)]
    pub fn clear_refresh(&self) {
        self.should_refresh.store(false, Ordering::Relaxed);
    }

    /// Pause application
    #[inline(always)]
    pub fn pause(&self) {
        self.set_state(AppState::Paused);
    }

    /// Resume application
    #[inline(always)]
    pub fn resume(&self) {
        self.set_state(AppState::Running);
        self.request_refresh();
    }
}

impl Default for TuiAppCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// TUI Application Runner
///
/// # Integration (UCE34 Q13-Q21)
/// - CommandPalette: Fuzzy search, command execution
/// - InputHandler: Command input bar, history, tab completion
/// - DashboardContentCapsule: Live metrics cache (atomic, lockfree)
/// - MetricsPoller: Background HTTP polling (5s interval)
/// - ProgressIndicatorCapsule: Spinner animation for async commands
/// - HelpOverlayCapsule: Keyboard shortcuts guide (? key)
/// - CommandOutputCapsule: Command output ring buffer for TUI display
/// - CommandDispatcher: Lockfree command execution (100% async)
pub struct TuiApp {
    pub(crate) capsule: TuiAppCapsule,
    terminal: Terminal<CrosstermBackend<Stdout>>,
    palette: CommandPalette,
    pub(crate) input: InputHandler,
    content: Arc<DashboardContentCapsule>,
    poller: MetricsPoller,
    poller_handle: Option<JoinHandle<()>>,
    pub(crate) progress: ProgressIndicatorCapsule,
    pub(crate) help: HelpOverlayCapsule,
    pub(crate) output: Arc<CommandOutputCapsule>,  // 100% lockfree with UnsafeCell
    pub(crate) tabs: TabStateCapsule,  // Tab state management
    dispatcher: Arc<CommandDispatcher>,  // Command execution (shared)
    error_pending: Arc<AtomicBool>,  // Flag set by async task to trigger immediate refresh
    ctrl_c_pressed: bool,
    ctrl_c_time: Instant,
}

impl TuiApp {
    /// Create new TUI application
    ///
    /// # Errors
    /// - Terminal initialization fails
    /// - Crossterm setup fails
    /// - Input handler initialization fails (history file I/O)
    /// - Tokio runtime not available (async context required)
    ///
    /// # ASSUM Framework
    /// - #ASSUME: Tokio runtime is available (async context)
    /// - #VERIFY: tokio::spawn requires runtime, will panic if not available
    /// - #ASSUME: Arc<T> is thread-safe for DashboardContentCapsule
    /// - #VERIFY: Compiler enforces Send + Sync bounds on Arc
    /// - #ASSUME: Polling interval ≥100ms (safe, no flooding)
    /// - #VERIFY: MetricsPoller enforces 100ms minimum in constructor
    pub fn new() -> io::Result<Self> {
        // Setup terminal
        crossterm::terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        crossterm::execute!(
            stdout,
            crossterm::terminal::EnterAlternateScreen,
            crossterm::event::EnableMouseCapture
        )?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // IMMEDIATE RENDER: Fill screen immediately to eliminate timing gap
        // This prevents gray flicker between alternate screen entry and first event loop render
        terminal.draw(|f| {
            use ratatui::{
                widgets::{Block, Borders},
                style::Style,
            };

            // Render a transparent block that fills the screen
            // No explicit bg color - uses terminal's default background
            let filler = Block::default()
                .borders(Borders::NONE)
                .style(Style::default());  // Terminal default, no forced colors

            f.render_widget(filler, f.area());
        })?;

        // Initialize command palette, input handler, progress indicator, help overlay, and output capsule
        let palette = CommandPalette::new();
        let input = InputHandler::new()?;
        let progress = ProgressIndicatorCapsule::new();
        let help = HelpOverlayCapsule::new();
        let output = Arc::new(CommandOutputCapsule::new());  // 100% lockfree with UnsafeCell

        // Initialize dashboard content capsule (5s refresh interval)
        let content = Arc::new(DashboardContentCapsule::new(5000));

        // Initialize metrics poller (localhost endpoint)
        let poller = MetricsPoller::new("http://localhost:8080/metrics".to_string());

        // Start background polling thread
        let poller_handle = poller.start(content.clone());

        // Initialize command dispatcher (same base URL as poller, wrapped in Arc for sharing)
        let dispatcher = Arc::new(CommandDispatcher::new("http://localhost:8080"));

        // Initialize error_pending flag for immediate refresh after async errors
        let error_pending = Arc::new(AtomicBool::new(false));

        Ok(Self {
            capsule: TuiAppCapsule::new(),
            terminal,
            palette,
            input,
            content,
            poller,
            poller_handle: Some(poller_handle),
            progress,
            help,
            output,
            tabs: TabStateCapsule::new(),
            dispatcher,
            error_pending,
            ctrl_c_pressed: false,
            ctrl_c_time: Instant::now(),
        })
    }

    /// Run TUI event loop
    ///
    /// # Performance Target
    /// - <16ms per frame (60 FPS)
    /// - <5ms event processing
    /// - <11ms rendering
    ///
    /// # Errors
    /// - Terminal I/O errors
    /// - Event processing errors
    ///
    /// # ASSUM Safety
    /// - #ASSUME: cleanup() is called before returning (terminal restoration)
    /// - #VERIFY: cleanup() called at end of function for all paths (line 320)
    pub fn run(&mut self) -> io::Result<()> {
        let frame_duration = Duration::from_millis(16); // 60 FPS target

        // Request initial render (show TUI on startup)
        self.capsule.request_refresh();

        while !self.capsule.should_quit() {
            let frame_start = Instant::now();

            // Update progress indicator spinner frame (if active and 100ms elapsed)
            if self.progress.update_frame_if_needed() {
                self.capsule.request_refresh();
            }

            // Process events (budget: 5ms)
            if event::poll(Duration::from_millis(5))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key.code, key.modifiers);
                }
            }

            // Check for pending async error updates
            if self.error_pending.load(Ordering::Acquire) {
                self.error_pending.store(false, Ordering::Release); // Clear flag
                self.capsule.request_refresh(); // Trigger immediate refresh
            }

            // Render if needed (budget: 11ms)
            if self.capsule.should_refresh() {
                // Extract references we need before borrowing terminal
                // This avoids borrowing self immutably while terminal is borrowed mutably
                let capsule_ref = &self.capsule;
                let content_ref = &self.content;
                let progress_ref = &self.progress;
                let help_ref = &self.help;
                let tabs_ref = &self.tabs;
                let palette_ref = &self.palette;
                let output_ref = &self.output;

                self.terminal.draw(|f| {
                    use crate::tui::layout::render_layout;
                    render_layout(f, capsule_ref, Some(content_ref.as_ref()), Some(progress_ref), Some(help_ref), tabs_ref, Some(palette_ref), Some(output_ref));
                })?;
                self.capsule.clear_refresh();
            }

            // Sleep to maintain frame rate
            let elapsed = frame_start.elapsed();
            if elapsed < frame_duration {
                std::thread::sleep(frame_duration - elapsed);
            }
        }

        // CRITICAL: Restore terminal state before returning
        // #VERIFY: This ensures raw mode is disabled and alternate screen is left
        // even if the event loop exits unexpectedly
        self.cleanup()?;

        Ok(())
    }

    /// Handle key events
    ///
    /// # Key Bindings (Global)
    /// - `/`: Toggle command palette
    /// - `Esc`: Hide palette or quit (if palette not visible)
    /// - `Ctrl+C`: Quit
    /// - `p`: Pause
    /// - `r`: Resume
    /// - `Ctrl+R`: Refresh
    ///
    /// # Key Bindings (Palette Visible)
    /// - `↑/↓`: Navigate commands
    /// - `Enter`: Execute selected command
    /// - `Esc`: Hide palette
    /// - `Char(c)`: Filter commands
    /// - `Backspace`: Delete filter character
    ///
    /// # Key Bindings (Input Bar)
    /// - `Char(c)`: Insert character
    /// - `Backspace`: Delete character before cursor
    /// - `Delete`: Delete character after cursor
    /// - `Left/Right`: Move cursor
    /// - `Home/End`: Jump to start/end
    /// - `Ctrl+U`: Clear line
    /// - `Ctrl+A/E`: Jump to start/end
    /// - `Up/Down`: Navigate history
    /// - `Tab`: Tab completion
    /// - `Enter`: Execute command
    fn handle_key_event(&mut self, code: KeyCode, modifiers: KeyModifiers) {
        // Priority 0: Help overlay visible - intercept keys for help navigation
        if self.help.is_visible() {
            match (code, modifiers) {
                // Toggle help (? or Esc)
                (KeyCode::Char('?'), KeyModifiers::NONE) | (KeyCode::Esc, _) => {
                    self.help.toggle();
                    self.capsule.request_refresh();
                }

                // Scroll navigation in help
                (KeyCode::Up, _) => {
                    self.help.scroll_up();
                    self.capsule.request_refresh();
                }
                (KeyCode::Down, _) => {
                    // Max scroll is large enough to cover all help content
                    self.help.scroll_down(100);
                    self.capsule.request_refresh();
                }

                // Ignore all other keys when help is visible
                _ => {}
            }
            return;
        }

        // Priority 1: Command palette visible - intercept all keys except Esc
        if self.palette.is_visible() {
            match (code, modifiers) {
                // Esc: Hide palette
                (KeyCode::Esc, _) => {
                    self.palette.hide();
                    self.capsule.request_refresh();
                }

                // Navigation: Up/Down (selection)
                (KeyCode::Up, KeyModifiers::NONE) => {
                    self.palette.prev();
                    self.capsule.request_refresh();
                }
                (KeyCode::Down, KeyModifiers::NONE) => {
                    self.palette.next();
                    self.capsule.request_refresh();
                }

                // Scrolling: Ctrl+Up/Down (scroll content)
                (KeyCode::Up, KeyModifiers::CONTROL) => {
                    self.palette.scroll_up();
                    self.capsule.request_refresh();
                }
                (KeyCode::Down, KeyModifiers::CONTROL) => {
                    // Calculate max scroll based on filtered commands
                    let filtered_count = self.palette.filtered_commands().len();
                    self.palette.scroll_down(filtered_count as u32);
                    self.capsule.request_refresh();
                }

                // Enter: Execute selected command
                (KeyCode::Enter, _) => {
                    if let Some(command) = self.palette.execute() {
                        // Parse command and args (simple split for now)
                        let parts: Vec<String> = command.split_whitespace()
                            .map(|s| s.to_string())
                            .collect();

                        if let Some(cmd_name) = parts.first() {
                            let args = parts[1..].to_vec();

                            // Clone dispatcher, output, and error_pending flag for async task
                            let dispatcher = self.dispatcher.clone();
                            let output = self.output.clone();
                            let error_pending = self.error_pending.clone();
                            let cmd_name = cmd_name.clone();

                            // Spawn async task to execute command
                            tokio::spawn(async move {
                                match dispatcher.execute(&cmd_name, &args).await {
                                    Ok(success_msg) => {
                                        // Clear error on success (100% lockfree)
                                        output.set_last_error("");
                                        // Set flag to trigger immediate refresh
                                        error_pending.store(true, Ordering::Release);
                                        // Note: Success output could be displayed in a separate notification area
                                    }
                                    Err(error) => {
                                        // Format friendly error and store it (100% lockfree)
                                        let friendly_error = format_friendly_error(&cmd_name, &error.to_string());
                                        output.set_last_error(&friendly_error);
                                        // Set flag to trigger immediate refresh
                                        error_pending.store(true, Ordering::Release);
                                    }
                                }
                            });
                        }
                    }
                    self.capsule.request_refresh();
                }

                // Text input: Update filter
                (KeyCode::Char(c), KeyModifiers::NONE) | (KeyCode::Char(c), KeyModifiers::SHIFT) => {
                    let mut filter = self.palette.current_filter().to_string();
                    filter.push(c);
                    self.palette.update_filter(filter);
                    self.capsule.request_refresh();
                }

                // Backspace: Delete filter character
                (KeyCode::Backspace, _) => {
                    let mut filter = self.palette.current_filter().to_string();
                    filter.pop();
                    self.palette.update_filter(filter);
                    self.capsule.request_refresh();
                }

                // Ignore all other keys when palette is visible
                _ => {}
            }
            return;
        }

        // Priority 2: Global key bindings (when palette not visible)
        match (code, modifiers) {
            // Toggle help overlay (?)
            (KeyCode::Char('?'), KeyModifiers::NONE) => {
                self.ctrl_c_pressed = false;
                self.help.toggle();
                self.capsule.request_refresh();
            }

            // Toggle command palette (/)
            (KeyCode::Char('/'), KeyModifiers::NONE) => {
                self.ctrl_c_pressed = false;
                self.palette.toggle();
                self.capsule.request_refresh();
            }

            // Tab switching (number keys 1-5)
            (KeyCode::Char('1'), KeyModifiers::NONE) => {
                self.tabs.set_tab(0);
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }
            (KeyCode::Char('2'), KeyModifiers::NONE) => {
                self.tabs.set_tab(1);
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }
            (KeyCode::Char('3'), KeyModifiers::NONE) => {
                self.tabs.set_tab(2);
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }
            (KeyCode::Char('4'), KeyModifiers::NONE) => {
                self.tabs.set_tab(3);
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }
            (KeyCode::Char('5'), KeyModifiers::NONE) => {
                self.tabs.set_tab(4);
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }

            // Escape - close help overlay or palette if open, otherwise do nothing
            (KeyCode::Esc, _) => {
                if self.help.is_visible() {
                    self.help.toggle();
                }
                if self.palette.is_visible() {
                    self.palette.hide();
                }
                // Reset Ctrl+C counter on any key press
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }
            // Quit with Ctrl+C twice (1 second window, prevents accidental exit)
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                if self.ctrl_c_pressed && self.ctrl_c_time.elapsed() < Duration::from_secs(1) {
                    // Second Ctrl+C within 1 second - quit
                    self.capsule.request_quit();
                } else {
                    // First Ctrl+C or timeout - set flag
                    self.ctrl_c_pressed = true;
                    self.ctrl_c_time = Instant::now();
                    self.capsule.request_refresh();
                }
            }

            // Pause/Resume
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                self.ctrl_c_pressed = false;
                if self.capsule.state() == AppState::Running {
                    self.capsule.pause();
                } else if self.capsule.state() == AppState::Paused {
                    self.capsule.resume();
                }
            }
            (KeyCode::Char('r'), KeyModifiers::NONE) => {
                self.ctrl_c_pressed = false;
                self.capsule.resume();
            }

            // Refresh
            (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                self.ctrl_c_pressed = false;
                self.capsule.request_refresh();
            }

            // Input bar handling (all other keys)
            _ => {
                // Reset Ctrl+C counter on any other key press
                self.ctrl_c_pressed = false;

                let event = crossterm::event::KeyEvent::new(code, modifiers);
                if self.input.handle_key(event) {
                    // Enter pressed - execute command from input bar
                    let command = self.input.buffer().to_string();
                    self.input.clear();

                    // TODO: Dispatch command to CommandDispatcher
                    // (silently clear command; actual dispatch happens in dispatcher)
                    // Command is stored in history by InputHandler::handle_key()

                    self.capsule.request_refresh();
                }
            }
        }
    }

    /// Cleanup terminal state
    fn cleanup(&mut self) -> io::Result<()> {
        crossterm::terminal::disable_raw_mode()?;
        crossterm::execute!(
            self.terminal.backend_mut(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        // Stop polling thread (graceful shutdown)
        self.poller.stop();

        // Wait for polling thread to finish (max 5s timeout)
        if let Some(handle) = self.poller_handle.take() {
            // In Drop, we can't await directly
            // If we're inside a runtime, we can't block_on (it would panic)
            // So just abort the task - it's a background poller anyway
            if tokio::runtime::Handle::try_current().is_ok() {
                // Inside a runtime - abort instead of block_on
                handle.abort();
            } else {
                // No runtime - safe to create one and block_on
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
                });
            }
        }

        // Best-effort cleanup (ignore errors)
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_alignment() {
        assert_eq!(std::mem::size_of::<TuiAppCapsule>(), 64);
        assert_eq!(std::mem::align_of::<TuiAppCapsule>(), 64);
    }

    #[test]
    fn test_initial_state() {
        let app = TuiAppCapsule::new();
        assert_eq!(app.state(), AppState::Running);
        assert!(!app.should_quit());
        assert!(app.should_refresh()); // Initial refresh required
    }

    #[test]
    fn test_state_transitions() {
        let app = TuiAppCapsule::new();

        // Running -> Paused
        app.pause();
        assert_eq!(app.state(), AppState::Paused);

        // Paused -> Running
        app.resume();
        assert_eq!(app.state(), AppState::Running);
        assert!(app.should_refresh()); // Refresh on resume

        // Running -> Exiting
        app.request_quit();
        assert_eq!(app.state(), AppState::Exiting);
        assert!(app.should_quit());
    }

    #[test]
    fn test_refresh_flag() {
        let app = TuiAppCapsule::new();

        // Initial state
        assert!(app.should_refresh());

        // Clear refresh
        app.clear_refresh();
        assert!(!app.should_refresh());

        // Request refresh
        app.request_refresh();
        assert!(app.should_refresh());
    }

    #[test]
    fn test_tab_state_capsule_size_alignment() {
        assert_eq!(std::mem::size_of::<TabStateCapsule>(), 64);
        assert_eq!(std::mem::align_of::<TabStateCapsule>(), 64);
    }

    #[test]
    fn test_tab_state_initial() {
        let tabs = TabStateCapsule::new();
        assert_eq!(tabs.get_tab(), 0);
    }

    #[test]
    fn test_tab_state_switching() {
        let tabs = TabStateCapsule::new();

        // Switch to different tabs
        tabs.set_tab(1);
        assert_eq!(tabs.get_tab(), 1);

        tabs.set_tab(4);
        assert_eq!(tabs.get_tab(), 4);

        tabs.set_tab(0);
        assert_eq!(tabs.get_tab(), 0);
    }

    #[test]
    fn test_tab_state_bounds() {
        let tabs = TabStateCapsule::new();

        // Test upper bound constraint
        tabs.set_tab(10);
        assert_eq!(tabs.get_tab(), 4); // Constrained to max (4)

        tabs.set_tab(5);
        assert_eq!(tabs.get_tab(), 4); // Constrained to max (4)

        // Test valid range
        tabs.set_tab(2);
        assert_eq!(tabs.get_tab(), 2);
    }
}
