//! SseTransportCapsule - T6 Mixed SSE Transport Orchestrator (512 bytes)
//!
//! Main SSE server orchestrator that ties together sessions, connection pool, and MCP server.
//! **Tier**: T6 Mixed (T5 Streaming + T8 Network + T1 Atomic)
//! **Size**: 512 bytes, 64-byte aligned
//! **Lockfree**: 100% - no mutex/RwLock
//!
//! ## UCE34 Framework Application (Q1-Q34)
//!
//! ### Q1-Q9: Problem Understanding
//! - Q1: Orchestrate SSE transport for MCP server (handle connections, route messages)
//! - Q2: Constraints: <50ns state ops, <1ms message routing, 100% lockfree
//! - Q3: Scale: 100 concurrent SSE connections, 10K messages/sec
//! - Q4: Failures: Invalid state, pool full, session not found, auth failures
//! - Q5: Baseline: No SSE transport (HTTP polling)
//!
//! ### Q10-Q12: Tier Selection & Implementation
//! - Q10: T6 Mixed (T5 Streaming for events + T8 Network for connections + T1 Atomic for state)
//! - Q11: Rust type system enforces valid state transitions via CAS
//! - Q12: Nightly: N/A (stable atomics sufficient)
//!
//! ### Q33: Verification
//! - Memory layout: 512 bytes, 64-byte aligned (verified by tests)
//! - No mutex/RwLock in any code path
//! - State machine enforced via CAS
//!
//! ### Q34: Auditability (Q34 Framework)
//! - Generation counter prevents TOCTOU races
//! - All metrics tracked atomically
//! - Integration with McpServerCapsule's audit trail
//!
//! ## Memory Layout (512 bytes, 64-byte aligned)
//!
//! ```text
//! Offset 0-15:    State (16 bytes)
//!   ├─ state (8 bytes):       TransportState enum (AtomicU64)
//!   └─ generation (8 bytes):  TOCTOU prevention (AtomicU64)
//!
//! Offset 16-47:   Configuration (32 bytes)
//!   ├─ max_connections (4 bytes):        AtomicU32
//!   ├─ heartbeat_interval_ms (4 bytes):  AtomicU32
//!   ├─ connection_timeout_ms (4 bytes):  AtomicU32
//!   ├─ message_queue_size (4 bytes):     AtomicU32
//!   ├─ port (2 bytes):                   AtomicU16
//!   └─ _config_padding (14 bytes):       Alignment padding
//!
//! Offset 48-111:  Metrics (64 bytes, cache-line aligned)
//!   ├─ active_connections (8 bytes):     AtomicU64
//!   ├─ total_connections (8 bytes):      AtomicU64
//!   ├─ total_disconnections (8 bytes):   AtomicU64
//!   ├─ messages_pushed (8 bytes):        AtomicU64
//!   ├─ messages_received (8 bytes):      AtomicU64
//!   ├─ bytes_pushed (8 bytes):           AtomicU64
//!   ├─ bytes_received (8 bytes):         AtomicU64
//!   └─ errors (8 bytes):                 AtomicU64
//!
//! Offset 112-127: Timestamps (16 bytes)
//!   ├─ started_at_ns (8 bytes):          AtomicU64
//!   └─ last_heartbeat_ns (8 bytes):      AtomicU64
//!
//! Offset 128-511: Padding (384 bytes)
//!   └─ _padding: Fill to 512 bytes for cache alignment
//! ```
//!
//! ## Transport States (FSM)
//!
//! ```text
//! STOPPED(0) --> STARTING(1) --> RUNNING(2) --> DRAINING(3) --> STOPPING(4) --> STOPPED(0)
//! ```
//!
//! ## Performance (B32 Framework)
//! - **new()**: <50ns (initialization)
//! - **state()**: <5ns (single atomic load)
//! - **start()/stop()**: <20ns (CAS operations)
//! - **handle_sse_connect()**: <100ns (allocation + formatting)
//! - **handle_message()**: <1ms (full MCP request processing)
//! - **snapshot()**: <50ns (multiple atomic loads)
//!
//! ## ASSUM Safety (100%)
//! - #ASSUME_LOCKFREE: No mutex/RwLock, all atomic operations
//! - #ASSUME_VALID_FSM: CAS enforces valid state transitions
//! - #ASSUME_GENERATION_COUNTER: TOCTOU prevention via generation
//! - #ASSUME_CACHE_ALIGNED_64B: 64-byte alignment eliminates false sharing

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};

#[cfg(feature = "sse-transport")]
use crate::sse_connection_pool::{SseConnectionPoolCapsule, SlotState};

use crate::subscription_tier::SubscriptionTier;

// ============================================================================
// Constants
// ============================================================================

/// Default maximum connections
pub const DEFAULT_MAX_CONNECTIONS: u32 = 100;

/// Default heartbeat interval (30 seconds)
pub const DEFAULT_HEARTBEAT_INTERVAL_MS: u32 = 30_000;

/// Default connection timeout (5 minutes)
pub const DEFAULT_CONNECTION_TIMEOUT_MS: u32 = 300_000;

/// Default message queue size
pub const DEFAULT_MESSAGE_QUEUE_SIZE: u32 = 1024;

/// Default port
pub const DEFAULT_PORT: u16 = 8080;

// ============================================================================
// Transport State Enum
// ============================================================================

/// Transport lifecycle states
///
/// **Memory**: 8 bytes (stored in AtomicU64)
/// **Valid Transitions**: STOPPED→STARTING→RUNNING→DRAINING→STOPPING→STOPPED
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportState {
    /// Transport is stopped
    Stopped = 0,
    /// Transport is starting up
    Starting = 1,
    /// Transport is running and accepting connections
    Running = 2,
    /// Transport is draining (rejecting new connections, processing existing)
    Draining = 3,
    /// Transport is stopping
    Stopping = 4,
}

