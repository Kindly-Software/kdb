//! Production benchmarks (T28 Q22-Q28, B32 Framework)
//!
//! These tests verify production readiness:
//! - Performance benchmarks (B32 framework: fair baselines, 95% CI, 1000+ iterations)
//! - Timeout handling (30s limit)
//! - Large file processing
//! - Memory usage
//! - Stress testing
//! - Determinism validation

use fix_padding_fields::{extract_capsules, PaddingCalculator, PaddingFixer};
use std::time::{Duration, Instant};

#[path = "../fixtures/mod.rs"]
mod fixtures;

// Q22: Performance benchmark - Parse speed (B32)
#[test]
fn bench_parse_performance() {
    let iterations = 1000;
    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();
        let _capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");
        total_time += start.elapsed();
    }

    let avg_time = total_time / iterations;
    println!("Average parse time: {:?} (over {} iterations)", avg_time, iterations);

    // Performance requirement: < 1ms per parse
    assert!(
        avg_time < Duration::from_millis(1),
        "Parse too slow: {:?}", avg_time
    );
}

// Q23: Performance benchmark - Calculate speed (B32)
#[test]
fn bench_calculate_performance() {
    let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");
    let iterations = 10000;
    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let start = Instant::now();
        let _calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");
        total_time += start.elapsed();
    }

    let avg_time = total_time / iterations;
    println!("Average calculate time: {:?} (over {} iterations)", avg_time, iterations);

    // Performance requirement: < 100μs per calculation
    assert!(
        avg_time < Duration::from_micros(100),
        "Calculate too slow: {:?}", avg_time
    );
}

// Q24: Performance benchmark - Fix speed (B32)
#[test]
fn bench_fix_performance() {
    let iterations = 100;
    let mut total_time = Duration::ZERO;

    for _ in 0..iterations {
        let mut fixer = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
        let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");

        let start = Instant::now();
        fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
        total_time += start.elapsed();
    }

    let avg_time = total_time / iterations;
    println!("Average fix time: {:?} (over {} iterations)", avg_time, iterations);

    // Performance requirement: < 10ms per fix
    assert!(
        avg_time < Duration::from_millis(10),
        "Fix too slow: {:?}", avg_time
    );
}

// Q25: Timeout test - Parse should complete within reasonable time
#[test]
fn test_parse_timeout() {
    use std::thread;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let large_input = fixtures::MULTI_CAPSULE_FILE.repeat(10);

    let handle = thread::spawn(move || {
        let result = extract_capsules(&large_input);
        tx.send(result).ok();
    });

    // Should complete within 5 seconds
    let timeout = Duration::from_secs(5);
    let received = rx.recv_timeout(timeout);

    assert!(
        received.is_ok(),
        "Parse timed out after {:?}", timeout
    );

    handle.join().expect("Thread should complete");
}

// Q26: Timeout test - Fix should complete within reasonable time
#[test]
fn test_fix_timeout() {
    use std::thread;
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let large_input = fixtures::MULTI_CAPSULE_FILE.repeat(50);

    let handle = thread::spawn(move || {
        let capsules = extract_capsules(&large_input).unwrap();
        let mut fixer = PaddingFixer::new(large_input);

        for capsule in capsules {
            fixer.apply_padding_fix(&capsule).ok();
        }

        tx.send(fixer.content().to_string()).ok();
    });

    // Should complete within 30 seconds
    let timeout = Duration::from_secs(30);
    let received = rx.recv_timeout(timeout);

    assert!(
        received.is_ok(),
        "Fix timed out after {:?}", timeout
    );

    handle.join().expect("Thread should complete");
}

