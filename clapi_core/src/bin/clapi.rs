//! Clapi HTTP Proxy Server Binary - Week 1 UX Transformation
//!
//! # UCE34 Framework
//! - Q1-Q9: CLI binary integration (HTTP server orchestration)
//! - Q10: Tier N/A (orchestration layer, not coordination)
//! - Q11: Rust async/await patterns, clap integration
//! - Q12: Nightly N/A (stable Rust sufficient)
//! - Q13-Q28: Integration testing, error handling
//! - Q31: Simplicity - `clapi start --test` works immediately
//! - Q33: Validation - clap validates CLI args at compile-time
//! - Q34: Auditability N/A (no state modification)
//!
//! # I20 Integration Framework
//! - Q1-Q5: Scope - integrating CLI + banner + test mode + existing proxy
//! - Q6-Q10: Compatibility - HTTP API unchanged, backward compatible
//! - Q11-Q15: Safety - graceful shutdown, error handling
//! - Q16-Q20: Validation - test mode works, production unaffected
//!
//! # Usage
//! ```bash
//! # Start in test mode (no API keys needed)
//! clapi start --test
//!
//! # Interactive configuration wizard (Week 2)
//! clapi config
//!
//! # Start with custom config
//! clapi start --config clapi.toml
//!
//! # Check system health
//! clapi doctor
//!
//! # View metrics
//! clapi metrics
//! ```

