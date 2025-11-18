//! Confirmation screen for kindly_dedup CLI (Phase 3.3)
//!
//! Review summary before processing:
//! - Input file details (path, size, document count)
//! - Output directory
//! - Settings (threshold, threads, memory, features)
//! - Estimated performance metrics
//! - Action buttons (Start/Edit/Cancel)
//!
//! ## UCE34 Framework Compliance
//! - **Q10 (Tier)**: T1 Atomic (MenuStateCapsule for selection)
//! - **Q13 (Architecture)**: Confirmation UI with action selection
//! - **Q14 (Pattern)**: Uses MenuStateCapsule for navigation
//! - **Q28 (Simplicity)**: Clear summary + action menu
//! - **Q31 (Rust Transform)**: 100% safe, no unsafe code
//! - **Q33 (Verification)**: Settings validated before processing

use crate::cli::screens::configuration::DedupConfig;
use crate::cli::state::MenuStateCapsule;
use crate::utils::terminal::{emoji, Colorize};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// Action choice from confirmation screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationAction {
    Start = 0,
    EditConfig = 1,
    Cancel = 2,
}

impl ConfirmationAction {
    /// Convert index to action
    pub fn from_index(index: u8) -> Self {
        match index {
            0 => ConfirmationAction::Start,
            1 => ConfirmationAction::EditConfig,
            _ => ConfirmationAction::Cancel,
        }
    }
}

/// Confirmation screen
pub struct ConfirmationScreen {
    menu_state: Arc<MenuStateCapsule>,
    input_file: PathBuf,
    output_dir: PathBuf,
    config: DedupConfig,
    total_documents: u64,
}

impl ConfirmationScreen {
    /// Create a new confirmation screen
    pub fn new(input_file: PathBuf, output_dir: PathBuf, config: DedupConfig, total_documents: u64) -> Self {
        Self {
            menu_state: Arc::new(MenuStateCapsule::new()),
            input_file,
            output_dir,
            config,
            total_documents,
        }
    }

    /// Get selected action
    pub fn selected_action(&self) -> ConfirmationAction {
        ConfirmationAction::from_index(self.menu_state.selected())
    }

    /// Render confirmation screen
    pub fn render(&self) -> Result<(), io::Error> {
        clearscreen()?;

        // Header
        println!("╔════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║  {}  kindly_dedup → {} Confirm Deduplication{}║",
            emoji::PURPLE_HEART,
            "✓ ",
            " ".repeat(42)
        );
        println!("╠════════════════════════════════════════════════════════════════════════════╣");
        println!("║{}║", " ".repeat(78));

        // Input file section
        println!("║  {} Input File{}║", "📂".bright_gold(), " ".repeat(64));
        println!("║{}║", " ".repeat(78));

        let input_str = format!("{}", self.input_file.display());
        let input_display = if input_str.len() > 65 {
            format!("...{}", &input_str[input_str.len() - 62..])
        } else {
            input_str
        };

        println!(
            "║    Path: {}{}║",
            input_display.byzantine_gold(),
            " ".repeat(78 - 10 - input_display.len())
        );

        println!(
            "║    Documents: {}{}║",
            format_number(self.total_documents).byzantine_gold(),
            " ".repeat(56 - format_number(self.total_documents).len())
        );
        println!("║{}║", " ".repeat(78));

        // Output directory section
        println!("║  {} Output Directory{}║", "💾".bright_gold(), " ".repeat(55));
        println!("║{}║", " ".repeat(78));

        let output_str = format!("{}", self.output_dir.display());
        let output_display = if output_str.len() > 65 {
            format!("...{}", &output_str[output_str.len() - 62..])
        } else {
            output_str
        };

        println!(
            "║    {}{}║",
            output_display.byzantine_gold(),
            " ".repeat(78 - 5 - output_display.len())
        );
        println!("║{}║", " ".repeat(78));

        // Settings section
        self.render_settings()?;

        // Estimated performance
        self.render_performance_estimate()?;

        // Action buttons
        println!("║{}║", " ".repeat(78));
        self.render_action_buttons()?;

        println!("║  [↑↓] Navigate  [Enter] Select  [Esc] Cancel{}║", " ".repeat(24));
        println!("║{}║", " ".repeat(78));
        println!("╚════════════════════════════════════════════════════════════════════════════╝");

        io::stdout().flush()?;
        Ok(())
    }

    /// Render settings summary
    fn render_settings(&self) -> io::Result<()> {
        println!("║  {} Configuration{}║", "⚙️ ".bright_gold(), " ".repeat(58));
        println!("║{}║", " ".repeat(78));

        println!(
            "║    Threshold: {} | Threads: {} | Memory: {}GB{}║",
            format!("{:.2}", self.config.jaccard_threshold),
            self.config.num_threads,
            self.config.memory_limit_gb,
            " ".repeat(30)
        );

        let features = [
            ("Q34", self.config.enable_q34_audit),
            ("Bloom", self.config.enable_bloom_filter),
            ("SIMD", self.config.enable_simd),
            ("Batch", self.config.enable_batch_lsh),
        ];

        let enabled: Vec<&str> = features
            .iter()
            .filter_map(|(name, enabled)| if *enabled { Some(*name) } else { None })
            .collect();

        if !enabled.is_empty() {
            println!(
                "║    Features: {}{}║",
                enabled.join(", ").byzantine_gold(),
                " ".repeat(65 - enabled.join(", ").len())
            );
        }

        println!("║{}║", " ".repeat(78));
        Ok(())
    }

