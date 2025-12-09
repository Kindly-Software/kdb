//! Stress test to reproduce segfault

use atomic_capsule::parallel::ThreadPool;

#[test]
fn test_stress_100_tasks() {
    let pool = ThreadPool::new(8).unwrap();

    for i in 0..100 {
        pool.push(Box::new(move || {
            let _ = i + 1; // Minimal work
        }))
        .unwrap();
    }

    pool.wait();
    println!("100 tasks completed");
}

#[test]
fn test_stress_1000_tasks() {
    let pool = ThreadPool::new(8).unwrap();

    for i in 0..1000 {
        pool.push(Box::new(move || {
            let _ = i + 1;
        }))
        .unwrap();
    }

    pool.wait();
    println!("1000 tasks completed");
}

#[test]
fn test_stress_repeated_pool_creation() {
    for _ in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for i in 0..100 {
            pool.push(Box::new(move || {
                let _ = i + 1;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pool creations completed");
}
