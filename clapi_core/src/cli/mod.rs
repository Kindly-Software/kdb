//! CLI Framework - Kindly User Experience
//!
//! # Purpose
//! Provides a friendly, emoji-enhanced command-line interface for clapi with:
//! - Interactive configuration wizard
//! - Rich error messages with actionable fixes
//! - Progress indicators and visual feedback
//! - Comprehensive diagnostics
//!
//! # Design Principles
//! - Progressive Disclosure: Simple defaults, advanced options available
//! - Instant Gratification: Working in <60 seconds
//! - Actionable Errors: Every error includes explanation + fix + docs link
//! - Visual Feedback: Emojis for status, spinners for loading, colors for severity
//!
//! # UCE34 Framework
//! - Q31 (Simplicity): One-line installer, zero-config test mode
//! - Q32 (Constraints): Minimal dependencies, works on all platforms
//! - Q33 (Validation): Comprehensive help text, input validation

pub mod banner;
pub mod budget_provider_cli;
pub mod cache_commands;
pub mod dashboard;
pub mod doctor;
pub mod error_formatter;
pub mod first_run;
pub mod handlers;  // Command handlers for CLI and TUI reuse
pub mod profile_commands;
pub mod tui;  // TUI widgets (SelectWidget, InputWidget, ConfirmWidget)
pub mod wizard;

pub use budget_provider_cli::{
    handle_budget_add, handle_budget_list, handle_budget_show, handle_provider_list,
    handle_provider_show, handle_provider_test, BudgetStatus, CliError, ProviderStatus,
};
pub use cache_commands::{handle_cache_clear, handle_cache_export, handle_cache_stats};
pub use dashboard::MetricsDashboard;
pub use doctor::{DiagnosticReport, OutputFormat, Status, SystemDoctor};
pub use error_formatter::{ErrorFormatter, Verbosity};
// handlers module is available via mod declaration above (no pub use needed)
pub use profile_commands::{
    handle_profile_export_prometheus, handle_profile_report, handle_profile_start,
    handle_profile_stop,
};
pub use wizard::{
    CacheConfig, CompressionConfig, ConfigWizard, LoadBalancerConfig, PerformanceConfig,
    ProfilingConfig,
};

use clap::{Parser, Subcommand};

/// clapi - AI Gateway with Budget Protection from Kindly
///
/// Protect your AI budgets with lockfree budget tracking, circuit breaker failover,
/// and comprehensive audit trails.
#[derive(Parser)]
#[command(name = "clapi")]
#[command(author = "Kindly <hello@kindly.software>")]
#[command(version)]
#[command(about = "AI Gateway with Budget Protection from Kindly", long_about = None)]
#[command(after_help = "\
Examples:
  # Start in test mode (no API keys needed)
  clapi start --test

  # Interactive configuration wizard
  clapi config

  # Start with custom config
  clapi start --config clapi.toml

  # Check system health
  clapi doctor

  # View metrics
  clapi metrics

  # Wizard control (wizard shows by default)
  clapi --no-wizard       # Skip wizard, launch TUI directly
  clapi --wizard          # Force show wizard (even if disabled in config)

  # Toggle wizard permanently: Edit clapi.toml
  show_wizard_on_start = false

Documentation: https://docs.clapi.dev
Support: https://kindly.feedback
")]
pub struct Cli {
    /// Optional subcommand (launches TUI if not provided)
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Force show wizard on startup (override config)
    #[arg(long, global = true)]
    pub wizard: bool,

    /// Skip wizard on startup (override config)
    #[arg(long, global = true, conflicts_with = "wizard")]
    pub no_wizard: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the clapi proxy server
    Start {
        /// Config file path
        #[arg(short, long, default_value = "clapi.toml")]
        config: String,

        /// Enable test mode (mock AI responses, no API keys needed)
        #[arg(long)]
        test: bool,

        /// Server listen address (overrides config)
        #[arg(short, long)]
        listen: Option<String>,

        /// Default budget in cents (overrides config)
        #[arg(short, long)]
        budget: Option<i64>,
    },

