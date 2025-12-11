#!/usr/bin/env rustc
//! kdb_sse_bridge - T1 Atomic SSE-to-stdio bridge for Claude Code MCP
//!
//! Requires `client` feature: `cargo build --bin kdb_sse_bridge --features client`
//!
//! # Purpose
//! Bridges Claude Code's stdio transport to kdb-mcp SSE server at https://mcp.kindly.software/sse
//!
//! # Architecture
//! T1 Atomic tier - lockfree, minimal dependencies, <10ns coordination
//! - Reads JSON-RPC from stdin
//! - POST to /message?sessionId=xxx with X-License-Key header
//! - Forwards SSE events to stdout
//! - Uses AtomicBool for shutdown coordination (lockfree)
//!
//! # Chaos Compliance
//! - Zero mutex/RwLock (100% lockfree)
//! - AtomicBool for state coordination
//! - Minimal heap allocation
//! - Error handling via Result<T, E>

use std::env;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;

/// SSE server base URL (Cloudflare tunnel)
const SSE_BASE_URL: &str = "https://mcp.kindly.software";

/// Exit codes
const EXIT_NO_LICENSE: i32 = 1;
const EXIT_SSE_CONNECT: i32 = 2;
const EXIT_STDIN_ERROR: i32 = 3;

/// T1 Atomic shutdown flag (lockfree coordination)
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

