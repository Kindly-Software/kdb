//! # Helper Function Tests for Sustained Load Benchmarks
//!
//! **Purpose**: Unit tests for percentile and RSS calculation functions

#[test]
fn test_percentile_calculation() {
    // Test data
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

    // Helper function (copy from benchmark)
    fn percentile(data: &[u64], p: usize) -> u64 {
        if data.is_empty() {
            return 0;
        }

        let mut sorted = data.to_vec();
        sorted.sort_unstable();
        let idx = ((sorted.len() * p) / 100).min(sorted.len() - 1);
        sorted[idx]
    }

    // Test p50 (median) - with 10 elements, index = 10 * 50 / 100 = 5, data[5] = 6
    assert_eq!(percentile(&data, 50), 6);

    // Test p0 (min)
    assert_eq!(percentile(&data, 0), 1);

    // Test p100 (max, clamped to last element)
    assert_eq!(percentile(&data, 100), 10);

    // Test p99 (near max)
    assert_eq!(percentile(&data, 99), 10);

    // Test empty array
    assert_eq!(percentile(&[], 50), 0);

    // Test single element
    assert_eq!(percentile(&[42], 50), 42);
}

#[test]
fn test_percentile_large_dataset() {
    // Generate 1000 values (1..1000)
    let data: Vec<u64> = (1..=1000).collect();

    fn percentile(data: &[u64], p: usize) -> u64 {
        if data.is_empty() {
            return 0;
        }

        let mut sorted = data.to_vec();
        sorted.sort_unstable();
        let idx = ((sorted.len() * p) / 100).min(sorted.len() - 1);
        sorted[idx]
    }

    // p50 should be ~500
    let p50 = percentile(&data, 50);
    assert!(p50 >= 495 && p50 <= 505, "p50 = {}, expected ~500", p50);

    // p99 should be ~990
    let p99 = percentile(&data, 99);
    assert!(p99 >= 985 && p99 <= 995, "p99 = {}, expected ~990", p99);

    // p1 should be ~10
    let p1 = percentile(&data, 1);
    assert!(p1 >= 5 && p1 <= 15, "p1 = {}, expected ~10", p1);
}

#[cfg(target_os = "linux")]
#[test]
fn test_get_process_rss() {
    use std::fs;

    // Helper function (copy from benchmark)
    fn get_process_rss() -> usize {
        match fs::read_to_string("/proc/self/status") {
            Ok(status) => {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return kb * 1024; // Convert KB to bytes
                            }
                        }
                    }
                }
                0
            }
            Err(_) => 0,
        }
    }

    let rss = get_process_rss();

    // Sanity check: RSS should be >0 and <10GB for a test process
    assert!(rss > 0, "RSS should be non-zero on Linux");
    assert!(rss < 10 * 1024 * 1024 * 1024, "RSS should be <10GB");

    // Typical test process: 1MB - 100MB
    println!("Test process RSS: {:.2} MB", rss as f64 / (1024.0 * 1024.0));
}

#[cfg(not(target_os = "linux"))]
#[test]
fn test_get_process_rss_fallback() {
    // On non-Linux, should return 0 (graceful degradation)
    fn get_process_rss() -> usize {
        0
    }

    let rss = get_process_rss();
    assert_eq!(rss, 0, "RSS should be 0 on non-Linux platforms");
}

#[test]
fn test_memory_sample_struct() {
    #[derive(Debug, Clone)]
    struct MemorySample {
        timestamp_ms: u64,
        rss_bytes: usize,
        map_len: usize,
    }

    let sample = MemorySample {
        timestamp_ms: 1000,
        rss_bytes: 50 * 1024 * 1024, // 50 MB
        map_len: 10_000,
    };

    assert_eq!(sample.timestamp_ms, 1000);
    assert_eq!(sample.rss_bytes, 50 * 1024 * 1024);
    assert_eq!(sample.map_len, 10_000);
}

#[test]
fn test_latency_sample_struct() {
    #[derive(Debug, Clone)]
    struct LatencySample {
        timestamp_ms: u64,
        latency_ns: u64,
    }

    let sample = LatencySample {
        timestamp_ms: 1000,
        latency_ns: 150_000, // 150 μs
    };

    assert_eq!(sample.timestamp_ms, 1000);
    assert_eq!(sample.latency_ns, 150_000);
}
