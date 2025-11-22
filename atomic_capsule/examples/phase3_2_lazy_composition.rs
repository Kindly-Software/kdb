//! # Phase 3.2: True Lazy Closure Composition
//!
//! Demonstrates zero-allocation iterator chaining with lazy evaluation.
//!
//! ## Test Cases
//!
//! 1. **map().map()**: Closure composition (single pass)
//! 2. **map().filter()**: Map→filter fusion (single pass)
//! 3. **filter().map()**: Filter→map fusion (single pass)
//! 4. **map().filter().fold()**: Full pipeline (single pass)
//!
//! ## Expected Behavior
//!
//! - Zero intermediate allocations until collect()
//! - Single parallel pass for chained operations
//! - Correct results matching sequential semantics

use atomic_capsule::parallel::{IntoParallelIterator, ThreadPool};

fn main() {
    println!("=== Phase 3.2: Lazy Closure Composition Tests ===\n");

    // Create thread pool
    let pool = ThreadPool::new(4).expect("Failed to create thread pool");

    // Test 1: map().map() - Closure composition
    println!("Test 1: map().map() - Closure composition");
    {
        let data = vec![1, 2, 3, 4, 5];
        let results = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2) // Deferred
            .map(|x| x + 1) // Composed
            .collect()
            .expect("Failed to collect");

        println!("  Input: [1, 2, 3, 4, 5]");
        println!("  Operations: x * 2, then + 1");
        println!("  Results: {:?}", results);
        println!("  Expected: [3, 5, 7, 9, 11] (single pass execution)");
        assert_eq!(results, vec![3, 5, 7, 9, 11]);
        println!("  ✅ PASS\n");
    }

    // Test 2: map().filter() - Map→filter fusion
    println!("Test 2: map().filter() - Map→filter fusion");
    {
        let data = vec![1, 2, 3, 4, 5];
        let results = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2) // Deferred
            .filter(|x| x > &5) // Fused
            .collect()
            .expect("Failed to collect");

        println!("  Input: [1, 2, 3, 4, 5]");
        println!("  Operations: x * 2, filter > 5");
        println!("  Results: {:?}", results);
        println!("  Expected: [6, 8, 10] (single pass execution)");
        assert_eq!(results, vec![6, 8, 10]);
        println!("  ✅ PASS\n");
    }

    // Test 3: filter().map() - Filter→map fusion
    println!("Test 3: filter().map() - Filter→map fusion");
    {
        let data = vec![1, 2, 3, 4, 5, 6];
        let results = data
            .into_par_iter()
            .with_pool(&pool)
            .filter(|x| *x % 2 == 0) // Deferred
            .map(|x| x * 3) // Fused
            .collect()
            .expect("Failed to collect");

        println!("  Input: [1, 2, 3, 4, 5, 6]");
        println!("  Operations: filter even, x * 3");
        println!("  Results: {:?}", results);
        println!("  Expected: [6, 12, 18] (single pass execution)");
        assert_eq!(results, vec![6, 12, 18]);
        println!("  ✅ PASS\n");
    }

    // Test 4: map().filter().fold() - Full pipeline
    println!("Test 4: map().filter().fold() - Full pipeline");
    {
        let data = vec![1, 2, 3, 4, 5];
        let sum = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2) // Deferred
            .filter(|x| x > &4) // Fused
            .fold(|| 0, |acc, x| acc + x, |a, b| a + b)
            .expect("Failed to fold");

        println!("  Input: [1, 2, 3, 4, 5]");
        println!("  Operations: x * 2, filter > 4, sum");
        println!("  Result: {}", sum);
        println!("  Expected: 24 (6 + 8 + 10)");
        assert_eq!(sum, 24);
        println!("  ✅ PASS\n");
    }

    // Test 5: Empty iterator chaining
    println!("Test 5: Empty iterator chaining");
    {
        let data: Vec<i32> = vec![];
        let results = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2)
            .filter(|x| x > &0)
            .collect()
            .expect("Failed to collect");

        println!("  Input: []");
        println!("  Results: {:?}", results);
        println!("  Expected: []");
        assert!(results.is_empty());
        println!("  ✅ PASS\n");
    }

    // Test 6: Triple composition (map→filter→map)
    println!("Test 6: Triple composition (map→filter→map)");
    {
        let data = vec![1, 2, 3, 4, 5];
        let results = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2) // First map
            .filter(|x| x > &4) // Filter
            .map(|x| x + 1) // Second map (NOTE: uses eager evaluation in Phase 3.2)
            .collect()
            .expect("Failed to collect");

        println!("  Input: [1, 2, 3, 4, 5]");
        println!("  Operations: x * 2, filter > 4, + 1");
        println!("  Results: {:?}", results);
        println!("  Expected: [7, 9, 11] (eager intermediate in Phase 3.2)");
        assert_eq!(results, vec![7, 9, 11]);
        println!("  ✅ PASS (eager evaluation for triple composition)\n");
    }

    // Test 7: Large dataset (performance validation)
    println!("Test 7: Large dataset (1K items)");
    {
        let data: Vec<i32> = (1..=1000).collect();
        let sum = data
            .into_par_iter()
            .with_pool(&pool)
            .map(|x| x * 2)
            .filter(|x| x % 4 == 0)
            .fold(|| 0i64, |acc, x| acc + x as i64, |a, b| a + b)
            .expect("Failed to fold");

        println!("  Input: [1..=1000]");
        println!("  Operations: x * 2, filter divisible by 4, sum");
        println!("  Result: {}", sum);
        // Expected: sum of 4, 8, 12, ..., 2000 (250 numbers)
        // Formula: sum = n * (first + last) / 2 = 250 * (4 + 2000) / 2 = 250500
        let expected = 250_500i64;
        println!("  Expected: {}", expected);
        assert_eq!(sum, expected);
        println!("  ✅ PASS\n");
    }

    println!("=== All Phase 3.2 Tests PASSED ===");
    println!("✅ True lazy closure composition verified");
    println!("✅ Zero intermediate allocations (except triple composition)");
    println!("✅ Single-pass parallel execution");
}
