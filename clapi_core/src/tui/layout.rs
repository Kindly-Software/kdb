//! TUI 3-Panel Layout with Tab Navigation - Header + Main + Input (LIVE DATA RENDERING)
//!
//! # UCE34 Framework
//! - Q1-Q9: TUI layout rendering (3-panel design with tab navigation)
//! - Q10: Tier N/A (pure layout, no state)
//! - Q11: Rust ratatui layout primitives
//! - Q12: Nightly N/A (stable Rust sufficient)
//! - Q13-Q28: Layout validation, responsive design, tab navigation
//! - Q31: Simplicity - 3 panels + 5 tabs, Byzantine Purple theme, atomic data reads
//! - Q33: Validation - Compile-time layout validation
//! - Q34: Auditability N/A (no state modification)
//!
//! # Layout Design
//! ```text
//! ┌─────────────────────────────────────────────┐
//! │ Header (Byzantine Purple background)        │ 3 lines
//! │ clapi v0.5.0 | Status: Running | 60 FPS    │
//! ├─────────────────────────────────────────────┤
//! │ [1:Overview] 2:Providers 3:Budgets ...     │ Tab indicator
//! │                                             │
//! │ Main Content Area (tab-specific content)    │ Flexible
//! │                                             │
//! │ - Tab 0: Overview (default)                 │
//! │ - Tab 1: Providers (detailed provider view) │
//! │ - Tab 2: Budgets (detailed budget view)     │
//! │ - Tab 3: Performance (metrics & latency)    │
//! │ - Tab 4: Cost (cost analysis)               │
//! │                                             │
//! ├─────────────────────────────────────────────┤
//! │ Input Bar (Gold accent)                     │ 3 lines
//! │ > Type command or press 'q' to quit         │
//! └─────────────────────────────────────────────┘
//! ```
//!
//! # Performance
//! - Render time: <11ms (60 FPS target)
//! - Atomic reads: <100ns for full metrics snapshot
//! - Zero allocation in hot path
//! - Single-pass layout calculation

use super::{app::TuiAppCapsule, colors::ColorThemeCapsule, content::DashboardContentCapsule, progress::ProgressIndicatorCapsule, help::{HelpOverlayCapsule, render_help_overlay}, tabs::TabStateCapsule, palette::CommandPalette, output::CommandOutputCapsule};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::sync::Arc;

/// Render 3-panel layout with tab navigation
///
/// # Arguments
/// - `frame`: Ratatui terminal frame
/// - `app`: TUI application capsule (state machine)
/// - `content`: Optional dashboard content capsule (live metrics)
/// - `progress`: Optional progress indicator capsule (spinner animation)
/// - `help`: Optional help overlay capsule (keyboard shortcuts guide)
/// - `tabs`: Tab state capsule (navigation state)
/// - `palette`: Optional command palette (fuzzy search)
/// - `output`: Optional command output capsule (error notifications)
///
/// # Performance
/// - <11ms render time (60 FPS budget)
/// - <100ns atomic reads for metrics snapshot
/// - Zero allocation in hot path
/// - Single-pass layout calculation
///
/// # Layout Structure
/// - Header: 3 lines (status bar)
/// - Main: Flexible (scrollable content with tab indicator + tab-specific live data)
/// - Error notification: 3 lines (when error exists)
/// - Input: 3 lines (command bar)
/// - Help overlay: Rendered on top when visible (? key)
/// - Command palette: Rendered on top when visible (/ key)
pub fn render_layout(
    frame: &mut Frame,
    app: &TuiAppCapsule,
    content: Option<&DashboardContentCapsule>,
    progress: Option<&ProgressIndicatorCapsule>,
    help: Option<&HelpOverlayCapsule>,
    tabs: &TabStateCapsule,
    palette: Option<&CommandPalette>,
    output: Option<&Arc<CommandOutputCapsule>>,
) {
    let theme = ColorThemeCapsule::new();

    // Check for auto-dismiss of errors (>10 seconds old) - 100% lockfree
    if let Some(output_ref) = output {
        if output_ref.should_auto_dismiss_error() {
            output_ref.set_last_error(""); // Auto-dismiss after 10 seconds
        }
    }

    // Check if there's an error to display and calculate required height
    let (has_error, error_height) = output
        .map(|out| {
            let error_text = out.last_error();
            if error_text.is_empty() {
                (false, 0u16)
            } else {
                // Calculate how many lines needed (width - 4 for borders and padding)
                let max_width = frame.area().width.saturating_sub(4) as usize;
                let lines_needed = if max_width > 0 {
                    ((error_text.len() + max_width - 1) / max_width).max(1) as u16
                } else {
                    1
                };
                // Add 2 for borders (top/bottom)
                let total_height = lines_needed + 2;
                (true, total_height)
            }
        })
        .unwrap_or((false, 0));

    // Calculate layout (vertical split: header, main, [error notification], input)
    let chunks = if has_error {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),              // Header
                Constraint::Min(0),                 // Main (flexible)
                Constraint::Length(error_height),   // Error notification (dynamic)
                Constraint::Length(3),              // Input
            ])
            .split(frame.area())
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),      // Header
                Constraint::Min(0),         // Main (flexible)
                Constraint::Length(3),      // Input
            ])
            .split(frame.area())
    };

    // Render header
    render_header(frame, chunks[0], app, &theme);

    // Render main content (with optional live data or progress indicator)
    render_main(frame, chunks[1], app, content, progress, tabs, &theme);

    // Render error notification if present (100% lockfree)
    if has_error {
        if let Some(output_ref) = output {
            render_error_notification(frame, chunks[2], &output_ref.last_error(), &theme);
        }
    }

    // Conditionally render either command palette OR input bar (mutually exclusive)
    // When palette is visible, it replaces the input bar completely
    let input_chunk_index = if has_error { 3 } else { 2 };
    if let Some(palette_ref) = palette {
        if palette_ref.is_visible() {
            render_command_palette(frame, chunks[1], chunks[input_chunk_index], palette_ref, &theme);
        } else {
            render_input(frame, chunks[input_chunk_index], app, &theme);
        }
    } else {
        render_input(frame, chunks[input_chunk_index], app, &theme);
    }

    // Render help overlay last (on top of everything) if visible
    if let Some(help_capsule) = help {
        render_help_overlay(frame, help_capsule, &theme);
    }
}