    /// Render performance estimate
    fn render_performance_estimate(&self) -> io::Result<()> {
        println!("║  {} Estimated Performance{}║", "⚡".bright_gold(), " ".repeat(48));
        println!("║{}║", " ".repeat(78));

        // Conservative estimates based on tier composition
        let throughput_docs_per_sec =
            if self.config.enable_bloom_filter && self.config.enable_simd && self.config.enable_batch_lsh {
                373_000 / (self.config.num_threads.max(1) as u64)  // Single-threaded base
                * std::thread::available_parallelism()
                    .map(|p| p.get() as u64)
                    .unwrap_or(8)
            } else {
                60_000 / (self.config.num_threads.max(1) as u64)
            };

        let estimated_seconds = if throughput_docs_per_sec > 0 {
            (self.total_documents as f64) / (throughput_docs_per_sec as f64)
        } else {
            0.0
        };

        let throughput_display = if throughput_docs_per_sec > 300_000 {
            format!("{} docs/sec", format_number(throughput_docs_per_sec))
                .bright_gold()
                .bold()
        } else if throughput_docs_per_sec > 100_000 {
            format!("{} docs/sec", format_number(throughput_docs_per_sec)).byzantine_gold()
        } else {
            format!("{} docs/sec", format_number(throughput_docs_per_sec)).to_string()
        };

        println!(
            "║    Throughput: {}{}║",
            throughput_display,
            " ".repeat(56 - format_number(throughput_docs_per_sec).len())
        );

        let time_str = if estimated_seconds < 60.0 {
            format!("{:.1}s", estimated_seconds)
        } else {
            format!("{:.1}m", estimated_seconds / 60.0)
        };

        println!(
            "║    Estimated Time: {}{}║",
            time_str.byzantine_gold(),
            " ".repeat(50 - time_str.len())
        );

        println!("║{}║", " ".repeat(78));
        Ok(())
    }

    /// Render action buttons
    fn render_action_buttons(&self) -> io::Result<()> {
        let selected = self.menu_state.selected();

        println!("║  {} Actions{}║", "🎯".bright_gold(), " ".repeat(64));
        println!("║{}║", " ".repeat(78));

        self.render_action_button(
            0,
            selected,
            "🚀",
            "Start Deduplication",
            "Begin processing with configured settings",
        )?;
        self.render_action_button(
            1,
            selected,
            "✏️ ",
            "Edit Configuration",
            "Return to settings screen to make changes",
        )?;
        self.render_action_button(2, selected, "❌", "Cancel", "Abort and return to main menu")?;

        Ok(())
    }

    /// Render a single action button
    fn render_action_button(
        &self,
        index: u8,
        selected: u8,
        emoji_icon: &str,
        title: &str,
        description: &str,
    ) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        let title_str = if is_selected {
            format!("{} {}", emoji_icon, title).byzantine_gold().bold()
        } else {
            format!("{} {}", emoji_icon, title)
        };

        let desc_str = if is_selected {
            description.light_purple()
        } else {
            description.dim()
        };

        println!("║  {} {}{}│  ║", marker, title_str, " ".repeat(50 - title.len() - 2));
        println!("║       {}{}║", desc_str, " ".repeat(68 - description.len()));

        Ok(())
    }
}

/// Format number with thousand separators
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;

    for ch in s.chars().rev() {
        if count > 0 && count % 3 == 0 {
            result.insert(0, ',');
        }
        result.insert(0, ch);
        count += 1;
    }

    result
}

/// Clear the terminal screen
#[inline]
fn clearscreen() -> io::Result<()> {
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
        assert_eq!(format_number(100), "100");
    }

    #[test]
    fn test_confirmation_screen_creation() {
        let screen = ConfirmationScreen::new(
            PathBuf::from("/tmp/test.jsonl"),
            PathBuf::from("/tmp/output"),
            DedupConfig::default(),
            1_000_000,
        );
        assert_eq!(screen.selected_action(), ConfirmationAction::Start);
    }

    #[test]
    fn test_action_selection() {
        let screen = ConfirmationScreen::new(
            PathBuf::from("/tmp/test.jsonl"),
            PathBuf::from("/tmp/output"),
            DedupConfig::default(),
            1_000_000,
        );

        // Default selection (Start)
        assert_eq!(screen.selected_action(), ConfirmationAction::Start);
    }

    #[test]
    fn test_confirmation_action_from_index() {
        assert_eq!(ConfirmationAction::from_index(0), ConfirmationAction::Start);
        assert_eq!(ConfirmationAction::from_index(1), ConfirmationAction::EditConfig);
        assert_eq!(ConfirmationAction::from_index(2), ConfirmationAction::Cancel);
        assert_eq!(ConfirmationAction::from_index(99), ConfirmationAction::Cancel);
    }
}
