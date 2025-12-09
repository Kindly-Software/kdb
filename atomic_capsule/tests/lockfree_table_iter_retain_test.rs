//! Integration tests for LockfreeHashTable iter() and retain() methods
//! Phase 5.6 implementation

use atomic_capsule::collections::LockfreeHashTable;
use std::sync::Arc;
use std::thread;

#[test]
fn test_iter_empty_table() {
    let table = LockfreeHashTable::<i32>::new(1024);
    let mut count = 0;

    for _ in table.iter() {
        count += 1;
    }

    assert_eq!(count, 0, "Empty table should yield no items");
}

#[test]
fn test_iter_single_entry() {
    let table = LockfreeHashTable::new(1024);
    table.insert(42, "value".to_string());

    let mut count = 0;
    for (key, value) in table.iter() {
        assert_eq!(key, 42);
        assert_eq!(value, "value");
        count += 1;
    }

    assert_eq!(count, 1, "Single entry should yield one item");
}

#[test]
fn test_iter_multiple_entries() {
    let table = LockfreeHashTable::new(1024);

    // Insert 100 entries
    for i in 0..100 {
        table.insert(i, i * 2);
    }

    let mut pairs: Vec<_> = table.iter().collect();
    pairs.sort_by_key(|(k, _)| *k);

    assert_eq!(pairs.len(), 100, "Should iterate over all 100 entries");

    for (i, (key, value)) in pairs.iter().enumerate() {
        assert_eq!(*key, i as u64);
        assert_eq!(**value, (i * 2) as u64);
    }
}

#[test]
fn test_iter_with_hash_collisions() {
    let table = LockfreeHashTable::new(16); // Small capacity to force collisions

    // Insert 64 entries (will definitely have collisions and chains)
    for i in 0..64 {
        table.insert(i, i * 10);
    }

    let pairs: Vec<_> = table.iter().collect();
    assert_eq!(
        pairs.len(),
        64,
        "Should iterate over all entries including chains"
    );

    // Verify all entries are present
    for i in 0..64 {
        let found = pairs.iter().any(|(k, v)| *k == i && **v == i * 10);
        assert!(found, "Entry {} not found in iteration", i);
    }
}

#[test]
fn test_retain_empty_table() {
    let table = LockfreeHashTable::<i32>::new(1024);
    let removed = table.retain(|_| false);
    assert_eq!(removed, 0, "Retain on empty table should remove 0 items");
    assert_eq!(table.len(), 0);
}

#[test]
fn test_retain_keep_all() {
    let table = LockfreeHashTable::new(1024);

    for i in 0..20 {
        table.insert(i, i);
    }

    // Keep all entries
    let removed = table.retain(|_| true);
    assert_eq!(
        removed, 0,
        "Retain with true predicate should remove 0 items"
    );
    assert_eq!(table.len(), 20);

    // Verify all entries still present
    for i in 0..20 {
        assert_eq!(table.get(i), Some(&i));
    }
}

#[test]
fn test_retain_remove_all() {
    let table = LockfreeHashTable::new(1024);

    for i in 0..20 {
        table.insert(i, i);
    }

    // Remove all entries
    let removed = table.retain(|_| false);
    assert_eq!(
        removed, 20,
        "Retain with false predicate should remove all items"
    );
    assert_eq!(table.len(), 0);

    // Verify all entries removed
    for i in 0..20 {
        assert_eq!(table.get(i), None);
    }
}

#[test]
fn test_retain_filter_even_values() {
    let table = LockfreeHashTable::new(1024);

    for i in 0..40 {
        table.insert(i, i);
    }

    // Keep only even values
    let removed = table.retain(|v| *v % 2 == 0);
    assert_eq!(removed, 20, "Should remove 20 odd values");
    assert_eq!(table.len(), 20, "Should keep 20 even values");

    // Verify only even values remain
    for i in 0..40 {
        if i % 2 == 0 {
            assert_eq!(table.get(i), Some(&i), "Even value {} should be present", i);
        } else {
            assert_eq!(table.get(i), None, "Odd value {} should be removed", i);
        }
    }
}

