//! Tab-specific rendering functions for the TUI dashboard
//!
//! Each tab has a dedicated render function that takes metrics and returns
//! a vector of Lines for display.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::colors::ColorThemeCapsule;

/// Render tab indicator header showing current active tab
pub fn render_tab_indicator(active_tab: u8, theme: &ColorThemeCapsule) -> Line<'static> {
    let tabs = [
        ("1", "Overview"),
        ("2", "Providers"),
        ("3", "Budgets"),
        ("4", "Perf"),
        ("5", "Cost"),
    ];

    let mut spans = vec![];
    for (i, (key, name)) in tabs.iter().enumerate() {
        if i as u8 == active_tab {
            // Active tab - bold with brackets
            spans.push(Span::styled(
                format!("[{}:{}]", key, name),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            // Inactive tab - dimmed
            spans.push(Span::styled(
                format!(" {}:{} ", key, name),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ));
        }
    }

    Line::from(spans)
}

/// Render Overview tab - Quick status summary + recent activity
#[allow(clippy::too_many_arguments)]
pub fn render_overview_tab(
    budgets_count: u32,
    providers_count: u32,
    total_requests: u64,
    avg_latency_ms: u32,
    memory_mb: u32,
    uptime_str: String,
    is_paused: bool,
    has_error: bool,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    let mut lines = vec![];

    // System Status
    lines.push(Line::from(vec![
        Span::styled(
            "System Status",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Status indicator
    let (status_icon, status_text, status_color) = if has_error {
        ("❌", "Failing", theme.accent_error())
    } else if is_paused {
        ("⚠️", "Paused", theme.accent_warning())
    } else {
        ("✅", "Healthy", theme.accent_success())
    };

    lines.push(Line::from(vec![
        Span::raw("  Status: "),
        Span::styled(
            format!("{} {}", status_icon, status_text),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(status_color)),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Uptime: "),
        Span::styled(
            uptime_str,
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::raw(""));

    // Quick Metrics
    lines.push(Line::from(vec![
        Span::styled(
            "Quick Metrics",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::raw("  Active Budgets: "),
        Span::styled(
            format!("{}", budgets_count),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Providers: "),
        Span::styled(
            format!("{}", providers_count),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Total Requests: "),
        Span::styled(
            format!("{}", total_requests),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::from(vec![
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
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Memory: "),
        Span::styled(
            format!("{}MB", memory_mb),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines
}

/// Render Providers tab - Per-provider circuit breaker states
pub fn render_providers_tab(
    providers_count: u32,
    _total_requests: u64,
    _avg_latency_ms: u32,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "Provider Status",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Mock provider data (in real implementation, read from DashboardContentCapsule)
    for i in 0..providers_count.min(8) {
        let provider_name = match i {
            0 => "Anthropic",
            1 => "OpenAI",
            2 => "Mistral",
            3 => "Cohere",
            _ => "Generic",
        };

        // Mock status (would come from capsule)
        let (status_icon, status_text, status_color) = if i == 0 {
            ("✅", "Healthy", theme.accent_success())
        } else if i == 1 {
            ("⚠️", "Degraded", theme.accent_warning())
        } else {
            ("❌", "Failing", theme.accent_error())
        };

        lines.push(Line::from(vec![
            Span::raw(format!("  {}: ", provider_name)),
            Span::styled(
                format!("{} {}", status_icon, status_text),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(status_color)),
            ),
        ]));
    }

    lines
}

/// Render Budgets tab - ASCII progress bars and utilization
pub fn render_budgets_tab(
    budgets_count: u32,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "Budget Utilization",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Mock budget data (in real implementation, read from DashboardContentCapsule)
    for i in 0..budgets_count.min(8) {
        let budget_name = format!("Budget {}", i + 1);
        let utilization = (i * 15) % 100; // Mock utilization percentage

        // Build ASCII progress bar
        let bar_width = 20;
        let filled = (utilization as usize * bar_width) / 100;
        let empty = bar_width - filled;

        let bar_color = if utilization < 70 {
            theme.accent_success()
        } else if utilization < 90 {
            theme.accent_warning()
        } else {
            theme.accent_error()
        };

        lines.push(Line::from(vec![
            Span::raw(format!("  {}: [", budget_name)),
            Span::styled(
                "█".repeat(filled),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(bar_color)),
            ),
            Span::styled(
                "░".repeat(empty),
                Style::default()
                    .fg(ColorThemeCapsule::to_ratatui_color(theme.text_muted())),
            ),
            Span::raw(format!("] {}%", utilization)),
        ]));
    }

    lines
}

/// Render Performance tab - Latency distribution (P50/P99/P999)
pub fn render_performance_tab(
    avg_latency_ms: u32,
    total_requests: u64,
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "Performance Metrics",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Mock percentile data (in real implementation, read from DashboardContentCapsule)
    let p50 = avg_latency_ms;
    let p99 = avg_latency_ms * 2;
    let p999 = avg_latency_ms * 3;

    lines.push(Line::from(vec![
        Span::raw("  P50:  "),
        Span::styled(
            format!("{}ms", p50),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_success())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  P99:  "),
        Span::styled(
            format!("{}ms", p99),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_warning())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  P999: "),
        Span::styled(
            format!("{}ms", p999),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.accent_error())),
        ),
    ]));

    lines.push(Line::raw(""));

    lines.push(Line::from(vec![
        Span::raw("  Total Requests: "),
        Span::styled(
            format!("{}", total_requests),
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines
}

/// Render Cost tab - Spending tracking and burn rate
pub fn render_cost_tab(
    theme: &ColorThemeCapsule,
) -> Vec<Line<'static>> {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "Cost Tracking",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.gold()))
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::raw(""));

    // Mock cost data (in real implementation, read from DashboardContentCapsule)
    lines.push(Line::from(vec![
        Span::raw("  Total Spent: "),
        Span::styled(
            "$0.00",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Burn Rate: "),
        Span::styled(
            "$0.00/day",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Projected (30d): "),
        Span::styled(
            "$0.00",
            Style::default()
                .fg(ColorThemeCapsule::to_ratatui_color(theme.text_primary())),
        ),
    ]));

    lines
}