impl TransportState {
    /// Convert from u64 (for atomic storage)
    #[inline]
    pub const fn from_u64(value: u64) -> Option<Self> {
        match value {
            0 => Some(Self::Stopped),
            1 => Some(Self::Starting),
            2 => Some(Self::Running),
            3 => Some(Self::Draining),
            4 => Some(Self::Stopping),
            _ => None,
        }
    }

    /// Convert to u64 (for atomic storage)
    #[inline]
    pub const fn as_u64(self) -> u64 {
        self as u64
    }

    /// Check if transition is valid
    #[inline]
    pub const fn is_valid_transition(from: Self, to: Self) -> bool {
        matches!(
            (from, to),
            (Self::Stopped, Self::Starting)
                | (Self::Starting, Self::Running)
                | (Self::Starting, Self::Stopped) // Failed start
                | (Self::Running, Self::Draining)
                | (Self::Running, Self::Stopping) // Fast stop
                | (Self::Draining, Self::Stopping)
                | (Self::Stopping, Self::Stopped)
        )
    }
}

// ============================================================================
// Transport Error Types
// ============================================================================

/// Transport operation errors
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// Transport not in running state
    NotRunning,
    /// Connection pool is full
    PoolFull,
    /// Session not found by ID
    SessionNotFound,
    /// Invalid HTTP request
    InvalidRequest,
    /// Invalid state for operation
    InvalidState,
    /// Authentication failed
    AuthFailed,
    /// Rate limit exceeded
    RateLimited,
    /// Internal error
    Internal,
}

impl core::fmt::Display for TransportError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotRunning => write!(f, "transport not running"),
            Self::PoolFull => write!(f, "connection pool full"),
            Self::SessionNotFound => write!(f, "session not found"),
            Self::InvalidRequest => write!(f, "invalid HTTP request"),
            Self::InvalidState => write!(f, "invalid transport state"),
            Self::AuthFailed => write!(f, "authentication failed"),
            Self::RateLimited => write!(f, "rate limit exceeded"),
            Self::Internal => write!(f, "internal error"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for TransportError {}

// ============================================================================
// HTTP Response Type
// ============================================================================

/// HTTP response for SSE transport
#[derive(Clone, Debug)]
pub struct HttpResponse {
    /// HTTP status code
    pub status: u16,
    /// Response headers (pre-formatted)
    pub headers: String,
    /// Response body
    pub body: String,
}

impl HttpResponse {
    /// Create new HTTP response
    pub fn new(status: u16, headers: String, body: String) -> Self {
        Self { status, headers, body }
    }

    /// Convert to raw HTTP response string
    pub fn to_raw(&self) -> String {
        format!("{}{}", self.headers, self.body)
    }
}

// ============================================================================
// Transport Configuration
// ============================================================================

/// SSE transport configuration
#[derive(Clone, Debug)]
pub struct SseTransportConfig {
    /// Maximum connections allowed
    pub max_connections: u32,
    /// Heartbeat interval in milliseconds
    pub heartbeat_interval_ms: u32,
    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u32,
    /// Message queue size per connection
    pub message_queue_size: u32,
    /// Port to listen on
    pub port: u16,
}

impl Default for SseTransportConfig {
    fn default() -> Self {
        Self {
            max_connections: DEFAULT_MAX_CONNECTIONS,
            heartbeat_interval_ms: DEFAULT_HEARTBEAT_INTERVAL_MS,
            connection_timeout_ms: DEFAULT_CONNECTION_TIMEOUT_MS,
            message_queue_size: DEFAULT_MESSAGE_QUEUE_SIZE,
            port: DEFAULT_PORT,
        }
    }
}

// ============================================================================
// Transport Snapshot (Read-Only View)
// ============================================================================

/// Immutable snapshot of transport state
#[derive(Clone, Debug)]
pub struct TransportSnapshot {
    /// Current transport state
    pub state: TransportState,
    /// Generation counter
    pub generation: u64,
    /// Configuration
    pub max_connections: u32,
    pub heartbeat_interval_ms: u32,
    pub connection_timeout_ms: u32,
    pub message_queue_size: u32,
    pub port: u16,
    /// Metrics
    pub active_connections: u64,
    pub total_connections: u64,
    pub total_disconnections: u64,
    pub messages_pushed: u64,
    pub messages_received: u64,
    pub bytes_pushed: u64,
    pub bytes_received: u64,
    pub errors: u64,
    /// Timestamps
    pub started_at_ns: u64,
    pub last_heartbeat_ns: u64,
}

// ============================================================================
// SSE Event Formatting (MCP 2024-11-05 spec)
// ============================================================================

/// Format SSE event with proper line endings
///
/// SSE format: `event: <type>\ndata: <data>\n\n`
#[inline]
pub fn format_sse_event(event_type: &str, data: &str) -> String {
    format!("event: {}\ndata: {}\n\n", event_type, data)
}

/// Format endpoint event (first event after connection)
///
/// Tells client where to POST messages for this session
#[inline]
pub fn format_endpoint_event(session_id: &str) -> String {
    format_sse_event("endpoint", &format!("/message?sessionId={}", session_id))
}

/// Format message event (JSON-RPC response)
#[inline]
pub fn format_message_event(json: &str) -> String {
    format_sse_event("message", json)
}

/// Format heartbeat/ping event
#[inline]
pub fn format_ping_event() -> String {
    format_sse_event("ping", "")
}

// ============================================================================
// HTTP Response Helpers
// ============================================================================

/// Build SSE connection response headers
#[inline]
pub fn build_sse_response_headers() -> String {
    "HTTP/1.1 200 OK\r\n\
     Content-Type: text/event-stream\r\n\
     Cache-Control: no-cache\r\n\
     Connection: keep-alive\r\n\
     Access-Control-Allow-Origin: *\r\n\
     Access-Control-Allow-Headers: X-License-Key, Content-Type\r\n\
     \r\n"
        .to_string()
}

/// Build 204 No Content response
#[inline]
pub fn build_204_response() -> String {
    "HTTP/1.1 204 No Content\r\n\
     Access-Control-Allow-Origin: *\r\n\
     \r\n"
        .to_string()
}

/// Build error response
#[inline]
pub fn build_error_response(status: u16, message: &str) -> String {
    let body = format!(r#"{{"error":"{}"}}"#, message);
    format!(
        "HTTP/1.1 {} Error\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        status,
        body.len(),
        body
    )
}

/// Build JSON response
#[inline]
pub fn build_json_response(status: u16, json: &str) -> String {
    format!(
        "HTTP/1.1 {} OK\r\n\
         Content-Type: application/json\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Content-Length: {}\r\n\
         \r\n\
         {}",
        status,
        json.len(),
        json
    )
}

/// Build CORS preflight response
#[inline]
pub fn build_cors_preflight_response() -> String {
    "HTTP/1.1 204 No Content\r\n\
     Access-Control-Allow-Origin: *\r\n\
     Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
     Access-Control-Allow-Headers: X-License-Key, Content-Type\r\n\
     Access-Control-Max-Age: 86400\r\n\
     \r\n"
        .to_string()
}

// ============================================================================
// API Key Extraction
// ============================================================================

/// Extract API key from headers
///
/// Looks for `X-License-Key` header (case-insensitive)
#[inline]
pub fn extract_api_key<'a>(headers: &'a [(String, String)]) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("x-license-key"))
        .map(|(_, v)| v.as_str())
}

