//! CheckboxCapsule Integration Tests
//!
//! # T28 Coverage
//! - Q1-Q7: Unit tests (core functionality)
//! - Q8-Q14: Property tests (toggle consistency)
//! - Q15-Q21: Integration tests (full lifecycle)

#![cfg(feature = "std")]

use atomic_capsule::terminal::widget::foundation::{CheckboxCapsule, CheckState};
use atomic_capsule::terminal::widget::Widget;
use atomic_capsule::terminal::event::KeyEvent;
use atomic_capsule::terminal::render::{RenderCommandBuffer, Rect};

// Q1-Q7: Unit Tests

#[test]
fn test_new_checkbox() {
    let cb = CheckboxCapsule::new("Test");
    assert_eq!(cb.is_checked(), false);
    assert_eq!(cb.check_state(), CheckState::Unchecked);
    assert_eq!(cb.is_enabled(), true);
    assert_eq!(cb.is_tristate(), false);
    assert_eq!(cb.toggle_count(), 0);
}

#[test]
fn test_with_checked() {
    let cb = CheckboxCapsule::new("Test").with_checked(true);
    assert_eq!(cb.is_checked(), true);
    assert_eq!(cb.check_state(), CheckState::Checked);
}

#[test]
fn test_toggle_bistate() {
    let cb = CheckboxCapsule::new("Test");

    // Unchecked -> Checked
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Checked);
    assert_eq!(cb.toggle_count(), 1);

    // Checked -> Unchecked
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Unchecked);
    assert_eq!(cb.toggle_count(), 2);
}

#[test]
fn test_toggle_tristate() {
    let cb = CheckboxCapsule::new("Test").with_tristate();

    // Unchecked -> Checked
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Checked);

    // Checked -> Indeterminate
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Indeterminate);

    // Indeterminate -> Unchecked
    cb.toggle();
    assert_eq!(cb.check_state(), CheckState::Unchecked);

    assert_eq!(cb.toggle_count(), 3);
}

#[test]
fn test_set_checked() {
    let cb = CheckboxCapsule::new("Test");

    cb.set_checked(CheckState::Checked);
    assert_eq!(cb.check_state(), CheckState::Checked);

    cb.set_checked(CheckState::Indeterminate);
    assert_eq!(cb.check_state(), CheckState::Indeterminate);

    cb.set_checked(CheckState::Unchecked);
    assert_eq!(cb.check_state(), CheckState::Unchecked);
}

#[test]
fn test_enabled() {
    let cb = CheckboxCapsule::new("Test");
    assert_eq!(cb.is_enabled(), true);

    cb.set_enabled(false);
    assert_eq!(cb.is_enabled(), false);

    // Should not toggle when disabled
    let before = cb.check_state();
    cb.handle_click();
    assert_eq!(cb.check_state(), before);

    cb.set_enabled(true);
    cb.handle_click();
    assert_ne!(cb.check_state(), before);
}

#[test]
fn test_animation_update() {
    let cb = CheckboxCapsule::new("Test");

    // Set to checked (animation should animate to 256)
    cb.set_checked(CheckState::Checked);

    // Update with 50ms delta
    cb.update_animation(50);

    // Animation should be progressing
    // (Exact value depends on implementation, just verify it's called)
}

#[test]
fn test_widget_trait() {
    let cb = CheckboxCapsule::new("Test");

    // Widget trait should be implemented
    assert_eq!(cb.is_focusable(), true);

    // Disabled widgets not focusable
    cb.set_enabled(false);
    assert_eq!(cb.is_focusable(), false);
}

// Q8-Q14: Property Tests

#[cfg(feature = "std")]
#[test]
fn test_property_toggle_consistency() {
    for count in 0u32..100 {
        let cb = CheckboxCapsule::new("Test");

        for _ in 0..count {
            cb.toggle();
        }

        // Toggle count should match
        assert_eq!(cb.toggle_count(), count);

        // Final state should be predictable (bistate)
        let expected = if count % 2 == 0 {
            CheckState::Unchecked
        } else {
            CheckState::Checked
        };
        assert_eq!(cb.check_state(), expected);
    }
}

