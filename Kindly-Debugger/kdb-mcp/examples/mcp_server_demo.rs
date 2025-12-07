//! MCP Server Demo
//!
//! Demonstrates the kdb_mcp with all 9 debugging tools.

use kdb_mcp::{McpServerCapsule};
use kdb::DebuggerCapsule;

fn main() {
    println!("=== Atomic MCP Server Demo ===\n");

    // Create debugger (1 MB, normally heap-allocated or mmap'd)
    let debugger = Box::leak(Box::new(DebuggerCapsule::new(12345)));

    // Create MCP server (256 KB)
    let server = Box::leak(Box::new(McpServerCapsule::new(debugger)));

    // Set license (valid until 2030)
    server.license.set_license("demo-license-key-12345", 1893456000);

    println!("Server initialized:");
    println!("  License: Valid");
    println!("  Rate limit: 1000 req/sec");
    println!("  Quota: 10K daily, 100K monthly");
    println!("  Tools registered: 9\n");

    // Demo: Tool 1 - Attach to process
    let request1 = r#"{"jsonrpc":"2.0","id":1,"method":"debugger/attach","params":{"pid":12345}}"#;
    println!("Request 1: {}", request1);
    match server.handle_request(request1, None, None, debugger) {
        Ok(response) => println!("Response 1: {}\n", response),
        Err(e) => println!("Error 1: {}\n", e),
    }

    // Demo: Tool 2 - Set breakpoint
    let request2 = r#"{"jsonrpc":"2.0","id":2,"method":"debugger/set_breakpoint","params":{"address":"0x1000"}}"#;
    println!("Request 2: {}", request2);
    match server.handle_request(request2, None, None, debugger) {
        Ok(response) => println!("Response 2: {}\n", response),
        Err(e) => println!("Error 2: {}\n", e),
    }

    // Demo: Tool 3 - Continue execution
    let request3 = r#"{"jsonrpc":"2.0","id":3,"method":"debugger/continue","params":{}}"#;
    println!("Request 3: {}", request3);
    match server.handle_request(request3, None, None, debugger) {
        Ok(response) => println!("Response 3: {}\n", response),
        Err(e) => println!("Error 3: {}\n", e),
    }

    // Demo: Tool 4 - Step forward
    let request4 = r#"{"jsonrpc":"2.0","id":4,"method":"debugger/step_forward","params":{}}"#;
    println!("Request 4: {}", request4);
    match server.handle_request(request4, None, None, debugger) {
        Ok(response) => println!("Response 4: {}\n", response),
        Err(e) => println!("Error 4: {}\n", e),
    }

    // Demo: Tool 5 - Step backward (time-travel!)
    let request5 = r#"{"jsonrpc":"2.0","id":5,"method":"debugger/step_backward","params":{}}"#;
    println!("Request 5: {}", request5);
    match server.handle_request(request5, None, None, debugger) {
        Ok(response) => println!("Response 5: {}\n", response),
        Err(e) => println!("Error 5: {}\n", e),
    }

    // Demo: Tool 6 - Get stack trace
    let request6 = r#"{"jsonrpc":"2.0","id":6,"method":"debugger/get_stack_trace","params":{}}"#;
    println!("Request 6: {}", request6);
    match server.handle_request(request6, None, None, debugger) {
        Ok(response) => println!("Response 6: {}\n", response),
        Err(e) => println!("Error 6: {}\n", e),
    }

    // Demo: Tool 7 - Get variables
    let request7 = r#"{"jsonrpc":"2.0","id":7,"method":"debugger/get_variables","params":{"address":"0x7fff0000"}}"#;
    println!("Request 7: {}", request7);
    match server.handle_request(request7, None, None, debugger) {
        Ok(response) => println!("Response 7: {}\n", response),
        Err(e) => println!("Error 7: {}\n", e),
    }

    // Demo: Tool 8 - Find similar bugs
    let request8 = r#"{"jsonrpc":"2.0","id":8,"method":"debugger/find_similar_bugs","params":{"threshold":0.8}}"#;
    println!("Request 8: {}", request8);
    match server.handle_request(request8, None, None, debugger) {
        Ok(response) => println!("Response 8: {}\n", response),
        Err(e) => println!("Error 8: {}\n", e),
    }

    // Demo: Tool 9 - Export trace
    let request9 = r#"{"jsonrpc":"2.0","id":9,"method":"debugger/export_trace","params":{}}"#;
    println!("Request 9: {}", request9);
    match server.handle_request(request9, None, None, debugger) {
        Ok(response) => println!("Response 9: {}\n", response),
        Err(e) => println!("Error 9: {}\n", e),
    }

    // Print final statistics
    let stats = server.get_stats();
    let json_stats = server.json_rpc.get_stats();
    let rate_stats = server.rate_limiter.get_stats();
    let quota_stats = server.quota.get_stats();
    let registry_stats = server.tools.get_stats();

    println!("\n=== Final Statistics ===");
    println!("Server:");
    println!("  Total requests: {}", stats.total_requests);
    println!("  Successful: {}", stats.successful_requests);
    println!("  Failed: {}", stats.failed_requests);
    println!("  Avg latency: {}ns", stats.avg_latency_ns);
    println!("  Max latency: {}ns", stats.max_latency_ns);

    println!("\nJSON-RPC:");
    println!("  Requests parsed: {}", json_stats.requests_parsed);
    println!("  Parse errors: {}", json_stats.parse_errors);
    println!("  Avg parse latency: {}ns", json_stats.avg_latency_ns);

    println!("\nRate Limiter:");
    println!("  Allowed: {}", rate_stats.requests_allowed);
    println!("  Denied: {}", rate_stats.requests_denied);

    println!("\nQuota:");
    println!("  Total: {}", quota_stats.total_requests);
    println!("  Daily: {}", quota_stats.daily_requests);
    println!("  Quota exceeded: {}", quota_stats.quota_exceeded);

    println!("\nTool Registry:");
    println!("  Tools registered: {}", registry_stats.tool_count);
    println!("  Lookups: {}", registry_stats.lookup_count);
    println!("  Hits: {}", registry_stats.lookup_hits);
    println!("  Misses: {}", registry_stats.lookup_misses);
}
