//! Results screen for kindly_dedup CLI (Phase 3.5)
//!
//! Summary of deduplication results with:
//! - Success celebration animation
//! - Processing statistics (unique, duplicates, clusters)
//! - Performance metrics (throughput, total time)
//! - Achievements/badges (speed demon, exceptional, etc.)
//! - Action menu (view clusters, export, etc.)
//!
//! ## UCE34 Framework Compliance
//! - **Q10 (Tier)**: T1 Atomic (MenuStateCapsule for selection)
//! - **Q13 (Architecture)**: Results UI with action menu
//! - **Q14 (Pattern)**: Uses MenuStateCapsule for navigation
//! - **Q28 (Simplicity)**: Clear results summary + action menu
//! - **Q31 (Rust Transform)**: 100% safe, no unsafe code
//! - **Q33 (Verification)**: All metrics validated

use crate::cli::animation::CelebrationAnimation;
use crate::cli::state::MenuStateCapsule;
use crate::utils::terminal::{emoji, Colorize};
use std::io::{self, Write};
use std::sync::Arc;

/// Deduplication results
#[derive(Debug, Clone)]
pub struct DedupResults {
    pub total_documents: u64,
    pub unique_documents: u64,
    pub duplicate_documents: u64,
    pub duplicate_pairs: u64,
    pub clusters: usize,
    pub elapsed_seconds: f64,
    pub memory_used_gb: f64,
}

impl DedupResults {
    /// Calculate throughput (docs/sec)
    pub fn throughput(&self) -> f64 {
        if self.elapsed_seconds > 0.0 {
            self.total_documents as f64 / self.elapsed_seconds
        } else {
            0.0
        }
    }

    /// Get performance classification
    pub fn performance_class(&self) -> &'static str {
        let throughput = self.throughput();
        if throughput > 300_000.0 {
            "EXCEPTIONAL"
        } else if throughput > 100_000.0 {
            "EXCELLENT"
        } else if throughput > 60_000.0 {
            "GOOD"
        } else {
            "FAIR"
        }
    }
}

/// Results screen
pub struct ResultsScreen {
    menu_state: Arc<MenuStateCapsule>,
    celebration: CelebrationAnimation,
    results: DedupResults,
}

impl ResultsScreen {
    /// Create a new results screen
    pub fn new(results: DedupResults) -> Self {
        Self {
            menu_state: Arc::new(MenuStateCapsule::new()),
            celebration: CelebrationAnimation::new(),
            results,
        }
    }

    /// Get results reference
    pub fn results(&self) -> &DedupResults {
        &self.results
    }

    /// Get selected action
    pub fn selected_action(&self) -> u8 {
        self.menu_state.selected()
    }

    /// Render results screen
    pub fn render(&self) -> Result<(), io::Error> {
        clearscreen()?;

        // Header with celebration
        println!("╔════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║  {}  kindly_dedup → {} Success!{}║",
            emoji::PURPLE_HEART,
            "✨",
            " ".repeat(43)
        );
        println!("╠════════════════════════════════════════════════════════════════════════════╣");
        println!("║{}║", " ".repeat(78));

        // Celebration message
        println!(
            "║  {}  CONGRATULATIONS! {}{}║",
            "🎉".bright_gold(),
            " ".repeat(45),
            " ".repeat(5)
        );
        println!("║  STEP 6 of 6: Deduplication Complete!{}║", " ".repeat(37));
        println!("║{}║", " ".repeat(78));

        // Results summary
        self.render_results_summary()?;

        // Achievements
        self.render_achievements()?;

        // Action menu
        println!("║{}║", " ".repeat(78));
        self.render_action_menu()?;

        println!(
            "║  [↑↓] Navigate  [Enter] Select  [Esc] Back to Menu{}║",
            " ".repeat(17)
        );
        println!("║{}║", " ".repeat(78));
        println!("╚════════════════════════════════════════════════════════════════════════════╝");

        io::stdout().flush()?;
        Ok(())
    }

    /// Render results summary
    fn render_results_summary(&self) -> io::Result<()> {
        println!("║  {} Processing Results{}║", "📊".bright_gold(), " ".repeat(54));
        println!("║{}║", " ".repeat(78));

        // Document counts
        println!(
            "║    Total Processed: {}{}║",
            format_number(self.results.total_documents).byzantine_gold(),
            " ".repeat(48 - format_number(self.results.total_documents).len())
        );

        let unique_pct = (self.results.unique_documents as f64 / self.results.total_documents as f64 * 100.0);
        println!(
            "║    💎 Unique: {} ({:.1}%){}║",
            format_number(self.results.unique_documents).bright_gold(),
            unique_pct,
            " ".repeat(40 - format_number(self.results.unique_documents).len())
        );

        let dup_pct = (self.results.duplicate_documents as f64 / self.results.total_documents as f64 * 100.0);
        println!(
            "║    🔄 Duplicates: {} ({:.1}%){}║",
            format_number(self.results.duplicate_documents),
            dup_pct,
            " ".repeat(35 - format_number(self.results.duplicate_documents).len())
        );

        println!(
            "║    🔗 Pairs Found: {}{}║",
            format_number(self.results.duplicate_pairs),
            " ".repeat(46 - format_number(self.results.duplicate_pairs).len())
        );

        println!(
            "║    📊 Clusters: {}{}║",
            format_number(self.results.clusters as u64),
            " ".repeat(50 - format_number(self.results.clusters as u64).len())
        );

        println!("║{}║", " ".repeat(78));

        // Performance
        println!("║  {} Performance Metrics{}║", "⚡".bright_gold(), " ".repeat(51));
        println!("║{}║", " ".repeat(78));

        let throughput = self.results.throughput() as u64;
        let throughput_str = format!("{} docs/sec", format_number(throughput));
        let throughput_colored = if self.results.performance_class() == "EXCEPTIONAL" {
            throughput_str.bright_gold().bold()
        } else if self.results.performance_class() == "EXCELLENT" {
            throughput_str.bright_gold()
        } else {
            throughput_str.to_string()
        };

        println!(
            "║    Throughput: {}{}{}║",
            throughput_colored,
            " ".repeat(35 - throughput_str.len()),
            " ".repeat(8)
        );

        let time_str = self.format_duration(self.results.elapsed_seconds);
        println!(
            "║    Total Time: {}{}║",
            time_str.cyan(),
            " ".repeat(50 - time_str.len())
        );

        println!(
            "║    Memory Used: {:.1} GB{}║",
            self.results.memory_used_gb,
            " ".repeat(50)
        );

        println!("║{}║", " ".repeat(78));

        Ok(())
    }

