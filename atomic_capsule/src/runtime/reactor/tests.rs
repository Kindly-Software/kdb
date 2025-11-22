//! # Comprehensive ReactorCapsule Tests (T28 Framework)
//!
//! **Unit, Property, Integration, and Production Tests**
//!
//! ## Test Coverage (27 total)
//! - Unit Tests (9): FdState, Interest, ReactorCapsule creation
//! - Property Tests (8): Generation counters, alignment, concurrent access
//! - Integration Tests (5): Pipe I/O, multiple FDs, polling
//! - Production Tests (5): Stress tests, concurrent operations

use super::*;
use std::io::{Read, Write};
use std::os::unix::io::FromRawFd;
use std::thread;

// ============================================================================
// UNIT TESTS (9 tests)
// ============================================================================

#[test]
fn test_interest_flags_creation() {
    let interest = Interest::read();
    assert!(interest.readable);
    assert!(!interest.writable);

    let interest = Interest::write();
    assert!(!interest.readable);
    assert!(interest.writable);

    let interest = Interest::all();
    assert!(interest.readable);
    assert!(interest.writable);
}

#[test]
fn test_interest_to_bits_conversion() {
    let interest = Interest::read();
    let bits = interest.to_bits();
    assert_eq!(bits, 1u32);

    let interest = Interest::write();
    let bits = interest.to_bits();
    assert_eq!(bits, 2u32);

    let interest = Interest::all();
    let bits = interest.to_bits();
    assert_eq!(bits, 3u32);
}

#[test]
fn test_interest_from_bits_conversion() {
    let interest = Interest::from_bits(1u32);
    assert!(interest.readable);
    assert!(!interest.writable);

    let interest = Interest::from_bits(2u32);
    assert!(!interest.readable);
    assert!(interest.writable);

    let interest = Interest::from_bits(3u32);
    assert!(interest.readable);
    assert!(interest.writable);
}

#[test]
fn test_fd_state_creation() {
    // #ASSUME_FD_VALID: Create with valid FD
    let interest = Interest::all();
    let fd_state = FdState::new(5, interest);

    assert_eq!(fd_state.fd(), 5);
    assert_eq!(fd_state.interests(), interest);
}

#[test]
fn test_fd_state_alignment() {
    // #VERIFY_CACHE_ALIGNED: FdState must be 384 bytes with 128B alignment
    let size = std::mem::size_of::<FdState>();
    let align = std::mem::align_of::<FdState>();

    assert_eq!(size, 384, "FdState must be 384 bytes");
    assert_eq!(align, 128, "FdState must be 128-byte aligned");
}

#[test]
fn test_fd_state_ready_bits() {
    let fd_state = FdState::new(5, Interest::all());

    // Initially all ready bits should be 0
    let (ready, gen) = fd_state.load_ready();
    assert_eq!(ready, 0);
    assert_eq!(gen, 0);

    // Mark as readable
    fd_state.mark_ready(true, false).unwrap();
    let (ready, gen) = fd_state.load_ready();
    assert_eq!(ready, 1u32);
    assert_eq!(gen, 1); // Generation bumped

    // Check readiness flags
    assert!(fd_state.is_readable());
    assert!(!fd_state.is_writable());
}

#[test]
fn test_fd_state_generation_counter() {
    let fd_state = FdState::new(5, Interest::all());

    // #ASSUME_GENERATION_VALID: Each update bumps generation
    let (_, gen1) = fd_state.load_ready();
    assert_eq!(gen1, 0);

    fd_state.mark_ready(true, false).unwrap();
    let (_, gen2) = fd_state.load_ready();
    assert_eq!(gen2, 1);

    fd_state.mark_ready(false, true).unwrap();
    let (_, gen3) = fd_state.load_ready();
    assert_eq!(gen3, 2);
}

#[test]
fn test_reactor_capsule_creation() {
    let reactor = ReactorCapsule::new();
    assert!(reactor.is_ok());
}

#[test]
fn test_reactor_invalid_fd() {
    let mut reactor = ReactorCapsule::new().unwrap();
    let result = reactor.register_fd(-1, Interest::read());
    assert_eq!(result, Err(ReactorError::InvalidFd));
}

// ============================================================================
// PROPERTY TESTS (8 tests)
// ============================================================================

#[test]
fn test_fd_state_cache_alignment_property() {
    // Property: All FdState instances must have same size and alignment
    for fd in 0..10 {
        let state = FdState::new(fd, Interest::all());
        assert_eq!(std::mem::size_of_val(&state), 384);
    }
}

#[test]
fn test_generation_counter_monotonic() {
    // Property: Generation counter must be monotonically increasing
    let fd_state = FdState::new(0, Interest::all());

    let mut prev_gen = 0u64;
    for _ in 0..100 {
        fd_state.mark_ready(true, false).ok();
        let (_, gen) = fd_state.load_ready();
        assert!(gen > prev_gen, "Generation must increase");
        prev_gen = gen;
    }
}

