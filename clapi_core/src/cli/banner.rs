//! Welcome Banner - Kindly Branding
//!
//! # Purpose
//! Provides a friendly, emoji-enhanced welcome banner for clapi with:
//! - ASCII art logo
//! - Version information
//! - Quick start instructions
//! - Links to documentation and support
//!
//! # Design
//! - Minimal and friendly (not overwhelming)
//! - Informative (shows key info at startup)
//! - Actionable (includes next steps)
//!
//! # UCE34 Framework
//! - Q31 (Simplicity): Clear, concise, helpful
//! - Q32 (Constraints): Works on all terminals (ASCII-safe)

use colored::Colorize;

/// Display the welcome banner with Kindly branding
///
/// # Output
/// - ASCII art logo
/// - Version and status
/// - Quick start instructions
/// - Documentation links
///
/// # Example Output
/// ```text
///  ██████╗██╗      █████╗ ██████╗ ██╗
/// ██╔════╝██║     ██╔══██╗██╔══██╗██║
/// ██║     ██║     ███████║██████╔╝██║
/// ██║     ██║     ██╔══██║██╔═══╝ ██║
/// ╚██████╗███████╗██║  ██║██║     ██║
///  ╚═════╝╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝
///
/// AI Gateway with Budget Protection
/// from Kindly
///
/// Version: 0.4.8 | Status: Production Ready ✅
/// ```
pub fn show_banner(version: &str, test_mode: bool) {
    let banner = format!(
        r#"
 ██████╗██╗      █████╗ ██████╗ ██╗
██╔════╝██║     ██╔══██╗██╔══██╗██║
██║     ██║     ███████║██████╔╝██║
██║     ██║     ██╔══██║██╔═══╝ ██║
╚██████╗███████╗██║  ██║██║     ██║
 ╚═════╝╚══════╝╚═╝  ╚═╝╚═╝     ╚═╝

{}
{}

Version: {} | Status: {}
"#,
        "AI Gateway with Budget Protection".bright_cyan().bold(),
        "from Kindly".bright_blue(),
        version.bright_white(),
        if test_mode {
            "🧪 Test Mode".bright_yellow()
        } else {
            "Production Ready ✅".bright_green()
        }
    );

    println!("{}", banner);

    if test_mode {
        println!(
            "{}\n",
            "⚠️  Test mode enabled - using mock AI responses (no real API calls)"
                .bright_yellow()
        );
    }
}

/// Display quick start instructions
///
/// # Purpose
/// Shown after successful startup to guide users on next steps
pub fn show_quick_start() {
    println!("{}", "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());
    println!("{}", "Quick Start".bright_cyan().bold());
    println!("{}", "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());

    println!("\n{} {}", "→".bright_blue(), "Test the API:".bright_white());
    println!(
        "  {}",
        "curl http://localhost:8080/health".bright_black()
    );

    println!("\n{} {}", "→".bright_blue(), "Make a request:".bright_white());
    println!(
        "  {}",
        r#"curl -X POST http://localhost:8080/v1/chat/completions \"#.bright_black()
    );
    println!(
        "    {}",
        r#"-H "Content-Type: application/json" \"#.bright_black()
    );
    println!(
        "    {}",
        r#"-d '{"model": "gpt-4", "messages": [{"role": "user", "content": "Hello!"}]}'"#.bright_black()
    );

    println!("\n{} {}", "→".bright_blue(), "View metrics:".bright_white());
    println!(
        "  {}",
        "curl http://localhost:8080/metrics".bright_black()
    );

    println!("\n{} {}", "→".bright_blue(), "Documentation:".bright_white());
    println!("  {}", "https://docs.clapi.dev".bright_cyan().underline());

    println!("\n{} {}", "→".bright_blue(), "Need help?".bright_white());
    println!(
        "  {}",
        "https://kindly.feedback".bright_cyan().underline()
    );

    println!("{}", "\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black());
}

/// Display server startup message with emoji
///
/// # Arguments
/// - `listen_addr`: Server listen address (e.g., "0.0.0.0:8080")
/// - `test_mode`: Whether test mode is enabled
pub fn show_startup(listen_addr: &str, test_mode: bool) {
    if test_mode {
        println!(
            "\n{} Server starting in {} mode on {}",
            "🧪".bright_yellow(),
            "TEST".bright_yellow().bold(),
            listen_addr.bright_white().bold()
        );
        println!(
            "   {}",
            "Mock AI responses enabled (no real API calls)".bright_black()
        );
    } else {
        println!(
            "\n{} Server starting on {}",
            "🚀".bright_green(),
            listen_addr.bright_white().bold()
        );
    }
}

/// Display shutdown message with emoji
pub fn show_shutdown() {
    println!(
        "\n{} {}",
        "👋".bright_blue(),
        "Server shutting down gracefully...".bright_white()
    );
}

/// Display feature status (what's enabled)
///
/// # Arguments
/// - `features`: List of enabled features
pub fn show_features(features: &[&str]) {
    if features.is_empty() {
        return;
    }

    println!("\n{} {}", "✨".bright_yellow(), "Enabled features:".bright_white());
    for feature in features {
        println!("   {} {}", "•".bright_blue(), feature.bright_black());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_banner_display() {
        // Just verify it doesn't panic
        show_banner("0.4.8", false);
        show_banner("0.4.8", true);
    }

    #[test]
    fn test_quick_start_display() {
        // Just verify it doesn't panic
        show_quick_start();
    }

    #[test]
    fn test_startup_display() {
        // Just verify it doesn't panic
        show_startup("0.0.0.0:8080", false);
        show_startup("0.0.0.0:8080", true);
    }

    #[test]
    fn test_shutdown_display() {
        // Just verify it doesn't panic
        show_shutdown();
    }

    #[test]
    fn test_features_display() {
        // Just verify it doesn't panic
        show_features(&["OAuth", "Payments", "Compliance"]);
        show_features(&[]);
    }
}
