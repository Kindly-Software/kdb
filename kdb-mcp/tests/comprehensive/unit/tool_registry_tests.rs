//! Unit Tests for McpToolRegistryCapsule (Q1-Q7: 20 tests)
//!
//! Bug #2 Fix Validation: Tests verify proper bounds checking for tool names

use kdb_mcp::McpToolRegistryCapsule;

#[test]
fn test_registry_size() {
    assert_eq!(
        std::mem::size_of::<McpToolRegistryCapsule>(),
        16384,
        "McpToolRegistryCapsule must be 16 KB"
    );
}

#[test]
fn test_registry_alignment() {
    assert_eq!(
        std::mem::align_of::<McpToolRegistryCapsule>(),
        64,
        "McpToolRegistryCapsule must be 64-byte aligned"
    );
}

#[test]
fn test_register_single_tool() {
    let registry = McpToolRegistryCapsule::new();

    let tool_id = registry.register_tool("debugger/attach", 1).unwrap();
    assert_eq!(tool_id, 1, "First tool ID should be 1");

    let stats = registry.get_stats();
    assert_eq!(stats.tool_count, 1);
}

#[test]
fn test_register_multiple_tools() {
    let registry = McpToolRegistryCapsule::new();

    let id1 = registry.register_tool("debugger/attach", 1).unwrap();
    let id2 = registry.register_tool("debugger/detach", 2).unwrap();
    let id3 = registry.register_tool("debugger/step", 3).unwrap();

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);

    let stats = registry.get_stats();
    assert_eq!(stats.tool_count, 3);
}

#[test]
fn test_lookup_existing_tool() {
    let registry = McpToolRegistryCapsule::new();

    registry.register_tool("debugger/attach", 42).unwrap();

    let handle = registry.lookup("debugger/attach").unwrap();
    assert_eq!(handle.tool_id, 1);
    assert_eq!(handle.handler_id, 42);

    let stats = registry.get_stats();
    assert_eq!(stats.lookup_hits, 1);
    assert_eq!(stats.lookup_misses, 0);
}

#[test]
fn test_lookup_missing_tool() {
    let registry = McpToolRegistryCapsule::new();

    registry.register_tool("debugger/attach", 1).unwrap();

    let result = registry.lookup("nonexistent/tool");
    assert!(result.is_none());

    let stats = registry.get_stats();
    assert_eq!(stats.lookup_hits, 0);
    assert_eq!(stats.lookup_misses, 1);
}

#[test]
fn test_lookup_empty_registry() {
    let registry = McpToolRegistryCapsule::new();

    let result = registry.lookup("any/tool");
    assert!(result.is_none());

    let stats = registry.get_stats();
    assert_eq!(stats.lookup_misses, 1);
}

#[test]
fn test_registry_full() {
    let registry = McpToolRegistryCapsule::new();

    // Register 64 tools (MAX_TOOLS)
    for i in 0..64 {
        let name = format!("tool{}", i);
        let result = registry.register_tool(&name, i as u64);
        assert!(result.is_ok(), "Tool {} registration failed", i);
    }

    // 65th should fail
    let result = registry.register_tool("tool65", 65);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Registry full");
}

// ============================================================================
// Bug #2 Fix Validation: Bounds Checking Tests (Critical)
// ============================================================================

#[test]
fn test_tool_name_empty() {
    let registry = McpToolRegistryCapsule::new();

    let result = registry.register_tool("", 1);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Tool name cannot be empty");
}

#[test]
fn test_tool_name_exact_boundary() {
    let registry = McpToolRegistryCapsule::new();

    // 63 characters exactly (max allowed: 63 chars + 1 null terminator = 64 bytes)
    let name_63 = "a".repeat(63);
    let result = registry.register_tool(&name_63, 1);
    assert!(result.is_ok(), "63-char tool name should succeed");
}

#[test]
fn test_tool_name_too_long() {
    let registry = McpToolRegistryCapsule::new();

    // 64 characters (too long: needs 1 byte for null terminator)
    let name_64 = "a".repeat(64);
    let result = registry.register_tool(&name_64, 1);
    assert!(result.is_err(), "64-char tool name should fail (no space for null terminator)");
    assert_eq!(result.unwrap_err(), "Tool name too long (max 63 chars)");
}