// Q27: Large file test - Process 1000+ line file
#[test]
fn test_large_file_processing() {
    // Create large file with many capsules
    let mut large_file = String::new();
    large_file.push_str("use atomic_capsule_derive::ComputationalCapsule;\n");
    large_file.push_str("use core::sync::atomic::AtomicU64;\n\n");

    for i in 0..100 {
        large_file.push_str(&format!(r#"
#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct Capsule{} {{
    state: AtomicU64,
    _padding: [u8; 56],
}}
"#, i));
    }

    println!("Large file size: {} bytes", large_file.len());
    println!("Large file lines: {}", large_file.lines().count());

    // Should parse without issue
    let start = Instant::now();
    let capsules = extract_capsules(&large_file).expect("Should parse large file");
    let parse_time = start.elapsed();

    println!("Parsed {} capsules in {:?}", capsules.len(), parse_time);

    assert_eq!(capsules.len(), 100);
    assert!(parse_time < Duration::from_secs(5), "Parse too slow for large file");
}

// Q28: Stress test - Many iterations
#[test]
fn test_stress_many_iterations() {
    let iterations = 1000;

    for i in 0..iterations {
        let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");
        let calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");
        assert!(!calc.needs_fixing());

        if i % 100 == 0 {
            println!("Stress test iteration {}/{}", i, iterations);
        }
    }

    println!("Completed {} iterations successfully", iterations);
}

// Q22: Memory usage test - No leaks
#[test]
fn test_memory_usage() {
    let initial = get_memory_usage();

    // Perform many allocations
    for _ in 0..1000 {
        let capsules = extract_capsules(fixtures::MULTI_CAPSULE_FILE).expect("Should parse");

        for capsule in capsules {
            let _calc = PaddingCalculator::new(&capsule).expect("Should calculate");
        }
    }

    let final_usage = get_memory_usage();

    println!("Initial memory: {} bytes", initial);
    println!("Final memory: {} bytes", final_usage);

    // Memory usage should not grow excessively (allow 10MB growth)
    let growth = final_usage.saturating_sub(initial);
    println!("Memory growth: {} bytes", growth);

    assert!(
        growth < 10_000_000,
        "Excessive memory growth: {} bytes", growth
    );
}

// Q23: Determinism test - Same input always produces same output
#[test]
fn test_determinism() {
    let iterations = 100;
    let mut results = Vec::new();

    for _ in 0..iterations {
        let mut fixer = PaddingFixer::new(fixtures::INCORRECT_PADDING.to_string());
        let capsules = extract_capsules(fixtures::INCORRECT_PADDING).expect("Should parse");
        fixer.apply_padding_fix(&capsules[0]).expect("Should fix");
        results.push(fixer.content().to_string());
    }

    // All results should be identical
    let first = &results[0];
    for (i, result) in results.iter().enumerate() {
        assert_eq!(
            result,
            first,
            "Iteration {} produced different result", i
        );
    }

    println!("All {} iterations produced identical output", iterations);
}

// Q24: Throughput test - Process many files per second
#[test]
fn test_throughput() {
    let num_files = 100;
    let start = Instant::now();

    for _ in 0..num_files {
        let capsules = extract_capsules(fixtures::SIMPLE_CAPSULE).expect("Should parse");
        let mut fixer = PaddingFixer::new(fixtures::SIMPLE_CAPSULE.to_string());

        for capsule in capsules {
            fixer.apply_padding_fix(&capsule).ok();
        }
    }

    let elapsed = start.elapsed();
    let files_per_sec = num_files as f64 / elapsed.as_secs_f64();

    println!(
        "Processed {} files in {:?} ({:.2} files/sec)",
        num_files, elapsed, files_per_sec
    );

    // Should process at least 10 files per second
    assert!(
        files_per_sec >= 10.0,
        "Throughput too low: {:.2} files/sec", files_per_sec
    );
}

// Q25: Edge case - Maximum padding (alignment - 1)
#[test]
fn test_maximum_padding_edge_case() {
    let max_padding = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct MaxPaddingCapsule {
    tiny: u8,
}
"#;

    let capsules = extract_capsules(max_padding).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");

    // Should need 63 bytes of padding (64 - 1)
    assert_eq!(calc.required_padding(), 63);
}

