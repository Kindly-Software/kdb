//! Diagnostic test to understand why parallel is slower than sequential

use atomic_capsule::parallel::iter::{IntoParallelIterator, ParallelIterator};
use std::time::Instant;

fn main() {
    let size = 100_000;
    let data: Vec<i32> = (0..size).map(|i| i as i32).collect();
    let threshold = (size / 2) as i32;

    println!("Testing with {} elements", size);
    println!();

    // Warmup
    for _ in 0..10 {
        let _: Vec<i32> = data.iter().copied().filter(|&x| x > threshold).collect();
        let filtered_refs: Vec<&i32> = data.as_slice().into_par_iter().filter(|&&x| x > threshold);
        let _: Vec<i32> = filtered_refs.into_iter().copied().collect();
    }

    // Sequential
    let start = Instant::now();
    for _ in 0..100 {
        let _: Vec<i32> = data.iter().copied().filter(|&x| x > threshold).collect();
    }
    let seq_time = start.elapsed();
    println!("Sequential (100 iterations): {:?}", seq_time);
    println!("Sequential per iteration: {:?}", seq_time / 100);

    // Parallel
    let start = Instant::now();
    for _ in 0..100 {
        let filtered_refs: Vec<&i32> = data.as_slice().into_par_iter().filter(|&&x| x > threshold);
        let _: Vec<i32> = filtered_refs.into_iter().copied().collect();
    }
    let par_time = start.elapsed();
    println!("Parallel (100 iterations): {:?}", par_time);
    println!("Parallel per iteration: {:?}", par_time / 100);

    println!();
    println!("Speedup: {:.2}×", seq_time.as_nanos() as f64 / par_time.as_nanos() as f64);

    // Check CPU cores
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("CPU cores: {}", cores);
}