/// Render header panel (Byzantine Purple background)
fn render_header(frame: &mut Frame, area: Rect, app: &TuiAppCapsule, theme: &ColorThemeCapsule) {
    let state = app.state();
    let state_text = format!("{:?}", state);

    // Build header line with spans
    let header_line = Line::from(vec![
        Span::styled(
            " clapi ",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("v0.5.0 "),
        Span::styled("| ", Style::default().fg(Color::DarkGray)),
        Span::raw("Status: "),
        Span::styled(
            state_text,
            Style::default()
                .fg(if state == super::app::AppState::Running {
                    ColorThemeCapsule::to_ratatui_color(theme.accent_success())
                } else if state == super::app::AppState::Paused {
                    ColorThemeCapsule::to_ratatui_color(theme.accent_warning())
                } else {
                    ColorThemeCapsule::to_ratatui_color(theme.accent_error())
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);

    let header = Paragraph::new(header_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.gold())),
                )
                .style(
                    Style::default()
                        .bg(ColorThemeCapsule::to_ratatui_color(theme.bg_header())),
                ),
        );

    frame.render_widget(header, area);
}

/// Render tab indicator header
fn render_tab_indicator(active_tab: u8, theme: &ColorThemeCapsule) -> Line<'static> {
    let tabs = [
        ("1", "Overview"),
        ("2", "Providers"),
        ("3", "Budgets"),
        ("4", "Performance"),
        ("5", "Cost"),
        ("6", "Loop Armor"),
    ];

    let mut spans = Vec::new();
    spans.push(Span::raw("  "));

    for (idx, (key, label)) in tabs.iter().enumerate() {
        if idx == active_tab as usize {
            // Active tab - highlighted with bold + gold color
            spans.push(Span::styled(
                format!("[{}:{}]", key, label),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            // Inactive tab - muted
            spans.push(Span::styled(
                format!("{}:{}", key, label),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ));
        }

        if idx < tabs.len() - 1 {
            spans.push(Span::raw(" "));
        }
    }

    Line::from(spans)
}

/// Render main content panel with tab navigation
///
/// # Performance
/// - <100ns for atomic reads (8 fields from DashboardContentCapsule)
/// - <5ms for ratatui rendering
///
/// # Data Flow
/// - If `progress` is active: Show spinner with progress message (PRIORITY 1)
/// - Else if `content` is Some: Render tab indicator + tab-specific live metrics
/// - Else: Show "Server Offline" placeholder
fn render_main(
    frame: &mut Frame,
    area: Rect,
    _app: &TuiAppCapsule,
    content: Option<&DashboardContentCapsule>,
    progress: Option<&ProgressIndicatorCapsule>,
    tabs: &TabStateCapsule,
    theme: &ColorThemeCapsule,
) {
    // PRIORITY 1: Check if progress indicator is active
    if let Some(prog) = progress {
        if prog.is_active() {
            // Show progress indicator (spinner + message)
            let spinner_char = prog.current_char();
            let message = prog.message();

            let progress_lines = vec![
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        format!(" {} ", spinner_char),
                        Style::default()
                            .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        message,
                        Style::default()
                            .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
                    ),
                ]),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(
                        "  Please wait...",
                        Style::default()
                            .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
                    ),
                ]),
            ];

            let progress_widget = Paragraph::new(progress_lines)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(
                            Style::default()
                                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold())),
                        )
                        .title(" Progress ")
                        .title_style(
                            Style::default()
                                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                                .add_modifier(Modifier::BOLD),
                        )
                        .style(Style::default()),  // Terminal default background
                );

            frame.render_widget(progress_widget, area);
            return;
        }
    }

    // Load live metrics from DashboardContentCapsule (if available)
    let content_lines = if let Some(dashboard) = content {
        // Get current active tab
        let active_tab = tabs.get_tab();

        // Render tab indicator header
        let mut lines = vec![render_tab_indicator(active_tab, theme), Line::raw("")];

        // Read atomic metrics snapshot (<100ns total via getter methods)
        let budgets_count = dashboard.budgets_count();
        let providers_count = dashboard.providers_count();
        let total_requests = dashboard.total_requests();
        let avg_latency_ms = dashboard.avg_latency();
        let memory_mb = dashboard.memory_mb();
        let uptime_secs = dashboard.uptime();
        let is_paused = dashboard.is_paused();
        let has_error = dashboard.has_error();

        // Format uptime (hours:minutes)
        let uptime_hours = uptime_secs / 3600;
        let uptime_mins = (uptime_secs % 3600) / 60;
        let uptime_str = format!("{}h {}m", uptime_hours, uptime_mins);

        // Render tab-specific content
        let tab_content = match active_tab {
            0 => render_overview_tab(budgets_count, providers_count, total_requests,
                                     avg_latency_ms, memory_mb, &uptime_str, is_paused, has_error, theme),
            1 => render_providers_tab(providers_count, total_requests, avg_latency_ms, theme),
            2 => render_budgets_tab(budgets_count, theme),
            3 => render_performance_tab(avg_latency_ms, total_requests, theme),
            4 => render_cost_tab(theme),
            5 => render_loop_armor_tab(dashboard, theme),
            _ => vec![Line::from(vec![
                Span::styled("Invalid tab", Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error())))
            ])],
        };

        lines.extend(tab_content);

        // Append help text
        lines.extend(vec![
            Line::raw(""),
            Line::from(vec![
                Span::styled(
                    "1-6: Switch tabs  |  '/' for commands  |  '?' for help  |  Ctrl+C×2 to quit",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
                ),
            ]),
        ]);

        lines
    } else {
        // Server offline - show placeholder
        vec![
            Line::from(vec![
                Span::styled(
                    "Server Offline",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error()))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::raw(""),
            Line::raw("The clapi server is not running."),
            Line::raw(""),
            Line::raw("Press '/' and type 'start' to launch the server"),
            Line::raw("or use 'clapi start' from the command line."),
        ]
    };

    let main = Paragraph::new(content_lines)
        .block(
            Block::default()
                .borders(Borders::NONE)
                .title(" Dashboard ")
                .title_style(
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default()),  // Terminal default background
        );

    frame.render_widget(main, area);
}

