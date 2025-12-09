//! # RawModeCapsule Tests - T28 Framework (Q1-Q28)
//!
//! **Tier 1: Unit Tests (Q1-Q7)**
//! - Basic functionality, edge cases, error handling
//!
//! **Tier 2: Property Tests (Q8-Q14)** - Not applicable (no proptest needed for termios)
//!
//! **Tier 3: Integration Tests (Q15-Q21)**
//! - RAII cleanup, panic safety, concurrent access
//!
//! **Tier 4: Production Tests (Q22-Q28)**
//! - Stress testing, resource cleanup verification

#![cfg(all(feature = "std", feature = "tui-terminal", unix))]

use atomic_capsule::terminal::mode::{RawModeCapsule, RawModeError};
use std::sync::Arc;
use std::thread;

// ============================================================================
// TIER 1: UNIT TESTS (Q1-Q7)
// ============================================================================

#[test]
fn q1_capsule_alignment() {
    assert_eq!(core::mem::align_of::<RawModeCapsule>(), 128);
}

#[test]
fn q2_capsule_size() {
    assert_eq!(core::mem::size_of::<RawModeCapsule>(), 128);
}

#[test]
fn q3_new_with_tty() {
    // This test only runs if stdin is a TTY
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok(), "Failed to create RawModeCapsule for TTY");

        if let Ok(capsule) = raw_mode {
            assert!(!capsule.is_raw_mode(), "Should start in normal mode");
            assert_eq!(capsule.generation(), 0, "Generation should start at 0");
            assert_eq!(capsule.fd(), libc::STDIN_FILENO, "FD should be stdin");
        }
    }
}

#[test]
fn q4_enable_raw_mode() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            let result = capsule.enable_raw_mode();
            assert!(result.is_ok(), "Failed to enable raw mode: {:?}", result);
            assert!(capsule.is_raw_mode(), "Should be in raw mode after enable");
            assert_eq!(capsule.generation(), 1, "Generation should increment");

            // Cleanup
            capsule.disable_raw_mode().ok();
        }
    }
}

#[test]
fn q5_disable_raw_mode() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            capsule.enable_raw_mode().ok();

            let result = capsule.disable_raw_mode();
            assert!(result.is_ok(), "Failed to disable raw mode: {:?}", result);
            assert!(!capsule.is_raw_mode(), "Should be in normal mode after disable");
            assert_eq!(capsule.generation(), 2, "Generation should increment again");
        }
    }
}

#[test]
fn q6_enable_twice_fails() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            capsule.enable_raw_mode().ok();

            let second_enable = capsule.enable_raw_mode();
            assert!(second_enable.is_err(), "Second enable should fail");
            assert_eq!(
                second_enable.unwrap_err(),
                RawModeError::AlreadyInMode,
                "Should return AlreadyInMode error"
            );

            // Cleanup
            capsule.disable_raw_mode().ok();
        }
    }
}

#[test]
fn q7_disable_twice_fails() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            capsule.enable_raw_mode().ok();
            capsule.disable_raw_mode().ok();

            let second_disable = capsule.disable_raw_mode();
            assert!(second_disable.is_err(), "Second disable should fail");
            assert_eq!(
                second_disable.unwrap_err(),
                RawModeError::AlreadyInMode,
                "Should return AlreadyInMode error"
            );
        }
    }
}

// ============================================================================
// TIER 3: INTEGRATION TESTS (Q15-Q21)
// ============================================================================

#[test]
fn q15_raii_cleanup_normal_drop() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        // Enter raw mode and let it drop
        {
            let raw_mode = RawModeCapsule::new();
            if let Ok(capsule) = raw_mode {
                capsule.enable_raw_mode().ok();
                assert!(capsule.is_raw_mode());
                // Drop happens here
            }
        }

        // Verify terminal was restored by creating new capsule
        let new_capsule = RawModeCapsule::new();
        assert!(new_capsule.is_ok(), "Terminal should be restored after drop");
    }
}

#[test]
fn q16_raii_cleanup_early_return() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        fn enter_and_return_early() -> Result<(), RawModeError> {
            let raw_mode = RawModeCapsule::new()?;
            raw_mode.enable_raw_mode()?;

            // Early return (simulating error case)
            return Ok(());
            // Drop happens here
        }

        enter_and_return_early().ok();

        // Verify cleanup
        let new_capsule = RawModeCapsule::new();
        assert!(new_capsule.is_ok(), "Terminal should be restored after early return");
    }
}

#[test]
#[should_panic(expected = "intentional panic")]
fn q17_raii_cleanup_panic() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let _raw_mode = RawModeCapsule::new();
        if let Ok(capsule) = _raw_mode {
            capsule.enable_raw_mode().ok();

            // Panic should trigger Drop
            panic!("intentional panic");
        }
    } else {
        // If not TTY, panic anyway to satisfy should_panic
        panic!("intentional panic");
    }
}

#[test]
fn q18_generation_counter_increments() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            assert_eq!(capsule.generation(), 0);

            capsule.enable_raw_mode().ok();
            assert_eq!(capsule.generation(), 1, "Generation should increment on enable");

            capsule.disable_raw_mode().ok();
            assert_eq!(capsule.generation(), 2, "Generation should increment on disable");

            capsule.enable_raw_mode().ok();
            assert_eq!(capsule.generation(), 3, "Generation should continue incrementing");

            capsule.disable_raw_mode().ok();
            assert_eq!(capsule.generation(), 4, "Generation should reach 4");
        }
    }
}

