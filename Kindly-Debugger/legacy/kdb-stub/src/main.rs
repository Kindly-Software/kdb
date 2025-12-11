//! KDB MCP Stub Server - Schema advertiser for Glama inspection
//!
//! This is NOT the real KDB server. It only exposes tool schemas.
//! For actual debugging capabilities, sign up at https://kindly.software

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[allow(dead_code)]
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    #[allow(dead_code)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

fn tool_schemas() -> Value {
    json!({
        "tools": [
            {
                "name": "debugger/attach",
                "description": "Attach to a running process by PID. Enables time-travel debugging with audit-compliant snapshots.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pid": { "type": "integer", "description": "Process ID to attach to" }
                    },
                    "required": ["pid"]
                }
            },
            {
                "name": "debugger/set_breakpoint",
                "description": "Set a breakpoint at a memory address. Uses lockfree atomic hash table (<100ns lookup).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "string", "description": "Memory address in hex (e.g., '0x7f1234567890')" }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "debugger/continue",
                "description": "Resume process execution until next breakpoint or signal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "debugger/step_forward",
                "description": "Execute one instruction and capture state snapshot (~5us step + 6ns snapshot).",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "debugger/step_backward",
                "description": "Time-travel backward to previous state (<10ns lockfree replay). Unique to KDB.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "debugger/get_stack_trace",
                "description": "Get current call stack with SIMD-accelerated unwinding (<20us per 10 frames, 5000x vs GDB).",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "debugger/get_variables",
                "description": "Read memory at specified address and size. Returns hex dump with ASCII sidebar.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "address": { "type": "string", "description": "Memory address in hex (e.g., '0x7ffe12340000')" },
                        "size": { "type": "integer", "description": "Number of bytes to read (default: 64)" }
                    },
                    "required": ["address"]
                }
            },
            {
                "name": "debugger/export_trace",
                "description": "Export Q34-compliant audit trail with cryptographic hash-chain integrity.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "format": {
                            "type": "string",
                            "enum": ["json", "csv"],
                            "description": "Export format (json or csv)"
                        }
                    },
                    "required": ["format"]
                }
            }
        ]
    })
}

fn handle_request(req: &JsonRpcRequest) -> JsonRpcResponse {
    let id = req.id.clone().unwrap_or(Value::Null);

    match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {
                    "name": "kdb-stub",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            })),
            error: None,
        },

        "notifications/initialized" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(json!({})),
            error: None,
        },

        "tools/list" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: Some(tool_schemas()),
            error: None,
        },

        "tools/call" => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32000,
                message: "KDB stub server - tool execution disabled. Please sign up at https://kindly.software to use KDB time-travel debugging.".to_string(),
            }),
        },

        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
            }),
        },
    }
}

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_response = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {}", e)
                    }
                });
                let _ = writeln!(stdout, "{}", err_response);
                let _ = stdout.flush();
                continue;
            }
        };

        let response = handle_request(&req);
        let response_json = serde_json::to_string(&response).unwrap();
        let _ = writeln!(stdout, "{}", response_json);
        let _ = stdout.flush();
    }
}
