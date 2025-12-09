//! T28 Tests for MetricsCapsule - Progress Tracking and Metrics Collection
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! **Test Coverage**: Q1-Q21 (Unit + Property + Integration)
//! - Q1-Q7: Unit tests (basic functionality)
//! - Q8-Q14: Property tests (EWMA convergence, metric bounds)
//! - Q15-Q21: Integration tests (encoder simulation, TUI integration)
//!
//! **Performance Validation**: B32 Framework
//! - <100ns update_frame() target
//! - <200ns snapshot() target
//! - <50ns ETA calculation target

use kindly_av1::progress::{MetricsCapsule, MetricsSnapshot};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// ============================================================================
// Q1-Q7: Unit Tests
// ============================================================================

#[test]
fn test_q1_capsule_size_and_alignment() {
    // Q1: Verify 256B size and alignment (Chaos compliance)
    assert_eq!(std::mem::size_of::<MetricsCapsule>(), 256);
    assert_eq!(std::mem::align_of::<MetricsCapsule>(), 256);
}

#[test]
fn test_q2_new_capsule_zeroed() {
    // Q2: Verify initialization is correct
    let capsule = MetricsCapsule::new();
    let snap = capsule.snapshot();

    assert_eq!(snap.frames_encoded, 0);
    assert_eq!(snap.frames_total, 0);
    assert_eq!(snap.bytes_written, 0);
    assert_eq!(snap.input_bytes, 0);
    assert_eq!(snap.gpu_utilization, 0);
    assert!((snap.current_fps - 0.0).abs() < 0.001);
    assert!((snap.current_psnr - 0.0).abs() < 0.001);
    assert!((snap.current_ssim - 0.0).abs() < 0.001);
}

#[test]
fn test_q3_init_sets_values() {
    // Q3: Verify init() sets total frames and input size
    let capsule = MetricsCapsule::new();
    capsule.init(1440, 100_000_000); // 24fps × 60s, 100MB

    let snap = capsule.snapshot();
    assert_eq!(snap.frames_total, 1440);
    assert_eq!(snap.input_bytes, 100_000_000);
    assert_eq!(snap.frames_encoded, 0);
    assert!(snap.elapsed_ms >= 0); // Should have started timing
}

#[test]
fn test_q4_update_frame_basic() {
    // Q4: Verify basic frame update
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    capsule.update_frame(
        16_666_666, // 16.67ms (60fps)
        42.5,       // PSNR: 42.5 dB
        0.98,       // SSIM: 0.98
        87,         // GPU: 87%
    );

    let snap = capsule.snapshot();
    assert_eq!(snap.frames_encoded, 1);
    assert_eq!(snap.gpu_utilization, 87);
    assert!((snap.current_psnr - 42.5).abs() < 0.1);
    assert!((snap.current_ssim - 0.98).abs() < 0.01);
    assert!(snap.current_fps > 50.0 && snap.current_fps < 70.0); // ~60fps ± tolerance
}

#[test]
fn test_q5_add_bytes() {
    // Q5: Verify byte accumulation
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    capsule.add_bytes(512);
    assert_eq!(capsule.snapshot().bytes_written, 512);

    capsule.add_bytes(256);
    assert_eq!(capsule.snapshot().bytes_written, 768);

    capsule.add_bytes(1024);
    assert_eq!(capsule.snapshot().bytes_written, 1792);
}

#[test]
fn test_q6_progress_calculation() {
    // Q6: Verify progress percentage calculation
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    // 0% progress
    assert!((capsule.progress() - 0.0).abs() < 0.001);

    // 25% progress
    for _ in 0..25 {
        capsule.update_frame(16_666_666, 40.0, 0.95, 80);
    }
    assert!((capsule.progress() - 0.25).abs() < 0.01);

    // 50% progress
    for _ in 0..25 {
        capsule.update_frame(16_666_666, 40.0, 0.95, 80);
    }
    assert!((capsule.progress() - 0.50).abs() < 0.01);

    // 100% progress
    for _ in 0..50 {
        capsule.update_frame(16_666_666, 40.0, 0.95, 80);
    }
    assert!((capsule.progress() - 1.0).abs() < 0.01);
}

#[test]
fn test_q7_compression_ratio() {
    // Q7: Verify compression ratio calculation
    let capsule = MetricsCapsule::new();
    capsule.init(100, 100_000); // 100KB input

    // No bytes written yet
    assert_eq!(capsule.compression_ratio(), 0.0);

    // Write 10KB (10:1 compression)
    capsule.add_bytes(10_000);
    assert!((capsule.compression_ratio() - 10.0).abs() < 0.01);

    // Write another 10KB (total 20KB, 5:1 compression)
    capsule.add_bytes(10_000);
    assert!((capsule.compression_ratio() - 5.0).abs() < 0.01);
}

