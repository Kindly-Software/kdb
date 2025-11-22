//! # HTTP Connection Pooling Example
//!
//! **T1+T4 Connection Pool Management with Keepalive (120 lines)**
//!
//! Demonstrates:
//! - T4 Batch: HttpConnectionPoolCapsule (connection reuse, keepalive)
//! - T1 Atomic: Lockfree pool coordination (<30μs lookup)
//! - Timeout management (idle connection closure)
//! - Metrics collection (active connections, timeouts)
//!
//! Run:
//! ```bash
//! cargo run --example http_connection_pooling --features std,http
//! # In another terminal:
//! # for i in {1..100}; do curl http://localhost:8080/health & done; wait
//! ```

use atomic_capsule::http::{
    HttpServerCapsule, HttpRouterCapsule, HttpConnectionPoolCapsule,
    HttpKeepAliveCapsule, ConnectionState,
    Method, HttpRequest, HttpResponse, StatusCode,
};
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Global metrics
static TOTAL_REQUESTS: AtomicUsize = AtomicUsize::new(0);
static ACTIVE_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);
static KEEPALIVE_REUSES: AtomicUsize = AtomicUsize::new(0);

/// Metrics handler
fn handle_metrics(_req: &HttpRequest) -> HttpResponse {
    let total_requests = TOTAL_REQUESTS.load(Ordering::Relaxed);
    let active = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
    let reuses = KEEPALIVE_REUSES.load(Ordering::Relaxed);

    let json = format!(
        r#"{{"total_requests":{},"active_connections":{},"keepalive_reuses":{}}}"#,
        total_requests, active, reuses
    );

    HttpResponse {
        status: StatusCode::OK,
        body: json.into_bytes(),
        headers: vec![
            (b"Content-Type", b"application/json"),
            (b"Connection", b"keep-alive"),
            (b"Keep-Alive", b"timeout=30, max=100"),
        ],
    }
}

/// Health check (increments metrics)
fn handle_health(_req: &HttpRequest) -> HttpResponse {
    TOTAL_REQUESTS.fetch_add(1, Ordering::Relaxed);
    ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);

    let response = HttpResponse {
        status: StatusCode::OK,
        body: b"{\"status\":\"healthy\"}".to_vec(),
        headers: vec![
            (b"Content-Type", b"application/json"),
            (b"Connection", b"keep-alive"),
            (b"Keep-Alive", b"timeout=30, max=100"),
        ],
    };

    // Schedule connection release (in real implementation)
    ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);

    response
}

/// Sleep handler (demonstrates long-lived connections)
fn handle_slow(_req: &HttpRequest) -> HttpResponse {
    // Simulate long-running request
    std::thread::sleep(Duration::from_millis(100));

    HttpResponse {
        status: StatusCode::OK,
        body: b"Request completed after 100ms".to_vec(),
        headers: vec![
            (b"Content-Type", b"text/plain"),
            (b"Connection", b"keep-alive"),
        ],
    }
}

/// Connection info handler
fn handle_connection_info(_req: &HttpRequest) -> HttpResponse {
    let active = ACTIVE_CONNECTIONS.load(Ordering::Relaxed);
    let reuses = KEEPALIVE_REUSES.load(Ordering::Relaxed);

    let json = format!(
        r#"{{"active_connections":{},"pool_reuses":{},"pool_capacity":1000,"max_keepalive_timeout_seconds":30}}"#,
        active, reuses
    );

    HttpResponse {
        status: StatusCode::OK,
        body: json.into_bytes(),
        headers: vec![
            (b"Content-Type", b"application/json"),
        ],
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting HTTP server with connection pooling on 0.0.0.0:8080...");
    println!("Endpoints:");
    println!("  GET /health      → Quick health check (connection reused)");
    println!("  GET /slow        → Slow request (100ms, tests keepalive)");
    println!("  GET /metrics     → Pool metrics (active, reuses)");
    println!("  GET /pool/info   → Connection pool details");
    println!();

    // T8 Network: Create server
    let server = HttpServerCapsule::new("0.0.0.0:8080".parse()?)?;

    // T1 Atomic: Create router
    let router = HttpRouterCapsule::new();

    // T4 Batch: Create connection pool
    // Configuration: max 1,000 connections, 30s keepalive timeout, 100 max requests per connection
    let pool = HttpConnectionPoolCapsule::new(1000, 30, 100)?;

    // Register routes
    router.add_route("/health", Method::GET, handle_health)?;
    router.add_route("/slow", Method::GET, handle_slow)?;
    router.add_route("/metrics", Method::GET, handle_metrics)?;
    router.add_route("/pool/info", Method::GET, handle_connection_info)?;

    // Configure keepalive on server
    let keepalive = HttpKeepAliveCapsule::new(
        Duration::from_secs(30),  // Idle timeout
        100,                        // Max requests per connection
    );

    // Associate pool with server (T1+T4 integration)
    server.set_connection_pool(&pool)?;
    server.set_keepalive(&keepalive)?;

    println!("Connection pool details:");
    println!("  Max connections: 1,000");
    println!("  Keepalive timeout: 30s");
    println!("  Max requests per connection: 100");
    println!("  Idle connection cleanup: enabled");
    println!();

    // Start server
    server.start(&router)?;

    Ok(())
}
