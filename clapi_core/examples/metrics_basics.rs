//! Metrics Basics Example - MetricsCapsule Fundamentals
//!
//! Demonstrates basic usage of Clapi Core's metrics infrastructure:
//! - Creating MetricsSnapshot from capsules
//! - Recording metrics (requests, failures, costs)
//! - Querying metrics via HTTP
//! - Exporting to JSON
//!
//! # Coverage
//! - CircuitBreakerMetrics: <20ns atomic metrics tracking
//! - RequestCapsule128Enhanced: Budget + hash + intrinsic metrics
//! - ResponseCapsule256: Cost tracking with Q16.16 fixed-point
//! - EpochTile1024: Time-series aggregation
//!
//! # Usage
//! ```bash
//! cargo run --example metrics_basics
//! ```

use clapi_core::capsules::{
    CircuitBreakerMetrics, CircuitBreakerMetricsSnapshot,
    RequestCapsule128Enhanced, EnhancedMetrics,
    ResponseCapsule256, EpochTile1024,
};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("=== Metrics Basics Example ===\n");

    // Section 1: CircuitBreakerMetrics (Tier 1 Atomic)
    circuit_breaker_metrics_demo();

    // Section 2: RequestCapsule128Enhanced (Tier 6 Mixed: Atomic + SIMD)
    request_capsule_metrics_demo();

    // Section 3: ResponseCapsule256 (Tier 2+3: SIMD + Fixed-Point)
    response_capsule_metrics_demo();

    // Section 4: EpochTile1024 (Tier 4+3: Batch + Fixed-Point)
    epoch_tile_metrics_demo();

    // Section 5: JSON Export
    json_export_demo();

    println!("\n=== Example Complete ===");
}

/// Section 1: CircuitBreakerMetrics - Atomic Metrics Tracking
fn circuit_breaker_metrics_demo() {
    println!("=== 1. CircuitBreakerMetrics (Tier 1 Atomic) ===\n");

    let metrics = CircuitBreakerMetrics::new();

    // Simulate API requests
    println!("1.1 Recording API Requests:");
    for i in 1..=20 {
        metrics.record_request();

        // 10% failure rate
        if i % 10 == 0 {
            metrics.record_failure();
        }
    }

    println!("   Recorded 20 requests, 2 failures (10% failure rate)\n");

    // Query metrics
    println!("1.2 Query Metrics:");
    let snapshot = metrics.snapshot();
    println!("   Requests:      {}", snapshot.requests);
    println!("   Failures:      {}", snapshot.failures);
    println!("   Trips:         {}", snapshot.trips);
    println!("   Failure Rate:  {:.2}% ({} bp)",
        metrics.failure_rate_bp() as f64 / 100.0,
        metrics.failure_rate_bp());

    // Simulate circuit breaker trip
    println!("\n1.3 Circuit Breaker Trip:");
    if metrics.failure_rate_bp() >= 1000 { // 10%
        metrics.record_trip();
        println!("   Circuit breaker tripped at {:.2}% failure rate",
            metrics.failure_rate_bp() as f64 / 100.0);
        println!("   Last trip timestamp: {} ns", metrics.last_trip_ns());
    }

    // Performance characteristics
    println!("\n1.4 Performance:");
    println!("   record_request():  <10ns (single atomic increment)");
    println!("   record_failure():  <10ns (single atomic increment)");
    println!("   record_trip():     <15ns (increment + timestamp store)");
    println!("   failure_rate_bp(): <20ns (two loads + division)");
    println!("   snapshot():        <30ns (four atomic loads)");

    println!("\n");
}

