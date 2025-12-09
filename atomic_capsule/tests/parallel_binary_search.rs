//! Binary search for failing task count

use atomic_capsule::parallel::ThreadPool;

#[test]
fn test_75_tasks() {
    for i in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for j in 0..75 {
            pool.push(Box::new(move || {
                let _ = i + j;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pools × 75 tasks completed");
}

#[test]
fn test_60_tasks() {
    for i in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for j in 0..60 {
            pool.push(Box::new(move || {
                let _ = i + j;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pools × 60 tasks completed");
}
