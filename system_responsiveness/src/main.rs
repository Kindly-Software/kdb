/// System Responsiveness Daemon (sysrespond)
/// Computational capsule-based process monitoring and resource governance
///
/// Architecture: T6 Mixed Capsule
/// - T1 (Atomic): ProcessStateCapsule, ResourceGovernorCapsule
/// - T4 (Batch): Parallel process scanning
/// - T5 (Streaming): Continuous monitoring loop
///
/// Performance Targets:
/// - Detection latency: <1s
/// - Action latency: <2s
/// - False positive rate: <0.1%
/// - System overhead: <5% CPU, <50MB RAM

mod capsules;

use capsules::{ResourceGovernorCapsule, StreamingMonitorCapsule};
use capsules::streaming_monitor::MonitorConfig;
use std::sync::Arc;
use std::path::PathBuf;
use tracing::{info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Configuration file structure matching config.toml
#[derive(Debug, serde::Deserialize)]
struct ConfigFile {
    thresholds: Thresholds,
    circuit_breaker: CircuitBreaker,
    test_patterns: Patterns,
    whitelist_patterns: Patterns,
}

#[derive(Debug, serde::Deserialize)]
struct Thresholds {
    cpu_threshold_pct: f64,
    runtime_threshold_sec: u64,
    scan_interval_sec: u64,
    sigkill_grace_sec: u64,
}

#[derive(Debug, serde::Deserialize)]
struct CircuitBreaker {
    kill_threshold: u8,
    cooldown_sec: u64,
}

#[derive(Debug, serde::Deserialize)]
struct Patterns {
    patterns: Vec<String>,
}

/// Load configuration from ~/.config/sysrespond/config.toml
/// Returns (MonitorConfig, circuit_breaker settings)
fn load_config() -> anyhow::Result<(MonitorConfig, u8, u64)> {
    let mut config_path = PathBuf::from(std::env::var("HOME")?);
    config_path.push(".config/sysrespond/config.toml");

    let content = std::fs::read_to_string(&config_path)?;
    let file: ConfigFile = toml::from_str(&content)?;

    let monitor_config = MonitorConfig {
        cpu_threshold_pct: file.thresholds.cpu_threshold_pct,
        runtime_threshold_sec: file.thresholds.runtime_threshold_sec,
        scan_interval_sec: file.thresholds.scan_interval_sec,
        sigkill_grace_sec: file.thresholds.sigkill_grace_sec,
        test_patterns: file.test_patterns.patterns,
        whitelist_patterns: file.whitelist_patterns.patterns,
    };

    Ok((monitor_config, file.circuit_breaker.kill_threshold, file.circuit_breaker.cooldown_sec))
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    info!("🚀 System Responsiveness Daemon v0.1.0");
    info!("📊 Computational Capsule Architecture: T6 (Mixed)");
    info!("   - T1 (Atomic): Process state tracking, resource limits");
    info!("   - T4 (Batch): Parallel process scanning");
    info!("   - T5 (Streaming): Continuous monitoring");

    // Load configuration from ~/.config/sysrespond/config.toml
    let (config, kill_threshold, cooldown_sec) = load_config().unwrap_or_else(|e| {
        warn!("Failed to load config: {}. Using defaults.", e);
        (MonitorConfig::default(), 5, 60)
    });

    // Create resource governor capsule (T1 Atomic)
    // Parameters: cpu_limit_pct, mem_limit_mb, kill_threshold, cooldown_sec
    let governor = Arc::new(ResourceGovernorCapsule::new(
        config.cpu_threshold_pct,
        4096,  // 4GB memory limit
        kill_threshold,
        cooldown_sec as u16,  // Convert u64 to u16
    ));

    info!("⚡ Resource Governor initialized:");
    info!("   CPU limit: {:.1}%", governor.cpu_limit_pct());
    info!("   Circuit breaker: {:?}", governor.circuit_state());
    info!("   Kill threshold: {}/minute", kill_threshold);

    // Create streaming monitor capsule (T5 Streaming)
    let monitor = StreamingMonitorCapsule::new(config.clone(), Arc::clone(&governor));

    info!("🔍 Monitor configuration:");
    info!("   CPU threshold: {:.1}%", config.cpu_threshold_pct);
    info!("   Runtime threshold: {}s", config.runtime_threshold_sec);
    info!("   Scan interval: {}s", config.scan_interval_sec);
    info!("   SIGKILL grace period: {}s", config.sigkill_grace_sec);

    info!("✅ Daemon started successfully");
    info!("📝 Monitoring {} test patterns, {} whitelist patterns",
        config.test_patterns.len(),
        config.whitelist_patterns.len()
    );

    // Start monitoring loop
    monitor.monitor_loop().await;
}