#[cfg(feature = "std")]
#[test]
fn test_property_tristate_cycle() {
    for count in 0u32..100 {
        let cb = CheckboxCapsule::new("Test").with_tristate();

        for _ in 0..count {
            cb.toggle();
        }

        // Should cycle through 3 states
        let expected = match count % 3 {
            0 => CheckState::Unchecked,
            1 => CheckState::Checked,
            2 => CheckState::Indeterminate,
            _ => unreachable!(),
        };
        assert_eq!(cb.check_state(), expected);
    }
}

#[cfg(feature = "std")]
#[test]
fn test_property_animation_bounds() {
    for delta_ms in &[0u16, 10, 50, 100, 500, 1000] {
        for iterations in 1..100 {
            let cb = CheckboxCapsule::new("Test");
            cb.set_checked(CheckState::Checked);

            for _ in 0..iterations {
                cb.update_animation(*delta_ms);
            }

            // Animation should always be in valid range (just verify no panic)
        }
    }
}

#[cfg(feature = "std")]
#[test]
fn test_property_disabled_no_toggle() {
    for clicks in 1usize..50 {
        let cb = CheckboxCapsule::new("Test");
        cb.set_enabled(false);
        let initial = cb.check_state();

        for _ in 0..clicks {
            cb.handle_click();
        }

        // State should not change when disabled
        assert_eq!(cb.check_state(), initial);
        assert_eq!(cb.toggle_count(), 0);
    }
}

// Q15-Q21: Integration Tests

#[test]
fn test_integration_full_lifecycle() {
    let cb = CheckboxCapsule::new("Accept Terms").with_tristate();

    // Initial state
    assert_eq!(cb.check_state(), CheckState::Unchecked);
    assert_eq!(cb.is_enabled(), true);
    assert_eq!(cb.toggle_count(), 0);

    // User clicks
    cb.handle_click();
    assert_eq!(cb.check_state(), CheckState::Checked);
    assert_eq!(cb.toggle_count(), 1);

    // Animate
    for _ in 0..10 {
        cb.update_animation(10);
    }

    // Another click (tristate)
    cb.handle_click();
    assert_eq!(cb.check_state(), CheckState::Indeterminate);

    // Disable
    cb.set_enabled(false);
    let before = cb.toggle_count();
    cb.handle_click();
    assert_eq!(cb.toggle_count(), before); // No change

    // Re-enable
    cb.set_enabled(true);
    cb.handle_click();
    assert_eq!(cb.toggle_count(), before + 1);
}

#[test]
fn test_integration_keyboard_navigation() {
    let cb = CheckboxCapsule::new("Option");

    // Space to toggle
    let space_event = KeyEvent { code: ' ' as u32, modifiers: 0 };
    assert_eq!(cb.handle_key(&space_event), true);
    assert_eq!(cb.check_state(), CheckState::Checked);

    // Enter to toggle
    let enter_event = KeyEvent { code: 13, modifiers: 0 };
    assert_eq!(cb.handle_key(&enter_event), true);
    assert_eq!(cb.check_state(), CheckState::Unchecked);

    // Other keys ignored
    let other_event = KeyEvent { code: 'a' as u32, modifiers: 0 };
    assert_eq!(cb.handle_key(&other_event), false);
}

#[test]
fn test_integration_render_no_panic() {
    let cb = CheckboxCapsule::new("Test Checkbox");

    let mut cmd = RenderCommandBuffer::new();
    let area = Rect::new(0, 0, 20, 1);

    // Should not panic
    cb.render(area, &mut cmd);

    // Toggle and render again
    cb.toggle();
    cb.render(area, &mut cmd);

    // Tristate
    let cb2 = CheckboxCapsule::new("Tristate").with_tristate();
    cb2.set_checked(CheckState::Indeterminate);
    cb2.render(area, &mut cmd);
}

#[test]
fn test_integration_concurrent_toggles() {
    use std::sync::Arc;
    use std::thread;

    let cb = Arc::new(CheckboxCapsule::new("Concurrent"));

    let mut handles = vec![];

    // Spawn 10 threads, each toggling 100 times
    for _ in 0..10 {
        let cb_clone = Arc::clone(&cb);
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                cb_clone.toggle();
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Should have 1000 toggles total
    assert_eq!(cb.toggle_count(), 1000);

    // Final state should be consistent
    assert_eq!(cb.check_state(), CheckState::Unchecked); // Even number
}