/// Section 2: RequestCapsule128Enhanced - Budget Metrics with Hash Integrity
fn request_capsule_metrics_demo() {
    println!("=== 2. RequestCapsule128Enhanced (Tier 6 Mixed) ===\n");

    let capsule = RequestCapsule128Enhanced::new(1000_00); // $1000.00

    // Perform operations
    println!("2.1 Recording Budget Operations:");
    let mut history = vec![capsule.metrics().expect("Initial metrics")];

    capsule.try_deduct(250_00).expect("Deduct $250");
    history.push(capsule.metrics().expect("After deduct 1"));
    println!("   Deducted: $250.00 (success)");

    capsule.try_deduct(150_00).expect("Deduct $150");
    history.push(capsule.metrics().expect("After deduct 2"));
    println!("   Deducted: $150.00 (success)");

    let _ = capsule.try_deduct(2000_00); // Will fail
    history.push(capsule.metrics().expect("After failed deduct"));
    println!("   Deducted: $2000.00 (failed - insufficient budget)");

    capsule.credit(500_00).expect("Credit $500");
    history.push(capsule.metrics().expect("After credit"));
    println!("   Credited: $500.00 (success)");

    // Query metrics
    println!("\n2.2 Query Metrics:");
    let metrics = capsule.metrics().expect("Final metrics");
    println!("   Budget:              ${:.2}", metrics.budget_cents as f64 / 100.0);
    println!("   Total Spent:         ${:.2}", metrics.total_spent as f64 / 100.0);
    println!("   Request Count:       {}", metrics.request_count);
    println!("   Successful Deducts:  {}", metrics.deduction_count);
    println!("   Failed Deducts:      {}", metrics.failed_deductions);
    println!("   Success Rate:        {:.2}% ({} bp)",
        capsule.success_rate_bp() as f64 / 100.0,
        capsule.success_rate_bp());
    println!("   Failure Rate:        {:.2}% ({} bp)",
        capsule.failure_rate_bp() as f64 / 100.0,
        capsule.failure_rate_bp());

    // Hash chain verification
    println!("\n2.3 Hash Chain Verification:");
    println!("   Current Hash:        0x{:016x}", metrics.hash);
    println!("   Previous Hash:       0x{:016x}", metrics.prev_hash);
    println!("   Integrity Verified:  {}", if metrics.integrity_verified { "✓ VALID" } else { "✗ VIOLATED" });

    let chain_result = capsule.verify_chain(&history);
    println!("   Chain Validation:    {}", if chain_result.is_valid { "✓ VALID" } else { "✗ VIOLATED" });
    println!("   Chain Length:        {} entries", history.len());
    println!("   Broken Links:        {}", chain_result.broken_links);

    // Audit trail export
    println!("\n2.4 Audit Trail Export:");
    let audit_trail = capsule.export_audit_trail(&history);
    println!("   Audit Entries:       {}", audit_trail.len());
    for (i, entry) in audit_trail.iter().take(3).enumerate() {
        println!("   [{}] {}: ${:.2} -> ${:.2}",
            i,
            entry.operation,
            entry.budget_before as f64 / 100.0,
            entry.budget_after as f64 / 100.0);
    }

    // Performance characteristics
    println!("\n2.5 Performance:");
    println!("   try_deduct():        <100ns (CAS loop + hash update)");
    println!("   credit():            <100ns (fetch_add + hash update)");
    println!("   metrics():           <150ns (6 loads + hash verify)");
    println!("   verify_integrity():  <100ns (6 loads + hash compute)");
    println!("   verify_chain():      <80ns per link (O(n) validation)");

    println!("\n");
}

/// Section 3: ResponseCapsule256 - Cost Tracking with Fixed-Point
fn response_capsule_metrics_demo() {
    println!("=== 3. ResponseCapsule256 (Tier 2+3: SIMD + Fixed-Point) ===\n");

    // Create response capsule for cost tracking
    println!("3.1 Recording Response Metrics:");
    let capsule = ResponseCapsule256::new();

    // Record multiple API responses
    capsule.record_response(500_000, 250, 150); // 500μs, 250 tokens, $1.50
    println!("   Response 1: 500μs, 250 tokens, $1.50");

    capsule.record_response(300_000, 180, 108); // 300μs, 180 tokens, $1.08
    println!("   Response 2: 300μs, 180 tokens, $1.08");

    capsule.record_response(700_000, 350, 210); // 700μs, 350 tokens, $2.10
    println!("   Response 3: 700μs, 350 tokens, $2.10");

    // Query aggregated metrics
    println!("\n3.2 Query Aggregated Metrics:");
    let metrics = capsule.load_metrics();
    println!("   Total Latency:       {}μs", metrics.latency_ns / 1000);
    println!("   Total Tokens:        {}", metrics.tokens);
    println!("   Total Cost:          ${:.2}", metrics.cost_f64);
    println!("   Generation:          {}", metrics.generation);

    // Calculate averages
    let response_count = 3;
    println!("\n3.3 Average Metrics:");
    println!("   Avg Latency:         {}μs", metrics.latency_ns / 1000 / response_count);
    println!("   Avg Tokens:          {}", metrics.tokens / response_count);
    println!("   Avg Cost:            ${:.2}", metrics.cost_f64 / response_count as f64);

    // Performance characteristics
    println!("\n3.4 Performance:");
    println!("   record_response():   <150ns (atomic updates + Q16.16 conversion)");
    println!("   load_metrics():      <50ns (4 atomic loads)");
    println!("   Q16.16 fixed-point:  Deterministic arithmetic (no FP drift)");

    println!("\n");
}

