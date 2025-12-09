//! # standalone.rs - Standalone Dashboard Example (No clapi_core)
//!
//! **Purpose**: Demonstrates kindly_dash as a standalone monitoring solution with custom metrics
//!
//! ## What This Example Shows
//!
//! 1. **Custom MetricsSource**: Implement the trait from scratch:
//!    - No dependency on clapi_core
//!    - Pure atomic counters for state
//!    - Suitable for any Rust project
//!
//! 2. **Live Updates**: Background thread updating metrics:
//!    - Requests: +100 every second
//!    - Cost: Incrementing by $0.50-$2.00 per second
//!    - Latency: Random 50-200ms simulation
//!
//! 3. **Simple Integration**: Minimal code to get dashboard running:
//!    - ~250 lines total (vs 400+ with clapi_core)
//!    - Clear patterns for custom implementations
//!
//! 4. **Production Patterns**: Shows how to:
//!    - Use Arc<AtomicU64> for shared state
//!    - Implement all MetricsSource methods
//!    - Handle optional features (forecast, alerts)
//!
//! ## Usage
//!
//! ```bash
//! # Run without any features (no clapi_core dependency)
//! cargo run --example standalone --no-default-features
//!
//! # Or with default features (still works)
//! cargo run --example standalone
//! ```
//!
//! ## API Endpoints
//!
//! - **Dashboard**: http://localhost:8081/dashboard (HTML placeholder)
//! - **Metrics JSON**: http://localhost:8081/dashboard/metrics
//! - **Health Check**: http://localhost:8081/dashboard/health
//!
//! ## Expected Output
//!
//! ```text
//! [INFO] ========================================
//! [INFO] Standalone Dashboard Example
//! [INFO] ========================================
//! [INFO] Initializing custom metrics source...
//! [INFO] ✓ MyMetricsSource created (atomic counters)
//! [INFO] Building DashboardServer...
//! [INFO] ✓ Server configured (port 8081)
//! [INFO] Spawning HTTP server on 0.0.0.0:8081...
//! [INFO] Dashboard URLs:
//! [INFO]   Metrics: http://localhost:8081/dashboard/metrics
//! [INFO]   Health:  http://localhost:8081/dashboard/health
//! [INFO] ----------------------------------------
//! [INFO] Press Ctrl-C to stop server...
//! ```
//!
//! ## Performance Targets
//!
//! - Metrics snapshot: <50ns (4 atomic loads)
//! - Health check: <30ns (2 atomic loads)
//! - Live metric updates: 1Hz (every 1 second)
//!
//! ## UCE34 Compliance
//!
//! - **Q10 Tier**: T1 Atomic (AtomicU64 counters)
//! - **Q28 Simplification**: Minimal API surface, clear patterns
//! - **Q31 Rust**: 100% safe Rust, no unsafe blocks

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicI64, Ordering};
use std::time::Duration;

use kindly_dash::{
    DashboardServer, MetricsSource,
    DashboardSnapshot, BudgetMetrics, ProviderMetrics, Alert, Forecast,
    CircuitState,
};

use tracing::info;
use tracing_subscriber;

/// Custom metrics source using atomic counters
///
/// This is the simplest possible MetricsSource implementation.
/// Suitable for any Rust project that wants a dashboard.
///
/// # Architecture
///
/// - All state is Arc<AtomicU64> for lockfree shared access
/// - Methods use Relaxed ordering (counters are independent)
/// - No complex data structures (no HashMaps, no circuits)
/// - Budget/provider/alert features return empty (optional)
struct MyMetricsSource {
    /// Total requests processed (monotonic counter)
    ///
    /// # Safety
    /// - Relaxed ordering sufficient (independent counter)
    /// - Never decreases (monotonic)
    requests: Arc<AtomicU64>,

    /// Total cost in cents (monotonic counter)
    ///
    /// # Safety
    /// - Relaxed ordering sufficient (independent counter)
    /// - Can overflow after $184,467,440,737,095.51615 (2^64 cents)
    cost_cents: Arc<AtomicI64>,

    /// P99 latency in milliseconds (last recorded value)
    ///
    /// # Safety
    /// - Relaxed ordering sufficient (last-write-wins)
    /// - Approximate (not exact P99, just a demo)
    latency_p99_ms: Arc<AtomicU64>,

