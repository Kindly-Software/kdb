//! ThemeComponentsCapsule Demo
//!
//! Demonstrates widget-specific styling with theme derivation.

#![cfg(all(feature = "terminal-gpu", feature = "tui-terminal"))]

use atomic_capsule::terminal::style::{
    ThemeColorsCapsule, ThemeComponentsCapsule,
    ButtonVariant, InputVariant, PanelVariant,
};

fn main() {
    println!("ThemeComponentsCapsule Demo");
    println!("============================\n");

    // Create Byzantine Dark theme
    let theme = ThemeColorsCapsule::byzantine_dark();
    println!("Theme: Byzantine Dark");
    println!("  Primary: #{:08X}", theme.primary());
    println!("  Secondary: #{:08X}", theme.secondary());
    println!("  Error: #{:08X}\n", theme.error());

    // Derive components from theme
    let components = ThemeComponentsCapsule::from_theme(&theme);
    println!("Derived Components:");

    // Button styles
    println!("  Buttons:");
    let primary_btn = components.button(ButtonVariant::Primary);
    println!("    Primary: bg={:08X} fg={:08X} padding={}×{}",
        primary_btn.bg, primary_btn.fg, primary_btn.padding_h, primary_btn.padding_v);

    let danger_btn = components.button(ButtonVariant::Danger);
    println!("    Danger: bg={:08X} fg={:08X}", danger_btn.bg, danger_btn.fg);

    // Input styles
    println!("  Inputs:");
    let default_input = components.input(InputVariant::Default);
    println!("    Default: bg={:08X} border={:08X}",
        default_input.bg, default_input.border);

    let error_input = components.input(InputVariant::Error);
    println!("    Error: bg={:08X} border={:08X}",
        error_input.bg, error_input.border);

    // Panel styles
    println!("  Panels:");
    let panel = components.panel(PanelVariant::Default);
    println!("    Default: bg={:08X} shadow={:08X}", panel.bg, panel.shadow);

    // List items
    println!("  List Items:");
    let list_normal = components.list_item(false);
    let list_selected = components.list_item(true);
    println!("    Normal: bg={:08X}", list_normal.bg);
    println!("    Selected: bg={:08X}", list_selected.bg);

    // Tabs
    println!("  Tabs:");
    let tab_inactive = components.tab(false);
    let tab_active = components.tab(true);
    println!("    Inactive: fg={:08X}", tab_inactive.fg);
    println!("    Active: bg={:08X} indicator={:08X}", tab_active.bg, tab_active.indicator);

    // Verify size and alignment
    println!("\nCapsule Properties:");
    println!("  Size: {} bytes", core::mem::size_of::<ThemeComponentsCapsule>());
    println!("  Alignment: {} bytes", core::mem::align_of::<ThemeComponentsCapsule>());
    println!("  Generation: {}", components.generation());

    println!("\n✓ ThemeComponentsCapsule demo complete!");
}
