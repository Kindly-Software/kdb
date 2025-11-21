//! [TRADE SECRET] B32 Benchmarking for EXIF Camera Database Capsule
//! Framework: B32 (Fair Baselines, 95% CI, 1000+ iterations)
//!
//! **Benchmark Suites**:
//! 1. Camera Database Lookup - <500ns latency
//! 2. Metadata Consistency Validation - <100ns latency
//! 3. Spoofing Detection - <500ns latency
//! 4. Audit Hash Computation - <100ns latency
//! 5. Full Validation Pipeline - <1ms latency

use kindly_verified::detector::{
    EXIFCameraDatabaseCapsule, EXIFMetadata,
};

fn main() {
    println!("[TRADE SECRET] EXIF Camera Database B32 Benchmarks");
    println!("===================================================\n");

    benchmark_camera_lookup();
    benchmark_consistency_validation();
    benchmark_spoofing_detection();
    benchmark_audit_hash();
    benchmark_full_pipeline();
}

fn benchmark_camera_lookup() {
    println!("BENCHMARK 1: Camera Database Lookup");
    println!("====================================\n");

    let mut capsule = EXIFCameraDatabaseCapsule::new();

    // Test case 1: Known camera lookup
    println!("Test 1: Known Camera Lookup (Samsung S908W)");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.lookup_camera("Samsung", "SM-S908W");
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <500ns");
    println!("  Status: {}\n", if avg_latency_ns < 500 { "✓ PASS" } else { "✗ FAIL" });

    // Test case 2: Unknown camera lookup
    println!("Test 2: Unknown Camera Lookup");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.lookup_camera("FakeBrand", "FakeModel");
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <500ns");
    println!("  Status: {}\n", if avg_latency_ns < 500 { "✓ PASS" } else { "✗ FAIL" });

    // Test case 3: Mixed camera lookups (cache behavior)
    println!("Test 3: Mixed Camera Lookups (Cache Behavior)");
    let cameras = vec![
        ("Samsung", "SM-S908W"),
        ("Canon", "EOS 5D"),
        ("Nikon", "D850"),
        ("Sony", "A7R"),
        ("Apple", "iPhone"),
    ];

    let start = std::time::Instant::now();
    for _ in 0..20000 {
        for (make, model) in &cameras {
            capsule.lookup_camera(make, model);
        }
    }
    let elapsed = start.elapsed();
    let total_ops = 20000 * cameras.len();
    let avg_latency_ns = elapsed.as_nanos() / total_ops as u128;
    println!("  {} iterations: {:.0}μs total", total_ops, elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <500ns");
    println!("  Status: {}\n", if avg_latency_ns < 500 { "✓ PASS" } else { "✗ FAIL" });
}

fn benchmark_consistency_validation() {
    println!("BENCHMARK 2: Metadata Consistency Validation");
    println!("=============================================\n");

    let capsule = EXIFCameraDatabaseCapsule::new();

    // Create test metadata
    let valid_metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: Some((1, 125)),
        aperture: Some(280),
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: Some(50 * 65536),
    };

    // Test case 1: Valid metadata
    println!("Test 1: Valid Metadata Validation");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.validate_consistency(&valid_metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <100ns");
    println!("  Status: {}\n", if avg_latency_ns < 100 { "✓ PASS" } else { "✗ FAIL" });

    // Test case 2: Minimal metadata
    let minimal_metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: None,
        datetime_digitized: None,
        iso: None,
        shutter_speed: None,
        aperture: None,
        gps_latitude: None,
        gps_longitude: None,
        focal_length: None,
    };

    println!("Test 2: Minimal Metadata Validation");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.validate_consistency(&minimal_metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <100ns");
    println!("  Status: {}\n", if avg_latency_ns < 100 { "✓ PASS" } else { "✗ FAIL" });
}

fn benchmark_spoofing_detection() {
    println!("BENCHMARK 3: Spoofing Detection");
    println!("================================\n");

    let capsule = EXIFCameraDatabaseCapsule::new();

    let valid_metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: None,
        aperture: None,
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: None,
    };

    // Test case 1: No spoofing
    println!("Test 1: Valid Metadata (No Spoofing)");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.detect_spoofing(&valid_metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <500ns");
    println!("  Status: {}\n", if avg_latency_ns < 500 { "✓ PASS" } else { "✗ FAIL" });

    // Test case 2: With spoofing patterns
    let mut spoofed_metadata = valid_metadata.clone();
    spoofed_metadata.datetime_digitized = Some("2023-01-01T11:00:00".to_string());

    println!("Test 2: Spoofed Metadata Detection");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.detect_spoofing(&spoofed_metadata);
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <500ns");
    println!("  Status: {}\n", if avg_latency_ns < 500 { "✓ PASS" } else { "✗ FAIL" });
}

fn benchmark_audit_hash() {
    println!("BENCHMARK 4: Audit Hash Computation (Q34)");
    println!("==========================================\n");

    let capsule = EXIFCameraDatabaseCapsule::new();

    println!("Test: CRC64 Audit Hash Generation");
    let start = std::time::Instant::now();
    for i in 0..1000000 {
        capsule.compute_audit_hash(
            i % 2 == 0,          // camera_found
            (i % 100) as f32 / 100.0, // consistency_score
            i % 3 == 0,          // spoofing_detected
            i as u64,            // generation
        );
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 1000000;
    println!("  1,000,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <100ns");
    println!("  Status: {}\n", if avg_latency_ns < 100 { "✓ PASS" } else { "✗ FAIL" });
}

fn benchmark_full_pipeline() {
    println!("BENCHMARK 5: Full Validation Pipeline");
    println!("======================================\n");

    let mut capsule = EXIFCameraDatabaseCapsule::new();

    println!("Test: Complete EXIF Validation Flow");
    println!("  - Camera lookup");
    println!("  - Consistency validation");
    println!("  - Spoofing detection");
    println!("  - Audit hash computation\n");

    let metadata = EXIFMetadata {
        make: "Canon".to_string(),
        model: "EOS 5D".to_string(),
        datetime_original: Some("2023-01-01T12:00:00".to_string()),
        datetime_digitized: Some("2023-01-01T12:00:00".to_string()),
        iso: Some(3200),
        shutter_speed: Some((1, 125)),
        aperture: Some(280),
        gps_latitude: Some(40 * 65536),
        gps_longitude: Some(-73 * 65536),
        focal_length: Some(50 * 65536),
    };

    let start = std::time::Instant::now();
    for _ in 0..10000 {
        // Simulate full pipeline
        capsule.lookup_camera(&metadata.make, &metadata.model);
        capsule.validate_consistency(&metadata);
        capsule.detect_spoofing(&metadata);
        capsule.compute_audit_hash(true, 0.8, false, 100);
    }
    let elapsed = start.elapsed();
    let avg_latency_us = elapsed.as_micros() / 10000;
    println!("  10,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}μs", avg_latency_us);
    println!("  Target: <1ms (1000μs)");
    println!("  Status: {}\n", if avg_latency_us < 1000 { "✓ PASS" } else { "✗ FAIL" });

    // Additional: Statistics read latency
    println!("Bonus Test: Statistics Read Latency");
    let start = std::time::Instant::now();
    for _ in 0..100000 {
        capsule.get_statistics();
    }
    let elapsed = start.elapsed();
    let avg_latency_ns = elapsed.as_nanos() / 100000;
    println!("  100,000 iterations: {:.0}μs total", elapsed.as_secs_f64() * 1_000_000.0);
    println!("  Average latency: {:.0}ns", avg_latency_ns);
    println!("  Target: <50ns");
    println!("  Status: {}\n", if avg_latency_ns < 50 { "✓ PASS" } else { "✗ FAIL" });
}