    /// Number of failures (monotonic counter)
    ///
    /// # Safety
    /// - Relaxed ordering sufficient (independent counter)
    /// - Never decreases (monotonic)
    failures: Arc<AtomicU64>,
}

impl MyMetricsSource {
    /// Create new metrics source with zero initial values
    ///
    /// # Performance
    /// - <20ns (4 Arc::new allocations)
    fn new() -> Self {
        Self {
            requests: Arc::new(AtomicU64::new(0)),
            cost_cents: Arc::new(AtomicI64::new(0)),
            latency_p99_ms: Arc::new(AtomicU64::new(150)), // Start at 150ms
            failures: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Increment request counter
    ///
    /// # Performance
    /// - <10ns (atomic fetch_add)
    ///
    /// # Safety
    /// - Relaxed ordering: Request counter is independent
    fn increment_requests(&self, count: u64) {
        self.requests.fetch_add(count, Ordering::Relaxed);
    }

    /// Add to cost
    ///
    /// # Performance
    /// - <10ns (atomic fetch_add)
    ///
    /// # Safety
    /// - Relaxed ordering: Cost counter is independent
    fn add_cost(&self, cents: i64) {
        self.cost_cents.fetch_add(cents, Ordering::Relaxed);
    }

    /// Update P99 latency
    ///
    /// # Performance
    /// - <10ns (atomic store)
    ///
    /// # Safety
    /// - Relaxed ordering: Last-write-wins for latency
    fn update_latency(&self, ms: u64) {
        self.latency_p99_ms.store(ms, Ordering::Relaxed);
    }

    /// Increment failure counter
    ///
    /// # Performance
    /// - <10ns (atomic fetch_add)
    ///
    /// # Safety
    /// - Relaxed ordering: Failure counter is independent
    fn increment_failures(&self, count: u64) {
        self.failures.fetch_add(count, Ordering::Relaxed);
    }
}

impl MetricsSource for MyMetricsSource {
    /// Complete snapshot of all metrics
    ///
    /// # Performance
    /// - <50ns (4 atomic loads @ ~10ns each)
    ///
    /// # Safety
    /// - Relaxed ordering: Each counter is independent
    /// - Snapshot may be inconsistent (some counters newer than others)
    /// - Acceptable for dashboard display
    fn snapshot(&self) -> DashboardSnapshot {
        // Load all counters (Relaxed is sufficient for independent counters)
        let total_requests = self.requests.load(Ordering::Relaxed);
        let total_cost_cents = self.cost_cents.load(Ordering::Relaxed);
        let total_failures = self.failures.load(Ordering::Relaxed);

        // Calculate success rate (basis points: 0-10000 = 0.00%-100.00%)
        let global_success_rate_bp = if total_requests > 0 {
            let successes = total_requests.saturating_sub(total_failures);
            ((successes as f64 / total_requests as f64) * 10000.0) as u64
        } else {
            10000 // 100% if no requests yet
        };

        DashboardSnapshot {
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64,
            total_cost_cents,
            total_requests,
            total_failures,
            global_success_rate_bp,
            circuit_breaker_state: CircuitState::Closed, // Always closed (no circuit breaker)
            circuit_failure_rate_bp: 0, // No circuit breaker
            circuit_last_trip_ns: 0,    // No circuit breaker
            active_providers: 1,         // Single "provider" (this app)
            total_providers: 1,
            active_budgets: 0,           // No budget tracking
            total_budgets: 0,
            budgets_low: 0,
            budgets_critical: 0,
            active_alerts: 0,            // No alerts
            alerts_critical: 0,
            alerts_warning: 0,
        }
    }

    /// Budget-specific metrics (not implemented)
    ///
    /// # Note
    /// This example doesn't track individual budgets.
    /// Return None for all budget_id queries.
    fn budget_metrics(&self, _budget_id: u64) -> Option<BudgetMetrics> {
        None
    }

    /// Provider metrics (single "provider" = this app)
    ///
    /// # Performance
    /// - <100ns (allocate Vec + populate struct)
    fn provider_metrics(&self) -> Vec<ProviderMetrics> {
        let total_requests = self.requests.load(Ordering::Relaxed);
        let total_failures = self.failures.load(Ordering::Relaxed);
        let latency_p99_ms = self.latency_p99_ms.load(Ordering::Relaxed);

        vec![ProviderMetrics {
            provider_id: 1,
            name: "MyApp".to_string(),
            circuit_state: CircuitState::Closed,
            requests: total_requests,
            failures: total_failures,
            success_rate_bp: if total_requests > 0 {
                let successes = total_requests.saturating_sub(total_failures);
                ((successes as f64 / total_requests as f64) * 10000.0) as u64
            } else {
                10000 // 100%
            },
            cost_cents: self.cost_cents.load(Ordering::Relaxed),
            latency_p50_ms: latency_p99_ms / 2, // Approximate (demo only)
            latency_p99_ms,
            latency_p999_ms: latency_p99_ms + 50, // Approximate
            latency_max_ms: latency_p99_ms + 100, // Approximate
        }]
    }

    /// Alert history (not implemented)
    ///
    /// # Note
    /// This example doesn't track alerts.
    /// Return empty vector.
    fn alert_history(&self) -> Vec<Alert> {
        Vec::new()
    }

    /// Budget forecast (not implemented)
    ///
    /// # Note
    /// This example doesn't implement forecasting.
    /// Return None for all queries.
    fn forecast(&self, _budget_id: u64, _days: u32) -> Option<Forecast> {
        None
    }

    /// Maximum budgets (not implemented)
    fn max_budgets(&self) -> u64 {
        0
    }

    /// Maximum providers (single provider)
    fn max_providers(&self) -> u64 {
        1
    }

    /// Implementation name
    fn implementation_name(&self) -> &str {
        "standalone_example"
    }
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (INFO level)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("========================================");
    info!("Standalone Dashboard Example");
    info!("========================================");

    // ============================================================================
    // STEP 1: Create Custom Metrics Source
    // ============================================================================

    info!("Initializing custom metrics source...");
    let metrics = Arc::new(MyMetricsSource::new());
    info!("✓ MyMetricsSource created (4 atomic counters)");

    info!("----------------------------------------");

    // ============================================================================
    // STEP 2: Build and Spawn DashboardServer
    // ============================================================================

    info!("Building DashboardServer...");
    let mut server = DashboardServer::builder()
        .metrics_source(metrics.clone())
        .port(8081) // Different port from clapi_integration example
        .enable_compression()
        .build()
        .map_err(|e| format!("Failed to build server: {}", e))?;

    info!("✓ Server configured (port 8081, Brotli compression)");

    info!("Spawning HTTP server on 0.0.0.0:8081...");
    server.spawn().await
        .map_err(|e| format!("Failed to spawn server: {}", e))?;

    info!("✓ Server spawned successfully");
    info!("========================================");
    info!("Dashboard URLs:");
    info!("  HTML:    http://localhost:8081/dashboard");
    info!("  Metrics: http://localhost:8081/dashboard/metrics");
    info!("  Health:  http://localhost:8081/dashboard/health");
    info!("========================================");

    // ============================================================================
    // STEP 3: Spawn Background Metric Updater
    // ============================================================================

    info!("Starting background metric updater (1Hz)...");

    let metrics_clone = metrics.clone();

    let updater_handle = tokio::spawn(async move {
        let mut tick = 0u64;

        loop {
            tokio::time::sleep(Duration::from_secs(1)).await; // 1Hz updates
            tick += 1;

            // Simulate request traffic: +100 requests per second
            metrics_clone.increment_requests(100);

            // Simulate cost: Random $0.50-$2.00 per second
            let cost_cents = 50 + (tick % 150); // 50-200 cents ($0.50-$2.00)
            metrics_clone.add_cost(cost_cents as i64);

            // Simulate latency fluctuation: 50-200ms
            let latency_ms = 50 + (tick % 150);
            metrics_clone.update_latency(latency_ms);

            // Simulate occasional failures: 1-5 per second
            let failure_count = 1 + (tick % 5);
            metrics_clone.increment_failures(failure_count);

            // Print stats every 10 seconds
            if tick % 10 == 0 {
                let snapshot = metrics_clone.snapshot();
                info!(
                    "Stats @ {}s: {} requests, ${:.2} cost, {:.2}% success, {}ms P99",
                    tick,
                    snapshot.total_requests,
                    snapshot.total_cost_cents as f64 / 100.0,
                    snapshot.global_success_rate_bp as f64 / 100.0,
                    metrics_clone.latency_p99_ms.load(Ordering::Relaxed),
                );
            }
        }
    });

    info!("✓ Background updater started (1Hz simulation)");
    info!("----------------------------------------");
    info!("Press Ctrl-C to stop server...");
    info!("----------------------------------------");

    // ============================================================================
    // STEP 4: Wait for Ctrl-C and Graceful Shutdown
    // ============================================================================

    // Wait for Ctrl-C signal
    tokio::signal::ctrl_c().await
        .map_err(|e| format!("Failed to listen for Ctrl-C: {}", e))?;

    info!("");
    info!("========================================");
    info!("Received Ctrl-C, shutting down...");
    info!("========================================");

    // Abort background updater
    updater_handle.abort();
    let _ = updater_handle.await;
    info!("✓ Background updater stopped");

    // Shutdown server gracefully
    server.shutdown().await;
    info!("✓ Dashboard server stopped");

    // Print final stats
    let final_snapshot = metrics.snapshot();
    info!("----------------------------------------");
    info!("Final Statistics:");
    info!("  Total requests: {}", final_snapshot.total_requests);
    info!("  Total failures: {}", final_snapshot.total_failures);
    info!("  Total cost:     ${:.2}", final_snapshot.total_cost_cents as f64 / 100.0);
    info!("  Success rate:   {:.2}%", final_snapshot.global_success_rate_bp as f64 / 100.0);
    info!("========================================");
    info!("Shutdown complete. Goodbye!");

    Ok(())
}

/// Test module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_metrics_source_creation() {
        let metrics = MyMetricsSource::new();

        // Initial values should be zero
        assert_eq!(metrics.requests.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.cost_cents.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.failures.load(Ordering::Relaxed), 0);
        assert_eq!(metrics.latency_p99_ms.load(Ordering::Relaxed), 150); // Initial value
    }