// ============================================================================
// Q8-Q14: Property Tests (EWMA, Bounds, Monotonicity)
// ============================================================================

#[test]
fn test_q8_ewma_convergence() {
    // Q8: Verify EWMA converges to stable value
    // Property: After many identical values, EWMA → value
    let capsule = MetricsCapsule::new();
    capsule.init(1000, 10_000_000);

    // Feed 100 identical frames (60fps, PSNR 42, SSIM 0.98)
    for _ in 0..100 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
    }

    let snap = capsule.snapshot();

    // Average FPS should converge to ~60fps
    assert!(
        (snap.average_fps - 60.0).abs() < 1.0,
        "FPS EWMA failed to converge"
    );

    // Average PSNR should converge to 42.0
    assert!(
        (snap.average_psnr - 42.0).abs() < 0.5,
        "PSNR EWMA failed to converge"
    );

    // Average SSIM should converge to 0.98
    assert!(
        (snap.average_ssim - 0.98).abs() < 0.01,
        "SSIM EWMA failed to converge"
    );
}

#[test]
fn test_q9_ewma_responsiveness() {
    // Q9: Verify EWMA responds to step changes (α=0.2 should respond quickly)
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    // Initial 50 frames at 30fps
    for _ in 0..50 {
        capsule.update_frame(33_333_333, 40.0, 0.95, 80);
    }
    let avg_fps_30 = capsule.snapshot().average_fps;
    assert!((avg_fps_30 - 30.0).abs() < 2.0);

    // Step change to 60fps for 50 frames
    for _ in 0..50 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
    }
    let avg_fps_60 = capsule.snapshot().average_fps;

    // With α=0.2, should adapt significantly after 50 samples
    assert!(
        avg_fps_60 > avg_fps_30 + 10.0,
        "EWMA not responsive to step change"
    );
    assert!(avg_fps_60 < 60.0, "EWMA overshot target"); // Should not overshoot
}

#[test]
fn test_q10_metric_bounds() {
    // Q10: Property - All metrics stay within valid bounds
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    for i in 0..100 {
        // Vary PSNR (30-50 dB typical range)
        let psnr = 30.0 + (i as f64 / 100.0) * 20.0;
        // Vary SSIM (0.85-0.99 typical range)
        let ssim = 0.85 + (i as f64 / 100.0) * 0.14;
        // Vary GPU util (50-100%)
        let gpu = 50 + (i * 50 / 100) as u8;

        capsule.update_frame(16_666_666, psnr, ssim, gpu);
    }

    let snap = capsule.snapshot();

    // Verify bounds
    assert!(
        snap.current_psnr >= 0.0 && snap.current_psnr <= 100.0,
        "PSNR out of bounds"
    );
    assert!(
        snap.current_ssim >= 0.0 && snap.current_ssim <= 1.0,
        "SSIM out of bounds"
    );
    assert!(snap.gpu_utilization <= 100, "GPU util out of bounds");
    assert!(snap.current_fps >= 0.0, "FPS negative");
    assert!(
        snap.progress() >= 0.0 && snap.progress() <= 1.0,
        "Progress out of bounds"
    );
}

#[test]
fn test_q11_monotonic_frame_count() {
    // Q11: Property - Frame count is monotonically increasing
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    let mut prev_count = 0u64;
    for _ in 0..50 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
        let current_count = capsule.snapshot().frames_encoded;
        assert!(current_count > prev_count, "Frame count not monotonic");
        prev_count = current_count;
    }
}

#[test]
fn test_q12_monotonic_bytes_written() {
    // Q12: Property - Bytes written is monotonically increasing
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    let mut prev_bytes = 0u64;
    for _ in 0..50 {
        capsule.add_bytes(1024);
        let current_bytes = capsule.snapshot().bytes_written;
        assert!(current_bytes > prev_bytes, "Bytes written not monotonic");
        prev_bytes = current_bytes;
    }
}

#[test]
fn test_q13_eta_decreases_over_time() {
    // Q13: Property - ETA should decrease as encoding progresses
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    // Encode first 25 frames
    for _ in 0..25 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
    }
    let eta_25 = capsule.snapshot().eta_seconds;

    // Encode next 25 frames
    for _ in 0..25 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
    }
    let eta_50 = capsule.snapshot().eta_seconds;

    // ETA should decrease (or stay same if already 0)
    assert!(eta_50 <= eta_25, "ETA increased instead of decreased");
}

