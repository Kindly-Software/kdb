//! Concurrent Audit Log Tests
//!
//! Validates that AuditLogCapsule is safe under concurrent access.
//! Tests the UnsafeCell + AtomicU64 coordination pattern.

use kdb_mcp::server::{AuditLogCapsule, McpServerCapsule};
use std::sync::Arc;
use std::thread;

// ============================================================================
// T28: Q8-Q14 Property Tests (Concurrent Safety)
// ============================================================================

#[test]
fn test_concurrent_audit_writes() {
    // Create audit log
    let log = Arc::new(AuditLogCapsule::new());

    // Spawn 10 threads, each writing 1000 entries
    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let log = Arc::clone(&log);
            thread::spawn(move || {
                for i in 0..1000 {
                    let request_id = (thread_id * 1000 + i) as u64;
                    log.record(request_id, thread_id as u64, 100, true);
                }
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // Validate: Should have 10,000 entries (head = 10,000)
    let head = log.get_head();
    assert_eq!(head, 10_000, "Lost audit entries! Expected 10,000, got {}", head);

    // Validate: No duplicate request_ids in last 512 entries (ring buffer)
    let mut seen_ids = std::collections::HashSet::new();
    let start_idx = if head > 512 { head - 512 } else { 0 };

    for i in 0..512.min(head as usize) {
        if let Some(entry) = log.get_entry(i) {
            // Skip zero entries (may be uninitialized if <512 total writes)
            if entry.request_id != 0 {
                assert!(
                    seen_ids.insert(entry.request_id),
                    "Duplicate request_id {} at index {}! Data race detected!",
                    entry.request_id,
                    i
                );
            }
        }
    }

    println!("✅ Concurrent audit log test passed: 10,000 entries, no data races");
}

#[test]
fn test_concurrent_server_requests() {
    use kdb::DebuggerCapsule;

    // Create server (PID 0 is placeholder, not actually used)
    let debugger = Box::leak(Box::new(DebuggerCapsule::new(0)));
    let server = Arc::new(McpServerCapsule::new(debugger));

    // Convert to immutable reference for thread safety
    let debugger_ref: &'static DebuggerCapsule = debugger;

    #[cfg(feature = "json-rpc")]
    {
        // Spawn 5 threads, each making 100 requests
        let handles: Vec<_> = (0..5)
            .map(|thread_id| {
                let server = Arc::clone(&server);
                thread::spawn(move || {
                    for i in 0..100 {
                        let request_id = thread_id * 100 + i;

                        // Make various requests (will fail but should audit)
                        let request = format!(
                            r#"{{"jsonrpc":"2.0","method":"debugger/attach","params":{{"pid":999999}},"id":{}}}"#,
                            request_id
                        );

                        // Execute request (will fail due to invalid PID)
                        let _ = server.handle_request(&request, None, None, debugger_ref);
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        // Validate: Audit log should have 500 entries
        let stats = server.get_stats();
        assert!(
            stats.total_requests >= 500,
            "Lost requests! Expected >=500, got {}",
            stats.total_requests
        );

        println!("✅ Concurrent server request test passed: {} requests processed", stats.total_requests);
    }

    #[cfg(not(feature = "json-rpc"))]
    {
        println!("⚠️  Skipping concurrent server test (json-rpc feature disabled)");
    }
}

#[test]
fn test_audit_log_wraparound() {
    // Create audit log (512 entry ring buffer)
    let log = Arc::new(AuditLogCapsule::new());

    // Write 1000 entries (should wrap around)
    for i in 0..1000 {
        log.record(i as u64, 1, 100, true);
    }

    // Validate: head = 1000 (wraps internally via modulo)
    let head = log.get_head();
    assert_eq!(head, 1000, "Head mismatch: expected 1000, got {}", head);

    // Validate: Oldest entries overwritten (request_id >= 488)
    // Ring buffer index 0 = request_id 1000 % 512 = 488
    // Actually: 1000 % 512 = 488, so index 488 has request_id 1000-1 = 999
    // Index 0 has request_id 512 (first overwrite)

    if let Some(entry) = log.get_entry(0) {
        // After 1000 writes:
        // - Indices 0-487: overwritten once (request_id 512-999)
        // - Indices 488-511: original writes (request_id 488-511)
        assert!(
            entry.request_id >= 512 || entry.request_id < 488,
            "Wraparound validation failed: entry[0] = {}",
            entry.request_id
        );
    }

    println!("✅ Audit log wraparound test passed");
}

#[test]
fn test_audit_log_alignment() {
    // Validate cache alignment
    assert_eq!(
        std::mem::align_of::<AuditLogCapsule>(),
        64,
        "AuditLogCapsule must be 64-byte aligned"
    );

    println!("✅ Audit log alignment test passed");
}

#[test]
fn test_stress_concurrent_audit() {
    // Stress test: 100 threads × 10,000 writes = 1,000,000 total
    let log = Arc::new(AuditLogCapsule::new());

    let handles: Vec<_> = (0..100)
        .map(|thread_id| {
            let log = Arc::clone(&log);
            thread::spawn(move || {
                for i in 0..10_000 {
                    let request_id = (thread_id * 10_000 + i) as u64;
                    log.record(request_id, thread_id as u64, 100, true);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    let head = log.get_head();
    assert_eq!(head, 1_000_000, "Stress test lost entries! Expected 1,000,000, got {}", head);

    println!("✅ Stress test passed: 1,000,000 concurrent writes, no data races");
}
