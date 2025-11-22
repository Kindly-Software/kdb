//! HttpRouterCapsule Demonstration
//!
//! Demonstrates lockfree HTTP route matching with static, dynamic, and wildcard patterns.

use atomic_capsule::http::{HttpRouterCapsule, Method};
use std::collections::HashMap;

fn main() {
    println!("=== HttpRouterCapsule Demo ===\n");

    // Create router with capacity for 100 routes
    let router = HttpRouterCapsule::new(100).expect("Failed to create router");
    println!("Created router with capacity 100\n");

    // Define some handlers
    fn handle_users(req: &atomic_capsule::http::router::Request, _params: &HashMap<std::borrow::Cow<'static, str>, std::borrow::Cow<'static, str>>) -> atomic_capsule::http::router::Response {
        println!("  [Handler] GET /api/users");
        atomic_capsule::http::router::Response {
            status: 200,
            body: b"Users list".to_vec(),
        }
    }

    fn handle_user_detail(req: &atomic_capsule::http::router::Request, params: &HashMap<std::borrow::Cow<'static, str>, std::borrow::Cow<'static, str>>) -> atomic_capsule::http::router::Response {
        if let Some(id) = params.get("id") {
            println!("  [Handler] GET /api/users/{} (id from URL)", id);
        }
        atomic_capsule::http::router::Response {
            status: 200,
            body: b"User detail".to_vec(),
        }
    }

    fn handle_posts(req: &atomic_capsule::http::router::Request, _params: &HashMap<std::borrow::Cow<'static, str>, std::borrow::Cow<'static, str>>) -> atomic_capsule::http::router::Response {
        println!("  [Handler] POST /api/users");
        atomic_capsule::http::router::Response {
            status: 201,
            body: b"User created".to_vec(),
        }
    }

    fn handle_not_found(req: &atomic_capsule::http::router::Request, _params: &HashMap<std::borrow::Cow<'static, str>, std::borrow::Cow<'static, str>>) -> atomic_capsule::http::router::Response {
        println!("  [Handler] Wildcard (404)");
        atomic_capsule::http::router::Response {
            status: 404,
            body: b"Not found".to_vec(),
        }
    }

    // Test 1: Add static routes
    println!("--- Test 1: Static Routes ---");
    router
        .add_route(Method::GET, "/api/users", handle_users)
        .expect("Failed to add GET /api/users");
    println!("✓ Added GET /api/users");

    router
        .add_route(Method::POST, "/api/users", handle_posts)
        .expect("Failed to add POST /api/users");
    println!("✓ Added POST /api/users");
    println!("  Route count: {}\n", router.route_count());

    // Test 2: Dynamic route with parameter
    println!("--- Test 2: Dynamic Route with Parameter ---");
    router
        .add_route(Method::GET, "/api/users/:id", handle_user_detail)
        .expect("Failed to add GET /api/users/:id");
    println!("✓ Added GET /api/users/:id");
    println!("  Route count: {}\n", router.route_count());

    // Test 3: Wildcard fallback
    println!("--- Test 3: Wildcard Fallback ---");
    router
        .set_wildcard(handle_not_found)
        .expect("Failed to set wildcard");
    println!("✓ Set wildcard handler\n");

    // Test 4: Match static route
    println!("--- Test 4: Match Static Route ---");
    println!("  Matching: GET /api/users");
    if let Some((handler, params)) = router.match_route(Method::GET, "/api/users") {
        let response = handler(
            &atomic_capsule::http::router::Request {
                method: Method::GET,
                path: "/api/users",
            },
            &params,
        );
        println!("  Response status: {}", response.status);
        println!("  ✓ Static route matched\n");
    }

    // Test 5: Match dynamic route with parameter
    println!("--- Test 5: Match Dynamic Route ---");
    println!("  Matching: GET /api/users/42");
    if let Some((handler, params)) = router.match_route(Method::GET, "/api/users/42") {
        println!("  Parameters extracted:");
        for (key, value) in &params {
            println!("    {} = {}", key, value);
        }
        let response = handler(
            &atomic_capsule::http::router::Request {
                method: Method::GET,
                path: "/api/users/42",
            },
            &params,
        );
        println!("  Response status: {}", response.status);
        println!("  ✓ Dynamic route matched\n");
    }

    // Test 6: Different HTTP method
    println!("--- Test 6: Match Different Method ---");
    println!("  Matching: POST /api/users");
    if let Some((handler, params)) = router.match_route(Method::POST, "/api/users") {
        let response = handler(
            &atomic_capsule::http::router::Request {
                method: Method::POST,
                path: "/api/users",
            },
            &params,
        );
        println!("  Response status: {}", response.status);
        println!("  ✓ POST route matched\n");
    }

    // Test 7: Wildcard fallback for non-existent route
    println!("--- Test 7: Wildcard Fallback ---");
    println!("  Matching: GET /api/nonexistent");
    if let Some((handler, params)) = router.match_route(Method::GET, "/api/nonexistent") {
        let response = handler(
            &atomic_capsule::http::router::Request {
                method: Method::GET,
                path: "/api/nonexistent",
            },
            &params,
        );
        println!("  Response status: {}", response.status);
        println!("  ✓ Wildcard matched\n");
    }

    // Test 8: Metrics
    println!("--- Test 8: Metrics ---");
    let (static_hits, dynamic_hits, wildcard_hits, misses) = router.get_metrics();
    println!("  Static hits: {}", static_hits);
    println!("  Dynamic hits: {}", dynamic_hits);
    println!("  Wildcard hits: {}", wildcard_hits);
    println!("  Misses: {}\n", misses);

    println!("=== All Tests Complete ===");
}
