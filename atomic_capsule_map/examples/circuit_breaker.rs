//! Circuit breaker integration example.
//!
//! Run with: cargo run --example circuit_breaker

use atomic_capsule_map::{AtomicCapsuleMap, BreakerLevel};

fn main() {
    println!("=== Circuit Breaker Integration ===\n");

    let map = AtomicCapsuleMap::new();

    // Check initial health
    println!("1. Initial Health Status");
    let health = map.health_status();
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Total ops: {}", health.total_ops);
    println!("   Failed ops: {}", health.failed_ops);
    println!("   Error rate: {} bp\n", health.error_rate_bp);

    // Normal operations
    println!("2. Normal Operations (L0)");
    for i in 0..10 {
        map.insert(format!("key:{}", i), i);
    }
    println!("   Inserted 10 items");
    println!(
        "   Breaker level: {:?}\n",
        map.health_status().breaker_level
    );

    // Simulate elevated risk (L1)
    println!("3. Elevated Risk (L1)");
    map.set_breaker_level(BreakerLevel::L1);
    let health = map.health_status();
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Action: Reduce operation rate, increase monitoring\n");

    // Continue operations with caution
    for i in 10..15 {
        map.insert(format!("key:{}", i), i);
    }
    println!("   Continued with reduced load (5 items)");

    // High risk (L2)
    println!("\n4. High Risk (L2)");
    map.set_breaker_level(BreakerLevel::L2);
    let health = map.health_status();
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Action: Emergency mode, minimal operations only\n");

    // Critical - circuit open (L3)
    println!("5. Critical - Circuit Open (L3)");
    map.set_breaker_level(BreakerLevel::L3);
    let health = map.health_status();
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Action: Reject new operations, initiate recovery\n");

    // Recovery - back to normal
    println!("6. Recovery - Back to Normal");
    map.set_breaker_level(BreakerLevel::L0);
    let health = map.health_status();
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Action: Resume normal operations\n");

    // Final statistics
    println!("7. Final Statistics");
    let health = map.health_status();
    println!("   Total entries: {}", map.len());
    println!("   Breaker level: {:?}", health.breaker_level);
    println!("   Total ops: {}", health.total_ops);
    println!("   Error rate: {} bp\n", health.error_rate_bp);

    println!("=== Circuit Breaker Levels ===\n");

    println!("L0 (Normal):");
    println!("  - Full operation rate");
    println!("  - All features enabled");
    println!("  - Standard monitoring\n");

    println!("L1 (Elevated Caution):");
    println!("  - Reduce operation rate");
    println!("  - Increase health checks");
    println!("  - Enhanced logging\n");

    println!("L2 (High Risk):");
    println!("  - Emergency operations only");
    println!("  - Prepare for degradation");
    println!("  - Alert operators\n");

    println!("L3 (Critical - Circuit Open):");
    println!("  - Reject new operations");
    println!("  - Initiate recovery procedures");
    println!("  - System stabilization mode\n");

    println!("=== Integration Patterns ===\n");

    println!("1. Health-Based Rate Limiting:");
    println!("```rust");
    println!("let health = map.health_status();");
    println!("match health.breaker_level {{");
    println!("    BreakerLevel::L0 => /* full rate */,");
    println!("    BreakerLevel::L1 => /* 50% rate */,");
    println!("    BreakerLevel::L2 => /* 10% rate */,");
    println!("    BreakerLevel::L3 => /* reject */,");
    println!("}}");
    println!("```\n");

    println!("2. Automatic Degradation:");
    println!("```rust");
    println!("if map.health_status().breaker_level >= BreakerLevel::L2 {{");
    println!("    // Switch to fallback storage");
    println!("    // Reduce feature set");
    println!("}}");
    println!("```\n");

    println!("3. Monitoring Integration:");
    println!("```rust");
    println!("loop {{");
    println!("    let health = map.health_status();");
    println!("    metrics.record(\"error_rate_bp\", health.error_rate_bp);");
    println!("    metrics.record(\"breaker_level\", health.breaker_level as u8);");
    println!("}}");
    println!("```\n");

    println!("=== Circuit Breaker Complete ===");
}