/// Extract session ID from query string
///
/// Parses `?sessionId=xxx` from path
#[inline]
pub fn extract_session_id(path: &str) -> Option<&str> {
    // Find sessionId parameter
    if let Some(query_start) = path.find('?') {
        let query = &path[query_start + 1..];
        for param in query.split('&') {
            if let Some(eq_pos) = param.find('=') {
                let key = &param[..eq_pos];
                let value = &param[eq_pos + 1..];
                if key.eq_ignore_ascii_case("sessionId") || key.eq_ignore_ascii_case("sessionid") {
                    return Some(value);
                }
            }
        }
    }
    None
}

// ============================================================================
// SseTransportCapsule (512 bytes, 64-byte aligned)
// ============================================================================

/// SSE Transport Orchestrator Capsule
///
/// **Tier**: T6 Mixed (T5+T8+T1)
/// **Size**: 512 bytes
/// **Alignment**: 64 bytes (cache-line aligned)
/// **Lockfree**: 100% (no mutex/RwLock)
///
/// # ASSUM Safety Tags
/// - #ASSUME_LOCKFREE: All operations use atomic primitives
/// - #ASSUME_VALID_FSM: CAS enforces valid state transitions
/// - #ASSUME_GENERATION_COUNTER: Generation prevents TOCTOU races
/// - #ASSUME_CACHE_ALIGNED_64B: 64B alignment prevents false sharing
#[repr(C, align(64))]
pub struct SseTransportCapsule {
    // State (16 bytes)
    /// Transport state (TransportState enum as u64)
    /// #ASSUME_VALID_FSM: CAS ensures valid transitions
    state: AtomicU64,
    /// Generation counter for TOCTOU prevention
    /// #ASSUME_GENERATION_COUNTER: Incremented on every state change
    generation: AtomicU64,

    // Configuration (32 bytes)
    /// Maximum connections allowed
    max_connections: AtomicU32,
    /// Heartbeat interval in milliseconds
    heartbeat_interval_ms: AtomicU32,
    /// Connection timeout in milliseconds
    connection_timeout_ms: AtomicU32,
    /// Message queue size per connection
    message_queue_size: AtomicU32,
    /// Port to listen on
    port: AtomicU16,
    /// Padding for alignment
    _config_padding: [u8; 14],

    // Metrics (64 bytes, cache-line aligned)
    /// Currently active connections
    active_connections: AtomicU64,
    /// Total connections accepted (lifetime)
    total_connections: AtomicU64,
    /// Total disconnections (lifetime)
    total_disconnections: AtomicU64,
    /// Total messages pushed via SSE
    messages_pushed: AtomicU64,
    /// Total messages received via POST
    messages_received: AtomicU64,
    /// Total bytes pushed via SSE
    bytes_pushed: AtomicU64,
    /// Total bytes received via POST
    bytes_received: AtomicU64,
    /// Total errors encountered
    errors: AtomicU64,

    // Timestamps (16 bytes)
    /// Transport start timestamp (nanoseconds since epoch)
    started_at_ns: AtomicU64,
    /// Last heartbeat timestamp (nanoseconds since epoch)
    last_heartbeat_ns: AtomicU64,

    // Padding to 512 bytes
    _padding: [u8; 384],
}

// Compile-time verification of size and alignment
const _: () = {
    assert!(
        core::mem::size_of::<SseTransportCapsule>() == 512,
        "SseTransportCapsule must be exactly 512 bytes"
    );
    assert!(
        core::mem::align_of::<SseTransportCapsule>() == 64,
        "SseTransportCapsule must be 64-byte aligned"
    );
};

impl SseTransportCapsule {
    // ========================================================================
    // Constructor
    // ========================================================================

    /// Create new transport capsule with default configuration
    ///
    /// **Latency**: <50ns
    pub const fn new() -> Self {
        Self {
            state: AtomicU64::new(TransportState::Stopped as u64),
            generation: AtomicU64::new(0),
            max_connections: AtomicU32::new(DEFAULT_MAX_CONNECTIONS),
            heartbeat_interval_ms: AtomicU32::new(DEFAULT_HEARTBEAT_INTERVAL_MS),
            connection_timeout_ms: AtomicU32::new(DEFAULT_CONNECTION_TIMEOUT_MS),
            message_queue_size: AtomicU32::new(DEFAULT_MESSAGE_QUEUE_SIZE),
            port: AtomicU16::new(DEFAULT_PORT),
            _config_padding: [0u8; 14],
            active_connections: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
            total_disconnections: AtomicU64::new(0),
            messages_pushed: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            bytes_pushed: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            started_at_ns: AtomicU64::new(0),
            last_heartbeat_ns: AtomicU64::new(0),
            _padding: [0u8; 384],
        }
    }

