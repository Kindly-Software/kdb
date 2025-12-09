//! Command Palette Demo
//!
//! Demonstrates the command palette with fuzzy search (Claude Code style).
//!
//! Usage:
//! ```bash
//! cargo run --example command_palette_demo --features nightly-all
//! ```
//!
//! Controls:
//! - Type to filter commands
//! - Enter to execute selected command
//! - Ctrl+C to quit

use clapi_core::tui::palette::{CommandPalette, COMMANDS};

fn main() {
    println!("Command Palette Demo\n");
    println!("Available commands:");
    println!();

    // Show all commands
    for cmd in COMMANDS {
        println!("  /{:<12} - {}", cmd.name, cmd.description);
        println!("             Args: {}", cmd.args);
        println!("             Example: {}", cmd.example);
        println!();
    }

    // Test fuzzy search
    println!("\nFuzzy Search Tests:");
    println!();

    let test_queries = vec!["", "aud", "met", "pro", "cache", "xyz"];

    for query in test_queries {
        let palette = CommandPalette::new();
        let mut palette_mut = palette;
        palette_mut.update_filter(query.to_string());
        let filtered = palette_mut.filtered_commands();

        println!("Query: '{}' → {} matches", query, filtered.len());
        for (i, cmd) in filtered.iter().take(3).enumerate() {
            println!("  {}. {}", i + 1, cmd.name);
        }
        println!();
    }

    // Test navigation
    println!("Navigation Test:");
    let mut palette = CommandPalette::new();
    palette.update_filter("".to_string()); // Show all

    println!("Selected: {:?}", palette.selected_command().map(|c| c.name));
    palette.next();
    println!("After next: {:?}", palette.selected_command().map(|c| c.name));
    palette.next();
    println!("After next: {:?}", palette.selected_command().map(|c| c.name));
    palette.prev();
    println!("After prev: {:?}", palette.selected_command().map(|c| c.name));

    println!("\nDone!");
}
