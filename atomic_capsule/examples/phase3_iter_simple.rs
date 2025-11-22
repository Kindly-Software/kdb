//! Simple Phase 3 ParallelIterator smoke test
//!
//! Validates basic API functionality without complex test infrastructure

use atomic_capsule::parallel::iter::{IntoParallelIterator, ParallelIterator};

fn main() {
    println!("Phase 3 ParallelIterator Smoke Test\n");

    // Test 1: for_each
    println!("Test 1: for_each");
    let data = vec![1, 2, 3, 4, 5];
    data.into_par_iter().for_each(|x| {
        println!("  Item: {}", x);
    });

    // Test 2: map
    println!("\nTest 2: map");
    let data = vec![1, 2, 3, 4, 5];
    let results: Vec<i32> = data.into_par_iter().map(|x| x * 2);
    println!("  Doubled: {:?}", results);
    assert_eq!(results, vec![2, 4, 6, 8, 10]);

    // Test 3: filter
    println!("\nTest 3: filter");
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let evens: Vec<i32> = data.into_par_iter().filter(|x| x % 2 == 0);
    println!("  Evens: {:?}", evens);
    assert_eq!(evens, vec![2, 4, 6, 8, 10]);

    // Test 4: fold (with combiner)
    println!("\nTest 4: fold (with combiner)");
    let data = vec![1, 2, 3, 4, 5];
    let sum = data
        .into_par_iter()
        .fold(|| 0, |acc, x| acc + x, |a, b| a + b);
    println!("  Sum: {}", sum);
    assert_eq!(sum, 15); // ✅ Now correct with combiner!

    // Test 5: reduce (simplified API)
    println!("\nTest 5: reduce");
    let data = vec![1, 2, 3, 4, 5];
    let sum = data.into_par_iter().reduce(0, |a, b| a + b);
    println!("  Sum: {}", sum);
    assert_eq!(sum, 15);

    println!("\n✅ All smoke tests passed!");
}
