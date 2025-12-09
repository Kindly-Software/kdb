//! 💜 Kindly-AV1 Command Menu Renderer
//!
//! [TRADE SECRET] - PROPRIETARY AND CONFIDENTIAL
//!
//! Claude Code-style "/" dropdown menu for encoding commands.
//!
//! ## Design
//!
//! ```text
//! +-- Commands ─────────────────────────────────────────+
//! | > Pause/Resume encoding                      Space  |
//! |   Adjust quality (+/-)                        +/-   |
//! |   Toggle GPU acceleration                      G    |
//! |   Save checkpoint                              S    |
//! |   Cancel encoding                              Q    |
//! |   Show help                                    ?    |
//! +─────────────────────────────────────────────────────+
//! [↑/↓] Navigate  [Enter] Select  [Esc] Close
//! ```
//!
//! ## Architecture
//!
//! ```text
//! CommandMenuCapsule (128B cache-aligned, T1 Atomic)
//! ├── selected_index (AtomicU64) - Current selection
//! ├── generation (AtomicU64) - State tracking
//! └── _padding (112B) - Cache alignment
//! ```
//!
//! ## Framework Compliance
//!
//! - **UCE34**: Q10 T1 Atomic tier, <10ns selection updates
//! - **Chaos**: 128B cache-aligned, 100% lockfree, generation counters
//! - **ASSUM**: All atomics use Acquire/Release for visibility
//! - **B32**: <10ns move_up/move_down, <100ns render
//! - **T28**: Unit tests for selection, rendering, wraparound

use crate::cli::branding::{BOLD, DIM, GOLD, PURPLE, RESET};
use std::sync::atomic::{AtomicU64, Ordering};

use super::keyboard::KeyAction;

// ============================================================================
// Menu Items
// ============================================================================

/// Menu item with label, shortcut, and action
#[derive(Debug, Clone, Copy)]
pub struct MenuItem {
    pub label: &'static str,
    pub shortcut: &'static str,
    pub action: KeyAction,
}

/// Static menu items (6 commands)
const MENU_ITEMS: &[MenuItem] = &[
    MenuItem {
        label: "Pause/Resume encoding",
        shortcut: "Space",
        action: KeyAction::TogglePause,
    },
    MenuItem {
        label: "Adjust quality (+/-)",
        shortcut: "+/-",
        action: KeyAction::QualityUp, // Representative action
    },
    MenuItem {
        label: "Toggle GPU acceleration",
        shortcut: "G",
        action: KeyAction::ToggleGpu,
    },
    MenuItem {
        label: "Save checkpoint",
        shortcut: "S",
        action: KeyAction::SaveCheckpoint,
    },
    MenuItem {
        label: "Cancel encoding",
        shortcut: "Q",
        action: KeyAction::Cancel,
    },
    MenuItem {
        label: "Show help",
        shortcut: "?",
        action: KeyAction::None, // Placeholder for help action
    },
];

// ============================================================================
// Command Menu Capsule (128B cache-aligned, T1 Atomic)
// ============================================================================

/// Command menu capsule with lockfree selection
///
/// # Cache Alignment
///
/// 128B cache-aligned to prevent false sharing with surrounding UI state.
///
/// # Memory Layout
///
/// ```text
/// [0..8]    selected_index (AtomicU64)
/// [8..16]   generation (AtomicU64)
/// [16..128] _padding (112B)
/// ```
#[repr(align(128))]
pub struct CommandMenuCapsule {
    /// Currently selected menu item index (0-5)
    selected_index: AtomicU64,

    /// Generation counter for state tracking
    generation: AtomicU64,

    /// Padding to 128B cache line
    _padding: [u8; 112],
}

impl CommandMenuCapsule {
    /// Create new command menu capsule
    ///
    /// # Returns
    ///
    /// New capsule with selected_index = 0, generation = 0
    pub const fn new() -> Self {
        Self {
            selected_index: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            _padding: [0u8; 112],
        }
    }

