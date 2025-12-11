//! KDB MCP Client Bridge - T1+T3+T4+T5+T6 Atomic Capsule
//!
//! stdio->HTTP bridge with Phase 1+2+3+4 resilience, performance, and protection features:
//!
//! ## Phase 1: Resilience
//! - McpMetricsCapsule (T1+T3): Q16.16 fixed-point latency tracking
//! - MutableRetryConfig (T1): Exponential backoff on network errors
//! - MutableCircuitBreaker (T1): Fast-fail when service unavailable
//!
//! ## Phase 2: Performance
//! - MutableResponseCache (T1+T5): LRU response caching with TTL
//! - ClientIdempotencyCache (T1): Request deduplication
//!
//! ## Phase 3: Advanced Resilience
//! - OfflineQueueCapsule (T1+T5): FIFO queue for offline mode
//! - RequestBatcherCapsule (T1+T4): Batch read-only requests
//! - ConnectionState (T1): Track online/offline transitions
//!
//! ## Phase 4: Protection (P0 Critical)
//! - P0ProtectionLayer (T1+T6): Anti-debug, emulator detection, license validation
//! - SelfDestructHandler (T1): Tamper response with UCE35 Q35 compliance
//!
//! # UCE35 Framework Compliance
//!
//! - **Q10 Tier**: T1+T3+T4+T5 Mixed (Atomic + Fixed-Point + Batch + Streaming)
//! - **Q11 Rust**: 100% safe Rust, minimal unsafe (ureq)
//! - **Q28 Interface**: Simple stdio <-> HTTP bridge
//! - **Q33 Lockfree**: Stats/dedup/connection state use AtomicU64
//! - **Q34 Audit**: All requests logged to stderr with timestamps
//!
//! # Architecture
//!
//! ```text
//! stdin (JSON-RPC) --> [Connection Check] --> [Offline Queue?]
//!                           |                       |
//!                           v                       v
//!                       (online?)              (queue for later)
//!                           |
//!                           v
//!                     [Dedup Check] --> [Batch Check] --> [Cache Check]
//!                           |                 |                  |
//!                           v                 v                  v
//!                      (duplicate?)      (batchable?)       (cache hit?)
//!                           |                 |                  |
//!                           v                 v                  v
//!                         skip         (accumulate)        return cached
//!                                            |                   |
//!                                            v                   v
//!                                     [Flush Batch?] --> [CircuitBreaker]
//!                                            |                   |
//!                                            v                   v
//!                                         (size/timeout)     (open?)
//!                                            |                   |
//!                                            v                   v
//!                                       batch send          fast-fail
//!                                            |                   |
//!                                            v                   v
//!                                     [Retry Loop] --> HTTP POST
//!                                            |
//!                                            v
//! stdout (JSON-RPC) <-- [Metrics] <-- [Cache Store] <-- Response
//! ```
//!
//! # Performance
//!
//! - Cache hit path: <500ns (FNV-1a hash + lookup)
//! - Dedup check: <50ns (atomic lookup)
//! - Batch accumulate: <30ns (RwLock + push)
//! - Offline queue: <50ns (RwLock + enqueue)
//! - Connection state check: <10ns (atomic load)
//! - Metrics recording: <50ns (atomic ops)
//! - Circuit breaker check: <10ns (atomic load)
//! - Retry delay: configurable (100ms-10s exponential)
//! - Total overhead: <700ns per cache hit (excluding network)
//!
//! # Environment Variables
//!
//! ## Required
//! - `KDB_LICENSE_KEY`: License key for authentication
//!
//! ## Optional
//! - `KDB_MCP_URL`: MCP endpoint (default: https://mcp.kindly.software/mcp)
//! - `KDB_TIMEOUT`: Request timeout in seconds (default: 30)
//!
//! ## Retry Configuration
//! - `KDB_RETRY_MAX`: Maximum retries (default: 5)
//! - `KDB_RETRY_BACKOFF`: Strategy (immediate|light|standard|persistent, default: standard)
//!
//! ## Circuit Breaker Configuration
//! - `KDB_CB_FAILURE_THRESHOLD`: Failures before open (default: 5)
//! - `KDB_CB_RECOVERY_TIMEOUT`: Recovery timeout seconds (default: 60)
//! - `KDB_CB_HALF_OPEN_SUCCESS`: Successes to close (default: 3)
//!
//! ## Cache Configuration (Phase 2)
//! - `KDB_CACHE_TTL`: Cache TTL in seconds (default: 300 = 5 minutes)
//! - `KDB_CACHE_SIZE`: Max cache entries (default: 256)
//! - `KDB_DEDUP_ENABLED`: Enable deduplication (default: true)
//! - `KDB_DEDUP_TIMEOUT`: Dedup window in seconds (default: 30)
//!
//! ## Offline Queue Configuration (Phase 3)
//! - `KDB_OFFLINE_QUEUE_ENABLED`: Enable offline queueing (default: true)
//! - `KDB_OFFLINE_QUEUE_SIZE`: Max queue slots (default: 4096)
//! - `KDB_OFFLINE_OVERFLOW`: Overflow policy (drop_oldest|drop_newest|reject, default: drop_oldest)
//!
//! ## Batching Configuration (Phase 3)
//! - `KDB_BATCH_ENABLED`: Enable request batching (default: true)
//! - `KDB_BATCH_SIZE`: Max batch size before flush (default: 10)
//! - `KDB_BATCH_TIMEOUT_MS`: Max wait time before flush (default: 50ms)

