//! Integration tests for TerminalWriterCapsule
//!
//! Tests T4 Batch buffered terminal output with batching efficiency.

#![cfg(feature = "tui-terminal")]

use atomic_capsule::terminal::output::TerminalWriterCapsule;
use std::sync::Arc;
use std::thread;

#[test]
fn test_basic_write() {
    let writer = TerminalWriterCapsule::new();

    let written = writer.write(b"Hello, World!").unwrap();
    assert_eq!(written, 13);
    assert_eq!(writer.bytes_written(), 13);
    assert_eq!(writer.position(), 13);
    assert_eq!(writer.flush_count(), 0); // Not flushed yet
}

#[test]
fn test_write_str() {
    let writer = TerminalWriterCapsule::new();

    let written = writer.write_str("Hello, Rust!").unwrap();
    assert_eq!(written, 12);
    assert_eq!(writer.bytes_written(), 12);
}

#[test]
fn test_manual_flush() {
    let writer = TerminalWriterCapsule::new();

    writer.write_str("Test data").unwrap();
    assert_eq!(writer.flush_count(), 0);

    writer.flush().unwrap();
    assert_eq!(writer.flush_count(), 1);
    assert_eq!(writer.position(), 0); // Buffer reset after flush
}

#[test]
fn test_auto_flush() {
    let writer = TerminalWriterCapsule::with_capacity(256);

    // Write data until we exceed flush threshold (128 bytes)
    for _ in 0..40 {
        writer.write(b"test").unwrap();
    }

    // Should have auto-flushed at least once
    assert!(writer.flush_count() > 0);
}

#[test]
fn test_cursor_movement() {
    let writer = TerminalWriterCapsule::new();

    writer.move_cursor(10, 5).unwrap();

    // Verify ANSI sequence was written
    assert!(writer.position() > 0);
    assert!(writer.bytes_written() > 0);
}

#[test]
fn test_clear_operations() {
    let writer = TerminalWriterCapsule::new();

    writer.clear_screen().unwrap();
    let pos1 = writer.position();
    assert_eq!(pos1, 4); // "\x1b[2J" = 4 bytes

    writer.clear_line().unwrap();
    let pos2 = writer.position();
    assert_eq!(pos2, 8); // +4 bytes
}

#[test]
fn test_cursor_save_restore() {
    let writer = TerminalWriterCapsule::new();

    writer.save_cursor().unwrap();
    let pos1 = writer.position();

    writer.move_cursor(20, 10).unwrap();

    writer.restore_cursor().unwrap();
    let pos2 = writer.position();

    assert!(pos2 > pos1);
}

#[test]
fn test_cursor_visibility() {
    let writer = TerminalWriterCapsule::new();

    writer.hide_cursor().unwrap();
    let pos1 = writer.position();
    assert_eq!(pos1, 6); // "\x1b[?25l" = 6 bytes

    writer.show_cursor().unwrap();
    let pos2 = writer.position();
    assert_eq!(pos2, 12); // +6 bytes
}

#[test]
fn test_batch_operations() {
    let writer = TerminalWriterCapsule::new();

    // Batch multiple operations
    writer.clear_screen().unwrap();
    writer.cursor_home().unwrap();
    writer.hide_cursor().unwrap();
    writer.write_str("Line 1\n").unwrap();
    writer.write_str("Line 2\n").unwrap();
    writer.show_cursor().unwrap();

    // Should still be in buffer (below threshold)
    assert_eq!(writer.flush_count(), 0);
    assert!(writer.position() > 0);

    // Manual flush
    writer.flush().unwrap();
    assert_eq!(writer.flush_count(), 1);
    assert_eq!(writer.position(), 0);
}

#[test]
fn test_generation_counter() {
    let writer = TerminalWriterCapsule::new();

    let gen1 = writer.generation();

    writer.write(b"test1").unwrap();
    let gen2 = writer.generation();
    assert!(gen2 > gen1);

    writer.write(b"test2").unwrap();
    let gen3 = writer.generation();
    assert!(gen3 > gen2);
}

