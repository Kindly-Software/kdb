//! # clapi_integration.rs - Full clapi_core Integration Example
//!
//! **Purpose**: Demonstrates complete integration with clapi_core using ClapiMetricsAdapter
//!
//! **NOTE**: This example requires the `dashboard` feature in clapi_core, which is currently
//! disabled due to circular dependency during Phase 2 development. This example will be
//! functional once the circular dependency is resolved.
//!
//! ## What This Example Shows
//!
//! 1. **Real Metrics Integration**: Uses actual clapi_core components:
//!    - BudgetRegistry (1M budget slots, lockfree)
//!    - MetricsSnapshot (global stats tracking)
//!    - ProviderCircuitArray (16 circuit breakers)
//!    - HistogramCapsule (50× faster latency tracking)
//!
//! 2. **Dashboard Server Setup**: Complete setup with:
//!    - HTTP server on localhost:8080
//!    - WebSocket endpoint (future Phase 2.1)
//!    - JSON metrics API
//!    - Health check endpoint
//!
//! 3. **Live Metric Updates**: Background thread simulating:
//!    - Budget allocations and deductions
//!    - Provider circuit state changes
//!    - Latency spikes and normal operations
//!
//! 4. **Graceful Shutdown**: Ctrl-C handler with cleanup
//!
//! ## Usage
//!
//! ```bash
//! # This example is temporarily disabled due to circular dependency
//! # It will be enabled once clapi_core resolves the circular dependency
//! # For now, use standalone.rs or custom_metrics.rs examples
//!
//! # Future usage (when enabled):
//! cargo run --example clapi_integration --features clapi-integration
//! ```
//!
//! ## API Endpoints
//!
//! - **Dashboard**: http://localhost:8080/dashboard (HTML placeholder)
//! - **WebSocket**: ws://localhost:8080/dashboard/ws (Phase 2.1)
//! - **Metrics JSON**: http://localhost:8080/dashboard/metrics
//! - **Health Check**: http://localhost:8080/dashboard/health
//!
//! ## Expected Output
//!
//! ```text
//! [INFO] Initializing clapi_core components...
//! [INFO] Budget Registry: 1M slots, 128B per slot
//! [INFO] Metrics Snapshot: Global counters
//! [INFO] Provider Circuits: 16 independent circuits
//! [INFO] Latency Histogram: 1024 buckets, 1ns-10s range
//! [INFO] -------------------------
//! [INFO] Creating ClapiMetricsAdapter...
//! [INFO] Building DashboardServer...
//! [INFO] Spawning HTTP server on 0.0.0.0:8080...
//! [INFO] Dashboard server listening on http://0.0.0.0:8080/dashboard
//! [INFO] -------------------------
//! [INFO] Dashboard URLs:
//! [INFO]   HTML:      http://localhost:8080/dashboard
//! [INFO]   WebSocket: ws://localhost:8080/dashboard/ws (Phase 2.1)
//! [INFO]   Metrics:   http://localhost:8080/dashboard/metrics
//! [INFO]   Health:    http://localhost:8080/dashboard/health
//! [INFO] -------------------------
//! [INFO] Press Ctrl-C to stop server...
//! ```
//!
//! ## Performance Targets
//!
//! - Metrics snapshot: <100ns (8 atomic loads)
//! - Health check: <50ns (StatsCapsule64)
//! - Live metric updates: 10Hz (every 100ms)
//! - Zero allocations on hot path
//!
//! ## UCE34 Compliance
//!
//! - **Q1-Q9**: Example demonstrates full integration pattern
//! - **Q10 Tier**: Uses T1 Atomic (BudgetRegistry, MetricsSnapshot, ProviderCircuitArray)
//! - **Q33 Verification**: All capsules use #[derive(ComputationalCapsule)]
//! - **Q34 Auditability**: Hash chains via audit-trail feature (future)

use std::sync::Arc;
use std::time::Duration;

// Conditional compilation based on whether clapi_core has dashboard feature enabled
#[cfg(feature = "clapi-integration")]
use clapi_core::dashboard::ClapiMetricsAdapter;
#[cfg(feature = "clapi-integration")]
use clapi_core::proxy::BudgetRegistry;
#[cfg(feature = "clapi-integration")]
use clapi_core::capsules::{MetricsSnapshot, ProviderCircuitArray};
#[cfg(feature = "clapi-integration")]
use atomic_capsule::collections::HistogramCapsule;