use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

// Phase 1 resilience capsules
use kdb_mcp::client::{
    McpMetricsCapsule,
    MutableRetryConfig,
    MutableCircuitBreaker,
    CircuitBreakerError,
    is_retryable_error,
};

// Phase 2 performance capsules
use kdb_mcp::client::{
    MutableResponseCache,
    ResponseCacheConfig,
    IdempotencyCacheCapsule,
    hash_request,
};

// Phase 3 advanced resilience capsules (conditionally compiled)
#[cfg(feature = "client-offline")]
use kdb_mcp::client::{OfflineQueueCapsule, QueuedRequest};

#[cfg(feature = "client-batching")]
use kdb_mcp::client::{RequestBatcherCapsule, BatchableRequest};

// Phase 4: Protection capsules (conditionally compiled)
#[cfg(feature = "client-protection")]
use kdb_mcp::client::{
    P0ProtectionLayer, ProtectionError,
    SelfDestructHandler, TamperReason,
};

/// Log to stderr with timestamp (Q34 audit trail)
macro_rules! log {
    ($($arg:tt)*) => {{
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        eprintln!("[{}.{:03}] {}", now.as_secs(), now.subsec_millis(), format!($($arg)*));
    }};
}

/// Bridge configuration - 64-byte aligned for cache efficiency
#[repr(C, align(64))]
struct BridgeConfig {
    /// MCP endpoint URL
    url: String,
    /// License key for authentication
    license_key: String,
    /// Request timeout in seconds
    timeout_secs: u64,
    /// Padding for cache alignment
    _pad: [u8; 8],
}

/// Get current Unix timestamp in seconds
#[inline]
fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Connection state tracking - T1 Atomic (64-byte aligned)
///
/// Tracks online/offline status based on consecutive failures.
/// Transitions to offline mode after 3 consecutive failures,
/// allowing requests to be queued for later replay.
///
/// **UCE35 Compliance**:
/// - Q10 Tier: T1 Atomic (all state via AtomicU64/AtomicBool)
/// - Q33 Lockfree: 100% lockfree operations
/// - Cache-aligned (64B) to prevent false sharing
#[repr(C, align(64))]
struct ConnectionState {
    /// Whether connection is currently online
    is_online: AtomicBool,
    /// Number of consecutive failures
    consecutive_failures: AtomicU64,
    /// Unix timestamp of last successful request
    last_success_unix: AtomicU64,
    /// Total times we've gone offline
    total_offline_transitions: AtomicU64,
    /// Padding for cache alignment
    _pad: [u8; 32],
}

impl ConnectionState {
    /// Create new connection state (starts online)
    const fn new() -> Self {
        Self {
            is_online: AtomicBool::new(true),
            consecutive_failures: AtomicU64::new(0),
            last_success_unix: AtomicU64::new(0),
            total_offline_transitions: AtomicU64::new(0),
            _pad: [0u8; 32],
        }
    }

    /// Mark a successful request - resets failure counter, transitions to online
    fn mark_success(&self) {
        let was_offline = !self.is_online.load(Ordering::Acquire);

        self.is_online.store(true, Ordering::Release);
        self.consecutive_failures.store(0, Ordering::Relaxed);
        self.last_success_unix.store(current_unix_timestamp(), Ordering::Relaxed);

        if was_offline {
            log!("[Client-Connection] Connection restored, back online");
        }
    }

    /// Mark a failed request - increments failure counter, may transition to offline
    ///
    /// Transitions to offline after 3 consecutive failures.
    fn mark_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;

        if failures >= 3 && self.is_online.load(Ordering::Acquire) {
            self.is_online.store(false, Ordering::Release);
            self.total_offline_transitions.fetch_add(1, Ordering::Relaxed);
            log!("[Client-Connection] {} consecutive failures, entering offline mode", failures);
        }
    }

    /// Check if currently online
    #[inline]
    fn is_online(&self) -> bool {
        self.is_online.load(Ordering::Acquire)
    }

    /// Get consecutive failure count
    #[inline]
    fn failure_count(&self) -> u64 {
        self.consecutive_failures.load(Ordering::Relaxed)
    }

    /// Get total offline transitions
    #[inline]
    fn total_offline_transitions(&self) -> u64 {
        self.total_offline_transitions.load(Ordering::Relaxed)
    }
}

/// Create JSON-RPC error response
fn json_error(id: Option<&str>, code: i32, message: &str) -> String {
    let id_str = id.map(|s| format!("\"{}\"", s)).unwrap_or_else(|| "null".to_string());
    format!(
        r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":{},"message":"{}"}}}}"#,
        id_str, code, message
    )
}

/// Extract "id" field from JSON-RPC request (simple parser, no serde)
fn extract_id(json: &str) -> Option<String> {
    let id_start = json.find("\"id\"")?;
    let after_id = &json[id_start + 4..];

    let colon_pos = after_id.find(':')?;
    let after_colon = after_id[colon_pos + 1..].trim_start();

    if after_colon.starts_with('"') {
        let end_quote = after_colon[1..].find('"')?;
        Some(after_colon[1..=end_quote].to_string())
    } else {
        let end = after_colon
            .find(|c: char| c == ',' || c == '}' || c.is_whitespace())
            .unwrap_or(after_colon.len());
        Some(after_colon[..end].to_string())
    }
}