/// Section 4: EpochTile1024 - Time-Series Aggregation
fn epoch_tile_metrics_demo() {
    println!("=== 4. EpochTile1024 (Tier 4+3: Batch + Fixed-Point) ===\n");

    let now_ns = now_ns();
    let epoch_tile = EpochTile1024::new(now_ns);

    // Record metrics across multiple providers
    println!("4.1 Recording Per-Provider Metrics:");

    // Provider 0: OpenAI
    epoch_tile.record_request(0, 250_000, 500, 300, 0); // 250μs, 500 tokens, $3.00, success
    epoch_tile.record_request(0, 300_000, 450, 270, 0);
    epoch_tile.record_request(0, 280_000, 520, 312, 0);
    println!("   Provider 0 (OpenAI):   3 requests recorded");

    // Provider 1: Anthropic
    epoch_tile.record_request(1, 180_000, 600, 240, 0); // 180μs, 600 tokens, $2.40, success
    epoch_tile.record_request(1, 200_000, 550, 220, 0);
    println!("   Provider 1 (Anthropic): 2 requests recorded");

    // Provider 2: OpenRouter (with 1 error)
    epoch_tile.record_request(2, 400_000, 300, 150, 0);
    epoch_tile.record_request(2, 0, 0, 0, 1); // Error
    println!("   Provider 2 (OpenRouter): 2 requests (1 error)");

    // Query per-provider metrics
    println!("\n4.2 Query Per-Provider Metrics:");
    let snapshot = epoch_tile.snapshot();

    for i in 0..3 {
        let provider = &snapshot.providers[i];
        println!("\n   Provider {} Statistics:", i);
        println!("     Request Count:     {}", provider.request_count);
        println!("     Total Cost:        ${:.2}", provider.total_cost_cents as f64 / 100.0);
        println!("     Total Tokens:      {}", provider.total_tokens);
        println!("     Error Count:       {}", provider.error_count);

        if provider.request_count > 0 {
            let success_rate = ((provider.request_count - provider.error_count) as f64
                / provider.request_count as f64) * 100.0;
            println!("     Success Rate:      {:.1}%", success_rate);
        }
    }

    // Performance characteristics
    println!("\n4.3 Performance:");
    println!("   record_request():    <50ns per call (batch atomic updates)");
    println!("   snapshot():          <500ns (16-provider aggregation)");
    println!("   Per-provider tracking: O(1) access, zero contention");

    println!("\n");
}

/// Section 5: JSON Export Demo
fn json_export_demo() {
    println!("=== 5. JSON Export ===\n");

    let metrics = CircuitBreakerMetrics::new();

    // Record sample metrics
    for i in 1..=100 {
        metrics.record_request();
        if i % 10 == 0 {
            metrics.record_failure();
        }
    }

    // Simulate JSON export
    println!("5.1 CircuitBreakerMetrics JSON Format:");
    let snapshot = metrics.snapshot();
    let json = format!(
        r#"{{
  "circuit_breaker": {{
    "requests": {},
    "failures": {},
    "trips": {},
    "failure_rate_bp": {},
    "last_trip_ns": {}
  }}
}}"#,
        snapshot.requests,
        snapshot.failures,
        snapshot.trips,
        metrics.failure_rate_bp(),
        snapshot.last_trip_ns
    );
    println!("{}", json);

    // RequestCapsule128Enhanced JSON
    println!("\n5.2 RequestCapsule128Enhanced JSON Format:");
    let capsule = RequestCapsule128Enhanced::new(1000_00);
    capsule.try_deduct(250_00).unwrap();

    let metrics = capsule.metrics().unwrap();
    let json = format!(
        r#"{{
  "request_capsule": {{
    "budget_cents": {},
    "total_spent": {},
    "request_count": {},
    "generation": {},
    "deduction_count": {},
    "failed_deductions": {},
    "hash": "0x{:016x}",
    "prev_hash": "0x{:016x}",
    "integrity_verified": {}
  }}
}}"#,
        metrics.budget_cents,
        metrics.total_spent,
        metrics.request_count,
        metrics.generation,
        metrics.deduction_count,
        metrics.failed_deductions,
        metrics.hash,
        metrics.prev_hash,
        metrics.integrity_verified
    );
    println!("{}", json);

    println!("\n5.3 Export Options:");
    println!("   - JSON: Human-readable, universal compatibility");
    println!("   - CSV:  Excel-friendly, audit trail export");
    println!("   - Binary: Compact, efficient storage (future)");

    println!("\n");
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
