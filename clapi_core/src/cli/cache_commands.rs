//! Cache Commands - Request/Response Cache Management
//!
//! # Purpose
//! CLI handlers for managing the request/response cache (Week 3 feature).
//!
//! # UCE34 Framework
//! - Q1-Q9: CLI presentation layer for cache operations
//! - Q10: Tier N/A (no capsules, HTTP calls to cache API)
//! - Q31: Simplicity - clear stats display, single-command operations
//! - Q33: Validation - input validation, error handling
//!
//! # Commands
//! - `clapi cache stats`: Show cache hit rate, memory usage, entry count
//! - `clapi cache clear`: Clear all cached responses
//! - `clapi cache export`: Export cache to JSON file
//!
//! # Performance Targets
//! - HTTP API call: <100ms (local endpoint)
//! - JSON parsing: <10ms
//! - File export: <500ms (10K entries)

use crate::error::{ClapiError, ClapiResult};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

/// Cache statistics (matches API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,

    /// Total cache hits
    pub hits: u64,

    /// Total cache misses
    pub misses: u64,

    /// Current entry count
    pub entry_count: u64,

    /// Memory usage in bytes
    pub memory_bytes: u64,

    /// Average entry size in bytes
    pub avg_entry_size_bytes: u64,

    /// Cache capacity (max entries)
    pub max_entries: u64,

    /// Time-to-live in seconds
    pub ttl_seconds: u64,
}

/// Cache export format (JSON)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheExport {
    /// Export timestamp
    pub timestamp: String,

    /// Cache statistics
    pub stats: CacheStats,

    /// Total entries exported
    pub entry_count: u64,

    /// Entries (truncated in summary)
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<CacheEntry>,
}

/// Single cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Request hash (unique ID)
    pub request_hash: u64,

    /// Response size in bytes
    pub response_size_bytes: u64,

    /// TTL remaining in seconds
    pub ttl_remaining_secs: u64,

    /// Hit count
    pub hit_count: u64,
}

/// Handle cache stats command
///
/// # Arguments
/// - `format`: Output format (text, json)
/// - `url`: Metrics endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on HTTP or parsing error
///
/// # Example Output (text)
/// ```text
/// Cache Statistics
/// ─────────────────────────────────────────────
///   Hit Rate:     87.3% (1,234 hits / 1,414 total)
///   Memory:       42 MB (10,432 entries)
///   Avg Entry:    4.1 KB
///   Capacity:     74.5% (10,432 / 14,000)
///   TTL:          3600 seconds (1 hour)
/// ```
///
/// # Performance
/// - HTTP GET: <100ms (local endpoint)
/// - JSON parsing: <10ms
pub async fn handle_cache_stats(format: &str, url: &str) -> ClapiResult<()> {
    // Fetch cache metrics from endpoint
    let response = reqwest::get(url)
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to fetch cache stats: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    let stats: CacheStats = response
        .json()
        .await
        .map_err(|e| ClapiError::ConfigError(format!("Failed to parse cache stats: {}", e)))?;

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&stats)
                .map_err(|e| ClapiError::ConfigError(format!("JSON serialization failed: {}", e)))?;
            println!("{}", json);
        }
        "text" => {
            print_cache_stats_text(&stats);
        }
        _ => {
            return Err(ClapiError::InvalidRequest {
                reason: format!("Unknown format: {}", format),
            })
        }
    }

    Ok(())
}

/// Handle cache clear command
///
/// # Arguments
/// - `url`: API endpoint URL
/// - `force`: Skip confirmation prompt
///
/// # Returns
/// Ok(()) on success, Err on HTTP error
///
/// # Safety
/// - Asks for confirmation unless force=true
/// - HTTP POST to /cache/clear endpoint
pub async fn handle_cache_clear(url: &str, force: bool) -> ClapiResult<()> {
    // Confirmation prompt (unless force)
    if !force {
        use dialoguer::{theme::ColorfulTheme, Confirm};

        let confirmed = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "{} This will clear all cached responses. Continue?",
                "⚠️".bright_yellow()
            ))
            .default(false)
            .interact()
            .map_err(|e| ClapiError::ConfigError(format!("Failed to confirm: {}", e)))?;

        if !confirmed {
            println!("{}", "Cancelled by user.".bright_black());
            return Ok(());
        }
    }

    // Send clear request
    let clear_url = format!("{}/cache/clear", url);
    let client = reqwest::Client::new();
    let response = client
        .post(&clear_url)
        .send()
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to clear cache: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    println!("{} Cache cleared successfully.", "✅".bright_green());

    Ok(())
}