    #[test]
    fn test_increment_operations() {
        let metrics = MyMetricsSource::new();

        // Increment requests
        metrics.increment_requests(100);
        assert_eq!(metrics.requests.load(Ordering::Relaxed), 100);

        // Add cost
        metrics.add_cost(500);
        assert_eq!(metrics.cost_cents.load(Ordering::Relaxed), 500);

        // Update latency
        metrics.update_latency(200);
        assert_eq!(metrics.latency_p99_ms.load(Ordering::Relaxed), 200);

        // Increment failures
        metrics.increment_failures(5);
        assert_eq!(metrics.failures.load(Ordering::Relaxed), 5);
    }

    #[test]
    fn test_snapshot() {
        let metrics = MyMetricsSource::new();

        // Add some metrics
        metrics.increment_requests(1000);
        metrics.add_cost(5000);
        metrics.increment_failures(50);

        // Get snapshot
        let snapshot = metrics.snapshot();

        // Verify
        assert_eq!(snapshot.total_requests, 1000);
        assert_eq!(snapshot.total_cost_cents, 5000);
        assert_eq!(snapshot.total_failures, 50);

        // Success rate: (1000 - 50) / 1000 = 95.00% = 9500 bp
        assert_eq!(snapshot.global_success_rate_bp, 9500);
    }

    #[test]
    fn test_provider_metrics() {
        let metrics = MyMetricsSource::new();
        metrics.increment_requests(1000);
        metrics.add_cost(10000);

        let providers = metrics.provider_metrics();

        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "MyApp");
        assert_eq!(providers[0].requests, 1000);
        assert_eq!(providers[0].cost_cents, 10000);
    }

    #[test]
    fn test_implementation_name() {
        let metrics = MyMetricsSource::new();
        assert_eq!(metrics.implementation_name(), "standalone_example");
    }

    #[tokio::test]
    async fn test_server_creation() {
        let metrics = Arc::new(MyMetricsSource::new());

        let server = DashboardServer::builder()
            .metrics_source(metrics)
            .port(8082) // Different port
            .build();

        assert!(server.is_ok(), "Server should build successfully");
    }
}
