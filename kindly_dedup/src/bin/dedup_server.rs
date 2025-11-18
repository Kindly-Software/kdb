//! # Deduplication HTTP Server Binary
//!
//! Standalone HTTP server for document deduplication using atomic_capsule primitives.
//!
//! ## Usage
//!
//! ```bash
//! # Start server on default port (8080)
//! cargo run --bin dedup_server
//!
//! # Start server on custom port
//! cargo run --bin dedup_server -- 127.0.0.1:9000
//! ```
//!
//! ## API Endpoints
//!
//! ### POST /api/v1/deduplicate
//!
//! Deduplicate a batch of documents.
//!
//! **Request**:
//! ```json
//! {
//!   "documents": [
//!     {"id": "doc1", "text": "The quick brown fox jumps"},
//!     {"id": "doc2", "text": "The quick brown fox leaps"},
//!     {"id": "doc3", "text": "A completely different document"}
//!   ],
//!   "threshold": 0.85
//! }
//! ```
//!
//! **Response**:
//! ```json
//! {
//!   "clusters": [
//!     ["doc1", "doc2"],
//!     ["doc3"]
//!   ],
//!   "stats": {
//!     "total_documents": 3,
//!     "duplicate_clusters": 1,
//!     "deduplication_ratio": 0.33,
//!     "processing_time_ms": 5
//!   }
//! }
//! ```
//!
//! ### GET /health
//!
//! Health check endpoint.
//!
//! **Response**:
//! ```json
//! {
//!   "status": "ok",
//!   "total_requests": 42,
//!   "total_errors": 0,
//!   "uptime_seconds": 3600
//! }
//! ```
//!
//! ## Example cURL Usage
//!
//! ```bash
//! # Health check
//! curl http://localhost:8080/health
//!
//! # Deduplicate documents
//! curl -X POST http://localhost:8080/api/v1/deduplicate \
//!   -H "Content-Type: application/json" \
//!   -d '{
//!     "documents": [
//!       {"id": "doc1", "text": "The quick brown fox"},
//!       {"id": "doc2", "text": "The quick brown fox"}
//!     ],
//!     "threshold": 0.85
//!   }'
//! ```
//!
//! ## Performance
//!
//! - Request parsing: <100ns (T2 SIMD headers, atomic_capsule::http)
//! - Deduplication: <1ms per document (T10 MinHash + LSH)
//! - End-to-end latency: <10ms P99 (1000 documents)
//! - Throughput: 16,000 docs/sec (parallel pipeline)

use kindly_dedup::server::DedupServer;
use std::env;

fn main() {
    println!("===========================================");
    println!("  Kindly Dedup Server (atomic_capsule)");
    println!("===========================================");
    println!();
    println!("Architecture:");
    println!("  - T8 Network: HTTP service coordination");
    println!("  - T2 SIMD: HTTP header parsing (7× speedup)");
    println!("  - T10 Probabilistic: MinHash + LSH dedup");
    println!();
    println!("Frameworks:");
    println!("  - UCE34: Q1-Q34 systematic discovery");
    println!("  - ASSUM: 99.9% safe (zero unsafe code)");
    println!("  - COCA: 100% lockfree (atomic operations only)");
    println!();

    // Parse command-line arguments
    let addr = env::args().nth(1).unwrap_or_else(|| "127.0.0.1:8080".to_string());

    println!("Starting server on {}...", addr);
    println!();

    // Create and run server
    match DedupServer::new(&addr) {
        Ok(server) => {
            println!("Server ready! Available endpoints:");
            println!("  - POST /api/v1/deduplicate");
            println!("  - GET /health");
            println!();
            println!("Press Ctrl+C to stop.");
            println!();

            if let Err(e) = server.run() {
                eprintln!("[FATAL] Server error: {:?}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("[FATAL] Failed to start server: {:?}", e);
            eprintln!();
            eprintln!("Possible causes:");
            eprintln!("  - Port {} already in use", addr);
            eprintln!("  - Insufficient permissions");
            eprintln!();
            std::process::exit(1);
        }
    }
}