/// Handle cache export command
///
/// # Arguments
/// - `output`: Output file path (e.g., "cache_export.json")
/// - `url`: API endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on HTTP or file error
///
/// # Performance
/// - HTTP GET: <200ms (10K entries)
/// - JSON serialization: <100ms
/// - File write: <200ms
pub async fn handle_cache_export(output: &str, url: &str) -> ClapiResult<()> {
    // Fetch cache export
    let export_url = format!("{}/cache/export", url);
    let response = reqwest::get(&export_url)
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to export cache: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    let export: CacheExport = response
        .json()
        .await
        .map_err(|e| ClapiError::ConfigError(format!("Failed to parse cache export: {}", e)))?;

    // Write to file
    let json = serde_json::to_string_pretty(&export)
        .map_err(|e| ClapiError::ConfigError(format!("JSON serialization failed: {}", e)))?;

    fs::write(output, json).map_err(|e| {
        ClapiError::ConfigError(format!("Failed to write {}: {}", output, e))
    })?;

    println!(
        "{} Cache exported to {} ({} entries, {} KB)",
        "✅".bright_green(),
        output.bright_cyan(),
        export.entry_count,
        export.stats.memory_bytes / 1024
    );

    Ok(())
}

/// Print cache stats in text format (internal helper)
fn print_cache_stats_text(stats: &CacheStats) {
    println!("\n{}", "Cache Statistics".bright_cyan().bold());
    println!("{}", "─".repeat(45).bright_black());

    // Hit rate
    let hit_rate_pct = stats.hit_rate * 100.0;
    let total_requests = stats.hits + stats.misses;
    let hit_rate_color = if hit_rate_pct >= 80.0 {
        "green"
    } else if hit_rate_pct >= 50.0 {
        "yellow"
    } else {
        "red"
    };

    let hit_rate_text = format!(
        "{:.1}% ({} hits / {} total)",
        hit_rate_pct, stats.hits, total_requests
    );

    let hit_rate_display = match hit_rate_color {
        "green" => hit_rate_text.green(),
        "yellow" => hit_rate_text.yellow(),
        "red" => hit_rate_text.red(),
        _ => hit_rate_text.white(),
    };

    println!("  {}: {}", "Hit Rate".bright_white().bold(), hit_rate_display);

    // Memory usage
    let memory_mb = stats.memory_bytes as f64 / 1_048_576.0;
    println!(
        "  {}: {:.1} MB ({} entries)",
        "Memory".bright_white().bold(),
        memory_mb,
        stats.entry_count
    );

    // Average entry size
    let avg_kb = stats.avg_entry_size_bytes as f64 / 1024.0;
    println!(
        "  {}: {:.1} KB",
        "Avg Entry".bright_white().bold(),
        avg_kb
    );

    // Capacity utilization
    let capacity_pct = (stats.entry_count as f64 / stats.max_entries as f64) * 100.0;
    let capacity_text = format!(
        "{:.1}% ({} / {})",
        capacity_pct, stats.entry_count, stats.max_entries
    );

    let capacity_color = if capacity_pct < 80.0 {
        capacity_text.green()
    } else if capacity_pct < 95.0 {
        capacity_text.yellow()
    } else {
        capacity_text.red()
    };

    println!("  {}: {}", "Capacity".bright_white().bold(), capacity_color);

    // TTL
    let ttl_display = if stats.ttl_seconds >= 3600 {
        format!(
            "{} seconds ({} hours)",
            stats.ttl_seconds,
            stats.ttl_seconds / 3600
        )
    } else if stats.ttl_seconds >= 60 {
        format!(
            "{} seconds ({} minutes)",
            stats.ttl_seconds,
            stats.ttl_seconds / 60
        )
    } else {
        format!("{} seconds", stats.ttl_seconds)
    };

    println!("  {}: {}", "TTL".bright_white().bold(), ttl_display);

    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats_serialization() {
        let stats = CacheStats {
            hit_rate: 0.873,
            hits: 1234,
            misses: 180,
            entry_count: 10_432,
            memory_bytes: 44_040_192, // ~42 MB
            avg_entry_size_bytes: 4_224,
            max_entries: 14_000,
            ttl_seconds: 3600,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("0.873"));
        assert!(json.contains("1234"));

        let parsed: CacheStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.hits, 1234);
        assert_eq!(parsed.misses, 180);
    }

    #[test]
    fn test_cache_stats_text_format() {
        let stats = CacheStats {
            hit_rate: 0.873,
            hits: 1234,
            misses: 180,
            entry_count: 10_432,
            memory_bytes: 44_040_192,
            avg_entry_size_bytes: 4_224,
            max_entries: 14_000,
            ttl_seconds: 3600,
        };

        // Just verify it doesn't panic
        print_cache_stats_text(&stats);
    }

    #[test]
    fn test_cache_export_serialization() {
        let export = CacheExport {
            timestamp: "2025-10-19T12:00:00Z".to_string(),
            stats: CacheStats {
                hit_rate: 0.8,
                hits: 800,
                misses: 200,
                entry_count: 1000,
                memory_bytes: 4_194_304, // 4 MB
                avg_entry_size_bytes: 4096,
                max_entries: 10_000,
                ttl_seconds: 3600,
            },
            entry_count: 1000,
            entries: vec![],
        };

        let json = serde_json::to_string_pretty(&export).unwrap();
        assert!(json.contains("2025-10-19"));
        assert!(json.contains("800"));
    }
}
