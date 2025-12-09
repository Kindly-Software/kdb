//! Metrics Dashboard - Real-Time Terminal UI for clapi Metrics
//!
//! # Purpose
//! Provides an interactive, color-coded terminal dashboard for monitoring clapi metrics:
//! - Real-time metric polling from HTTP endpoints
//! - Responsive table layouts with tabled crate
//! - Color-coded status indicators (✅ ⚠️ ❌)
//! - Live updates with configurable refresh intervals
//! - Interactive controls (q=quit, p=pause, r=resume)
//!
//! # UCE34 Framework
//! - Q1-Q9: Terminal UI for metrics display (read-only data presentation)
//! - Q10: Tier N/A (no capsules, uses existing HTTP metrics)
//! - Q11-Q28: Terminal rendering, HTTP polling, event handling
//! - Q31: Simplicity - clear table layout, minimal dependencies
//! - Q33: Validation - compile-time type checking for metrics schema
//! - Q34: N/A (read-only, no state modification)
//!
//! # Design Principles
//! - Progressive Disclosure: Simple snapshot view, detailed tables
//! - Visual Feedback: Color-coded status, trend indicators
//! - Responsive Layout: Terminal width-aware table formatting
//! - Graceful Degradation: Handle network errors, missing metrics
//!
//! # Performance Targets
//! - HTTP polling: <50ms (local endpoint)
//! - Terminal rendering: <100ms (full screen refresh)
//! - Memory usage: <10MB (stateless polling)

use colored::Colorize;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, ClearType},
};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::time::{Duration, SystemTime};
use tabled::{
    settings::Style,
    Table, Tabled,
};

/// Metrics dashboard with real-time polling
///
/// # Example
/// ```ignore
/// use clapi_core::cli::dashboard::MetricsDashboard;
///
/// let dashboard = MetricsDashboard::new("http://localhost:8080/metrics", 5);
/// dashboard.run(60).await?; // Watch for 60 seconds
/// ```
pub struct MetricsDashboard {
    /// Metrics endpoint URL
    url: String,

    /// Refresh interval in seconds
    refresh_interval: Duration,

    /// Paused state
    paused: bool,
}

/// Dashboard frame (single snapshot of all metrics)
#[derive(Debug, Clone)]
pub struct DashboardFrame {
    /// Timestamp of this snapshot (for future trend analysis)
    #[allow(dead_code)]
    timestamp: SystemTime,

    /// Budget metrics
    budget_metrics: Vec<BudgetMetric>,

    /// Provider metrics
    provider_metrics: Vec<ProviderMetric>,

    /// System metrics
    system_metrics: SystemMetrics,

    /// Cache metrics (Week 3)
    cache_metrics: Option<CacheMetricsPanel>,

    /// Compression metrics (Week 3)
    compression_metrics: Option<CompressionMetricsPanel>,

    /// Load balancer metrics (Week 3)
    load_balancer_metrics: Option<LoadBalancerMetricsPanel>,

    /// Performance metrics (Week 3)
    performance_metrics: Option<PerformanceMetricsPanel>,
}

/// Budget metric (single budget snapshot)
#[derive(Debug, Clone, Tabled)]
pub struct BudgetMetric {
    #[tabled(rename = "Budget ID")]
    pub budget_id: String,

    #[tabled(rename = "Available")]
    pub available: String,

    #[tabled(rename = "Spent")]
    pub spent: String,

    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Trend")]
    pub trend: String,
}

/// Provider metric (single provider snapshot)
#[derive(Debug, Clone, Tabled)]
pub struct ProviderMetric {
    #[tabled(rename = "Provider")]
    pub provider: String,

    #[tabled(rename = "Status")]
    pub status: String,

    #[tabled(rename = "Failures")]
    pub failures: String,

    #[tabled(rename = "Latency")]
    pub latency: String,

    #[tabled(rename = "Response Rate")]
    pub response_rate: String,
}

/// System metrics (global stats)
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub uptime: String,
    pub total_requests: u64,
    pub avg_latency_ms: u64,
    pub memory_mb: u64,
    pub uptime_secs: u64,  // Added for TUI atomic updates
}

/// Cache metrics panel (Week 3)
#[derive(Debug, Clone)]
pub struct CacheMetricsPanel {
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,
    /// Memory usage in MB
    pub memory_mb: f64,
    /// Entry count
    pub entry_count: u64,
    /// Eviction rate
    pub eviction_rate: f64,
}

