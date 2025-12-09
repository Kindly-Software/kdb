//! Demo of ThemeCapsule - Byzantine purple + gold theme
//!
//! Run with: cargo run --example theme_demo --features std

use atomic_capsule::gui::theme::{
    ThemeCapsule, ThemeMode,
    PURPLE_ROYAL, GOLD_BRIGHT, rgba,
};

fn main() {
    println!("=== ThemeCapsule Demo ===\n");

    // Create dark theme
    let mut dark = ThemeCapsule::byzantine_dark();
    println!("Dark Byzantine Theme:");
    println!("  Mode: {:?}", dark.mode());
    println!("  Primary: 0x{:08X}", dark.primary);
    println!("  Accent: 0x{:08X}", dark.accent);
    println!("  Background: 0x{:08X}", dark.background);
    println!("  Generation: {}", dark.generation());

    // Extract RGBA from primary
    let (r, g, b, a) = rgba(dark.primary);
    println!("  Primary RGBA: ({}, {}, {}, {})", r, g, b, a);

    println!("\n");

    // Create light theme
    let light = ThemeCapsule::byzantine_light();
    println!("Light Byzantine Theme:");
    println!("  Mode: {:?}", light.mode());
    println!("  Primary: 0x{:08X}", light.primary);
    println!("  Accent: 0x{:08X}", light.accent);
    println!("  Background: 0x{:08X}", light.background);
    println!("  Generation: {}", light.generation());

    println!("\n");

    // Toggle mode
    println!("Toggling dark theme to light...");
    dark.toggle_mode();
    println!("  New mode: {:?}", dark.mode());
    println!("  New accent: 0x{:08X}", dark.accent);
    println!("  New background: 0x{:08X}", dark.background);
    println!("  Generation: {}", dark.generation());

    println!("\n");

    // Verify constants
    println!("Color Constants:");
    println!("  PURPLE_ROYAL: 0x{:08X}", PURPLE_ROYAL);
    println!("  GOLD_BRIGHT: 0x{:08X}", GOLD_BRIGHT);

    println!("\n=== ThemeCapsule Demo Complete ===");
}
