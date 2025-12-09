//! Profile Commands - Performance Profiling
//!
//! # Purpose
//! CLI handlers for performance profiling (Week 3 feature).
//!
//! # UCE34 Framework
//! - Q1-Q9: CLI presentation layer for profiling operations
//! - Q10: Tier N/A (no capsules, HTTP calls to profiling API)
//! - Q31: Simplicity - clear latency reports, Prometheus export
//! - Q33: Validation - input validation, error handling
//!
//! # Commands
//! - `clapi profile start`: Start profiling session
//! - `clapi profile stop`: Stop profiling session
//! - `clapi profile report`: Show latency percentiles (p50, p99, p999)
//! - `clapi profile export-prometheus`: Export metrics to Prometheus format
//!
//! # Performance Targets
//! - HTTP API call: <100ms (local endpoint)
//! - JSON parsing: <10ms
//! - Prometheus export: <200ms

use crate::error::{ClapiError, ClapiResult};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use std::fs;

/// Profiling statistics (matches API response)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingStats {
    /// Is profiling active?
    pub active: bool,

    /// Total samples collected
    pub sample_count: u64,

    /// Latency percentiles (in milliseconds)
    pub p50_ms: f64,
    pub p90_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub p999_ms: f64,

    /// Mean latency (in milliseconds)
    pub mean_ms: f64,

    /// Min/max latency (in milliseconds)
    pub min_ms: f64,
    pub max_ms: f64,

    /// Standard deviation (in milliseconds)
    pub stddev_ms: f64,

    /// Throughput (requests per second)
    pub throughput_rps: f64,
}

/// Prometheus export format
#[derive(Debug, Clone)]
pub struct PrometheusExport {
    /// Metrics in Prometheus text format
    pub metrics: String,
}

/// Handle profile start command
///
/// # Arguments
/// - `url`: API endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on HTTP error
///
/// # Example Output
/// ```text
/// ✅ Profiling started successfully.
///    Use `clapi profile report` to view latency stats.
/// ```
pub async fn handle_profile_start(url: &str) -> ClapiResult<()> {
    let start_url = format!("{}/profile/start", url);
    let client = reqwest::Client::new();
    let response = client
        .post(&start_url)
        .send()
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to start profiling: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    println!("{} Profiling started successfully.", "✅".bright_green());
    println!(
        "   Use {} to view latency stats.",
        "clapi profile report".bright_cyan()
    );

    Ok(())
}

/// Handle profile stop command
///
/// # Arguments
/// - `url`: API endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on HTTP error
///
/// # Example Output
/// ```text
/// ✅ Profiling stopped.
///    12,345 samples collected.
/// ```
pub async fn handle_profile_stop(url: &str) -> ClapiResult<()> {
    let stop_url = format!("{}/profile/stop", url);
    let client = reqwest::Client::new();
    let response = client
        .post(&stop_url)
        .send()
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to stop profiling: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    // Parse response to get sample count
    #[derive(Deserialize)]
    struct StopResponse {
        sample_count: u64,
    }

    let stop_data: StopResponse = response
        .json()
        .await
        .map_err(|e| ClapiError::ConfigError(format!("Failed to parse stop response: {}", e)))?;

    println!("{} Profiling stopped.", "✅".bright_green());
    println!(
        "   {} samples collected.",
        stop_data.sample_count.to_string().bright_white()
    );

    Ok(())
}

/// Handle profile report command
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
/// Performance Report
/// ─────────────────────────────────────────────
///   Status:      ✅ ACTIVE (12,345 samples)
///   Throughput:  847 req/s
///
/// Latency Percentiles (milliseconds)
/// ─────────────────────────────────────────────
///   P50:    123.4 ms
///   P90:    234.7 ms
///   P95:    298.2 ms
///   P99:    456.8 ms
///   P999:   892.3 ms
///
/// Statistics
/// ─────────────────────────────────────────────
///   Mean:     145.6 ms
///   Min:       45.2 ms
///   Max:     1,234.5 ms
///   StdDev:    98.7 ms
/// ```
///
/// # Performance
/// - HTTP GET: <100ms (local endpoint)
/// - JSON parsing: <10ms
pub async fn handle_profile_report(format: &str, url: &str) -> ClapiResult<()> {
    // Fetch profiling stats from endpoint
    let response = reqwest::get(url)
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to fetch profiling stats: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    let stats: ProfilingStats = response
        .json()
        .await
        .map_err(|e| ClapiError::ConfigError(format!("Failed to parse profiling stats: {}", e)))?;

    match format {
        "json" => {
            let json = serde_json::to_string_pretty(&stats)
                .map_err(|e| ClapiError::ConfigError(format!("JSON serialization failed: {}", e)))?;
            println!("{}", json);
        }
        "text" => {
            print_profiling_stats_text(&stats);
        }
        _ => {
            return Err(ClapiError::InvalidRequest {
                reason: format!("Unknown format: {}", format),
            })
        }
    }

    Ok(())
}