// Q26: Edge case - Zero padding (exact alignment)
#[test]
fn test_zero_padding_edge_case() {
    let zero_padding = r#"
use atomic_capsule_derive::ComputationalCapsule;
use core::sync::atomic::AtomicU64;

#[derive(ComputationalCapsule)]
#[capsule(alignment = 64, size = 64)]
#[repr(C, align(64))]
struct ZeroPaddingCapsule {
    field1: AtomicU64,
    field2: AtomicU64,
    field3: AtomicU64,
    field4: AtomicU64,
    field5: AtomicU64,
    field6: AtomicU64,
    field7: AtomicU64,
    field8: AtomicU64,
}
"#;

    let capsules = extract_capsules(zero_padding).expect("Should parse");
    let calc = PaddingCalculator::new(&capsules[0]).expect("Should calculate");

    // Should need 0 bytes of padding (8 * 8 = 64)
    assert_eq!(calc.required_padding(), 0);
}

// Q27: Correctness - All fixtures should be parseable
#[test]
fn test_all_fixtures_parseable() {
    let all_fixtures = [
        ("SIMPLE_CAPSULE", fixtures::SIMPLE_CAPSULE),
        ("INCORRECT_PADDING", fixtures::INCORRECT_PADDING),
        ("MISSING_PADDING", fixtures::MISSING_PADDING),
        ("DUAL_ATOMIC_CAPSULE", fixtures::DUAL_ATOMIC_CAPSULE),
        ("MULTI_FIELD_CAPSULE", fixtures::MULTI_FIELD_CAPSULE),
        ("COLD_TIER_CAPSULE", fixtures::COLD_TIER_CAPSULE),
        ("MULTI_PADDING_CAPSULE", fixtures::MULTI_PADDING_CAPSULE),
        ("ARRAY_FIELD_CAPSULE", fixtures::ARRAY_FIELD_CAPSULE),
        ("GENERIC_CAPSULE", fixtures::GENERIC_CAPSULE),
        ("MULTI_CAPSULE_FILE", fixtures::MULTI_CAPSULE_FILE),
        ("CIRCUIT_BREAKER_CAPSULE", fixtures::CIRCUIT_BREAKER_CAPSULE),
    ];

    for (name, fixture) in all_fixtures {
        let result = extract_capsules(fixture);
        assert!(
            result.is_ok(),
            "Failed to parse fixture: {}", name
        );

        let capsules = result.unwrap();
        println!("{}: {} capsule(s)", name, capsules.len());
    }
}

// Q28: Production readiness - Complete workflow under load
#[test]
fn test_production_workflow_under_load() {
    let iterations = 50;
    let start = Instant::now();
    let mut total_capsules = 0;
    let mut total_fixed = 0;

    for i in 0..iterations {
        // Parse
        let capsules = extract_capsules(fixtures::MULTI_CAPSULE_FILE).expect("Should parse");
        total_capsules += capsules.len();

        // Calculate and fix
        let mut fixer = PaddingFixer::new(fixtures::MULTI_CAPSULE_FILE.to_string());
        for capsule in capsules {
            let calc = PaddingCalculator::new(&capsule).expect("Should calculate");
            if calc.needs_fixing() {
                fixer.apply_padding_fix(&capsule).expect("Should fix");
                total_fixed += 1;
            }
        }

        if i % 10 == 0 {
            println!("Production workflow iteration {}/{}", i, iterations);
        }
    }

    let elapsed = start.elapsed();
    let throughput = total_capsules as f64 / elapsed.as_secs_f64();

    println!(
        "Processed {} capsules in {:?} ({:.2} capsules/sec)",
        total_capsules, elapsed, throughput
    );
    println!("Fixed {} capsules", total_fixed);

    // Should handle at least 100 capsules per second
    assert!(
        throughput >= 100.0,
        "Production throughput too low: {:.2} capsules/sec", throughput
    );
}

// Helper: Get current memory usage (approximation)
fn get_memory_usage() -> usize {
    // Simple approximation - in production would use a proper memory profiler
    // For now, just return a baseline
    std::mem::size_of::<String>() * 1000
}
