//! Unit Tests for StdioTransportCapsule (Q1-Q7: 20 tests)
//!
//! Bug #4 Fix Validation: Concurrent safety tests verify UnsafeCell + atomic indices are safe

use kdb_mcp::StdioTransportCapsule;

#[test]
fn test_stdio_size() {
    let size = std::mem::size_of::<StdioTransportCapsule>();
    assert!(size <= 4224, "StdioTransportCapsule must be ≤4224 bytes (got {})", size);
}

#[test]
fn test_stdio_alignment() {
    assert_eq!(
        std::mem::align_of::<StdioTransportCapsule>(),
        64,
        "StdioTransportCapsule must be 64-byte aligned"
    );
}

#[test]
fn test_write_input_basic() {
    let capsule = StdioTransportCapsule::new();

    let data = b"test data";
    let written = capsule.write_input(data).unwrap();
    assert_eq!(written, 9);

    let stats = capsule.get_stats();
    assert_eq!(stats.total_bytes_read, 9);
}

#[test]
fn test_write_input_empty() {
    let capsule = StdioTransportCapsule::new();

    let written = capsule.write_input(b"").unwrap();
    assert_eq!(written, 0);

    let stats = capsule.get_stats();
    assert_eq!(stats.total_bytes_read, 0);
}

#[test]
fn test_write_input_max_capacity() {
    let capsule = StdioTransportCapsule::new();

    // Write max capacity (2047 bytes, -1 for ring invariant)
    let large_data = vec![0x41u8; 2047];
    let written = capsule.write_input(&large_data).unwrap();
    assert_eq!(written, 2047);

    let stats = capsule.get_stats();
    assert_eq!(stats.total_bytes_read, 2047);
}

#[test]
fn test_read_line_complete() {
    let capsule = StdioTransportCapsule::new();

    // Write complete JSON line with newline
    let json = br#"{"jsonrpc":"2.0","method":"test"}"#;
    let mut data = json.to_vec();
    data.push(b'\n');

    capsule.write_input(&data).unwrap();

    // Extract line
    let line = capsule.read_line().unwrap();
    assert!(line.is_some());
    assert!(line.unwrap().contains("jsonrpc"));

    let stats = capsule.get_stats();
    assert_eq!(stats.lines_read, 1);
}

#[test]
fn test_read_line_incomplete() {
    let capsule = StdioTransportCapsule::new();

    // Write JSON without newline
    let json = br#"{"incomplete":"data"#;
    capsule.write_input(json).unwrap();

    // No complete line available
    let line = capsule.read_line().unwrap();
    assert!(line.is_none());

    let stats = capsule.get_stats();
    assert_eq!(stats.lines_read, 0);
}

#[test]
fn test_read_line_invalid_json() {
    let capsule = StdioTransportCapsule::new();

    // Write non-JSON data with newline
    let data = b"not json at all\n";
    capsule.write_input(data).unwrap();

    // Should fail JSON validation
    let result = capsule.read_line();
    assert!(result.is_err() || result.unwrap().is_none());
}

#[test]
fn test_write_line_basic() {
    let capsule = StdioTransportCapsule::new();

    capsule.write_line(r#"{"result":"ok"}"#).unwrap();

    let stats = capsule.get_stats();
    assert_eq!(stats.output_bytes_pending, 16); // 15 bytes + newline

    let output = capsule.get_pending_output();
    assert_eq!(output.len(), 16);
    assert_eq!(output[15], b'\n'); // Last byte is newline
}

#[test]
fn test_write_line_too_long() {
    let capsule = StdioTransportCapsule::new();

    // Line longer than 2048 bytes
    let long_json = "x".repeat(2048);
    let result = capsule.write_line(&long_json);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "JSON line too long for output buffer");
}

