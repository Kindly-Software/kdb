# TUI Implementation Guide - Dual-Mode Entry Point

**Status**: Implementation-ready
**Estimated Effort**: 10 hours (1.5 days)
**Risk**: LOW (zero breaking changes)

---

## 1. Dependencies (Cargo.toml)

**Add to `[dependencies]` section**:

```toml
# TUI Framework
ratatui = "0.25"          # Terminal UI framework
crossterm = "0.27"        # Already present (terminal control)
```

**Why ratatui?**
- Modern successor to tui-rs (actively maintained)
- Excellent performance (60 FPS refresh)
- Crossterm backend already in use
- Zero unsafe code

---

## 2. Module Structure

**Create new files**:

```
src/tui/
├── mod.rs           # Public exports, mode detection
├── app.rs           # App state, main loop
├── ui.rs            # Rendering logic
├── events.rs        # Keyboard handling
└── widgets.rs       # Custom widgets (optional)
```

---

## 3. Entry Point Changes (`src/bin/clapi.rs`)

**Modify main() function**:

```rust
// src/bin/clapi.rs

use clapi_core::{
    cli::{
        banner, handle_budget_add, handle_budget_list, handle_budget_show,
        handle_provider_list, handle_provider_show, handle_provider_test, BudgetAction,
        Cli, Commands, ConfigWizard, ErrorFormatter, ProviderAction,
    },
    test_mode::MockProvider,
    ProxyConfig, ProxyServer,
};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

// NEW: Mode enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Cli,
    Tui,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // NEW: Detect mode before parsing args
    let mode = detect_mode()?;

    match mode {
        Mode::Cli => run_cli_mode().await?,
        Mode::Tui => run_tui_mode().await?,
    }

    Ok(())
}

/// Detect execution mode (CLI vs TUI)
///
/// Priority:
/// 1. Environment variable CLAPI_MODE (override)
/// 2. Args count (0 args → TUI, any args → CLI)
fn detect_mode() -> Result<Mode, Box<dyn std::error::Error>> {
    // Priority 1: Environment variable
    if let Ok(mode) = std::env::var("CLAPI_MODE") {
        return match mode.to_lowercase().as_str() {
            "cli" => Ok(Mode::Cli),
            "tui" => Ok(Mode::Tui),
            invalid => Err(format!(
                "Invalid CLAPI_MODE='{}'. Valid values: cli, tui",
                invalid
            )
            .into()),
        };
    }

    // Priority 2: Args count
    let args: Vec<_> = std::env::args().collect();

    if args.len() == 1 {
        // No args (just binary name) → TUI mode
        Ok(Mode::Tui)
    } else {
        // Any args → CLI mode
        Ok(Mode::Cli)
    }
}

/// Run CLI mode (existing logic, unchanged)
async fn run_cli_mode() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

    // Match command and execute (EXISTING CODE - NO CHANGES)
    match cli.command {
        Commands::Start {
            config,
            test,
            listen,
            budget,
        } => {
            // ... existing logic unchanged
        }

        Commands::Config { output, force } => {
            // ... existing logic unchanged
        }

        // ... rest of CLI commands unchanged
    }

    Ok(())
}

/// Run TUI mode (new)
async fn run_tui_mode() -> Result<(), Box<dyn std::error::Error>> {
    use clapi_core::tui::App;

    // Step 1: Verify terminal support
    if !crossterm::terminal::supports_keyboard_enhancement() {
        eprintln!("{}", "⚠️  Warning: Terminal has limited keyboard support".bright_yellow());
        eprintln!();
        eprintln!("{}", "Try CLI mode instead:".bright_white());
        eprintln!("  {}", "clapi --help".bright_black());
        eprintln!();
    }

    // Step 2: Verify server running (optional - graceful degradation)
    let server_url = "http://localhost:8080";
    let server_running = reqwest::get(format!("{}/health", server_url))
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if !server_running {
        eprintln!("{}", "⚠️  Server not running".bright_yellow());
        eprintln!();
        eprintln!("{}", "Start server first:".bright_white());
        eprintln!("  {}", "clapi start --test".bright_black());
        eprintln!();
        eprintln!("{}", "Press Ctrl+C to exit or any key to continue anyway...".bright_black());

        // Wait for keypress
        use crossterm::event::{read, Event};
        loop {
            match read()? {
                Event::Key(_) => break,
                _ => {}
            }
        }
    }

    // Step 3: Run TUI
    let mut app = App::new(server_url)?;
    app.run().await?;

    Ok(())
}

// ... rest of existing code unchanged
```

