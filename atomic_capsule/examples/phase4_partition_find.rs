//! Phase 4: partition() and find() operations demo

use atomic_capsule::parallel::iter::{IntoParallelIterator, ParallelIterator};

fn main() {
    println!("=== Phase 4: partition() and find() Demo ===\n");

    // Test 1: partition() - evens and odds
    println!("Test 1: partition() - evens and odds");
    let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let (evens, odds) = data.into_par_iter().partition(|x| x % 2 == 0);
    println!("Evens: {:?}", evens);
    println!("Odds: {:?}", odds);
    assert_eq!(evens, vec![2, 4, 6, 8, 10]);
    assert_eq!(odds, vec![1, 3, 5, 7, 9]);
    println!("✓ partition() correctness validated\n");

    // Test 2: find() - first even number
    println!("Test 2: find() - first even");
    let data = vec![1, 3, 5, 7, 2, 4, 6];
    let first_even = data.into_par_iter().find(|x| x % 2 == 0);
    println!("First even: {:?}", first_even);
    assert_eq!(first_even, Some(2));
    println!("✓ find() returns first match\n");

    // Test 3: find() - no match
    println!("Test 3: find() - no match");
    let data = vec![1, 3, 5, 7, 9];
    let result = data.into_par_iter().find(|x| x % 2 == 0);
    println!("Result: {:?}", result);
    assert_eq!(result, None);
    println!("✓ find() returns None when no match\n");

    // Test 4: find() - deterministic (returns lowest index)
    println!("Test 4: find() - deterministic (lowest index)");
    let data = vec![3, 1, 4, 1, 5, 9, 2, 6, 5];
    let first_gt_5 = data.into_par_iter().find(|x| *x > 5);
    println!("First > 5: {:?}", first_gt_5);
    // First element > 5 is at index 5 (value 9)
    assert_eq!(first_gt_5, Some(9));
    println!("✓ find() is deterministic\n");

    // Test 5: partition() with empty iterator
    println!("Test 5: partition() - empty");
    let data: Vec<i32> = vec![];
    let (matching, non_matching) = data.into_par_iter().partition(|x| *x > 0);
    println!("Matching: {:?}", matching);
    println!("Non-matching: {:?}", non_matching);
    assert!(matching.is_empty());
    assert!(non_matching.is_empty());
    println!("✓ partition() handles empty correctly\n");

    //  Test 6: Large dataset
    println!("Test 6: Large dataset (1000 items)");
    let data: Vec<i32> = (0..1000).collect();
    let (evens, odds) = data.into_par_iter().partition(|x| x % 2 == 0);
    println!("Evens count: {}", evens.len());
    println!("Odds count: {}", odds.len());
    assert_eq!(evens.len(), 500);
    assert_eq!(odds.len(), 500);
    println!("✓ partition() scales to large datasets\n");

    println!("=== All Phase 4 tests passed! ===");
}