/// Extract "method" field from JSON-RPC request
///
/// # Arguments
/// - `json`: Raw JSON-RPC request string
///
/// # Returns
/// - `Some(method)`: Extracted method name
/// - `None`: Method field not found
fn extract_method(json: &str) -> Option<String> {
    let method_start = json.find("\"method\"")?;
    let after_method = &json[method_start + 8..];

    let colon_pos = after_method.find(':')?;
    let after_colon = after_method[colon_pos + 1..].trim_start();

    if after_colon.starts_with('"') {
        let end_quote = after_colon[1..].find('"')?;
        Some(after_colon[1..=end_quote].to_string())
    } else {
        None // Method should always be a string
    }
}

/// Extract "params" field from JSON-RPC request as raw JSON string
///
/// This extracts the params object/array as a string for cache key generation.
/// For simplicity, we extract everything between the params value start and its end.
///
/// # Arguments
/// - `json`: Raw JSON-RPC request string
///
/// # Returns
/// - `Some(params_json)`: Extracted params as JSON string
/// - `None`: Params field not found (returns "{}" for empty params)
fn extract_params(json: &str) -> String {
    // Find "params"
    let params_start = match json.find("\"params\"") {
        Some(pos) => pos,
        None => return "{}".to_string(),
    };

    let after_params = &json[params_start + 8..];
    let colon_pos = match after_params.find(':') {
        Some(pos) => pos,
        None => return "{}".to_string(),
    };

    let after_colon = after_params[colon_pos + 1..].trim_start();

    // Find the end of the params value (object or array)
    // This is a simplified parser - it handles nested braces/brackets
    let (start_char, end_char) = if after_colon.starts_with('{') {
        ('{', '}')
    } else if after_colon.starts_with('[') {
        ('[', ']')
    } else if after_colon.starts_with("null") {
        return "null".to_string();
    } else {
        return "{}".to_string();
    };

    let mut depth = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, c) in after_colon.chars().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if c == '\\' && in_string {
            escape_next = true;
            continue;
        }

        if c == '"' {
            in_string = !in_string;
            continue;
        }

        if in_string {
            continue;
        }

        if c == start_char {
            depth += 1;
        } else if c == end_char {
            depth -= 1;
            if depth == 0 {
                return after_colon[..=i].to_string();
            }
        }
    }

    // Fallback - couldn't parse properly
    "{}".to_string()
}