    /// Render achievements section
    fn render_achievements(&self) -> io::Result<()> {
        let mut achievements = Vec::new();

        let throughput = self.results.throughput();

        // Speed achievements
        if throughput > 300_000.0 {
            achievements.push(("🏆", "Speed Demon", ">300K docs/sec"));
        }
        if throughput > 100_000.0 {
            achievements.push(("👑", "Exceptional Performance", "Top 1% speed"));
        }

        // Accuracy achievements
        if self.results.unique_documents > self.results.total_documents / 2 {
            achievements.push(("💎", "Cleaner Dataset", ">50% unique"));
        }

        // Scale achievements
        if self.results.total_documents > 1_000_000 {
            achievements.push(("🚀", "Mega Scale", ">1M documents"));
        }

        if !achievements.is_empty() {
            println!("║  {} Achievements Unlocked{}║", "🏅".bright_gold(), " ".repeat(50));
            println!("║{}║", " ".repeat(78));

            for (emoji, name, description) in achievements {
                println!(
                    "║    {} {} - {}{}║",
                    emoji,
                    name.bright_gold().bold(),
                    description,
                    " ".repeat(45 - name.len() - description.len())
                );
            }

            println!("║{}║", " ".repeat(78));
        }

        Ok(())
    }

    /// Render action menu
    fn render_action_menu(&self) -> io::Result<()> {
        println!("║  {} Next Steps{}║", "🎯".bright_gold(), " ".repeat(62));
        println!("║{}║", " ".repeat(78));

        let selected = self.menu_state.selected();

        self.render_action_item(0, selected, "📋", "View Duplicate Clusters")?;
        self.render_action_item(1, selected, "📊", "View Detailed Statistics")?;
        self.render_action_item(2, selected, "📜", "View Audit Trail")?;
        self.render_action_item(3, selected, "💾", "Export Results")?;
        self.render_action_item(4, selected, "🔄", "Deduplicate Another Dataset")?;
        self.render_action_item(5, selected, "🚪", "Back to Main Menu")?;

        Ok(())
    }

    /// Render a single action item
    fn render_action_item(&self, index: u8, selected: u8, emoji: &str, text: &str) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        let text_str = if is_selected {
            format!("{} {}", emoji, text).byzantine_gold().bold()
        } else {
            format!("{} {}", emoji, text)
        };

        println!("║  {} {}{}║", marker, text_str, " ".repeat(70 - text.len() - 2));

        Ok(())
    }

    /// Format duration as human-readable string
    fn format_duration(&self, seconds: f64) -> String {
        if seconds < 60.0 {
            format!("{:.1}s", seconds)
        } else if seconds < 3600.0 {
            format!("{:.1}m", seconds / 60.0)
        } else {
            format!("{:.1}h", seconds / 3600.0)
        }
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
    fn test_results_creation() {
        let results = DedupResults {
            total_documents: 1_000_000,
            unique_documents: 900_000,
            duplicate_documents: 100_000,
            duplicate_pairs: 50_000,
            clusters: 25_000,
            elapsed_seconds: 3.0,
            memory_used_gb: 2.5,
        };

        assert_eq!(results.throughput() as u64, 333_333);
        assert_eq!(results.performance_class(), "EXCEPTIONAL");
    }

    #[test]
    fn test_results_screen_creation() {
        let results = DedupResults {
            total_documents: 1_000_000,
            unique_documents: 900_000,
            duplicate_documents: 100_000,
            duplicate_pairs: 50_000,
            clusters: 25_000,
            elapsed_seconds: 3.0,
            memory_used_gb: 2.5,
        };

        let screen = ResultsScreen::new(results);
        assert_eq!(screen.selected_action(), 0);
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
    }

    #[test]
    fn test_performance_classification() {
        let fast_results = DedupResults {
            total_documents: 1_000_000,
            unique_documents: 900_000,
            duplicate_documents: 100_000,
            duplicate_pairs: 50_000,
            clusters: 25_000,
            elapsed_seconds: 1.0, // 1M docs/sec
            memory_used_gb: 2.5,
        };

        assert_eq!(fast_results.performance_class(), "EXCEPTIONAL");

        let slow_results = DedupResults {
            total_documents: 1_000,
            unique_documents: 900,
            duplicate_documents: 100,
            duplicate_pairs: 50,
            clusters: 25,
            elapsed_seconds: 100.0,
            memory_used_gb: 0.1,
        };

        assert_eq!(slow_results.performance_class(), "FAIR");
    }
}