#[cfg(feature = "clapi-integration")]
use kindly_dash::DashboardServer;

use tracing::{info, warn};
use tracing_subscriber;

/// Main entry point
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(not(feature = "clapi-integration"))]
    {
        eprintln!("ERROR: This example requires the clapi-integration feature");
        eprintln!("However, clapi-integration is temporarily disabled due to circular dependency");
        eprintln!("");
        eprintln!("Please use one of these examples instead:");
        eprintln!("  cargo run --example standalone");
        eprintln!("  cargo run --example custom_metrics");
        eprintln!("");
        eprintln!("This example will be enabled once clapi_core resolves the circular dependency.");
        std::process::exit(1);
    }

    #[cfg(feature = "clapi-integration")]
    run_example().await
}

#[cfg(feature = "clapi-integration")]
async fn run_example() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (INFO level for visibility)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("========================================");
    info!("clapi_core + kindly_dash Integration");
    info!("========================================");

    // ============================================================================
    // STEP 1: Initialize clapi_core components
    // ============================================================================

    info!("Initializing clapi_core components...");

    // Budget Registry: 1M budget slots, 128B per slot = 128MB preallocated
    let registry = Arc::new(BudgetRegistry::new(1_000_000));
    info!("✓ Budget Registry: 1M slots, 128B per slot (128MB preallocated)");

    // Metrics Snapshot: Global counters (atomic)
    let metrics = Arc::new(MetricsSnapshot::new());
    info!("✓ Metrics Snapshot: Global atomic counters");

    // Provider Circuit Array: 16 independent circuit breakers
    let circuits = Arc::new(ProviderCircuitArray::new());
    info!("✓ Provider Circuits: 16 independent circuits (64B per circuit)");

    // Latency Histogram: 1024 logarithmic buckets covering 1ns-10s range
    // 50× faster than hdrhistogram (200-500ns → <10ns record)
    let histogram = Arc::new(HistogramCapsule::new());
    info!("✓ Latency Histogram: 1024 buckets, 1ns-10s range (8KB memory)");

    info!("----------------------------------------");

    // ============================================================================
    // STEP 2: Create ClapiMetricsAdapter
    // ============================================================================

    info!("Creating ClapiMetricsAdapter...");
    let adapter = ClapiMetricsAdapter::new(
        metrics.clone(),
        registry.clone(),
        circuits.clone(),
        histogram.clone(),
    );
    info!("✓ ClapiMetricsAdapter created (<10ns via 4 Arc::clone)");

    info!("----------------------------------------");

    // ============================================================================
    // STEP 3: Build and Spawn DashboardServer
    // ============================================================================

    info!("Building DashboardServer...");
    let mut server = DashboardServer::builder()
        .metrics_source(Arc::new(adapter))
        .port(8080)
        .enable_cors(vec![
            "http://localhost:3000".to_string(),
            "http://localhost:5173".to_string(), // Vite dev server
        ])
        .enable_compression()
        .broadcast_capacity(1000)
        .build()
        .map_err(|e| format!("Failed to build server: {}", e))?;

    info!("✓ DashboardServer configured (port 8080, CORS + Brotli)");

    info!("Spawning HTTP server on 0.0.0.0:8080...");
    server.spawn().await
        .map_err(|e| format!("Failed to spawn server: {}", e))?;

    info!("✓ Server spawned successfully");
    info!("========================================");
    info!("Dashboard URLs:");
    info!("  HTML:      http://localhost:8080/dashboard");
    info!("  WebSocket: ws://localhost:8080/dashboard/ws (Phase 2.1)");
    info!("  Metrics:   http://localhost:8080/dashboard/metrics");
    info!("  Health:    http://localhost:8080/dashboard/health");
    info!("========================================");

    // ============================================================================
    // STEP 4: Spawn Background Metric Updater (Simulation)
    // ============================================================================

    info!("Starting background metric updater (10Hz)...");

    let metrics_clone = metrics.clone();
    let registry_clone = registry.clone();
    let circuits_clone = circuits.clone();
    let histogram_clone = histogram.clone();

    let updater_handle = tokio::spawn(async move {
        let mut tick = 0u64;

        loop {
            tokio::time::sleep(Duration::from_millis(100)).await; // 10Hz updates
            tick += 1;

            // Simulate budget deductions (every tick)
            // $1.00 per request (100 cents)
            if let Err(e) = metrics_clone.record_deduction(100) {
                warn!("Deduction failed: {:?}", e);
            }

            // Simulate latency recordings (vary between 50-200ms)
            let latency_ns = 50_000_000 + (tick % 150) * 1_000_000; // 50-200ms
            histogram_clone.record(latency_ns);

            // Simulate occasional failures (every 10 ticks = 1 second)
            if tick % 10 == 0 {
                metrics_clone.record_failure();
            }

            // Simulate circuit breaker state changes (every 50 ticks = 5 seconds)
            if tick % 50 == 0 {
                // Simulate provider 1 opening circuit (high error rate)
                // Note: In real code, this would be triggered by error thresholds
                info!("Simulation: Provider 1 circuit breaker opened (simulated high error rate)");
            }

            // Print stats every 10 seconds (100 ticks)
            if tick % 100 == 0 {
                let snapshot = metrics_clone.snapshot();
                info!(
                    "Stats @ {}s: {} deductions, {} failures, {} total cost (cents)",
                    tick / 10,
                    snapshot.deductions_total,
                    snapshot.failures_total,
                    snapshot.window_cost_cents,
                );
            }
        }
    });

    info!("✓ Background updater started (10Hz simulation)");
    info!("----------------------------------------");
    info!("Press Ctrl-C to stop server...");
    info!("----------------------------------------");

    // ============================================================================
    // STEP 5: Wait for Ctrl-C and Graceful Shutdown
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
    info!("  Total deductions: {}", final_snapshot.deductions_total);
    info!("  Total failures:   {}", final_snapshot.failures_total);
    info!("  Total cost:       ${:.2}", final_snapshot.window_cost_cents as f64 / 100.0);
    info!(
        "  Success rate:     {:.2}%",
        if final_snapshot.deductions_total > 0 {
            (final_snapshot.deductions_total as f64 / (final_snapshot.deductions_total + final_snapshot.failures_total) as f64) * 100.0
        } else {
            100.0
        }
    );
    info!("========================================");
    info!("Shutdown complete. Goodbye!");

    Ok(())
}

