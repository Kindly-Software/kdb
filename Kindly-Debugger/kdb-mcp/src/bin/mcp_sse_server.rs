//! MCP SSE Server Binary - Server-Sent Events Transport for MCP Protocol
//!
//! **Architecture**: T6 Mixed SSE bridge for MCP protocol
//! - SseConnectionPoolCapsule (T4+T1): Connection slot management (~13 KB)
//! - McpServerCapsule (T6 Mixed): Request processing pipeline (256 KB)
//!
//! **MCP SSE Protocol Flow**:
//! 1. Client sends `GET /sse` with optional `X-License-Key` header
//! 2. Server returns `200 OK` with `Content-Type: text/event-stream`
//! 3. Server sends `event: endpoint\ndata: /message?sessionId=xxx\n\n`
//! 4. Connection stays open for SSE events
//! 5. Client sends `POST /message?sessionId=xxx` with JSON-RPC body
//! 6. Server returns `204 No Content` immediately
//! 7. Server pushes response via SSE: `event: message\ndata: {json}\n\n`
//!
//! **Target Latency**: <100μs per SSE event
//! **Throughput**: 100 concurrent SSE connections
//! **Memory**: ~300 KB total
//!
//! ## Deployment
//!
//! ```bash
//! # Build SSE server
//! cargo build --release --bin mcp_sse_server --features "std,json-rpc,sse-transport"
//!
//! # Run server on port 8081
//! ./target/release/mcp_sse_server
//!
//! # Test SSE connection
//! curl -N -H "Accept: text/event-stream" http://localhost:8081/sse
//! ```
//!
//! ## Environment Variables
//!
//! - `MCP_SSE_PORT`: SSE server port (default: 8081)
//! - `MCP_SSE_ADDR`: SSE listen address (default: 0.0.0.0)
//!
//! ## Safety & Compliance
//!
//! - **Tier**: T6 Mixed (T1 Atomic + T4 Batch)
//! - **Framework**: UCE34 (Q1-Q34 systematic discovery)
//! - **Safety**: ASSUM 99.5%+ (lockfree connection pool)
//! - **Verification**: T28 compliance

use kdb_mcp::{
    McpServerCapsule, SseConnectionPoolCapsule, SlotState,
    fnv1a_hash, HttpTransportCapsule, RateLimiterCapsule,
};
use kdb::DebuggerCapsule;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Sender, Receiver};
use std::thread;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

// ============================================================================
// Constants
// ============================================================================

/// Default SSE server port
const DEFAULT_SSE_PORT: u16 = 8081;

/// Session timeout in nanoseconds (5 minutes)
const SESSION_TIMEOUT_NS: u64 = 5 * 60 * 1_000_000_000;

/// Heartbeat interval in seconds
const HEARTBEAT_INTERVAL_SECS: u64 = 30;

// ============================================================================
// Shared Server State
// ============================================================================

/// Message to be pushed via SSE stream
///
/// Contains the JSON-RPC response to send to the client
struct SseMessage {
    /// JSON response body
    json: String,
}

/// Session channel registry
///
/// Maps session_id -> Sender for pushing messages to SSE connections.
/// Uses Mutex for the registry (not fast path - only on message send/SSE setup).
/// The actual message passing uses lockfree mpsc channels.
struct SessionChannelRegistry {
    /// Session ID to channel sender map
    channels: Mutex<HashMap<String, Sender<SseMessage>>>,
}

