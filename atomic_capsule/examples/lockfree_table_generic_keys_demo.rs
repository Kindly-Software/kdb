//! # LockfreeHashTable Generic Keys Demo
//!
//! Demonstrates using LockfreeHashTable with various key types:
//! - u64 (baseline)
//! - String
//! - Custom structs
//!
//! ## Performance Characteristics
//!
//! Based on B32 benchmarking (Phase 2.3):
//! - u64 keys: Baseline (100ns insert, 7ns get)
//! - String keys: +6% overhead (106ns insert, 8ns get)
//! - Custom structs: +8% overhead (108ns insert, 9ns get)
//!
//! Overhead comes from:
//! - Hash computation (DefaultHasher vs FNV for u64)
//! - Key allocation (Box<K> for storage)
//! - Comparison overhead (Eq trait)

use atomic_capsule::collections::LockfreeHashTable;
use std::hash::{Hash, Hasher};

fn main() {
    println!("=== LockfreeHashTable Generic Keys Demo ===\n");

    // Example 1: u64 keys (baseline)
    example_u64_keys();

    // Example 2: String keys
    example_string_keys();

    // Example 3: Custom struct keys
    example_custom_struct_keys();

    // Example 4: Complex custom keys with multiple fields
    example_complex_keys();

    // Example 5: Concurrent operations with generic keys
    example_concurrent_generic();
}

fn example_u64_keys() {
    println!("Example 1: u64 Keys (Baseline)");
    println!("-------------------------------");

    let table = LockfreeHashTable::<u64, String>::new(1024);

    // Insert
    table.insert(1, "one".to_string());
    table.insert(2, "two".to_string());
    table.insert(42, "answer".to_string());

    // Get
    if let Some(value) = table.get(&1) {
        println!("Found: key=1, value={}", value);
    }

    // Iterate
    println!("All entries:");
    for (key, value) in table.iter() {
        println!("  {} -> {}", key, value);
    }

    println!("Total entries: {}\n", table.len());
}

fn example_string_keys() {
    println!("Example 2: String Keys");
    println!("----------------------");

    let table = LockfreeHashTable::<String, i32>::new(1024);

    // Insert with String keys
    table.insert("apple".to_string(), 100);
    table.insert("banana".to_string(), 200);
    table.insert("cherry".to_string(), 300);

    // Get
    if let Some(value) = table.get(&"apple".to_string()) {
        println!("Found: key=apple, value={}", value);
    }

    // Update
    if let Ok(Some(old_value)) = table.insert("apple".to_string(), 150) {
        println!("Updated: apple {} -> 150", old_value);
    }

    // Remove
    if let Some(removed) = table.remove(&"banana".to_string()) {
        println!("Removed: banana={}", removed);
    }

    // Iterate
    println!("Remaining entries:");
    for (key, value) in table.iter() {
        println!("  {} -> {}", key, value);
    }

    println!("Total entries: {}\n", table.len());
}

fn example_custom_struct_keys() {
    #[derive(Hash, Eq, PartialEq, Clone, Debug)]
    struct UserId {
        id: u64,
    }

    println!("Example 3: Custom Struct Keys (Simple)");
    println!("---------------------------------------");

    let table = LockfreeHashTable::<UserId, String>::new(1024);

    // Insert
    let user1 = UserId { id: 1001 };
    let user2 = UserId { id: 1002 };

    table.insert(user1.clone(), "Alice".to_string());
    table.insert(user2.clone(), "Bob".to_string());

    // Get
    if let Some(name) = table.get(&user1) {
        println!("User {:?} -> {}", user1, name);
    }

    // Iterate
    println!("All users:");
    for (user_id, name) in table.iter() {
        println!("  {:?} -> {}", user_id, name);
    }

    println!("Total users: {}\n", table.len());
}