#[test]
fn test_q14_fps_stability_under_constant_load() {
    // Q14: Property - FPS should stabilize under constant load
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    // Encode 100 frames at constant 60fps
    let mut fps_samples = Vec::new();
    for _ in 0..100 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
        if capsule.snapshot().frames_encoded > 10 {
            // Skip first 10 frames (warmup)
            fps_samples.push(capsule.snapshot().average_fps);
        }
    }

    // Calculate variance of last 90 FPS samples
    let mean_fps: f64 = fps_samples.iter().sum::<f64>() / fps_samples.len() as f64;
    let variance: f64 = fps_samples
        .iter()
        .map(|&fps| (fps - mean_fps).powi(2))
        .sum::<f64>()
        / fps_samples.len() as f64;
    let std_dev = variance.sqrt();

    // Standard deviation should be small (< 2fps) for stable load
    assert!(std_dev < 2.0, "FPS unstable: std_dev={}", std_dev);
}

// ============================================================================
// Q15-Q21: Integration Tests
// ============================================================================

#[test]
fn test_q15_full_encode_simulation() {
    // Q15: Simulate complete encoding workflow
    let capsule = MetricsCapsule::new();
    let total_frames = 1440; // 24fps × 60s
    let input_size = 100_000_000; // 100MB

    capsule.init(total_frames, input_size);

    // Simulate encoding with varying frame times
    for i in 0..total_frames {
        // Vary frame time: 15-18ms (55-66fps)
        let frame_time = 15_000_000 + (i % 3) * 1_000_000;

        // Vary quality: PSNR 40-44, SSIM 0.95-0.98
        let psnr = 40.0 + (i as f64 / total_frames as f64) * 4.0;
        let ssim = 0.95 + (i as f64 / total_frames as f64) * 0.03;

        // Vary GPU: 80-95%
        let gpu = (80 + (i % 15)) as u8;

        capsule.update_frame(frame_time, psnr, ssim, gpu);

        // Add bytes (average 69KB per frame for 10Mbps @ 60fps)
        capsule.add_bytes(69_000);
    }

    let snap = capsule.snapshot();

    // Verify final state
    assert_eq!(snap.frames_encoded, total_frames);
    assert_eq!(snap.frames_total, total_frames);
    assert!((snap.progress() - 1.0).abs() < 0.01);
    assert!(snap.average_fps > 50.0 && snap.average_fps < 70.0);
    assert!(snap.average_psnr > 40.0 && snap.average_psnr < 45.0);
    assert!(snap.average_ssim > 0.95 && snap.average_ssim < 0.99);
    assert!(snap.compression_ratio > 1.0); // Some compression achieved
}

#[test]
fn test_q16_snapshot_consistency() {
    // Q16: Verify snapshot returns consistent values
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    for _ in 0..50 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
        capsule.add_bytes(1000);
    }

    let snap1 = capsule.snapshot();
    let snap2 = capsule.snapshot();

    // Two snapshots taken consecutively should be identical
    // (no updates between them)
    assert_eq!(snap1.frames_encoded, snap2.frames_encoded);
    assert_eq!(snap1.bytes_written, snap2.bytes_written);
    assert!((snap1.average_fps - snap2.average_fps).abs() < 0.01);
    assert!((snap1.average_psnr - snap2.average_psnr).abs() < 0.01);
}

#[test]
fn test_q17_concurrent_updates() {
    // Q17: Verify thread-safe concurrent updates
    let capsule = Arc::new(MetricsCapsule::new());
    capsule.init(10_000, 10_000_000);

    let mut handles = vec![];

    // Spawn 4 encoder threads
    for _ in 0..4 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..2500 {
                c.update_frame(16_666_666, 42.0, 0.98, 85);
            }
        }));
    }

    // Spawn 2 writer threads
    for _ in 0..2 {
        let c = Arc::clone(&capsule);
        handles.push(thread::spawn(move || {
            for _ in 0..5000 {
                c.add_bytes(1000);
            }
        }));
    }

    // Wait for all threads
    for h in handles {
        h.join().unwrap();
    }

    let snap = capsule.snapshot();

    // Verify final counts
    assert_eq!(snap.frames_encoded, 10_000, "Frame count incorrect");
    assert_eq!(snap.bytes_written, 10_000_000, "Bytes written incorrect");
}

#[test]
fn test_q18_bitrate_calculation() {
    // Q18: Verify bitrate calculation accuracy
    let capsule = MetricsCapsule::new();
    capsule.init(60, 10_000_000); // 60 frames @ 60fps = 1 second

    // Encode at constant rate
    for _ in 0..60 {
        capsule.update_frame(16_666_666, 42.0, 0.98, 85);
        capsule.add_bytes(1_250_000); // 10 Mbps = 1.25 MB/frame @ 60fps
    }

    // Wait a bit for time to advance
    thread::sleep(Duration::from_millis(50));

    let snap = capsule.snapshot();

    // Bitrate should be ~10 Mbps (10,000,000 bps)
    // Allow 20% tolerance due to timing variations
    let expected_bps = 10_000_000.0;
    let tolerance = expected_bps * 0.2;

    assert!(
        (snap.current_bitrate as f64 - expected_bps).abs() < tolerance,
        "Bitrate calculation inaccurate: {} bps (expected ~{} bps)",
        snap.current_bitrate,
        expected_bps
    );
}

