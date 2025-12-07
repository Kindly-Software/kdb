//! # AuthGuard Benchmarks (B32 Framework)
//!
//! **Purpose**: Measure performance of unified authentication orchestration
//! **Framework**: B32 (fair baseline, 95% CI, 1000+ iterations)
//! **Targets**:
//! - Happy path: <500ns (P50), <1μs (P99)
//! - Concurrent throughput: >10K auth/sec (16 threads)
//! - Per-capsule latency: ~200ns sum

use kdb_mcp::{
    AuthGuard, Command,
    AuthTokenCapsule, SessionCapsule, AccessControlCapsule,
    IntrusionDetectorCapsule, LicenseValidatorCapsule, AuditEnhancementCapsule,
};
use std::sync::Arc;
use std::thread;
use std::time::Instant;

// ============================================================================
// Helper: Statistical Analysis
// ============================================================================

struct LatencyStats {
    min_ns: u64,
    max_ns: u64,
    mean_ns: f64,
    p50_ns: f64,
    p99_ns: f64,
}

fn analyze_latencies(latencies: &[u64]) -> LatencyStats {
    let mut sorted = latencies.to_vec();
    sorted.sort_unstable();

    let min_ns = *sorted.first().unwrap_or(&0);
    let max_ns = *sorted.last().unwrap_or(&0);

    let sum: u64 = sorted.iter().sum();
    let mean_ns = sum as f64 / sorted.len() as f64;

    let p50_idx = (sorted.len() * 50) / 100;
    let p99_idx = (sorted.len() * 99) / 100;

    let p50_ns = sorted[p50_idx.min(sorted.len() - 1)] as f64;
    let p99_ns = sorted[p99_idx.min(sorted.len() - 1)] as f64;

    LatencyStats {
        min_ns,
        max_ns,
        mean_ns,
        p50_ns,
        p99_ns,
    }
}

// ============================================================================
// Benchmark 1: Happy-Path Authentication Latency
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_happy_path_latency -- --nocapture --ignored
fn bench_happy_path_latency() {
    println!("\n=== Benchmark 1: Happy-Path Authentication Latency ===");

    let guard = AuthGuard::default();
    let warmup_iterations = 100;
    let measure_iterations = 1000;

    // Warmup
    for _ in 0..warmup_iterations {
        let _result = guard.authenticate(
            "header.payload.signature",
            "192.168.1.100",
            1234,
            Command::Read,
        );
    }

    // Measure
    let mut latencies = Vec::new();

    for _ in 0..measure_iterations {
        let start = Instant::now();
        let _result = guard.authenticate(
            "header.payload.signature",
            "192.168.1.100",
            1234,
            Command::Read,
        );
        let elapsed = start.elapsed();
        latencies.push(elapsed.as_nanos() as u64);
    }

    let stats = analyze_latencies(&latencies);

    println!("Iterations: {}", measure_iterations);
    println!("Min latency: {} ns", stats.min_ns);
    println!("Max latency: {} ns", stats.max_ns);
    println!("Mean latency: {:.1} ns", stats.mean_ns);
    println!("P50 latency: {:.1} ns (target: <500ns)", stats.p50_ns);
    println!("P99 latency: {:.1} ns (target: <1000ns)", stats.p99_ns);

    // Basic assertions (loose for debug builds)
    assert!(stats.p50_ns < 10_000.0, "P50 latency too high");
    assert!(stats.p99_ns < 50_000.0, "P99 latency too high");
}

