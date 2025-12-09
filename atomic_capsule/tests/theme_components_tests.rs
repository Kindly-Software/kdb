//! ThemeComponentsCapsule Integration Tests
//!
//! T28 5-tier testing for widget component styling.

#![cfg(all(feature = "terminal-gpu", feature = "tui-terminal"))]

use atomic_capsule::terminal::style::{
    ThemeColorsCapsule, ThemeComponentsCapsule,
    ButtonVariant, InputVariant, PanelVariant,
    ComponentStyle, InputStyle,
};

// ============================================================================
// Q1-Q7: UNIT TESTS
// ============================================================================

#[test]
fn test_size_and_alignment() {
    assert_eq!(core::mem::size_of::<ThemeComponentsCapsule>(), 512);
    assert_eq!(core::mem::align_of::<ThemeComponentsCapsule>(), 64);
}

#[test]
fn test_default_construction() {
    let components = ThemeComponentsCapsule::new();

    // Verify default button styles
    let primary = components.button(ButtonVariant::Primary);
    assert_eq!(primary.bg, 0x6366F1FF); // Indigo
    assert_eq!(primary.padding_h, 2);
    assert_eq!(primary.padding_v, 1);

    let danger = components.button(ButtonVariant::Danger);
    assert_eq!(danger.bg, 0xEF4444FF); // Red
}

#[test]
fn test_from_theme_derivation() {
    let theme = ThemeColorsCapsule::byzantine_dark();
    let components = ThemeComponentsCapsule::from_theme(&theme);

    // Verify colors derived from theme
    let primary_btn = components.button(ButtonVariant::Primary);
    assert_eq!(primary_btn.bg, theme.primary());
    assert_eq!(primary_btn.fg, theme.text_inverse());

    let input = components.input(InputVariant::Default);
    assert_eq!(input.bg, theme.bg_surface());
    assert_eq!(input.fg, theme.text_primary());
}

#[test]
fn test_component_access_performance() {
    let components = ThemeComponentsCapsule::new();

    // All these should be <5ns
    let _primary = components.button(ButtonVariant::Primary);
    let _secondary = components.button(ButtonVariant::Secondary);
    let _ghost = components.button(ButtonVariant::Ghost);
    let _danger = components.button(ButtonVariant::Danger);

    let _default_input = components.input(InputVariant::Default);
    let _error_input = components.input(InputVariant::Error);

    let _default_panel = components.panel(PanelVariant::Default);
    let _elevated_panel = components.panel(PanelVariant::Elevated);

    let _list = components.list_item(false);
    let _list_sel = components.list_item(true);

    let _tab = components.tab(false);
    let _tab_active = components.tab(true);

    let _menu = components.menu_item(false);
    let _menu_hover = components.menu_item(true);
}

#[test]
fn test_generation_counter() {
    let mut components = ThemeComponentsCapsule::new();
    let gen1 = components.generation();
    assert_eq!(gen1, 0);

    // Modify button
    components.set_button(
        ButtonVariant::Primary,
        ComponentStyle::new(0xFF0000FF, 0xFFFFFFFF, 0xFF0000FF),
    );
    let gen2 = components.generation();
    assert_eq!(gen2, 1);

    // Modify input
    components.set_input(
        InputVariant::Default,
        InputStyle::new(0x00FF00FF, 0xFFFFFFFF, 0x00FF00FF, 0x808080FF),
    );
    let gen3 = components.generation();
    assert_eq!(gen3, 2);
}

#[test]
fn test_bulk_theme_application() {
    let theme = ThemeColorsCapsule::byzantine_dark();
    let mut components = ThemeComponentsCapsule::new();
    let gen1 = components.generation();

    // Apply theme (should increment generation once)
    components.apply_theme(&theme);
    let gen2 = components.generation();
    assert_eq!(gen2, gen1 + 1);

    // Verify colors updated
    let btn = components.button(ButtonVariant::Primary);
    assert_eq!(btn.bg, theme.primary());

    let input = components.input(InputVariant::Default);
    assert_eq!(input.bg, theme.bg_surface());
}

#[test]
fn test_modal_settings() {
    let components = ThemeComponentsCapsule::new();

    let backdrop = components.modal_backdrop();
    assert_eq!(backdrop, 0x00000080); // Semi-transparent black

    let shadow_size = components.modal_shadow_size();
    assert_eq!(shadow_size, 2);
}

// ============================================================================
// Q8-Q14: PROPERTY TESTS
// ============================================================================