/// Test module (unit tests for example code)
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_compiles() {
        // Smoke test: Ensure example compiles
        // Actual test would require tokio runtime
    }

    #[tokio::test]
    async fn test_server_creation() {
        // Test: Can create server with minimal setup
        let registry = Arc::new(BudgetRegistry::new(100));
        let metrics = Arc::new(MetricsSnapshot::new());
        let circuits = Arc::new(ProviderCircuitArray::new());
        let histogram = Arc::new(HistogramCapsule::new());

        let adapter = ClapiMetricsAdapter::new(
            metrics,
            registry,
            circuits,
            histogram,
        );

        let server = DashboardServer::builder()
            .metrics_source(Arc::new(adapter))
            .port(8081) // Different port to avoid conflicts
            .build();

        assert!(server.is_ok(), "Server should build successfully");
    }

    #[tokio::test]
    async fn test_metrics_snapshot() {
        // Test: Metrics snapshot works
        let registry = Arc::new(BudgetRegistry::new(100));
        let metrics = Arc::new(MetricsSnapshot::new());
        let circuits = Arc::new(ProviderCircuitArray::new());
        let histogram = Arc::new(HistogramCapsule::new());

        // Record some metrics
        metrics.record_deduction(100).unwrap();
        metrics.record_failure();

        let adapter = ClapiMetricsAdapter::new(
            metrics.clone(),
            registry,
            circuits,
            histogram,
        );

        // Get snapshot
        let snapshot = adapter.snapshot();

        // Verify
        assert_eq!(snapshot.total_requests, 2); // 1 deduction + 1 failure
        assert_eq!(snapshot.total_failures, 1);
        assert_eq!(snapshot.total_cost_cents, 100); // 100 cents = $1.00
    }
}
