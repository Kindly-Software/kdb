//! MCP Server Main Entry Point
//!
//! Demonstrates full integration of McpRuntimeCapsule (T6 Mixed orchestration):
//! - Initializes all subsystem capsules
//! - Starts the async runtime event loop
//! - Handles graceful shutdown
//! - Monitors performance metrics
//!
//! **Usage**:
//! ```bash
//! cargo run --example mcp_server_main --features "std,json-rpc,async-runtime"
//! ```

use kdb_mcp::{
    McpRuntimeCapsule, McpServerCapsule, StdioTransportCapsule, ToolExecutorCapsule,
};
use kdb::DebuggerCapsule;
use std::sync::atomic::AtomicBool;

/// Global shutdown signal
static _SHUTDOWN_SIGNAL: AtomicBool = AtomicBool::new(false);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create tokio runtime
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    // Run async main
    rt.block_on(async_main())
}

async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    eprintln!("[MAIN] MCP Server Initializing");
    eprintln!("[MAIN] Rust version: {}", env!("CARGO_PKG_VERSION"));

    // ========================================================================
    // Phase 1: Initialize Capsule Architecture
    // ========================================================================

    eprintln!("[MAIN] Phase 1: Initializing capsules");

    // StdioTransportCapsule (T5 Streaming, 4 KB)
    let transport = Box::leak(Box::new(StdioTransportCapsule::new()));
    eprintln!("[MAIN] ✓ StdioTransportCapsule initialized (4 KB)");

    // ToolExecutorCapsule (T1 Atomic, 256 B)
    let executor = Box::leak(Box::new(ToolExecutorCapsule::new()));
    eprintln!("[MAIN] ✓ ToolExecutorCapsule initialized (256 B)");

    // DebuggerCapsule (1 MB, from kdb)
    let pid = std::process::id() as u64;
    let debugger = Box::leak(Box::new(DebuggerCapsule::new(pid)));
    eprintln!("[MAIN] ✓ DebuggerCapsule initialized (1 MB)");

    // McpServerCapsule (T6 Mixed, 256 KB)
    let server = Box::leak(Box::new(McpServerCapsule::new(debugger)));
    eprintln!("[MAIN] ✓ McpServerCapsule initialized (256 KB)");

    // McpRuntimeCapsule (T6 Mixed orchestration, 20.75 KB)
    let mut runtime = McpRuntimeCapsule::new();
    eprintln!("[MAIN] ✓ McpRuntimeCapsule initialized (20.75 KB)");

    // ========================================================================
    // Phase 2: Verify Capsule Sizes
    // ========================================================================

    eprintln!("[MAIN] Phase 2: Verifying capsule architecture");
    use std::mem::size_of;

    let sizes = [
        ("StdioTransportCapsule", size_of::<StdioTransportCapsule>()),
        ("ToolExecutorCapsule", size_of::<ToolExecutorCapsule>()),
        ("McpServerCapsule", size_of::<McpServerCapsule>()),
        ("McpRuntimeCapsule", size_of::<McpRuntimeCapsule>()),
    ];

    let total_capsule_bytes: usize = sizes.iter().map(|(_, sz)| sz).sum();

    for (name, size) in &sizes {
        eprintln!(
            "[MAIN]   {} = {} bytes ({:.2} KB)",
            name,
            size,
            *size as f64 / 1024.0
        );
    }
    eprintln!(
        "[MAIN] Total capsule memory: {} bytes ({:.2} MB)",
        total_capsule_bytes,
        total_capsule_bytes as f64 / 1024.0 / 1024.0
    );

    // ========================================================================
    // Phase 3: Start Runtime Event Loop
    // ========================================================================

    eprintln!("[MAIN] Phase 3: Starting MCP runtime event loop");
    eprintln!("[MAIN] Listening for JSON-RPC requests on stdin...");
    eprintln!("[MAIN] Note: Graceful shutdown requires sending EOF to stdin (Ctrl+D)");

    // Run the event loop (blocks until shutdown)
    match runtime.run(transport, server, executor, debugger).await {
        Ok(()) => {
            eprintln!("[MAIN] Event loop completed normally");
        }
        Err(e) => {
            eprintln!("[MAIN] Event loop error: {}", e);
            return Err(e);
        }
    }

    // ========================================================================
    // Phase 4: Print Final Statistics
    // ========================================================================

    eprintln!("[MAIN] Phase 4: Collecting statistics");
    let runtime_stats = runtime.get_stats();
    let transport_stats = transport.get_stats();
    let executor_stats = executor.get_stats();
    let server_stats = server.get_stats();

    eprintln!(
        "[MAIN] Runtime Statistics:
  State: {:?}
  Total requests: {}
  Total responses: {}
  Total errors: {}
  Success rate: {:.2}%
  Avg request latency: {:.2} μs
  Max request latency: {:.2} μs
  Loop iterations: {}
  Avg iterations/request: {:.2}",
        runtime_stats.state,
        runtime_stats.total_requests,
        runtime_stats.total_responses,
        runtime_stats.total_errors,
        runtime_stats.success_rate(),
        runtime_stats.avg_request_latency_ns as f64 / 1000.0,
        runtime_stats.max_request_latency_ns as f64 / 1000.0,
        runtime_stats.loop_iterations,
        runtime_stats.avg_iterations_per_request(),
    );

    eprintln!(
        "[MAIN] Transport Statistics:
  Lines read: {}
  Lines written: {}
  Read errors: {}
  Write errors: {}
  Total bytes read: {}
  Total bytes written: {}",
        transport_stats.lines_read,
        transport_stats.lines_written,
        transport_stats.read_errors,
        transport_stats.write_errors,
        transport_stats.total_bytes_read,
        transport_stats.total_bytes_written,
    );

    eprintln!(
        "[MAIN] Executor Statistics:
  Total executions: {}
  Total errors: {}
  Avg latency: {:.2} ns
  Max concurrent: {}",
        executor_stats.total_executions,
        executor_stats.total_errors,
        executor_stats.avg_latency_ns as f64,
        executor_stats.max_concurrent,
    );

    eprintln!(
        "[MAIN] Server Statistics:
  Total requests: {}
  Successful: {}
  Failed: {}
  Avg latency: {:.2} μs
  Max latency: {:.2} μs",
        server_stats.total_requests,
        server_stats.successful_requests,
        server_stats.failed_requests,
        server_stats.avg_latency_ns as f64 / 1000.0,
        server_stats.max_latency_ns as f64 / 1000.0,
    );

    eprintln!("[MAIN] MCP Server shutdown complete");
    Ok(())
}
