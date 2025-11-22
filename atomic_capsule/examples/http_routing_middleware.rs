//! # HTTP Routing and Middleware Example
//!
//! **T1 routing + T1 middleware with logging and CORS (150 lines)**
//!
//! Demonstrates:
//! - T1 Atomic: Static, dynamic, and wildcard routes
//! - T1 Atomic: Middleware pipeline (logging, CORS)
//! - <100ns route lookup
//! - Composable request/response processing
//!
//! Run:
//! ```bash
//! cargo run --example http_routing_middleware --features std,http
//! # Try: curl http://localhost:8080/
//! #      curl http://localhost:8080/api/users/123
//! #      curl http://localhost:8080/health
//! ```

use atomic_capsule::http::{
    HttpServerCapsule, HttpRouterCapsule, HttpMiddlewareCapsule,
    Method, HttpRequest, HttpResponse, StatusCode, LogLevel,
};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Request counter for metrics
static REQUEST_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Root handler
fn handle_root(_req: &HttpRequest) -> HttpResponse {
    HttpResponse {
        status: StatusCode::OK,
        body: b"Welcome to kindly_http! Try /api/users/123 or /health".to_vec(),
        headers: vec![
            (b"Content-Type", b"text/plain"),
        ],
    }
}

/// API handler (dynamic route)
fn handle_get_user(req: &HttpRequest) -> HttpResponse {
    // Extract user ID from path (e.g., /api/users/123 → id=123)
    let user_id = match req.path.split('/').nth(3) {
        Some(id) => id,
        None => "unknown",
    };

    let body = format!("User ID: {}", user_id).into_bytes();

    HttpResponse {
        status: StatusCode::OK,
        body,
        headers: vec![
            (b"Content-Type", b"application/json"),
        ],
    }
}

/// Health check handler
fn handle_health(_req: &HttpRequest) -> HttpResponse {
    REQUEST_COUNT.fetch_add(1, Ordering::Relaxed);

    let count = REQUEST_COUNT.load(Ordering::Relaxed);
    let body = format!("{{\"status\":\"healthy\",\"requests\":{}}}", count).into_bytes();

    HttpResponse {
        status: StatusCode::OK,
        body,
        headers: vec![
            (b"Content-Type", b"application/json"),
        ],
    }
}

/// Logging middleware (logs all requests)
fn logging_middleware(req: &HttpRequest) -> HttpResponse {
    println!("[LOG] {} {}", req.method.as_str(), req.path);
    // Note: In production, use HttpMiddlewareCapsule::add_middleware()
    // This is simplified for example purposes
    HttpResponse {
        status: StatusCode::OK,
        body: vec![],
        headers: vec![],
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting HTTP server with routing + middleware on 0.0.0.0:8080...");
    println!("Routes:");
    println!("  GET /               → Home page");
    println!("  GET /api/users/:id  → User details");
    println!("  GET /health         → Health check");
    println!();

    // T8 Network: Create server
    let server = HttpServerCapsule::new("0.0.0.0:8080".parse()?)?;

    // T1 Atomic: Create router
    let router = HttpRouterCapsule::new();

    // Register routes (static first, then dynamic)
    router.add_route("/", Method::GET, handle_root)?;
    router.add_route("/health", Method::GET, handle_health)?;

    // Dynamic route: /api/users/:id
    router.add_route("/api/users/*", Method::GET, handle_get_user)?;

    // T1 Atomic: Create middleware pipeline
    let middleware = HttpMiddlewareCapsule::new();

    // Add logging middleware (runs for all requests)
    middleware.add_middleware(
        "logging",
        Some(&["*"]),  // Apply to all routes
        logging_middleware,
        LogLevel::Info,
    )?;

    // Add CORS headers middleware
    middleware.add_cors_middleware(
        Some(&["http://localhost:3000"]),  // Allowed origins
        Some(&["GET", "POST", "OPTIONS"]), // Allowed methods
        None,                              // All headers allowed
    )?;

    // Start server
    server.start(&router)?;

    Ok(())
}