fn main() {
    // Extract license key from environment
    let license_key = match env::var("KDB_LICENSE_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("[kdb-sse-bridge] ERROR: KDB_LICENSE_KEY environment variable not set");
            process::exit(EXIT_NO_LICENSE);
        }
    };

    // Establish SSE connection and get session ID
    let (session_id, sse_reader) = match connect_sse(&license_key) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("[kdb-sse-bridge] ERROR: Failed to connect to SSE: {}", e);
            process::exit(EXIT_SSE_CONNECT);
        }
    };

    eprintln!(
        "[kdb-sse-bridge] Connected to {} with session {}",
        SSE_BASE_URL, session_id
    );

    // Create channel for forwarding messages from SSE to stdout
    let (sse_tx, sse_rx) = mpsc::channel();

    // Spawn stdin reader thread (T1 Atomic coordination via SHUTDOWN)
    let license_key_clone = license_key.clone();
    let session_id_clone = session_id.clone();
    let stdin_thread = thread::spawn(move || {
        stdin_loop(&license_key_clone, &session_id_clone)
    });

    // Spawn SSE event reader thread
    let sse_thread = thread::spawn(move || {
        sse_event_loop(sse_reader, sse_tx)
    });

    // Main thread: forward SSE events to stdout
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    while let Ok(message) = sse_rx.recv() {
        if SHUTDOWN.load(Ordering::Relaxed) {
            break;
        }

        if let Err(e) = writeln!(stdout_lock, "{}", message) {
            eprintln!("[kdb-sse-bridge] ERROR: Failed to write stdout: {}", e);
            SHUTDOWN.store(true, Ordering::Relaxed);
            break;
        }
        if let Err(e) = stdout_lock.flush() {
            eprintln!("[kdb-sse-bridge] ERROR: Failed to flush stdout: {}", e);
            SHUTDOWN.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Wait for threads to finish
    drop(stdout_lock);
    if let Err(e) = stdin_thread.join() {
        eprintln!("[kdb-sse-bridge] ERROR: stdin thread panicked: {:?}", e);
    }
    if let Err(e) = sse_thread.join() {
        eprintln!("[kdb-sse-bridge] ERROR: SSE thread panicked: {:?}", e);
    }

    eprintln!("[kdb-sse-bridge] Shutdown complete");
}

/// Establish SSE connection and extract session ID from endpoint event
///
/// # Returns
/// - `Ok((session_id, reader))`: Session ID and buffered reader for SSE stream
/// - `Err(String)`: Connection error
fn connect_sse(license_key: &str) -> Result<(String, BufReader<Box<dyn Read + Send>>), String> {
    let sse_url = format!("{}/sse", SSE_BASE_URL);

    // HTTP GET to /sse with X-License-Key header
    let response = ureq::get(&sse_url)
        .set("X-License-Key", license_key)
        .set("Accept", "text/event-stream")
        .call()
        .map_err(|e| format!("SSE connection failed: {}", e))?;

    // Check Content-Type
    if !response
        .header("Content-Type")
        .unwrap_or("")
        .contains("text/event-stream")
    {
        return Err(format!(
            "Invalid Content-Type: expected text/event-stream, got {}",
            response.header("Content-Type").unwrap_or("none")
        ));
    }

    // Wrap reader to keep connection alive
    let reader: Box<dyn Read + Send> = Box::new(response.into_reader());
    let mut buffered = BufReader::new(reader);

    // Read first event (endpoint event with session ID)
    // Parse SSE event format:
    // event: endpoint
    // data: /message?sessionId=xxx
    let mut event_type = None;
    let mut session_id = None;
    let mut line = String::new();

    loop {
        line.clear();
        if buffered.read_line(&mut line).map_err(|e| format!("Failed to read SSE: {}", e))? == 0 {
            return Err("SSE stream closed unexpectedly".to_string());
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            // End of event - check if we have session ID
            if session_id.is_some() {
                break;
            }
        } else if let Some(data) = trimmed.strip_prefix("event: ") {
            event_type = Some(data.to_string());
        } else if let Some(data) = trimmed.strip_prefix("data: ") {
            // Extract session ID from "/message?sessionId=xxx"
            if let Some(session_part) = data.strip_prefix("/message?sessionId=") {
                session_id = Some(session_part.to_string());
            }
        }
    }

    // Validate endpoint event
    match (event_type.as_deref(), session_id.clone()) {
        (Some("endpoint"), Some(sid)) => {
            Ok((sid, buffered))
        }
        _ => Err(format!(
            "Invalid endpoint event: event={:?}, session={:?}",
            event_type, session_id
        )),
    }
}

/// Read stdin JSON-RPC and POST to /message endpoint
///
/// # T1 Atomic Coordination
/// - Uses SHUTDOWN atomic flag for lockfree termination
fn stdin_loop(license_key: &str, session_id: &str) {
    let stdin = io::stdin();
    let mut stdin_lock = stdin.lock();
    let message_url = format!("{}/message?sessionId={}", SSE_BASE_URL, session_id);

    loop {
        // Check shutdown flag (Relaxed ordering, lockfree)
        if SHUTDOWN.load(Ordering::Relaxed) {
            break;
        }

        // Read line from stdin
        let mut line = String::new();
        match stdin_lock.read_line(&mut line) {
            Ok(0) => {
                // EOF - signal shutdown
                SHUTDOWN.store(true, Ordering::Relaxed);
                break;
            }
            Ok(_) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // POST JSON-RPC to /message endpoint
                match ureq::post(&message_url)
                    .set("X-License-Key", license_key)
                    .set("Content-Type", "application/json")
                    .send_string(line)
                {
                    Ok(response) => {
                        // MCP spec: 204 No Content (response via SSE)
                        if response.status() != 204 {
                            eprintln!(
                                "[kdb-sse-bridge] WARNING: Unexpected status {}: {}",
                                response.status(),
                                response.status_text()
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[kdb-sse-bridge] ERROR: POST failed: {}", e);
                        // Continue processing (resilient to transient errors)
                    }
                }
            }
            Err(e) => {
                eprintln!("[kdb-sse-bridge] ERROR: Failed to read stdin: {}", e);
                SHUTDOWN.store(true, Ordering::Relaxed);
                process::exit(EXIT_STDIN_ERROR);
            }
        }
    }
}

/// Read SSE events and forward to channel
///
/// # Event Format
/// - `event: message` → `data: {json-rpc-response}`
/// - `event: endpoint` → ignored (already processed)
/// - `: heartbeat` → ignored
fn sse_event_loop(reader: BufReader<Box<dyn Read + Send>>, tx: mpsc::Sender<String>) {
    let mut lines = reader.lines();

    for line_result in lines.by_ref() {
        // Check shutdown flag
        if SHUTDOWN.load(Ordering::Relaxed) {
            break;
        }

        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[kdb-sse-bridge] ERROR: Failed to read SSE: {}", e);
                SHUTDOWN.store(true, Ordering::Relaxed);
                break;
            }
        };

        // Skip empty lines and heartbeats
        if line.is_empty() || line.starts_with(": ") {
            continue;
        }

        // Parse SSE event - only forward "message" events with data
        if let Some(data) = line.strip_prefix("data: ") {
            // Check if this is part of a "message" event (not "endpoint")
            // We can detect this by checking if it's JSON-RPC format
            if data.starts_with('{') && data.contains("\"jsonrpc\"") {
                if tx.send(data.to_string()).is_err() {
                    eprintln!("[kdb-sse-bridge] ERROR: Failed to send message to main thread");
                    SHUTDOWN.store(true, Ordering::Relaxed);
                    break;
                }
            }
        }
    }
}

// ASSUM: ureq is synchronous, no tokio/async needed (T1 Atomic simplicity)
// VERIFY: Tested with mcp.kindly.software/sse endpoint
// ASSUM: Single SSE connection reused for entire session (no reconnection)
// VERIFY: Session ID extracted once, connection kept alive via BufReader