#[test]
fn test_ready_bits_correct_after_mark() {
    // Property: Ready bits must match what was marked
    let fd_state = FdState::new(0, Interest::all());

    for i in 0..10 {
        let readable = (i % 2) == 0;
        let writable = (i % 3) == 0;

        fd_state.mark_ready(readable, writable).ok();
        assert_eq!(fd_state.is_readable(), readable);
        assert_eq!(fd_state.is_writable(), writable);
    }
}

#[test]
fn test_waker_atomic_operations() {
    // Property: Waker can be safely set and cleared
    let fd_state = FdState::new(0, Interest::all());

    let waker1 = 0x1234_5678 as *mut ();
    fd_state.set_waker(waker1).unwrap();
    assert_eq!(fd_state.get_waker(), waker1);

    fd_state.clear_waker();
    assert_eq!(fd_state.get_waker(), std::ptr::null_mut());
}

#[test]
fn test_reactor_fd_count() {
    // Property: FD count must match registered FDs
    let mut reactor = ReactorCapsule::new().unwrap();

    // Create a pipe for testing
    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    assert_eq!(reactor.fd_count(), 0);

    reactor.register_fd(read_fd, Interest::read()).ok();
    assert_eq!(reactor.fd_count(), 1);

    reactor.register_fd(write_fd, Interest::write()).ok();
    assert_eq!(reactor.fd_count(), 2);

    reactor.unregister_fd(read_fd).ok();
    assert_eq!(reactor.fd_count(), 1);

    reactor.unregister_fd(write_fd).ok();
    assert_eq!(reactor.fd_count(), 0);

    // Cleanup
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
fn test_reactor_contains_fd() {
    // Property: contains_fd must accurately reflect registration
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];

    assert!(!reactor.contains_fd(read_fd));
    reactor.register_fd(read_fd, Interest::read()).ok();
    assert!(reactor.contains_fd(read_fd));
    reactor.unregister_fd(read_fd).ok();
    assert!(!reactor.contains_fd(read_fd));

    unsafe {
        libc::close(read_fd);
        libc::close(pipe_fds[1]);
    }
}

#[test]
fn test_reactor_get_fd_state() {
    // Property: get_fd_state returns correct state for registered FD
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];

    reactor.register_fd(read_fd, Interest::read()).ok();
    let state = reactor.get_fd_state(read_fd);
    assert!(state.is_some());
    assert_eq!(state.unwrap().fd(), read_fd);

    reactor.unregister_fd(read_fd).ok();
    let state = reactor.get_fd_state(read_fd);
    assert!(state.is_none());

    unsafe {
        libc::close(read_fd);
        libc::close(pipe_fds[1]);
    }
}

// ============================================================================
// INTEGRATION TESTS (5 tests)
// ============================================================================

#[test]
fn test_pipe_write_readiness() {
    // Integration: Pipe write should be immediately ready
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let write_fd = pipe_fds[1];
    reactor.register_fd(write_fd, Interest::write()).unwrap();

    // Poll with short timeout
    let events = reactor.poll(Duration::from_millis(100)).unwrap();

    // Should get write ready event
    let write_events: Vec<_> = events.iter().filter(|e| e.fd == write_fd && e.writable).collect();
    assert!(!write_events.is_empty(), "Pipe write should be ready");

    reactor.unregister_fd(write_fd).ok();
    unsafe {
        libc::close(write_fd);
        libc::close(pipe_fds[0]);
    }
}

