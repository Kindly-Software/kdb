//! Test tools/list JSON-RPC response
//!
//! Validates that all 13 tools have proper JSON Schema definitions.

use kdb_mcp::McpServerCapsule;
use kdb::DebuggerCapsule;

fn main() {
    println!("=== Testing MCP tools/list Response ===\n");

    // Create debugger (1 MB)
    let debugger = Box::leak(Box::new(DebuggerCapsule::new(12345)));

    // Create MCP server (256 KB)
    let server = Box::leak(Box::new(McpServerCapsule::new(debugger)));

    // Set license (valid until 2030)
    server.license.set_license("test-license-key", 1893456000);

    // Send initialize request (required before tools/list)
    let init_request = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}"#;
    match server.handle_request(init_request, None, None, debugger) {
        Ok(response) => println!("Initialize response:\n{}\n", response),
        Err(e) => {
            eprintln!("Initialize failed: {}", e);
            return;
        }
    }

    // Send initialized notification (using method="notifications/initialized" with id=99)
    // The server handles this as a special case
    let initialized_notification = r#"{"jsonrpc":"2.0","id":99,"method":"notifications/initialized","params":{}}"#;
    match server.handle_request(initialized_notification, None, None, debugger) {
        Ok(response) => println!("Initialized notification response:\n{}\n", response),
        Err(e) => {
            eprintln!("Notification failed: {}", e);
            // Continue anyway - some servers don't require this
        }
    }

    // Request tools/list
    let tools_list_request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    println!("Requesting tools/list...\n");

    match server.handle_request(tools_list_request, None, None, debugger) {
        Ok(response) => {
            println!("=== tools/list Response ===\n");

            // Pretty-print JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                let pretty = serde_json::to_string_pretty(&json).unwrap();
                println!("{}\n", pretty);

                // Validate schema presence
                if let Some(tools) = json["result"]["tools"].as_array() {
                    println!("=== Schema Validation ===");
                    println!("Total tools: {}\n", tools.len());

                    for (i, tool) in tools.iter().enumerate() {
                        let name = tool["name"].as_str().unwrap_or("unknown");
                        let has_schema = tool["inputSchema"]["properties"].is_object()
                            || tool["inputSchema"]["properties"] == serde_json::json!({});
                        let prop_count = if let Some(props) = tool["inputSchema"]["properties"].as_object() {
                            props.len()
                        } else {
                            0
                        };

                        let status = if has_schema { "✅" } else { "❌" };
                        println!("  {:<2}. {} {:<30} ({} properties)",
                            i + 1, status, name, prop_count);
                    }

                    // Summary
                    let with_schemas = tools.iter().filter(|t| {
                        t["inputSchema"]["properties"].is_object()
                    }).count();

                    println!("\n=== Summary ===");
                    println!("Tools with schemas: {}/{}", with_schemas, tools.len());

                    if with_schemas == 13 {
                        println!("✅ SUCCESS: All 13 tools have proper input schemas!");
                    } else {
                        println!("❌ FAIL: {} tools missing schemas", 13 - with_schemas);
                    }
                } else {
                    println!("❌ ERROR: No tools array in response");
                }
            } else {
                println!("Raw response:\n{}", response);
            }
        }
        Err(e) => {
            eprintln!("❌ tools/list failed: {}", e);
        }
    }
}