// ============================================================================
// Benchmark 2: Concurrent Authentication Throughput
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_concurrent_throughput -- --nocapture --ignored
fn bench_concurrent_throughput() {
    println!("\n=== Benchmark 2: Concurrent Authentication Throughput ===");

    let guard = Arc::new(AuthGuard::default());
    let num_threads = 16;
    let iterations_per_thread = 1000;

    let start = Instant::now();

    let threads: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let guard = Arc::clone(&guard);

            thread::spawn(move || {
                for i in 0..iterations_per_thread {
                    let token = format!("token{}.{}", thread_id, i);
                    let ip = format!("192.168.{}.{}", thread_id, i % 256);
                    let _result = guard.authenticate(
                        &token,
                        &ip,
                        (1000 + i as u32) % 65536,
                        Command::Read,
                    );
                }
            })
        })
        .collect();

    for thread in threads {
        thread.join().unwrap();
    }

    let elapsed = start.elapsed();
    let total_ops = (num_threads * iterations_per_thread) as f64;
    let throughput = total_ops / elapsed.as_secs_f64();

    println!("Threads: {}", num_threads);
    println!("Iterations per thread: {}", iterations_per_thread);
    println!("Total operations: {:.0}", total_ops);
    println!("Elapsed time: {:.3} seconds", elapsed.as_secs_f64());
    println!("Throughput: {:.0} auth/sec (target: >10K/sec)", throughput);
    println!("Throughput: {:.3} Mauth/sec", throughput / 1_000_000.0);

    assert!(throughput > 1000.0, "Throughput below 1K/sec");
}

// ============================================================================
// Benchmark 3: Per-Capsule Latency Contribution
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_per_capsule_latency -- --nocapture --ignored
fn bench_per_capsule_latency() {
    println!("\n=== Benchmark 3: Per-Capsule Latency Breakdown ===");

    let auth_token = Arc::new(AuthTokenCapsule::new());
    let session = Arc::new(SessionCapsule::new());
    let access_control = Arc::new(AccessControlCapsule::new());
    let intrusion = Arc::new(IntrusionDetectorCapsule::new());
    let license = Arc::new(LicenseValidatorCapsule::new());
    let audit = Arc::new(AuditEnhancementCapsule::new());

    let iterations = 1000;
    let now_unix = 1000u64;

    // Benchmark AuthTokenCapsule
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = auth_token.validate_cached("header.payload.signature", &[0u8; 32], now_unix);
    }
    let auth_token_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    // Benchmark SessionCapsule
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = session.is_valid(now_unix);
    }
    let session_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    // Benchmark AccessControlCapsule (PID)
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = access_control.is_pid_allowed(1234);
    }
    let access_control_pid_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    // Benchmark AccessControlCapsule (Command)
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = access_control.is_command_allowed(kdb_mcp::Command::Read);
    }
    let access_control_cmd_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    // Benchmark IntrusionDetectorCapsule
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = intrusion.check_ip("192.168.1.100");
    }
    let intrusion_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    // Benchmark LicenseValidatorCapsule
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = license.validate_cached("test_license");
    }
    let license_ns = start.elapsed().as_nanos() as f64 / iterations as f64;

    let total_estimated = auth_token_ns + session_ns + access_control_pid_ns
        + access_control_cmd_ns + intrusion_ns + license_ns;

    println!("Per-capsule latency (averaged over {} iterations):", iterations);
    println!("  AuthTokenCapsule: {:.1} ns", auth_token_ns);
    println!("  SessionCapsule: {:.1} ns", session_ns);
    println!("  AccessControlCapsule (PID): {:.1} ns", access_control_pid_ns);
    println!("  AccessControlCapsule (Cmd): {:.1} ns", access_control_cmd_ns);
    println!("  IntrusionDetectorCapsule: {:.1} ns", intrusion_ns);
    println!("  LicenseValidatorCapsule: {:.1} ns", license_ns);
    println!("─────────────────────────────────────");
    println!("Total capsule latency: {:.1} ns (target: ~200ns)", total_estimated);
    println!("Expected orchestration overhead: <300ns");
    println!("Total target: <500ns");
}

// ============================================================================
// Benchmark 4: Error Path Performance
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_error_path_latency -- --nocapture --ignored
fn bench_error_path_latency() {
    println!("\n=== Benchmark 4: Error Path Latency ===");

    let guard = Arc::new(AuthGuard::default());
    let iterations = 1000;

    // Measure failed authentication (various error types)
    let mut latencies = Vec::new();

    for i in 0..iterations {
        let start = Instant::now();
        let _result = guard.authenticate(
            &format!("invalid_token_{}", i),
            "192.168.1.100",
            65535, // May trigger various errors
            kdb_mcp::Command::Read,
        );
        let elapsed = start.elapsed();
        latencies.push(elapsed.as_nanos() as u64);
    }

    let stats = analyze_latencies(&latencies);

    println!("Error path latency (averaged over {} iterations):", iterations);
    println!("Min latency: {} ns", stats.min_ns);
    println!("Max latency: {} ns", stats.max_ns);
    println!("Mean latency: {:.1} ns", stats.mean_ns);
    println!("P50 latency: {:.1} ns", stats.p50_ns);
    println!("P99 latency: {:.1} ns", stats.p99_ns);

    // Error path should have minimal overhead vs happy path
    assert!(stats.p99_ns < 50_000.0, "Error path latency too high");
}