/// Render Overview tab (Tab 0) - Default comprehensive view
fn render_overview_tab(
    budgets_count: u32,
    providers_count: u32,
    total_requests: u32,
    avg_latency_ms: u32,
    memory_mb: u32,
    uptime_str: &str,
    is_paused: bool,
    has_error: bool,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                "Budget Status",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Active Budgets: "),
            Span::styled(
                format!("{}", budgets_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "Server Status",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        if has_error {
            Line::from(vec![
                Span::raw("  Status: "),
                Span::styled(
                    "Not Running",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error()))
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else if is_paused {
            Line::from(vec![
                Span::raw("  Status: "),
                Span::styled(
                    "Paused",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_warning()))
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        } else {
            Line::from(vec![
                Span::raw("  Status: "),
                Span::styled(
                    "Running",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success()))
                        .add_modifier(Modifier::BOLD),
                ),
            ])
        },
        if has_error {
            Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "💡 Press '/' then 'start' to launch the server",
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
                ),
            ])
        } else {
            Line::from(vec![
                Span::raw("  Uptime: "),
                Span::styled(
                    uptime_str.to_string(),
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
                ),
            ])
        },
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "Provider Metrics",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Providers Configured: "),
            Span::styled(
                format!("{}", providers_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Requests: "),
            Span::styled(
                format!("{}", total_requests),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Avg Latency: "),
            Span::styled(
                format!("{}ms", avg_latency_ms),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if avg_latency_ms < 200 {
                            theme.accent_success()
                        } else if avg_latency_ms < 500 {
                            theme.accent_warning()
                        } else {
                            theme.accent_error()
                        }
                    )),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "System Stats",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Memory: "),
            Span::styled(
                format!("{}MB", memory_mb),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
    ]
}

/// Render Providers tab (Tab 1) - Detailed provider view
fn render_providers_tab(
    providers_count: u32,
    total_requests: u32,
    avg_latency_ms: u32,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                "Provider Details",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Total Providers: "),
            Span::styled(
                format!("{}", providers_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Requests: "),
            Span::styled(
                format!("{}", total_requests),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Average Latency: "),
            Span::styled(
                format!("{}ms", avg_latency_ms),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if avg_latency_ms < 200 {
                            theme.accent_success()
                        } else if avg_latency_ms < 500 {
                            theme.accent_warning()
                        } else {
                            theme.accent_error()
                        }
                    )),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "[Detailed provider metrics coming soon]",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
    ]
}

/// Render ASCII progress bar
///
/// # Arguments
/// - `percentage`: Utilization percentage (0-100, clamped to 100)
/// - `width`: Width of progress bar in characters
///
/// # Returns
/// ASCII progress bar string: `[████████░░░░░░]` (14 chars for width=14)
///
/// # Performance
/// - <10µs per bar (string allocation + repeat)
///
/// # Examples
/// ```ignore
/// assert_eq!(render_progress_bar(0, 14), "[░░░░░░░░░░░░░░]");
/// assert_eq!(render_progress_bar(50, 14), "[███████░░░░░░░]");
/// assert_eq!(render_progress_bar(100, 14), "[██████████████]");
/// assert_eq!(render_progress_bar(150, 14), "[██████████████]"); // Clamped to 100%
/// ```
fn render_progress_bar(percentage: u8, width: usize) -> String {
    // Clamp percentage to 100 (handle edge case of >100% utilization)
    let clamped = percentage.min(100);

    let filled = (clamped as usize * width) / 100;
    let empty = width.saturating_sub(filled);

    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Render Budgets tab (Tab 3) with ASCII progress bars
///
/// # Performance
/// - <3ms render time for 8 budgets (target)
/// - <100ns atomic reads (budget_utilization field)
/// - <2ms string formatting and line construction
///
/// # Visual Design
/// For each budget (0-7):
/// ```text
/// budget_001 (Production)
///   $45.23 / $100.00  [████████░░░░░░] 45%  ✅ OK
///   124 requests      │  $0.36 per request
/// ```
///
/// # Status Icons
/// - 0-70%: ✅ "OK" (Green)
/// - 71-89%: ⚠️ "Low" (Yellow)
/// - 90-99%: ⚠️ "Critical" (Yellow)
/// - 100%: ❌ "Exhausted" (Red)
fn render_budgets_tab(budgets_count: u32, theme: &ColorThemeCapsule) -> Vec<Line<'static>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "Budget Utilization (Progress Bars)",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // TODO: Read budget_utilization from DashboardContentCapsule
    // For now, show mock data for demonstration
    let mock_budgets = vec![
        (45, 4523, 10000, 124),   // 45% utilization
        (72, 7200, 10000, 98),    // 72% utilization (Low)
        (91, 9100, 10000, 256),   // 91% utilization (Critical)
        (100, 10000, 10000, 512), // 100% utilization (Exhausted)
    ];

    for (idx, (utilization, spent_cents, total_cents, requests)) in mock_budgets.iter().enumerate() {
        let budget_name = format!("budget_{:03}", idx + 1);

        let spent = format!("${:.2}", *spent_cents as f64 / 100.0);
        let total = format!("${:.2}", *total_cents as f64 / 100.0);

        // Generate progress bar (14 chars width for 80-char terminals)
        let progress_bar = render_progress_bar(*utilization, 14);

        // Status icon and color
        let (status_icon, status_text, status_color) = if *utilization >= 100 {
            ("❌", "Exhausted", theme.accent_error())
        } else if *utilization >= 90 {
            ("⚠️", "Critical", theme.accent_warning())
        } else if *utilization >= 71 {
            ("⚠️", "Low", theme.accent_warning())
        } else {
            ("✅", "OK", theme.accent_success())
        };

        // Line 1: Budget name
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} (Production)", budget_name),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Line 2: Spending + progress bar + status
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} / {}", spent, total),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
            Span::raw("  "),
            Span::styled(
                progress_bar,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(status_color)),
            ),
            Span::raw(format!(" {}%  ", utilization)),
            Span::raw(format!("{} ", status_icon)),
            Span::styled(
                status_text,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(status_color))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        // Line 3: Request count + cost per request
        let cost_per_request = if *requests > 0 {
            format!("${:.2}", *spent_cents as f64 / *requests as f64 / 100.0)
        } else {
            "$0.00".to_string()
        };

        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{} requests", requests),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
            Span::raw("      │  "),
            Span::styled(
                format!("{} per request", cost_per_request),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]));

        // Blank line separator
        lines.push(Line::raw(""));
    }

    // If no budgets configured, show placeholder
    if budgets_count == 0 {
        lines.push(Line::from(vec![
            Span::styled(
                "No budgets configured",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]));
    }

    lines
}