/// Compression metrics panel (Week 3)
#[derive(Debug, Clone)]
pub struct CompressionMetricsPanel {
    /// Compression ratio (original / compressed)
    pub compression_ratio: f64,
    /// Throughput in MB/s
    pub throughput_mbps: f64,
    /// Bandwidth saved in MB
    pub bandwidth_saved_mb: u64,
}

/// Load balancer metrics panel (Week 3)
#[derive(Debug, Clone)]
pub struct LoadBalancerMetricsPanel {
    /// Cost per 1K tokens (in cents)
    pub cost_per_1k_tokens: f64,
    /// Provider latencies (P99) in milliseconds
    pub provider_latencies: Vec<(String, f64)>,
    /// Failover rate (0.0 - 1.0)
    pub failover_rate: f64,
}

/// Performance metrics panel (Week 3)
#[derive(Debug, Clone)]
pub struct PerformanceMetricsPanel {
    /// P50 latency in milliseconds
    pub p50_ms: f64,
    /// P99 latency in milliseconds
    pub p99_ms: f64,
    /// P999 latency in milliseconds
    pub p999_ms: f64,
}

/// Metrics API response (matches /metrics endpoint schema)
#[derive(Debug, Deserialize, Serialize)]
struct MetricsResponse {
    #[serde(default)]
    budgets: Vec<BudgetMetricRaw>,

    #[serde(default)]
    providers: Vec<ProviderMetricRaw>,

    #[serde(default)]
    system: SystemMetricsRaw,

    #[serde(default)]
    cache: Option<CacheMetricsRaw>,

    #[serde(default)]
    compression: Option<CompressionMetricsRaw>,

    #[serde(default)]
    load_balancer: Option<LoadBalancerMetricsRaw>,