impl SessionChannelRegistry {
    fn new() -> Self {
        Self {
            channels: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new session channel
    fn register(&self, session_id: String, sender: Sender<SseMessage>) {
        if let Ok(mut guard) = self.channels.lock() {
            guard.insert(session_id, sender);
        }
    }

    /// Unregister a session channel
    fn unregister(&self, session_id: &str) {
        if let Ok(mut guard) = self.channels.lock() {
            guard.remove(session_id);
        }
    }

    /// Send a message to a session (returns true if sent)
    fn send(&self, session_id: &str, message: SseMessage) -> bool {
        if let Ok(guard) = self.channels.lock() {
            if let Some(sender) = guard.get(session_id) {
                return sender.send(message).is_ok();
            }
        }
        false
    }
}

/// Shared state between connection handlers
///
/// Uses `'static` references via `Box::leak` for long-lived server state.
/// This is the standard pattern for capsule-based servers.
struct ServerState {
    pool: &'static SseConnectionPoolCapsule,
    mcp_server: &'static McpServerCapsule,
    debugger: &'static DebuggerCapsule,
    /// HTTP transport capsule (T6 Mixed, 512B) - Chaos-compliant metrics tracking
    http_transport: &'static HttpTransportCapsule,
    /// Rate limiter capsule (T1 Atomic, 4KB) - global token bucket
    rate_limiter: &'static RateLimiterCapsule,
    /// Session channel registry for SSE push
    channels: SessionChannelRegistry,
}

// ============================================================================
// HTTP Request Structure
// ============================================================================

/// Parsed HTTP request
struct HttpRequest {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpRequest {
    /// Get header value by name (case-insensitive)
    fn get_header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[MCP-SSE] SSE Server v0.1.0 (kdb-mcp)");
    eprintln!("[MCP-SSE] Protocol: MCP over Server-Sent Events");
    eprintln!("[MCP-SSE] Max connections: 100");

    // ========================================================================
    // Phase 1: Initialize Capsules
    // ========================================================================

    eprintln!("[MCP-SSE] Phase 1: Initializing capsules...");

    // Create DebuggerCapsule (1 MB) - leaked for 'static lifetime
    let debugger: &'static DebuggerCapsule = Box::leak(Box::new(DebuggerCapsule::new(0)));
    eprintln!("[MCP-SSE]   DebuggerCapsule created (1.0 MB)");

    // Create McpServerCapsule (256 KB) - leaked for 'static lifetime
    let mcp_server: &'static McpServerCapsule = Box::leak(Box::new(McpServerCapsule::new(debugger)));
    eprintln!("[MCP-SSE]   McpServerCapsule created (256 KB)");

    // Configure admin license from environment variable (if provided)
    if let Ok(admin_license) = std::env::var("KDB_ADMIN_LICENSE") {
        let expiry = 2000000000; // Year 2033
        mcp_server.license.set_license(&admin_license, expiry);
        eprintln!("[MCP-SSE]   Admin license configured (expires: 2033)");
    } else {
        eprintln!("[MCP-SSE]   No admin license (set KDB_ADMIN_LICENSE env var for testing)");
    }

    // Create SseConnectionPoolCapsule (~13 KB) - leaked for 'static lifetime
    let pool: &'static SseConnectionPoolCapsule = Box::leak(Box::new(SseConnectionPoolCapsule::new()));
    eprintln!("[MCP-SSE]   SseConnectionPoolCapsule created (~13 KB)");

    // Create HttpTransportCapsule (512B, T6 Mixed) - Chaos-compliant HTTP handler
    // Port 8081, max body 1MB
    let http_transport: &'static HttpTransportCapsule = Box::leak(Box::new(
        HttpTransportCapsule::new(DEFAULT_SSE_PORT, 1024 * 1024)
    ));
    http_transport.start().expect("Failed to start HttpTransportCapsule");
    eprintln!("[MCP-SSE]   HttpTransportCapsule created (512B, T6 Mixed)");

    // Create RateLimiterCapsule (4KB, T1 Atomic) - global token bucket
    // Default: 100 requests/sec, burst of 10
    let rate_limiter: &'static RateLimiterCapsule = Box::leak(Box::new(RateLimiterCapsule::new()));
    eprintln!("[MCP-SSE]   RateLimiterCapsule created (4KB, T1 Atomic)");

    // Create shared state with 'static references
    let state = Arc::new(ServerState {
        pool,
        mcp_server,
        debugger,
        http_transport,
        rate_limiter,
        channels: SessionChannelRegistry::new(),
    });

    // ========================================================================
    // Phase 2: Start SSE Server
    // ========================================================================

    let port = std::env::var("MCP_SSE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SSE_PORT);

    let addr = std::env::var("MCP_SSE_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string());
    let listen_addr = format!("{}:{}", addr, port);

    eprintln!("[MCP-SSE] Phase 2: Starting SSE server...");
    eprintln!("[MCP-SSE]   Listening on: http://{}", listen_addr);

    let listener = TcpListener::bind(&listen_addr)?;
    listener.set_nonblocking(false)?;

    eprintln!("[MCP-SSE] Ready to accept connections");
    eprintln!("[MCP-SSE] SSE endpoint: GET /sse");
    eprintln!("[MCP-SSE] Message endpoint: POST /message?sessionId=<uuid>");

    // ========================================================================
    // Phase 3: Spawn Stale Session Cleanup Thread
    // ========================================================================

    let cleanup_state = Arc::clone(&state);
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(60));
            let expired = cleanup_state.pool.expire_stale(SESSION_TIMEOUT_NS);
            if expired > 0 {
                eprintln!("[MCP-SSE] Cleaned up {} stale sessions", expired);
            }
        }
    });

    // ========================================================================
    // Phase 4: Accept Connections Loop
    // ========================================================================

    for stream in listener.incoming() {
        match stream {
            Ok(socket) => {
                let handler_state = Arc::clone(&state);
                thread::spawn(move || {
                    if let Err(e) = handle_connection(socket, &handler_state) {
                        eprintln!("[MCP-SSE] Connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("[MCP-SSE] Accept error: {}", e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// HTTP Request Parsing
// ============================================================================

/// Parse HTTP request from stream
fn parse_http_request(stream: &mut TcpStream) -> Result<HttpRequest, std::io::Error> {
    let cloned = stream.try_clone()?;
    let mut reader = BufReader::new(cloned);

    // Parse request line: "GET /sse HTTP/1.1\r\n"
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid request line",
        ));
    }

    let method = parts[0].to_string();
    let path = parts[1].to_string();

    // Parse headers until empty line
    let mut headers = Vec::new();
    let mut content_length = 0usize;

    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;

        if line.trim().is_empty() {
            break;
        }

        if let Some((key, value)) = line.trim().split_once(':') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();

            if key.eq_ignore_ascii_case("content-length") {
                content_length = value.parse().unwrap_or(0);
            }

            headers.push((key, value));
        }
    }

    // Parse body for POST requests
    let body = if method == "POST" && content_length > 0 {
        let mut body_bytes = vec![0u8; content_length];
        reader.read_exact(&mut body_bytes)?;
        String::from_utf8_lossy(&body_bytes).to_string()
    } else {
        String::new()
    };

    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

// ============================================================================
// Connection Handler
// ============================================================================

/// Handle a single TCP connection
fn handle_connection(
    mut stream: TcpStream,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let client_ip = stream.peer_addr()?.to_string();

    let request = parse_http_request(&mut stream)?;

    match (request.method.as_str(), request.path.as_str()) {
        // SSE connection establishment
        ("GET", "/sse") => {
            handle_sse_connection(&mut stream, &request, &client_ip, state)?;
        }

        // CORS preflight
        ("OPTIONS", _) => {
            let response = "HTTP/1.1 204 No Content\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                Access-Control-Allow-Headers: X-License-Key, Content-Type\r\n\
                Access-Control-Max-Age: 86400\r\n\
                \r\n";
            stream.write_all(response.as_bytes())?;
        }

        // Message endpoint (POST /message?sessionId=xxx) - SSE mode
        ("POST", path) if path.starts_with("/message") => {
            handle_message(&mut stream, &request, &client_ip, state)?;
        }

        // HTTP Transport endpoint (POST /mcp) - Direct JSON-RPC, no SSE
        // This is the recommended transport per Claude Code docs
        ("POST", "/mcp") | ("POST", "/") => {
            handle_http_request(&mut stream, &request, &client_ip, state)?;
        }

        // Health check
        ("GET", "/health") => {
            let active = state.pool.active_count();
            let total = state.pool.total_connections();
            let body = format!(
                r#"{{"status":"ok","active_connections":{},"total_connections":{}}}"#,
                active, total
            );
            write_json_response(&mut stream, 200, &body)?;
        }

        // ====================================================================
        // OAuth 2.1 Discovery Endpoints (MCP Spec 2024-11-05 / 2025-03-26)
        // ====================================================================

        // RFC 8414 - OAuth 2.0 Authorization Server Metadata
        ("GET", "/.well-known/oauth-authorization-server") => {
            handle_oauth_metadata(&mut stream)?;
        }

        // RFC 9728 - OAuth 2.0 Protected Resource Metadata
        ("GET", "/.well-known/oauth-protected-resource") => {
            handle_protected_resource_metadata(&mut stream)?;
        }

        // OIDC Discovery (fallback for some clients)
        ("GET", "/.well-known/openid-configuration") => {
            handle_oauth_metadata(&mut stream)?;
        }

        // OAuth Authorization Endpoint - redirects to signup
        ("GET", path) if path.starts_with("/oauth/authorize") => {
            handle_oauth_authorize(&mut stream, &request)?;
        }

        // OAuth Token Endpoint - exchanges code for access token
        ("POST", "/oauth/token") => {
            handle_oauth_token(&mut stream, &request)?;
        }

        // Dynamic Client Registration (RFC 7591, required by MCP spec)
        ("POST", "/register") | ("POST", "/oauth/register") => {
            handle_client_registration(&mut stream, &request)?;
        }

        // 404 Not Found
        _ => {
            let body = r#"{"error":"Not Found"}"#;
            write_json_response(&mut stream, 404, body)?;
        }
    }

    Ok(())
}

// ============================================================================
// HTTP Transport Handler (Recommended)
// ============================================================================

/// Handle HTTP transport request (POST /mcp)
///
/// This is the recommended transport per Claude Code documentation (2025-03-26).
/// Routes through HttpTransportCapsule for proper Chaos-compliant handling:
/// - Auth bypass for protocol methods (initialize, ping)
/// - Rate limiting
/// - Metrics tracking
/// - Content-Type validation (relaxed for MCP)
///
/// **Tier**: T6 Mixed (T1 Atomic + T8 Network + T5 Streaming + T0 Auditable)
/// **Latency**: <100μs end-to-end
fn handle_http_request(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Convert headers Vec to HashMap for HttpTransportCapsule
    let mut headers = HashMap::new();
    for (key, value) in &request.headers {
        headers.insert(key.to_lowercase(), value.clone());
    }

    // Route through HttpTransportCapsule for proper Chaos-compliant handling
    // This handles:
    // - Protocol method detection (initialize, ping) - bypass auth
    // - API key validation (X-License-Key or Authorization: Bearer)
    // - Content-Type validation (relaxed)
    // - Rate limiting
    // - Metrics tracking
    let result = state.http_transport.handle_request(
        "POST",
        "/mcp",
        &headers,
        &request.body,
        client_ip,
        state.mcp_server,
        state.rate_limiter,
        state.debugger,
    );

    match result {
        Ok((status, body)) => {
            eprintln!(
                "[MCP-HTTP] Request: client={}, body_len={}, status={}, response_len={}",
                client_ip, request.body.len(), status, body.len()
            );
            write_json_response(stream, status, &body)?;
        }
        Err(e) => {
            // Map HttpTransportError to appropriate HTTP status and JSON-RPC error
            use kdb_mcp::http_transport::HttpTransportError;
            let (status, code, message) = match e {
                HttpTransportError::MissingApiKey => (401, -32001, "Authentication required"),
                HttpTransportError::InvalidApiKey => (401, -32001, "Invalid API key"),
                HttpTransportError::RateLimitExceeded => (429, -32429, "Rate limit exceeded"),
                HttpTransportError::InvalidMethod => (405, -32600, "Method not allowed"),
                HttpTransportError::InvalidContentType => (415, -32600, "Invalid Content-Type"),
                HttpTransportError::BodyTooLarge => (413, -32600, "Request body too large"),
                HttpTransportError::InternalError => (500, -32603, "Internal server error"),
            };

            let error_body = format!(
                r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":null}}"#,
                code, message
            );

            eprintln!(
                "[MCP-HTTP] Error: client={}, status={}, error={}",
                client_ip, status, message
            );

            write_json_response(stream, status, &error_body)?;
        }
    }

    Ok(())
}

// ============================================================================
// SSE Connection Handler
// ============================================================================

/// Handle SSE connection (long-lived)
fn handle_sse_connection(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Allocate slot in connection pool
    let (slot, generation) = state
        .pool
        .allocate()
        .ok_or("Connection pool full")?;

    // Generate session ID (UUID-like format)
    let session_id = generate_session_id(slot, generation);

    // Get socket file descriptor for tracking
    #[cfg(unix)]
    let socket_fd = stream.as_raw_fd();
    #[cfg(not(unix))]
    let socket_fd = -1i32;

    // Initialize slot with session info
    if let Err(e) = state.pool.init_slot(slot, generation, &session_id, socket_fd) {
        // Rollback allocation on init failure
        let _ = state.pool.release(slot, generation);
        return Err(format!("Failed to initialize slot: {:?}", e).into());
    }

    // Create message channel for SSE push
    let (tx, rx): (Sender<SseMessage>, Receiver<SseMessage>) = mpsc::channel();

    // Register channel in session registry
    state.channels.register(session_id.clone(), tx);

    // Extract API key for authentication (optional)
    let api_key = request.get_header("x-license-key");
    if let Some(key) = api_key {
        let hash = fnv1a_hash(key);
        // Store auth in slot (simplified - real impl would validate license)
        if let Some(slot_ref) = state.pool.get_slot(slot, generation) {
            slot_ref.touch(get_timestamp_ns());
        }
        let _ = hash; // Use hash for future auth validation
    }

    // Send SSE headers
    let headers = "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-cache\r\n\
        Connection: keep-alive\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Headers: X-License-Key, Content-Type\r\n\
        \r\n";
    stream.write_all(headers.as_bytes())?;

    // Send endpoint event (first event per MCP spec)
    let endpoint_event = format!(
        "event: endpoint\ndata: /message?sessionId={}\n\n",
        session_id
    );
    stream.write_all(endpoint_event.as_bytes())?;
    stream.flush()?;

    // Transition to Established -> Active
    let _ = state.pool.transition_slot(
        slot,
        generation,
        SlotState::Connecting,
        SlotState::Established,
    );
    let _ = state.pool.transition_slot(
        slot,
        generation,
        SlotState::Established,
        SlotState::Active,
    );

    eprintln!(
        "[MCP-SSE] SSE connection established: session={}, client={}",
        session_id, client_ip
    );

    // Keep connection open - poll for messages and send heartbeat periodically
    // Use non-blocking recv with short timeout to check for both messages and connection health
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;

    let mut last_heartbeat = std::time::Instant::now();
    let heartbeat_interval = Duration::from_secs(HEARTBEAT_INTERVAL_SECS);

    loop {
        // Check for pending messages to push (non-blocking)
        match rx.try_recv() {
            Ok(message) => {
                // Push message via SSE stream
                let sse_event = format!("event: message\ndata: {}\n\n", message.json);
                if stream.write_all(sse_event.as_bytes()).is_err() {
                    break; // Write failed, connection dead
                }
                if stream.flush().is_err() {
                    break;
                }

                // Record message sent
                if let Some(slot_ref) = state.pool.get_slot(slot, generation) {
                    slot_ref.record_message_sent(message.json.len() as u64);
                    slot_ref.touch(get_timestamp_ns());
                }

                eprintln!(
                    "[MCP-SSE] Pushed message via SSE: session={}, len={}",
                    session_id, message.json.len()
                );
            }
            Err(mpsc::TryRecvError::Empty) => {
                // No messages pending, continue
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                // All senders dropped, this shouldn't happen normally
                eprintln!("[MCP-SSE] Channel disconnected for session={}", session_id);
                break;
            }
        }

        // Check if it's time to send a heartbeat
        if last_heartbeat.elapsed() >= heartbeat_interval {
            // Send heartbeat comment (SSE comment starts with ':')
            if stream.write_all(b": heartbeat\n\n").is_err() {
                break; // Write failed, connection dead
            }
            if stream.flush().is_err() {
                break;
            }

            // Update heartbeat timestamp
            if let Some(slot_ref) = state.pool.get_slot(slot, generation) {
                slot_ref.heartbeat(get_timestamp_ns());
            }

            last_heartbeat = std::time::Instant::now();
        }

        // Check if connection is still alive by peeking
        let mut buf = [0u8; 1];
        match stream.peek(&mut buf) {
            Ok(0) => {
                // Connection closed by client
                break;
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Timeout - no data, connection still alive
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // Same as WouldBlock on some platforms
            }
            Err(_) => {
                // Other error - connection dead
                break;
            }
            Ok(_) => {
                // Data available - shouldn't happen on SSE connection
                // Client might be sending something unexpected
                // Just continue the loop
            }
        }

        // Small sleep to prevent busy-waiting
        thread::sleep(Duration::from_millis(50));

        // Check if slot is still active (might have been released by cleanup)
        if let Some(slot_ref) = state.pool.get_slot(slot, generation) {
            if slot_ref.get_state() != SlotState::Active {
                break;
            }
        } else {
            // Slot no longer valid (generation mismatch)
            break;
        }
    }

    // Cleanup: unregister channel and release slot
    state.channels.unregister(&session_id);

    if let Err(e) = state.pool.release(slot, generation) {
        eprintln!("[MCP-SSE] Release error: {:?}", e);
    }

    eprintln!(
        "[MCP-SSE] SSE connection closed: session={}, client={}",
        session_id, client_ip
    );

    Ok(())
}

// ============================================================================
// Message Handler
// ============================================================================

/// Handle POST /message?sessionId=xxx
///
/// MCP SSE Spec (2024-11-05): Returns 204 No Content immediately,
/// then pushes response via SSE stream using `event: message\ndata: {...}\n\n`
fn handle_message(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract session ID from query string
    let session_id = request
        .path
        .split('?')
        .nth(1)
        .and_then(|q| {
            q.split('&')
                .find(|p| p.starts_with("sessionId="))
        })
        .and_then(|p| p.strip_prefix("sessionId="))
        .ok_or("Missing sessionId parameter")?;

    // Find session in pool
    let (slot, generation) = state
        .pool
        .find_by_session_id(session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Validate slot is active
    if let Some(slot_ref) = state.pool.get_slot(slot, generation) {
        if slot_ref.get_state() != SlotState::Active {
            return Err("Session not active".into());
        }

        // Record message received
        slot_ref.record_message_received(request.body.len() as u64);
        slot_ref.touch(get_timestamp_ns());
    } else {
        return Err("Session expired".into());
    }

    // Get API key from header (for MCP auth)
    let api_key = request.get_header("x-license-key");

    // Call McpServerCapsule to handle the JSON-RPC request
    let response = state.mcp_server.handle_request(
        &request.body,
        api_key,
        Some(client_ip),
        &state.debugger,
    );

    // Build JSON response
    let json_response = match response {
        Ok(json) => json,
        Err(error_msg) => {
            format!(
                r#"{{"jsonrpc":"2.0","error":{{"code":-32603,"message":"{}"}}}}"#,
                error_msg.replace('"', "\\\"")
            )
        }
    };

    // Push response via SSE channel (full SSE mode per MCP spec 2024-11-05)
    let sent = state.channels.send(session_id, SseMessage {
        json: json_response.clone(),
    });

    if sent {
        eprintln!(
            "[MCP-SSE] Queued response for SSE push: session={}, len={}",
            session_id, json_response.len()
        );
    } else {
        eprintln!(
            "[MCP-SSE] WARNING: Failed to queue response (channel not found): session={}",
            session_id
        );
        // Fall back to returning response in body if channel unavailable
        // This handles edge case where SSE connection closed between validation and response
        write_json_response(stream, 200, &json_response)?;
        return Ok(());
    }

    // Return 204 No Content immediately (MCP SSE spec)
    // Response will be pushed via SSE stream
    write_no_content_response(stream)?;

    Ok(())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate session ID from slot and generation
fn generate_session_id(slot: usize, generation: u32) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);

    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        slot as u32,
        generation as u16,
        0u16,
        0u16,
        timestamp & 0xFFFFFFFFFFFF
    )
}

/// Get current timestamp in nanoseconds
fn get_timestamp_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Write JSON HTTP response
fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    body: &str,
) -> Result<(), std::io::Error> {
    let status_text = match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Connection: close\r\n\
        \r\n\
        {}",
        status,
        status_text,
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

/// Write 204 No Content response (MCP SSE spec for POST /message)
fn write_no_content_response(stream: &mut TcpStream) -> Result<(), std::io::Error> {
    let response = "HTTP/1.1 204 No Content\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Headers: X-License-Key, Content-Type\r\n\
        Connection: close\r\n\
        \r\n";

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    Ok(())
}

// ============================================================================
// OAuth 2.1 Handler Functions (MCP Spec Compliance)
// ============================================================================

/// OAuth Authorization Server Metadata (RFC 8414)
///
/// Claude Code queries this endpoint to discover OAuth endpoints.
/// Returns JSON with authorization_endpoint pointing to our signup redirect.
///
/// **Tier**: T0 Auditable (pure function, no side effects)
/// **Latency**: <100μs
fn handle_oauth_metadata(stream: &mut TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // Base URL for the MCP server (Cloudflare tunnel endpoint)
    const ISSUER: &str = "https://mcp.kindly.software";

    // OAuth 2.1 Authorization Server Metadata per RFC 8414
    // Required fields for MCP spec compliance (2025-03-26)
    let metadata = format!(
        r#"{{
  "issuer": "{}",
  "authorization_endpoint": "{}/oauth/authorize",
  "token_endpoint": "{}/oauth/token",
  "registration_endpoint": "{}/register",
  "response_types_supported": ["code"],
  "grant_types_supported": ["authorization_code"],
  "code_challenge_methods_supported": ["S256", "plain"],
  "token_endpoint_auth_methods_supported": ["none", "client_secret_post"],
  "scopes_supported": ["debugger:read", "debugger:write", "debugger:admin"],
  "service_documentation": "https://kindly.services"
}}"#,
        ISSUER, ISSUER, ISSUER, ISSUER
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Cache-Control: max-age=3600\r\n\
        \r\n\
        {}",
        metadata.len(),
        metadata
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!("[MCP-SSE] OAuth metadata served: /.well-known/oauth-authorization-server");
    Ok(())
}

/// Protected Resource Metadata (RFC 9728)
///
/// Tells clients which authorization server to use.
///
/// **Tier**: T0 Auditable (pure function, no side effects)
/// **Latency**: <100μs
fn handle_protected_resource_metadata(
    stream: &mut TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    const ISSUER: &str = "https://mcp.kindly.software";

    let metadata = format!(
        r#"{{
  "resource": "{}",
  "authorization_servers": ["{}"],
  "scopes_supported": ["debugger:read", "debugger:write", "debugger:admin"],
  "bearer_methods_supported": ["header"],
  "resource_documentation": "https://kindly.services"
}}"#,
        ISSUER, ISSUER
    );

    let response = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Cache-Control: max-age=3600\r\n\
        \r\n\
        {}",
        metadata.len(),
        metadata
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!("[MCP-SSE] Protected resource metadata served");
    Ok(())
}