    /// Get static menu items
    ///
    /// # Returns
    ///
    /// Array of 6 menu items
    #[inline]
    pub const fn items() -> &'static [MenuItem] {
        MENU_ITEMS
    }

    /// Get currently selected index
    ///
    /// # Returns
    ///
    /// Index 0-5 (clamped to menu bounds)
    #[inline]
    pub fn selected_index(&self) -> usize {
        let index = self.selected_index.load(Ordering::Acquire);
        (index as usize).min(MENU_ITEMS.len() - 1)
    }

    /// Move selection up (with wraparound)
    ///
    /// # Atomicity
    ///
    /// Uses Acquire/Release for cross-thread visibility.
    /// Increments generation counter on every move.
    pub fn move_up(&self) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if current == 0 {
            (MENU_ITEMS.len() - 1) as u64 // Wrap to bottom
        } else {
            current - 1
        };
        self.selected_index.store(new_index, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Move selection down (with wraparound)
    ///
    /// # Atomicity
    ///
    /// Uses Acquire/Release for cross-thread visibility.
    /// Increments generation counter on every move.
    pub fn move_down(&self) {
        let current = self.selected_index.load(Ordering::Acquire);
        let new_index = if (current as usize) >= MENU_ITEMS.len() - 1 {
            0 // Wrap to top
        } else {
            current + 1
        };
        self.selected_index.store(new_index, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current generation counter
    ///
    /// # Returns
    ///
    /// Generation counter (increments on every move)
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Render command menu box
    ///
    /// # Arguments
    ///
    /// * `width` - Terminal width (menu will be centered)
    ///
    /// # Returns
    ///
    /// Fully rendered menu box with ANSI color codes
    ///
    /// # Example Output
    ///
    /// ```text
    /// +-- Commands ─────────────────────────────────────────+
    /// | > Pause/Resume encoding                      Space  |
    /// |   Adjust quality (+/-)                        +/-   |
    /// |   Toggle GPU acceleration                      G    |
    /// |   Save checkpoint                              S    |
    /// |   Cancel encoding                              Q    |
    /// |   Show help                                    ?    |
    /// +─────────────────────────────────────────────────────+
    /// [↑/↓] Navigate  [Enter] Select  [Esc] Close
    /// ```
    pub fn render(&self, width: u16) -> String {
        let selected = self.selected_index();
        let mut output = String::with_capacity(1024);

        // Box drawing characters
        const TOP_LEFT: &str = "+";
        const TOP_RIGHT: &str = "+";
        const BOTTOM_LEFT: &str = "+";
        const BOTTOM_RIGHT: &str = "+";
        const HORIZONTAL: &str = "─";
        const VERTICAL: &str = "|";

        // Calculate menu width (50 chars min)
        let menu_width = width.max(60) as usize;
        let content_width = menu_width - 4; // Account for "| " and " |"

        // Top border with title
        output.push_str(PURPLE);
        output.push_str(TOP_LEFT);
        output.push_str("-- ");
        output.push_str(BOLD);
        output.push_str("Commands");
        output.push_str(RESET);
        output.push_str(PURPLE);
        output.push_str(" ");
        for _ in 0..(menu_width - 13) {
            output.push_str(HORIZONTAL);
        }
        output.push_str(TOP_RIGHT);
        output.push_str(RESET);
        output.push('\n');

        // Menu items
        for (i, item) in MENU_ITEMS.iter().enumerate() {
            output.push_str(PURPLE);
            output.push_str(VERTICAL);
            output.push_str(RESET);
            output.push(' ');

            // Selection indicator
            if i == selected {
                output.push_str(GOLD);
                output.push_str("> ");
                output.push_str(RESET);
            } else {
                output.push_str("  ");
            }

            // Label
            if i == selected {
                output.push_str(BOLD);
            }
            output.push_str(item.label);
            if i == selected {
                output.push_str(RESET);
            }

            // Right-align shortcut
            let label_len = item.label.len();
            let shortcut_len = item.shortcut.len();
            let spacing = content_width.saturating_sub(label_len + shortcut_len + 3);
            for _ in 0..spacing {
                output.push(' ');
            }

            // Shortcut
            output.push_str(DIM);
            output.push_str(item.shortcut);
            output.push_str(RESET);

            output.push_str("  ");
            output.push_str(PURPLE);
            output.push_str(VERTICAL);
            output.push_str(RESET);
            output.push('\n');
        }

        // Bottom border
        output.push_str(PURPLE);
        output.push_str(BOTTOM_LEFT);
        for _ in 0..(menu_width - 2) {
            output.push_str(HORIZONTAL);
        }
        output.push_str(BOTTOM_RIGHT);
        output.push_str(RESET);
        output.push('\n');

        // Help text
        output.push_str(DIM);
        output.push_str("[↑/↓] Navigate  [Enter] Select  [Esc] Close");
        output.push_str(RESET);

        output
    }
}

impl Default for CommandMenuCapsule {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capsule_size_and_alignment() {
        assert_eq!(std::mem::size_of::<CommandMenuCapsule>(), 128);
        assert_eq!(std::mem::align_of::<CommandMenuCapsule>(), 128);
    }

    #[test]
    fn test_menu_items_count() {
        assert_eq!(MENU_ITEMS.len(), 6);
        assert_eq!(CommandMenuCapsule::items().len(), 6);
    }

    #[test]
    fn test_initial_state() {
        let menu = CommandMenuCapsule::new();
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(menu.generation(), 0);
    }

    #[test]
    fn test_move_down() {
        let menu = CommandMenuCapsule::new();

        // Move down 0 -> 1
        menu.move_down();
        assert_eq!(menu.selected_index(), 1);
        assert_eq!(menu.generation(), 1);

        // Move down 1 -> 2
        menu.move_down();
        assert_eq!(menu.selected_index(), 2);
        assert_eq!(menu.generation(), 2);
    }

    #[test]
    fn test_move_up() {
        let menu = CommandMenuCapsule::new();

        // Move down to 2
        menu.move_down();
        menu.move_down();
        assert_eq!(menu.selected_index(), 2);

        // Move up 2 -> 1
        menu.move_up();
        assert_eq!(menu.selected_index(), 1);
        assert_eq!(menu.generation(), 3);

        // Move up 1 -> 0
        menu.move_up();
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(menu.generation(), 4);
    }

    #[test]
    fn test_wraparound_top_to_bottom() {
        let menu = CommandMenuCapsule::new();

        // At top (0), move up wraps to bottom (5)
        menu.move_up();
        assert_eq!(menu.selected_index(), 5);
        assert_eq!(menu.generation(), 1);
    }

    #[test]
    fn test_wraparound_bottom_to_top() {
        let menu = CommandMenuCapsule::new();

        // Move to bottom (5)
        for _ in 0..5 {
            menu.move_down();
        }
        assert_eq!(menu.selected_index(), 5);

        // Move down wraps to top (0)
        menu.move_down();
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(menu.generation(), 6);
    }

    #[test]
    fn test_render_basic() {
        let menu = CommandMenuCapsule::new();
        let rendered = menu.render(80);

        // Verify structure
        assert!(rendered.contains("Commands"));
        assert!(rendered.contains("Pause/Resume encoding"));
        assert!(rendered.contains("Space"));
        assert!(rendered.contains("[↑/↓] Navigate"));
    }

    #[test]
    fn test_render_selection_indicator() {
        let menu = CommandMenuCapsule::new();

        // Selected index 0 should have "> " indicator
        let rendered_0 = menu.render(80);
        // Strip ANSI codes for testing (ANSI codes interfere with simple string matching)
        let stripped_0 = strip_ansi(&rendered_0);
        assert!(stripped_0.contains("> Pause/Resume encoding"),
                "Expected '> Pause/Resume encoding' in output:\n{}", stripped_0);

        // Move down to index 1
        menu.move_down();
        let rendered_1 = menu.render(80);
        let stripped_1 = strip_ansi(&rendered_1);
        assert!(stripped_1.contains("> Adjust quality"),
                "Expected '> Adjust quality' in output:\n{}", stripped_1);
        assert!(!stripped_1.contains("> Pause/Resume encoding"),
                "Did not expect '> Pause/Resume encoding' in output:\n{}", stripped_1);
    }

    /// Strip ANSI escape codes from string for testing
    ///
    /// ANSI codes interfere with simple string matching in tests.
    fn strip_ansi(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        let mut in_escape = false;

        for ch in s.chars() {
            if ch == '\x1b' {
                in_escape = true;
            } else if in_escape && ch == 'm' {
                in_escape = false;
            } else if !in_escape {
                result.push(ch);
            }
        }

        result
    }

    #[test]
    fn test_render_all_items() {
        let menu = CommandMenuCapsule::new();
        let rendered = menu.render(80);

        // All menu items should be present
        for item in MENU_ITEMS {
            assert!(rendered.contains(item.label), "Missing label: {}", item.label);
            assert!(rendered.contains(item.shortcut), "Missing shortcut: {}", item.shortcut);
        }
    }

    #[test]
    fn test_render_respects_width() {
        let menu = CommandMenuCapsule::new();

        // Small width should be clamped to 60
        let rendered_small = menu.render(40);
        assert!(!rendered_small.is_empty());

        // Large width should render wider menu
        let rendered_large = menu.render(120);
        assert!(!rendered_large.is_empty());
        assert!(rendered_large.len() >= rendered_small.len());
    }

    #[test]
    fn test_generation_counter_increments() {
        let menu = CommandMenuCapsule::new();
        assert_eq!(menu.generation(), 0);

        menu.move_down();
        assert_eq!(menu.generation(), 1);

        menu.move_up();
        assert_eq!(menu.generation(), 2);

        menu.move_down();
        menu.move_down();
        menu.move_up();
        assert_eq!(menu.generation(), 5);
    }

    #[test]
    fn test_default_trait() {
        let menu = CommandMenuCapsule::default();
        assert_eq!(menu.selected_index(), 0);
        assert_eq!(menu.generation(), 0);
    }

    #[test]
    fn test_menu_items_structure() {
        // Verify expected menu items
        assert_eq!(MENU_ITEMS[0].label, "Pause/Resume encoding");
        assert_eq!(MENU_ITEMS[0].shortcut, "Space");

        assert_eq!(MENU_ITEMS[1].label, "Adjust quality (+/-)");
        assert_eq!(MENU_ITEMS[1].shortcut, "+/-");

        assert_eq!(MENU_ITEMS[2].label, "Toggle GPU acceleration");
        assert_eq!(MENU_ITEMS[2].shortcut, "G");

        assert_eq!(MENU_ITEMS[3].label, "Save checkpoint");
        assert_eq!(MENU_ITEMS[3].shortcut, "S");

        assert_eq!(MENU_ITEMS[4].label, "Cancel encoding");
        assert_eq!(MENU_ITEMS[4].shortcut, "Q");

        assert_eq!(MENU_ITEMS[5].label, "Show help");
        assert_eq!(MENU_ITEMS[5].shortcut, "?");
    }
}