/// Handle profile export Prometheus command
///
/// # Arguments
/// - `output`: Output file path (e.g., "metrics.prom")
/// - `url`: Metrics endpoint URL
///
/// # Returns
/// Ok(()) on success, Err on HTTP or file error
///
/// # Prometheus Format
/// ```text
/// # HELP clapi_request_latency_seconds Request latency in seconds
/// # TYPE clapi_request_latency_seconds summary
/// clapi_request_latency_seconds{quantile="0.5"} 0.123
/// clapi_request_latency_seconds{quantile="0.9"} 0.234
/// clapi_request_latency_seconds{quantile="0.95"} 0.298
/// clapi_request_latency_seconds{quantile="0.99"} 0.456
/// clapi_request_latency_seconds{quantile="0.999"} 0.892
/// clapi_request_latency_seconds_sum 1794.5
/// clapi_request_latency_seconds_count 12345
/// ```
pub async fn handle_profile_export_prometheus(output: &str, url: &str) -> ClapiResult<()> {
    // Fetch profiling stats
    let response = reqwest::get(url)
        .await
        .map_err(|e| ClapiError::ProviderError(format!("Failed to fetch profiling stats: {}", e)))?;

    if !response.status().is_success() {
        return Err(ClapiError::ProviderError(format!(
            "HTTP error: {}",
            response.status()
        )));
    }

    let stats: ProfilingStats = response
        .json()
        .await
        .map_err(|e| ClapiError::ConfigError(format!("Failed to parse profiling stats: {}", e)))?;

    // Convert to Prometheus format
    let prometheus_text = format_prometheus(&stats);

    // Write to file
    fs::write(output, prometheus_text).map_err(|e| {
        ClapiError::ConfigError(format!("Failed to write {}: {}", output, e))
    })?;

    println!(
        "{} Metrics exported to {} (Prometheus format)",
        "✅".bright_green(),
        output.bright_cyan()
    );

    Ok(())
}

/// Print profiling stats in text format (internal helper)
fn print_profiling_stats_text(stats: &ProfilingStats) {
    println!("\n{}", "Performance Report".bright_cyan().bold());
    println!("{}", "─".repeat(45).bright_black());

    // Status
    let status_icon = if stats.active {
        "✅ ACTIVE".green()
    } else {
        "⏸️  STOPPED".yellow()
    };

    println!(
        "  {}: {} ({} samples)",
        "Status".bright_white().bold(),
        status_icon,
        stats.sample_count
    );

    // Throughput
    println!(
        "  {}: {:.0} req/s",
        "Throughput".bright_white().bold(),
        stats.throughput_rps
    );

    println!();

    // Latency percentiles
    println!("{}", "Latency Percentiles (milliseconds)".bright_cyan().bold());
    println!("{}", "─".repeat(45).bright_black());

    println!(
        "  {}: {:.1} ms",
        "P50".bright_white().bold(),
        stats.p50_ms
    );
    println!(
        "  {}: {:.1} ms",
        "P90".bright_white().bold(),
        stats.p90_ms
    );
    println!(
        "  {}: {:.1} ms",
        "P95".bright_white().bold(),
        stats.p95_ms
    );
    println!(
        "  {}: {:.1} ms",
        "P99".bright_white().bold(),
        stats.p99_ms
    );

    // Highlight P999 if very high
    let p999_text = format!("{:.1} ms", stats.p999_ms);
    let p999_display = if stats.p999_ms > stats.p99_ms * 2.0 {
        p999_text.red()
    } else {
        p999_text.white()
    };

    println!("  {}: {}", "P999".bright_white().bold(), p999_display);

    println!();

    // Statistics
    println!("{}", "Statistics".bright_cyan().bold());
    println!("{}", "─".repeat(45).bright_black());

    println!(
        "  {}: {:.1} ms",
        "Mean".bright_white().bold(),
        stats.mean_ms
    );
    println!(
        "  {}: {:.1} ms",
        "Min".bright_white().bold(),
        stats.min_ms
    );
    println!(
        "  {}: {:.1} ms",
        "Max".bright_white().bold(),
        stats.max_ms
    );
    println!(
        "  {}: {:.1} ms",
        "StdDev".bright_white().bold(),
        stats.stddev_ms
    );

    println!();
}

