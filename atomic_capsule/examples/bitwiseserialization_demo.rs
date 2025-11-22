//! # BitwiseSerializable Demo - Real-World Usage Examples
//!
//! Demonstrates BitwiseSerializable trait with ConcurrentMapCapsule for:
//! - Arc<T> values (shared state)
//! - String keys and values
//! - Primitive types
//! - Complex composite types
//!
//! ## Performance Targets (B32 Framework)
//! - Primitive serialization: <2ns (identity transform)
//! - Arc serialization: <5ns (pointer cast)
//! - String serialization: <10ns (Box allocation)
//! - Zero allocation overhead for primitives

use atomic_capsule::collections::{BitwiseSerializable, ConcurrentMapCapsule};
use std::sync::Arc;

fn main() {
    println!("\n=== BitwiseSerializable Demo ===\n");

    demo_primitives();
    demo_arc_values();
    demo_string_keys();
    demo_complex_types();
    demo_service_registry();

    println!("\n=== All Demos Complete ===\n");
}

// ============================================================================
// Demo 1: Primitive Types
// ============================================================================

fn demo_primitives() {
    println!("--- Demo 1: Primitive Types ---");

    // All primitive types implement BitwiseSerializable
    let u64_val: u64 = 42;
    let storage = u64_val.to_storage();
    let restored = u64::from_storage(storage);
    println!("u64: {} -> {} (identity)", u64_val, restored);
    unsafe {
        u64::drop_storage(storage);
    }

    // ConcurrentMapCapsule with primitive types
    let map = ConcurrentMapCapsule::new();

    map.insert(1, 100u64);
    map.insert(2, 200u64);
    map.insert(3, 300u64);

    println!("Map size: {}", map.len());

    for i in 1..=3 {
        if let Some(value) = map.get(&i) {
            println!("  Key {}: Value {}", i, value);
        }
    }

    println!();
}

// ============================================================================
// Demo 2: Arc<T> Values (Shared State)
// ============================================================================

fn demo_arc_values() {
    println!("--- Demo 2: Arc<T> Values (Shared State) ---");

    #[derive(Debug, Clone)]
    struct Config {
        endpoint: String,
        timeout_ms: u64,
        max_retries: u32,
    }

    // Create shared config
    let config = Arc::new(Config {
        endpoint: String::from("https://api.example.com"),
        timeout_ms: 5000,
        max_retries: 3,
    });

    println!("Initial refcount: {}", Arc::strong_count(&config));

    // Store in map
    let map = ConcurrentMapCapsule::new();
    map.insert("api_config", config.clone());

    println!("After insert refcount: {}", Arc::strong_count(&config));

    // Read from map (borrows, doesn't clone)
    if let Some(stored_config) = map.get(&"api_config") {
        println!(
            "Config: endpoint={}, timeout={}ms, retries={}",
            stored_config.endpoint, stored_config.timeout_ms, stored_config.max_retries
        );
        println!(
            "During read refcount: {}",
            Arc::strong_count(&stored_config)
        );
    }

    println!("After read refcount: {}", Arc::strong_count(&config));

    // Multiple readers see same Arc
    if let Some(c1) = map.get(&"api_config") {
        if let Some(c2) = map.get(&"api_config") {
            println!("Same pointer? {}", Arc::as_ptr(&c1) == Arc::as_ptr(&c2));
        }
    }

    println!();
}

// ============================================================================
// Demo 3: String Keys and Values
// ============================================================================

fn demo_string_keys() {
    println!("--- Demo 3: String Keys and Values ---");

    // Map with String keys and String values
    let map = ConcurrentMapCapsule::new();

    let services = vec![
        ("web-server", "Running on port 8080"),
        ("database", "Connected to postgres"),
        ("cache", "Redis ready"),
        ("queue", "RabbitMQ connected"),
    ];

    for (key, value) in &services {
        map.insert(key.to_string(), value.to_string());
    }

    println!("Service status:");
    for (key, _) in &services {
        let key_string = key.to_string();
        if let Some(status) = map.get(&key_string) {
            println!("  {}: {}", key, status);
        }
    }

    println!();
}

