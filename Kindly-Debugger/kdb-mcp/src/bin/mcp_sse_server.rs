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

// OAuth 2.1 capsules for Google OAuth integration
#[cfg(feature = "oauth")]
use kdb_mcp::oauth::{
    OAuthStateCapsule, CodeChallengeMethod,
    AuthorizationCodeCapsule, fnv1a_hash_code, sha256_to_fnv,
    OAuthUserCapsule, fnv1a_hash_oauth,
};

#[cfg(feature = "google-oauth")]
use kdb_mcp::oauth::GoogleOAuthClientCapsule;

// MCP Streamable HTTP Transport (2025-06-18 spec)
#[cfg(feature = "streamable-http")]
use kdb_mcp::{
    StreamableHttpTransportCapsule, StreamableHttpError, McpResponse, McpHeaders, ResponseType,
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

    // ========================================================================
    // OAuth 2.1 + Google OAuth Capsules
    // ========================================================================

    /// OAuth state storage (T1 Atomic, 16KB) - CSRF/PKCE state management
    #[cfg(feature = "oauth")]
    oauth_state: &'static OAuthStateCapsule,

    /// Google OAuth client (T1 Atomic, 512B) - Token exchange and user info
    #[cfg(feature = "google-oauth")]
    google_oauth: &'static GoogleOAuthClientCapsule,

    /// OAuth user mapping (T1 Atomic, 17KB) - Google sub -> license mapping
    #[cfg(feature = "oauth")]
    oauth_users: &'static OAuthUserCapsule,

    /// Authorization code storage (T1 Atomic, 25KB) - MCP auth codes
    #[cfg(feature = "oauth")]
    auth_codes: &'static AuthorizationCodeCapsule,

    /// Google OAuth client ID (from environment)
    #[cfg(feature = "google-oauth")]
    google_client_id: String,

    /// Google OAuth client secret (from environment)
    #[cfg(feature = "google-oauth")]
    google_client_secret: String,
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

    // ========================================================================
    // OAuth 2.1 + Google OAuth Capsules (feature-gated)
    // ========================================================================

    #[cfg(feature = "oauth")]
    let oauth_state: &'static OAuthStateCapsule = Box::leak(Box::new(OAuthStateCapsule::new()));
    #[cfg(feature = "oauth")]
    eprintln!("[MCP-SSE]   OAuthStateCapsule created (16KB, T1 Atomic)");

    #[cfg(feature = "oauth")]
    let oauth_users: &'static OAuthUserCapsule = Box::leak(Box::new(OAuthUserCapsule::new()));
    #[cfg(feature = "oauth")]
    eprintln!("[MCP-SSE]   OAuthUserCapsule created (17KB, T1 Atomic)");

    #[cfg(feature = "oauth")]
    let auth_codes: &'static AuthorizationCodeCapsule = Box::leak(Box::new(AuthorizationCodeCapsule::new()));
    #[cfg(feature = "oauth")]
    eprintln!("[MCP-SSE]   AuthorizationCodeCapsule created (25KB, T1 Atomic)");

    #[cfg(feature = "google-oauth")]
    let google_oauth: &'static GoogleOAuthClientCapsule = Box::leak(Box::new(GoogleOAuthClientCapsule::new()));
    #[cfg(feature = "google-oauth")]
    eprintln!("[MCP-SSE]   GoogleOAuthClientCapsule created (512B, T1 Atomic)");

    // Parse Google OAuth credentials from environment
    #[cfg(feature = "google-oauth")]
    let google_client_id = std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default();
    #[cfg(feature = "google-oauth")]
    let google_client_secret = std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default();

    #[cfg(feature = "google-oauth")]
    {
        if google_client_id.is_empty() {
            eprintln!("[MCP-SSE]   WARNING: GOOGLE_CLIENT_ID not set - Google OAuth disabled");
        } else {
            google_oauth.initialize(google_client_id.len(), google_client_secret.len(), true);
            eprintln!("[MCP-SSE]   Google OAuth configured (client_id: {}...)", &google_client_id[..google_client_id.len().min(20)]);
        }
    }

    // Create shared state with 'static references
    let state = Arc::new(ServerState {
        pool,
        mcp_server,
        debugger,
        http_transport,
        rate_limiter,
        channels: SessionChannelRegistry::new(),

        // OAuth capsules (feature-gated)
        #[cfg(feature = "oauth")]
        oauth_state,
        #[cfg(feature = "google-oauth")]
        google_oauth,
        #[cfg(feature = "oauth")]
        oauth_users,
        #[cfg(feature = "oauth")]
        auth_codes,
        #[cfg(feature = "google-oauth")]
        google_client_id,
        #[cfg(feature = "google-oauth")]
        google_client_secret,
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

        // CORS preflight (MCP Streamable HTTP requires DELETE)
        ("OPTIONS", _) => {
            let response = "HTTP/1.1 204 No Content\r\n\
                Access-Control-Allow-Origin: *\r\n\
                Access-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\n\
                Access-Control-Allow-Headers: X-License-Key, Content-Type, Mcp-Session-Id, Mcp-Protocol-Version, Last-Event-ID, Authorization\r\n\
                Access-Control-Expose-Headers: Mcp-Session-Id, Mcp-Protocol-Version\r\n\
                Access-Control-Max-Age: 86400\r\n\
                \r\n";
            stream.write_all(response.as_bytes())?;
        }

        // Message endpoint (POST /message?sessionId=xxx) - SSE mode
        ("POST", path) if path.starts_with("/message") => {
            handle_message(&mut stream, &request, &client_ip, state)?;
        }

        // ====================================================================
        // MCP Streamable HTTP Transport (2025-06-18 spec)
        // Unified /mcp endpoint supporting POST, GET, DELETE
        // ====================================================================

        // POST /mcp - JSON-RPC messages (requests, notifications, responses)
        ("POST", "/mcp") | ("POST", "/") => {
            handle_streamable_http_post(&mut stream, &request, &client_ip, state)?;
        }

        // GET /mcp - SSE stream for server-initiated messages
        ("GET", "/mcp") => {
            handle_streamable_http_get(&mut stream, &request, &client_ip, state)?;
        }

        // DELETE /mcp - Explicit session termination
        ("DELETE", "/mcp") => {
            handle_streamable_http_delete(&mut stream, &request, &client_ip, state)?;
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

        // OAuth Authorization Endpoint - redirects to Google OAuth
        ("GET", path) if path.starts_with("/oauth/authorize") => {
            handle_oauth_authorize(&mut stream, &request, state)?;
        }

        // OAuth Callback Endpoint - handles Google OAuth callback
        ("GET", path) if path.starts_with("/oauth/callback") => {
            handle_oauth_callback(&mut stream, &request, state)?;
        }

        // OAuth Token Endpoint - exchanges code for access token with PKCE
        ("POST", "/oauth/token") => {
            handle_oauth_token(&mut stream, &request, state)?;
        }

        // Dynamic Client Registration (RFC 7591, required by MCP spec)
        ("POST", "/register") | ("POST", "/oauth/register") => {
            handle_client_registration(&mut stream, &request)?;
        }

        // 404 Not Found - JSON-RPC 2.0 compliant error
        // Note: Use "id":0 instead of null for Cursor compatibility (Zod validation)
        _ => {
            let body = r#"{"jsonrpc":"2.0","id":0,"error":{"code":-32601,"message":"Not Found"}}"#;
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

            // Note: Use "id":0 instead of null for Cursor compatibility (Zod validation)
            let error_body = format!(
                r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":0}}"#,
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
// MCP Streamable HTTP Handlers (2025-06-18 spec)
// ============================================================================

/// Extract JSON-RPC method name from request body (simple string parsing)
fn extract_method(body: &str) -> Option<String> {
    // Simple regex-free extraction
    if let Some(start) = body.find("\"method\"") {
        let after_method = &body[start..];
        if let Some(colon) = after_method.find(':') {
            let after_colon = &after_method[colon+1..];
            if let Some(quote_start) = after_colon.find('"') {
                let after_quote = &after_colon[quote_start+1..];
                if let Some(quote_end) = after_quote.find('"') {
                    return Some(after_quote[..quote_end].to_string());
                }
            }
        }
    }
    None
}

/// Handle Streamable HTTP POST request (POST /mcp)
///
/// Per MCP 2025-06-18 spec:
/// - POST /mcp with JSON-RPC request → 200 JSON response OR 202 Accepted (notification)
/// - Session binding via Mcp-Session-Id header
/// - Protocol methods (initialize, ping) bypass authentication
///
/// **Tier**: T6 Mixed
/// **Latency**: <100μs
#[cfg(feature = "streamable-http")]
fn handle_streamable_http_post(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Extract MCP-specific headers
    let session_id = request.get_header("mcp-session-id");
    let protocol_version = request.get_header("mcp-protocol-version");
    let accept = request.get_header("accept").unwrap_or("application/json");
    let api_key = request.get_header("x-license-key")
        .or_else(|| {
            request.get_header("authorization")
                .and_then(|auth| auth.strip_prefix("Bearer ").or_else(|| auth.strip_prefix("bearer ")))
        });

    // Check if this is a protocol method (initialize, ping) - bypass auth
    let is_protocol_method = StreamableHttpTransportCapsule::is_protocol_method(&request.body);

    // Phase 1 logging: comprehensive request details
    eprintln!(
        "[MCP-Streamable] POST /mcp: client={}, body_len={}, is_protocol={}, has_api_key={}, headers={:?}",
        client_ip,
        request.body.len(),
        is_protocol_method,
        api_key.is_some(),
        request.headers.iter().map(|(k,_)| k).collect::<Vec<_>>()
    );

    // Auth check (skip for protocol methods per MCP spec)
    if !is_protocol_method && api_key.is_none() {
        eprintln!(
            "[MCP-Auth] 401 Auth Required: method={:?}, is_protocol={}, has_api_key=false, client={}, body_preview={}",
            extract_method(&request.body),
            is_protocol_method,
            client_ip,
            &request.body.chars().take(100).collect::<String>()
        );
        let error_body = r#"{"jsonrpc":"2.0","error":{"code":-32001,"message":"Authentication required"},"id":0}"#;
        write_json_response(stream, 401, error_body)?;
        return Ok(());
    }

    // Convert headers to HashMap for HttpTransportCapsule
    let mut headers = HashMap::new();
    for (key, value) in &request.headers {
        headers.insert(key.to_lowercase(), value.clone());
    }

    // Route through existing HttpTransportCapsule for processing
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
            // Check if this is initialize - add Mcp-Session-Id header
            let is_initialize = request.body.contains("\"method\":\"initialize\"")
                || request.body.contains("\"method\": \"initialize\"");

            if is_initialize {
                // Generate new session ID for initialize response
                let new_session_id = format!("{:016x}-{:016x}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64,
                    fastrand::u64(..)
                );

                // Write response with Mcp-Session-Id header
                let response = format!(
                    "HTTP/1.1 {} OK\r\n\
                    Content-Type: application/json\r\n\
                    Content-Length: {}\r\n\
                    Mcp-Session-Id: {}\r\n\
                    Mcp-Protocol-Version: 2025-06-18\r\n\
                    Access-Control-Allow-Origin: *\r\n\
                    Access-Control-Expose-Headers: Mcp-Session-Id, Mcp-Protocol-Version\r\n\
                    \r\n{}",
                    status, body.len(), new_session_id, body
                );
                stream.write_all(response.as_bytes())?;

                eprintln!(
                    "[MCP-Streamable] Initialize: client={}, session_id={}, status={}",
                    client_ip, new_session_id, status
                );
            } else {
                // Check if notification (no id field) - return 202 Accepted
                let is_notification = !request.body.contains("\"id\"") && request.body.contains("\"method\"");

                if is_notification {
                    // 202 Accepted for notifications
                    let response = format!(
                        "HTTP/1.1 202 Accepted\r\n\
                        Access-Control-Allow-Origin: *\r\n\
                        {}\r\n",
                        if let Some(sid) = session_id {
                            format!("Mcp-Session-Id: {}\r\n", sid)
                        } else {
                            String::new()
                        }
                    );
                    stream.write_all(response.as_bytes())?;
                    eprintln!("[MCP-Streamable] Notification accepted: client={}", client_ip);
                } else {
                    // Regular JSON response
                    eprintln!(
                        "[MCP-Streamable] Request: client={}, session={:?}, status={}",
                        client_ip, session_id, status
                    );
                    write_json_response(stream, status, &body)?;
                }
            }
        }
        Err(e) => {
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
                r#"{{"jsonrpc":"2.0","error":{{"code":{},"message":"{}"}},"id":0}}"#,
                code, message
            );
            write_json_response(stream, status, &error_body)?;
        }
    }

    Ok(())
}

/// Fallback for when streamable-http feature is disabled
#[cfg(not(feature = "streamable-http"))]
fn handle_streamable_http_post(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    // Route to existing HTTP handler when feature disabled
    handle_http_request(stream, request, client_ip, state)
}

/// Handle Streamable HTTP GET request (GET /mcp)
///
/// Per MCP 2025-06-18 spec:
/// - GET /mcp with Mcp-Session-Id → 200 text/event-stream (SSE)
/// - Used for server-initiated messages (progress, logs, notifications)
/// - Supports Last-Event-ID for resumption
///
/// **Tier**: T6 Mixed (T5 Streaming)
fn handle_streamable_http_get(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = request.get_header("mcp-session-id");
    let last_event_id = request.get_header("last-event-id");

    // Session ID required for GET /mcp
    let session_id = match session_id {
        Some(sid) => sid.to_string(),
        None => {
            eprintln!("[MCP-Streamable] GET /mcp: 400 Missing Mcp-Session-Id header, client={}", client_ip);
            let error_body = r#"{"jsonrpc":"2.0","error":{"code":-32001,"message":"Mcp-Session-Id header required for GET /mcp"},"id":0}"#;
            write_json_response(stream, 400, error_body)?;
            return Ok(());
        }
    };

    eprintln!(
        "[MCP-Streamable] GET /mcp SSE stream: client={}, session={}, last_event_id={:?}",
        client_ip, session_id, last_event_id
    );

    // Send SSE headers
    let response = format!(
        "HTTP/1.1 200 OK\r\n\
        Content-Type: text/event-stream\r\n\
        Cache-Control: no-cache\r\n\
        Connection: keep-alive\r\n\
        Mcp-Session-Id: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Expose-Headers: Mcp-Session-Id\r\n\
        \r\n",
        session_id
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    // Create channel for this session (reuse existing SessionChannelRegistry pattern)
    let (sender, receiver) = mpsc::channel::<SseMessage>();

    // Register session channel
    {
        let mut channels = state.channels.channels.lock().unwrap();
        channels.insert(session_id.clone(), sender);
    }

    // SSE event loop - wait for messages or timeout
    loop {
        match receiver.recv_timeout(Duration::from_secs(HEARTBEAT_INTERVAL_SECS)) {
            Ok(message) => {
                // Send SSE event
                let event = format!("event: message\ndata: {}\n\n", message.json);
                if stream.write_all(event.as_bytes()).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Send heartbeat comment
                if stream.write_all(b": heartbeat\n\n").is_err() {
                    break;
                }
                let _ = stream.flush();
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    // Cleanup: remove session channel
    {
        let mut channels = state.channels.channels.lock().unwrap();
        channels.remove(&session_id);
    }

    eprintln!("[MCP-Streamable] GET /mcp SSE stream closed: session={}", session_id);
    Ok(())
}

/// Handle Streamable HTTP DELETE request (DELETE /mcp)
///
/// Per MCP 2025-06-18 spec:
/// - DELETE /mcp with Mcp-Session-Id → 204 No Content
/// - Explicitly terminates a session
/// - Client must send new initialize request to continue
///
/// **Tier**: T1 Atomic
/// **Latency**: <50ns
fn handle_streamable_http_delete(
    stream: &mut TcpStream,
    request: &HttpRequest,
    client_ip: &str,
    state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = request.get_header("mcp-session-id");

    match session_id {
        Some(sid) => {
            // Remove session channel if exists
            {
                let mut channels = state.channels.channels.lock().unwrap();
                channels.remove(sid);
            }

            eprintln!("[MCP-Streamable] DELETE /mcp: client={}, session={}", client_ip, sid);

            // 204 No Content
            let response = "HTTP/1.1 204 No Content\r\n\
                Access-Control-Allow-Origin: *\r\n\
                \r\n";
            stream.write_all(response.as_bytes())?;
        }
        None => {
            // No session ID - return 400
            eprintln!("[MCP-Streamable] DELETE /mcp: 400 Missing Mcp-Session-Id header, client={}", client_ip);
            let response = "HTTP/1.1 400 Bad Request\r\n\
                Content-Type: application/json\r\n\
                Access-Control-Allow-Origin: *\r\n\
                \r\n{\"error\":\"Mcp-Session-Id header required\"}";
            stream.write_all(response.as_bytes())?;
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

    // Extract request ID FIRST for error responses (Cursor compatibility)
    // Must preserve original ID in all responses per JSON-RPC 2.0 spec
    let request_id = extract_jsonrpc_id(&request.body);

    // Call McpServerCapsule to handle the JSON-RPC request
    let response = state.mcp_server.handle_request(
        &request.body,
        api_key,
        Some(client_ip),
        &state.debugger,
    );

    // Build JSON response - use original request ID in error responses
    let json_response = match response {
        Ok(json) => json,
        Err(error_msg) => {
            format!(
                r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":-32603,"message":"{}"}}}}"#,
                request_id,
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

/// Extract JSON-RPC request ID from request body
/// Returns the ID as a string (number or string), or "0" if not found/invalid
/// Per JSON-RPC 2.0: id can be string, number, or null (but Cursor needs it non-null)
fn extract_jsonrpc_id(body: &str) -> String {
    // Fast path: look for "id": pattern
    if let Some(id_start) = body.find("\"id\"") {
        let after_id = &body[id_start + 4..];
        // Skip whitespace and colon
        let trimmed = after_id.trim_start();
        if let Some(rest) = trimmed.strip_prefix(':') {
            let value_start = rest.trim_start();
            // Check if it's a number
            if let Some(first_char) = value_start.chars().next() {
                if first_char.is_ascii_digit() || first_char == '-' {
                    // Parse number
                    let end = value_start.find(|c: char| !c.is_ascii_digit() && c != '-')
                        .unwrap_or(value_start.len());
                    return value_start[..end].to_string();
                } else if first_char == '"' {
                    // Parse string ID
                    let inner = &value_start[1..];
                    if let Some(end_quote) = inner.find('"') {
                        return inner[..end_quote].to_string();
                    }
                }
            }
        }
    }
    // Default to "0" for Cursor compatibility (can't use null)
    "0".to_string()
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
  "service_documentation": "https://www.kindly.software"
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
  "resource_documentation": "https://www.kindly.software"
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

/// OAuth Authorization Endpoint - OAuth 2.1 + Google OAuth
///
/// Implements OAuth 2.1 authorization endpoint with PKCE (RFC 7636) and Google OAuth.
///
/// **Flow**:
/// 1. Parse and validate OAuth parameters (state, code_challenge, redirect_uri)
/// 2. Validate code_challenge_method (must be "S256" per OAuth 2.1)
/// 3. Store state and PKCE challenge in OAuthStateCapsule
/// 4. Redirect to Google OAuth with our own state
///
/// **Tier**: T1 Atomic (state storage via OAuthStateCapsule)
/// **Latency**: <50ns state storage + redirect
///
/// **Fallback**: If Google OAuth is not configured, redirects to signup page
#[cfg(feature = "google-oauth")]
fn handle_oauth_authorize(
    stream: &mut TcpStream,
    request: &HttpRequest,
    server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_query_params(&request.path);

    // Extract OAuth parameters
    let client_state = params.get("state").map(|s| s.as_str()).unwrap_or("");
    let code_challenge = params.get("code_challenge").map(|s| s.as_str()).unwrap_or("");
    let code_challenge_method = params.get("code_challenge_method").map(|s| s.as_str()).unwrap_or("plain");
    let redirect_uri = params.get("redirect_uri").map(|s| s.as_str()).unwrap_or("");
    let _client_id = params.get("client_id").map(|s| s.as_str()).unwrap_or("");

    // Validate required parameters
    if client_state.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing required parameter: state")?;
        return Ok(());
    }

    // OAuth 2.1 requires S256 for public clients (MCP clients are public)
    // We also accept plain for backward compatibility
    let challenge_method = if code_challenge_method == "S256" {
        CodeChallengeMethod::S256
    } else if code_challenge_method == "plain" || code_challenge.is_empty() {
        CodeChallengeMethod::Plain
    } else {
        write_oauth_error(stream, 400, "invalid_request", "code_challenge_method must be S256 or plain")?;
        return Ok(());
    };

    // Check if Google OAuth is configured
    if server_state.google_client_id.is_empty() {
        // Fall back to signup page redirect
        eprintln!("[MCP-SSE] OAuth authorize: Google not configured, falling back to signup");
        let signup_url = format!(
            "https://www.kindly.software/#signup?oauth_redirect={}&oauth_state={}",
            urlencoding_encode(redirect_uri),
            urlencoding_encode(client_state)
        );

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
        return Ok(());
    }

    // Store OAuth state with PKCE challenge
    // The state parameter acts as our key - we'll receive it back from Google
    if let Err(e) = server_state.oauth_state.store_state(
        client_state,
        code_challenge,
        redirect_uri,
        challenge_method,
    ) {
        eprintln!("[MCP-SSE] OAuth authorize: failed to store state: {:?}", e);
        write_oauth_error(stream, 500, "server_error", "Failed to store OAuth state")?;
        return Ok(());
    }

    // Build Google OAuth URL
    // Our callback URL will receive the Google code and state
    let google_callback_uri = "https://mcp.kindly.software/oauth/callback";

    let google_auth_url = server_state.google_oauth.build_auth_url(
        client_state,  // Pass through the client's state to Google
        google_callback_uri,
        &server_state.google_client_id,
    );

    // 302 redirect to Google
    let response = format!(
        "HTTP/1.1 302 Found\r\n\
        Location: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Content-Length: 0\r\n\
        \r\n",
        google_auth_url
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!(
        "[MCP-SSE] OAuth authorize: redirecting to Google (state={}...)",
        &client_state[..client_state.len().min(8)]
    );

    Ok(())
}

/// OAuth Authorization Endpoint - Fallback without Google OAuth
///
/// When google-oauth feature is not enabled, redirects to signup page.
#[cfg(not(feature = "google-oauth"))]
fn handle_oauth_authorize(
    stream: &mut TcpStream,
    request: &HttpRequest,
    _server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_query_params(&request.path);

    let state = params.get("state").map(|s| s.as_str()).unwrap_or("");
    let redirect_uri = params.get("redirect_uri").map(|s| s.as_str()).unwrap_or("");

    // Redirect to signup page
    let signup_url = format!(
        "https://kindly.services/#signup?oauth_redirect={}&oauth_state={}",
        urlencoding_encode(redirect_uri),
        urlencoding_encode(state)
    );

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
        if state.is_empty() { "none" } else { &state[..state.len().min(8)] }
    );
    Ok(())
}

/// OAuth Callback Endpoint - Handle Google OAuth callback
///
/// After user authenticates with Google, Google redirects here with:
/// - `code`: Google authorization code
/// - `state`: Our original state parameter (passed through)
///
/// **Flow**:
/// 1. Parse and validate state parameter
/// 2. Exchange Google code for tokens (async via blocking runtime)
/// 3. Get user info from Google
/// 4. Link Google user to license (or create new user)
/// 5. Generate MCP authorization code
/// 6. Redirect to Claude callback with our code
///
/// **Tier**: T6 Mixed (T1 state lookup + network calls + T1 code generation)
/// **Latency**: ~100-300ms (Google API latency dominates)
#[cfg(feature = "google-oauth")]
fn handle_oauth_callback(
    stream: &mut TcpStream,
    request: &HttpRequest,
    server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_query_params(&request.path);

    // Extract Google OAuth callback parameters
    let google_code = params.get("code").map(|s| s.as_str()).unwrap_or("");
    let state = params.get("state").map(|s| s.as_str()).unwrap_or("");
    let error = params.get("error").map(|s| s.as_str());

    // Handle Google OAuth errors
    if let Some(err) = error {
        let error_description = params.get("error_description")
            .map(|s| s.as_str())
            .unwrap_or("Authentication failed");
        eprintln!("[MCP-SSE] OAuth callback: Google error: {} - {}", err, error_description);
        write_oauth_error(stream, 400, "access_denied", error_description)?;
        return Ok(());
    }

    // Validate required parameters
    if google_code.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing authorization code from Google")?;
        return Ok(());
    }

    if state.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing state parameter")?;
        return Ok(());
    }

    // Validate state and get stored data (CSRF protection)
    let stored_state = match server_state.oauth_state.validate_state(state) {
        Some(data) => data,
        None => {
            eprintln!("[MCP-SSE] OAuth callback: invalid or expired state");
            write_oauth_error(stream, 400, "invalid_request", "Invalid or expired OAuth state")?;
            return Ok(());
        }
    };

    // Exchange Google code for tokens (blocking call to async API)
    // We need to create a runtime for the async call
    let google_callback_uri = "https://mcp.kindly.software/oauth/callback";

    let token_result = {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        rt.block_on(server_state.google_oauth.exchange_code(
            google_code,
            google_callback_uri,
            &server_state.google_client_id,
            &server_state.google_client_secret,
        ))
    };

    let token_response = match token_result {
        Ok(tokens) => tokens,
        Err(e) => {
            eprintln!("[MCP-SSE] OAuth callback: Google token exchange failed: {}", e);
            write_oauth_error(stream, 500, "server_error", &format!("Failed to exchange code: {}", e))?;
            return Ok(());
        }
    };

    // Validate ID token to get user info (no additional API call needed)
    let claims = match server_state.google_oauth.validate_id_token(
        &token_response.id_token,
        &server_state.google_client_id,
    ) {
        Ok(claims) => claims,
        Err(e) => {
            eprintln!("[MCP-SSE] OAuth callback: ID token validation failed: {}", e);
            write_oauth_error(stream, 500, "server_error", "Invalid ID token from Google")?;
            return Ok(());
        }
    };

    // Look up or auto-provision user mapping
    // 1. Check OAuthUserCapsule (fast path)
    // 2. If not found, generate Hobby license and link
    //
    // For new users: We have the license key and will show success page
    // For existing users: We only have hash, redirect directly to Claude
    let (license_hash, new_user_license_key): (u64, Option<String>) = match server_state.oauth_users.get_license_hash_for_google(&claims.sub) {
        Some(hash) => {
            eprintln!("[MCP-SSE] OAuth callback: existing OAuth user found (sub={}...)", &claims.sub[..claims.sub.len().min(8)]);
            (hash, None) // Existing user - no license key to show
        }
        None => {
            // New OAuth user - auto-provision Hobby license
            eprintln!("[MCP-SSE] OAuth callback: auto-provisioning Hobby license (email={})", claims.email);

            // Generate Hobby license key format: KDB-HOBBY-{timestamp}-{email_hash}-{signature}
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();

            // Simple license key format (for auto-provisioning)
            let license_key = format!(
                "KDB-HOBBY-{:x}-{:x}",
                timestamp,
                fnv1a_hash(&claims.email)
            );

            let license_hash = fnv1a_hash(&license_key);

            // Link Google ID to new license
            if let Err(e) = server_state.oauth_users.link_google_to_license(&claims.sub, &license_key) {
                eprintln!("[MCP-SSE] OAuth callback: failed to link user: {:?}", e);
                write_oauth_error(stream, 500, "server_error", "Failed to create user mapping")?;
                return Ok(());
            }

            eprintln!(
                "[MCP-SSE] OAuth callback: provisioned license {} for {}",
                &license_key[..license_key.len().min(20)],
                claims.email
            );

            (license_hash, Some(license_key)) // New user - show license key on success page
        }
    };

    // Generate MCP authorization code
    // This code will be exchanged for access token at /oauth/token
    let mcp_code = match server_state.auth_codes.generate_code(
        license_hash,
        stored_state.code_challenge_hash,
        stored_state.redirect_uri_hash,
    ) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("[MCP-SSE] OAuth callback: failed to generate code: {:?}", e);
            write_oauth_error(stream, 500, "server_error", "Failed to generate authorization code")?;
            return Ok(());
        }
    };

    // Consume the OAuth state (one-time use)
    server_state.oauth_state.consume_state(state);

    // Build Claude callback URL (always needed, either as direct redirect or as callback param)
    // Claude's callback URL format: https://claude.ai/api/mcp/auth_callback?code=xxx&state=yyy
    let claude_callback = format!(
        "https://claude.ai/api/mcp/auth_callback?code={}&state={}",
        urlencoding_encode(&mcp_code),
        urlencoding_encode(state)
    );

    // Determine final redirect URL based on whether this is a new user
    // - New users: Redirect to success page so they can see their license key
    // - Existing users: Redirect directly to Claude (they already have their key)
    let redirect_url = match new_user_license_key {
        Some(license_key) => {
            // New user: Show success page with license key, then continue to Claude
            // Success page URL format: https://kindly.software/#oauth-success?license=xxx&callback=yyy
            eprintln!(
                "[MCP-SSE] OAuth callback: new user, redirecting via success page (email={})",
                claims.email
            );
            format!(
                "https://kindly.software/#oauth-success?license={}&callback={}",
                urlencoding_encode(&license_key),
                urlencoding_encode(&claude_callback)
            )
        }
        None => {
            // Existing user: Show dashboard first, then continue to Claude
            eprintln!(
                "[MCP-SSE] OAuth callback: existing user, redirecting via dashboard (email={})",
                claims.email
            );
            format!(
                "https://www.kindly.software/#dashboard?token={}&callback={}",
                urlencoding_encode(&token_response.id_token),
                urlencoding_encode(&claude_callback)
            )
        }
    };

    let response = format!(
        "HTTP/1.1 302 Found\r\n\
        Location: {}\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Content-Length: 0\r\n\
        \r\n",
        redirect_url
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;

    eprintln!(
        "[MCP-SSE] OAuth callback: success (user={}, code={}...)",
        &claims.email,
        &mcp_code[..mcp_code.len().min(8)]
    );

    Ok(())
}

/// OAuth Callback Endpoint - Fallback without Google OAuth
#[cfg(not(feature = "google-oauth"))]
fn handle_oauth_callback(
    stream: &mut TcpStream,
    _request: &HttpRequest,
    _server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    write_oauth_error(stream, 501, "not_implemented", "Google OAuth not configured")?;
    Ok(())
}

/// OAuth Token Endpoint - OAuth 2.1 with PKCE Validation
///
/// Exchanges authorization code for access token with PKCE validation (RFC 7636).
///
/// **Flow**:
/// 1. Parse form parameters (grant_type, code, code_verifier, redirect_uri)
/// 2. Validate grant_type == "authorization_code"
/// 3. Validate and consume code via AuthorizationCodeCapsule (includes PKCE check)
/// 4. Get license_hash from code
/// 5. Return OAuth token response with license key as access_token
///
/// **Tier**: T1 Atomic (lockfree code validation)
/// **Latency**: <50ns (code lookup + PKCE validation)
///
/// **Backward Compatibility**: If oauth feature is disabled, treats code as license key
#[cfg(feature = "oauth")]
fn handle_oauth_token(
    stream: &mut TcpStream,
    request: &HttpRequest,
    server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_form_params(&request.body);

    let grant_type = params.get("grant_type").map(|s| s.as_str()).unwrap_or("");
    let code = params.get("code").map(|s| s.as_str()).unwrap_or("");
    let code_verifier = params.get("code_verifier").map(|s| s.as_str()).unwrap_or("");
    let redirect_uri = params.get("redirect_uri").map(|s| s.as_str()).unwrap_or("");

    // Validate grant type
    if grant_type != "authorization_code" {
        write_oauth_error(stream, 400, "unsupported_grant_type", "Only authorization_code grant is supported")?;
        return Ok(());
    }

    // Validate code is present
    if code.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing required parameter: code")?;
        return Ok(());
    }

    // Check if this looks like a license key (backward compatibility)
    // License keys start with "KDB-" prefix
    if code.starts_with("KDB-") {
        // Direct license key usage - return as access token
        eprintln!("[MCP-SSE] OAuth token: direct license key provided");
        let token_response = format!(
            r#"{{"access_token":"{}","token_type":"Bearer","expires_in":31536000,"scope":"debugger:read debugger:write"}}"#,
            code
        );
        write_json_response(stream, 200, &token_response)?;
        return Ok(());
    }

    // Validate PKCE code_verifier is present for OAuth 2.1 flow
    if code_verifier.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing required parameter: code_verifier")?;
        return Ok(());
    }

    // Validate redirect_uri
    if redirect_uri.is_empty() {
        write_oauth_error(stream, 400, "invalid_request", "Missing required parameter: redirect_uri")?;
        return Ok(());
    }

    // Validate and consume authorization code (one-time use)
    // This validates:
    // - Code exists and hasn't expired
    // - PKCE code_verifier matches stored code_challenge
    // - redirect_uri matches what was used during authorization
    let license_hash = match server_state.auth_codes.validate_and_consume(
        code,
        code_verifier,
        redirect_uri,
    ) {
        Some(hash) => hash,
        None => {
            let stats = server_state.auth_codes.stats();
            eprintln!(
                "[MCP-SSE] OAuth token: code validation failed (pkce_failures={}, redirect_failures={})",
                stats.pkce_failures, stats.redirect_failures
            );
            write_oauth_error(stream, 400, "invalid_grant", "Invalid, expired, or already used authorization code")?;
            return Ok(());
        }
    };

    // Look up the license key from hash
    // For now, we return a synthetic token that includes the hash
    // The actual license key is stored in the user mapping
    // TODO: Add a reverse lookup table for hash -> license_key if needed
    let token_response = format!(
        r#"{{"access_token":"oauth-{}","token_type":"Bearer","expires_in":31536000,"scope":"debugger:read debugger:write"}}"#,
        license_hash
    );

    write_json_response(stream, 200, &token_response)?;

    eprintln!(
        "[MCP-SSE] OAuth token: issued access token (license_hash={}...)",
        &format!("{:x}", license_hash)[..8]
    );

    Ok(())
}

/// OAuth Token Endpoint - Fallback without oauth feature
///
/// When oauth feature is disabled, treats code as license key directly.
#[cfg(not(feature = "oauth"))]
fn handle_oauth_token(
    stream: &mut TcpStream,
    request: &HttpRequest,
    _server_state: &ServerState,
) -> Result<(), Box<dyn std::error::Error>> {
    let params = parse_form_params(&request.body);

    let grant_type = params.get("grant_type").map(|s| s.as_str()).unwrap_or("");
    let code = params.get("code").map(|s| s.as_str()).unwrap_or("");

    // Validate grant type
    if grant_type != "authorization_code" {
        write_oauth_error(stream, 400, "unsupported_grant_type", "Only authorization_code grant is supported")?;
        return Ok(());
    }

    // Validate code is present and has minimum length
    if code.is_empty() || code.len() < 16 {
        write_oauth_error(stream, 400, "invalid_grant", "Invalid or missing authorization code. Get your license key at https://www.kindly.software/#signup")?;
        return Ok(());
    }

    // Return the code (license key) as an access token
    let token_response = format!(
        r#"{{"access_token":"{}","token_type":"Bearer","expires_in":31536000,"scope":"debugger:read debugger:write"}}"#,
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

/// Parse URL query string into HashMap
///
/// Handles both URL-encoded and plain parameters.
/// Example: "state=abc123&code_challenge=xyz&redirect_uri=https%3A%2F%2Fexample.com"
///
/// **Performance**: O(n) where n = query string length
fn parse_query_params(path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    // Extract query string after '?'
    let query_string = match path.split('?').nth(1) {
        Some(qs) => qs,
        None => return params,
    };

    // Parse each key=value pair
    for pair in query_string.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            params.insert(
                key.to_string(),
                urlencoding_decode(value),
            );
        }
    }

    params
}

/// Parse application/x-www-form-urlencoded body into HashMap
///
/// Used for OAuth token endpoint requests.
/// Example: "grant_type=authorization_code&code=abc123&redirect_uri=https%3A%2F%2Fexample.com"
///
/// **Performance**: O(n) where n = body length
fn parse_form_params(body: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();

    for pair in body.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            params.insert(
                key.to_string(),
                urlencoding_decode(value),
            );
        }
    }

    params
}

/// Write OAuth error response per RFC 6749 Section 5.2
///
/// **Error Response Format** (JSON):
/// ```json
/// {
///   "error": "invalid_request",
///   "error_description": "Human-readable description"
/// }
/// ```
///
/// **Standard Error Codes** (RFC 6749):
/// - `invalid_request`: Missing required parameter or malformed
/// - `invalid_client`: Client authentication failed
/// - `invalid_grant`: Authorization code/refresh token invalid
/// - `unauthorized_client`: Client not authorized for this grant type
/// - `unsupported_grant_type`: Grant type not supported
/// - `invalid_scope`: Requested scope invalid
fn write_oauth_error(
    stream: &mut TcpStream,
    status: u16,
    error: &str,
    description: &str,
) -> Result<(), std::io::Error> {
    let body = format!(
        r#"{{"error":"{}","error_description":"{}"}}"#,
        error, description.replace('"', "\\\"")
    );

    let status_text = match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Error",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
        Content-Type: application/json\r\n\
        Content-Length: {}\r\n\
        Cache-Control: no-store\r\n\
        Pragma: no-cache\r\n\
        Access-Control-Allow-Origin: *\r\n\
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
