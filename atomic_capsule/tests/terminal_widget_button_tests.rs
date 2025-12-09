//! Integration tests for ButtonCapsule
//!
//! These tests verify the ButtonCapsule implementation including:
//! - State packing/unpacking
//! - Animation updates
//! - Mouse interaction
//! - Keyboard interaction
//! - Rendering

#![cfg(feature = "terminal-widgets")]

use atomic_capsule::terminal::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use atomic_capsule::terminal::widget::{ButtonCapsule, Constraints, Rect, RenderCommandBuffer, Widget};
use atomic_capsule::terminal::widget::foundation::button::{ButtonState, ButtonStyle, PressState};

#[test]
fn test_button_creation() {
    let btn = ButtonCapsule::new("Test Button");
    assert_eq!(btn.label(), "Test Button");
    assert!(btn.is_enabled());
    assert!(!btn.is_focused());
}

#[test]
fn test_button_styles() {
    let primary = ButtonCapsule::new("Primary").with_style(ButtonStyle::Primary);
    let secondary = ButtonCapsule::new("Secondary").with_style(ButtonStyle::Secondary);
    let danger = ButtonCapsule::new("Danger").with_style(ButtonStyle::Danger);

    // All buttons should be valid
    assert!(primary.is_enabled());
    assert!(secondary.is_enabled());
    assert!(danger.is_enabled());
}

#[test]
fn test_button_state_packing() {
    let state = ButtonState {
        press_state: PressState::Pressed as u8,
        animation_progress: 128,
        ripple_x: 64,
        ripple_y: 192,
        click_count: 5,
    };

    let packed = state.pack();
    let unpacked = ButtonState::unpack(packed);

    assert_eq!(unpacked.press_state, PressState::Pressed as u8);
    assert_eq!(unpacked.animation_progress, 128);
    assert_eq!(unpacked.ripple_x, 64);
    assert_eq!(unpacked.ripple_y, 192);
    assert_eq!(unpacked.click_count, 5);
}

#[test]
fn test_button_animation_update() {
    let btn = ButtonCapsule::new("Animate");

    // Initial state
    let state = btn.state();
    assert_eq!(state.animation_progress, 0);

    // Update by 50ms (50 + 50>>2 = 50 + 12 = 62 units)
    btn.update_animation(50);
    let state = btn.state();
    assert_eq!(state.animation_progress, 62);

    // Update to saturation
    btn.update_animation(200);
    let state = btn.state();
    assert_eq!(state.animation_progress, 256); // Capped at 256
}

#[test]
fn test_button_mouse_interaction() {
    let btn = ButtonCapsule::new("Click Me");
    let bounds = Rect::new(10, 10, 20, 3);

    // Mouse down inside bounds
    let down_event = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 15,
        row: 11,
        modifiers: KeyModifiers::empty(),
    };

    let clicked = btn.handle_mouse(&down_event, bounds);
    assert!(!clicked); // Click not complete yet

    let state = btn.state();
    assert_eq!(state.press_state, PressState::Pressed as u8);

    // Mouse up inside bounds
    let up_event = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 15,
        row: 11,
        modifiers: KeyModifiers::empty(),
    };

    let clicked = btn.handle_mouse(&up_event, bounds);
    assert!(clicked); // Click complete
}

#[test]
fn test_button_keyboard_activation() {
    let btn = ButtonCapsule::new("Press Me");
    btn.set_focused(true);

    let enter_event = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let activated = btn.handle_key(&enter_event);

    assert!(activated);
}

#[test]
fn test_button_measure() {
    let btn = ButtonCapsule::new("Measure");
    let state = ButtonState::default();

    let constraints = Constraints::loose(100, 100);
    let (width, height) = btn.measure(constraints, &state);

    assert!(width >= 7); // "Measure" = 7 chars
    assert!(height >= 1);
}

#[test]
fn test_button_render() {
    let btn = ButtonCapsule::new("Render");
    let state = ButtonState::default();

    let mut cmd = RenderCommandBuffer::new();
    let area = Rect::new(0, 0, 20, 3);

    btn.render(area, &state, &mut cmd);

    assert!(cmd.commands().len() >= 2); // At least rect + text
}

#[test]
fn test_button_generation_counter() {
    let btn = ButtonCapsule::new("Gen");

    let gen1 = btn.generation();

    btn.update_animation(10);

    let gen2 = btn.generation();
    assert_eq!(gen2, gen1 + 1);
}
