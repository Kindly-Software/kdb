//! Debug fold issue

use atomic_capsule::parallel::{IntoParallelIterator, ThreadPool};

fn main() {
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");

    // Test direct fold (no chaining)
    println!("Test 1: Direct fold");
    {
        let data = vec![6, 8, 10];
        let sum = data
            .into_par_iter()
            .with_pool(&pool)
            .fold(|| 0, |acc, x| acc + x, |a, b| a + b)
            .expect("Fold failed");
        println!("  Data: [6, 8, 10]");
        println!("  Sum: {}", sum);
        assert_eq!(sum, 24);
    }

    // Test map then collect
    println!("\nTest 2: Map then collect");
    {
        let data = vec![1, 2, 3, 4, 5];
        let mapped = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2)
            .collect()
            .expect("Failed");
        println!("  Mapped: {:?}", mapped);
        assert_eq!(mapped, vec![2, 4, 6, 8, 10]);
    }

    // Test map.filter then collect
    println!("\nTest 3: Map.filter then collect");
    {
        let data = vec![1, 2, 3, 4, 5];
        let filtered = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2)
            .filter(|x| x > &4)
            .collect()
            .expect("Failed");
        println!("  Filtered: {:?}", filtered);
        assert_eq!(filtered, vec![6, 8, 10]);
    }

    // Test collected vec then fold
    println!("\nTest 4: Collected vec then fold");
    {
        let data = vec![6, 8, 10];
        let iter = data.into_par_iter().with_pool(&pool);
        let sum = iter
            .fold(|| 0, |acc, x| acc + x, |a, b| a + b)
            .expect("Failed");
        println!("  Sum: {}", sum);
        assert_eq!(sum, 24);
    }

    println!("\n All tests passed!");
}