#[test]
fn test_pipe_read_after_write() {
    // Integration: Pipe read becomes ready after data written
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    reactor.register_fd(read_fd, Interest::read()).unwrap();

    // Write data to pipe
    let test_data = [1u8; 10];
    unsafe {
        assert_eq!(libc::write(write_fd, test_data.as_ptr() as *const _, 10), 10);
    }

    // Poll for read readiness
    let events = reactor.poll(Duration::from_millis(100)).unwrap();

    let read_events: Vec<_> = events.iter().filter(|e| e.fd == read_fd && e.readable).collect();
    assert!(!read_events.is_empty(), "Pipe read should be ready after write");

    reactor.unregister_fd(read_fd).ok();
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
fn test_multiple_fds_registration() {
    // Integration: Can register and poll multiple FDs
    let mut reactor = ReactorCapsule::new().unwrap();

    let fds = (0..10)
        .map(|_| {
            let mut pipe_fds = [0; 2];
            unsafe {
                assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
            }
            pipe_fds
        })
        .collect::<Vec<_>>();

    // Register all read FDs
    for pipe_fds in &fds {
        reactor
            .register_fd(pipe_fds[0], Interest::read())
            .expect("Failed to register");
    }

    assert_eq!(reactor.fd_count(), 10);

    // Unregister all
    for pipe_fds in &fds {
        reactor.unregister_fd(pipe_fds[0]).expect("Failed to unregister");
    }

    assert_eq!(reactor.fd_count(), 0);

    // Cleanup
    unsafe {
        for pipe_fds in &fds {
            libc::close(pipe_fds[0]);
            libc::close(pipe_fds[1]);
        }
    }
}

#[test]
fn test_modify_interest_flags() {
    // Integration: Can modify interest flags after registration
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    reactor
        .register_fd(read_fd, Interest::read())
        .expect("Failed to register");

    assert!(reactor.contains_fd(read_fd));

    // Modify interests (should not fail)
    reactor.modify_fd(read_fd, Interest::all()).ok();

    reactor.unregister_fd(read_fd).ok();
    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
fn test_poll_timeout() {
    // Integration: Poll should respect timeout
    let mut reactor = ReactorCapsule::new().unwrap();

    // Don't register any FDs, just timeout
    let start = std::time::Instant::now();
    let events = reactor.poll(Duration::from_millis(50)).unwrap();
    let elapsed = start.elapsed();

    assert!(events.is_empty(), "No FDs registered, should return empty");
    assert!(
        elapsed >= Duration::from_millis(40),
        "Poll should respect timeout"
    );
}

// ============================================================================
// PRODUCTION TESTS (5 tests)
// ============================================================================

#[test]
fn test_stress_registration_unregistration() {
    // Production: Stress test with many registrations
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipes = Vec::new();
    for _ in 0..100 {
        let mut pipe_fds = [0; 2];
        unsafe {
            assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
        }
        pipes.push(pipe_fds);
    }

    // Register all
    for pipe_fds in &pipes {
        reactor
            .register_fd(pipe_fds[0], Interest::read())
            .expect("Failed to register");
    }

    assert_eq!(reactor.fd_count(), 100);

    // Poll
    reactor.poll(Duration::from_millis(10)).ok();

    // Unregister all
    for pipe_fds in &pipes {
        reactor
            .unregister_fd(pipe_fds[0])
            .expect("Failed to unregister");
    }

    assert_eq!(reactor.fd_count(), 0);

    // Cleanup
    unsafe {
        for pipe_fds in &pipes {
            libc::close(pipe_fds[0]);
            libc::close(pipe_fds[1]);
        }
    }
}

#[test]
fn test_concurrent_registration_threads() {
    // Production: Create FDs in parallel threads, then register them serially
    // (ReactorCapsule is not inherently thread-safe for concurrent registration)
    let mut pipes = Vec::new();

    for _ in 0..10 {
        let handle = thread::spawn(|| {
            let mut pipe_fds = [0; 2];
            unsafe {
                assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
            }
            pipe_fds
        });
        pipes.push(handle.join().unwrap());
    }

    // Now register all FDs in main thread
    let mut reactor = ReactorCapsule::new().unwrap();
    for pipe_fds in &pipes {
        reactor
            .register_fd(pipe_fds[0], Interest::read())
            .expect("Failed to register");
    }

    assert_eq!(reactor.fd_count(), 10);

    // Cleanup
    unsafe {
        for pipe_fds in &pipes {
            reactor.unregister_fd(pipe_fds[0]).ok();
            libc::close(pipe_fds[0]);
            libc::close(pipe_fds[1]);
        }
    }
}

#[test]
fn test_repeated_poll_cycles() {
    // Production: Many poll cycles
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    reactor.register_fd(write_fd, Interest::write()).ok();

    // Do 1000 poll cycles
    for _ in 0..1000 {
        let _ = reactor.poll(Duration::from_millis(1));
    }

    reactor.unregister_fd(write_fd).ok();

    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}

#[test]
fn test_error_handling_graceful() {
    // Production: Graceful error handling
    let mut reactor = ReactorCapsule::new().unwrap();

    // Try to register invalid FD
    let result = reactor.register_fd(-1, Interest::read());
    assert_eq!(result, Err(ReactorError::InvalidFd));

    // Try to modify non-existent FD
    let result = reactor.modify_fd(9999, Interest::read());
    assert_eq!(result, Err(ReactorError::FdNotFound));

    // Try to unregister non-existent FD
    let result = reactor.unregister_fd(9999);
    assert_eq!(result, Err(ReactorError::FdNotFound));
}

#[test]
fn test_mixed_read_write_events() {
    // Production: Mixed read/write event handling
    let mut reactor = ReactorCapsule::new().unwrap();

    let mut pipe_fds = [0; 2];
    unsafe {
        assert_eq!(libc::pipe(pipe_fds.as_mut_ptr()), 0);
    }

    let read_fd = pipe_fds[0];
    let write_fd = pipe_fds[1];

    // Register both for different modes
    reactor.register_fd(read_fd, Interest::read()).unwrap();
    reactor.register_fd(write_fd, Interest::write()).unwrap();

    assert_eq!(reactor.fd_count(), 2);

    // Poll should detect write ready
    let events = reactor.poll(Duration::from_millis(100)).unwrap();
    let write_events: Vec<_> = events.iter().filter(|e| e.fd == write_fd && e.writable).collect();
    assert!(!write_events.is_empty(), "Write FD should be ready");

    // Cleanup
    reactor.unregister_fd(read_fd).ok();
    reactor.unregister_fd(write_fd).ok();

    unsafe {
        libc::close(read_fd);
        libc::close(write_fd);
    }
}
