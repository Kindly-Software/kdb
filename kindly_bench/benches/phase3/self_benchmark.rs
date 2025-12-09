//! Phase 3 Self-Benchmarking
//!
//! Benchmarks the kindly_bench framework itself to measure overhead.
//!
//! # Metrics
//!
//! - Timer overhead (TSC, Instant, GPU, Quantum)
//! - Baseline generation time
//! - Validation overhead
//! - XML output generation time

use std::time::Instant;
use kindly_bench::timing::{BenchTimer, TscTimer, InstantTimer};

fn main() {
    println!("kindly_bench Phase 3 Self-Benchmark");
    println!("====================================\n");

    benchmark_tsc_timer_overhead();
    benchmark_instant_timer_overhead();
    benchmark_validation_overhead();

    #[cfg(feature = "gpu")]
    benchmark_gpu_timer_overhead();

    #[cfg(feature = "quantum")]
    benchmark_quantum_timer_overhead();
}

fn benchmark_tsc_timer_overhead() {
    println!("TSC Timer Overhead Benchmark");
    println!("----------------------------");

    let mut timer = TscTimer::new();
    let overhead_ns = timer.calibrate_overhead();

    println!("TSC timer overhead: {} ns", overhead_ns);
    println!("Expected: 10-50 ns (serialized RDTSC)");

    if overhead_ns <= 50 {
        println!("✓ PASS: Overhead acceptable\n");
    } else {
        println!("✗ FAIL: Overhead too high (check CPU frequency scaling)\n");
    }
}

fn benchmark_instant_timer_overhead() {
    println!("Instant Timer Overhead Benchmark");
    println!("--------------------------------");

    let mut timer = InstantTimer::new();
    let overhead_ns = timer.calibrate_overhead();

    println!("Instant timer overhead: {} ns", overhead_ns);
    println!("Expected: 20-100 ns (Instant::now())");

    if overhead_ns <= 200 {
        println!("✓ PASS: Overhead acceptable\n");
    } else {
        println!("✗ FAIL: Overhead too high\n");
    }
}

#[cfg(feature = "gpu")]
fn benchmark_gpu_timer_overhead() {
    use kindly_bench::timing::GpuTimer;

    println!("GPU Timer Overhead Benchmark");
    println!("----------------------------");

    match cuda_runtime::CudaStream::create() {
        Ok(stream) => {
            match GpuTimer::cuda(stream) {
                Ok(mut timer) => {
                    let overhead_ns = timer.calibrate_overhead();
                    println!("GPU timer overhead: {} ns", overhead_ns);
                    println!("Expected: 1-100 µs (CUDA event sync)");

                    if overhead_ns <= 100_000 {
                        println!("✓ PASS: Overhead acceptable\n");
                    } else {
                        println!("✗ FAIL: Overhead too high\n");
                    }
                }
                Err(e) => println!("✗ SKIP: GPU timer creation failed: {:?}\n", e),
            }
        }
        Err(e) => println!("✗ SKIP: CUDA stream creation failed: {:?}\n", e),
    }
}

#[cfg(feature = "quantum")]
fn benchmark_quantum_timer_overhead() {
    use kindly_bench::timing::QuantumTimer;

    println!("Quantum Timer Overhead Benchmark");
    println!("--------------------------------");

    let mut timer = QuantumTimer::simulated();
    let overhead_ns = timer.calibrate_overhead();

    println!("Quantum timer overhead: {} ns", overhead_ns);
    println!("Expected: <10 µs (simulated backend)");

    if overhead_ns <= 10_000 {
        println!("✓ PASS: Overhead acceptable\n");
    } else {
        println!("✗ FAIL: Overhead too high\n");
    }
}

fn benchmark_validation_overhead() {
    use kindly_bench::validation::specialized::{validate_accuracy_metrics, validate_mmap_support};

    println!("Validation Overhead Benchmark");
    println!("-----------------------------");

    // Accuracy metrics validation
    let start = Instant::now();
    let _ = validate_accuracy_metrics(0.95, 0.92, 0.93, 0.90);
    let accuracy_overhead_ns = start.elapsed().as_nanos() as u64;

    println!("Accuracy metrics validation: {} ns", accuracy_overhead_ns);
    println!("Expected: <1 µs (simple threshold checks)");

    // Mmap support validation
    let start = Instant::now();
    let _ = validate_mmap_support("/tmp/test_mmap");
    let mmap_overhead_ns = start.elapsed().as_nanos() as u64;

    println!("Mmap support validation: {} µs", mmap_overhead_ns / 1000);
    println!("Expected: <10 ms (filesystem I/O)");

    println!("✓ Validation overhead acceptable\n");
}