    #[serde(default)]
    performance: Option<PerformanceMetricsRaw>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct BudgetMetricRaw {
    budget_id: u64,
    available_cents: i64,
    spent_cents: i64,
    utilization_bp: u32, // Basis points (0-10000)
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct ProviderMetricRaw {
    provider_name: String,
    circuit_state: u8, // 0=Closed, 1=HalfOpen, 2=Open
    failures_count: u64,
    latency_p99_ns: u64,
    success_rate_bp: u32, // Basis points (0-10000)
    total_requests: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct SystemMetricsRaw {
    uptime_secs: u64,
    total_requests: u64,
    avg_latency_ns: u64,
    memory_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct CacheMetricsRaw {
    hit_rate: f64,
    hits: u64,
    misses: u64,
    memory_bytes: u64,
    entry_count: u64,
    eviction_rate: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct CompressionMetricsRaw {
    compression_ratio: f64,
    throughput_bytes_per_sec: u64,
    bandwidth_saved_bytes: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct LoadBalancerMetricsRaw {
    cost_per_1k_tokens_cents: f64,
    provider_latencies_ms: Vec<(String, f64)>,
    failover_rate: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct PerformanceMetricsRaw {
    p50_ms: f64,
    p99_ms: f64,
    p999_ms: f64,
}

impl MetricsDashboard {
    /// Create new metrics dashboard
    ///
    /// # Arguments
    /// - `url`: Metrics endpoint URL (e.g., "http://localhost:8080/metrics")
    /// - `refresh_interval_secs`: Refresh interval in seconds
    ///
    /// # Examples
    /// ```
    /// use clapi_core::cli::dashboard::MetricsDashboard;
    ///
    /// let dashboard = MetricsDashboard::new("http://localhost:8080/metrics", 5);
    /// ```
    pub fn new(url: String, refresh_interval_secs: u64) -> Self {
        Self {
            url,
            refresh_interval: Duration::from_secs(refresh_interval_secs),
            paused: false,
        }
    }

    /// Run dashboard in watch mode
    ///
    /// # Arguments
    /// - `watch_seconds`: Total watch duration (0 = infinite)
    ///
    /// # Performance
    /// - Polling overhead: <50ms per refresh (local HTTP)
    /// - Terminal rendering: <100ms (full screen)
    ///
    /// # Controls
    /// - `q`: Quit dashboard
    /// - `p`: Pause updates
    /// - `r`: Resume updates
    /// - `Ctrl+C`: Graceful shutdown
    ///
    /// # Examples
    /// ```ignore
    /// use clapi_core::cli::dashboard::MetricsDashboard;
    ///
    /// let dashboard = MetricsDashboard::new("http://localhost:8080/metrics", 5);
    /// dashboard.run(60).await?; // Watch for 60 seconds
    /// ```
    pub async fn run(&mut self, watch_seconds: u64) -> Result<(), String> {
        // Enable raw mode for terminal
        terminal::enable_raw_mode().map_err(|e| format!("Failed to enable raw mode: {}", e))?;

        let mut stdout = io::stdout();

        // Clear screen
        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .map_err(|e| format!("Failed to clear screen: {}", e))?;

        let start_time = SystemTime::now();
        let mut iteration = 0;

        loop {
            // Check for keyboard input (non-blocking)
            if event::poll(Duration::from_millis(100))
                .map_err(|e| format!("Failed to poll events: {}", e))?
            {
                if let Event::Key(key_event) = event::read()
                    .map_err(|e| format!("Failed to read event: {}", e))?
                {
                    match key_event.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        KeyCode::Char('p') | KeyCode::Char('P') => {
                            self.paused = true;
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            self.paused = false;
                        }
                        KeyCode::Char('c')
                            if key_event.modifiers == event::KeyModifiers::CONTROL =>
                        {
                            break
                        }
                        _ => {}
                    }
                }
            }

            // Skip rendering if paused
            if self.paused {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            // Check watch duration
            if watch_seconds > 0 {
                let elapsed = start_time
                    .elapsed()
                    .map_err(|e| format!("Time error: {}", e))?;
                if elapsed.as_secs() >= watch_seconds {
                    break;
                }
            }

            // Fetch metrics
            let frame = match self.fetch_metrics().await {
                Ok(f) => f,
                Err(e) => {
                    // Show error and retry
                    self.render_error(&e)?;
                    tokio::time::sleep(self.refresh_interval).await;
                    continue;
                }
            };

            // Render dashboard
            self.render_frame(&frame, iteration)?;

            // Wait for next refresh
            tokio::time::sleep(self.refresh_interval).await;
            iteration += 1;
        }

        // Restore terminal
        terminal::disable_raw_mode().map_err(|e| format!("Failed to disable raw mode: {}", e))?;

        // Clear screen and show goodbye message
        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .map_err(|e| format!("Failed to clear screen: {}", e))?;

        println!("{}", "Dashboard stopped.".bright_green());

        Ok(())
    }

    /// Fetch metrics from HTTP endpoint
    ///
    /// # Performance
    /// - HTTP GET: <50ms (local endpoint)
    /// - JSON parsing: <10ms (serde_json)
    ///
    /// # Safety
    /// - #ASSUME: HTTP endpoint returns valid JSON
    /// - #VERIFY: Unit tests validate response parsing
    async fn fetch_metrics(&self) -> Result<DashboardFrame, String> {
        let response = reqwest::get(&self.url)
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("HTTP error: {}", response.status()));
        }

        let metrics: MetricsResponse = response
            .json()
            .await
            .map_err(|e| format!("JSON parsing failed: {}", e))?;

        Ok(self.convert_response(metrics))
    }

    /// Convert API response to dashboard frame
    ///
    /// # Performance
    /// - O(n) where n = number of budgets + providers
    /// - <1ms for typical workloads (<100 budgets)
    fn convert_response(&self, raw: MetricsResponse) -> DashboardFrame {
        DashboardFrame {
            timestamp: SystemTime::now(),
            budget_metrics: raw
                .budgets
                .into_iter()
                .map(|b| self.convert_budget_metric(b))
                .collect(),
            provider_metrics: raw
                .providers
                .into_iter()
                .map(|p| self.convert_provider_metric(p))
                .collect(),
            system_metrics: self.convert_system_metrics(raw.system),
            cache_metrics: raw.cache.map(|c| self.convert_cache_metrics(c)),
            compression_metrics: raw.compression.map(|c| self.convert_compression_metrics(c)),
            load_balancer_metrics: raw.load_balancer.map(|lb| self.convert_load_balancer_metrics(lb)),
            performance_metrics: raw.performance.map(|p| self.convert_performance_metrics(p)),
        }
    }

    /// Convert budget metric (raw → display)
    fn convert_budget_metric(&self, raw: BudgetMetricRaw) -> BudgetMetric {
        let budget_id = format!("budget_{}", raw.budget_id);
        let available = format_cents(raw.available_cents);
        let spent = format_cents(raw.spent_cents);

        // Status based on utilization
        let status = if raw.utilization_bp < 2000 {
            format!("{} Healthy ({}% used)", "✅", raw.utilization_bp / 100)
        } else if raw.utilization_bp < 5000 {
            format!("{} Warning ({}% used)", "⚠️", raw.utilization_bp / 100)
        } else {
            format!("{} Critical ({}% used)", "❌", raw.utilization_bp / 100)
        };

        // Trend (placeholder - would need historical data)
        let trend = "→".to_string();

        BudgetMetric {
            budget_id,
            available,
            spent,
            status,
            trend,
        }
    }

    /// Convert provider metric (raw → display)
    fn convert_provider_metric(&self, raw: ProviderMetricRaw) -> ProviderMetric {
        let provider = raw.provider_name;

        // Circuit breaker status
        let status = match raw.circuit_state {
            0 => format!("{} Closed", "✅"),
            1 => format!("{} HalfOpen", "⚠️"),
            2 => format!("{} Open", "❌"),
            _ => "Unknown".to_string(),
        };

        let failures = raw.failures_count.to_string();

        // Latency (convert ns → ms)
        let latency_ms = raw.latency_p99_ns / 1_000_000;
        let latency = format!("{}ms", latency_ms);

        // Response rate
        let success_rate_pct = raw.success_rate_bp / 100;
        let response_rate = format!(
            "{}% ({}/{})",
            success_rate_pct,
            (raw.total_requests * (raw.success_rate_bp as u64)) / 10000,
            raw.total_requests
        );

        ProviderMetric {
            provider,
            status,
            failures,
            latency,
            response_rate,
        }
    }

    /// Convert system metrics (raw → display)
    fn convert_system_metrics(&self, raw: SystemMetricsRaw) -> SystemMetrics {
        let uptime = format_duration(raw.uptime_secs);
        let avg_latency_ms = raw.avg_latency_ns / 1_000_000;
        let memory_mb = raw.memory_bytes / 1_048_576; // bytes → MB

        SystemMetrics {
            uptime,
            total_requests: raw.total_requests,
            avg_latency_ms,
            memory_mb,
            uptime_secs: raw.uptime_secs,
        }
    }

    /// Convert cache metrics (Week 3)
    fn convert_cache_metrics(&self, raw: CacheMetricsRaw) -> CacheMetricsPanel {
        let memory_mb = raw.memory_bytes as f64 / 1_048_576.0;

        CacheMetricsPanel {
            hit_rate: raw.hit_rate,
            memory_mb,
            entry_count: raw.entry_count,
            eviction_rate: raw.eviction_rate,
        }
    }

    /// Convert compression metrics (Week 3)
    fn convert_compression_metrics(&self, raw: CompressionMetricsRaw) -> CompressionMetricsPanel {
        let throughput_mbps = raw.throughput_bytes_per_sec as f64 / 1_048_576.0;
        let bandwidth_saved_mb = raw.bandwidth_saved_bytes / 1_048_576;

        CompressionMetricsPanel {
            compression_ratio: raw.compression_ratio,
            throughput_mbps,
            bandwidth_saved_mb,
        }
    }

    /// Convert load balancer metrics (Week 3)
    fn convert_load_balancer_metrics(&self, raw: LoadBalancerMetricsRaw) -> LoadBalancerMetricsPanel {
        LoadBalancerMetricsPanel {
            cost_per_1k_tokens: raw.cost_per_1k_tokens_cents,
            provider_latencies: raw.provider_latencies_ms,
            failover_rate: raw.failover_rate,
        }
    }

    /// Convert performance metrics (Week 3)
    fn convert_performance_metrics(&self, raw: PerformanceMetricsRaw) -> PerformanceMetricsPanel {
        PerformanceMetricsPanel {
            p50_ms: raw.p50_ms,
            p99_ms: raw.p99_ms,
            p999_ms: raw.p999_ms,
        }
    }

    /// Render dashboard frame to terminal
    ///
    /// # Performance
    /// - Terminal I/O: <100ms (full screen)
    /// - Table formatting: <10ms (tabled crate)
    ///
    /// # Layout
    /// - Header (title + refresh info)
    /// - Budget summary table
    /// - Provider status table
    /// - System metrics summary
    /// - Footer (controls)
    fn render_frame(&self, frame: &DashboardFrame, iteration: u64) -> Result<(), String> {
        let mut stdout = io::stdout();

        // Clear screen and move to top
        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .map_err(|e| format!("Failed to clear screen: {}", e))?;

        // Header
        writeln!(
            stdout,
            "{}",
            "┌─────────────────────────────────────────────────────────────────────────────┐"
                .bright_cyan()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ {} - Refreshing every {} seconds (iteration {})  │",
            "clapi Metrics Dashboard".bright_white().bold(),
            self.refresh_interval.as_secs().to_string().bright_yellow(),
            iteration.to_string().bright_black()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "{}",
            "├─────────────────────────────────────────────────────────────────────────────┤"
                .bright_cyan()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        // Budget Summary
        if frame.budget_metrics.is_empty() {
            writeln!(
                stdout,
                "│ {} {}",
                "BUDGET SUMMARY".bright_white().bold(),
                "(No budgets configured)".bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
        } else {
            writeln!(stdout, "│ {}", "BUDGET SUMMARY".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;

            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;

            let table = Table::new(&frame.budget_metrics).with(Style::modern()).to_string();
            for line in table.lines() {
                writeln!(stdout, "│ {}", line).map_err(|e| format!("Write failed: {}", e))?;
            }
        }

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        // Provider Status
        if frame.provider_metrics.is_empty() {
            writeln!(
                stdout,
                "│ {} {}",
                "PROVIDER STATUS".bright_white().bold(),
                "(No providers configured)".bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
        } else {
            writeln!(stdout, "│ {}", "PROVIDER STATUS".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;

            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;

            let table = Table::new(&frame.provider_metrics)
                .with(Style::modern())
                .to_string();
            for line in table.lines() {
                writeln!(stdout, "│ {}", line).map_err(|e| format!("Write failed: {}", e))?;
            }
        }

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        // System Metrics
        writeln!(stdout, "│ {}", "SYSTEM METRICS".bright_white().bold())
            .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ {}",
            "────────────────────────────────────────────────────────────────────────────"
                .bright_black()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ Uptime: {}  │  Memory: {} MB  │  Total Requests: {}",
            frame.system_metrics.uptime.bright_white(),
            frame.system_metrics.memory_mb.to_string().bright_white(),
            frame.system_metrics.total_requests.to_string().bright_white()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ Avg Latency: {}ms",
            frame
                .system_metrics
                .avg_latency_ms
                .to_string()
                .bright_white()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        // Week 3 Performance Panels
        if let Some(ref cache) = frame.cache_metrics {
            writeln!(stdout, "│ {}", "CACHE PERFORMANCE".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ Hit Rate: {:.1}%  │  Memory: {:.1} MB  │  Entries: {}  │  Evictions: {:.2}%",
                cache.hit_rate * 100.0,
                cache.memory_mb,
                cache.entry_count,
                cache.eviction_rate * 100.0
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;
        }

        if let Some(ref compression) = frame.compression_metrics {
            writeln!(stdout, "│ {}", "COMPRESSION".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ Ratio: {:.2}×  │  Throughput: {:.1} MB/s  │  Bandwidth Saved: {} MB",
                compression.compression_ratio,
                compression.throughput_mbps,
                compression.bandwidth_saved_mb
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;
        }

        if let Some(ref lb) = frame.load_balancer_metrics {
            writeln!(stdout, "│ {}", "LOAD BALANCER".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ Cost: ${:.4}/1K tokens  │  Failover Rate: {:.1}%",
                lb.cost_per_1k_tokens / 100.0,
                lb.failover_rate * 100.0
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            for (provider, latency) in &lb.provider_latencies {
                writeln!(
                    stdout,
                    "│   {}: {:.1}ms",
                    provider.bright_white(),
                    latency
                )
                .map_err(|e| format!("Write failed: {}", e))?;
            }
            writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;
        }

        if let Some(ref perf) = frame.performance_metrics {
            writeln!(stdout, "│ {}", "PERFORMANCE PROFILING".bright_white().bold())
                .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ {}",
                "────────────────────────────────────────────────────────────────────────────"
                    .bright_black()
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(
                stdout,
                "│ P50: {:.1}ms  │  P99: {:.1}ms  │  P999: {:.1}ms",
                perf.p50_ms,
                perf.p99_ms,
                perf.p999_ms
            )
            .map_err(|e| format!("Write failed: {}", e))?;
            writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;
        }

        // Footer (controls)
        let pause_status = if self.paused {
            "PAUSED".bright_red()
        } else {
            "LIVE".bright_green()
        };

        writeln!(
            stdout,
            "│ Status: {} │ Press 'q' to quit, 'p' to pause, 'r' to resume │",
            pause_status
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "{}",
            "└─────────────────────────────────────────────────────────────────────────────┘"
                .bright_cyan()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        stdout.flush().map_err(|e| format!("Flush failed: {}", e))?;

        Ok(())
    }

    /// Render error message
    fn render_error(&self, error: &str) -> Result<(), String> {
        let mut stdout = io::stdout();

        execute!(
            stdout,
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )
        .map_err(|e| format!("Failed to clear screen: {}", e))?;

        writeln!(stdout, "{}", "┌─────────────────────────────────────────────────────────────────────────────┐".bright_red())
            .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ {} {}",
            "❌".bright_red(),
            "Error fetching metrics".bright_red().bold()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│ {}", error.bright_white())
            .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ {} Retrying in {} seconds...",
            "⏳".bright_yellow(),
            self.refresh_interval.as_secs().to_string().bright_white()
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "│").map_err(|e| format!("Write failed: {}", e))?;

        writeln!(
            stdout,
            "│ Press 'q' to quit"
        )
        .map_err(|e| format!("Write failed: {}", e))?;

        writeln!(stdout, "{}", "└─────────────────────────────────────────────────────────────────────────────┘".bright_red())
            .map_err(|e| format!("Write failed: {}", e))?;

        stdout.flush().map_err(|e| format!("Flush failed: {}", e))?;

        Ok(())
    }
}

/// Format cents as dollars (helper function)
///
/// # Examples
/// ```
/// assert_eq!(format_cents(100), "$1.00");
/// assert_eq!(format_cents(10_000), "$100.00");
/// ```
fn format_cents(cents: i64) -> String {
    let dollars = cents as f64 / 100.0;
    if cents >= 0 {
        format!("${:.2}", dollars)
    } else {
        format!("-${:.2}", dollars.abs())
    }
}

/// Format duration as human-readable string
///
/// # Examples
/// ```
/// assert_eq!(format_duration(65), "1m 5s");
/// assert_eq!(format_duration(3661), "1h 1m 1s");
/// ```
fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
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
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(65), "1m 5s");
        assert_eq!(format_duration(3661), "1h 1m 1s");
        assert_eq!(format_duration(7200), "2h 0m 0s");
    }

    #[test]
    fn test_dashboard_creation() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);
        assert_eq!(dashboard.url, "http://localhost:8080/metrics");
        assert_eq!(dashboard.refresh_interval, Duration::from_secs(5));
        assert!(!dashboard.paused);
    }

    #[test]
    fn test_convert_budget_metric() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);

        // Test healthy status (<20% utilization)
        let raw = BudgetMetricRaw {
            budget_id: 123,
            available_cents: 90_00,
            spent_cents: 10_00,
            utilization_bp: 1000, // 10%
        };

        let metric = dashboard.convert_budget_metric(raw);
        assert_eq!(metric.budget_id, "budget_123");
        assert_eq!(metric.available, "$90.00");
        assert_eq!(metric.spent, "$10.00");
        assert!(metric.status.contains("Healthy"));
        assert!(metric.status.contains("10%"));

        // Test warning status (20-50% utilization)
        let raw_warning = BudgetMetricRaw {
            budget_id: 456,
            available_cents: 60_00,
            spent_cents: 40_00,
            utilization_bp: 4000, // 40%
        };
        let metric_warning = dashboard.convert_budget_metric(raw_warning);
        assert!(metric_warning.status.contains("Warning"));
        assert!(metric_warning.status.contains("40%"));

        // Test critical status (>50% utilization)
        let raw_critical = BudgetMetricRaw {
            budget_id: 789,
            available_cents: 20_00,
            spent_cents: 80_00,
            utilization_bp: 8000, // 80%
        };
        let metric_critical = dashboard.convert_budget_metric(raw_critical);
        assert!(metric_critical.status.contains("Critical"));
        assert!(metric_critical.status.contains("80%"));
    }

    #[test]
    fn test_convert_provider_metric() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);
        let raw = ProviderMetricRaw {
            provider_name: "anthropic".to_string(),
            circuit_state: 0, // Closed
            failures_count: 0,
            latency_p99_ns: 234_000_000, // 234ms
            success_rate_bp: 10000,      // 100%
            total_requests: 50,
        };

        let metric = dashboard.convert_provider_metric(raw);
        assert_eq!(metric.provider, "anthropic");
        assert!(metric.status.contains("Closed"));
        assert_eq!(metric.failures, "0");
        assert_eq!(metric.latency, "234ms");
        assert!(metric.response_rate.contains("100%"));
    }

    #[test]
    fn test_convert_system_metrics() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);
        let raw = SystemMetricsRaw {
            uptime_secs: 9254, // 2h 34m 14s
            total_requests: 847,
            avg_latency_ns: 389_000_000, // 389ms
            memory_bytes: 268_435_456,   // 256MB
        };

        let metrics = dashboard.convert_system_metrics(raw);
        assert_eq!(metrics.uptime, "2h 34m 14s");
        assert_eq!(metrics.total_requests, 847);
        assert_eq!(metrics.avg_latency_ms, 389);
        assert_eq!(metrics.memory_mb, 256);
    }

    #[test]
    fn test_convert_response_empty() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);
        let raw = MetricsResponse {
            budgets: vec![],
            providers: vec![],
            system: SystemMetricsRaw::default(),
            cache: None,
            compression: None,
            load_balancer: None,
            performance: None,
        };

        let frame = dashboard.convert_response(raw);
        assert_eq!(frame.budget_metrics.len(), 0);
        assert_eq!(frame.provider_metrics.len(), 0);
    }