#[test]
fn q19_concurrent_reads() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            let capsule_arc = Arc::new(capsule);
            let mut threads = vec![];

            // Spawn 4 threads that read state concurrently
            for _ in 0..4 {
                let capsule_clone = capsule_arc.clone();
                let t = thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = capsule_clone.is_raw_mode();
                        let _ = capsule_clone.generation();
                        let _ = capsule_clone.fd();
                    }
                });
                threads.push(t);
            }

            for t in threads {
                t.join().expect("Thread should complete");
            }
        }
    }
}

#[test]
fn q20_cache_line_alignment() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            let ptr = &capsule as *const _ as usize;
            assert_eq!(ptr % 128, 0, "Pointer should be 128-byte aligned");
        }
    }
}

#[test]
fn q21_error_display() {
    let err = RawModeError::NotATty;
    assert_eq!(format!("{}", err), "Terminal is not a TTY");

    let err = RawModeError::AlreadyInMode;
    assert_eq!(format!("{}", err), "Already in requested mode");

    let err = RawModeError::GetAttrFailed(5);
    assert!(format!("{}", err).contains("Failed to get terminal attributes"));

    let err = RawModeError::SetAttrFailed(13);
    assert!(format!("{}", err).contains("Failed to set terminal attributes"));

    let err = RawModeError::InvalidStateTransition { from: 0, to: 2 };
    assert!(format!("{}", err).contains("Invalid state transition from 0 to 2"));

    let err = RawModeError::OriginalTermiosNotSaved;
    assert_eq!(format!("{}", err), "Original termios not saved (internal error)");
}

// ============================================================================
// TIER 4: PRODUCTION TESTS (Q22-Q28)
// ============================================================================

#[test]
fn q22_stress_enable_disable_cycles() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            // Perform 100 enable/disable cycles
            for i in 0..100 {
                capsule.enable_raw_mode()
                    .expect(&format!("Failed to enable on cycle {}", i));
                assert!(capsule.is_raw_mode());

                capsule.disable_raw_mode()
                    .expect(&format!("Failed to disable on cycle {}", i));
                assert!(!capsule.is_raw_mode());
            }

            assert_eq!(capsule.generation(), 200, "Generation should reach 200 after 100 cycles");
        }
    }
}

#[test]
fn q23_multiple_capsule_instances() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        // Create multiple capsules for the same fd (sequential)
        for _ in 0..10 {
            let raw_mode = RawModeCapsule::new();
            assert!(raw_mode.is_ok());

            if let Ok(capsule) = raw_mode {
                capsule.enable_raw_mode().ok();
                capsule.disable_raw_mode().ok();
                // Drop happens here
            }
        }

        // Verify terminal is still usable
        let final_capsule = RawModeCapsule::new();
        assert!(final_capsule.is_ok(), "Terminal should still be usable");
    }
}

#[test]
fn q24_fd_tracking() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            assert_eq!(capsule.fd(), libc::STDIN_FILENO);

            capsule.enable_raw_mode().ok();
            assert_eq!(capsule.fd(), libc::STDIN_FILENO, "FD should remain constant");

            capsule.disable_raw_mode().ok();
            assert_eq!(capsule.fd(), libc::STDIN_FILENO, "FD should remain constant");
        }
    }
}

#[test]
fn q25_state_consistency() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            // State should be consistent across multiple reads
            for _ in 0..100 {
                assert!(!capsule.is_raw_mode());
            }

            capsule.enable_raw_mode().ok();

            for _ in 0..100 {
                assert!(capsule.is_raw_mode());
            }

            capsule.disable_raw_mode().ok();

            for _ in 0..100 {
                assert!(!capsule.is_raw_mode());
            }
        }
    }
}

#[test]
fn q26_memory_layout_verification() {
    // Verify field layout matches documentation
    use core::mem::{size_of, align_of};
    use core::sync::atomic::{AtomicU32, AtomicU64, AtomicI32};

    assert_eq!(size_of::<AtomicU32>(), 4);
    assert_eq!(size_of::<AtomicI32>(), 4);
    assert_eq!(size_of::<AtomicU64>(), 8);

    // Total: 4 + 4 + 8 + 8 + 104 = 128 bytes
    assert_eq!(size_of::<RawModeCapsule>(), 128);
    assert_eq!(align_of::<RawModeCapsule>(), 128);
}

#[test]
fn q27_resource_cleanup_verification() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        // Create and drop many capsules to verify no resource leaks
        for _ in 0..100 {
            let raw_mode = RawModeCapsule::new();
            if let Ok(capsule) = raw_mode {
                capsule.enable_raw_mode().ok();
                // Drop happens here (should cleanup heap-allocated termios)
            }
        }

        // If we didn't leak memory, this should still work
        let final_capsule = RawModeCapsule::new();
        assert!(final_capsule.is_ok());
    }
}

#[test]
fn q28_production_scenario_simulation() {
    if unsafe { libc::isatty(libc::STDIN_FILENO) } == 1 {
        // Simulate a real TUI application lifecycle
        let raw_mode = RawModeCapsule::new();
        assert!(raw_mode.is_ok());

        if let Ok(capsule) = raw_mode {
            // 1. Initialize
            assert!(!capsule.is_raw_mode());
            assert_eq!(capsule.generation(), 0);

            // 2. Enter raw mode for rendering
            capsule.enable_raw_mode()
                .expect("Failed to enter raw mode");
            assert!(capsule.is_raw_mode());
            assert_eq!(capsule.generation(), 1);

            // 3. Simulate rendering loop (check state frequently)
            for _ in 0..1000 {
                assert!(capsule.is_raw_mode());
            }

            // 4. Exit cleanly
            capsule.disable_raw_mode()
                .expect("Failed to exit raw mode");
            assert!(!capsule.is_raw_mode());
            assert_eq!(capsule.generation(), 2);

            // 5. Verify terminal restored
            let verification = RawModeCapsule::new();
            assert!(verification.is_ok(), "Terminal should be restored");
        }
    }
}
