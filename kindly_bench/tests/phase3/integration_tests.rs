//! Phase 3 Integration Tests
//!
//! Tests multi-timer infrastructure, baseline generation, and specialized validation.

use kindly_bench::timing::{BenchTimer, TscTimer, InstantTimer};
use kindly_bench::validation::specialized::{
    validate_accuracy_metrics, validate_mmap_support,
};

#[test]
fn test_tsc_timer_basic() {
    let mut timer = TscTimer::new();

    // Measure small workload
    let start = timer.start();
    let mut sum = 0u64;
    for i in 0..1000 {
        sum = sum.wrapping_add(i);
    }
    let elapsed_ns = timer.end(start);

    // Should measure something > 0ns
    assert!(elapsed_ns > 0);
    // Prevent optimization
    assert!(sum > 0);
}

#[test]
fn test_instant_timer_basic() {
    let mut timer = InstantTimer::new();

    // Measure small workload
    let start = timer.start();
    std::thread::sleep(std::time::Duration::from_micros(100));
    let elapsed_ns = timer.end(start);

    // Should measure ~100µs (100,000ns)
    assert!(elapsed_ns >= 50_000);  // Allow 50% tolerance
    assert!(elapsed_ns <= 200_000);
}

#[test]
fn test_timer_trait_uniformity() {
    // Both timers implement same trait
    fn test_timer<T: BenchTimer>(mut timer: T) -> u64 {
        let start = timer.start();
        std::thread::sleep(std::time::Duration::from_micros(10));
        timer.end(start)
    }

    let tsc_timer = TscTimer::new();
    let instant_timer = InstantTimer::new();

    let tsc_result = test_timer(tsc_timer);
    let instant_result = test_timer(instant_timer);

    // Both should measure ~10µs
    assert!(tsc_result > 0);
    assert!(instant_result > 0);
}

#[test]
fn test_validate_accuracy_metrics_pass() {
    let result = validate_accuracy_metrics(0.95, 0.92, 0.93, 0.90);
    assert!(result.is_ok());
}

#[test]
fn test_validate_accuracy_metrics_fail() {
    let result = validate_accuracy_metrics(0.85, 0.92, 0.88, 0.90);
    assert!(result.is_err());
}

#[test]
fn test_validate_mmap_support() {
    let result = validate_mmap_support("/tmp/test_mmap");
    // Should succeed on Unix-like systems
    if cfg!(unix) {
        assert!(result.is_ok());
    }
}

#[cfg(feature = "gpu")]
#[test]
fn test_gpu_timer_creation() {
    use kindly_bench::timing::GpuTimer;

    // This test requires CUDA runtime
    // Skip if GPU not available
    if let Ok(stream) = cuda_runtime::CudaStream::create() {
        let timer = GpuTimer::cuda(stream);
        assert!(timer.is_ok());
    }
}

#[cfg(feature = "quantum")]
#[test]
fn test_quantum_timer_basic() {
    use kindly_bench::timing::QuantumTimer;

    let mut timer = QuantumTimer::simulated();
    let start = timer.start();
    std::thread::sleep(std::time::Duration::from_millis(1));
    let elapsed_ns = timer.end(start);

    // Should measure ~1ms
    assert!(elapsed_ns >= 800_000);
    assert!(elapsed_ns <= 1_200_000);
}

#[test]
fn test_baseline_generator_manual_guide() {
    use kindly_bench::baseline::{BaselineGenerator, T7GpuBaseline};

    let baseline = T7GpuBaseline;

    // T7 is manual, not auto-generated
    assert!(!baseline.is_auto_generated());

    // Should provide guide
    let guide = baseline.manual_guide();
    assert!(guide.contains("GPU"));
    assert!(guide.contains("CPU"));
    assert!(guide.contains("OpenBLAS"));
}

#[test]
fn test_baseline_generator_t9_auto() {
    use kindly_bench::baseline::{BaselineGenerator, T9PersistentBaseline};

    let baseline = T9PersistentBaseline;

    // T9 is auto-generated
    assert!(baseline.is_auto_generated());

    // Should provide guide
    let guide = baseline.manual_guide();
    assert!(guide.contains("Persistent"));
    assert!(guide.contains("In-memory"));
}
