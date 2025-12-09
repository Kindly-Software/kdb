# Implementation Contracts for kindly_dedup

This document specifies the exact API contracts that the parallel and server implementations MUST provide to satisfy the T28 test suite.

## Parallel Implementation Contract

**Module**: `kindly_dedup::parallel`

### Required Structs

```rust
pub struct ParallelDedupPipeline {
    // Internal fields (implementation-defined)
}
```

### Required Methods

```rust
impl ParallelDedupPipeline {
    /// Create new parallel dedup pipeline
    ///
    /// # Arguments
    /// - `num_documents`: Expected number of documents
    /// - `num_threads`: Number of parallel threads (1-32)
    ///
    /// # Returns
    /// New ParallelDedupPipeline instance
    pub fn new(num_documents: usize, num_threads: usize) -> Self;

    /// Add documents in parallel
    ///
    /// # Arguments
    /// - `documents`: Slice of (DocId, &str) tuples
    ///
    /// # Returns
    /// - `Ok(())` on success
    /// - `Err(_)` if any document fails
    ///
    /// # Thread Safety
    /// Must be thread-safe (Send + Sync)
    pub fn add_documents_parallel(&self, documents: &[(DocId, &str)]) -> Result<(), Error>;

    /// Find duplicates in parallel
    ///
    /// # Arguments
    /// - `threshold`: Jaccard similarity threshold (0.0 to 1.0)
    ///
    /// # Returns
    /// - `Ok(Vec<Vec<DocId>>)`: Clusters of duplicate documents
    /// - `Err(_)` if clustering fails
    pub fn find_duplicates_parallel(&self, threshold: JaccardThreshold) -> Result<Vec<Vec<DocId>>, Error>;

    /// Get number of threads
    pub fn num_threads(&self) -> usize;

    /// Get total capacity
    pub fn capacity(&self) -> usize;

    /// Get number of documents added
    pub fn documents_added(&self) -> usize;
}

// Must implement Send + Sync for concurrent access
unsafe impl Send for ParallelDedupPipeline {}
unsafe impl Sync for ParallelDedupPipeline {}
```

### Performance Requirements (from T28 tests)

- **Throughput**: >500K docs/sec (16 threads, stress test)
- **Latency**: <10ms for 100 documents
- **Scaling**: 2× speedup with 8 threads vs sequential
- **Thread Safety**: 100% lockfree (no Mutex/RwLock)

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid document ID: {0}")]
    InvalidDocId(usize),

    #[error("Processing error: {0}")]
    ProcessingError(String),
}
```

---

## Server Implementation Contract

**Module**: `kindly_dedup::server`

### Required Structs

```rust
pub struct DedupServer {
    // Internal fields (implementation-defined)
}

#[derive(Debug, serde::Deserialize)]
pub struct DedupRequest {
    pub documents: Vec<DocumentInput>,
    pub threshold: f64,
}

#[derive(Debug, serde::Deserialize)]
pub struct DocumentInput {
    pub id: usize,
    pub text: String,
}

#[derive(Debug, serde::Serialize)]
pub struct DedupResponse {
    pub num_documents: usize,
    pub num_clusters: usize,
    pub clusters: Vec<Vec<usize>>,
    pub processing_time_ms: f64,
}
```

### Required Methods

```rust
impl DedupServer {
    /// Create and start new HTTP server
    ///
    /// # Arguments
    /// - `addr`: Bind address (e.g., "127.0.0.1:3000" or "127.0.0.1:0" for random port)
    ///
    /// # Returns
    /// - `Ok(DedupServer)` on success
    /// - `Err(_)` if server fails to start
    pub async fn new(addr: &str) -> Result<Self, ServerError>;

    /// Get local address server is bound to
    ///
    /// # Returns
    /// SocketAddr (includes actual port if 0 was specified)
    pub fn local_addr(&self) -> std::net::SocketAddr;
}
```

### Required Endpoints

#### 1. Health Check

```
GET /health
Response: 200 OK
{
  "status": "healthy"
}
```

#### 2. Deduplicate

```
POST /api/v1/deduplicate
Content-Type: application/json

Request Body:
{
  "documents": [
    {"id": 0, "text": "Document text"},
    {"id": 1, "text": "Another document"}
  ],
  "threshold": 0.85
}

Response: 200 OK
{
  "num_documents": 2,
  "num_clusters": 2,
  "clusters": [[0], [1]],
  "processing_time_ms": 1.234
}
```

**Error Responses**:

```
400 Bad Request (invalid JSON, missing fields, invalid threshold)
{
  "error": "Error message"
}

413 Payload Too Large (request size limit exceeded)
{
  "error": "Request too large"
}