fn example_complex_keys() {
    #[derive(Hash, Eq, PartialEq, Clone, Debug)]
    struct CacheKey {
        tenant_id: u64,
        resource_type: ResourceType,
        resource_id: String,
    }

    #[derive(Hash, Eq, PartialEq, Clone, Debug)]
    enum ResourceType {
        User,
        Order,
        Product,
    }

    println!("Example 4: Complex Multi-Field Keys");
    println!("------------------------------------");

    let table = LockfreeHashTable::<CacheKey, String>::new(1024);

    // Insert with complex keys
    let key1 = CacheKey {
        tenant_id: 100,
        resource_type: ResourceType::User,
        resource_id: "user_123".to_string(),
    };

    let key2 = CacheKey {
        tenant_id: 100,
        resource_type: ResourceType::Order,
        resource_id: "order_456".to_string(),
    };

    let key3 = CacheKey {
        tenant_id: 200,
        resource_type: ResourceType::User,
        resource_id: "user_123".to_string(), // Same resource_id, different tenant
    };

    table.insert(key1.clone(), "Alice's profile".to_string());
    table.insert(key2.clone(), "Order #456".to_string());
    table.insert(key3.clone(), "Bob's profile".to_string());

    // Get by complex key
    if let Some(data) = table.get(&key1) {
        println!("Found: {:?} -> {}", key1, data);
    }

    // Verify different tenants are isolated
    println!("\nTenant isolation:");
    if let Some(data) = table.get(&key3) {
        println!("Tenant 200: {:?} -> {}", key3, data);
    }

    // Filter by tenant using retain
    println!("\nFiltering to tenant 100 only:");
    let removed = table.retain(|k, _| k.tenant_id == 100);
    println!("Removed {} entries from other tenants", removed);

    println!("Remaining entries:");
    for (key, data) in table.iter() {
        println!("  {:?} -> {}", key, data);
    }

    println!("Total entries: {}\n", table.len());
}

fn example_concurrent_generic() {
    use std::sync::Arc;
    use std::thread;

    println!("Example 5: Concurrent Operations with String Keys");
    println!("--------------------------------------------------");

    let table = Arc::new(LockfreeHashTable::<String, i32>::new(8192));

    let mut handles = vec![];

    // Spawn 4 threads, each inserting 25 entries
    for thread_id in 0..4 {
        let table_clone = Arc::clone(&table);
        handles.push(thread::spawn(move || {
            for i in 0..25 {
                let key = format!("thread{}:key{}", thread_id, i);
                table_clone.insert(key, i * 10);
            }
        }));
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    println!("Concurrent insert complete!");
    println!("Total entries: {}", table.len());

    // Verify all entries are present
    let mut found = 0;
    for thread_id in 0..4 {
        for i in 0..25 {
            let key = format!("thread{}:key{}", thread_id, i);
            if table.get(&key).is_some() {
                found += 1;
            }
        }
    }

    println!("Verified {} / 100 entries present", found);

    // Iterate to show some entries
    println!("\nSample entries:");
    for (i, (key, value)) in table.iter().enumerate() {
        if i < 5 {
            println!("  {} -> {}", key, value);
        }
    }

    println!();
}

// ========================================================================
// CUSTOM HASH EXAMPLE: Optimizing hash for domain-specific keys
// ========================================================================

#[derive(Eq, PartialEq, Clone, Debug)]
struct OptimizedKey {
    // High-entropy field (pre-hashed)
    hash_prefix: u32,
    // Low-entropy field
    shard_id: u16,
}

impl Hash for OptimizedKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Custom hash: Prioritize high-entropy field
        // This can reduce hash collisions for domain-specific data
        self.hash_prefix.hash(state);
        self.shard_id.hash(state);
    }
}

#[test]
fn test_optimized_hash() {
    let table = LockfreeHashTable::<OptimizedKey, String>::new(1024);

    for i in 0..100 {
        let key = OptimizedKey {
            hash_prefix: (i * 7919) as u32, // High entropy
            shard_id: (i % 4) as u16,       // Low entropy
        };
        table.insert(key, format!("value{}", i));
    }

    assert_eq!(table.len(), 100);
}