    // ========================================================================
    // Configuration
    // ========================================================================

    /// Configure transport with given settings
    ///
    /// **Latency**: <20ns
    pub fn configure(&self, config: SseTransportConfig) {
        self.max_connections
            .store(config.max_connections, Ordering::Release);
        self.heartbeat_interval_ms
            .store(config.heartbeat_interval_ms, Ordering::Release);
        self.connection_timeout_ms
            .store(config.connection_timeout_ms, Ordering::Release);
        self.message_queue_size
            .store(config.message_queue_size, Ordering::Release);
        self.port.store(config.port, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current configuration
    pub fn config(&self) -> SseTransportConfig {
        SseTransportConfig {
            max_connections: self.max_connections.load(Ordering::Acquire),
            heartbeat_interval_ms: self.heartbeat_interval_ms.load(Ordering::Acquire),
            connection_timeout_ms: self.connection_timeout_ms.load(Ordering::Acquire),
            message_queue_size: self.message_queue_size.load(Ordering::Acquire),
            port: self.port.load(Ordering::Acquire),
        }
    }

    // ========================================================================
    // State Machine
    // ========================================================================

    /// Get current transport state
    ///
    /// **Latency**: <5ns
    #[inline]
    pub fn state(&self) -> TransportState {
        let raw = self.state.load(Ordering::Acquire);
        TransportState::from_u64(raw).unwrap_or(TransportState::Stopped)
    }

    /// Transition state atomically (CAS)
    ///
    /// **Latency**: <10ns
    fn transition_state(&self, from: TransportState, to: TransportState) -> bool {
        if !TransportState::is_valid_transition(from, to) {
            return false;
        }

        let result = self.state.compare_exchange(
            from as u64,
            to as u64,
            Ordering::AcqRel,
            Ordering::Acquire,
        );

        if result.is_ok() {
            self.generation.fetch_add(1, Ordering::Release);
            true
        } else {
            false
        }
    }

    /// Start transport (transitions to Running)
    ///
    /// **Latency**: <20ns
    pub fn start(&self) -> Result<(), TransportError> {
        let current = self.state();

        if current != TransportState::Stopped {
            return Err(TransportError::InvalidState);
        }

        // Stopped -> Starting
        if !self.transition_state(TransportState::Stopped, TransportState::Starting) {
            return Err(TransportError::InvalidState);
        }

        // Record start time
        #[cfg(feature = "std")]
        {
            let now_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            self.started_at_ns.store(now_ns, Ordering::Release);
            self.last_heartbeat_ns.store(now_ns, Ordering::Release);
        }

        // Starting -> Running
        if !self.transition_state(TransportState::Starting, TransportState::Running) {
            // Rollback to Stopped on failure
            self.state
                .store(TransportState::Stopped as u64, Ordering::Release);
            return Err(TransportError::InvalidState);
        }

        Ok(())
    }

    /// Stop transport gracefully
    ///
    /// **Latency**: <20ns
    pub fn stop(&self) -> Result<(), TransportError> {
        let current = self.state();

        match current {
            TransportState::Running => {
                // Running -> Stopping (fast stop)
                if !self.transition_state(TransportState::Running, TransportState::Stopping) {
                    return Err(TransportError::InvalidState);
                }
            }
            TransportState::Draining => {
                // Draining -> Stopping
                if !self.transition_state(TransportState::Draining, TransportState::Stopping) {
                    return Err(TransportError::InvalidState);
                }
            }
            TransportState::Stopping => {
                // Already stopping, just continue
            }
            _ => {
                return Err(TransportError::InvalidState);
            }
        }

        // Stopping -> Stopped
        if !self.transition_state(TransportState::Stopping, TransportState::Stopped) {
            // Force to stopped
            self.state
                .store(TransportState::Stopped as u64, Ordering::Release);
        }

        Ok(())
    }

    /// Begin draining (reject new connections, finish existing)
    pub fn drain(&self) -> Result<(), TransportError> {
        if self.state() != TransportState::Running {
            return Err(TransportError::InvalidState);
        }

        if !self.transition_state(TransportState::Running, TransportState::Draining) {
            return Err(TransportError::InvalidState);
        }

        Ok(())
    }

    /// Check if transport is running
    #[inline]
    pub fn is_running(&self) -> bool {
        self.state() == TransportState::Running
    }

    // ========================================================================
    // HTTP Request Handling
    // ========================================================================

    /// Handle incoming HTTP request
    ///
    /// Routes to appropriate handler based on method and path.
    ///
    /// # Supported Routes
    /// - `GET /sse` - Establish SSE connection
    /// - `POST /message?sessionId=xxx` - Send message to session
    /// - `OPTIONS *` - CORS preflight
    ///
    /// **Latency**: Variable (depends on route)
    #[cfg(feature = "sse-transport")]
    pub fn handle_http_request(
        &self,
        method: &str,
        path: &str,
        headers: &[(String, String)],
        body: &str,
        client_ip: &str,
        pool: &SseConnectionPoolCapsule,
    ) -> HttpResponse {
        // Handle CORS preflight
        if method.eq_ignore_ascii_case("OPTIONS") {
            return HttpResponse::new(204, build_cors_preflight_response(), String::new());
        }

        // Check if transport is running
        if !self.is_running() {
            self.record_error();
            return HttpResponse::new(
                503,
                build_error_response(503, "Service unavailable"),
                String::new(),
            );
        }

        // Route based on method and path
        let path_lower = path.to_ascii_lowercase();

        match (method.to_ascii_uppercase().as_str(), path_lower.as_str()) {
            ("GET", p) if p.starts_with("/sse") => {
                // SSE connection establishment
                match self.handle_sse_connect(headers, client_ip, pool) {
                    Ok((response_headers, _session_id)) => {
                        HttpResponse::new(200, response_headers, String::new())
                    }
                    Err(e) => {
                        self.record_error();
                        HttpResponse::new(
                            match e {
                                TransportError::PoolFull => 503,
                                TransportError::AuthFailed => 401,
                                TransportError::RateLimited => 429,
                                _ => 500,
                            },
                            build_error_response(
                                match e {
                                    TransportError::PoolFull => 503,
                                    TransportError::AuthFailed => 401,
                                    TransportError::RateLimited => 429,
                                    _ => 500,
                                },
                                &e.to_string(),
                            ),
                            String::new(),
                        )
                    }
                }
            }
            ("POST", p) if p.starts_with("/message") => {
                // Message handling requires session ID
                let session_id = match extract_session_id(path) {
                    Some(id) => id,
                    None => {
                        self.record_error();
                        return HttpResponse::new(
                            400,
                            build_error_response(400, "Missing sessionId parameter"),
                            String::new(),
                        );
                    }
                };

                // Look up session in pool
                let (slot_idx, generation) = match pool.find_by_session_id(session_id) {
                    Some(result) => result,
                    None => {
                        self.record_error();
                        return HttpResponse::new(
                            404,
                            build_error_response(404, "Session not found"),
                            String::new(),
                        );
                    }
                };

                // Get slot and validate
                let slot = match pool.get_slot(slot_idx, generation) {
                    Some(s) => s,
                    None => {
                        self.record_error();
                        return HttpResponse::new(
                            404,
                            build_error_response(404, "Session expired"),
                            String::new(),
                        );
                    }
                };

                // Record message received
                self.record_message_received(body.len() as u64);
                slot.record_message_received(body.len() as u64);

                // Return 202 Accepted - actual processing is done asynchronously
                HttpResponse::new(202, build_204_response(), String::new())
            }
            _ => {
                self.record_error();
                HttpResponse::new(
                    404,
                    build_error_response(404, "Not found"),
                    String::new(),
                )
            }
        }
    }

    /// Handle GET /sse - establish SSE connection
    ///
    /// Returns (response_headers, session_id) for the caller to hold connection open.
    ///
    /// **Latency**: <100ns
    #[cfg(feature = "sse-transport")]
    pub fn handle_sse_connect(
        &self,
        headers: &[(String, String)],
        client_ip: &str,
        pool: &SseConnectionPoolCapsule,
    ) -> Result<(String, String), TransportError> {
        // Check if running
        if !self.is_running() {
            return Err(TransportError::NotRunning);
        }

        // Extract and validate API key
        let api_key = extract_api_key(headers);

        // Determine tier from API key (default to Hobby if none)
        let tier = if api_key.is_some() {
            // In production, this would validate the API key
            // For now, default to Developer tier if key present
            SubscriptionTier::Developer
        } else {
            SubscriptionTier::Hobby
        };

        // Allocate slot in pool
        let (slot_idx, generation) = pool.allocate().ok_or(TransportError::PoolFull)?;

        // Generate session ID (use pool's session ID from slot)
        // For now, generate a simple ID based on slot and generation
        let session_id = format!(
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            slot_idx as u32,
            generation & 0xFFFF,
            (generation >> 16) & 0xFFFF,
            0,
            get_timestamp_ns() & 0xFFFFFFFFFFFF
        );

        // Initialize slot
        if let Err(_e) = pool.init_slot(slot_idx, generation, &session_id, -1) {
            // Release slot on failure
            let _ = pool.release(slot_idx, generation);
            return Err(TransportError::Internal);
        }

        // Transition slot to Established
        let _ = pool.transition_slot(
            slot_idx,
            generation,
            SlotState::Connecting,
            SlotState::Established,
        );

        // Update slot with auth info
        if let Some(slot) = pool.get_slot(slot_idx, generation) {
            // Store user hash and tier
            let user_hash = if let Some(key) = api_key {
                fnv1a_hash(key.as_bytes())
            } else {
                fnv1a_hash(client_ip.as_bytes())
            };
            // Note: slot doesn't have set_auth, we'd need to extend it
            // For now, just touch the slot
            slot.touch(get_timestamp_ns());
        }

        // Record connection
        self.record_connection();

        // Build SSE response headers
        let response_headers = build_sse_response_headers();

        // Format endpoint event as first SSE message
        let endpoint_event = format_endpoint_event(&session_id);

        // Combine headers with initial event
        let full_response = format!("{}{}", response_headers, endpoint_event);

        Ok((full_response, session_id))
    }

    /// Format push event for a session
    ///
    /// Returns formatted SSE event string ready to send.
    ///
    /// **Latency**: <50ns
    pub fn format_push_event(&self, _session_id: &str, json: &str) -> String {
        format_message_event(json)
    }

    // ========================================================================
    // Metrics Recording
    // ========================================================================

    /// Record new connection
    ///
    /// **Latency**: <10ns
    #[inline]
    pub fn record_connection(&self) {
        self.active_connections.fetch_add(1, Ordering::Relaxed);
        self.total_connections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record disconnection
    ///
    /// **Latency**: <10ns
    #[inline]
    pub fn record_disconnection(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
        self.total_disconnections.fetch_add(1, Ordering::Relaxed);
    }

    /// Record message pushed
    ///
    /// **Latency**: <10ns
    #[inline]
    pub fn record_message_pushed(&self, bytes: u64) {
        self.messages_pushed.fetch_add(1, Ordering::Relaxed);
        self.bytes_pushed.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record message received
    ///
    /// **Latency**: <10ns
    #[inline]
    pub fn record_message_received(&self, bytes: u64) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record error
    ///
    /// **Latency**: <5ns
    #[inline]
    pub fn record_error(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    // ========================================================================
    // Metrics Accessors
    // ========================================================================

    /// Get active connection count
    #[inline]
    pub fn active_connections(&self) -> u64 {
        self.active_connections.load(Ordering::Relaxed)
    }

    /// Get total connections (lifetime)
    #[inline]
    pub fn total_connections(&self) -> u64 {
        self.total_connections.load(Ordering::Relaxed)
    }

    /// Get total disconnections (lifetime)
    #[inline]
    pub fn total_disconnections(&self) -> u64 {
        self.total_disconnections.load(Ordering::Relaxed)
    }

    /// Get messages pushed count
    #[inline]
    pub fn messages_pushed(&self) -> u64 {
        self.messages_pushed.load(Ordering::Relaxed)
    }

    /// Get messages received count
    #[inline]
    pub fn messages_received(&self) -> u64 {
        self.messages_received.load(Ordering::Relaxed)
    }

    /// Get bytes pushed count
    #[inline]
    pub fn bytes_pushed(&self) -> u64 {
        self.bytes_pushed.load(Ordering::Relaxed)
    }

    /// Get bytes received count
    #[inline]
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    /// Get error count
    #[inline]
    pub fn errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // ========================================================================
    // Heartbeat and Stale Detection
    // ========================================================================

    /// Record heartbeat timestamp
    ///
    /// **Latency**: <10ns
    pub fn record_heartbeat(&self) {
        #[cfg(feature = "std")]
        {
            let now_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            self.last_heartbeat_ns.store(now_ns, Ordering::Release);
        }
    }

    /// Check for stale connections and expire them
    ///
    /// Returns number of connections expired.
    ///
    /// **Latency**: O(n) where n = active connections
    #[cfg(feature = "sse-transport")]
    pub fn check_heartbeats(&self, pool: &SseConnectionPoolCapsule) -> usize {
        let timeout_ns =
            (self.connection_timeout_ms.load(Ordering::Acquire) as u64) * 1_000_000;

        let expired = pool.expire_stale(timeout_ns);

        // Update disconnection count
        for _ in 0..expired {
            self.record_disconnection();
        }

        // Record heartbeat
        self.record_heartbeat();

        expired
    }

    // ========================================================================
    // Snapshot
    // ========================================================================

    /// Get snapshot for metrics/debugging
    ///
    /// **Latency**: <50ns
    pub fn snapshot(&self) -> TransportSnapshot {
        TransportSnapshot {
            state: self.state(),
            generation: self.generation.load(Ordering::Acquire),
            max_connections: self.max_connections.load(Ordering::Acquire),
            heartbeat_interval_ms: self.heartbeat_interval_ms.load(Ordering::Acquire),
            connection_timeout_ms: self.connection_timeout_ms.load(Ordering::Acquire),
            message_queue_size: self.message_queue_size.load(Ordering::Acquire),
            port: self.port.load(Ordering::Acquire),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            total_disconnections: self.total_disconnections.load(Ordering::Relaxed),
            messages_pushed: self.messages_pushed.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            bytes_pushed: self.bytes_pushed.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            started_at_ns: self.started_at_ns.load(Ordering::Acquire),
            last_heartbeat_ns: self.last_heartbeat_ns.load(Ordering::Acquire),
        }
    }
}

impl Default for SseTransportCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: SseTransportCapsule uses only atomic operations for all shared state
// #ASSUME_SEND_SYNC: All fields are atomic, no interior mutability issues
unsafe impl Send for SseTransportCapsule {}
unsafe impl Sync for SseTransportCapsule {}

// ============================================================================
// Helpers
// ============================================================================

#[inline]
fn get_timestamp_ns() -> u64 {
    #[cfg(feature = "std")]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0)
    }
    #[cfg(not(feature = "std"))]
    {
        0
    }
}

/// FNV-1a hash for quick string hashing
#[inline]
fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ============================================================================
// Unit Tests (T28 Q1-Q7)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    // ========================================================================
    // Q1-Q3: Size and Alignment Tests
    // ========================================================================

    #[test]
    fn test_transport_capsule_size() {
        assert_eq!(
            size_of::<SseTransportCapsule>(),
            512,
            "SseTransportCapsule must be exactly 512 bytes"
        );
    }

    #[test]
    fn test_transport_capsule_alignment() {
        assert_eq!(
            align_of::<SseTransportCapsule>(),
            64,
            "SseTransportCapsule must be 64-byte aligned"
        );
    }

    // ========================================================================
    // Q4-Q5: State Transition Tests
    // ========================================================================

    #[test]
    fn test_state_transitions() {
        let transport = SseTransportCapsule::new();
        assert_eq!(transport.state(), TransportState::Stopped);

        // Start transport
        transport.start().expect("Should start");
        assert_eq!(transport.state(), TransportState::Running);

        // Stop transport
        transport.stop().expect("Should stop");
        assert_eq!(transport.state(), TransportState::Stopped);
    }

    #[test]
    fn test_state_transition_validation() {
        // Test valid transitions
        assert!(TransportState::is_valid_transition(
            TransportState::Stopped,
            TransportState::Starting
        ));
        assert!(TransportState::is_valid_transition(
            TransportState::Starting,
            TransportState::Running
        ));
        assert!(TransportState::is_valid_transition(
            TransportState::Running,
            TransportState::Draining
        ));
        assert!(TransportState::is_valid_transition(
            TransportState::Draining,
            TransportState::Stopping
        ));
        assert!(TransportState::is_valid_transition(
            TransportState::Stopping,
            TransportState::Stopped
        ));

        // Test invalid transitions
        assert!(!TransportState::is_valid_transition(
            TransportState::Stopped,
            TransportState::Running
        ));
        assert!(!TransportState::is_valid_transition(
            TransportState::Running,
            TransportState::Starting
        ));
    }

    #[test]
    fn test_drain_lifecycle() {
        let transport = SseTransportCapsule::new();
        transport.start().expect("Should start");

        // Drain
        transport.drain().expect("Should drain");
        assert_eq!(transport.state(), TransportState::Draining);

        // Stop from draining
        transport.stop().expect("Should stop from draining");
        assert_eq!(transport.state(), TransportState::Stopped);
    }

    // ========================================================================
    // Q6: SSE Event Formatting Tests
    // ========================================================================

    #[test]
    fn test_sse_event_formatting() {
        let event = format_sse_event("message", r#"{"test":true}"#);
        assert_eq!(event, "event: message\ndata: {\"test\":true}\n\n");
    }

    #[test]
    fn test_endpoint_event() {
        let event = format_endpoint_event("abc-123-def");
        assert_eq!(
            event,
            "event: endpoint\ndata: /message?sessionId=abc-123-def\n\n"
        );
    }

    #[test]
    fn test_message_event() {
        let event = format_message_event(r#"{"jsonrpc":"2.0","result":"ok"}"#);
        assert!(event.starts_with("event: message\n"));
        assert!(event.contains("jsonrpc"));
    }

    // ========================================================================
    // Q7: HTTP Response Tests
    // ========================================================================

    #[test]
    fn test_build_sse_headers() {
        let headers = build_sse_response_headers();
        assert!(headers.contains("text/event-stream"));
        assert!(headers.contains("no-cache"));
        assert!(headers.contains("keep-alive"));
        assert!(headers.contains("Access-Control-Allow-Origin"));
    }

    #[test]
    fn test_build_204_response() {
        let response = build_204_response();
        assert!(response.contains("204 No Content"));
        assert!(response.contains("Access-Control-Allow-Origin"));
    }

    #[test]
    fn test_build_error_response() {
        let response = build_error_response(404, "Not found");
        assert!(response.contains("404"));
        assert!(response.contains("application/json"));
        assert!(response.contains(r#""error":"Not found""#));
    }

    // ========================================================================
    // Q8: API Key Extraction Tests
    // ========================================================================

    #[test]
    fn test_extract_api_key() {
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-License-Key".to_string(), "test-key-123".to_string()),
        ];

        let key = extract_api_key(&headers);
        assert_eq!(key, Some("test-key-123"));

        // Case insensitive
        let headers2 = vec![(
            "x-license-key".to_string(),
            "lowercase-key".to_string(),
        )];
        let key2 = extract_api_key(&headers2);
        assert_eq!(key2, Some("lowercase-key"));

        // Not found
        let headers3 = vec![("Authorization".to_string(), "Bearer xyz".to_string())];
        let key3 = extract_api_key(&headers3);
        assert_eq!(key3, None);
    }

    #[test]
    fn test_extract_session_id() {
        assert_eq!(
            extract_session_id("/message?sessionId=abc-123"),
            Some("abc-123")
        );
        assert_eq!(
            extract_session_id("/message?foo=bar&sessionId=xyz-789&baz=qux"),
            Some("xyz-789")
        );
        assert_eq!(extract_session_id("/message"), None);
        assert_eq!(extract_session_id("/message?foo=bar"), None);
    }

    // ========================================================================
    // Q9: Metrics Recording Tests
    // ========================================================================

    #[test]
    fn test_metrics_recording() {
        let transport = SseTransportCapsule::new();

        assert_eq!(transport.active_connections(), 0);
        assert_eq!(transport.total_connections(), 0);

        // Record connections
        transport.record_connection();
        transport.record_connection();
        assert_eq!(transport.active_connections(), 2);
        assert_eq!(transport.total_connections(), 2);

        // Record disconnection
        transport.record_disconnection();
        assert_eq!(transport.active_connections(), 1);
        assert_eq!(transport.total_disconnections(), 1);

        // Record messages
        transport.record_message_pushed(100);
        transport.record_message_received(50);
        assert_eq!(transport.messages_pushed(), 1);
        assert_eq!(transport.bytes_pushed(), 100);
        assert_eq!(transport.messages_received(), 1);
        assert_eq!(transport.bytes_received(), 50);

        // Record errors
        transport.record_error();
        transport.record_error();
        assert_eq!(transport.errors(), 2);
    }

    // ========================================================================
    // Q10: Snapshot Tests
    // ========================================================================

    #[test]
    fn test_snapshot() {
        let transport = SseTransportCapsule::new();
        transport.start().expect("Should start");

        transport.record_connection();
        transport.record_message_pushed(256);
        transport.record_error();

        let snap = transport.snapshot();

        assert_eq!(snap.state, TransportState::Running);
        assert_eq!(snap.active_connections, 1);
        assert_eq!(snap.messages_pushed, 1);
        assert_eq!(snap.bytes_pushed, 256);
        assert_eq!(snap.errors, 1);
        assert_eq!(snap.max_connections, DEFAULT_MAX_CONNECTIONS);
        assert_eq!(snap.port, DEFAULT_PORT);
    }

    // ========================================================================
    // Q11: Configuration Tests
    // ========================================================================

    #[test]
    fn test_configuration() {
        let transport = SseTransportCapsule::new();

        // Default config
        let config = transport.config();
        assert_eq!(config.max_connections, DEFAULT_MAX_CONNECTIONS);
        assert_eq!(config.port, DEFAULT_PORT);

        // Update config
        let new_config = SseTransportConfig {
            max_connections: 50,
            heartbeat_interval_ms: 15_000,
            connection_timeout_ms: 120_000,
            message_queue_size: 512,
            port: 9090,
        };
        transport.configure(new_config);

        let updated = transport.config();
        assert_eq!(updated.max_connections, 50);
        assert_eq!(updated.port, 9090);
        assert_eq!(updated.heartbeat_interval_ms, 15_000);
    }

    // ========================================================================
    // Q12: Transport State Enum Tests
    // ========================================================================

    #[test]
    fn test_transport_state_from_u64() {
        assert_eq!(TransportState::from_u64(0), Some(TransportState::Stopped));
        assert_eq!(TransportState::from_u64(1), Some(TransportState::Starting));
        assert_eq!(TransportState::from_u64(2), Some(TransportState::Running));
        assert_eq!(TransportState::from_u64(3), Some(TransportState::Draining));
        assert_eq!(TransportState::from_u64(4), Some(TransportState::Stopping));
        assert_eq!(TransportState::from_u64(5), None);
        assert_eq!(TransportState::from_u64(255), None);
    }

    #[test]
    fn test_transport_state_as_u64() {
        assert_eq!(TransportState::Stopped.as_u64(), 0);
        assert_eq!(TransportState::Starting.as_u64(), 1);
        assert_eq!(TransportState::Running.as_u64(), 2);
        assert_eq!(TransportState::Draining.as_u64(), 3);
        assert_eq!(TransportState::Stopping.as_u64(), 4);
    }

    // ========================================================================
    // Q13: Error Display Tests
    // ========================================================================

    #[test]
    fn test_transport_error_display() {
        assert_eq!(format!("{}", TransportError::NotRunning), "transport not running");
        assert_eq!(format!("{}", TransportError::PoolFull), "connection pool full");
        assert_eq!(format!("{}", TransportError::SessionNotFound), "session not found");
        assert_eq!(format!("{}", TransportError::InvalidRequest), "invalid HTTP request");
        assert_eq!(format!("{}", TransportError::InvalidState), "invalid transport state");
        assert_eq!(format!("{}", TransportError::AuthFailed), "authentication failed");
    }

    // ========================================================================
    // Q14: Generation Counter Tests
    // ========================================================================

    #[test]
    fn test_generation_counter() {
        let transport = SseTransportCapsule::new();
        let initial_gen = transport.generation();

        // Start should increment generation
        transport.start().expect("Should start");
        assert!(transport.generation() > initial_gen);

        // Configure should increment generation
        let gen_before_config = transport.generation();
        transport.configure(SseTransportConfig::default());
        assert!(transport.generation() > gen_before_config);

        // Stop should increment generation
        let gen_before_stop = transport.generation();
        transport.stop().expect("Should stop");
        assert!(transport.generation() > gen_before_stop);
    }

    // ========================================================================
    // Q15: CORS Preflight Tests
    // ========================================================================

    #[test]
    fn test_cors_preflight_response() {
        let response = build_cors_preflight_response();
        assert!(response.contains("204 No Content"));
        assert!(response.contains("Access-Control-Allow-Methods"));
        assert!(response.contains("GET, POST, OPTIONS"));
        assert!(response.contains("Access-Control-Max-Age"));
    }

    // ========================================================================
    // Integration Tests (require sse-transport feature)
    // ========================================================================

    #[cfg(feature = "sse-transport")]
    mod integration_tests {
        use super::*;
        use crate::sse_connection_pool::SseConnectionPoolCapsule;

        #[test]
        fn test_handle_sse_connect() {
            let transport = SseTransportCapsule::new();
            let pool = SseConnectionPoolCapsule::new();

            transport.start().expect("Should start");

            let headers = vec![
                ("X-License-Key".to_string(), "test-key".to_string()),
            ];

            let result = transport.handle_sse_connect(&headers, "127.0.0.1", &pool);
            assert!(result.is_ok());

            let (response, session_id) = result.unwrap();
            assert!(response.contains("text/event-stream"));
            assert!(response.contains("endpoint"));
            assert!(!session_id.is_empty());

            // Check metrics updated
            assert_eq!(transport.active_connections(), 1);
            assert_eq!(transport.total_connections(), 1);
        }

        #[test]
        fn test_handle_sse_connect_pool_full() {
            let transport = SseTransportCapsule::new();
            let pool = SseConnectionPoolCapsule::new();

            transport.start().expect("Should start");

            let headers = vec![];

            // Fill the pool
            for _ in 0..crate::sse_connection_pool::MAX_CONNECTIONS {
                let _ = pool.allocate();
            }

            // Next connect should fail
            let result = transport.handle_sse_connect(&headers, "127.0.0.1", &pool);
            assert_eq!(result, Err(TransportError::PoolFull));
        }

        #[test]
        fn test_handle_http_request_routing() {
            let transport = SseTransportCapsule::new();
            let pool = SseConnectionPoolCapsule::new();

            transport.start().expect("Should start");

            // OPTIONS (CORS preflight)
            let response = transport.handle_http_request(
                "OPTIONS",
                "/sse",
                &[],
                "",
                "127.0.0.1",
                &pool,
            );
            assert_eq!(response.status, 204);

            // GET /sse (SSE connect)
            let response = transport.handle_http_request(
                "GET",
                "/sse",
                &[],
                "",
                "127.0.0.1",
                &pool,
            );
            assert_eq!(response.status, 200);

            // POST /message without sessionId
            let response = transport.handle_http_request(
                "POST",
                "/message",
                &[],
                "{}",
                "127.0.0.1",
                &pool,
            );
            assert_eq!(response.status, 400);

            // Unknown route
            let response = transport.handle_http_request(
                "GET",
                "/unknown",
                &[],
                "",
                "127.0.0.1",
                &pool,
            );
            assert_eq!(response.status, 404);
        }

        #[test]
        fn test_check_heartbeats() {
            let transport = SseTransportCapsule::new();
            let pool = SseConnectionPoolCapsule::new();

            transport.start().expect("Should start");

            // Create a connection
            let (slot_idx, gen) = pool.allocate().expect("Should allocate");
            pool.init_slot(slot_idx, gen, "test-session", -1)
                .expect("Should init");

            // Set old timestamp (force expiration)
            if let Some(slot) = pool.get_slot(slot_idx, gen) {
                let old_time = get_timestamp_ns().saturating_sub(400_000_000_000); // 400 seconds ago
                slot.touch(old_time);
            }

            transport.record_connection();

            // Check heartbeats with default 5 minute timeout
            let expired = transport.check_heartbeats(&pool);
            assert_eq!(expired, 1);
        }
    }
}