#[test]
fn test_variant_consistency() {
    let components = ThemeComponentsCapsule::new();

    // All button variants should have same padding
    let primary = components.button(ButtonVariant::Primary);
    let secondary = components.button(ButtonVariant::Secondary);
    assert_eq!(primary.padding_h, secondary.padding_h);
    assert_eq!(primary.padding_v, secondary.padding_v);
}

#[test]
fn test_theme_switch_consistency() {
    let byzantine = ThemeColorsCapsule::byzantine_dark();
    let solarized = ThemeColorsCapsule::solarized_dark();

    let comp1 = ThemeComponentsCapsule::from_theme(&byzantine);
    let comp2 = ThemeComponentsCapsule::from_theme(&solarized);

    // Primary button should match theme primary
    assert_eq!(comp1.button(ButtonVariant::Primary).bg, byzantine.primary());
    assert_eq!(comp2.button(ButtonVariant::Primary).bg, solarized.primary());
}

// ============================================================================
// Q15-Q21: INTEGRATION TESTS
// ============================================================================

#[test]
fn test_theme_components_integration() {
    // Create theme
    let theme = ThemeColorsCapsule::byzantine_dark();

    // Derive components
    let components = ThemeComponentsCapsule::from_theme(&theme);

    // Verify all components use theme colors
    let btn = components.button(ButtonVariant::Primary);
    assert_eq!(btn.bg, theme.primary());
    assert_eq!(btn.fg, theme.text_inverse());

    let panel = components.panel(PanelVariant::Default);
    assert_eq!(panel.bg, theme.bg_surface());
    assert_eq!(panel.border, theme.border_default());

    let tab_active = components.tab(true);
    assert_eq!(tab_active.bg, theme.bg_surface());
    assert_eq!(tab_active.fg, theme.text_primary());
}

#[test]
fn test_custom_override_with_theme() {
    let theme = ThemeColorsCapsule::byzantine_dark();
    let mut components = ThemeComponentsCapsule::from_theme(&theme);

    // Override primary button
    let custom_btn = ComponentStyle::new(0xFF00FFFF, 0x000000FF, 0xFF00FFFF);
    components.set_button(ButtonVariant::Primary, custom_btn);

    // Verify override
    let btn = components.button(ButtonVariant::Primary);
    assert_eq!(btn.bg, 0xFF00FFFF);

    // Other variants should still use theme
    let secondary = components.button(ButtonVariant::Secondary);
    assert_eq!(secondary.bg, theme.secondary());
}

// ============================================================================
// Q22-Q28: PRODUCTION TESTS
// ============================================================================

#[test]
fn test_concurrent_read_access() {
    use std::sync::Arc;
    use std::thread;

    let components = Arc::new(ThemeComponentsCapsule::new());
    let mut handles = vec![];

    // Spawn 8 readers
    for _ in 0..8 {
        let comp = Arc::clone(&components);
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                let _btn = comp.button(ButtonVariant::Primary);
                let _input = comp.input(InputVariant::Default);
                let _panel = comp.panel(PanelVariant::Default);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_theme_hot_reload() {
    // Simulate hot reload scenario
    let mut components = ThemeComponentsCapsule::new();
    let initial_gen = components.generation();

    // Apply Byzantine theme
    let byzantine = ThemeColorsCapsule::byzantine_dark();
    components.apply_theme(&byzantine);
    assert_eq!(components.generation(), initial_gen + 1);

    // Apply Solarized theme
    let solarized = ThemeColorsCapsule::solarized_dark();
    components.apply_theme(&solarized);
    assert_eq!(components.generation(), initial_gen + 2);

    // Verify final colors match Solarized
    let btn = components.button(ButtonVariant::Primary);
    assert_eq!(btn.bg, solarized.primary());
}

#[test]
fn test_all_variants_accessible() {
    let components = ThemeComponentsCapsule::new();

    // Buttons
    let _ = components.button(ButtonVariant::Primary);
    let _ = components.button(ButtonVariant::Secondary);
    let _ = components.button(ButtonVariant::Ghost);
    let _ = components.button(ButtonVariant::Danger);

    // Inputs
    let _ = components.input(InputVariant::Default);
    let _ = components.input(InputVariant::Error);

    // Panels
    let _ = components.panel(PanelVariant::Default);
    let _ = components.panel(PanelVariant::Elevated);

    // List items
    let _ = components.list_item(false);
    let _ = components.list_item(true);

    // Tabs
    let _ = components.tab(false);
    let _ = components.tab(true);

    // Menu items
    let _ = components.menu_item(false);
    let _ = components.menu_item(true);

    // Modal
    let _ = components.modal_backdrop();
    let _ = components.modal_shadow_size();
}
