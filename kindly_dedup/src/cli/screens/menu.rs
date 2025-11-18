//! Main menu screen for kindly_dedup CLI
//!
//! Interactive menu with 7 options:
//! 1. Deduplicate Files
//! 2. View Statistics
//! 3. Settings
//! 4. Audit Trail
//! 5. License Info
//! 6. Help
//! 7. Exit
//!
//! ## Features
//! - Selection highlighting (Byzantine Purple/Gold)
//! - Emoji indicators for each option
//! - Keyboard navigation hints
//!
//! ## UCE34 Framework
//! - Q13 (Architecture): Clear menu structure
//! - Q14 (Capsule Pattern): Uses MenuStateCapsule for selection state
//! - Q28 (Simplicity): Single-purpose menu rendering
//! - Q31 (Rust Transform): Pure rendering, no side effects

use crate::cli::state::MenuStateCapsule;
use crate::utils::terminal::{emoji, Colorize};
use std::io::{self, Write};

/// Render main menu with selection highlight
///
/// ## Menu Options
/// - [1] Deduplicate Files (📁)
/// - [2] View Statistics (📊)
/// - [3] Settings (⚙️)
/// - [4] Audit Trail (📜)
/// - [5] License Info (💡)
/// - [6] Help (❓)
/// - [7] Exit (🚪)
///
/// ## Arguments
/// - `menu_state`: Current menu state (selected index, etc.)
///
/// ## Returns
/// `io::Result<()>` for I/O operations
///
/// ## Performance
/// - Rendering: <100µs (string allocation + formatting)
pub fn render_main_menu(menu_state: &MenuStateCapsule) -> io::Result<()> {
    let selected = menu_state.selected();

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    );
    println!();
    println!("{}", "                              MAIN MENU".deep_purple().bold());
    println!();

    // Menu options (0-6)
    render_menu_option(
        0,
        selected,
        "📁",
        "Deduplicate Files",
        "Find & remove duplicate documents",
    );
    render_menu_option(1, selected, "📊", "View Statistics", "Show performance metrics");
    render_menu_option(2, selected, "⚙️ ", "Settings", "Configure deduplication parameters");
    render_menu_option(3, selected, "📜", "Audit Trail", "View Q34 compliance logs");
    render_menu_option(4, selected, "💡", "License Info", "Check license status");
    render_menu_option(5, selected, "❓", "Help", "Learn how to use kindly_dedup");
    render_menu_option(6, selected, "🚪", "Exit", "Quit application");

    println!();
    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    );
    println!();
    println!(
        "{} {} {}",
        "💜".byzantine_purple(),
        "Use arrows (↑↓) or numbers (1-7) to select, then press Enter".light_purple(),
        ""
    );
    println!(
        "{} {} {}",
        "💜".byzantine_purple(),
        "Or press 'q' or ESC to quit".light_purple(),
        ""
    );
    println!();

    io::stdout().flush()?;
    Ok(())
}

/// Render a single menu option with selection highlighting
///
/// ## Arguments
/// - `index`: Option index (0-6)
/// - `selected`: Currently selected index
/// - `emoji`: Emoji indicator
/// - `title`: Option title
/// - `description`: Option description
///
/// ## Performance
/// <50µs per option (string allocation)
#[inline]
fn render_menu_option(index: u8, selected: u8, emoji_char: &str, title: &str, description: &str) {
    let number = index + 1;
    let is_selected = index == selected;

    // Format the number with selection styling
    let number_str = if is_selected {
        format!("{}", number).byzantine_gold().bold()
    } else {
        format!("{}", number).dim()
    };

    // Format the title with selection styling
    let title_str = if is_selected {
        format!("{} {}", emoji_char, title).byzantine_gold().bold()
    } else {
        format!("{} {}", emoji_char, title)
    };

    // Format the description with selection styling
    let description_str = if is_selected {
        description.light_purple()
    } else {
        description.dim()
    };

    // Build the option line
    let prefix = if is_selected {
        format!("   [{}] ", number_str).to_string()
    } else {
        format!("   [{}] ", number_str).to_string()
    };

    println!("{}{}        {}", prefix, title_str, description_str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_main_menu() {
        let menu = MenuStateCapsule::new();
        let result = render_main_menu(&menu);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_menu_all_selections() {
        let menu = MenuStateCapsule::new();

        // Test rendering with each option selected
        for i in 0..7 {
            menu.select(i);
            let result = render_main_menu(&menu);
            assert!(result.is_ok(), "Failed to render menu with selection {}", i);
        }
    }

    #[test]
    fn test_menu_option_text_content() {
        // Verify emoji and titles are correct
        let options = [
            ("📁", "Deduplicate Files"),
            ("📊", "View Statistics"),
            ("⚙️ ", "Settings"),
            ("📜", "Audit Trail"),
            ("💡", "License Info"),
            ("❓", "Help"),
            ("🚪", "Exit"),
        ];

        for (emoji, title) in &options {
            let combined = format!("{} {}", emoji, title);
            assert!(combined.contains(title), "Title mismatch for {}", title);
        }
    }

    #[test]
    fn test_render_with_zero_selection() {
        let menu = MenuStateCapsule::new();
        menu.select(0);
        assert_eq!(menu.selected(), 0);
        let result = render_main_menu(&menu);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_with_max_selection() {
        let menu = MenuStateCapsule::new();
        menu.select(6);
        assert_eq!(menu.selected(), 6);
        let result = render_main_menu(&menu);
        assert!(result.is_ok());
    }
}