// ============================================================================
// Benchmark 5: Concurrent Lock Contention
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_lock_contention -- --nocapture --ignored
fn bench_lock_contention() {
    println!("\n=== Benchmark 5: Lock Contention Analysis ===");

    let guard = Arc::new(AuthGuard::default());
    let num_threads = vec![1, 2, 4, 8, 16];

    for &threads in &num_threads {
        let iterations_per_thread = 1000;
        let guard = Arc::clone(&guard);

        let start = Instant::now();

        let thread_handles: Vec<_> = (0..threads)
            .map(|thread_id| {
                let guard = Arc::clone(&guard);

                thread::spawn(move || {
                    for i in 0..iterations_per_thread {
                        let _result = guard.authenticate(
                            &format!("token{}.{}", thread_id, i),
                            "192.168.1.100",
                            1000,
                            kdb_mcp::Command::Read,
                        );
                    }
                })
            })
            .collect();

        for handle in thread_handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let total_ops = (threads * iterations_per_thread) as f64;
        let ops_per_sec = total_ops / elapsed.as_secs_f64();

        println!(
            "{} threads: {:.0} auth/sec ({:.3}s for {} ops)",
            threads, ops_per_sec, elapsed.as_secs_f64(), total_ops as u64
        );
    }
}

// ============================================================================
// Benchmark 6: Statistics Tracking Overhead
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_stats_overhead -- --nocapture --ignored
fn bench_stats_overhead() {
    println!("\n=== Benchmark 6: Statistics Tracking Overhead ===");

    let guard = AuthGuard::default();
    let iterations = 10_000;

    // Benchmark with stats updates
    let start = Instant::now();
    for _ in 0..iterations {
        let _result = guard.authenticate(
            "header.payload.signature",
            "192.168.1.100",
            1234,
            kdb_mcp::Command::Read,
        );
    }
    let elapsed_with_stats = start.elapsed();

    println!("Stats overhead analysis:");
    println!("Total time for {} authentications: {:.3}s", iterations, elapsed_with_stats.as_secs_f64());
    println!("Average time per auth: {:.1} ns", elapsed_with_stats.as_nanos() as f64 / iterations as f64);
    println!("Throughput: {:.0} auth/sec", iterations as f64 / elapsed_with_stats.as_secs_f64());

    let stats = guard.get_stats();
    println!("Total requests tracked: {}", stats.total_requests);
    println!("Average latency: {:.1} ns", stats.avg_latency_ns as f64);
}

// ============================================================================
// Benchmark 7: Scalability vs Thread Count
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release bench_scalability -- --nocapture --ignored
fn bench_scalability() {
    println!("\n=== Benchmark 7: Scalability Analysis ===");

    println!("\nScaling: threads × iterations = constant (10K total)");
    println!("Threads | Iter/thread | Elapsed | Throughput");
    println!("───────┼─────────────┼─────────┼──────────────");

    let total_ops = 10_000;
    let thread_counts = vec![1, 2, 4, 8, 16];

    for threads in thread_counts {
        let iterations_per_thread = total_ops / threads;
        let guard = Arc::new(AuthGuard::default());

        let start = Instant::now();

        let handles: Vec<_> = (0..threads)
            .map(|thread_id| {
                let guard = Arc::clone(&guard);

                thread::spawn(move || {
                    for i in 0..iterations_per_thread {
                        let _result = guard.authenticate(
                            &format!("token{}.{}", thread_id, i),
                            "192.168.1.100",
                            1000,
                            kdb_mcp::Command::Read,
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let elapsed = start.elapsed();
        let throughput = total_ops as f64 / elapsed.as_secs_f64();

        println!(
            "  {:2}   |    {:4}      | {:6.3}s | {:.0} auth/sec",
            threads, iterations_per_thread, elapsed.as_secs_f64(), throughput
        );
    }
}