#[test]
fn test_tool_name_way_too_long() {
    let registry = McpToolRegistryCapsule::new();

    // 1000 characters (way too long)
    let name_1000 = "x".repeat(1000);
    let result = registry.register_tool(&name_1000, 1);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Tool name too long (max 63 chars)");
}

#[test]
fn test_tool_name_boundary_lookup() {
    let registry = McpToolRegistryCapsule::new();

    // Register 63-char name
    let name_63 = "b".repeat(63);
    let tool_id = registry.register_tool(&name_63, 99).unwrap();

    // Lookup should work
    let handle = registry.lookup(&name_63).unwrap();
    assert_eq!(handle.tool_id, tool_id);
    assert_eq!(handle.handler_id, 99);
}

#[test]
fn test_tool_name_null_terminator() {
    let registry = McpToolRegistryCapsule::new();

    // Register tool with 10-char name
    let name = "short_tool";
    registry.register_tool(name, 1).unwrap();

    // Lookup with exact name should succeed
    let handle = registry.lookup(name).unwrap();
    assert_eq!(handle.tool_id, 1);

    // Lookup with prefix should fail (null terminator check)
    let prefix = "short";
    let result = registry.lookup(prefix);
    assert!(result.is_none(), "Prefix lookup should fail due to null terminator check");
}

#[test]
fn test_tool_name_with_special_chars() {
    let registry = McpToolRegistryCapsule::new();

    let names = vec![
        "debugger/attach",
        "file:read",
        "command::execute",
        "tool-with-dashes",
        "tool_with_underscores",
        "tool.with.dots",
    ];

    for (i, name) in names.iter().enumerate() {
        let result = registry.register_tool(name, i as u64);
        assert!(result.is_ok(), "Tool name '{}' should be valid", name);
    }
}

#[test]
fn test_lookup_case_sensitive() {
    let registry = McpToolRegistryCapsule::new();

    registry.register_tool("ToolName", 1).unwrap();

    // Exact case should work
    assert!(registry.lookup("ToolName").is_some());

    // Different case should fail (case-sensitive)
    assert!(registry.lookup("toolname").is_none());
    assert!(registry.lookup("TOOLNAME").is_none());
}

#[test]
fn test_tool_handler_record_call() {
    let registry = McpToolRegistryCapsule::new();

    registry.register_tool("test_tool", 1).unwrap();
    let handle = registry.lookup("test_tool").unwrap();

    // Record some calls
    handle.record_call(1000); // 1μs
    handle.record_call(2000); // 2μs
    handle.record_call(1500); // 1.5μs

    // Note: We can't easily verify call_count/total_latency without exposing internals
    // This test validates the API doesn't panic
}

#[test]
fn test_registry_stats_accumulation() {
    let registry = McpToolRegistryCapsule::new();

    // Register 3 tools
    registry.register_tool("tool1", 1).unwrap();
    registry.register_tool("tool2", 2).unwrap();
    registry.register_tool("tool3", 3).unwrap();

    // Perform lookups (mix of hits and misses)
    registry.lookup("tool1"); // hit
    registry.lookup("tool2"); // hit
    registry.lookup("missing1"); // miss
    registry.lookup("tool3"); // hit
    registry.lookup("missing2"); // miss
    registry.lookup("missing3"); // miss

    let stats = registry.get_stats();
    assert_eq!(stats.tool_count, 3);
    assert_eq!(stats.lookup_count, 6);
    assert_eq!(stats.lookup_hits, 3);
    assert_eq!(stats.lookup_misses, 3);
}

#[test]
fn test_concurrent_registration_same_name() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(McpToolRegistryCapsule::new());
    let mut handles = vec![];

    // Try to register same tool name from 5 threads concurrently
    for i in 0..5 {
        let reg = registry.clone();
        let handle = thread::spawn(move || {
            reg.register_tool("concurrent_tool", i as u64)
        });
        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Exactly one should succeed
    let successes = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(successes, 1, "Only one thread should successfully register the tool");

    let stats = registry.get_stats();
    assert_eq!(stats.tool_count, 1);
}