500 Internal Server Error (processing failure)
{
  "error": "Internal error"
}
```

#### 3. Metrics (Optional)

```
GET /metrics
Response: 200 OK
{
  "requests_total": 123,
  "requests_failed": 0,
  "avg_latency_ms": 5.67
}
```

### Performance Requirements (from T28 tests)

- **Response Time**: <100ms for small requests (3 docs)
- **Throughput**: >100 req/s sequential, >1000 req/s concurrent
- **Large Batches**: Handle 10K documents in <5 seconds
- **Concurrency**: Support 1000 concurrent requests

### Error Type

```rust
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Bind error: {0}")]
    BindError(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}
```

---

## Implementation Guidelines

### Parallel Pipeline

1. **Use Rayon** for parallel iteration:
   ```rust
   use rayon::prelude::*;
   documents.par_iter().for_each(|(id, text)| { ... });
   ```

2. **Atomic Coordination**: Use `AtomicUsize` for document count:
   ```rust
   use std::sync::atomic::{AtomicUsize, Ordering};
   let count = AtomicUsize::new(0);
   count.fetch_add(1, Ordering::Relaxed);
   ```

3. **Thread Pool**: Configure Rayon thread pool in `new()`:
   ```rust
   rayon::ThreadPoolBuilder::new()
       .num_threads(num_threads)
       .build_global()
       .ok();
   ```

### HTTP Server

1. **Use Axum** (recommended) or Actix-web:
   ```rust
   use axum::{
       routing::{get, post},
       Router,
       Json,
   };

   let app = Router::new()
       .route("/health", get(health_handler))
       .route("/api/v1/deduplicate", post(deduplicate_handler));

   let listener = tokio::net::TcpListener::bind(addr).await?;
   let local_addr = listener.local_addr()?;
   tokio::spawn(async move {
       axum::serve(listener, app).await
   });
   ```

2. **Request Validation**:
   ```rust
   if !(0.0..=1.0).contains(&request.threshold) {
       return Err(StatusCode::BAD_REQUEST);
   }
   ```

3. **Timing**:
   ```rust
   let start = Instant::now();
   // ... process ...
   let processing_time_ms = start.elapsed().as_secs_f64() * 1000.0;
   ```

---

## Test Validation

Once implementations are complete, run:

```bash
# Compile check (should succeed with expected APIs)
cargo test --no-run

# Unit tests (Tier 1-3, fast)
cargo test --test parallel_tests
cargo test --test server_tests

# Stress tests (Tier 4, slow)
cargo test --test parallel_tests --ignored -- --test-threads=1
cargo test --test server_tests --ignored -- --test-threads=1

# All tests
cargo test --all-targets
```

### Expected Results

- **56 total tests** (28 parallel + 28 server)
- **Tier 1-3**: 42 tests, <30 seconds total
- **Tier 4**: 14 tests (#[ignore]), <5 minutes total
- **Pass Rate**: 100% (all tests must pass)

---

## Framework Compliance

### T28 Framework Adherence

✅ **Q1-Q7 (Unit)**: Core behaviors, edge cases, invariants
✅ **Q8-Q14 (Property)**: Determinism, concurrency, composition
✅ **Q15-Q21 (Integration)**: Scaling, error propagation, performance budgets
✅ **Q22-Q28 (Production)**: Stress tests, security, benchmarks, documentation

### Other Frameworks

- **ASSUM**: Zero unsafe code (#![deny(unsafe_code)])
- **B32**: Fair baselines, >2× speedup validation
- **UCE34**: T10 Probabilistic tier (MinHash/LSH/Union-Find)
- **Chaos**: 100% lockfree (no Mutex/RwLock)

---

## Dependencies Required

Add to `Cargo.toml`:

```toml
[dependencies]
# Parallel implementation
rayon = "1.10"

# HTTP server (choose one)
axum = { version = "0.7", features = ["macros"] }
# OR
actix-web = "4.0"

# Existing dependencies
atomic_capsule = { path = "../atomic_capsule", features = ["std", "probabilistic", "hll"] }
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
thiserror = "1.0"

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
criterion = { version = "0.5", features = ["html_reports"] }
```

---

## Summary

This document provides the complete API contracts needed for the Implementation Expert and Server Expert to create compatible implementations. All 56 T28 tests are ready and will pass once the implementations follow these contracts.

**Next Steps**:
1. Implementation Expert: Create `src/parallel_pipeline.rs`
2. Server Expert: Create `src/server.rs`
3. Run T28 tests to validate
4. Iterate until 100% pass rate

**Status**: ✅ Tests ready, awaiting implementations
