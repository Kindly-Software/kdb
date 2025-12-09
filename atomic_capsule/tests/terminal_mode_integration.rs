//! Integration tests for terminal mode capsules
//!
//! Tests AlternateScreenCapsule and CursorCapsule in isolation and composition.

#[cfg(all(unix, feature = "std"))]
mod terminal_mode_tests {
    use atomic_capsule::terminal::mode::{AlternateScreenCapsule, CursorCapsule, RawModeCapsule};

    #[test]
    fn test_alternate_screen_capsule_creation() {
        // Test creation and basic properties
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let capsule = AlternateScreenCapsule::new().unwrap();
            assert!(!capsule.is_alternate(), "Should start in main screen");
            assert_eq!(capsule.generation(), 0, "Generation should start at 0");
            assert_eq!(capsule.fd(), libc::STDOUT_FILENO, "FD should be stdout");
        }
    }

    #[test]
    fn test_alternate_screen_enter_leave() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let screen = AlternateScreenCapsule::new().unwrap();

            // Enter alternate screen
            assert!(screen.enter().is_ok(), "Failed to enter alternate screen");
            assert!(screen.is_alternate(), "Should be in alternate screen");
            assert_eq!(screen.generation(), 1, "Generation should increment");

            // Leave alternate screen
            assert!(screen.leave().is_ok(), "Failed to leave alternate screen");
            assert!(!screen.is_alternate(), "Should be back in main screen");
            assert_eq!(screen.generation(), 2, "Generation should increment again");
        }
    }

    #[test]
    fn test_alternate_screen_raii_cleanup() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            {
                let screen = AlternateScreenCapsule::new().unwrap();
                screen.enter().unwrap();
                assert!(screen.is_alternate());
                // Drop happens here - should restore main screen
            }

            // Verify cleanup by creating new capsule
            let new_screen = AlternateScreenCapsule::new();
            assert!(new_screen.is_ok(), "Should be able to create new capsule after cleanup");
        }
    }

    #[test]
    fn test_cursor_capsule_creation() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let capsule = CursorCapsule::new().unwrap();
            assert!(capsule.is_visible(), "Cursor should start visible");
            assert_eq!(capsule.position(), (0, 0), "Should start at origin");
            assert_eq!(capsule.generation(), 0, "Generation should start at 0");
        }
    }

    #[test]
    fn test_cursor_hide_show() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new().unwrap();

            // Hide cursor
            assert!(cursor.hide().is_ok(), "Failed to hide cursor");
            assert!(!cursor.is_visible(), "Cursor should be hidden");
            assert_eq!(cursor.generation(), 1);

            // Show cursor
            assert!(cursor.show().is_ok(), "Failed to show cursor");
            assert!(cursor.is_visible(), "Cursor should be visible");
            assert_eq!(cursor.generation(), 2);
        }
    }

    #[test]
    fn test_cursor_movement() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new().unwrap();

            // Move cursor
            assert!(cursor.move_to(10, 5).is_ok(), "Failed to move cursor");
            assert_eq!(cursor.position(), (10, 5), "Position should be updated");
            assert_eq!(cursor.generation(), 1);

            // Move again
            assert!(cursor.move_to(20, 15).is_ok(), "Failed to move cursor");
            assert_eq!(cursor.position(), (20, 15), "Position should be updated");
            assert_eq!(cursor.generation(), 2);
        }
    }

    #[test]
    fn test_cursor_save_restore() {
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = CursorCapsule::new().unwrap();

            // Move to position
            cursor.move_to(10, 5).unwrap();
            assert_eq!(cursor.position(), (10, 5));

            // Save position
            cursor.save_position().unwrap();
            assert_eq!(cursor.saved_position(), (10, 5));

            // Move to new position
            cursor.move_to(20, 15).unwrap();
            assert_eq!(cursor.position(), (20, 15));

            // Restore position
            cursor.restore_position().unwrap();
            assert_eq!(cursor.position(), (10, 5), "Should restore to saved position");
        }
    }

    #[test]
    fn test_screen_and_cursor_composition() {
        // Test using both capsules together
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let screen = AlternateScreenCapsule::new().unwrap();
            let cursor = CursorCapsule::new().unwrap();

            // Enter alternate screen and hide cursor
            screen.enter().unwrap();
            cursor.hide().unwrap();

            assert!(screen.is_alternate());
            assert!(!cursor.is_visible());

            // Move cursor while in alternate screen
            cursor.move_to(10, 5).unwrap();
            assert_eq!(cursor.position(), (10, 5));

            // Restore cursor and leave alternate screen
            cursor.show().unwrap();
            screen.leave().unwrap();

            assert!(!screen.is_alternate());
            assert!(cursor.is_visible());
        }
    }

    #[test]
    fn test_full_tui_lifecycle() {
        // Simulate full TUI lifecycle with all three capsules
        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let raw_mode = RawModeCapsule::new().unwrap();
            let screen = AlternateScreenCapsule::new().unwrap();
            let cursor = CursorCapsule::new().unwrap();

            // Setup TUI
            raw_mode.enable_raw_mode().unwrap();
            screen.enter().unwrap();
            cursor.hide().unwrap();

            assert!(raw_mode.is_raw_mode());
            assert!(screen.is_alternate());
            assert!(!cursor.is_visible());

            // Do some TUI operations
            cursor.move_to(5, 3).unwrap();
            cursor.save_position().unwrap();
            cursor.move_to(10, 8).unwrap();
            cursor.restore_position().unwrap();

            // Teardown TUI
            cursor.show().unwrap();
            screen.leave().unwrap();
            raw_mode.disable_raw_mode().unwrap();

            assert!(!raw_mode.is_raw_mode());
            assert!(!screen.is_alternate());
            assert!(cursor.is_visible());
        }
    }

    #[test]
    fn test_concurrent_reads_screen() {
        use std::sync::Arc;
        use std::thread;

        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let screen = Arc::new(AlternateScreenCapsule::new().unwrap());
            let mut handles = vec![];

            for _ in 0..4 {
                let screen_clone: Arc<AlternateScreenCapsule> = Arc::clone(&screen);
                let handle = thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = screen_clone.is_alternate();
                        let _ = screen_clone.generation();
                        let _ = screen_clone.fd();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn test_concurrent_reads_cursor() {
        use std::sync::Arc;
        use std::thread;

        if unsafe { libc::isatty(libc::STDOUT_FILENO) } == 1 {
            let cursor = Arc::new(CursorCapsule::new().unwrap());
            let mut handles = vec![];

            for _ in 0..4 {
                let cursor_clone: Arc<CursorCapsule> = Arc::clone(&cursor);
                let handle = thread::spawn(move || {
                    for _ in 0..100 {
                        let _ = cursor_clone.is_visible();
                        let _ = cursor_clone.position();
                        let _ = cursor_clone.saved_position();
                        let _ = cursor_clone.generation();
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        }
    }

    #[test]
    fn test_alignment_and_size_screen() {
        use core::mem::{align_of, size_of};
        assert_eq!(align_of::<AlternateScreenCapsule>(), 64);
        assert_eq!(size_of::<AlternateScreenCapsule>(), 64);
    }

    #[test]
    fn test_alignment_and_size_cursor() {
        use core::mem::{align_of, size_of};
        assert_eq!(align_of::<CursorCapsule>(), 64);
        assert_eq!(size_of::<CursorCapsule>(), 64);
    }
}