/// HTTP error that captures status for retry classification
#[derive(Debug)]
struct HttpError {
    status: Option<u16>,
    message: String,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(status) = self.status {
            write!(f, "HTTP {}: {}", status, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for HttpError {}

impl HttpError {
    fn from_ureq_error(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(status, resp) => {
                let body = resp.into_string().unwrap_or_default();
                Self {
                    status: Some(status),
                    message: body,
                }
            }
            _ => Self {
                status: None,
                message: e.to_string(),
            },
        }
    }

    fn is_retryable(&self) -> bool {
        if let Some(status) = self.status {
            // Retry on 5xx server errors and 429 rate limit
            is_retryable_error(status)
        } else {
            // Network errors (connection refused, timeout) are retryable
            let lower = self.message.to_lowercase();
            lower.contains("connection refused")
                || lower.contains("timeout")
                || lower.contains("timed out")
                || lower.contains("connection reset")
                || lower.contains("network unreachable")
                || lower.contains("eof")
        }
    }
}

/// Send HTTP request to MCP server
fn send_http_request(
    config: &BridgeConfig,
    body: &str,
) -> Result<String, HttpError> {
    let response = ureq::post(&config.url)
        .timeout(std::time::Duration::from_secs(config.timeout_secs))
        .set("Content-Type", "application/json")
        .set("X-License-Key", &config.license_key)
        .send_string(body)
        .map_err(HttpError::from_ureq_error)?;

    response
        .into_string()
        .map_err(|e| HttpError {
            status: None,
            message: format!("Response read error: {}", e),
        })
}

/// Main bridge loop - T1+T3+T4+T5 Atomic request processing with full resilience
fn run_bridge(
    config: &BridgeConfig,
    metrics: &McpMetricsCapsule,
    retry_config: &mut MutableRetryConfig,
    circuit_breaker: &MutableCircuitBreaker,
    response_cache: &MutableResponseCache,
    idempotency_cache: &IdempotencyCacheCapsule,
    #[cfg(feature = "client-offline")] offline_queue: &OfflineQueueCapsule,
    #[cfg(feature = "client-batching")] request_batcher: &RequestBatcherCapsule,
    #[cfg(feature = "client-protection")] protection: &P0ProtectionLayer,
    #[cfg(feature = "client-protection")] self_destruct: &SelfDestructHandler,
    connection_state: &ConnectionState,
) -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut line_buf = String::with_capacity(64 * 1024); // 64KB initial buffer

    #[cfg(feature = "client-protection")]
    log!("KDB MCP Bridge started (Phase 1+2+3+4: resilience + caching + offline/batching + protection)");
    #[cfg(not(feature = "client-protection"))]
    log!("KDB MCP Bridge started (Phase 1+2+3: resilience + caching + offline/batching)");
    log!("URL: {}", config.url);
    log!("Timeout: {}s", config.timeout_secs);
    log!("Retry: max={} backoff=standard", retry_config.max_retries());
    log!(
        "CircuitBreaker: threshold={} recovery={}s",
        circuit_breaker.failure_threshold(),
        circuit_breaker.recovery_timeout_secs()
    );
    log!(
        "Cache: enabled={} capacity={}",
        response_cache.config().is_enabled(),
        response_cache.capacity()
    );
    log!(
        "Dedup: capacity={} ttl={}s",
        idempotency_cache.capacity(),
        idempotency_cache.ttl_secs()
    );
    #[cfg(feature = "client-offline")]
    log!(
        "OfflineQueue: capacity={}",
        offline_queue.capacity()
    );
    #[cfg(feature = "client-batching")]
    log!(
        "Batcher: enabled={} max_size={} timeout={}ms",
        request_batcher.is_enabled(),
        request_batcher.max_batch_size(),
        request_batcher.timeout_ms()
    );
    #[cfg(feature = "client-protection")]
    log!("[Protection] P0 protection layer initialized (anti-debug, emulator detection, license validation)");

    for line_result in stdin.lock().lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                log!("stdin read error: {}", e);
                break;
            }
        };

        // Skip empty lines
        if line.trim().is_empty() {
            continue;
        }

        let start = Instant::now();
        let request_id = extract_id(&line);

        // Extract method and params for caching (Phase 2)
        let method = extract_method(&line).unwrap_or_default();
        let params = extract_params(&line);

        log!(
            "REQ [{}]: {} bytes (method={})",
            request_id.as_deref().unwrap_or("?"),
            line.len(),
            method
        );

        // =====================================================================
        // Phase 4: P0 Protection Check (BEFORE all other operations)
        // =====================================================================
        #[cfg(feature = "client-protection")]
        {
            match protection.check_all() {
                Ok(()) => {
                    // Protection passed, continue with request processing
                }
                Err(e) => {
                    log!("[Protection] Check failed: {:?}", e);

                    // Map ProtectionError to TamperReason for self-destruct
                    let tamper_reason = match e {
                        ProtectionError::LicenseInvalid => TamperReason::LicenseViolation,
                        ProtectionError::DebuggerDetected => TamperReason::DebuggerAttached,
                        ProtectionError::EmulatorDetected => TamperReason::EmulatorDetected,
                        ProtectionError::TamperDetected => TamperReason::IntegrityViolation,
                    };

                    // Trigger self-destruct (this does NOT return - process exits)
                    self_destruct.trigger(tamper_reason);
                    // Note: Code never reaches here because trigger() calls std::process::exit()
                }
            }
        }

        // =====================================================================
        // Phase 3A: Offline Queue Replay (when transitioning back online)
        // =====================================================================
        #[cfg(feature = "client-offline")]
        {
            // If we're online and have queued requests, replay them first
            if connection_state.is_online() && !offline_queue.is_empty() {
                log!("[Client-Offline] Connection restored, replaying queued requests...");
                let replayed = offline_queue.replay_all(|queued_req| {
                    // Reconstruct JSON-RPC request
                    let id_str = queued_req.id.map(|n| n.to_string()).unwrap_or_else(|| "null".to_string());
                    let json_body = format!(
                        r#"{{"jsonrpc":"2.0","id":{},"method":"{}","params":{}}}"#,
                        id_str, queued_req.method, queued_req.params
                    );
                    let age_secs = current_unix_timestamp().saturating_sub(queued_req.queued_at_unix);

                    match send_http_request(config, &json_body) {
                        Ok(_response) => {
                            connection_state.mark_success();
                            log!(
                                "[Client-Offline] Replayed request [{}]: {} (age={}s)",
                                id_str,
                                queued_req.method,
                                age_secs
                            );
                            Ok(())
                        }
                        Err(e) => {
                            connection_state.mark_failure();
                            log!(
                                "[Client-Offline] Replay failed [{}]: {}",
                                id_str,
                                e
                            );
                            Err(e.to_string())
                        }
                    }
                });
                log!("[Client-Offline] Replayed {} queued requests", replayed);
            }
        }

        // =====================================================================
        // Phase 3B: Offline Mode Check (queue if disconnected)
        // =====================================================================
        #[cfg(feature = "client-offline")]
        if !connection_state.is_online() {
            // We're offline - queue this request for later
            let request_id_num = request_id
                .as_ref()
                .and_then(|s| s.parse::<u64>().ok());

            log!(
                "[Client-Offline] Queueing request [{}]: {}",
                request_id.as_deref().unwrap_or("?"),
                method
            );

            match offline_queue.enqueue(QueuedRequest::new(
                request_id_num,
                method.clone(),
                params.clone(),
            )) {
                Ok(()) => {
                    // Return a "queued" response to the client
                    let response = format!(
                        r#"{{"jsonrpc":"2.0","id":{},"result":{{"queued":true,"message":"Request queued for offline replay","queue_depth":{}}}}}"#,
                        request_id.as_deref().unwrap_or("null"),
                        offline_queue.size()
                    );
                    line_buf.clear();
                    line_buf.push_str(&response);
                    line_buf.push('\n');

                    if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                        log!("stdout write error: {}", e);
                        break;
                    }
                    if let Err(e) = stdout.flush() {
                        log!("stdout flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log!("[Client-Offline] Queue error: {}", e);
                    let response = json_error(
                        request_id.as_deref(),
                        -32005,
                        &format!("Offline queue error: {}", e),
                    );
                    line_buf.clear();
                    line_buf.push_str(&response);
                    line_buf.push('\n');

                    if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                        log!("stdout write error: {}", e);
                        break;
                    }
                    if let Err(e) = stdout.flush() {
                        log!("stdout flush error: {}", e);
                        break;
                    }
                }
            }
            continue; // Skip normal processing while offline
        }

        // =====================================================================
        // Phase 2A: Idempotency check (request deduplication)
        // =====================================================================
        // Note: hash_request ignores request_id since duplicates may have different IDs
        let request_hash = hash_request(&method, &params, None);
        if idempotency_cache.check_duplicate(request_hash) {
            // Duplicate request in flight - skip
            let latency_us = start.elapsed().as_micros() as u32;
            log!(
                "DEDUP [{}]: duplicate request skipped ({}us)",
                request_id.as_deref().unwrap_or("?"),
                latency_us
            );
            // Don't write response - the original request will handle it
            continue;
        }
        // Insert hash to track this request as in-flight
        idempotency_cache.insert(request_hash);

        // Phase 2B: Response cache check (cache hit path)
        if let Some(cached_response) = response_cache.get(&method, &params) {
            let latency_us = start.elapsed().as_micros() as u32;
            metrics.record_request(true, latency_us, true, 0); // from_cache=true

            log!(
                "CACHE HIT [{}]: {} bytes ({}us)",
                request_id.as_deref().unwrap_or("?"),
                cached_response.len(),
                latency_us
            );

            // Write cached response to stdout
            line_buf.clear();
            line_buf.push_str(&cached_response);
            line_buf.push('\n');

            if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                log!("stdout write error: {}", e);
                break;
            }
            if let Err(e) = stdout.flush() {
                log!("stdout flush error: {}", e);
                break;
            }
            continue;
        }

        // =====================================================================
        // Phase 3C: Request Batching (accumulate read-only requests)
        // =====================================================================
        #[cfg(feature = "client-batching")]
        {
            if request_batcher.is_batchable(&method) {
                let request_id_num = request_id
                    .as_ref()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);

                if let Err(e) = request_batcher.accumulate(BatchableRequest::new(
                    request_id_num,
                    method.clone(),
                    params.clone(),
                )) {
                    log!("[Client-Batch] Accumulate error: {}", e);
                } else {
                    log!(
                        "[Client-Batch] Accumulated [{}]: {} (pending={})",
                        request_id.as_deref().unwrap_or("?"),
                        method,
                        request_batcher.pending_count()
                    );
                }

                // Check if we should flush the batch
                if request_batcher.should_flush() {
                    let batch = request_batcher.flush();
                    if !batch.is_empty() {
                        log!(
                            "[Client-Batch] Flushing batch of {} requests",
                            batch.len()
                        );

                        // Build batch JSON-RPC array
                        let batch_body = RequestBatcherCapsule::build_batch_json(&batch);

                        // Send batch request
                        match send_http_request(config, &batch_body) {
                            Ok(response) => {
                                connection_state.mark_success();
                                circuit_breaker.record_success();
                                log!(
                                    "[Client-Batch] Batch response: {} bytes",
                                    response.len()
                                );

                                // Write batch response to stdout
                                // Note: For proper batch response handling, each response
                                // should be matched to its request ID. For now, we write
                                // the entire batch response.
                                line_buf.clear();
                                line_buf.push_str(&response);
                                line_buf.push('\n');

                                if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                                    log!("stdout write error: {}", e);
                                    break;
                                }
                                if let Err(e) = stdout.flush() {
                                    log!("stdout flush error: {}", e);
                                    break;
                                }
                            }
                            Err(e) => {
                                connection_state.mark_failure();
                                circuit_breaker.record_failure();
                                log!("[Client-Batch] Batch request failed: {}", e);

                                // Return error for each request in batch
                                for req in &batch {
                                    let error_response = json_error(
                                        Some(&req.id.to_string()),
                                        -32001,
                                        &format!("Batch request failed: {}", e),
                                    );
                                    line_buf.clear();
                                    line_buf.push_str(&error_response);
                                    line_buf.push('\n');

                                    if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                                        log!("stdout write error: {}", e);
                                        break;
                                    }
                                }
                                if let Err(e) = stdout.flush() {
                                    log!("stdout flush error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
                continue; // Batched requests handled above
            } else {
                // Non-batchable request - record for stats
                request_batcher.record_passthrough();
            }
        }

        // Phase 1A: Circuit breaker check (fast-fail if open)
        if let Err(cb_err) = circuit_breaker.check() {
            let latency_us = start.elapsed().as_micros() as u32;
            metrics.record_circuit_breaker_reject();

            let error_msg = match cb_err {
                CircuitBreakerError::Open => "Service unavailable (circuit breaker open)",
                CircuitBreakerError::ForcedOpen => "Service unavailable (circuit breaker forced open)",
            };

            log!(
                "CB REJECT [{}]: {} ({}us)",
                request_id.as_deref().unwrap_or("?"),
                error_msg,
                latency_us
            );

            let response = json_error(request_id.as_deref(), -32000, error_msg);
            line_buf.clear();
            line_buf.push_str(&response);
            line_buf.push('\n');

            if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
                log!("stdout write error: {}", e);
                break;
            }
            if let Err(e) = stdout.flush() {
                log!("stdout flush error: {}", e);
                break;
            }
            continue;
        }

        // Phase 1B: Execute request with retry
        let mut retry_attempt = 0u8;
        let response = loop {
            match send_http_request(config, &line) {
                Ok(resp) => {
                    // Success - record and break
                    circuit_breaker.record_success();
                    connection_state.mark_success();
                    break Ok(resp);
                }
                Err(e) => {
                    // Check if retryable
                    if !e.is_retryable() {
                        // Non-retryable (4xx client errors) - fail immediately
                        log!(
                            "NON-RETRYABLE [{}]: {}",
                            request_id.as_deref().unwrap_or("?"),
                            e
                        );
                        circuit_breaker.record_failure();
                        connection_state.mark_failure();
                        break Err(e);
                    }

                    // Retryable error - check if we have retries left
                    retry_attempt += 1;
                    if retry_attempt > retry_config.max_retries() {
                        log!(
                            "RETRY EXHAUSTED [{}]: {} (after {} attempts)",
                            request_id.as_deref().unwrap_or("?"),
                            e,
                            retry_attempt
                        );
                        circuit_breaker.record_failure();
                        connection_state.mark_failure();
                        break Err(e);
                    }

                    // Calculate backoff delay
                    let delay_ms = retry_config.next_delay_ms();
                    log!(
                        "RETRY [{}]: attempt {}/{}, {} - waiting {}ms",
                        request_id.as_deref().unwrap_or("?"),
                        retry_attempt,
                        retry_config.max_retries(),
                        e,
                        delay_ms
                    );

                    if delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }

                    // Increment retry counter for next delay calculation
                    retry_config.increment_attempt();
                }
            }
        };

        // Reset retry state for next request
        retry_config.reset();

        let latency_us = start.elapsed().as_micros() as u32;

        // Build response and record metrics
        let output = match response {
            Ok(body) => {
                metrics.record_request(true, latency_us, false, retry_attempt);

                // Phase 2C: Store successful response in cache (if cacheable)
                if response_cache.is_cacheable(&method) {
                    response_cache.put(&method, &params, body.clone());
                    log!(
                        "OK [{}]: {} bytes, {}us (cached){}",
                        request_id.as_deref().unwrap_or("?"),
                        body.len(),
                        latency_us,
                        if retry_attempt > 0 {
                            format!(" (after {} retries)", retry_attempt)
                        } else {
                            String::new()
                        }
                    );
                } else {
                    log!(
                        "OK [{}]: {} bytes, {}us{}",
                        request_id.as_deref().unwrap_or("?"),
                        body.len(),
                        latency_us,
                        if retry_attempt > 0 {
                            format!(" (after {} retries)", retry_attempt)
                        } else {
                            String::new()
                        }
                    );
                }
                body
            }
            Err(e) => {
                metrics.record_request(false, latency_us, false, retry_attempt);
                log!(
                    "ERR [{}]: {} ({}us, {} retries)",
                    request_id.as_deref().unwrap_or("?"),
                    e,
                    latency_us,
                    retry_attempt
                );

                let code = match e.status {
                    Some(status) if status >= 500 => -32003, // Server error
                    Some(status) if status == 429 => -32004, // Rate limited
                    Some(_) => -32002,                       // Client error
                    None => -32001,                          // Network error
                };

                // Don't cache error responses
                json_error(request_id.as_deref(), code, &e.to_string())
            }
        };

        // Write response to stdout
        line_buf.clear();
        line_buf.push_str(&output);
        line_buf.push('\n');

        if let Err(e) = stdout.write_all(line_buf.as_bytes()) {
            log!("stdout write error: {}", e);
            break;
        }

        if let Err(e) = stdout.flush() {
            log!("stdout flush error: {}", e);
            break;
        }
    }

    // Print metrics summary on shutdown
    metrics.print_summary();

    // Phase 2 cache stats on shutdown
    log!(
        "Cache hit rate: {:.2}% (hits: {}, misses: {})",
        response_cache.hit_rate(),
        response_cache.hits(),
        response_cache.misses()
    );

    // Idempotency dedup stats
    let dedup_stats = idempotency_cache.stats();
    log!(
        "Dedup stats: total_hits={}, total_misses={}, hit_rate={:.2}%",
        dedup_stats.total_hits,
        dedup_stats.total_misses,
        dedup_stats.hit_rate() * 100.0
    );

    // Circuit breaker stats
    log!(
        "Circuit Breaker final state: {:?} (failures: {}/{})",
        circuit_breaker.state(),
        circuit_breaker.failure_count(),
        circuit_breaker.failure_threshold()
    );

    // Phase 3: Offline queue stats
    #[cfg(feature = "client-offline")]
    {
        let offline_stats = offline_queue.stats();
        log!(
            "[Client-Offline] total_queued={}, total_replayed={}, total_dropped={}",
            offline_stats.total_queued,
            offline_stats.total_replayed,
            offline_stats.total_dropped
        );
    }

    // Phase 3: Batching stats
    #[cfg(feature = "client-batching")]
    {
        let batch_stats = request_batcher.stats();
        log!(
            "[Client-Batch] total_batches={}, requests_batched={}, passthrough={}, avg_batch_size={:.2}",
            batch_stats.total_batches,
            batch_stats.total_requests_batched,
            batch_stats.total_passthrough,
            batch_stats.avg_batch_size
        );
    }

    // Connection state stats
    log!(
        "[Client-Connection] online={}, failures={}, offline_transitions={}",
        connection_state.is_online(),
        connection_state.failure_count(),
        connection_state.total_offline_transitions()
    );

    // Phase 4: Protection stats
    #[cfg(feature = "client-protection")]
    {
        let protection_stats = protection.stats();
        log!(
            "[Protection] Checks: {}, failures: {}",
            protection_stats.total_checks,
            protection_stats.total_failures
        );
    }

    Ok(())
}