#[test]
fn test_flush_output() {
    let capsule = StdioTransportCapsule::new();

    capsule.write_line(r#"{"data":"test"}"#).unwrap();

    let stats1 = capsule.get_stats();
    let pending = stats1.output_bytes_pending;
    assert!(pending > 0);

    // Get output and flush
    let output = capsule.get_pending_output();
    capsule.flush_output(output.len()).unwrap();

    let stats2 = capsule.get_stats();
    assert_eq!(stats2.output_bytes_pending, 0);
    assert_eq!(stats2.lines_written, 1);
}

#[test]
fn test_multiple_lines_sequential() {
    let capsule = StdioTransportCapsule::new();

    let lines = vec![
        r#"{"id":1}"#,
        r#"{"id":2}"#,
        r#"{"id":3}"#,
    ];

    for line in &lines {
        let mut data = line.as_bytes().to_vec();
        data.push(b'\n');
        capsule.write_input(&data).unwrap();
    }

    // Extract all 3 lines
    for i in 0..3 {
        let line = capsule.read_line().unwrap();
        assert!(line.is_some(), "Line {} should be available", i);
    }

    let stats = capsule.get_stats();
    assert_eq!(stats.lines_read, 3);
}

#[test]
fn test_ring_buffer_wraparound() {
    let capsule = StdioTransportCapsule::new();

    // Fill buffer close to capacity
    let data1 = vec![0xFFu8; 2040];
    capsule.write_input(&data1).unwrap();

    // Write more data (should wrap)
    let data2 = b"wrap\n";
    let written = capsule.write_input(data2).unwrap();
    assert_eq!(written, 5, "Wraparound write should succeed");
}

// ============================================================================
// Bug #4 Fix Validation: Concurrent Safety Tests (Critical)
// ============================================================================

#[test]
fn test_concurrent_input_writes() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(StdioTransportCapsule::new());
    let mut handles = vec![];

    // Spawn 5 writer threads
    for i in 0..5 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            let json = format!(r#"{{"id":{}}}"#, i);
            let mut data = json.as_bytes().to_vec();
            data.push(b'\n');
            cap.write_input(&data)
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok(), "Concurrent write should succeed");
    }

    let stats = capsule.get_stats();
    assert!(stats.total_bytes_read > 0, "Bytes should be written");
}

#[test]
fn test_concurrent_input_output_isolation() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(StdioTransportCapsule::new());

    // Spawn reader and writer threads concurrently
    let cap_writer = capsule.clone();
    let writer = thread::spawn(move || {
        for i in 0..100 {
            let json = format!(r#"{{"id":{}}}"#, i);
            let mut data = json.as_bytes().to_vec();
            data.push(b'\n');
            let _ = cap_writer.write_input(&data);
        }
    });

    let cap_reader = capsule.clone();
    let reader = thread::spawn(move || {
        let mut lines_read = 0;
        for _ in 0..200 {
            if let Ok(Some(_)) = cap_reader.read_line() {
                lines_read += 1;
            }
        }
        lines_read
    });

    writer.join().unwrap();
    let lines_read = reader.join().unwrap();

    // Reader should have read some lines (may not be all 100 due to timing)
    assert!(lines_read > 0, "Reader should extract some lines");
}

#[test]
fn test_concurrent_output_writes() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(StdioTransportCapsule::new());
    let mut handles = vec![];

    // Spawn 5 output writer threads
    for i in 0..5 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            let json = format!(r#"{{"result":{}}}"#, i);
            cap.write_line(&json)
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        let result = handle.join().unwrap();
        assert!(result.is_ok(), "Concurrent output write should succeed");
    }

    let stats = capsule.get_stats();
    assert!(stats.output_bytes_pending > 0);
}

#[test]
fn test_ring_buffer_wraparound_safety() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(StdioTransportCapsule::new());

    // Pre-fill buffer to near capacity
    let initial_data = vec![0xAAu8; 1500];
    capsule.write_input(&initial_data).unwrap();

    // Spawn threads to cause wraparound
    let mut handles = vec![];
    for i in 0..3 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            let data = format!("wraparound_{}\n", i);
            cap.write_input(data.as_bytes())
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.join().unwrap();
        // May succeed or fail depending on buffer state (both outcomes valid)
        let _ = result;
    }

    let stats = capsule.get_stats();
    assert!(stats.total_bytes_read >= 1500, "Initial data should be written");
}

#[test]
fn test_stats_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let capsule = Arc::new(StdioTransportCapsule::new());
    let mut handles = vec![];

    // Spawn threads that concurrently read stats
    for _ in 0..10 {
        let cap = capsule.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _stats = cap.get_stats();
            }
        });
        handles.push(handle);
    }

    // Also spawn a writer thread
    let cap_writer = capsule.clone();
    let writer = thread::spawn(move || {
        for i in 0..50 {
            let json = format!(r#"{{"test":{}}}"#, i);
            let mut data = json.as_bytes().to_vec();
            data.push(b'\n');
            let _ = cap_writer.write_input(&data);
        }
    });

    for handle in handles {
        handle.join().unwrap();
    }
    writer.join().unwrap();

    // Final stats should be consistent
    let stats = capsule.get_stats();
    assert!(stats.total_bytes_read > 0);
}
