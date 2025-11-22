//! # Borrow<Q> Zero-Allocation Demo
//!
//! Demonstrates the zero-allocation benefits of Borrow<Q> support in ConcurrentMapCapsule.
//!
//! ## Key Benefits
//! 1. **String keys with &str lookups**: No String allocation (~20ns savings per lookup)
//! 2. **Vec<T> keys with &[T] lookups**: No Vec allocation
//! 3. **Backward compatible**: Existing code with owned lookups still works
//!
//! ## Use Cases
//! - HTTP header lookups (String keys, &str queries)
//! - Configuration maps (String keys, &str queries)
//! - Cache lookups (owned keys, borrowed queries)

use atomic_capsule::collections::ConcurrentMapCapsule;
use std::time::Instant;

fn main() {
    println!("=== Borrow<Q> Zero-Allocation Demo ===\n");

    demo_string_str_lookups();
    demo_vec_slice_lookups();
    demo_backward_compatibility();
    demo_performance_comparison();
    demo_real_world_http_headers();
}

/// Demo 1: String keys with &str borrowed lookups
fn demo_string_str_lookups() {
    println!("1. String Keys with &str Borrowed Lookups");
    println!("-----------------------------------------");

    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Insert with owned Strings
    map.insert("user_id".to_string(), 12345);
    map.insert("session_token".to_string(), 67890);
    map.insert("api_key".to_string(), 11111);

    // Lookup with &str (zero allocation)
    println!("✓ get(\"user_id\"): {:?}", map.get("user_id"));
    println!("✓ get(\"session_token\"): {:?}", map.get("session_token"));
    println!("✓ get(\"missing_key\"): {:?}", map.get("missing_key"));

    // Check existence with &str
    println!(
        "✓ contains_key(\"api_key\"): {}",
        map.contains_key("api_key")
    );

    // Remove with &str
    println!("✓ remove(\"user_id\"): {:?}", map.remove("user_id"));
    println!(
        "✓ After removal, contains_key(\"user_id\"): {}",
        map.contains_key("user_id")
    );

    println!();
}

/// Demo 2: Vec<u8> keys with &[u8] borrowed lookups
fn demo_vec_slice_lookups() {
    println!("2. Vec<u8> Keys with &[u8] Borrowed Lookups");
    println!("-------------------------------------------");

    let map: ConcurrentMapCapsule<Vec<u8>, String> = ConcurrentMapCapsule::new();

    // Insert with owned Vec<u8>
    map.insert(vec![1, 2, 3], "data_1".to_string());
    map.insert(vec![4, 5, 6], "data_2".to_string());
    map.insert(vec![7, 8, 9], "data_3".to_string());

    // Lookup with &[u8] (zero allocation)
    println!("✓ get(&[1, 2, 3]): {:?}", map.get(&[1, 2, 3][..]));
    println!("✓ get(&[4, 5, 6]): {:?}", map.get(&[4, 5, 6][..]));
    println!("✓ get(&[10, 11, 12]): {:?}", map.get(&[10, 11, 12][..]));

    println!();
}

/// Demo 3: Backward compatibility (owned lookups still work)
fn demo_backward_compatibility() {
    println!("3. Backward Compatibility (Owned Lookups)");
    println!("-----------------------------------------");

    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Insert
    map.insert("key1".to_string(), 100);

    // Old style: owned key lookup (still works!)
    let owned_key = "key1".to_string();
    println!("✓ get(&owned_key): {:?}", map.get(&owned_key));
    println!(
        "✓ contains_key(&owned_key): {}",
        map.contains_key(&owned_key)
    );
    println!("✓ remove(&owned_key): {:?}", map.remove(&owned_key));

    println!();
}

/// Demo 4: Performance comparison (owned vs borrowed)
fn demo_performance_comparison() {
    println!("4. Performance Comparison (Owned vs Borrowed)");
    println!("----------------------------------------------");

    let map: ConcurrentMapCapsule<String, u64> = ConcurrentMapCapsule::new();

    // Pre-populate
    for i in 0..1000 {
        map.insert(format!("key{:04}", i), i);
    }

    const ITERATIONS: usize = 10_000;

    // Benchmark owned String lookups (allocates every time)
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        for i in 0..100 {
            let key = format!("key{:04}", i); // ALLOCATES
            let _ = map.get(&key);
        }
    }
    let owned_duration = start.elapsed();

    // Benchmark borrowed &str lookups (zero allocation)
    let keys: Vec<String> = (0..100).map(|i| format!("key{:04}", i)).collect();
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        for key_str in &keys {
            let _ = map.get(key_str.as_str()); // NO ALLOCATION
        }
    }
    let borrowed_duration = start.elapsed();

    println!(
        "✓ Owned String lookups:   {:?} ({} iterations)",
        owned_duration,
        ITERATIONS * 100
    );
    println!(
        "✓ Borrowed &str lookups:  {:?} ({} iterations)",
        borrowed_duration,
        ITERATIONS * 100
    );
    println!(
        "✓ Speedup: {:.2}× faster",
        owned_duration.as_nanos() as f64 / borrowed_duration.as_nanos() as f64
    );
    println!(
        "✓ Savings: ~{:.1}ns per lookup (String allocation cost)",
        (owned_duration.as_nanos() - borrowed_duration.as_nanos()) as f64
            / (ITERATIONS * 100) as f64
    );

    println!();
}

/// Demo 5: Real-world use case - HTTP header lookups
fn demo_real_world_http_headers() {
    println!("5. Real-World Use Case: HTTP Header Lookups");
    println!("--------------------------------------------");

    // Simulate HTTP headers map
    let headers: ConcurrentMapCapsule<String, String> = ConcurrentMapCapsule::new();

    headers.insert("content-type".to_string(), "application/json".to_string());
    headers.insert("user-agent".to_string(), "Mozilla/5.0".to_string());
    headers.insert("accept".to_string(), "text/html".to_string());
    headers.insert("authorization".to_string(), "Bearer token123".to_string());

    // Typical HTTP header lookups (string literals = zero allocation)
    println!("✓ Lookup 'content-type': {:?}", headers.get("content-type"));
    println!("✓ Lookup 'user-agent': {:?}", headers.get("user-agent"));
    println!("✓ Lookup 'accept': {:?}", headers.get("accept"));
    println!(
        "✓ Lookup 'authorization': {:?}",
        headers.get("authorization")
    );

    // Check if header exists
    if headers.contains_key("authorization") {
        println!("✓ Authorization header present");
    }

    // Remove sensitive headers (zero allocation)
    if let Some(token) = headers.remove("authorization") {
        println!("✓ Removed authorization: {} chars", token.len());
    }

    println!();
}