    #[test]
    fn test_convert_response_multiple_budgets() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);
        let raw = MetricsResponse {
            budgets: vec![
                BudgetMetricRaw {
                    budget_id: 1,
                    available_cents: 100_00,
                    spent_cents: 0,
                    utilization_bp: 0,
                },
                BudgetMetricRaw {
                    budget_id: 2,
                    available_cents: 50_00,
                    spent_cents: 50_00,
                    utilization_bp: 5000,
                },
            ],
            providers: vec![],
            system: SystemMetricsRaw::default(),
            cache: None,
            compression: None,
            load_balancer: None,
            performance: None,
        };

        let frame = dashboard.convert_response(raw);
        assert_eq!(frame.budget_metrics.len(), 2);
        assert_eq!(frame.budget_metrics[0].budget_id, "budget_1");
        assert_eq!(frame.budget_metrics[1].budget_id, "budget_2");
    }

    #[test]
    fn test_circuit_breaker_status_mapping() {
        let dashboard = MetricsDashboard::new("http://localhost:8080/metrics".to_string(), 5);

        // Closed (0)
        let raw_closed = ProviderMetricRaw {
            provider_name: "test".to_string(),
            circuit_state: 0,
            failures_count: 0,
            latency_p99_ns: 100_000_000,
            success_rate_bp: 10000,
            total_requests: 100,
        };
        let metric = dashboard.convert_provider_metric(raw_closed);
        assert!(metric.status.contains("Closed"));

        // HalfOpen (1)
        let raw_half = ProviderMetricRaw {
            provider_name: "test".to_string(),
            circuit_state: 1,
            failures_count: 0,
            latency_p99_ns: 100_000_000,
            success_rate_bp: 10000,
            total_requests: 100,
        };
        let metric = dashboard.convert_provider_metric(raw_half);
        assert!(metric.status.contains("HalfOpen"));

        // Open (2)
        let raw_open = ProviderMetricRaw {
            provider_name: "test".to_string(),
            circuit_state: 2,
            failures_count: 0,
            latency_p99_ns: 100_000_000,
            success_rate_bp: 10000,
            total_requests: 100,
        };
        let metric = dashboard.convert_provider_metric(raw_open);
        assert!(metric.status.contains("Open"));
    }
}
