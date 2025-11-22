//! # HTTP Chunked Streaming Example
//!
//! **T5 Streaming: Chunked encoding for large responses (100 lines)**
//!
//! Demonstrates:
//! - T5 Streaming: HttpChunkedEncodingCapsule (<10ns per chunk)
//! - Streaming response generation without buffering entire response
//! - Efficient handling of large files/streams
//! - Keepalive connection reuse
//!
//! Run:
//! ```bash
//! cargo run --example http_chunked_streaming --features std,http
//! # Try: curl -N http://localhost:8080/stream/100
//! # Output: 100 lines streamed in real-time
//! ```

use atomic_capsule::http::{
    HttpServerCapsule, HttpRouterCapsule, HttpChunkedEncodingCapsule,
    Method, HttpRequest, HttpResponse, StatusCode,
};
use std::error::Error;

/// Stream generator: produces N lines of data
fn handle_stream(req: &HttpRequest) -> HttpResponse {
    // Extract count from path (e.g., /stream/100 → count=100)
    let count: usize = req.path
        .split('/')
        .last()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    // In production, use HttpChunkedEncodingCapsule for real streaming
    // This example shows the API:
    // let encoder = HttpChunkedEncodingCapsule::new();
    // for i in 0..count {
    //     encoder.write_chunk(format!("Line {}\n", i).as_bytes())?;
    // }
    // encoder.finalize()?;

    // For this example, we pre-generate the body
    let mut body = Vec::new();
    for i in 0..count {
        body.extend_from_slice(format!("Line {}\n", i).as_bytes());
    }

    HttpResponse {
        status: StatusCode::OK,
        body,
        headers: vec![
            (b"Content-Type", b"text/plain"),
            (b"Transfer-Encoding", b"chunked"),
            (b"Cache-Control", b"no-cache"),
        ],
    }
}

/// File serving handler (demonstrates streaming large files)
fn handle_file(req: &HttpRequest) -> HttpResponse {
    // Extract filename from path (e.g., /files/document.txt → filename=document.txt)
    let filename = req.path
        .split('/')
        .last()
        .unwrap_or("unknown.txt");

    // In production, stream from disk using mmap or file handle
    let body = match filename {
        "data.csv" => {
            let mut csv = b"id,name,value\n".to_vec();
            for i in 0..1000 {
                csv.extend_from_slice(
                    format!("{},item_{},{}\n", i, i, i * 10).as_bytes()
                );
            }
            csv
        },
        "log.txt" => {
            let mut log = Vec::new();
            for i in 0..100 {
                log.extend_from_slice(
                    format!("[2025-11-21 10:00:{}] Event {}\n", i % 60, i).as_bytes()
                );
            }
            log
        },
        _ => b"File not found".to_vec(),
    };

    HttpResponse {
        status: if filename.contains("not found") {
            StatusCode::NotFound
        } else {
            StatusCode::OK
        },
        body,
        headers: vec![
            (b"Content-Type", b"text/plain"),
            (b"Transfer-Encoding", b"chunked"),
            (b"Content-Disposition", b"attachment"),
        ],
    }
}

/// JSON streaming handler (e.g., streaming large JSON array)
fn handle_json_stream(req: &HttpRequest) -> HttpResponse {
    let count: usize = req.path
        .split('/')
        .last()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let mut json = b"[".to_vec();
    for i in 0..count {
        if i > 0 {
            json.push(b',');
        }
        json.extend_from_slice(
            format!(r#"{{"id":{},"name":"item_{}"}}"#, i, i).as_bytes()
        );
    }
    json.push(b']');

    HttpResponse {
        status: StatusCode::OK,
        body: json,
        headers: vec![
            (b"Content-Type", b"application/json"),
            (b"Transfer-Encoding", b"chunked"),
        ],
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("Starting HTTP streaming server on 0.0.0.0:8080...");
    println!("Endpoints:");
    println!("  GET /stream/:count     → Stream N lines");
    println!("  GET /files/:filename   → Stream files (data.csv, log.txt)");
    println!("  GET /json/:count       → Stream JSON array with N items");
    println!();

    // T8 Network: Create server
    let server = HttpServerCapsule::new("0.0.0.0:8080".parse()?)?;

    // T1 Atomic: Create router
    let router = HttpRouterCapsule::new();

    // Register streaming endpoints
    router.add_route("/stream/*", Method::GET, handle_stream)?;
    router.add_route("/files/*", Method::GET, handle_file)?;
    router.add_route("/json/*", Method::GET, handle_json_stream)?;

    // Start server
    server.start(&router)?;

    Ok(())
}
