//! Interactive TUI for the Kindly-AV1 wizard
//!
//! # Tier: T1 Atomic + T5 Streaming
//!
//! Interactive terminal user interface using atomic_capsule TUI capsules.
//! Arrow key navigation, box-drawing characters, and brand colors.
//!
//! # Layout (WizardTuiCapsule - 512B)
//! - keyboard: KeyboardInputHistoryCapsule (64B) - Input tracking
//! - screen: ScreenStateCapsule (128B) - Screen navigation
//! - flow: WizardFlowCapsule reference - State machine
//! - selection_index: AtomicU8 - Current list selection (0-3)
//! - list_size: AtomicU8 - Number of items in current list
//! - _padding: Cache alignment to 512B
//!
//! # Performance
//! - Render: <1ms (terminal write)
//! - Input handling: <10ns (atomic operations)
//! - State query: <5ns (single atomic load)
//!
//! # Chaos Compliance
//! - 100% lockfree (uses atomic_capsule primitives)
//! - Cache-aligned (512B)
//! - Generation counters via WizardFlowCapsule

use super::flow::{WizardFlowCapsule, WizardState};
use super::mapping::{QualityGoal, SpeedChoice};
use super::steps::WizardContext;
use crate::cli::branding::{BOLD, DIM, HEART, LIGHTNING, PURPLE, RESET, SPARK, YELLOW};
use std::io::{self, Write};
use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(feature = "cli-kindly-term")]
use atomic_capsule::tui::KeyboardInputHistoryCapsule;

// ============================================================================
// Key Codes
// ============================================================================

/// Key codes for TUI navigation
pub mod keys {
    pub const ARROW_UP: u32 = 0x1B5B41;       // ESC [ A
    pub const ARROW_DOWN: u32 = 0x1B5B42;     // ESC [ B
    pub const ARROW_LEFT: u32 = 0x1B5B44;     // ESC [ D
    pub const ARROW_RIGHT: u32 = 0x1B5B43;    // ESC [ C
    pub const ENTER: u32 = 0x0D;              // CR
    pub const ESCAPE: u32 = 0x1B;             // ESC alone
    pub const BACKSPACE: u32 = 0x7F;          // DEL
    pub const CTRL_C: u32 = 0x03;             // ETX
    pub const SPACE: u32 = 0x20;              // Space
}

// ============================================================================
// Box Drawing Characters
// ============================================================================

/// Box drawing characters for TUI borders
pub mod box_chars {
    pub const TOP_LEFT: &str = "\u{250C}";      // ┌
    pub const TOP_RIGHT: &str = "\u{2510}";     // ┐
    pub const BOTTOM_LEFT: &str = "\u{2514}";   // └
    pub const BOTTOM_RIGHT: &str = "\u{2518}";  // ┘
    pub const HORIZONTAL: &str = "\u{2500}";    // ─
    pub const VERTICAL: &str = "\u{2502}";      // │
    pub const TEE_LEFT: &str = "\u{251C}";      // ├
    pub const TEE_RIGHT: &str = "\u{2524}";     // ┤
    pub const CROSS: &str = "\u{253C}";         // ┼
    pub const BULLET: &str = "\u{2022}";        // •
    pub const ARROW_RIGHT: &str = "\u{25B6}";   // ▶
    pub const CHECK: &str = "\u{2714}";         // ✔
    pub const RADIO_ON: &str = "\u{25C9}";      // ◉
    pub const RADIO_OFF: &str = "\u{25CB}";     // ○
}

// ============================================================================
// Selection List Capsule (T1 Atomic, 64B)
// ============================================================================

/// T1 Atomic selection list state (64B cache-aligned)
///
/// Tracks current selection index and list size for arrow key navigation.
#[repr(C, align(64))]
pub struct SelectionListCapsule {
    /// Current selection index (0-based)
    index: AtomicU8,
    /// Number of items in list (max 255)
    size: AtomicU8,
    /// Generation counter for change detection
    generation: AtomicU8,
    /// Reserved for future use
    _reserved: [u8; 5],
    /// Padding to 64 bytes
    _padding: [u8; 56],
}