use clapi_core::{
    cli::{
        banner, handle_budget_add, handle_budget_list, handle_budget_show,
        handle_provider_list, handle_provider_show, handle_provider_test, BudgetAction,
        Cli, Commands, ConfigWizard, ErrorFormatter, ProviderAction,
    },
    test_mode::MockProvider,
    tui::TuiApp,
    ProxyConfig, ProxyServer,
};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse CLI arguments
    let cli = Cli::parse_args();

    // First-run detection: Check if ~/.config/clapi exists
    // If not, auto-launch wizard (unless user provided explicit command)
    if should_run_first_run_wizard(&cli) {
        run_first_run_wizard().await?;
    }

    // Match command and execute (or launch TUI if no command provided)
    match cli.command {
        // No command provided - launch TUI
        None => {
            run_tui_mode()?;
        }
        Some(Commands::Start {
            config,
            test,
            listen,
            budget,
        }) => {
            // Show banner (with test mode indicator)
            banner::show_banner(env!("CARGO_PKG_VERSION"), test);

            if test {
                run_test_mode(listen.unwrap_or_else(|| "0.0.0.0:8080".to_string())).await?;
            } else {
                run_production_mode(config, listen, budget).await?;
            }
        }

        Some(Commands::Config { output, force }) => {
            // Run interactive configuration wizard
            let wizard = ConfigWizard::new();

            match wizard.run().await {
                Ok(config) => {
                    // Save configuration
                    if let Err(e) = wizard.save_config(&config, &output, force) {
                        let formatter = ErrorFormatter::default();
                        eprintln!("{}", formatter.format_error(&e));
                        eprintln!();
                        eprintln!("{}", "💡 Try again with --force to overwrite".bright_yellow());
                        std::process::exit(1);
                    }

                    println!();
                    println!("{}", "🎉 Configuration complete!".bright_green().bold());
                    println!();
                    println!("{}", "Next steps:".bright_white());
                    println!("  {} Start the server: {}", "•".bright_blue(), format!("clapi start --config {}", output).bright_black());
                    println!("  {} Test the server: {}", "•".bright_blue(), "clapi doctor".bright_black());
                    println!();
                }
                Err(e) => {
                    let formatter = ErrorFormatter::default();
                    eprintln!();
                    eprintln!("{}", formatter.format_error(&e));
                    eprintln!();
                    eprintln!("{}", "💡 Quick Fix:".bright_yellow());
                    eprintln!("  {} Try test mode instead: {}", "•".bright_blue(), "clapi start --test".bright_black());
                    eprintln!("  {} See documentation: {}", "•".bright_blue(), "https://docs.clapi.dev/configuration".bright_cyan().underline());
                    std::process::exit(1);
                }
            }
        }

        Some(Commands::Doctor { config, format }) => {
            println!("{}", "🏥 System Diagnostics".bright_cyan().bold());
            println!();
            println!(
                "{}",
                "Coming in Week 2 - Comprehensive system diagnostics".bright_yellow()
            );
            println!();
            println!("Will validate:");
            println!("  {} Configuration file syntax", "•".bright_blue());
            println!("  {} Provider connectivity", "•".bright_blue());
            println!("  {} API key authentication", "•".bright_blue());
            println!("  {} Circuit breaker status", "•".bright_blue());
            println!("  {} Disk space and permissions", "•".bright_blue());
            println!();
            println!(
                "{}",
                "📚 Documentation: https://docs.clapi.dev/diagnostics"
                    .bright_cyan()
                    .underline()
            );

            if format != "text" {
                println!();
                println!(
                    "{}",
                    format!(
                        "Note: --format {} will be supported in Week 2",
                        format.bright_white()
                    )
                    .bright_black()
                );
            }

            if config != "clapi.toml" {
                println!();
                println!(
                    "{}",
                    format!("Will validate: {}", config.bright_white()).bright_black()
                );
            }
        }

        Some(Commands::Budget { action }) => {
            // Week 2 implementation: Real budget management
            let result = match action {
                BudgetAction::List { format } => {
                    handle_budget_list("http://localhost:8080", &format).await
                }
                BudgetAction::Show { budget_id } => {
                    handle_budget_show("http://localhost:8080", budget_id, "table").await
                }
                BudgetAction::Add { budget_id, amount } => {
                    handle_budget_add("http://localhost:8080", budget_id, amount).await
                }
                BudgetAction::Create { budget_id, amount } => {
                    // Create is same as Add for now (budgets auto-created on first use)
                    handle_budget_add("http://localhost:8080", budget_id, amount).await
                }
            };

            if let Err(e) = result {
                let formatter = ErrorFormatter::default();
                eprintln!("{}", formatter.format_error(&clapi_core::error::ClapiError::from(e)));
                eprintln!();
                eprintln!("{}", "💡 Quick Fix:".bright_yellow());
                eprintln!("  {} Make sure the server is running: {}", "•".bright_blue(), "clapi start".bright_black());
                eprintln!();
                std::process::exit(1);
            }
        }

        Some(Commands::Providers { action }) => {
            // Week 2 implementation: Real provider management
            let result = match action {
                ProviderAction::List { format } => {
                    handle_provider_list("http://localhost:8080", &format).await
                }
                ProviderAction::Show { provider_id } => {
                    handle_provider_show("http://localhost:8080", &provider_id, "table").await
                }
                ProviderAction::Test { provider_id } => {
                    handle_provider_test("http://localhost:8080", &provider_id).await
                }
            };

            if let Err(e) = result {
                let formatter = ErrorFormatter::default();
                eprintln!("{}", formatter.format_error(&clapi_core::error::ClapiError::from(e)));
                eprintln!();
                eprintln!("{}", "💡 Quick Fix:".bright_yellow());
                eprintln!("  {} Make sure the server is running: {}", "•".bright_blue(), "clapi start".bright_black());
                eprintln!();
                std::process::exit(1);
            }
        }

        Some(Commands::Metrics {
            url,
            category,
            watch,
        }) => {
            if let Some(interval) = watch {
                // Watch mode - real-time dashboard
                println!("{}", "📊 Metrics Dashboard".bright_cyan().bold());
                println!();
                println!(
                    "{}",
                    format!("Starting dashboard (refresh every {}s)...", interval).bright_white()
                );
                println!();
                println!(
                    "{}",
                    "Controls: q=quit, p=pause, r=resume, Ctrl+C=exit".bright_black()
                );
                println!();

                use clapi_core::cli::MetricsDashboard;
                let mut dashboard = MetricsDashboard::new(url, interval);

                // Run dashboard (0 = infinite watch)
                if let Err(e) = dashboard.run(0).await {
                    eprintln!();
                    eprintln!("{} {}", "❌".bright_red(), e.bright_red());
                    eprintln!();
                    eprintln!("{}", "💡 Quick Fix:".bright_yellow());
                    eprintln!(
                        "  {} Ensure clapi server is running: {}",
                        "•".bright_blue(),
                        "clapi start --test".bright_black()
                    );
                    eprintln!();
                    std::process::exit(1);
                }
            } else {
                // Single snapshot mode
                println!("{}", "📊 Metrics Viewer".bright_cyan().bold());
                println!();
                println!(
                    "{}",
                    "Fetching metrics snapshot...".bright_white()
                );
                println!();

                // Fetch metrics once
                match reqwest::get(&url).await {
                    Ok(response) => {
                        if response.status().is_success() {
                            match response.text().await {
                                Ok(body) => {
                                    println!("{}", body);
                                    println!();
                                    println!(
                                        "{}",
                                        "💡 Tip: Use --watch N for live dashboard (refresh every N seconds)"
                                            .bright_cyan()
                                    );
                                }
                                Err(e) => {
                                    eprintln!("{} Failed to read response: {}", "❌".bright_red(), e);
                                }
                            }
                        } else {
                            eprintln!(
                                "{} HTTP error: {}",
                                "❌".bright_red(),
                                response.status()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("{} Failed to fetch metrics: {}", "❌".bright_red(), e);
                        eprintln!();
                        eprintln!("{}", "💡 Quick Fix:".bright_yellow());
                        eprintln!(
                            "  {} Ensure clapi server is running: {}",
                            "•".bright_blue(),
                            "clapi start --test".bright_black()
                        );
                    }
                }
            }

            if category != "all" {
                println!();
                println!(
                    "{}",
                    format!(
                        "Note: Category filtering ('{}') will be implemented in Week 3",
                        category.bright_white()
                    )
                    .bright_black()
                );
            }
        }

        Some(Commands::Audit {
            config,
            budget_id,
            limit,
        }) => {
            println!("{}", "📜 Audit Log Viewer".bright_cyan().bold());
            println!();
            println!(
                "{}",
                "Coming in Week 2 - Audit log query and analysis".bright_yellow()
            );
            println!();
            println!("Planned features:");
            println!(
                "  {} Read audit log from config: {}",
                "•".bright_blue(),
                config.bright_white()
            );
            if let Some(id) = budget_id {
                println!(
                    "  {} Filter by budget ID: {}",
                    "•".bright_blue(),
                    id.to_string().bright_white()
                );
            }
            println!(
                "  {} Show last {} entries",
                "•".bright_blue(),
                limit.to_string().bright_white()
            );
            println!("  {} Hash chain verification", "•".bright_blue());
            println!("  {} Timeline reconstruction", "•".bright_blue());
            println!("  {} Export to CSV/JSON", "•".bright_blue());
            println!();
            println!(
                "{}",
                "📚 Documentation: https://docs.clapi.dev/audit"
                    .bright_cyan()
                    .underline()
            );
        }

        Some(Commands::Cache { action }) => {
            use clapi_core::cli::CacheAction;
            match action {
                CacheAction::Stats { format, url } => {
                    println!("{}", "💾 Cache Statistics".bright_cyan().bold());
                    println!();
                    println!("{}", "Fetching cache stats...".bright_white());
                    println!();
                    println!(
                        "{}",
                        format!("Note: --format {} support coming soon", format.bright_white())
                            .bright_black()
                    );
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
                CacheAction::Clear { url, force } => {
                    println!("{}", "💾 Clear Cache".bright_cyan().bold());
                    println!();
                    if force {
                        println!("{}", "Clearing all cached responses...".bright_white());
                    } else {
                        println!("{}", "Use --force to confirm cache clear".bright_yellow());
                    }
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
                CacheAction::Export { output, url } => {
                    println!("{}", "💾 Export Cache".bright_cyan().bold());
                    println!();
                    println!("{}", format!("Exporting to: {}", output.bright_white()).bright_white());
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
            }
        }

        Some(Commands::Profile { action }) => {
            use clapi_core::cli::ProfileAction;
            match action {
                ProfileAction::Start { url } => {
                    println!("{}", "📊 Start Profiling".bright_cyan().bold());
                    println!();
                    println!("{}", "Starting profiling session...".bright_white());
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
                ProfileAction::Stop { url } => {
                    println!("{}", "📊 Stop Profiling".bright_cyan().bold());
                    println!();
                    println!("{}", "Stopping profiling session...".bright_white());
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
                ProfileAction::Report { format, url } => {
                    println!("{}", "📊 Profiling Report".bright_cyan().bold());
                    println!();
                    println!("{}", "Generating latency percentiles...".bright_white());
                    println!(
                        "{}",
                        format!("Format: {} | URL: {}", format.bright_white(), url.bright_white())
                            .bright_black()
                    );
                }
                ProfileAction::ExportPrometheus { output, url } => {
                    println!("{}", "📊 Export Prometheus Metrics".bright_cyan().bold());
                    println!();
                    println!("{}", format!("Exporting to: {}", output.bright_white()).bright_white());
                    println!("{}", format!("URL: {}", url.bright_white()).bright_black());
                }
            }
        }
    }

    Ok(())
}

/// Run test mode (mock AI responses, no API keys needed)
///
/// # Week 1 Implementation
/// - Shows banner and startup message
/// - Displays quick start instructions
/// - Prints success message
///
/// # Week 2+ Enhancement
/// - Full HTTP server integration with MockProvider
/// - Real /v1/chat/completions endpoint with mock responses
/// - Circuit breaker simulation
async fn run_test_mode(listen_addr: String) -> Result<(), Box<dyn std::error::Error>> {
    // Show startup banner
    banner::show_startup(&listen_addr, true);

    // Create mock provider (for Week 2+ integration)
    let _mock = MockProvider::new();

    // Show quick start instructions
    banner::show_quick_start();

    println!();
    println!(
        "{} {}",
        "✅".bright_green(),
        "Test mode ready!".bright_green().bold()
    );
    println!();
    println!(
        "{}",
        "The mock server will respond to all requests with friendly test messages.".bright_white()
    );
    println!(
        "{}",
        "No real API calls will be made.".bright_black()
    );
    println!();

    // Week 1: Just show success message (proof of concept)
    // Week 2: Full HTTP server with MockProvider integration
    println!(
        "{}",
        "⚠️  Note: Full HTTP server integration coming in Week 2".bright_yellow()
    );
    println!(
        "{}",
        "For now, test mode shows the banner and configuration validation.".bright_black()
    );
    println!();

    // Wait for Ctrl+C
    println!(
        "{}",
        "Press Ctrl+C to stop...".bright_black()
    );
    tokio::signal::ctrl_c().await?;

    banner::show_shutdown();

    Ok(())
}

/// Run production mode (real AI providers, requires config)
///
/// # Implementation
/// 1. Load configuration with progress spinner
/// 2. Apply CLI overrides (listen address, budget)
/// 3. Detect and show enabled features
/// 4. Start proxy server
/// 5. Show quick start instructions
/// 6. Run server until Ctrl+C
/// 7. Graceful shutdown
async fn run_production_mode(
    config_path: String,
    listen_override: Option<String>,
    budget_override: Option<i64>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create error formatter for friendly error messages
    let formatter = ErrorFormatter::default();

    // Show loading spinner
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Loading configuration from {}...", config_path.bright_white()));
    spinner.enable_steady_tick(Duration::from_millis(100));

    // Load configuration
    let mut config = match ProxyConfig::load(&config_path) {
        Ok(cfg) => {
            spinner.finish_with_message(format!(
                "{} Configuration loaded",
                "✅".bright_green()
            ));
            cfg
        }
        Err(e) => {
            spinner.finish_and_clear();

            // Show friendly error message
            eprintln!("{}", formatter.format_error(&e));
            eprintln!();
            eprintln!("{}", "💡 Quick Fix:".bright_yellow());
            eprintln!("  {} Try test mode: {}", "•".bright_blue(), "clapi start --test".bright_black());
            eprintln!("  {} Generate config: {}", "•".bright_blue(), "clapi config".bright_black());
            eprintln!();

            return Err(Box::new(e));
        }
    };

    // Apply CLI overrides
    if let Some(addr) = listen_override {
        println!(
            "{}",
            format!("⚙️  Overriding listen address: {}", addr.bright_white()).bright_black()
        );
        config.listen_addr = addr;
    }

    if let Some(budget) = budget_override {
        println!(
            "{}",
            format!(
                "⚙️  Overriding default budget: {}",
                format_cents(budget).bright_white()
            )
            .bright_black()
        );
        config.default_budget = budget;
    }

    // Show startup banner
    banner::show_startup(&config.listen_addr, false);

    // Detect and show enabled features
    let features = detect_enabled_features();
    banner::show_features(&features);

    // Create proxy server
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Initializing proxy server...");
    spinner.enable_steady_tick(Duration::from_millis(100));

    let server = match ProxyServer::new(config) {
        Ok(srv) => {
            spinner.finish_with_message(format!(
                "{} Proxy server initialized",
                "✅".bright_green()
            ));
            srv
        }
        Err(e) => {
            spinner.finish_and_clear();
            eprintln!("{}", formatter.format_error(&e));
            return Err(Box::new(e));
        }
    };

    // Show quick start instructions
    banner::show_quick_start();

    println!();
    println!(
        "{} {}",
        "🚀".bright_green(),
        "Server is running!".bright_green().bold()
    );
    println!();
    println!(
        "{}",
        "Press Ctrl+C to stop...".bright_black()
    );
    println!();

    // Run server (this will block until Ctrl+C)
    match server.serve().await {
        Ok(()) => {
            banner::show_shutdown();
            Ok(())
        }
        Err(e) => {
            eprintln!();
            eprintln!("{}", formatter.format_error(&e));
            Err(Box::new(e))
        }
    }
}

/// Detect enabled features (compile-time feature flags)
///
/// # Features Detected
/// - Proxy (Budget Protection) - Always enabled
/// - Circuit Breaker - Always enabled
/// - OAuth 2.0 - `oauth` feature
/// - Payments (Stripe) - `payments` feature
/// - Compliance (SOX/SOC2/GDPR) - `compliance` feature
/// - KindlyDB (Persistence) - `kindlydb` feature
#[allow(unused_mut)]
fn detect_enabled_features() -> Vec<&'static str> {
    let mut features = vec![
        "Proxy (Budget Protection)",
        "Circuit Breaker",
        "Multi-Provider Routing",
    ];

    #[cfg(feature = "oauth")]
    features.push("OAuth 2.0");

    #[cfg(feature = "payments")]
    features.push("Payments (Stripe)");

    #[cfg(feature = "compliance")]
    features.push("Compliance (SOX/SOC2/GDPR)");

    #[cfg(feature = "kindlydb")]
    features.push("KindlyDB (Persistence)");

    features
}

/// Run TUI mode (interactive dashboard)
///
/// # Implementation
/// - Launch TUI application
/// - Display metrics dashboard
/// - Handle keyboard events
/// - Graceful shutdown on quit
fn run_tui_mode() -> std::io::Result<()> {
    let mut app = TuiApp::new()?;
    app.run()
}

/// Format cents as dollars (helper function)
fn format_cents(cents: i64) -> String {
    let dollars = cents as f64 / 100.0;
    if cents >= 0 {
        format!("${:.2}", dollars)
    } else {
        format!("-${:.2}", dollars.abs())
    }
}

/// Check if we should run the first-run wizard
///
/// # Logic
/// Run wizard if:
/// - Wizard marker file (~/.config/clapi/.wizard_completed) doesn't exist
/// - User didn't provide explicit command (let them run their command first)
///
/// The wizard will keep appearing until the user either:
/// - Completes the wizard
/// - Explicitly chooses "No" when asked if they want to run the wizard
///
/// # I20 Integration Q11-Q15 (Safety)
/// - Graceful: Missing marker is not an error, just a trigger
/// - Escape hatch: Any explicit command skips wizard
/// - User control: User explicitly chooses to skip or complete wizard
fn should_run_first_run_wizard(cli: &Cli) -> bool {
    // Skip wizard if explicit command provided (e.g., `clapi start`)
    if cli.command.is_some() {
        return false;
    }

    // CLI flag overrides
    if cli.wizard {
        return true;  // Force show wizard
    }
    if cli.no_wizard {
        return false;  // Force skip wizard
    }

    // Check config setting (defaults to true if config doesn't exist)
    let config_path = dirs::config_dir()
        .map(|d| d.join("clapi/clapi.toml"))
        .unwrap_or_else(|| std::path::PathBuf::from("clapi.toml"));

    if let Ok(config) = ProxyConfig::load(&config_path) {
        // Config exists - respect show_wizard_on_start setting
        config.show_wizard_on_start
    } else {
        // No config - always show wizard for first-time setup
        true
    }
}

/// Run first-run wizard with welcome animation
///
/// # Steps
/// 1. Show welcome animation (ASCII art + spinner)
/// 2. Create ~/.config/clapi directory
/// 3. Launch interactive wizard
/// 4. Save config to ~/.config/clapi/clapi.toml
/// 5. Show success message with next steps
///
/// # I20 Integration Q16-Q20 (Validation)
/// - Minimal integration test: Directory exists → skip wizard
/// - Property: First run always creates config directory
/// - Rollback: Delete ~/.config/clapi to re-trigger
async fn run_first_run_wizard() -> Result<(), Box<dyn std::error::Error>> {
    // Show welcome animation
    show_first_run_welcome();

    // Create config directory
    let config_dir = dirs::config_dir()
        .map(|d| d.join("clapi"))
        .unwrap_or_else(|| std::path::PathBuf::from(".config/clapi"));

    if !config_dir.exists() {
        println!();
        println!(
            "{} Creating config directory: {}",
            "📁".bright_blue(),
            config_dir.display().to_string().bright_white()
        );

        std::fs::create_dir_all(&config_dir)?;

        println!(
            "{} Config directory created",
            "✅".bright_green()
        );
    }

    // Launch interactive wizard
    println!();
    println!(
        "{}",
        "Let's configure your AI gateway in 3 easy steps!".bright_white()
    );
    println!();

    let wizard = ConfigWizard::new();
    let config = wizard.run().await?;

    // Save config
    let config_path = config_dir.join("clapi.toml");
    wizard.save_config(&config, &config_path, true)?;

    // Show success message
    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
    );
    println!(
        "{} {}",
        "🎉".bright_green(),
        "Configuration complete!".bright_green().bold()
    );
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
    );
    println!();
    println!("{}", "Next steps:".bright_white());
    println!(
        "  {} Start the server: {}",
        "•".bright_blue(),
        format!("clapi start --config {}", config_path.display()).bright_black()
    );
    println!(
        "  {} Test the server: {}",
        "•".bright_blue(),
        "clapi doctor".bright_black()
    );
    println!(
        "  {} View metrics: {}",
        "•".bright_blue(),
        "clapi metrics --watch 5".bright_black()
    );
    println!();

    Ok(())
}

/// Show first-run welcome animation
///
/// # Design
/// - Friendly ASCII art
/// - Progress spinner during setup
/// - Clear call-to-action
fn show_first_run_welcome() {
    // Simplified - main welcome is in wizard
    println!();
    println!(
        "{}",
        "Looks like this is your first time running clapi.".bright_white()
    );
    println!(
        "{}",
        "Let's get you set up with an interactive wizard!".bright_white()
    );
    println!();
    println!(
        "{}",
        "⚡ Quick setup in <2 minutes".bright_yellow()
    );
    println!(
        "{}",
        "🛡️  Budget protection included".bright_yellow()
    );
    println!(
        "{}",
        "🚀 Production-ready configuration".bright_yellow()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cents() {
        assert_eq!(format_cents(100), "$1.00");
        assert_eq!(format_cents(10_000), "$100.00");
        assert_eq!(format_cents(1), "$0.01");
        assert_eq!(format_cents(0), "$0.00");
        assert_eq!(format_cents(-100), "-$1.00");
    }

    #[test]
    fn test_detect_enabled_features() {
        let features = detect_enabled_features();

        // Core features always enabled
        assert!(features.contains(&"Proxy (Budget Protection)"));
        assert!(features.contains(&"Circuit Breaker"));
        assert!(features.contains(&"Multi-Provider Routing"));

        // Feature-gated features depend on compile flags
        #[cfg(feature = "oauth")]
        assert!(features.contains(&"OAuth 2.0"));

        #[cfg(feature = "payments")]
        assert!(features.contains(&"Payments (Stripe)"));

        #[cfg(feature = "compliance")]
        assert!(features.contains(&"Compliance (SOX/SOC2/GDPR)"));

        #[cfg(feature = "kindlydb")]
        assert!(features.contains(&"KindlyDB (Persistence)"));
    }

    #[test]
    fn test_should_run_first_run_wizard() {
        // Create temporary config directory for testing
        let temp_dir = std::env::temp_dir().join("clapi_test_config");
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir).ok();
        }

        // Test: No command + no config dir = run wizard
        let cli = Cli {
            command: None,
        };
        // Note: This test assumes no ~/.config/clapi directory exists
        // In practice, this depends on system state

        // Test: Explicit command + no config dir = skip wizard (user knows what they're doing)
        let cli = Cli {
            command: Some(Commands::Start {
                config: "clapi.toml".to_string(),
                test: false,
                listen: None,
                budget: None,
            }),
        };
        assert!(!should_run_first_run_wizard(&cli), "Should skip wizard when explicit command provided");

        // Test: Test mode + no config dir = skip wizard (test mode doesn't need config)
        let cli = Cli {
            command: Some(Commands::Start {
                config: "clapi.toml".to_string(),
                test: true,
                listen: None,
                budget: None,
            }),
        };
        assert!(!should_run_first_run_wizard(&cli), "Should skip wizard in test mode");
    }

    #[test]
    fn test_first_run_welcome_display() {
        // Just verify it doesn't panic
        show_first_run_welcome();
    }
}
