//! Observability and Monitoring Demo
//!
//! This example demonstrates the comprehensive observability features of AtomicHedgeCapsule
//! including metrics collection, health monitoring, diagnostics, and performance tracking.
//!
//! Run with:
//! ```bash
//! cargo run --example observability_demo --features "metrics diagnostics logging"
//! ```

use atomic_hedge_capsule::{
    metrics::{global_metrics, HealthStatus, MetricsCollector},
    track_hedge_operation, AtomicHedgeCapsule, ErrorCategory, HedgeError,
};
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), HedgeError> {
    println!("🔍 AtomicHedgeCapsule Observability Demo");
    println!("========================================");

    // Initialize global metrics collection
    let metrics = global_metrics();

    println!("\n1. Basic Metrics Collection");
    println!("---------------------------");

    // Create hedge capsule and perform operations
    let hedge = AtomicHedgeCapsule::create_hedge("BTCUSD", "NDAX", 1.0, 45000.0, 55000.0)?;

    // Track operations using the macro
    let result1 = track_hedge_operation!("submit_order", { hedge.submit_order() });
    println!("✓ Submit order: {:?}", result1);

    let result2 = track_hedge_operation!("execute_hedge", { hedge.execute_hedge(1.0) });
    println!("✓ Execute hedge: {:?}", result2);

    // Manual metrics recording
    metrics.record_operation(true, 150_000); // 150μs operation
    metrics.record_operation(true, 80_000); // 80μs operation
    metrics.record_operation(false, 500_000); // Failed 500μs operation
    metrics.record_error(ErrorCategory::Timeout);

    // Display initial metrics
    let snapshot = metrics.get_metrics_snapshot();
    println!("\nInitial Metrics Snapshot:");
    println!("  Total operations: {}", snapshot.total_operations);
    println!("  Success rate: {:.2}%", snapshot.success_rate);
    println!("  Average latency: {}ns", snapshot.avg_latency_ns);
    println!("  P99 latency: {}ns", snapshot.p99_latency_ns);
    println!("  Performance grade: {}", snapshot.performance_grade);

    println!("\n2. Health Monitoring");
    println!("--------------------");

    // Check system health
    let health = metrics.health_check();
    println!("System health: {:?}", health);
    println!(
        "Performance degraded: {}",
        metrics.is_performance_degraded()
    );

    // Performance summary for dashboards
    println!("Dashboard summary: {}", metrics.performance_summary());

    println!("\n3. Simulating Load and Monitoring");
    println!("---------------------------------");

    // Simulate various operation patterns
    simulate_trading_operations(metrics)?;

    // Re-check health after load
    let health_after_load = metrics.health_check();
    println!("Health after load: {:?}", health_after_load);

    println!("\n4. Diagnostic Analysis");
    println!("----------------------");

    // Generate diagnostics
    let diagnostics = metrics.diagnostics();

    if diagnostics.is_empty() {
        println!("✓ No performance issues detected");
    } else {
        println!("⚠️  Performance issues detected:");

        if !diagnostics.performance_issues.is_empty() {
            println!("\nPerformance Issues:");
            for issue in &diagnostics.performance_issues {
                println!("  - {}", issue);
            }
        }

        if !diagnostics.contention_hotspots.is_empty() {
            println!("\nContention Hotspots:");
            for hotspot in &diagnostics.contention_hotspots {
                println!("  - {}", hotspot);
            }
        }

        if !diagnostics.error_patterns.is_empty() {
            println!("\nError Patterns:");
            for pattern in &diagnostics.error_patterns {
                println!("  - {}", pattern);
            }
        }

        if !diagnostics.recommendations.is_empty() {
            println!("\nRecommendations:");
            for rec in &diagnostics.recommendations {
                println!("  • {}", rec);
            }
        }
    }

    println!("\n5. Advanced Operation Tracking");
    println!("------------------------------");

    // Demonstrate RAII operation tracking
    demonstrate_operation_guards(metrics)?;

    println!("\n6. Concurrent Load Testing");
    println!("--------------------------");

    // Test concurrent metrics collection
    concurrent_load_test(metrics)?;

    println!("\n7. Final System Analysis");
    println!("------------------------");

    let final_snapshot = metrics.get_metrics_snapshot();
    let final_health = metrics.health_check();
    let final_diagnostics = metrics.diagnostics();

    println!("Final Performance Analysis:");
    println!("  Total operations: {}", final_snapshot.total_operations);
    println!("  Success rate: {:.2}%", final_snapshot.success_rate);
    println!(
        "  Average latency: {}ns ({:.2}μs)",
        final_snapshot.avg_latency_ns,
        final_snapshot.avg_latency_ns as f64 / 1000.0
    );
    println!("  P50 latency: {}ns", final_snapshot.p50_latency_ns);
    println!("  P95 latency: {}ns", final_snapshot.p95_latency_ns);
    println!("  P99 latency: {}ns", final_snapshot.p99_latency_ns);
    println!("  Throughput: {:.0} ops/sec", final_snapshot.ops_per_second);
    println!("  CAS retry rate: {:.2}%", final_snapshot.cas_retry_rate);
    println!("  Error rate: {:.2}%", final_snapshot.error_rate);
    println!("  Performance grade: {}", final_snapshot.performance_grade);
    println!("  System health: {:?}", final_health);

    if final_health.is_degraded() {
        println!("\n⚠️  System performance is degraded!");
        println!("Detailed diagnostics:");
        for rec in &final_diagnostics.recommendations {
            println!("  • {}", rec);
        }
    } else {
        println!("\n✅ System is operating at optimal performance");
    }

    println!("\n🎯 Observability Demo Complete!");
    Ok(())
}

