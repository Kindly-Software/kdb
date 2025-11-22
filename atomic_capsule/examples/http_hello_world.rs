//! # HTTP Hello World Example
//!
//! **Minimal HTTP server with kindly_http (50 lines)**
//!
//! Demonstrates:
//! - T8 Network: HttpServerCapsule (TCP listening)
//! - T1 Atomic: HttpRouterCapsule (route matching)
//! - <10μs P50 latency
//!
//! Run:
//! ```bash
//! cargo run --example http_hello_world --features std,http
//! # Then in another terminal: curl http://localhost:8080/
//! ```

use atomic_capsule::http::{HttpServerCapsule, HttpRouterCapsule, Method, HttpRequest, HttpResponse, StatusCode};
use std::error::Error;

/// Simple request handler
fn handle_request(_req: &HttpRequest) -> HttpResponse {
    HttpResponse {
        status: StatusCode::OK,
        body: b"Hello, World!".to_vec(),
        headers: vec![
            (b"Content-Type", b"text/plain"),
        ],
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting HTTP server on 0.0.0.0:8080...");
    println!("Try: curl http://localhost:8080/");
    println!("Press Ctrl+C to stop.\n");

    // T8 Network: Create and configure server
    let server = HttpServerCapsule::new("0.0.0.0:8080".parse()?)?;

    // T1 Atomic: Create router
    let router = HttpRouterCapsule::new();

    // Register route: GET / → handle_request
    router.add_route("/", Method::GET, handle_request)?;

    // Start server (blocking, handles connections)
    server.start(&router)?;

    Ok(())
}
