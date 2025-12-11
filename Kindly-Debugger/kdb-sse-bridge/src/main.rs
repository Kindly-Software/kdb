//! KDB MCP Stub - Schema advertiser for tool discovery
//!
//! This stub exposes tool schemas for MCP clients.
//! For actual debugging, configure SSE transport:
//!
//! ```json
//! {
//!   "mcpServers": {
//!     "kdb": {
//!       "transport": "sse",
//!       "url": "https://mcp.kindly.software/sse",
//!       "headers": { "X-License-Key": "YOUR_KEY" }
//!     }
//!   }
//! }
//! ```

use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let response = handle_request(&line);
        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }
}

fn handle_request(request: &str) -> String {
    // Extract method and id
    let method = extract_field(request, "method");
    let id = extract_field(request, "id");

    match method.as_deref() {
        Some("initialize") => json_result(&id, r#"{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"kdb","version":"1.0.0"}}"#),
        Some("tools/list") => json_result(&id, &tools_list()),
        Some("tools/call") => json_error(&id, -32000, "This is a schema-only stub. Configure SSE transport to use KDB: https://kindly.software"),
        Some("notifications/initialized") => String::new(), // No response for notifications
        _ => json_error(&id, -32601, "Method not found"),
    }
}

fn tools_list() -> String {
    r#"{"tools":[
        {"name":"debugger_attach","description":"Attach to running process via ptrace","inputSchema":{"type":"object","properties":{"pid":{"type":"integer","description":"Process ID to attach to via ptrace","minimum":1,"maximum":2147483647}},"required":["pid"]}},
        {"name":"debugger_set_breakpoint","description":"Set breakpoint at memory address","inputSchema":{"type":"object","properties":{"address":{"type":"string","description":"Memory address for breakpoint (hexadecimal format with 0x prefix)","pattern":"^0x[0-9a-fA-F]+$"}},"required":["address"]}},
        {"name":"debugger_continue","description":"Resume execution after breakpoint hit","inputSchema":{"type":"object","properties":{}}},
        {"name":"debugger_step_forward","description":"Single-step forward one instruction","inputSchema":{"type":"object","properties":{"count":{"type":"integer","default":1,"minimum":1,"maximum":1000,"description":"Number of instructions to step forward"}}}},
        {"name":"debugger_step_backward","description":"Time-travel debugging - step backward one instruction","inputSchema":{"type":"object","properties":{"count":{"type":"integer","default":1,"minimum":1,"maximum":2047,"description":"Number of instructions to step backward (time-travel)"}}}},
        {"name":"debugger_get_stack_trace","description":"SIMD-accelerated stack unwinding (<20μs per 10 frames)","inputSchema":{"type":"object","properties":{"max_depth":{"type":"integer","default":100,"minimum":1,"maximum":1000,"description":"Maximum stack depth to unwind"}}}},
        {"name":"debugger_get_variables","description":"Read process memory at address","inputSchema":{"type":"object","properties":{"address":{"type":"string","description":"Memory address in hexadecimal format (e.g., '0x7fff0000')","pattern":"^0x[0-9a-fA-F]+$"},"length":{"type":"integer","default":64,"minimum":1,"maximum":65536,"description":"Number of bytes to read from address"}},"required":["address"]}},
        {"name":"debugger_find_similar_bugs","description":"Probabilistic LSH similarity search for bugs","inputSchema":{"type":"object","properties":{"threshold":{"type":"number","default":0.8,"minimum":0,"maximum":1,"description":"LSH similarity threshold for bug matching (0.0-1.0)"},"max_results":{"type":"integer","default":10,"minimum":1,"maximum":100,"description":"Maximum number of similar bugs to return"}},"required":["threshold"]}},
        {"name":"debugger_export_trace","description":"Streaming export of execution trace","inputSchema":{"type":"object","properties":{"format":{"type":"string","enum":["json","binary"],"default":"json","description":"Output format for execution trace export"},"snapshot_ids":{"type":"array","items":{"type":"integer","minimum":0,"maximum":2046},"description":"Optional list of specific snapshot IDs to export"}}}},
        {"name":"debugger_quota_status","description":"Atomic quota status with tier/limits/usage (<70ns)","inputSchema":{"type":"object","properties":{}}},
        {"name":"debugger_license_info","description":"Atomic license info with tier/validation/expiry (<10ns cached)","inputSchema":{"type":"object","properties":{}}},
        {"name":"debugger_get_comprehensive_audit","description":"Auditable comprehensive audit metrics with compliance (<10μs)","inputSchema":{"type":"object","properties":{"include_audit_trail":{"type":"boolean","default":true,"description":"Include full audit trail in response"},"include_compliance":{"type":"boolean","default":true,"description":"Include compliance metadata (SOX/SOC2/GDPR frameworks)"},"audit_entry_limit":{"type":"integer","default":100,"minimum":1,"maximum":500,"description":"Maximum number of audit entries to return"}}}},
        {"name":"debugger_allocate_session","description":"Allocate tiered debugging session (<100ns lockfree)","inputSchema":{"type":"object","properties":{"tier_hint":{"type":"string","enum":["Light","Medium","Heavy"],"default":"Light","description":"Session tier hint: Light (64KB), Medium (256KB), or Heavy (1.09MB)"}}}},
        {"name":"debugger_release_session","description":"Release debugging session (<100ns lockfree)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID to release (from allocate_session)"}},"required":["session_id"]}},
        {"name":"debugger_get_session_tier","description":"Get session tier (<10ns)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID to query"}},"required":["session_id"]}},
        {"name":"debugger_upgrade_session","description":"Upgrade session to higher tier (<1μs with data migration)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID to upgrade (Light->Medium->Heavy)"}},"required":["session_id"]}},
        {"name":"debugger_get_pool_stats","description":"Pool statistics snapshot (<50ns)","inputSchema":{"type":"object","properties":{}}},
        {"name":"debugger_enable_memory_replay","description":"Enable COW memory tracking for session (<10ms initialization)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID to enable memory replay"},"config":{"type":"string","enum":["default","minimal","performance","compliance"],"default":"default","description":"Configuration preset"}},"required":["session_id"]}},
        {"name":"debugger_capture_memory_snapshot","description":"Capture memory snapshot (<50ms for typical workload)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID with memory replay enabled"}},"required":["session_id"]}},
        {"name":"debugger_read_memory_at_snapshot","description":"Read memory at historical snapshot (<2ms reconstruction)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID with memory replay enabled"},"snapshot_id":{"type":"integer","minimum":0,"description":"Target snapshot ID to read from"},"address":{"type":"string","pattern":"^0x[0-9a-fA-F]+$","description":"Memory address in hexadecimal format"},"length":{"type":"integer","default":64,"minimum":1,"maximum":65536,"description":"Number of bytes to read"}},"required":["session_id","snapshot_id","address"]}},
        {"name":"debugger_navigate_to_snapshot","description":"Navigate to specific snapshot (<100ns state update)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID with memory replay enabled"},"snapshot_id":{"type":"integer","minimum":0,"description":"Target snapshot ID to navigate to"}},"required":["session_id","snapshot_id"]}},
        {"name":"debugger_get_memory_replay_stats","description":"Memory replay statistics (<50ns)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID with memory replay enabled"}},"required":["session_id"]}},
        {"name":"debugger_verify_memory_integrity","description":"Memory integrity verification (O(n) hash-chain check)","inputSchema":{"type":"object","properties":{"session_id":{"type":"integer","minimum":1,"description":"Session ID with memory replay enabled"}},"required":["session_id"]}},
        {"name":"debugger_get_access_mode","description":"Get current Observer/Operator access mode (<10ns)","inputSchema":{"type":"object","properties":{}}},
        {"name":"debugger_request_operator_challenge","description":"Request Ed25519 challenge for Operator elevation (<1ms)","inputSchema":{"type":"object","properties":{"public_key_hex":{"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-fA-F]{64}$","description":"Hex-encoded Ed25519 public key (64 characters = 32 bytes)"}},"required":["public_key_hex"]}},
        {"name":"debugger_elevate_to_operator","description":"Elevate to Operator mode via signed challenge (<1ms)","inputSchema":{"type":"object","properties":{"public_key_hex":{"type":"string","minLength":64,"maxLength":64,"pattern":"^[0-9a-fA-F]{64}$","description":"Hex-encoded Ed25519 public key"},"signature_hex":{"type":"string","minLength":128,"maxLength":128,"pattern":"^[0-9a-fA-F]{128}$","description":"Hex-encoded Ed25519 signature (128 characters = 64 bytes)"}},"required":["public_key_hex","signature_hex"]}},
        {"name":"debugger_revoke_operator","description":"Revoke Operator mode and return to Observer (<10ns)","inputSchema":{"type":"object","properties":{}}}
    ]}"#.to_string()
}

fn extract_field(json: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\"", field);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let value_start = &after[colon + 1..].trim_start();

    if value_start.starts_with('"') {
        let end = value_start[1..].find('"')?;
        Some(value_start[1..=end].to_string())
    } else {
        let end = value_start.find(|c: char| c == ',' || c == '}' || c.is_whitespace()).unwrap_or(value_start.len());
        Some(value_start[..end].to_string())
    }
}

fn json_result(id: &Option<String>, result: &str) -> String {
    let id_str = id.as_ref().map(|s| {
        if s.chars().all(|c| c.is_ascii_digit()) {
            s.to_string()
        } else {
            format!("\"{}\"", s)
        }
    }).unwrap_or_else(|| "null".to_string());
    format!(r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#, id_str, result)
}

fn json_error(id: &Option<String>, code: i32, message: &str) -> String {
    let id_str = id.as_ref().map(|s| {
        if s.chars().all(|c| c.is_ascii_digit()) {
            s.to_string()
        } else {
            format!("\"{}\"", s)
        }
    }).unwrap_or_else(|| "null".to_string());
    format!(r#"{{"jsonrpc":"2.0","id":{},"error":{{"code":{},"message":"{}"}}}}"#, id_str, code, message)
}
