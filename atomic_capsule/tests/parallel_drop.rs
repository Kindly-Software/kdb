//! Test to isolate drop-related double-free

use atomic_capsule::parallel::ThreadPool;

#[test]
fn test_single_pool_drop() {
    let pool = ThreadPool::new(8).unwrap();
    pool.push(Box::new(|| {
        let _ = 1 + 1;
    }))
    .unwrap();
    pool.wait();
    drop(pool);
    println!("Single pool drop completed");
}

#[test]
fn test_two_pools_sequential() {
    {
        let pool = ThreadPool::new(8).unwrap();
        pool.push(Box::new(|| {
            let _ = 1 + 1;
        }))
        .unwrap();
        pool.wait();
    } // First pool drops

    {
        let pool = ThreadPool::new(8).unwrap();
        pool.push(Box::new(|| {
            let _ = 1 + 1;
        }))
        .unwrap();
        pool.wait();
    } // Second pool drops

    println!("Two sequential pools completed");
}

#[test]
fn test_five_pools_sequential() {
    for i in 0..5 {
        let pool = ThreadPool::new(8).unwrap();
        pool.push(Box::new(move || {
            let _ = i + 1;
        }))
        .unwrap();
        pool.wait();
        println!("Pool {} completed", i);
    }
    println!("Five sequential pools completed");
}