    /// Interactive configuration wizard
    Config {
        /// Config file path to create/edit
        #[arg(short, long, default_value = "clapi.toml")]
        output: String,

        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },

    /// Run system diagnostics
    Doctor {
        /// Config file to validate
        #[arg(short, long, default_value = "clapi.toml")]
        config: String,

        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Manage budgets
    Budget {
        #[command(subcommand)]
        action: BudgetAction,
    },

    /// Manage providers
    Providers {
        #[command(subcommand)]
        action: ProviderAction,
    },

    /// View metrics
    Metrics {
        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080/metrics")]
        url: String,

        /// Filter by category (all, budget, circuit_breaker, providers)
        #[arg(short, long, default_value = "all")]
        category: String,

        /// Watch mode (refresh every N seconds)
        #[arg(short, long)]
        watch: Option<u64>,
    },

    /// View audit logs
    Audit {
        /// Config file path
        #[arg(short, long, default_value = "clapi.toml")]
        config: String,

        /// Filter by budget ID
        #[arg(short, long)]
        budget_id: Option<u64>,

        /// Show last N entries
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },

    /// Manage request/response cache
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Performance profiling
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum BudgetAction {
    /// List all budgets
    List {
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show budget details
    Show {
        /// Budget ID
        budget_id: u64,
    },

    /// Add funds to budget
    Add {
        /// Budget ID
        budget_id: u64,

        /// Amount in cents
        #[arg(short, long)]
        amount: i64,
    },

    /// Create new budget
    Create {
        /// Budget ID
        budget_id: u64,

        /// Initial amount in cents
        #[arg(short, long)]
        amount: i64,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProviderAction {
    /// List all providers
    List {
        /// Output format (table, json)
        #[arg(short, long, default_value = "table")]
        format: String,
    },

    /// Show provider status
    Show {
        /// Provider ID
        provider_id: String,
    },

    /// Test provider connectivity
    Test {
        /// Provider ID
        provider_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum CacheAction {
    /// Show cache statistics
    Stats {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080/metrics")]
        url: String,
    },

    /// Clear all cached responses
    Clear {
        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,

        /// Force clear without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Export cache to file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: String,

        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProfileAction {
    /// Start profiling session
    Start {
        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
    },

    /// Stop profiling session
    Stop {
        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080")]
        url: String,
    },

    /// Show latency percentiles
    Report {
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080/metrics")]
        url: String,
    },

    /// Export metrics to Prometheus format
    ExportPrometheus {
        /// Output file path
        #[arg(short, long)]
        output: String,

        /// Metrics endpoint URL
        #[arg(short, long, default_value = "http://localhost:8080/metrics")]
        url: String,
    },
}

impl Cli {
    /// Parse CLI arguments from environment
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli_structure() {
        // Verify the CLI can be constructed
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn test_start_command_defaults() {
        let cli = Cli::parse_from(["clapi", "start"]);
        match cli.command {
            Some(Commands::Start { config, test, .. }) => {
                assert_eq!(config, "clapi.toml");
                assert!(!test);
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_start_with_test_flag() {
        let cli = Cli::parse_from(["clapi", "start", "--test"]);
        match cli.command {
            Some(Commands::Start { test, .. }) => {
                assert!(test);
            }
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_config_command() {
        let cli = Cli::parse_from(["clapi", "config"]);
        match cli.command {
            Some(Commands::Config { output, force }) => {
                assert_eq!(output, "clapi.toml");
                assert!(!force);
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_doctor_command() {
        let cli = Cli::parse_from(["clapi", "doctor"]);
        match cli.command {
            Some(Commands::Doctor { config, format }) => {
                assert_eq!(config, "clapi.toml");
                assert_eq!(format, "text");
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_no_args_defaults_to_tui() {
        // When clapi is called with no arguments, it should launch TUI
        // This test just verifies the command is None
        let cli = Cli::try_parse_from(["clapi"]);
        // clap will fail without subcommand if not optional, so this tests the fix
        assert!(cli.is_ok());
        assert!(cli.unwrap().command.is_none());
    }
}