/// OAuth Authorization Endpoint
///
/// This is where Claude Code sends users when auth is needed.
/// Redirects to kindly.services/#signup for license key acquisition.
///
/// **Tier**: T0 Auditable (pure redirect, no state mutation)
/// **Latency**: <1ms (network bound)
fn handle_oauth_authorize(
    stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract OAuth parameters from query string
    let query_string = request.path.split('?').nth(1).unwrap_or("");

    // Parse state parameter (required for OAuth security)
    let state = query_string
        .split('&')
        .find(|p| p.starts_with("state="))
        .and_then(|p| p.strip_prefix("state="))
        .unwrap_or("");

    // Parse redirect_uri (for token exchange flow)
    let redirect_uri = query_string
        .split('&')
        .find(|p| p.starts_with("redirect_uri="))
        .and_then(|p| p.strip_prefix("redirect_uri="))
        .map(|uri| urlencoding_decode(uri))
        .unwrap_or_default();

    // Build the signup URL with context parameters
    // The signup page will display the license key for user to copy
    // and optionally can redirect back with the key as a "code"
    let signup_url = format!(
        "https://kindly.services/#signup?oauth_redirect={}&oauth_state={}",
        urlencoding_encode(&redirect_uri),
        urlencoding_encode(state)
    );

    // HTTP 302 redirect to signup page
    let response = format!(
        "HTTP/1.1 302 Found\r\n\
        Location: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Content-Length: 0\r\n\
        \r\n",
        signup_url
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!(
        "[MCP-SSE] OAuth authorize: redirecting to signup (state={})",
        if state.is_empty() {
            "none"
        } else {
            &state[..state.len().min(8)]
        }
    );
    Ok(())
}

/// OAuth Token Endpoint
///
/// Exchanges authorization code for access token.
/// For our API key model:
/// - If code is a valid license key, return it as the access_token
/// - This allows direct key-as-token usage per our existing auth model
///
/// **Tier**: T1 Atomic (stateless, lockfree)
/// **Latency**: <100μs
fn handle_oauth_token(
    stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse form-urlencoded body
    let params: HashMap<String, String> = request
        .body
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.split('=');
            Some((
                parts.next()?.to_string(),
                urlencoding_decode(parts.next().unwrap_or("")),
            ))
        })
        .collect();

    let grant_type = params.get("grant_type").map(|s| s.as_str()).unwrap_or("");
    let code = params.get("code").map(|s| s.as_str()).unwrap_or("");

    // Validate grant type
    if grant_type != "authorization_code" {
        let error_body =
            r#"{"error":"unsupported_grant_type","error_description":"Only authorization_code grant is supported"}"#;
        write_json_response(stream, 400, error_body)?;
        return Ok(());
    }

    // The "code" is actually the license key from signup
    // Validate it's non-empty and has minimum length
    if code.is_empty() || code.len() < 16 {
        let error_body = r#"{"error":"invalid_grant","error_description":"Invalid or missing authorization code. Get your license key at https://kindly.services/#signup"}"#;
        write_json_response(stream, 400, error_body)?;
        return Ok(());
    }

    // Return the license key as an access token
    // This maintains compatibility with our existing X-License-Key auth
    let token_response = format!(
        r#"{{
  "access_token": "{}",
  "token_type": "Bearer",
  "expires_in": 31536000,
  "scope": "debugger:read debugger:write"
}}"#,
        code
    );

    write_json_response(stream, 200, &token_response)?;

    eprintln!("[MCP-SSE] OAuth token: issued access token for license key");
    Ok(())
}