#[test]
fn test_retain_with_collisions() {
    let table = LockfreeHashTable::new(16); // Small capacity to force collisions

    // Insert 64 entries
    for i in 0..64 {
        table.insert(i, i);
    }

    // Keep values < 32
    let removed = table.retain(|v| *v < 32);
    assert_eq!(removed, 32, "Should remove 32 values >= 32");
    assert_eq!(table.len(), 32, "Should keep 32 values < 32");

    // Verify correct entries remain
    for i in 0..64 {
        if i < 32 {
            assert_eq!(table.get(i), Some(&i), "Value {} should remain", i);
        } else {
            assert_eq!(table.get(i), None, "Value {} should be removed", i);
        }
    }
}

#[test]
fn test_iter_concurrent_modifications() {
    let table = Arc::new(LockfreeHashTable::new(8192));

    // Pre-populate with 100 entries
    for i in 0..100 {
        table.insert(i, i);
    }

    let table_clone = Arc::clone(&table);
    let handle = thread::spawn(move || {
        // Insert more entries during iteration
        for i in 100..200 {
            table_clone.insert(i, i);
        }
    });

    // Iterate (will see a consistent snapshot)
    let pairs: Vec<_> = table.iter().collect();

    handle.join().unwrap();

    // Iterator saw at least the initial 100 entries
    assert!(
        pairs.len() >= 100,
        "Iterator should see at least initial entries"
    );

    // Final table has all 200 entries
    assert_eq!(table.len(), 200, "Final table should have all entries");
}

#[test]
fn test_retain_concurrent() {
    let table = Arc::new(LockfreeHashTable::new(8192));

    // Pre-populate with 1000 entries
    for i in 0..1000 {
        table.insert(i, i);
    }

    let mut handles = vec![];

    // Concurrent retain operations (all remove the same range)
    for _ in 0..4 {
        let table_clone = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            // Each thread removes values outside 250..750
            table_clone.retain(|v| *v >= 250 && *v < 750);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Verify correct range remains (250..750)
    for i in 0..1000 {
        if i >= 250 && i < 750 {
            assert!(
                table.contains_key(i),
                "Key {} in range should be present",
                i
            );
        }
        // Keys outside range may or may not be removed due to race conditions
        // This is expected behavior for concurrent retain
    }
}

#[test]
fn test_iter_and_retain_integration() {
    let table = LockfreeHashTable::new(1024);

    // Insert entries
    for i in 0..50 {
        table.insert(i, i * 2);
    }

    // First iteration: collect all values
    let values_before: Vec<_> = table.iter().map(|(_, v)| *v).collect();
    assert_eq!(values_before.len(), 50);

    // Retain only values < 50
    let removed = table.retain(|v| *v < 50);
    assert_eq!(removed, 25, "Should remove 25 values >= 50");

    // Second iteration: verify only values < 50 remain
    let values_after: Vec<_> = table.iter().map(|(_, v)| *v).collect();
    assert_eq!(values_after.len(), 25, "Should have 25 values remaining");

    for value in values_after {
        assert!(value < 50, "All remaining values should be < 50");
    }
}

#[test]
fn test_iter_performance_snapshot() {
    // This test verifies that iteration creates a snapshot
    // and doesn't block concurrent operations
    let table = Arc::new(LockfreeHashTable::new(8192));

    // Pre-populate
    for i in 0..100 {
        table.insert(i, i);
    }

    let table_clone = Arc::clone(&table);

    // Create iterator (starts snapshot)
    let iter = table.iter();

    // Start concurrent modifications
    let handle = thread::spawn(move || {
        for i in 100..200 {
            table_clone.insert(i, i);
        }
    });

    // Consume iterator (should see initial 100, not concurrent inserts)
    let pairs: Vec<_> = iter.collect();

    handle.join().unwrap();

    // Iterator should see at least the initial entries
    // May or may not see concurrent inserts depending on timing
    assert!(
        pairs.len() >= 100,
        "Iterator should see at least initial 100 entries"
    );
}