#[test]
fn test_q19_min_max_frame_time() {
    // Q19: Verify min/max frame time tracking
    let capsule = MetricsCapsule::new();
    capsule.init(100, 1_000_000);

    // Feed varying frame times
    let frame_times = vec![
        10_000_000, // 10ms (fastest)
        15_000_000, 20_000_000, 25_000_000, 30_000_000, // 30ms (slowest)
        20_000_000, 15_000_000,
    ];

    for time in frame_times {
        capsule.update_frame(time, 42.0, 0.98, 85);
    }

    let snap = capsule.snapshot();

    assert_eq!(
        snap.min_frame_time_ns, 10_000_000,
        "Min frame time incorrect"
    );
    assert_eq!(
        snap.max_frame_time_ns, 30_000_000,
        "Max frame time incorrect"
    );
}

#[test]
fn test_q20_quality_score_vmaf_approximation() {
    // Q20: Verify quality score (VMAF approximation)
    // VMAF ≈ 0.6*PSNR + 0.4*SSIM*100
    let capsule = MetricsCapsule::new();
    capsule.init(10, 1_000_000);

    let psnr = 42.0;
    let ssim = 0.98;
    let expected_vmaf = (0.6 * psnr + 0.4 * ssim * 100.0) as u64; // ≈ 64

    capsule.update_frame(16_666_666, psnr, ssim, 85);

    let snap = capsule.snapshot();

    assert!(
        (snap.quality_score as i64 - expected_vmaf as i64).abs() <= 2,
        "Quality score (VMAF) inaccurate: {} (expected ~{})",
        snap.quality_score,
        expected_vmaf
    );
}

#[test]
fn test_q21_snapshot_formatting() {
    // Q21: Verify MetricsSnapshot helper methods
    let snap = MetricsSnapshot {
        frames_encoded: 50,
        frames_total: 100,
        bytes_written: 10_000_000,
        input_bytes: 100_000_000,
        elapsed_ms: 10_000,
        current_fps: 60.5,
        average_fps: 58.3,
        current_bitrate: 10_000_000,
        current_psnr: 42.5,
        average_psnr: 41.8,
        current_ssim: 0.98,
        average_ssim: 0.97,
        quality_score: 65,
        gpu_utilization: 87,
        eta_seconds: 3661, // 1:01:01
        min_frame_time_ns: 15_000_000,
        max_frame_time_ns: 18_000_000,
        frame_time_ns: 16_666_666,
    };

    // Test progress()
    assert!((snap.progress() - 0.5).abs() < 0.01);

    // Test compression_ratio()
    assert!((snap.compression_ratio() - 10.0).abs() < 0.01);

    // Test bitrate_mbps()
    assert!((snap.bitrate_mbps() - 10.0).abs() < 0.01);

    // Test eta_formatted()
    assert_eq!(snap.eta_formatted(), "01:01:01");

    // Test shorter ETA
    let snap2 = MetricsSnapshot {
        eta_seconds: 125, // 2:05
        ..snap
    };
    assert_eq!(snap2.eta_formatted(), "02:05");
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

#[test]
fn test_send_sync_traits() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<MetricsCapsule>();
    assert_sync::<MetricsCapsule>();
}

#[test]
fn test_concurrent_snapshot_while_updating() {
    // Verify snapshots can be taken safely while updates are in progress
    let capsule = Arc::new(MetricsCapsule::new());
    capsule.init(100_000, 100_000_000);

    let c_writer = Arc::clone(&capsule);
    let writer = thread::spawn(move || {
        for _ in 0..100_000 {
            c_writer.update_frame(16_666_666, 42.0, 0.98, 85);
        }
    });

    let c_reader = Arc::clone(&capsule);
    let reader = thread::spawn(move || {
        let mut snapshots = Vec::new();
        for _ in 0..1000 {
            snapshots.push(c_reader.snapshot());
            thread::sleep(Duration::from_micros(100));
        }
        snapshots
    });

    writer.join().unwrap();
    let snapshots = reader.join().unwrap();

    // Verify all snapshots are valid
    for snap in snapshots {
        assert!(snap.frames_encoded <= 100_000);
        assert!(snap.progress() >= 0.0 && snap.progress() <= 1.0);
    }
}