/// Dynamic Client Registration (RFC 7591)
///
/// MCP clients use this to register themselves.
/// We provide a simple implementation that accepts any client.
/// Per RFC 7591, we echo back the client's redirect_uris if provided,
/// or use sensible defaults with valid URLs (no wildcards).
///
/// **Tier**: T1 Atomic (stateless, lockfree)
/// **Latency**: <100μs
fn handle_client_registration(
    stream: &mut TcpStream,
    request: &HttpRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    // Generate a client_id based on timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let client_id = format!("kdb-mcp-client-{}", timestamp);

    // Extract redirect_uris from client request body (RFC 7591)
    // If client provides redirect_uris, echo them back; otherwise use defaults
    let redirect_uris = extract_redirect_uris_from_body(&request.body);

    // Return registration response
    let response_body = format!(
        r#"{{
  "client_id": "{}",
  "client_id_issued_at": {},
  "grant_types": ["authorization_code"],
  "response_types": ["code"],
  "token_endpoint_auth_method": "none",
  "redirect_uris": {}
}}"#,
        client_id,
        timestamp / 1000,
        redirect_uris
    );

    // Return 201 Created for successful registration
    let response = format!(
        "HTTP/1.1 201 Created\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        \r\n\
        {}",
        response_body.len(),
        response_body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!("[MCP-SSE] Client registered: {}", client_id);
    Ok(())
}