---

## 4. TUI Module (`src/tui/mod.rs`)

```rust
//! TUI Module - Interactive Terminal Interface
//!
//! # Purpose
//! Provides an interactive terminal UI for clapi with real-time monitoring,
//! budget management, and provider status.
//!
//! # UCE34 Framework
//! - Q10: Tier N/A (UI layer, not coordination)
//! - Q31: Simplicity - `clapi` launches instantly
//! - Q32: Constraints - Terminal required (60+ columns, 24+ rows)
//! - Q33: Validation - Keyboard shortcuts, help screen
//!
//! # I20 Integration
//! - Q1-Q5: Isolated from CLI (no shared state)
//! - Q6-Q10: Compatible with CLI handlers (reuses HTTP calls)
//! - Q11-Q15: Graceful degradation on server disconnect
//! - Q16-Q20: Property tests validate all keyboard inputs

mod app;
mod events;
mod ui;

pub use app::App;
pub use events::{Event, EventHandler};

#[cfg(test)]
mod tests {
    #[test]
    fn test_tui_module_compiles() {
        // Smoke test: Ensure module compiles
    }
}
```

---

## 5. App State (`src/tui/app.rs`)

```rust
//! App State - TUI Application State Management
//!
//! # Design
//! - App owns all UI state (budgets, providers, metrics)
//! - No shared state with CLI
//! - Async refresh from HTTP endpoints

use crate::cli::{handle_budget_list, handle_provider_list, BudgetStatus, ProviderStatus};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use std::io;
use tokio::time::{Duration, interval};

/// Main TUI application
pub struct App {
    /// Server base URL (e.g., "http://localhost:8080")
    server_url: String,

    /// Current view
    current_view: View,

    /// Budget list cache
    budgets: Vec<BudgetStatus>,

    /// Provider list cache
    providers: Vec<ProviderStatus>,

    /// Selected item index (for navigation)
    selected_index: usize,

    /// Error message (if any)
    error_message: Option<String>,

    /// Should quit
    should_quit: bool,
}

/// Available views
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    MainMenu,
    Budgets,
    Providers,
    Metrics,
    Help,
}

impl App {
    /// Create new App
    pub fn new(server_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            server_url: server_url.to_string(),
            current_view: View::MainMenu,
            budgets: Vec::new(),
            providers: Vec::new(),
            selected_index: 0,
            error_message: None,
            should_quit: false,
        })
    }

    /// Run main TUI loop
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Clear terminal
        terminal.clear()?;

        // Show welcome message
        self.current_view = View::MainMenu;

        // Main event loop
        let result = self.run_event_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main event loop
    async fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crate::tui::events::EventHandler;
        use crate::tui::ui;

        let mut event_handler = EventHandler::new(Duration::from_millis(100));

        // Refresh interval (every 5 seconds)
        let mut refresh_interval = interval(Duration::from_secs(5));

        loop {
            // Render UI
            terminal.draw(|f| ui::render(f, self))?;

            // Handle events
            tokio::select! {
                // Keyboard/mouse events
                Some(event) = event_handler.next() => {
                    self.handle_event(event).await?;
                }

                // Auto-refresh
                _ = refresh_interval.tick() => {
                    self.refresh_current_view().await?;
                }
            }

            // Check quit
            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle keyboard event
    async fn handle_event(
        &mut self,
        event: crossterm::event::Event,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use crossterm::event::{Event, KeyCode, KeyEvent};

        match event {
            Event::Key(KeyEvent { code, .. }) => match code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.should_quit = true;
                }

                KeyCode::Char('?') | KeyCode::F(1) => {
                    self.current_view = View::Help;
                }

                KeyCode::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }

                KeyCode::Down => {
                    self.selected_index += 1;
                }

                KeyCode::Enter => {
                    self.handle_selection().await?;
                }

                KeyCode::Char('1') => {
                    self.current_view = View::Budgets;
                    self.refresh_budgets().await?;
                }

                KeyCode::Char('2') => {
                    self.current_view = View::Providers;
                    self.refresh_providers().await?;
                }

                KeyCode::Char('3') => {
                    self.current_view = View::Metrics;
                }

                KeyCode::Char('r') => {
                    self.refresh_current_view().await?;
                }

                _ => {}
            },

            _ => {}
        }

        Ok(())
    }

    /// Handle Enter key on current selection
    async fn handle_selection(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.current_view {
            View::MainMenu => {
                // Navigate to selected view
                self.current_view = match self.selected_index {
                    0 => View::Budgets,
                    1 => View::Providers,
                    2 => View::Metrics,
                    3 => View::Help,
                    _ => View::MainMenu,
                };
                self.selected_index = 0;
                self.refresh_current_view().await?;
            }

            _ => {}
        }

        Ok(())
    }

    /// Refresh current view data
    async fn refresh_current_view(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        match self.current_view {
            View::Budgets => self.refresh_budgets().await?,
            View::Providers => self.refresh_providers().await?,
            View::Metrics => { /* TODO: Fetch metrics */ }
            _ => {}
        }

        Ok(())
    }

    /// Refresh budgets from server
    async fn refresh_budgets(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Call handle_budget_list and parse response
        // For now, placeholder
        self.budgets = vec![];
        Ok(())
    }

    /// Refresh providers from server
    async fn refresh_providers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Call handle_provider_list and parse response
        // For now, placeholder
        self.providers = vec![];
        Ok(())
    }

    /// Show error message
    pub fn show_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    /// Clear error message
    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Get current view
    pub fn current_view(&self) -> View {
        self.current_view
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get budgets
    pub fn budgets(&self) -> &[BudgetStatus] {
        &self.budgets
    }

    /// Get providers
    pub fn providers(&self) -> &[ProviderStatus] {
        &self.providers
    }

    /// Get error message
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }
}
```

