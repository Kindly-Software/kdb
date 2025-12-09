//! Test with varying task counts to find threshold

use atomic_capsule::parallel::ThreadPool;

#[test]
fn test_pool_with_10_tasks_repeated() {
    for i in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for j in 0..10 {
            pool.push(Box::new(move || {
                let _ = i + j;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pools × 10 tasks = 100 total");
}

#[test]
fn test_pool_with_50_tasks_repeated() {
    for i in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for j in 0..50 {
            pool.push(Box::new(move || {
                let _ = i + j;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pools × 50 tasks = 500 total");
}

#[test]
fn test_pool_with_100_tasks_repeated() {
    for i in 0..10 {
        let pool = ThreadPool::new(8).unwrap();
        for j in 0..100 {
            pool.push(Box::new(move || {
                let _ = i + j;
            }))
            .unwrap();
        }
        pool.wait();
    }
    println!("10 pools × 100 tasks = 1000 total");
}