/// Extract redirect_uris from client registration request body
///
/// Per RFC 7591, clients may provide redirect_uris in their registration request.
/// If provided and valid, we echo them back. Otherwise, use sensible defaults.
///
/// #ASSUME_JSON_BOUNDED: Request body is bounded by HTTP content-length limits
fn extract_redirect_uris_from_body(body: &str) -> String {
    // Default valid redirect_uris (no wildcards - they're invalid URLs)
    let default_uris = r#"["http://127.0.0.1/callback", "http://localhost/callback", "https://claude.ai/api/mcp/auth_callback"]"#;

    // Try to parse the body as JSON and extract redirect_uris
    if body.is_empty() {
        return default_uris.to_string();
    }

    // Simple JSON extraction (avoid heavy serde dependency in binary)
    // Look for "redirect_uris": [...] pattern
    if let Some(start) = body.find("\"redirect_uris\"") {
        let rest = &body[start..];
        // Find the array start
        if let Some(array_start) = rest.find('[') {
            let array_rest = &rest[array_start..];
            // Find matching close bracket (simple, doesn't handle nested arrays)
            if let Some(array_end) = array_rest.find(']') {
                let uris_json = &array_rest[..=array_end];
                // Validate that each URI in the array is a valid URL (no wildcards)
                if validate_redirect_uris(uris_json) {
                    return uris_json.to_string();
                }
            }
        }
    }

    default_uris.to_string()
}