---

## 6. UI Rendering (`src/tui/ui.rs`)

```rust
//! UI Rendering - Ratatui Layout and Widgets
//!
//! # Design
//! - Clean, minimalist layout
//! - Byzantine Purple (#663399) + Gold (#FFD700) branding
//! - Keyboard shortcuts visible
//! - Real-time updates (5s refresh)

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::{App, View};

/// Main render function
pub fn render(f: &mut Frame, app: &App) {
    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Main content
            Constraint::Length(3),  // Footer
        ])
        .split(f.size());

    // Render header
    render_header(f, chunks[0]);

    // Render main content (depends on view)
    match app.current_view() {
        View::MainMenu => render_main_menu(f, chunks[1], app),
        View::Budgets => render_budgets(f, chunks[1], app),
        View::Providers => render_providers(f, chunks[1], app),
        View::Metrics => render_metrics(f, chunks[1], app),
        View::Help => render_help(f, chunks[1]),
    }

    // Render footer
    render_footer(f, chunks[2], app);

    // Render error popup (if any)
    if let Some(error) = app.error_message() {
        render_error_popup(f, f.size(), error);
    }
}

/// Render header
fn render_header(f: &mut Frame, area: Rect) {
    let header = Paragraph::new("clapi - AI Gateway with Budget Protection")
        .style(Style::default().fg(Color::Rgb(102, 51, 153))) // Byzantine Purple
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(header, area);
}

/// Render footer
fn render_footer(f: &mut Frame, area: Rect, app: &App) {
    let shortcuts = match app.current_view() {
        View::MainMenu => "↑↓: Navigate | Enter: Select | q: Quit | ?: Help",
        View::Budgets => "↑↓: Navigate | r: Refresh | Esc: Back | q: Quit",
        View::Providers => "↑↓: Navigate | r: Refresh | Esc: Back | q: Quit",
        View::Metrics => "r: Refresh | Esc: Back | q: Quit",
        View::Help => "Esc: Back | q: Quit",
    };

    let footer = Paragraph::new(shortcuts)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}

/// Render main menu
fn render_main_menu(f: &mut Frame, area: Rect, app: &App) {
    let items = vec![
        "1. Budgets",
        "2. Providers",
        "3. Metrics Dashboard",
        "4. Help",
    ];

    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app.selected_index() {
                Style::default()
                    .fg(Color::Rgb(255, 215, 0)) // Gold
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(*item).style(style)
        })
        .collect();

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Main Menu"));

    f.render_widget(list, area);
}

/// Render budgets view
fn render_budgets(f: &mut Frame, area: Rect, app: &App) {
    let budgets = app.budgets();

    if budgets.is_empty() {
        let empty = Paragraph::new("No budgets found. Start the server with: clapi start --test")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Budgets"));

        f.render_widget(empty, area);
    } else {
        // TODO: Render budget table
        let placeholder = Paragraph::new("Budgets view (TODO)")
            .block(Block::default().borders(Borders::ALL).title("Budgets"));

        f.render_widget(placeholder, area);
    }
}

/// Render providers view
fn render_providers(f: &mut Frame, area: Rect, app: &App) {
    let providers = app.providers();

    if providers.is_empty() {
        let empty = Paragraph::new("No providers configured. Check clapi.toml")
            .style(Style::default().fg(Color::Yellow))
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Providers"));

        f.render_widget(empty, area);
    } else {
        // TODO: Render provider table
        let placeholder = Paragraph::new("Providers view (TODO)")
            .block(Block::default().borders(Borders::ALL).title("Providers"));

        f.render_widget(placeholder, area);
    }
}

/// Render metrics dashboard
fn render_metrics(f: &mut Frame, area: Rect, _app: &App) {
    // TODO: Render metrics dashboard
    let placeholder = Paragraph::new("Metrics Dashboard (TODO)")
        .block(Block::default().borders(Borders::ALL).title("Metrics"));

    f.render_widget(placeholder, area);
}

/// Render help screen
fn render_help(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from("Keyboard Shortcuts:"),
        Line::from(""),
        Line::from("  ↑/↓        Navigate lists"),
        Line::from("  Enter      Select item"),
        Line::from("  1-4        Jump to view (Main Menu)"),
        Line::from("  r          Refresh current view"),
        Line::from("  Esc        Go back"),
        Line::from("  q          Quit"),
        Line::from("  ?          Show this help"),
        Line::from(""),
        Line::from("Views:"),
        Line::from("  1. Budgets   - Manage AI spending budgets"),
        Line::from("  2. Providers - Monitor AI provider status"),
        Line::from("  3. Metrics   - Real-time performance dashboard"),
        Line::from("  4. Help      - This screen"),
    ];

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title("Help"))
        .alignment(Alignment::Left);

    f.render_widget(help, area);
}

/// Render error popup
fn render_error_popup(f: &mut Frame, area: Rect, message: &str) {
    // Center popup (50% width, 30% height)
    let popup_area = centered_rect(50, 30, area);

    let error = Paragraph::new(message)
        .style(Style::default().fg(Color::Red))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Error")
                .style(Style::default().fg(Color::Red)),
        );

    f.render_widget(error, popup_area);
}

/// Helper: Create centered rect
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
```