/// Simulate various trading operation patterns
fn simulate_trading_operations(metrics: &MetricsCollector) -> Result<(), HedgeError> {
    println!("Simulating trading operations...");

    // Fast successful operations (normal case)
    for i in 0..50 {
        let latency = 50_000 + (i * 1000); // 50-100μs range
        metrics.record_operation(true, latency);

        if i % 10 == 0 {
            print!(".");
        }
    }

    // Some slower operations
    for _ in 0..10 {
        metrics.record_operation(true, 250_000); // 250μs
    }

    // Occasional failures
    for _ in 0..5 {
        metrics.record_operation(false, 100_000);
        metrics.record_error(ErrorCategory::Coordination);
    }

    // Simulate contention
    for _ in 0..20 {
        metrics.record_cas_retry();
    }

    println!(" ✓ Completed 65 operations");
    Ok(())
}

/// Demonstrate RAII operation tracking
fn demonstrate_operation_guards(metrics: &MetricsCollector) -> Result<(), HedgeError> {
    println!("Testing operation guards...");

    // Successful operation
    {
        let guard = metrics.track_operation("successful_operation");
        thread::sleep(Duration::from_micros(100)); // 100μs work
        guard.success();
    }

    // Failed operation
    {
        let guard = metrics.track_operation("failed_operation");
        thread::sleep(Duration::from_micros(50)); // 50μs work
        guard.error(ErrorCategory::Validation);
    }

    // Auto-dropped operation (becomes timeout)
    {
        let _guard = metrics.track_operation("timeout_operation");
        thread::sleep(Duration::from_micros(200)); // 200μs work
                                                   // Guard dropped here, automatically recorded as timeout
    }

    println!("✓ Operation guards tested");
    Ok(())
}

/// Test concurrent metrics collection
fn concurrent_load_test(metrics: &MetricsCollector) -> Result<(), HedgeError> {
    println!("Running concurrent load test...");

    let start_time = Instant::now();
    let mut handles = Vec::new();

    // Spawn worker threads
    for thread_id in 0..4 {
        let metrics_clone = global_metrics().clone();
        let handle = thread::spawn(move || {
            for i in 0..25 {
                let operation_name = format!("thread_{}_op_{}", thread_id, i);
                let guard = metrics_clone.track_operation(&operation_name);

                // Simulate variable work
                let work_time = 50 + (thread_id * 20) + (i % 10); // 50-90μs range
                thread::sleep(Duration::from_micros(work_time));

                // 90% success rate
                if (thread_id + i) % 10 < 9 {
                    guard.success();
                } else {
                    guard.error(ErrorCategory::Timeout);
                }
            }
        });
        handles.push(handle);
    }

    // Wait for completion
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let duration = start_time.elapsed();
    println!(
        "✓ Completed 100 concurrent operations in {:.2}ms",
        duration.as_millis()
    );

    Ok(())
}