// ============================================================================
// Demo 4: Complex Composite Types
// ============================================================================

fn demo_complex_types() {
    println!("--- Demo 4: Complex Composite Types ---");

    #[derive(Debug, Clone)]
    struct User {
        id: u64,
        name: String,
        email: String,
        roles: Vec<String>,
        metadata: Option<String>,
    }

    // Map of user IDs to Arc<User>
    let users_map = ConcurrentMapCapsule::new();

    let alice = Arc::new(User {
        id: 1,
        name: String::from("Alice"),
        email: String::from("alice@example.com"),
        roles: vec![String::from("admin"), String::from("developer")],
        metadata: Some(String::from("Premium user")),
    });

    let bob = Arc::new(User {
        id: 2,
        name: String::from("Bob"),
        email: String::from("bob@example.com"),
        roles: vec![String::from("developer")],
        metadata: None,
    });

    users_map.insert(alice.id, alice.clone());
    users_map.insert(bob.id, bob.clone());

    println!("Users in system: {}", users_map.len());

    // Query users
    if let Some(user) = users_map.get(&1) {
        println!(
            "User {}: name={}, roles={:?}",
            user.id, user.name, user.roles
        );
    }

    if let Some(user) = users_map.get(&2) {
        println!(
            "User {}: name={}, roles={:?}",
            user.id, user.name, user.roles
        );
    }

    println!();
}

// ============================================================================
// Demo 5: Real-World Service Registry
// ============================================================================

fn demo_service_registry() {
    println!("--- Demo 5: Real-World Service Registry ---");

    #[derive(Debug, Clone)]
    struct ServiceEndpoint {
        name: String,
        host: String,
        port: u16,
        protocol: String,
        healthy: bool,
        version: String,
    }

    // Service discovery registry
    let registry = ConcurrentMapCapsule::new();

    // Register services
    let services = vec![
        ServiceEndpoint {
            name: String::from("api-gateway"),
            host: String::from("10.0.1.10"),
            port: 8080,
            protocol: String::from("http"),
            healthy: true,
            version: String::from("1.2.0"),
        },
        ServiceEndpoint {
            name: String::from("auth-service"),
            host: String::from("10.0.1.20"),
            port: 8081,
            protocol: String::from("http"),
            healthy: true,
            version: String::from("2.1.3"),
        },
        ServiceEndpoint {
            name: String::from("data-processor"),
            host: String::from("10.0.1.30"),
            port: 8082,
            protocol: String::from("grpc"),
            healthy: false,
            version: String::from("0.9.1"),
        },
    ];

    for service in services {
        let name = service.name.clone();
        registry.insert(name, Arc::new(service));
    }

    println!("Registered services: {}", registry.len());

    // Query services
    let api_gateway_key = String::from("api-gateway");
    if let Some(svc) = registry.get(&api_gateway_key) {
        println!("\nService: {}", svc.name);
        println!("  Endpoint: {}://{}:{}", svc.protocol, svc.host, svc.port);
        println!("  Version: {}", svc.version);
        println!(
            "  Status: {}",
            if svc.healthy { "Healthy" } else { "Unhealthy" }
        );
    }

    let auth_key = String::from("auth-service");
    if let Some(svc) = registry.get(&auth_key) {
        println!("\nService: {}", svc.name);
        println!("  Endpoint: {}://{}:{}", svc.protocol, svc.host, svc.port);
        println!("  Version: {}", svc.version);
        println!(
            "  Status: {}",
            if svc.healthy { "Healthy" } else { "Unhealthy" }
        );
    }

    // Filter healthy services
    let data_proc_key = String::from("data-processor");
    if let Some(svc) = registry.get(&data_proc_key) {
        if !svc.healthy {
            println!("\nWarning: {} is unhealthy!", svc.name);
        }
    }

    println!();
}
