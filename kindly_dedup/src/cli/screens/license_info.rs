//! [TRADE SECRET] License information screen for CLI
//!
//! Displays:
//! - Current license tier
//! - License status (Valid, Expired, Revoked, Trial days remaining)
//! - Usage statistics (GB used, remaining)
//! - Feature comparison table
//! - License management options
//!
//! ## Layout
//!
//! ```
//! ┌─────────────────────────────────────────────────┐
//! │         LICENSE INFORMATION                    │
//! │                                                 │
//! │  Current Tier: Pro                             │
//! │  Status: ✓ Valid                               │
//! │  Duration: 1 year                              │
//! │  GB Used: 42 / ∞ (Unlimited)                   │
//! │  Expires: 2024-11-10                           │
//! │                                                 │
//! │  ENABLED FEATURES                              │
//! │  ✓ Multi-threaded                              │
//! │  ✓ Audit Trail                                 │
//! │  ✓ SIMD MinHash                                │
//! │  ✓ Persistent Mode                             │
//! │                                                 │
//! │  FEATURE COMPARISON                            │
//! │  Feature       | Trial | Starter | Pro | Ent. │
//! │  ───────────────────────────────────────────── │
//! │  Multi-thread  │  ✓    │   ✓     │ ✓   │ ✓   │
//! │  ...                                            │
//! │                                                 │
//! └─────────────────────────────────────────────────┘
//! ```

use crate::license::{FeatureMatrix, LicenseFeature, LicenseManager};
use crate::license_capsule::LicenseStatus;
use crate::utils::terminal::{emoji, Colorize};
use std::io::{self, Write};

/// Render license information screen
pub fn render_license_info_screen(license: &LicenseManager) -> io::Result<()> {
    let tier = license.tier();
    let status = license.status();
    let config = license.config();

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    );
    println!();
    println!(
        "{}",
        "                         LICENSE INFORMATION".deep_purple().bold()
    );
    println!();

    // License tier and status
    println!(
        "  {} Current Tier: {}",
        "💼".byzantine_gold(),
        format!("{:?}", tier).bright_gold().bold()
    );

    // Status indicator
    let status_text = match status {
        LicenseStatus::Valid => "✓ Valid".bright_green(),
        LicenseStatus::Expired => "✗ Expired".bright_red(),
        LicenseStatus::Revoked => "✗ Revoked".bright_red(),
    };
    println!("  {} Status: {}", "📋".byzantine_gold(), status_text);

    // Duration
    println!(
        "  {} Duration: {}",
        "⏰".byzantine_gold(),
        config.duration_display().light_purple()
    );

    // Data usage
    if let Some(remaining) = license.remaining_gb() {
        println!("  {} Data Used: {} GB remaining", "📊".byzantine_gold(), remaining);
    } else {
        println!(
            "  {} Data Used: {} (Unlimited)",
            "📊".byzantine_gold(),
            "∞".bright_gold()
        );
    }

    // Expiration date (if available)
    let expiry_ts = license.capsule.expiry();
    if expiry_ts > 0 {
        if let Ok(date) = format_timestamp(expiry_ts) {
            println!("  {} Expires: {}", "📅".byzantine_gold(), date.light_purple());
        }
    }

    println!();
    println!("  {}", "ENABLED FEATURES".deep_purple().bold());
    println!();

    // List enabled features
    let features = license.features();
    if features.is_empty() {
        println!(
            "    {} (Free tier - basic deduplication only)",
            "No advanced features".light_purple()
        );
    } else {
        for feature in &features {
            println!("    {} {}", "✓".bright_green(), feature.to_string().light_purple());
        }
    }

    println!();
    println!("  {}", "FEATURE COMPARISON".deep_purple().bold());
    println!();

    // Feature comparison table
    render_feature_comparison_table()?;

    println!();

    // Actions based on tier
    render_license_actions()?;

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    );
    println!();

    io::stdout().flush()?;
    Ok(())
}

/// Render feature comparison table
fn render_feature_comparison_table() -> io::Result<()> {
    let matrix = FeatureMatrix::build_table();

    // Header
    println!(
        "    {:<20} | {:<8} | {:<10} | {:<8} | {:<6}",
        "Feature", "Trial", "Starter", "Pro", "Ent."
    );
    println!("    {}", "─".repeat(70));

    // Rows
    for (feature, trial, starter, pro, enterprise) in matrix {
        let trial_mark = if trial { "✓" } else { "✗" };
        let starter_mark = if starter { "✓" } else { "✗" };
        let pro_mark = if pro { "✓" } else { "✗" };
        let ent_mark = if enterprise { "✓" } else { "✗" };

        println!(
            "    {:<20} | {:<8} | {:<10} | {:<8} | {:<6}",
            feature,
            trial_mark.bright_green(),
            starter_mark.bright_green(),
            pro_mark.bright_green(),
            ent_mark.bright_green()
        );
    }

    Ok(())
}

/// Render license management actions
fn render_license_actions() -> io::Result<()> {
    println!("  {} LICENSE MANAGEMENT", "⚙️ ".byzantine_gold());
    println!();
    println!("    {} Activate new license  : kindly-dedup --license-key <KEY>");
    println!("    {} View usage statistics : kindly-dedup stats");
    println!("    {} Export license info   : kindly-dedup export-license");
    println!(
        "    {} Upgrade tier          : Visit {} for more features",
        emoji("🚀"),
        "https://kindly.ai/plans".bright_gold()
    );
    println!();

    Ok(())
}

/// Format Unix timestamp as readable date
fn format_timestamp(secs: u64) -> Result<String, Box<dyn std::error::Error>> {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    let datetime = UNIX_EPOCH + Duration::from_secs(secs);
    let duration = datetime.duration_since(UNIX_EPOCH)?;

    // Simple date formatting (YYYY-MM-DD)
    // In production, use chrono crate for proper formatting
    let days_since_epoch = duration.as_secs() / 86400;
    let years = days_since_epoch / 365;
    let remaining_days = days_since_epoch % 365;
    let month = (remaining_days / 30) + 1;
    let day = (remaining_days % 30) + 1;

    Ok(format!("20{:02}-{:02}-{:02}", years - 50, month, day))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp() {
        // Unix epoch: 1970-01-01
        let result = format_timestamp(0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_feature_comparison_table() {
        let result = render_feature_comparison_table();
        assert!(result.is_ok());
    }
}