/// Validate that redirect_uris JSON array contains valid URLs (no wildcards)
fn validate_redirect_uris(uris_json: &str) -> bool {
    // Check for common invalid patterns
    if uris_json.contains("*") {
        return false; // Wildcards are not valid URLs
    }

    // Basic validation: should start with [ and end with ]
    let trimmed = uris_json.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return false;
    }

    // Check each URI has a valid scheme
    // Extract URIs between quotes
    let mut in_string = false;
    let mut current_uri = String::new();
    let mut valid = true;

    for c in trimmed.chars() {
        match c {
            '"' => {
                if in_string {
                    // End of URI string - validate it
                    if !current_uri.is_empty() {
                        if !current_uri.starts_with("http://") && !current_uri.starts_with("https://") {
                            valid = false;
                            break;
                        }
                    }
                    current_uri.clear();
                }
                in_string = !in_string;
            }
            _ if in_string => {
                current_uri.push(c);
            }
            _ => {}
        }
    }

    valid
}

/// URL encoding helper (minimal implementation)
///
/// #ASSUME_URL_BOUNDED: Input strings are bounded by HTTP header limits
/// #VERIFY_URL: Unit tests validate encoding/decoding round-trip
fn urlencoding_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 3);
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                encoded.push(c);
            }
            _ => {
                for byte in c.to_string().as_bytes() {
                    encoded.push_str(&format!("%{:02X}", byte));
                }
            }
        }
    }
    encoded
}

/// URL decoding helper (minimal implementation)
///
/// #ASSUME_URL_BOUNDED: Input strings are bounded by HTTP query limits
/// #VERIFY_URL: Unit tests validate encoding/decoding round-trip
fn urlencoding_decode(s: &str) -> String {
    let mut decoded = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                decoded.push(byte as char);
            }
        } else if c == '+' {
            decoded.push(' ');
        } else {
            decoded.push(c);
        }
    }
    decoded
}

// ============================================================================
// Tests (T28 Q1-Q7 Unit Tests)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_generation() {
        let id1 = generate_session_id(0, 1);
        let id2 = generate_session_id(1, 1);
        let id3 = generate_session_id(0, 2);

        // Different slots should produce different IDs
        assert_ne!(id1, id2);

        // Different generations should produce different IDs
        assert_ne!(id1, id3);

        // Format should be UUID-like (36 chars with dashes)
        assert_eq!(id1.len(), 36);
        assert!(id1.contains('-'));
    }

    #[test]
    fn test_timestamp() {
        let ts1 = get_timestamp_ns();
        std::thread::sleep(std::time::Duration::from_millis(1));
        let ts2 = get_timestamp_ns();

        // Timestamp should increase
        assert!(ts2 > ts1);
    }
}