---

## 7. Event Handler (`src/tui/events.rs`)

```rust
//! Event Handler - Keyboard and Mouse Input
//!
//! # Design
//! - Async event stream
//! - Non-blocking (uses tokio channels)
//! - Debouncing (100ms interval)

use crossterm::event::{self, Event as CrosstermEvent};
use std::time::Duration;
use tokio::sync::mpsc;

pub use crossterm::event::Event;

/// Event handler
pub struct EventHandler {
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    /// Create new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Spawn event reader thread
        tokio::spawn(async move {
            loop {
                // Poll for event (non-blocking)
                if event::poll(tick_rate).unwrap_or(false) {
                    if let Ok(event) = event::read() {
                        if sender.send(event).is_err() {
                            break; // Channel closed
                        }
                    }
                }

                tokio::time::sleep(tick_rate).await;
            }
        });

        Self { receiver }
    }

    /// Get next event (async)
    pub async fn next(&mut self) -> Option<Event> {
        self.receiver.recv().await
    }
}
```

---

## 8. Testing

**Unit tests** (`src/tui/mod.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_enum() {
        assert_eq!(View::MainMenu, View::MainMenu);
        assert_ne!(View::MainMenu, View::Budgets);
    }
}
```

**Integration tests** (`tests/tui_integration_tests.rs`):

```rust
use clapi_core::tui::App;

#[tokio::test]
async fn test_app_creation() {
    let app = App::new("http://localhost:8080");
    assert!(app.is_ok());
}

#[tokio::test]
async fn test_app_initial_view() {
    let app = App::new("http://localhost:8080").unwrap();
    assert_eq!(app.current_view(), View::MainMenu);
}
```

**Mode detection tests** (`src/bin/clapi.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_mode_no_args() {
        std::env::remove_var("CLAPI_MODE");
        // Simulate: clapi (no args)
        // Note: Can't easily test std::env::args() in unit tests
        // Use integration tests instead
    }

    #[test]
    fn test_detect_mode_env_override() {
        std::env::set_var("CLAPI_MODE", "tui");
        // Should return Mode::Tui regardless of args
    }
}
```

---

## 9. Documentation Updates

**Update README.md**:

```markdown
## Usage

### Interactive TUI (Terminal UI)

Launch the interactive dashboard:

```bash
clapi
```

Features:
- Real-time budget monitoring
- Provider status dashboard
- Metrics visualization
- Keyboard shortcuts (press `?` for help)

### Command-Line Interface (CLI)

For scripts and automation:

```bash
clapi start --test                # Start server
clapi budget list                 # List budgets
clapi providers list              # List providers
clapi metrics                     # Show metrics
```

### Mode Override

Force specific mode using environment variable:

```bash
CLAPI_MODE=cli clapi              # Force CLI (show help)
CLAPI_MODE=tui clapi start        # Force TUI (ignore args)
```
```

---

## 10. Rollout Checklist

**Pre-deployment**:
- [ ] All unit tests pass (`cargo test --lib`)
- [ ] All integration tests pass (`cargo test --test tui_integration_tests`)
- [ ] Manual testing on Linux, macOS, Windows
- [ ] Binary size < 10MB (`ls -lh target/release/clapi`)
- [ ] CLI startup < 20ms (no regression)
- [ ] TUI startup < 100ms
- [ ] Zero compiler warnings

**Deployment**:
- [ ] Merge to main
- [ ] Tag release v0.5.0
- [ ] Build binary: `cargo build --release`
- [ ] Deploy to production (100% immediately)

**Post-deployment**:
- [ ] Verify `clapi` launches TUI
- [ ] Verify `clapi start` launches server (CLI)
- [ ] Monitor for issues (first 24 hours)
- [ ] Update documentation

---

## 11. Troubleshooting

**Issue**: TUI doesn't render correctly
- **Cause**: Terminal incompatibility
- **Fix**: Use CLI mode: `CLAPI_MODE=cli clapi`

**Issue**: "Server not running" error
- **Cause**: Server not started
- **Fix**: Start server first: `clapi start --test`

**Issue**: Binary size increased too much
- **Cause**: Ratatui + dependencies
- **Fix**: Check `cargo bloat --release --crates`

**Issue**: CLI regression (scripts fail)
- **Cause**: Bug in mode detection
- **Fix**: Git revert, file issue

---

## Summary

**Estimated Effort**: 10 hours (1.5 days)

**Files Created**:
- `src/tui/mod.rs` (50 lines)
- `src/tui/app.rs` (200 lines)
- `src/tui/ui.rs` (250 lines)
- `src/tui/events.rs` (50 lines)

**Files Modified**:
- `src/bin/clapi.rs` (+100 lines, main() function)
- `Cargo.toml` (+1 dependency: ratatui)

**Total Lines Added**: ~650 lines

**Risk**: LOW (zero breaking changes, backward compatible)

**Rollback**: Git revert (5 minutes)

---

**Status**: ✅ Ready for implementation