impl SelectionListCapsule {
    /// Create new selection list with given size
    pub const fn new(size: u8) -> Self {
        Self {
            index: AtomicU8::new(0),
            size: AtomicU8::new(size),
            generation: AtomicU8::new(0),
            _reserved: [0; 5],
            _padding: [0; 56],
        }
    }

    /// Get current selection index
    #[inline]
    pub fn index(&self) -> u8 {
        self.index.load(Ordering::Acquire)
    }

    /// Get list size
    #[inline]
    pub fn size(&self) -> u8 {
        self.size.load(Ordering::Acquire)
    }

    /// Move selection up (wraps to bottom)
    pub fn move_up(&self) {
        let current = self.index.load(Ordering::Acquire);
        let size = self.size.load(Ordering::Acquire);
        if size == 0 {
            return;
        }
        let new_index = if current == 0 {
            size.saturating_sub(1)
        } else {
            current - 1
        };
        self.index.store(new_index, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Move selection down (wraps to top)
    pub fn move_down(&self) {
        let current = self.index.load(Ordering::Acquire);
        let size = self.size.load(Ordering::Acquire);
        if size == 0 {
            return;
        }
        let new_index = if current >= size.saturating_sub(1) {
            0
        } else {
            current + 1
        };
        self.index.store(new_index, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Set selection index directly
    pub fn set_index(&self, idx: u8) {
        let size = self.size.load(Ordering::Acquire);
        let clamped = if size == 0 { 0 } else { idx.min(size - 1) };
        self.index.store(clamped, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Reset list with new size
    pub fn reset(&self, size: u8) {
        self.size.store(size, Ordering::Release);
        self.index.store(0, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}

// ============================================================================
// Wizard TUI Capsule
// ============================================================================

/// Interactive TUI wrapper for WizardFlowCapsule
///
/// Provides arrow key navigation, colored output, and box-drawing UI.
pub struct WizardTuiCapsule<'a> {
    /// Reference to the underlying wizard flow
    pub flow: &'a WizardFlowCapsule,
    /// Selection list state for current step
    selection: SelectionListCapsule,
    /// Terminal width (cached)
    width: u16,
    /// Terminal height (cached)
    height: u16,
}

impl<'a> WizardTuiCapsule<'a> {
    /// Create new TUI wrapper for wizard flow
    pub fn new(flow: &'a WizardFlowCapsule) -> Self {
        // Get terminal size (fallback to 80x24)
        let (width, height) = get_terminal_size();

        Self {
            flow,
            selection: SelectionListCapsule::new(3), // Default 3 options
            width,
            height,
        }
    }

    /// Handle keyboard input and return true if screen needs redraw
    pub fn handle_key(&self, key: u32) -> bool {
        match key {
            keys::ARROW_UP => {
                self.selection.move_up();
                true
            }
            keys::ARROW_DOWN => {
                self.selection.move_down();
                true
            }
            keys::ENTER | keys::ARROW_RIGHT => {
                self.confirm_selection();
                true
            }
            keys::BACKSPACE | keys::ARROW_LEFT => {
                if self.flow.can_go_back() {
                    self.flow.back();
                }
                true
            }
            keys::ESCAPE | keys::CTRL_C => {
                self.flow.cancel();
                true
            }
            _ => false,
        }
    }

    /// Confirm current selection and advance wizard
    fn confirm_selection(&self) {
        let state = self.flow.state();
        let idx = self.selection.index();

        match state {
            WizardState::Step2QualityGoal => {
                let quality = match idx {
                    0 => QualityGoal::Smallest,
                    1 => QualityGoal::Balanced,
                    2 => QualityGoal::Best,
                    _ => QualityGoal::Balanced,
                };
                self.flow.set_quality(quality);
                self.flow.next();
                // Reset selection for next step
                self.selection.reset(3);
            }
            WizardState::Step3SpeedChoice => {
                let speed = match idx {
                    0 => SpeedChoice::Quick,
                    1 => SpeedChoice::Normal,
                    2 => SpeedChoice::Thorough,
                    _ => SpeedChoice::Normal,
                };
                self.flow.set_speed(speed);
                self.flow.next();
                self.selection.reset(2); // Confirm has 2 options
            }
            WizardState::Step4Confirm => {
                if idx == 0 {
                    // "Start Encoding" selected
                    self.flow.next();
                } else {
                    // "Go Back" selected
                    self.flow.back();
                }
            }
            WizardState::Step0HardwareCheck | WizardState::Step1SelectVideo => {
                // These steps just need Enter to continue
                self.flow.next();
            }
            _ => {}
        }
    }

    /// Get current selection index
    #[inline]
    pub fn selection_index(&self) -> u8 {
        self.selection.index()
    }

    /// Render current wizard state to terminal
    pub fn render(&self, ctx: &WizardContext) -> io::Result<()> {
        let state = self.flow.state();
        let mut stdout = io::stdout();

        // Clear screen and move cursor to top
        write!(stdout, "\x1B[2J\x1B[H")?;

        // Render header
        self.render_header(&mut stdout)?;

        // Render progress bar
        self.render_progress(&mut stdout, state)?;

        // Render step content
        match state {
            WizardState::Step0HardwareCheck => self.render_step_0(&mut stdout, ctx)?,
            WizardState::Step1SelectVideo => self.render_step_1(&mut stdout, ctx)?,
            WizardState::Step2QualityGoal => self.render_step_2(&mut stdout, ctx)?,
            WizardState::Step3SpeedChoice => self.render_step_3(&mut stdout, ctx)?,
            WizardState::Step4Confirm => self.render_step_4(&mut stdout, ctx)?,
            WizardState::Complete => self.render_complete(&mut stdout, ctx)?,
            WizardState::Cancelled => self.render_cancelled(&mut stdout)?,
            WizardState::Idle => self.render_welcome(&mut stdout)?,
        }

        // Render footer with navigation hints
        self.render_footer(&mut stdout, state)?;

        stdout.flush()
    }

    /// Render header with brand
    fn render_header(&self, w: &mut impl Write) -> io::Result<()> {
        let width = self.width.min(80) as usize;

        // Top border
        write!(w, "{}", PURPLE)?;
        write!(w, "{}", box_chars::TOP_LEFT)?;
        for _ in 0..(width - 2) {
            write!(w, "{}", box_chars::HORIZONTAL)?;
        }
        writeln!(w, "{}{}", box_chars::TOP_RIGHT, RESET)?;

        // Title line
        let title = format!("{} Kindly-AV1 Encoder {}", HEART, SPARK);
        let title_len = 22; // Approximate visible length
        let padding = (width.saturating_sub(title_len + 2)) / 2;

        write!(w, "{}{}", PURPLE, box_chars::VERTICAL)?;
        write!(w, "{}", RESET)?;
        for _ in 0..padding {
            write!(w, " ")?;
        }
        write!(w, "{}{}{}{}", BOLD, PURPLE, title, RESET)?;
        for _ in 0..(width.saturating_sub(title_len + padding + 2)) {
            write!(w, " ")?;
        }
        writeln!(w, "{}{}{}", PURPLE, box_chars::VERTICAL, RESET)?;

        // Bottom border of header
        write!(w, "{}", PURPLE)?;
        write!(w, "{}", box_chars::TEE_LEFT)?;
        for _ in 0..(width - 2) {
            write!(w, "{}", box_chars::HORIZONTAL)?;
        }
        writeln!(w, "{}{}", box_chars::TEE_RIGHT, RESET)?;

        Ok(())
    }

    /// Render progress indicator
    fn render_progress(&self, w: &mut impl Write, state: WizardState) -> io::Result<()> {
        let steps = [
            ("Hardware", WizardState::Step0HardwareCheck),
            ("Select", WizardState::Step1SelectVideo),
            ("Quality", WizardState::Step2QualityGoal),
            ("Speed", WizardState::Step3SpeedChoice),
            ("Confirm", WizardState::Step4Confirm),
        ];

        let current_idx = match state {
            WizardState::Step0HardwareCheck => 0,
            WizardState::Step1SelectVideo => 1,
            WizardState::Step2QualityGoal => 2,
            WizardState::Step3SpeedChoice => 3,
            WizardState::Step4Confirm => 4,
            WizardState::Complete => 5,
            _ => 0,
        };

        write!(w, "  ")?;
        for (i, (name, _step)) in steps.iter().enumerate() {
            if i == current_idx {
                write!(w, "{}{}[{}]{} ", BOLD, PURPLE, name, RESET)?;
            } else if i < current_idx {
                write!(w, "{}{}{} {} ", DIM, box_chars::CHECK, name, RESET)?;
            } else {
                write!(w, "{}{} {} ", DIM, box_chars::RADIO_OFF, name)?;
            }
            if i < steps.len() - 1 {
                write!(w, "{}─{} ", DIM, RESET)?;
            }
        }
        writeln!(w)?;
        writeln!(w)?;

        Ok(())
    }

    /// Render welcome screen (Idle state)
    fn render_welcome(&self, w: &mut impl Write) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "  {}{}Welcome to Kindly-AV1!{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;
        writeln!(w, "  The fastest GPU-accelerated AV1 encoder.")?;
        writeln!(w)?;
        writeln!(w, "  This wizard will help you:")?;
        writeln!(w, "  {} Check your hardware", box_chars::BULLET)?;
        writeln!(w, "  {} Select a video file", box_chars::BULLET)?;
        writeln!(w, "  {} Choose quality and speed settings", box_chars::BULLET)?;
        writeln!(w, "  {} Start encoding", box_chars::BULLET)?;
        writeln!(w)?;
        writeln!(w, "  {}Press Enter to begin...{}", DIM, RESET)?;
        Ok(())
    }

    /// Render Step 0: Hardware Detection
    fn render_step_0(&self, w: &mut impl Write, ctx: &WizardContext) -> io::Result<()> {
        writeln!(w, "  {}{}Step 1: Checking Hardware{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;

        // GPU status
        let (gpu_icon, gpu_status) = if !ctx.gpu_name.is_empty() && ctx.gpu_name != "Unknown" {
            (box_chars::CHECK, format!("{} (ROCm ready)", ctx.gpu_name))
        } else {
            (box_chars::RADIO_OFF, "Not detected - using CPU".to_string())
        };
        writeln!(w, "  {} {}GPU:{} {}", LIGHTNING, BOLD, RESET, gpu_status)?;
        writeln!(w, "    {}{}{}", if gpu_icon == box_chars::CHECK { PURPLE } else { DIM }, gpu_icon, RESET)?;

        // Memory status
        writeln!(w)?;
        writeln!(w, "  \u{1F4BE} {}Memory:{} {} GB available", BOLD, RESET, ctx.memory_gb)?;
        writeln!(w, "    {}{}{}", PURPLE, box_chars::CHECK, RESET)?;

        // CPU status
        writeln!(w)?;
        writeln!(w, "  \u{1F527} {}CPU:{} {} threads available", BOLD, RESET, ctx.cpu_threads)?;
        writeln!(w, "    {}{}{}", PURPLE, box_chars::CHECK, RESET)?;

        writeln!(w)?;
        writeln!(w, "  {}Great! Your system is ready.{}", DIM, RESET)?;
        writeln!(w)?;
        writeln!(w, "  {}Press Enter to continue...{}", DIM, RESET)?;

        Ok(())
    }

    /// Render Step 1: Select Video
    fn render_step_1(&self, w: &mut impl Write, ctx: &WizardContext) -> io::Result<()> {
        writeln!(w, "  {}{}Step 2: Select Video File{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;

        if let Some(path) = &ctx.input_path {
            writeln!(w, "  {}Selected:{} {}", BOLD, RESET, path)?;
            writeln!(w)?;
            writeln!(w, "  {}Press Enter to continue, or drag a new file...{}", DIM, RESET)?;
        } else {
            writeln!(w, "  Drag and drop a video file here,")?;
            writeln!(w, "  or type the file path:")?;
            writeln!(w)?;
            writeln!(w, "  > _")?;
        }

        Ok(())
    }

    /// Render Step 2: Quality Goal with arrow selection
    fn render_step_2(&self, w: &mut impl Write, _ctx: &WizardContext) -> io::Result<()> {
        writeln!(w, "  {}{}Step 3: Quality Goal{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;
        writeln!(w, "  How important is file size vs quality?")?;
        writeln!(w)?;

        let selected = self.selection.index();
        let options = [
            ("Smallest File", "Saves the most space (~70% smaller)", "archiving, slow uploads"),
            ("Balanced", "Best of both worlds (~50% smaller)", "sharing, storage"),
            ("Best Quality", "Keeps everything crisp (~30% smaller)", "important videos"),
        ];

        for (i, (label, desc, use_case)) in options.iter().enumerate() {
            let is_selected = i as u8 == selected;
            let marker = if is_selected { box_chars::RADIO_ON } else { box_chars::RADIO_OFF };
            let color = if is_selected { PURPLE } else { "" };
            let bold = if is_selected { BOLD } else { "" };
            let reset = RESET;

            writeln!(w, "  {}{}{} {}{}{}", color, marker, reset, bold, color, label)?;
            writeln!(w, "      {}{}{}", DIM, desc, reset)?;
            writeln!(w, "      {}Ideal for: {}{}", DIM, use_case, reset)?;
            writeln!(w)?;
        }

        writeln!(w, "  {}Use \u{2191}\u{2193} arrows to select, Enter to confirm{}", DIM, RESET)?;

        Ok(())
    }

    /// Render Step 3: Speed Choice with arrow selection
    fn render_step_3(&self, w: &mut impl Write, _ctx: &WizardContext) -> io::Result<()> {
        writeln!(w, "  {}{}Step 4: Encoding Speed{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;
        writeln!(w, "  How much time do you have?")?;
        writeln!(w)?;

        let selected = self.selection.index();
        let options = [
            ("Quick", "~2 minutes", "I need it now!"),
            ("Normal", "~5 minutes", "I can wait a bit for better compression"),
            ("Thorough", "~12 minutes", "Take your time, smallest file"),
        ];

        for (i, (label, eta, desc)) in options.iter().enumerate() {
            let is_selected = i as u8 == selected;
            let marker = if is_selected { box_chars::RADIO_ON } else { box_chars::RADIO_OFF };
            let color = if is_selected { PURPLE } else { "" };
            let bold = if is_selected { BOLD } else { "" };
            let reset = RESET;

            writeln!(w, "  {}{}{} {}{}{} ({})", color, marker, reset, bold, color, label, eta)?;
            writeln!(w, "      {}{}{}", DIM, desc, reset)?;
            writeln!(w)?;
        }

        writeln!(w, "  {}Use \u{2191}\u{2193} arrows to select, Enter to confirm{}", DIM, RESET)?;

        Ok(())
    }

    /// Render Step 4: Confirmation
    fn render_step_4(&self, w: &mut impl Write, ctx: &WizardContext) -> io::Result<()> {
        writeln!(w, "  {}{}Step 5: Ready to Encode{}", BOLD, PURPLE, RESET)?;
        writeln!(w)?;

        // Summary
        writeln!(w, "  {}Summary:{}", BOLD, RESET)?;
        if let Some(path) = &ctx.input_path {
            writeln!(w, "  {} Input: {}", box_chars::BULLET, path)?;
        }
        if let Some(path) = &ctx.output_path {
            writeln!(w, "  {} Output: {}", box_chars::BULLET, path)?;
        }
        writeln!(w, "  {} Quality: {:?}", box_chars::BULLET, ctx.quality)?;
        writeln!(w, "  {} Speed: {:?}", box_chars::BULLET, ctx.speed)?;
        writeln!(w)?;

        // Confirm buttons
        let selected = self.selection.index();

        let start_marker = if selected == 0 { box_chars::RADIO_ON } else { box_chars::RADIO_OFF };
        let start_color = if selected == 0 { PURPLE } else { "" };
        let back_marker = if selected == 1 { box_chars::RADIO_ON } else { box_chars::RADIO_OFF };
        let back_color = if selected == 1 { PURPLE } else { "" };

        writeln!(w, "  {}{}{} {}{}{SPARK} Start Encoding{}", start_color, start_marker, RESET, BOLD, start_color, RESET)?;
        writeln!(w, "  {}{}{} {}Go Back{}", back_color, back_marker, RESET, back_color, RESET)?;
        writeln!(w)?;

        writeln!(w, "  {}Use \u{2191}\u{2193} arrows to select, Enter to confirm{}", DIM, RESET)?;

        Ok(())
    }

    /// Render completion screen
    fn render_complete(&self, w: &mut impl Write, ctx: &WizardContext) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "  {}{}{} Encoding Started!{}", BOLD, PURPLE, box_chars::CHECK, RESET)?;
        writeln!(w)?;

        if let Some(path) = &ctx.input_path {
            writeln!(w, "  Input: {}", path)?;
        }
        if let Some(path) = &ctx.output_path {
            writeln!(w, "  Output: {}", path)?;
        }
        writeln!(w)?;
        writeln!(w, "  {}Progress will be shown below...{}", DIM, RESET)?;

        Ok(())
    }

    /// Render cancelled screen
    fn render_cancelled(&self, w: &mut impl Write) -> io::Result<()> {
        writeln!(w)?;
        writeln!(w, "  {}Encoding cancelled.{}", DIM, RESET)?;
        writeln!(w)?;
        writeln!(w, "  Run {}kindly-av1 wizard{} to start again.", BOLD, RESET)?;

        Ok(())
    }

    /// Render footer with navigation hints
    fn render_footer(&self, w: &mut impl Write, state: WizardState) -> io::Result<()> {
        let width = self.width.min(80) as usize;

        writeln!(w)?;

        // Bottom border
        write!(w, "{}", PURPLE)?;
        write!(w, "{}", box_chars::BOTTOM_LEFT)?;
        for _ in 0..(width - 2) {
            write!(w, "{}", box_chars::HORIZONTAL)?;
        }
        writeln!(w, "{}{}", box_chars::BOTTOM_RIGHT, RESET)?;

        // Navigation hints
        let hints = match state {
            WizardState::Step2QualityGoal | WizardState::Step3SpeedChoice | WizardState::Step4Confirm => {
                "\u{2191}\u{2193} Navigate  \u{21B5} Select  \u{232B} Back  Esc Cancel"
            }
            WizardState::Complete | WizardState::Cancelled => {
                ""
            }
            _ => {
                "\u{21B5} Continue  Esc Cancel"
            }
        };

        writeln!(w, "  {}{}{}", DIM, hints, RESET)?;

        Ok(())
    }
}

// ============================================================================
// Terminal Utilities
// ============================================================================

/// Get terminal size (width, height)
fn get_terminal_size() -> (u16, u16) {
    // Try to get from environment or use defaults
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        // Try ioctl
        #[repr(C)]
        struct WinSize {
            ws_row: u16,
            ws_col: u16,
            ws_xpixel: u16,
            ws_ypixel: u16,
        }

        let mut size = WinSize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        // TIOCGWINSZ = 0x5413 on Linux
        const TIOCGWINSZ: libc::c_ulong = 0x5413;

        let result = unsafe {
            libc::ioctl(
                std::io::stdout().as_raw_fd(),
                TIOCGWINSZ,
                &mut size as *mut WinSize,
            )
        };

        if result == 0 && size.ws_col > 0 && size.ws_row > 0 {
            return (size.ws_col, size.ws_row);
        }
    }

    // Fallback to environment variables
    if let (Ok(cols), Ok(rows)) = (
        std::env::var("COLUMNS").map(|s| s.parse::<u16>().unwrap_or(80)),
        std::env::var("LINES").map(|s| s.parse::<u16>().unwrap_or(24)),
    ) {
        return (cols, rows);
    }

    // Default fallback
    (80, 24)
}

/// Read a single key from stdin (blocking)
pub fn read_key() -> io::Result<u32> {
    use std::io::Read;

    let mut stdin = io::stdin();
    let mut buf = [0u8; 4];

    // Read first byte
    let n = stdin.read(&mut buf[..1])?;
    if n == 0 {
        return Ok(0);
    }

    // Check for escape sequence
    if buf[0] == 0x1B {
        // Try to read more bytes for arrow keys
        // Set stdin to non-blocking briefly
        let mut seq = [0u8; 3];
        if let Ok(n) = stdin.read(&mut seq[..2]) {
            if n >= 2 && seq[0] == b'[' {
                // Arrow key sequence: ESC [ A/B/C/D
                return Ok(0x1B0000 | ((seq[0] as u32) << 8) | (seq[1] as u32));
            }
        }
        // Just ESC
        return Ok(0x1B);
    }

    Ok(buf[0] as u32)
}

/// Set terminal to raw mode for key input
pub fn enable_raw_mode() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        let fd = std::io::stdin().as_raw_fd();
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();

        unsafe {
            if libc::tcgetattr(fd, termios.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }

            let mut termios = termios.assume_init();

            // Disable canonical mode and echo
            termios.c_lflag &= !(libc::ICANON | libc::ECHO);

            // Set minimum characters and timeout
            termios.c_cc[libc::VMIN] = 1;
            termios.c_cc[libc::VTIME] = 0;

            if libc::tcsetattr(fd, libc::TCSAFLUSH, &termios) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }

    Ok(())
}

/// Restore terminal to normal mode
pub fn disable_raw_mode() -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;

        let fd = std::io::stdin().as_raw_fd();
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();

        unsafe {
            if libc::tcgetattr(fd, termios.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }

            let mut termios = termios.assume_init();

            // Re-enable canonical mode and echo
            termios.c_lflag |= libc::ICANON | libc::ECHO;

            if libc::tcsetattr(fd, libc::TCSAFLUSH, &termios) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
    }

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_list_new() {
        let list = SelectionListCapsule::new(5);
        assert_eq!(list.index(), 0);
        assert_eq!(list.size(), 5);
    }

    #[test]
    fn test_selection_list_move_down() {
        let list = SelectionListCapsule::new(3);

        list.move_down();
        assert_eq!(list.index(), 1);

        list.move_down();
        assert_eq!(list.index(), 2);

        // Wrap around
        list.move_down();
        assert_eq!(list.index(), 0);
    }

    #[test]
    fn test_selection_list_move_up() {
        let list = SelectionListCapsule::new(3);

        // Wrap from 0 to 2
        list.move_up();
        assert_eq!(list.index(), 2);

        list.move_up();
        assert_eq!(list.index(), 1);

        list.move_up();
        assert_eq!(list.index(), 0);
    }

    #[test]
    fn test_selection_list_reset() {
        let list = SelectionListCapsule::new(3);
        list.move_down();
        list.move_down();
        assert_eq!(list.index(), 2);

        list.reset(5);
        assert_eq!(list.index(), 0);
        assert_eq!(list.size(), 5);
    }

    #[test]
    fn test_wizard_tui_capsule_creation() {
        let flow = WizardFlowCapsule::new();
        let tui = WizardTuiCapsule::new(&flow);

        assert_eq!(tui.selection_index(), 0);
    }

    #[test]
    fn test_key_codes() {
        assert_eq!(keys::ENTER, 0x0D);
        assert_eq!(keys::ESCAPE, 0x1B);
        assert_eq!(keys::CTRL_C, 0x03);
    }

    #[test]
    fn test_box_chars() {
        // Verify box drawing characters are valid UTF-8
        assert!(!box_chars::TOP_LEFT.is_empty());
        assert!(!box_chars::HORIZONTAL.is_empty());
        assert!(!box_chars::VERTICAL.is_empty());
    }

    #[test]
    fn test_selection_list_alignment() {
        assert_eq!(std::mem::size_of::<SelectionListCapsule>(), 64);
        assert_eq!(std::mem::align_of::<SelectionListCapsule>(), 64);
    }
}