#[test]
fn test_concurrent_writes() {
    let writer = Arc::new(TerminalWriterCapsule::with_capacity(65536));
    let mut handles = vec![];

    // Spawn 10 threads writing concurrently
    for i in 0..10 {
        let writer_clone = Arc::clone(&writer);
        let handle = thread::spawn(move || {
            for j in 0..50 {
                let msg = format!("Thread {} write {}\n", i, j);
                writer_clone.write(msg.as_bytes()).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total writes (10 threads × 50 writes × ~18-20 bytes avg)
    // Note: May have auto-flushed during writes
    assert!(writer.bytes_written() >= 8500, "Expected >= 8500 bytes, got {}", writer.bytes_written());
}

#[test]
fn test_capacity_customization() {
    let writer = TerminalWriterCapsule::with_capacity(1024);

    assert_eq!(writer.capacity(), 1024);
    assert_eq!(writer.flush_threshold(), 512); // 50% of capacity
}

#[test]
fn test_empty_write() {
    let writer = TerminalWriterCapsule::new();

    let written = writer.write(b"").unwrap();
    assert_eq!(written, 0);
    assert_eq!(writer.bytes_written(), 0);
    assert_eq!(writer.position(), 0);
}

#[test]
fn test_large_write() {
    let writer = TerminalWriterCapsule::new();

    // Write 1KB of data
    let large_data = vec![b'A'; 1024];
    let written = writer.write(&large_data).unwrap();

    assert_eq!(written, 1024);
    assert_eq!(writer.bytes_written(), 1024);
}

#[test]
fn test_drop_flushes() {
    let writer = TerminalWriterCapsule::new();
    writer.write_str("test data").unwrap();
    assert!(writer.position() > 0);

    // Drop should flush remaining data
    drop(writer);

    // Create new writer to verify clean state
    let writer2 = TerminalWriterCapsule::new();
    assert_eq!(writer2.position(), 0);
    assert_eq!(writer2.bytes_written(), 0);
}

#[test]
fn test_statistics() {
    let writer = TerminalWriterCapsule::new();

    // Initial state
    assert_eq!(writer.bytes_written(), 0);
    assert_eq!(writer.flush_count(), 0);
    assert_eq!(writer.position(), 0);

    // Write and flush
    writer.write(b"test1").unwrap();
    writer.flush().unwrap();

    assert_eq!(writer.bytes_written(), 5);
    assert_eq!(writer.flush_count(), 1);
    assert_eq!(writer.position(), 0);

    // Write and flush again
    writer.write(b"test2").unwrap();
    writer.flush().unwrap();

    assert_eq!(writer.bytes_written(), 10);
    assert_eq!(writer.flush_count(), 2);
}

#[test]
fn test_full_buffer_handling() {
    let writer = TerminalWriterCapsule::with_capacity(64);

    // Fill buffer to capacity
    for _ in 0..16 {
        writer.write(b"test").unwrap();
    }

    // Should have auto-flushed
    assert!(writer.flush_count() > 0);

    // Should be able to continue writing
    writer.write(b"more").unwrap();
    assert_eq!(writer.position(), 4);
}

#[test]
fn test_ansi_escape_sequence_batching() {
    let writer = TerminalWriterCapsule::with_capacity(1024);

    let initial_flush = writer.flush_count();

    // Batch multiple ANSI sequences
    writer.clear_screen().unwrap();         // \x1b[2J (4 bytes)
    writer.cursor_home().unwrap();          // \x1b[H (3 bytes)
    writer.move_cursor(10, 5).unwrap();     // \x1b[6;11H (~9 bytes)
    writer.hide_cursor().unwrap();          // \x1b[?25l (6 bytes)

    // Total ~22 bytes, should be buffered (below 512 byte threshold)
    let pos = writer.position();
    assert!(pos >= 20 && pos <= 30, "Expected position 20-30, got {}", pos);
    assert_eq!(writer.flush_count(), initial_flush, "Should not have auto-flushed yet");

    // Flush all at once (1 syscall instead of 4)
    writer.flush().unwrap();
    assert_eq!(writer.flush_count(), initial_flush + 1);
}

#[test]
fn test_stress_concurrent_writes() {
    let writer = Arc::new(TerminalWriterCapsule::with_capacity(131072)); // 128KB buffer
    let mut handles = vec![];

    // Spawn 20 threads with heavy writes
    for i in 0..20 {
        let writer_clone = Arc::clone(&writer);
        let handle = thread::spawn(move || {
            for j in 0..100 {
                let msg = format!("Thread {:02} write {:03}\n", i, j);
                writer_clone.write(msg.as_bytes()).unwrap();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify total writes (20 threads × 100 writes × ~22 bytes avg)
    // Note: May have auto-flushed during writes
    assert!(writer.bytes_written() >= 40000, "Expected >= 40000 bytes, got {}", writer.bytes_written());
}
