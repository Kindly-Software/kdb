//! System Doctor Integration
//!
//! Standalone function for diagnostics to integrate into clapi binary

use clapi_core::cli::{OutputFormat, SystemDoctor};
use colored::Colorize;

/// Run system diagnostics
pub async fn run_diagnostics(
    config_path: String,
    format_str: String,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse output format
    let format = OutputFormat::from_str(&format_str)?;

    // Create system doctor
    let doctor = SystemDoctor::new(config_path).format(format);

    // Run diagnostics
    let report = doctor.run().await?;

    // Print report
    doctor.print_report(&report);

    // Exit with appropriate code
    if report.overall_status == clapi_core::cli::Status::Critical {
        std::process::exit(1);
    }

    Ok(())
}