/// Format profiling stats as Prometheus text format (internal helper)
fn format_prometheus(stats: &ProfilingStats) -> String {
    let mut output = String::new();

    // Latency summary
    output.push_str("# HELP clapi_request_latency_seconds Request latency in seconds\n");
    output.push_str("# TYPE clapi_request_latency_seconds summary\n");
    output.push_str(&format!(
        "clapi_request_latency_seconds{{quantile=\"0.5\"}} {:.6}\n",
        stats.p50_ms / 1000.0
    ));
    output.push_str(&format!(
        "clapi_request_latency_seconds{{quantile=\"0.9\"}} {:.6}\n",
        stats.p90_ms / 1000.0
    ));
    output.push_str(&format!(
        "clapi_request_latency_seconds{{quantile=\"0.95\"}} {:.6}\n",
        stats.p95_ms / 1000.0
    ));
    output.push_str(&format!(
        "clapi_request_latency_seconds{{quantile=\"0.99\"}} {:.6}\n",
        stats.p99_ms / 1000.0
    ));
    output.push_str(&format!(
        "clapi_request_latency_seconds{{quantile=\"0.999\"}} {:.6}\n",
        stats.p999_ms / 1000.0
    ));

    // Sum and count
    let sum_seconds = (stats.mean_ms * stats.sample_count as f64) / 1000.0;
    output.push_str(&format!(
        "clapi_request_latency_seconds_sum {:.6}\n",
        sum_seconds
    ));
    output.push_str(&format!(
        "clapi_request_latency_seconds_count {}\n",
        stats.sample_count
    ));

    // Throughput gauge
    output.push_str("\n# HELP clapi_request_throughput_rps Current request throughput\n");
    output.push_str("# TYPE clapi_request_throughput_rps gauge\n");
    output.push_str(&format!(
        "clapi_request_throughput_rps {:.2}\n",
        stats.throughput_rps
    ));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiling_stats_serialization() {
        let stats = ProfilingStats {
            active: true,
            sample_count: 12_345,
            p50_ms: 123.4,
            p90_ms: 234.7,
            p95_ms: 298.2,
            p99_ms: 456.8,
            p999_ms: 892.3,
            mean_ms: 145.6,
            min_ms: 45.2,
            max_ms: 1234.5,
            stddev_ms: 98.7,
            throughput_rps: 847.0,
        };

        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("12345"));
        assert!(json.contains("123.4"));

        let parsed: ProfilingStats = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.sample_count, 12_345);
        assert_eq!(parsed.p50_ms, 123.4);
    }

    #[test]
    fn test_profiling_stats_text_format() {
        let stats = ProfilingStats {
            active: true,
            sample_count: 12_345,
            p50_ms: 123.4,
            p90_ms: 234.7,
            p95_ms: 298.2,
            p99_ms: 456.8,
            p999_ms: 892.3,
            mean_ms: 145.6,
            min_ms: 45.2,
            max_ms: 1234.5,
            stddev_ms: 98.7,
            throughput_rps: 847.0,
        };

        // Just verify it doesn't panic
        print_profiling_stats_text(&stats);
    }

    #[test]
    fn test_prometheus_export_format() {
        let stats = ProfilingStats {
            active: true,
            sample_count: 1000,
            p50_ms: 100.0,
            p90_ms: 200.0,
            p95_ms: 250.0,
            p99_ms: 400.0,
            p999_ms: 800.0,
            mean_ms: 150.0,
            min_ms: 50.0,
            max_ms: 1000.0,
            stddev_ms: 100.0,
            throughput_rps: 500.0,
        };

        let prometheus = format_prometheus(&stats);

        // Verify Prometheus format
        assert!(prometheus.contains("# HELP clapi_request_latency_seconds"));
        assert!(prometheus.contains("# TYPE clapi_request_latency_seconds summary"));
        assert!(prometheus.contains("quantile=\"0.5\""));
        assert!(prometheus.contains("quantile=\"0.99\""));
        assert!(prometheus.contains("clapi_request_latency_seconds_count 1000"));
        assert!(prometheus.contains("clapi_request_throughput_rps"));
    }

    #[test]
    fn test_prometheus_latency_conversion() {
        let stats = ProfilingStats {
            active: true,
            sample_count: 100,
            p50_ms: 123.456,
            p90_ms: 200.0,
            p95_ms: 250.0,
            p99_ms: 400.0,
            p999_ms: 800.0,
            mean_ms: 150.0,
            min_ms: 50.0,
            max_ms: 1000.0,
            stddev_ms: 100.0,
            throughput_rps: 100.0,
        };

        let prometheus = format_prometheus(&stats);

        // P50 should be 0.123456 seconds (123.456 ms)
        assert!(prometheus.contains("0.123456"));
    }
}
