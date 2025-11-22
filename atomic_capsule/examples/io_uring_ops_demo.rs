//! io_uring Operations Demo - T1+T5 (Atomic + Streaming)
//!
//! Demonstrates all major io_uring operation builders with type-safe, validated SQE preparation.
//!
//! # Architecture
//!
//! - T1 Atomic: <50ns per operation SQE setup
//! - T5 Streaming: O(1) completion harvesting
//! - 100% lockfree (zero mutexes)
//! - Zero-copy operation chaining
//!
//! # Performance Targets
//!
//! - Operation prep: <50ns (B32 TYPICAL tier)
//! - Completion harvesting: <500ns per 10 CQEs
//! - Fixed buffer I/O: <20ns per operation (zero allocation)

#![allow(dead_code)]

#[cfg(all(target_os = "linux", feature = "std"))]
fn main() {
    use atomic_capsule::IoUringCapsule;

    println!("io_uring Operations Demo");
    println!("========================\n");

    // Initialize ring (256 SQE, 512 CQE)
    match IoUringCapsule::new(256, 0) {
        Ok(ring) => {
            println!("✓ Ring initialized (256 SQ, 512 CQ entries)");

            demo_read_write(&ring);
            demo_socket_ops(&ring);
            demo_file_ops(&ring);
            demo_polling(&ring);
            demo_chaining(&ring);
            demo_helper_ops(&ring);
        }
        Err(e) => println!("✗ Ring initialization failed: {}", e),
    }

    println!("\nDemo complete!");
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_read_write(ring: &atomic_capsule::IoUringCapsule) {
    use atomic_capsule::IoUringCapsule;

    println!("\n--- Read/Write Operations ---");

    // Read operation
    let mut buf = vec![0u8; 4096];
    match ring.prep_read(3, &mut buf, 0, 1) {
        Ok(()) => println!("✓ prep_read: FD=3, 4096 bytes, user_data=1"),
        Err(e) => println!("✗ prep_read failed: {}", e),
    }

    // Write operation
    let data = b"Hello, io_uring!";
    match ring.prep_write(3, data, 0, 2) {
        Ok(()) => println!("✓ prep_write: FD=3, {} bytes, user_data=2", data.len()),
        Err(e) => println!("✗ prep_write failed: {}", e),
    }

    // Fixed buffer operations (zero-copy)
    match ring.prep_read_fixed(3, 0, 0, 4096, 3) {
        Ok(()) => println!("✓ prep_read_fixed: buffer_index=0, 4096 bytes"),
        Err(e) => println!("✗ prep_read_fixed failed: {}", e),
    }

    match ring.prep_write_fixed(3, 0, 0, 4096, 4) {
        Ok(()) => println!("✓ prep_write_fixed: buffer_index=0, 4096 bytes"),
        Err(e) => println!("✗ prep_write_fixed failed: {}", e),
    }
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_socket_ops(ring: &atomic_capsule::IoUringCapsule) {
    println!("\n--- Socket Operations ---");

    // Accept incoming connection
    match ring.prep_accept(5, 10) {
        Ok(()) => println!("✓ prep_accept: listen_fd=5, user_data=10"),
        Err(e) => println!("✗ prep_accept failed: {}", e),
    }

    // Connect to remote
    match ring.prep_connect(6, 11) {
        Ok(()) => println!("✓ prep_connect: socket_fd=6, user_data=11"),
        Err(e) => println!("✗ prep_connect failed: {}", e),
    }

    // Send data
    let msg = b"Hello, TCP!";
    match ring.prep_send(6, msg, 0, 12) {
        Ok(()) => println!("✓ prep_send: FD=6, {} bytes, user_data=12", msg.len()),
        Err(e) => println!("✗ prep_send failed: {}", e),
    }

    // Receive data
    let mut rbuf = vec![0u8; 1024];
    match ring.prep_recv(6, &mut rbuf, 0, 13) {
        Ok(()) => println!("✓ prep_recv: FD=6, 1024 bytes, user_data=13"),
        Err(e) => println!("✗ prep_recv failed: {}", e),
    }

    // Send/recv with message structures
    match ring.prep_sendmsg(6, 14) {
        Ok(()) => println!("✓ prep_sendmsg: FD=6, user_data=14"),
        Err(e) => println!("✗ prep_sendmsg failed: {}", e),
    }

    match ring.prep_recvmsg(6, 15) {
        Ok(()) => println!("✓ prep_recvmsg: FD=6, user_data=15"),
        Err(e) => println!("✗ prep_recvmsg failed: {}", e),
    }
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_file_ops(ring: &atomic_capsule::IoUringCapsule) {
    println!("\n--- File Operations ---");

    // Open file
    match ring.prep_openat(-1, 0, 0o644, 20) {
        Ok(()) => println!("✓ prep_openat: AT_FDCWD, flags=0, mode=0644, user_data=20"),
        Err(e) => println!("✗ prep_openat failed: {}", e),
    }

    // Close file
    match ring.prep_close(3, 21) {
        Ok(()) => println!("✓ prep_close: FD=3, user_data=21"),
        Err(e) => println!("✗ prep_close failed: {}", e),
    }

    // Fsync
    match ring.prep_fsync(3, 0, 22) {
        Ok(()) => println!("✓ prep_fsync: FD=3, flags=0, user_data=22"),
        Err(e) => println!("✗ prep_fsync failed: {}", e),
    }

    // Stat file
    match ring.prep_statx(-1, 0, 0xFFF, 23) {
        Ok(()) => println!("✓ prep_statx: AT_FDCWD, mask=0xFFF, user_data=23"),
        Err(e) => println!("✗ prep_statx failed: {}", e),
    }

    // Sync file range
    match ring.prep_sync_file_range(3, 0, 65536, 0x3, 24) {
        Ok(()) => println!("✓ prep_sync_file_range: FD=3, 65536 bytes, user_data=24"),
        Err(e) => println!("✗ prep_sync_file_range failed: {}", e),
    }
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_polling(ring: &atomic_capsule::IoUringCapsule) {
    use atomic_capsule::{IORING_POLL_IN, IORING_POLL_OUT};

    println!("\n--- Polling Operations ---");

    // Poll for read
    match ring.prep_poll_add(6, IORING_POLL_IN, 30) {
        Ok(()) => println!("✓ prep_poll_add: FD=6, POLLIN, user_data=30"),
        Err(e) => println!("✗ prep_poll_add failed: {}", e),
    }

    // Poll for write
    match ring.prep_poll_add(6, IORING_POLL_OUT, 31) {
        Ok(()) => println!("✓ prep_poll_add: FD=6, POLLOUT, user_data=31"),
        Err(e) => println!("✗ prep_poll_add failed: {}", e),
    }

    // Remove poll
    match ring.prep_poll_remove(30) {
        Ok(()) => println!("✓ prep_poll_remove: user_data=30"),
        Err(e) => println!("✗ prep_poll_remove failed: {}", e),
    }

    // Timeout
    match ring.prep_timeout(1_000_000_000, 0, 32) {
        Ok(()) => println!("✓ prep_timeout: 1s, user_data=32"),
        Err(e) => println!("✗ prep_timeout failed: {}", e),
    }
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_chaining(ring: &atomic_capsule::IoUringCapsule) {
    println!("\n--- Operation Chaining ---");

    // Chained read -> write (Note: set_*_flag() requires IoUringCapsule API extension)
    let mut buf = vec![0u8; 256];
    match ring.prep_read(3, &mut buf, 0, 40) {
        Ok(()) => {
            println!("✓ prep_read: FD=3 (will be first in chain)");

            // Note: set_link_flag() currently requires API extension to IoUringCapsule
            // for safe flag modification on last SQE. In production, flag can be set
            // during prep_read by extending the operation builder interface.
            match ring.set_link_flag() {
                Ok(()) => println!("✓ set_link_flag: Chain to next operation"),
                Err(e) => println!("⚠ set_link_flag requires API extension: {}", e),
            }

            match ring.prep_write(4, &buf[..], 0, 41) {
                Ok(()) => println!("✓ prep_write: FD=4 (depends on read)"),
                Err(e) => println!("✗ prep_write failed: {}", e),
            }
        }
        Err(e) => println!("✗ prep_read failed: {}", e),
    }

    // Flag setting attempts
    println!("\n  Flag operations (require IoUringCapsule extension):");
    match ring.set_async_flag() {
        Ok(()) => println!("  ✓ set_async_flag"),
        Err(_) => println!("  ⚠ set_async_flag requires API extension"),
    }

    match ring.set_hardlink_flag() {
        Ok(()) => println!("  ✓ set_hardlink_flag"),
        Err(_) => println!("  ⚠ set_hardlink_flag requires API extension"),
    }

    match ring.set_skip_success_flag() {
        Ok(()) => println!("  ✓ set_skip_success_flag"),
        Err(_) => println!("  ⚠ set_skip_success_flag requires API extension"),
    }
}

#[cfg(all(target_os = "linux", feature = "std"))]
fn demo_helper_ops(ring: &atomic_capsule::IoUringCapsule) {
    println!("\n--- Helper Operations ---");

    // NOP (useful for testing)
    match ring.prep_nop(50) {
        Ok(()) => println!("✓ prep_nop: user_data=50"),
        Err(e) => println!("✗ prep_nop failed: {}", e),
    }

    // Show ring stats
    let stats = ring.stats();
    println!("\nRing Statistics:");
    println!("  Total submissions: {}", stats.total_submissions);
    println!("  Total completions: {}", stats.total_completions);
    println!("  Submission errors: {}", stats.submission_errors);
    println!("  Completion errors: {}", stats.completion_errors);
    println!("  SQ dropped: {}", stats.sq_dropped);
    println!("  CQ overflow: {}", stats.cq_overflow);
}

#[cfg(not(all(target_os = "linux", feature = "std")))]
fn main() {
    println!("io_uring is only available on Linux with 'std' feature enabled");
    println!("Run with: cargo run --example io_uring_ops_demo --features std");
}