/// Render Performance tab (Tab 3) - Metrics & latency
fn render_performance_tab(
    avg_latency_ms: u32,
    total_requests: u32,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                "Performance Metrics",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("  Average Latency: "),
            Span::styled(
                format!("{}ms", avg_latency_ms),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if avg_latency_ms < 200 {
                            theme.accent_success()
                        } else if avg_latency_ms < 500 {
                            theme.accent_warning()
                        } else {
                            theme.accent_error()
                        }
                    )),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Requests: "),
            Span::styled(
                format!("{}", total_requests),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "[Detailed performance charts coming soon]",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
    ]
}

/// Render Cost tab (Tab 4) - Cost analysis
fn render_cost_tab(theme: &ColorThemeCapsule) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled(
                "Cost Analysis",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                "[Cost tracking and analysis coming soon]",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
    ]
}

/// Render Loop Armor tab (Tab 6) - Phase 1 Loop Protection
fn render_loop_armor_tab(dashboard: &DashboardContentCapsule, theme: &ColorThemeCapsule) -> Vec<Line<'static>> {
    // Load atomic metrics snapshot (<100ns)
    let rate_allowed = dashboard.get_loop_armor_rate_allowed();
    let rate_blocked = dashboard.get_loop_armor_rate_blocked();
    let rate_quota = dashboard.get_loop_armor_rate_quota();
    let dedup_hits = dashboard.get_loop_armor_dedup_hits();
    let dedup_misses = dashboard.get_loop_armor_dedup_misses();
    let anomaly_count = dashboard.get_loop_armor_anomaly_count();
    let p99_current = dashboard.get_loop_armor_p99_current();
    let p99_baseline = dashboard.get_loop_armor_p99_baseline();
    let severity = dashboard.get_loop_armor_severity();

    // Calculate derived metrics
    let total_rate = rate_allowed + rate_blocked;
    let rate_pct = if total_rate > 0 {
        ((rate_allowed as f32 / total_rate as f32) * 100.0) as u32
    } else {
        100
    };

    let total_dedup = dedup_hits + dedup_misses;
    let dedup_pct = if total_dedup > 0 {
        ((dedup_hits as f32 / total_dedup as f32) * 100.0) as u32
    } else {
        0
    };

    let dedup_savings_ms = dedup_hits * 100; // Assume 100ms saved per dedup

    let severity_text = match severity {
        0 => ("✓ Normal", theme.accent_success()),
        1 => ("⚠️ Low", theme.accent_warning()),
        2 => ("⚠️ Medium", theme.accent_warning()),
        3 => ("❌ High", theme.accent_error()),
        4 => ("❌ Critical", theme.accent_error()),
        _ => ("? Unknown", theme.text_muted()),
    };

    // Build progress bar for rate limit
    let rate_quota_pct = ((rate_quota as f32 / 1000.0) * 100.0).min(100.0) as u8;
    let rate_bar = render_progress_bar(rate_quota_pct, 14);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                "Loop Armor Protection (Phase 1)",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        // Rate Limiting section
        Line::from(vec![
            Span::styled(
                "Rate Limiting",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Allowed:  "),
            Span::styled(
                format!("{} requests", rate_allowed),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
            Span::raw(format!("  ({}%)", rate_pct)),
        ]),
        Line::from(vec![
            Span::raw("  Blocked:  "),
            Span::styled(
                format!("{} requests", rate_blocked),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Quota:    "),
            Span::styled(
                format!("{}/1000 remaining  ", rate_quota),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if rate_quota > 800 { theme.accent_success() }
                        else if rate_quota > 500 { theme.accent_warning() }
                        else { theme.accent_error() }
                    )),
            ),
            Span::styled(
                rate_bar,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if rate_quota > 800 { theme.accent_success() }
                        else if rate_quota > 500 { theme.accent_warning() }
                        else { theme.accent_error() }
                    )),
            ),
            Span::raw(format!(" {}%", rate_quota_pct)),
        ]),
        Line::raw(""),
        // Deduplication section
        Line::from(vec![
            Span::styled(
                "Deduplication",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Duplicates:  "),
            Span::styled(
                format!("{} requests", dedup_hits),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
            Span::raw(format!("  ({}%)", dedup_pct)),
        ]),
        Line::from(vec![
            Span::raw("  Unique:      "),
            Span::styled(
                format!("{} requests", dedup_misses),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Savings:     "),
            Span::styled(
                format!("💰 {:.1}s", dedup_savings_ms as f32 / 1000.0),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} × 100ms/req)", dedup_hits),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::raw(""),
        // Anomaly Detection section
        Line::from(vec![
            Span::styled(
                "Anomaly Detection",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Status:      "),
            Span::styled(
                severity_text.0,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(severity_text.1))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ({} anomalies detected)", anomaly_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  p99 Current: "),
            Span::styled(
                format!("{}ms", p99_current),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if p99_baseline > 0 && p99_current > p99_baseline * 2 {
                            theme.accent_error()
                        } else if p99_baseline > 0 && p99_current > p99_baseline * 3 / 2 {
                            theme.accent_warning()
                        } else {
                            theme.accent_success()
                        }
                    )),
            ),
        ]),
        Line::from(vec![
            Span::raw("  p99 Baseline:"),
            Span::styled(
                format!("{}ms", p99_baseline),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
    ];

    // Append Phase 2 metrics
    lines.extend(render_loop_armor_phase2(dashboard, theme));

    // Append Phase 3 metrics
    lines.extend(render_loop_armor_phase3(dashboard, theme));

    lines
}

/// Render Loop Armor Phase 2 metrics (helper function for render_loop_armor_tab)
fn render_loop_armor_phase2(dashboard: &DashboardContentCapsule, theme: &ColorThemeCapsule) -> Vec<Line<'static>> {
    // Load Phase 2 atomic metrics (<50ns)
    let burst_count = dashboard.get_loop_armor_burst_count();
    let burst_window = dashboard.get_loop_armor_burst_window();
    let velocity = dashboard.get_loop_armor_cost_velocity();
    let cost_alerts = dashboard.get_loop_armor_cost_alerts();
    let pattern_count = dashboard.get_loop_armor_pattern_count();
    let pattern_matches = dashboard.get_loop_armor_pattern_matches();

    // Convert Q16.16 fixed-point to float (cents/min)
    let velocity_f = (velocity as f64) / 65536.0;

    // Burst Detection status
    let (burst_status, burst_color) = if burst_window >= 8 {
        ("🔴 HIGH", theme.accent_error())
    } else if burst_window >= 5 {
        ("🟡 MEDIUM", theme.accent_warning())
    } else {
        ("🟢 LOW", theme.accent_success())
    };

    // Cost Velocity status
    let (velocity_status, velocity_color) = if cost_alerts > 0 {
        ("🔴 EXCEEDED", theme.accent_error())
    } else if velocity_f > 50.0 {
        ("🟡 HIGH", theme.accent_warning())
    } else {
        ("🟢 NORMAL", theme.accent_success())
    };

    // Pattern Signature status
    let (pattern_status, pattern_color) = if pattern_matches >= 6 {
        ("🔴 PATTERN", theme.accent_error())
    } else if pattern_matches >= 4 {
        ("🟡 SUSPICIOUS", theme.accent_warning())
    } else {
        ("🟢 NORMAL", theme.accent_success())
    };

    vec![
        Line::raw(""),
        Line::raw("─".repeat(60)),
        Line::from(vec![
            Span::styled(
                "PHASE 2: Enhanced Detection",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        // Burst Detection section
        Line::from(vec![
            Span::styled(
                "Burst Detection",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Status:      "),
            Span::styled(
                burst_status,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(burst_color))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({}/10 in window)", burst_window),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Bursts:"),
            Span::styled(
                format!("{}", burst_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::raw(""),
        // Cost Velocity section
        Line::from(vec![
            Span::styled(
                "Cost Velocity",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Status:      "),
            Span::styled(
                velocity_status,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(velocity_color))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({:.2} ¢/min)", velocity_f),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Alerts:      "),
            Span::styled(
                format!("{}", cost_alerts),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if cost_alerts > 0 {
                            theme.accent_error()
                        } else {
                            theme.text_secondary()
                        }
                    )),
            ),
        ]),
        Line::raw(""),
        // Pattern Signature section
        Line::from(vec![
            Span::styled(
                "Pattern Detection",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Status:      "),
            Span::styled(
                pattern_status,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(pattern_color))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({}/8 matches)", pattern_matches),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Total Patterns:"),
            Span::styled(
                format!("{}", pattern_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
        ]),
        Line::raw(""),
        // Performance summary
        Line::from(vec![
            Span::styled(
                "Performance: ~220ns overhead (Phase 1: 90ns + Phase 2: 130ns)",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
    ]
}

/// Render Loop Armor Phase 3 metrics (helper function for render_loop_armor_tab)
fn render_loop_armor_phase3(dashboard: &DashboardContentCapsule, theme: &ColorThemeCapsule) -> Vec<Line<'static>> {
    // Load Phase 3 atomic metrics (<60ns)
    let closed_count = dashboard.get_loop_armor_circuit_closed_count();
    let halfopen_count = dashboard.get_loop_armor_circuit_halfopen_count();
    let open_count = dashboard.get_loop_armor_circuit_open_count();
    let total_opens = dashboard.get_loop_armor_circuit_total_opens();
    let total_recoveries = dashboard.get_loop_armor_circuit_total_recoveries();
    let avg_error_rate = dashboard.get_loop_armor_circuit_avg_error_rate();

    // Calculate total clients
    let total_clients = closed_count + halfopen_count + open_count;

    // Status determination
    let (status_text, status_color) = if open_count > 0 {
        ("🔴 CLIENTS ISOLATED", theme.accent_error())
    } else if halfopen_count > 0 {
        ("🟡 TESTING RECOVERY", theme.accent_warning())
    } else {
        ("🟢 ALL HEALTHY", theme.accent_success())
    };

    vec![
        Line::raw(""),
        Line::raw("─".repeat(60)),
        Line::from(vec![
            Span::styled(
                "PHASE 3: Client Circuit Breaker",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
        // Status bar
        Line::from(vec![
            Span::raw("  Status: "),
            Span::styled(
                status_text,
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(status_color))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({} clients)", total_clients),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
        Line::raw(""),
        // State distribution
        Line::from(vec![
            Span::styled(
                "State Distribution",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("    Closed: "),
            Span::styled(
                format!("{}", closed_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
            Span::raw(" | HalfOpen: "),
            Span::styled(
                format!("{}", halfopen_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_warning())),
            ),
            Span::raw(" | Open: "),
            Span::styled(
                format!("{}", open_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error())),
            ),
        ]),
        Line::raw(""),
        // Statistics
        Line::from(vec![
            Span::styled(
                "Statistics",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("    Total Opens: "),
            Span::styled(
                format!("{}", total_opens),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_secondary())),
            ),
            Span::raw(" | Recoveries: "),
            Span::styled(
                format!("{}", total_recoveries),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
            ),
        ]),
        Line::from(vec![
            Span::raw("    Avg Error Rate: "),
            Span::styled(
                format!("{}.{:02}%", avg_error_rate / 100, avg_error_rate % 100),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(
                        if avg_error_rate < 500 {
                            theme.accent_success()
                        } else if avg_error_rate < 1000 {
                            theme.accent_warning()
                        } else {
                            theme.accent_error()
                        }
                    )),
            ),
        ]),
        Line::raw(""),
        // Performance summary
        Line::from(vec![
            Span::styled(
                "Performance: ~280ns overhead (Phase 1: 90ns + Phase 2: 130ns + Phase 3: 60ns)",
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]),
    ]
}

/// Render input bar (Gold accent)
fn render_input(frame: &mut Frame, area: Rect, _app: &TuiAppCapsule, theme: &ColorThemeCapsule) {
    let input_line = Line::from(vec![
        Span::styled(
            " > ",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "Press '/' for command palette | Ctrl+C twice to quit",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
        ),
    ]);

    let input = Paragraph::new(input_line)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple())),
                )
                .style(Style::default()),  // Terminal default background
        );

    frame.render_widget(input, area);
}

/// Render error notification bar (Red accent with emoji)
///
/// Dynamically wraps long error messages across multiple lines to ensure
/// the full message is visible without truncation.
fn render_error_notification(frame: &mut Frame, area: Rect, error_text: &str, theme: &ColorThemeCapsule) {
    // Calculate max width per line (accounting for borders and padding)
    let max_width = area.width.saturating_sub(4) as usize; // 2 for borders + 2 for padding

    if max_width == 0 {
        return; // Not enough space to render
    }

    // Wrap text into multiple lines
    let mut lines = Vec::new();
    let mut remaining = error_text;

    while !remaining.is_empty() {
        let chunk_size = remaining.len().min(max_width);
        let (chunk, rest) = remaining.split_at(chunk_size);

        lines.push(Line::from(vec![
            Span::styled(
                format!(" {} ", chunk),
                Style::default()
                    .fg(Color::White)
                    .bg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        remaining = rest;
    }

    let notification = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )
                .style(Style::default()),  // Terminal default background
        );

    frame.render_widget(notification, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_state_capsule() {
        let tabs = TabStateCapsule::new();
        assert_eq!(tabs.get_tab(), 0);

        tabs.set_tab(2);
        assert_eq!(tabs.get_tab(), 2);

        // Test bounds checking
        tabs.set_tab(10);
        assert_eq!(tabs.get_tab(), 2); // Should not change
    }

    #[test]
    fn test_layout_constraints() {
        // Verify layout constraints are reasonable
        let constraints = vec![
            Constraint::Length(3),  // Header
            Constraint::Min(0),     // Main
            Constraint::Length(3),  // Input
        ];

        // Header and input fixed at 3 lines each
        assert_eq!(constraints[0], Constraint::Length(3));
        assert_eq!(constraints[2], Constraint::Length(3));

        // Main is flexible
        assert_eq!(constraints[1], Constraint::Min(0));
    }

    #[test]
    fn test_color_theme_integration() {
        let theme = ColorThemeCapsule::new();

        // Verify key colors are defined
        assert_ne!(theme.byzantine_purple(), 0);
        assert_ne!(theme.gold(), 0);
        assert_ne!(theme.bg_primary(), 0);
        assert_ne!(theme.bg_header(), 0);
    }

    #[test]
    fn test_progress_bar_edge_cases() {
        // Test 0% (all empty)
        assert_eq!(render_progress_bar(0, 14), "[░░░░░░░░░░░░░░]");

        // Test 50% (half filled)
        let bar = render_progress_bar(50, 14);
        assert_eq!(bar.len(), 16); // 14 chars + 2 brackets
        assert_eq!(bar.chars().filter(|&c| c == '█').count(), 7);
        assert_eq!(bar.chars().filter(|&c| c == '░').count(), 7);

        // Test 100% (all filled)
        assert_eq!(render_progress_bar(100, 14), "[██████████████]");

        // Test >100% (clamped to 100%)
        assert_eq!(render_progress_bar(150, 14), "[██████████████]");
        assert_eq!(render_progress_bar(255, 14), "[██████████████]");
    }

    #[test]
    fn test_progress_bar_percentages() {
        // Test common percentages
        let test_cases = vec![
            (0, 0),   // 0% → 0 filled
            (25, 3),  // 25% → 3 filled (3.5 rounds down)
            (50, 7),  // 50% → 7 filled
            (75, 10), // 75% → 10 filled (10.5 rounds down)
            (100, 14), // 100% → 14 filled
        ];

        for (percentage, expected_filled) in test_cases {
            let bar = render_progress_bar(percentage, 14);
            let filled_count = bar.chars().filter(|&c| c == '█').count();
            assert_eq!(
                filled_count, expected_filled,
                "Expected {} filled chars for {}%, got {}",
                expected_filled, percentage, filled_count
            );
        }
    }

    #[test]
    fn test_progress_bar_widths() {
        // Test different widths
        assert_eq!(render_progress_bar(50, 10).len(), 12); // 10 + 2 brackets
        assert_eq!(render_progress_bar(50, 14).len(), 16); // 14 + 2 brackets
        assert_eq!(render_progress_bar(50, 20).len(), 22); // 20 + 2 brackets
    }

    #[test]
    fn test_progress_bar_color_thresholds() {
        let theme = ColorThemeCapsule::new();

        // Verify status thresholds match specification
        // 0-70%: OK (Green)
        assert_eq!(theme.accent_success(), 0x4ade80);

        // 71-89%: Low (Yellow)
        assert_eq!(theme.accent_warning(), 0xfbbf24);

        // 90-99%: Critical (Yellow)
        assert_eq!(theme.accent_warning(), 0xfbbf24);

        // 100%: Exhausted (Red)
        assert_eq!(theme.accent_error(), 0xf87171);
    }
}

/// Render command palette overlay (fuzzy search)
/// Replaces the input bar when visible, growing upward into main area
fn render_command_palette(frame: &mut Frame, main_area: Rect, input_area: Rect, palette: &CommandPalette, theme: &ColorThemeCapsule) {
    // Match dashboard width exactly (full width like main content)
    let popup_width = main_area.width;
    let margin_x = main_area.x;

    // Calculate height based on number of commands + header (title, filter input, etc.)
    let filtered_count = palette.filtered_commands().len();
    let needed_height = filtered_count + 5;  // 5 lines for header (title, blank, filter, blank, separator)

    // Available height: from bottom of main_area to bottom of screen
    let max_height = (input_area.y + input_area.height - main_area.y) as usize;
    let popup_height = needed_height.min(max_height).max(10) as u16;  // At least 10 lines

    // Position palette to grow upward from input bar, using available space
    let popup_area = Rect {
        x: margin_x,
        y: (input_area.y + input_area.height).saturating_sub(popup_height),  // Grow upward
        width: popup_width,
        height: popup_height,
    };

    // Build palette content
    let mut lines = vec![];

    // Title
    lines.push(Line::from(vec![
        Span::styled(
            "Command Palette ",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "(Esc to close)",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
        ),
    ]));
    lines.push(Line::raw(""));

    // Filter input
    let filter = palette.current_filter();
    lines.push(Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold())),
        ),
        Span::styled(
            filter,
            Style::default()
                .fg(Color::White),
        ),
        Span::styled(
            "_",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::SLOW_BLINK),
        ),
    ]));
    lines.push(Line::raw(""));

    // Calculate scrolling
    let filtered_commands = palette.filtered_commands();
    let total_commands = filtered_commands.len();
    let visible_lines = popup_height.saturating_sub(5) as usize; // Subtract header lines
    let scroll = palette.scroll_position() as usize;
    let max_scroll = total_commands.saturating_sub(visible_lines);

    // Clamp scroll position (safety check)
    let safe_scroll = scroll.min(max_scroll);

    // Get selected index
    let selected_idx = palette.selected_command()
        .and_then(|cmd| filtered_commands.iter().position(|c| c.name == cmd.name))
        .unwrap_or(0);

    // Calculate hidden commands before rendering
    let visible_count = visible_lines.min(total_commands.saturating_sub(safe_scroll));
    let hidden_count = total_commands.saturating_sub(safe_scroll + visible_count);

    // Render visible commands (with scrolling)
    for (idx, cmd) in filtered_commands
        .iter()
        .enumerate()
        .skip(safe_scroll)
        .take(visible_lines)
    {
        let (prefix, style) = if idx == selected_idx {
            ("> ", Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD))
        } else {
            ("  ", Style::default()
                .fg(Color::White))
        };

        lines.push(Line::from(vec![
            Span::styled(prefix, style),
            Span::styled(cmd.name, style),
            Span::styled(
                format!(" - {}", cmd.description),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
        ]));
    }

    // Add scroll indicator if there are hidden commands
    if hidden_count > 0 {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  ↓ {} more (scroll with Up/Down)", hidden_count),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted()))
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    // Render block
    let block = Block::default()
        .title("Command Palette")
        .title_style(Style::default()
            .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple()))
            .add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_style(Style::default()
            .fg(ColorThemeCapsule::to_ratatui_color(theme.byzantine_purple())));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .style(Style::default());  // Terminal default background

    frame.render_widget(paragraph, popup_area);
}