fn main() {
    // Read configuration from environment
    let url = std::env::var("KDB_MCP_URL")
        .unwrap_or_else(|_| "https://mcp.kindly.software/mcp".to_string());

    let license_key = match std::env::var("KDB_LICENSE_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            let error = json_error(None, -32000, "KDB_LICENSE_KEY environment variable not set");
            println!("{}", error);
            log!("FATAL: KDB_LICENSE_KEY not set");
            std::process::exit(1);
        }
    };

    let timeout_secs = std::env::var("KDB_TIMEOUT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let config = BridgeConfig {
        url,
        license_key,
        timeout_secs,
        _pad: [0; 8],
    };

    // Initialize Phase 1 resilience capsules
    let metrics = McpMetricsCapsule::new();
    let mut retry_config = MutableRetryConfig::from_env();
    let circuit_breaker = MutableCircuitBreaker::from_env();

    // Initialize Phase 2 performance capsules
    let cache_config = ResponseCacheConfig::from_env();
    let response_cache = MutableResponseCache::new(cache_config);
    let idempotency_cache = IdempotencyCacheCapsule::from_env();

    // Initialize Phase 3 advanced resilience capsules (conditionally compiled)
    #[cfg(feature = "client-offline")]
    let offline_queue = OfflineQueueCapsule::from_env();

    #[cfg(feature = "client-batching")]
    let request_batcher = RequestBatcherCapsule::from_env();

    // Initialize Phase 4 protection capsules (conditionally compiled)
    #[cfg(feature = "client-protection")]
    let protection = P0ProtectionLayer::new(&config.license_key);

    #[cfg(feature = "client-protection")]
    let self_destruct = SelfDestructHandler::new();

    let connection_state = ConnectionState::new();

    if let Err(e) = run_bridge(
        &config,
        &metrics,
        &mut retry_config,
        &circuit_breaker,
        &response_cache,
        &idempotency_cache,
        #[cfg(feature = "client-offline")]
        &offline_queue,
        #[cfg(feature = "client-batching")]
        &request_batcher,
        #[cfg(feature = "client-protection")]
        &protection,
        #[cfg(feature = "client-protection")]
        &self_destruct,
        &connection_state,
    ) {
        log!("Bridge error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_id_number() {
        let json = r#"{"jsonrpc":"2.0","id":123,"method":"test"}"#;
        assert_eq!(extract_id(json), Some("123".to_string()));
    }

    #[test]
    fn test_extract_id_string() {
        let json = r#"{"jsonrpc":"2.0","id":"abc-123","method":"test"}"#;
        assert_eq!(extract_id(json), Some("abc-123".to_string()));
    }

    #[test]
    fn test_extract_id_null() {
        let json = r#"{"jsonrpc":"2.0","method":"test"}"#;
        assert_eq!(extract_id(json), None);
    }

    #[test]
    fn test_json_error() {
        let error = json_error(Some("123"), -32000, "Test error");
        assert!(error.contains("\"id\":\"123\""));
        assert!(error.contains("\"code\":-32000"));
        assert!(error.contains("\"message\":\"Test error\""));
    }

    #[test]
    fn test_json_error_null_id() {
        let error = json_error(None, -32000, "Test error");
        assert!(error.contains("\"id\":null"));
    }

    #[test]
    fn test_http_error_retryable_status() {
        let err_500 = HttpError {
            status: Some(500),
            message: "Internal Server Error".to_string(),
        };
        assert!(err_500.is_retryable());

        let err_503 = HttpError {
            status: Some(503),
            message: "Service Unavailable".to_string(),
        };
        assert!(err_503.is_retryable());

        let err_429 = HttpError {
            status: Some(429),
            message: "Too Many Requests".to_string(),
        };
        assert!(err_429.is_retryable());
    }

    #[test]
    fn test_http_error_non_retryable_status() {
        let err_400 = HttpError {
            status: Some(400),
            message: "Bad Request".to_string(),
        };
        assert!(!err_400.is_retryable());

        let err_401 = HttpError {
            status: Some(401),
            message: "Unauthorized".to_string(),
        };
        assert!(!err_401.is_retryable());

        let err_404 = HttpError {
            status: Some(404),
            message: "Not Found".to_string(),
        };
        assert!(!err_404.is_retryable());
    }

    #[test]
    fn test_http_error_network_errors() {
        let timeout = HttpError {
            status: None,
            message: "Connection timeout".to_string(),
        };
        assert!(timeout.is_retryable());

        let refused = HttpError {
            status: None,
            message: "Connection refused".to_string(),
        };
        assert!(refused.is_retryable());

        let invalid = HttpError {
            status: None,
            message: "Invalid JSON".to_string(),
        };
        assert!(!invalid.is_retryable());
    }

    #[test]
    fn test_bridge_config_alignment() {
        assert_eq!(std::mem::align_of::<BridgeConfig>(), 64);
    }

    // =========================================================================
    // Phase 2: Method and Params Extraction Tests
    // =========================================================================

    #[test]
    fn test_extract_method_basic() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        assert_eq!(extract_method(json), Some("tools/list".to_string()));
    }

    #[test]
    fn test_extract_method_with_params() {
        let json = r#"{"jsonrpc":"2.0","method":"debugger_attach","params":{"pid":123},"id":1}"#;
        assert_eq!(extract_method(json), Some("debugger_attach".to_string()));
    }

    #[test]
    fn test_extract_method_debugger() {
        let json = r#"{"jsonrpc":"2.0","id":"abc","method":"debugger/quota_status","params":{}}"#;
        assert_eq!(extract_method(json), Some("debugger/quota_status".to_string()));
    }

    #[test]
    fn test_extract_method_missing() {
        let json = r#"{"jsonrpc":"2.0","id":1,"params":{}}"#;
        assert_eq!(extract_method(json), None);
    }

    #[test]
    fn test_extract_params_simple_object() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"a":1},"id":1}"#;
        assert_eq!(extract_params(json), r#"{"a":1}"#);
    }

    #[test]
    fn test_extract_params_nested_object() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"outer":{"inner":"value"}},"id":1}"#;
        assert_eq!(extract_params(json), r#"{"outer":{"inner":"value"}}"#);
    }

    #[test]
    fn test_extract_params_array() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":["a","b"],"id":1}"#;
        assert_eq!(extract_params(json), r#"["a","b"]"#);
    }

    #[test]
    fn test_extract_params_empty_object() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{},"id":1}"#;
        assert_eq!(extract_params(json), "{}");
    }

    #[test]
    fn test_extract_params_missing() {
        let json = r#"{"jsonrpc":"2.0","method":"test","id":1}"#;
        assert_eq!(extract_params(json), "{}");
    }

    #[test]
    fn test_extract_params_null() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":null,"id":1}"#;
        assert_eq!(extract_params(json), "null");
    }

    #[test]
    fn test_extract_params_with_strings_containing_braces() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"code":"if (x) { }"},"id":1}"#;
        assert_eq!(extract_params(json), r#"{"code":"if (x) { }"}"#);
    }

    #[test]
    fn test_extract_params_with_escaped_quotes() {
        let json = r#"{"jsonrpc":"2.0","method":"test","params":{"msg":"say \"hi\""},"id":1}"#;
        assert_eq!(extract_params(json), r#"{"msg":"say \"hi\""}"#);
    }

    // =========================================================================
    // Phase 3: ConnectionState Tests
    // =========================================================================

    #[test]
    fn test_connection_state_alignment() {
        assert_eq!(std::mem::align_of::<ConnectionState>(), 64);
    }

    #[test]
    fn test_connection_state_starts_online() {
        let state = ConnectionState::new();
        assert!(state.is_online());
        assert_eq!(state.failure_count(), 0);
        assert_eq!(state.total_offline_transitions(), 0);
    }

    #[test]
    fn test_connection_state_mark_success_resets_failures() {
        let state = ConnectionState::new();

        // Simulate some failures
        state.mark_failure();
        state.mark_failure();
        assert_eq!(state.failure_count(), 2);
        assert!(state.is_online()); // Still online (need 3 failures)

        // Success resets counter
        state.mark_success();
        assert_eq!(state.failure_count(), 0);
        assert!(state.is_online());
    }

    #[test]
    fn test_connection_state_transitions_offline_after_3_failures() {
        let state = ConnectionState::new();

        state.mark_failure();
        assert!(state.is_online());
        assert_eq!(state.failure_count(), 1);

        state.mark_failure();
        assert!(state.is_online());
        assert_eq!(state.failure_count(), 2);

        state.mark_failure();
        assert!(!state.is_online()); // Now offline
        assert_eq!(state.failure_count(), 3);
        assert_eq!(state.total_offline_transitions(), 1);
    }

    #[test]
    fn test_connection_state_returns_online_on_success() {
        let state = ConnectionState::new();

        // Go offline
        state.mark_failure();
        state.mark_failure();
        state.mark_failure();
        assert!(!state.is_online());

        // Success brings back online
        state.mark_success();
        assert!(state.is_online());
        assert_eq!(state.failure_count(), 0);
    }

    #[test]
    fn test_connection_state_multiple_offline_transitions() {
        let state = ConnectionState::new();

        // First offline transition
        for _ in 0..3 {
            state.mark_failure();
        }
        assert!(!state.is_online());
        assert_eq!(state.total_offline_transitions(), 1);

        // Back online
        state.mark_success();
        assert!(state.is_online());

        // Second offline transition
        for _ in 0..3 {
            state.mark_failure();
        }
        assert!(!state.is_online());
        assert_eq!(state.total_offline_transitions(), 2);
    }
}
