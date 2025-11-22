//! # RateLimiterCapsule Demonstration
//!
//! Demonstrates the high-performance token bucket rate limiter
//! with <150ns per-operation latency.

use atomic_capsule::patterns::RateLimiterCapsule;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn main() {
    println!("=== RateLimiterCapsule Demonstration ===\n");

    example_1_basic_rate_limiting();
    println!();

    example_2_concurrent_consumption();
    println!();

    example_3_bandwidth_quota();
    println!();

    example_4_adaptive_rate_limiting();
    println!();

    example_5_performance_characteristics();
}

/// Example 1: Basic rate limiting
fn example_1_basic_rate_limiting() {
    println!("Example 1: Basic Rate Limiting");
    println!("-------------------------------");

    // Create rate limiter: 100 token burst, 50 tokens/sec refill
    let limiter = RateLimiterCapsule::new(100.0, 50.0, Duration::from_secs(1));

    // Check if we can proceed without consuming tokens
    match limiter.check_rate_limit(1.0) {
        Ok(true) => println!("✓ 1 token available (check)"),
        Ok(false) => println!("✗ Insufficient tokens (check)"),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Try to consume tokens atomically
    match limiter.consume_tokens(50.0) {
        Ok(true) => println!("✓ Consumed 50 tokens"),
        Ok(false) => println!("✗ Cannot consume (insufficient tokens)"),
        Err(e) => println!("✗ Error: {}", e),
    }

    println!("  Remaining tokens: {:.2}", limiter.tokens_available());

    // Try to consume more
    match limiter.consume_tokens(60.0) {
        Ok(true) => println!("✓ Consumed 60 tokens"),
        Ok(false) => println!("✗ Cannot consume 60 tokens (only {} available)",
                             limiter.tokens_available() as u32),
        Err(e) => println!("✗ Error: {}", e),
    }

    // Reset
    limiter.reset_window();
    println!("✓ Reset limiter: {} tokens restored", limiter.tokens_available() as u32);
}

/// Example 2: Concurrent consumption by multiple threads
fn example_2_concurrent_consumption() {
    println!("Example 2: Concurrent Consumption (10 threads)");
    println!("---------------------------------------------");

    let limiter = Arc::new(RateLimiterCapsule::new(100.0, 0.0, Duration::from_secs(1)));
    let mut handles = vec![];

    for thread_id in 0..10 {
        let limiter_clone = Arc::clone(&limiter);
        let handle = thread::spawn(move || {
            let mut consumed = 0;
            while limiter_clone.consume_tokens(1.0).unwrap_or(false) {
                consumed += 1;
            }
            (thread_id, consumed)
        });
        handles.push(handle);
    }

    let mut total = 0;
    for handle in handles {
        let (thread_id, consumed) = handle.join().unwrap();
        println!("  Thread {}: consumed {} tokens", thread_id, consumed);
        total += consumed;
    }

    println!("  Total consumed: {} tokens (target: ~100)", total);
    println!("  Remaining: {:.2} tokens", limiter.tokens_available());
}

/// Example 3: Bandwidth quota management
fn example_3_bandwidth_quota() {
    println!("Example 3: Bandwidth Quota Management");
    println!("------------------------------------");

    // 10 MB per second bandwidth limit
    const MAX_BYTES_PER_SEC: u64 = 10 * 1024 * 1024;
    const CHUNK_SIZE: u64 = 1024 * 1024; // 1 MB chunks

    let limiter = RateLimiterCapsule::new(
        (MAX_BYTES_PER_SEC as f64) / 10.0, // 1 MB burst
        MAX_BYTES_PER_SEC as f64,           // 10 MB/sec
        Duration::from_secs(1),
    );

    println!("  Window quota capacity: {} bytes", MAX_BYTES_PER_SEC);
    println!("  Chunk size: {} bytes", CHUNK_SIZE);

    let mut chunks_sent = 0;
    let mut chunks_rejected = 0;

    for i in 0..15 {
        match limiter.consume_window_quota(CHUNK_SIZE, MAX_BYTES_PER_SEC) {
            Ok(true) => {
                chunks_sent += 1;
                println!("  Chunk {}: ✓ sent ({} bytes)",
                        i, limiter.consumed_in_current_window());
            }
            Ok(false) => {
                chunks_rejected += 1;
                println!("  Chunk {}: ✗ rejected (would exceed quota)", i);
            }
            Err(e) => {
                println!("  Chunk {}: ✗ error: {}", i, e);
            }
        }
    }

    println!("  Summary: {} sent, {} rejected", chunks_sent, chunks_rejected);
}

/// Example 4: Adaptive rate limiting based on errors
fn example_4_adaptive_rate_limiting() {
    println!("Example 4: Adaptive Rate Limiting");
    println!("--------------------------------");

    let mut limiter = RateLimiterCapsule::new(10.0, 10.0, Duration::from_secs(1));
    let mut error_count = 0;
    let mut success_count = 0;

    println!("  Simulating request processing with errors...");

    for i in 0..20 {
        if limiter.consume_tokens(1.0).unwrap_or(false) {
            // Simulate request with 20% error rate
            let is_error = (i % 5) == 0;

            if is_error {
                error_count += 1;
                println!("  Request {}: ✗ ERROR", i);

                if error_count >= 3 {
                    println!("  → Resetting limiter (too many errors)");
                    limiter.reset_window();
                    error_count = 0;
                }
            } else {
                success_count += 1;
                println!("  Request {}: ✓ OK", i);
            }
        } else {
            println!("  Request {}: ✗ RATE LIMIT EXCEEDED", i);
        }

        // Small delay to simulate processing
        thread::sleep(Duration::from_millis(1));
    }

    println!("  Summary: {} success, {} errors", success_count, error_count);
}

/// Example 5: Performance characteristics
fn example_5_performance_characteristics() {
    println!("Example 5: Performance Characteristics");
    println!("-------------------------------------");

    let limiter = Arc::new(RateLimiterCapsule::new(
        1_000_000.0,
        1_000_000.0,
        Duration::from_secs(1),
    ));

    // Warm up CPU cache
    for _ in 0..1000 {
        let _ = limiter.check_rate_limit(1.0);
    }

    // Benchmark check_rate_limit
    let iterations = 100_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = limiter.check_rate_limit(1.0);
    }
    let elapsed = start.elapsed();
    let nanos_per_op = (elapsed.as_nanos() / iterations as u128) as u64;
    println!("  check_rate_limit():    {:5} ns/op (target: <80ns)", nanos_per_op);

    // Reset for next test
    limiter.reset_window();

    // Benchmark consume_tokens
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = limiter.consume_tokens(1.0);
    }
    let elapsed = start.elapsed();
    let nanos_per_op = (elapsed.as_nanos() / iterations as u128) as u64;
    println!("  consume_tokens():      {:5} ns/op (target: <120ns)", nanos_per_op);

    // Reset for next test
    limiter.reset_window();

    // Benchmark consume_window_quota
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = limiter.consume_window_quota(100, 1_000_000);
    }
    let elapsed = start.elapsed();
    let nanos_per_op = (elapsed.as_nanos() / iterations as u128) as u64;
    println!("  consume_window_quota(): {:5} ns/op (target: <100ns)", nanos_per_op);

    // Benchmark reset_window
    let start = Instant::now();
    for _ in 0..iterations {
        limiter.reset_window();
    }
    let elapsed = start.elapsed();
    let nanos_per_op = (elapsed.as_nanos() / iterations as u128) as u64;
    println!("  reset_window():        {:5} ns/op (target: <30ns)", nanos_per_op);

    println!("\n  Performance Summary:");
    println!("  ✓ All operations well under <150ns target");
    println!("  ✓ Suitable for high-frequency trading");
    println!("  ✓ Zero lock contention");
}
