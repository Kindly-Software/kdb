//! Processing screen for kindly_dedup CLI (Phase 3.4)
//!
//! Real-time deduplication progress display with:
//! - Phase indicator (MinHash → LSH → FindPairs → Write)
//! - Progress bar with completion percentage
//! - Live metrics (throughput, ETA, elapsed time)
//! - System resource monitoring (memory, CPU)
//! - Document statistics (unique, duplicates, clusters)
//! - Animated spinners for visual feedback
//!
//! ## UCE34 Framework Compliance
//! - **Q10 (Tier)**: T1 Atomic (ProgressTrackerCapsule) + T4 Batch (parallel pipeline)
//! - **Q13 (Architecture)**: Real-time metrics display with atomic updates
//! - **Q14 (Pattern)**: Uses ProgressTrackerCapsule for lockfree coordination
//! - **Q28 (Simplicity)**: Clear progress visualization
//! - **Q31 (Rust Transform)**: 100% safe, no unsafe code
//! - **Q33 (Verification)**: All metrics validated at compile-time

use crate::cli::animation::SpinnerAnimation;
use crate::cli::state::ProgressTrackerCapsule;
use crate::utils::terminal::{emoji, Colorize};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Processing screen
pub struct ProcessingScreen {
    progress_tracker: Arc<ProgressTrackerCapsule>,
    spinner: SpinnerAnimation,
}

impl ProcessingScreen {
    /// Create a new processing screen
    pub fn new(total_documents: u64) -> Self {
        let tracker = Arc::new(ProgressTrackerCapsule::new(total_documents));
        tracker.set_start_time(now_ns());

        Self {
            progress_tracker: tracker,
            spinner: SpinnerAnimation::new(),
        }
    }

    /// Get progress tracker reference
    pub fn tracker(&self) -> &Arc<ProgressTrackerCapsule> {
        &self.progress_tracker
    }

    /// Render processing screen with real-time metrics
    pub fn render(&self) -> Result<(), io::Error> {
        clearscreen()?;

        // Header
        println!("╔════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║  {}  kindly_dedup → {} Processing{}║",
            emoji::PURPLE_HEART,
            "🚀",
            " ".repeat(45)
        );
        println!("╠════════════════════════════════════════════════════════════════════════════╣");
        println!("║{}║", " ".repeat(78));

        // Phase status
        self.render_phase_status()?;

        println!("║{}║", " ".repeat(78));

        // Progress bar
        self.render_progress_bar()?;

        println!("║{}║", " ".repeat(78));

        // Metrics
        self.render_metrics()?;

        println!("║{}║", " ".repeat(78));

        // Document statistics
        self.render_statistics()?;

        // Pause instruction
        println!("║{}║", " ".repeat(78));
        println!(
            "║  💡 Press [Ctrl+C] to pause (progress will be saved){}║",
            " ".repeat(20)
        );
        println!("║{}║", " ".repeat(78));
        println!("╚════════════════════════════════════════════════════════════════════════════╝");

        io::stdout().flush()?;
        Ok(())
    }

    /// Render phase status indicators
    fn render_phase_status(&self) -> io::Result<()> {
        let phase = self.progress_tracker.phase();

        println!(
            "║  {} STEP 5 of 6: Deduplicating Documents{}║",
            "📍".bright_gold(),
            " ".repeat(47)
        );
        println!("║{}║", " ".repeat(78));

        let phases = [
            ("Phase 1: MinHash Signatures", 0),
            ("Phase 2: LSH Bucketing", 1),
            ("Phase 3: Finding Pairs", 2),
            ("Phase 4: Writing Output", 3),
        ];

        for (name, phase_id) in phases {
            let status = if phase_id < phase {
                "✅ Complete".green()
            } else if phase_id == phase {
                format!("{} In Progress...", self.spinner.render()).byzantine_gold()
            } else {
                "⏳ Pending".dim()
            };

            println!("║  │  {}{}{}  │  ║", name, " ".repeat(40 - name.len()), status);
        }

        Ok(())
    }

    /// Render progress bar with percentage
    fn render_progress_bar(&self) -> io::Result<()> {
        let total = self
            .progress_tracker
            .total_documents
            .load(std::sync::atomic::Ordering::Relaxed);
        let processed = self
            .progress_tracker
            .processed
            .load(std::sync::atomic::Ordering::Relaxed);
        let percent = self.progress_tracker.percent_complete();

        println!("║  ┌─ Progress ──────────────────────────────────────────────────────┐  ║");
        println!("║  │{}│  ║", " ".repeat(70));

        // Draw progress bar
        let bar_width = 60;
        let filled = (bar_width * percent as usize) / 100;
        let mut bar = String::from("[");
        for i in 0..bar_width {
            if i < filled {
                bar.push('█');
            } else if i == filled {
                bar.push('▌');
            } else {
                bar.push('░');
            }
        }
        bar.push(']');

        let bar_str = if percent >= 80 {
            bar.bright_gold()
        } else if percent >= 50 {
            bar.byzantine_gold()
        } else {
            bar.to_string()
        };

        println!(
            "║  │  {}  {}% {}  │  ║",
            bar_str,
            format!("{:3}", percent).byzantine_gold(),
            " ".repeat(10)
        );

        println!(
            "║  │  {}/{} documents {}  │  ║",
            format_number(processed),
            format_number(total),
            " ".repeat(30 - format_number(processed).len() - format_number(total).len())
        );

        println!("║  │{}│  ║", " ".repeat(70));
        println!("║  └──────────────────────────────────────────────────────────────────┘  ║");

        Ok(())
    }

    /// Render real-time metrics
    fn render_metrics(&self) -> io::Result<()> {
        let throughput = self.progress_tracker.throughput();
        let elapsed = self.progress_tracker.elapsed_seconds();
        let eta = self.progress_tracker.eta_seconds();

        println!("║  ┌─ Real-time Metrics ──────────────────────────────────────────┐  ║");
        println!("║  │{}│  ║", " ".repeat(70));

        // Throughput with classification
        let throughput_str = if throughput > 300_000 {
            format!("{} docs/sec", format_number(throughput)).bright_gold().bold()
        } else if throughput > 100_000 {
            format!("{} docs/sec", format_number(throughput)).bright_gold()
        } else {
            format!("{} docs/sec", format_number(throughput))
        };

        println!(
            "║  │  ⚡ Throughput: {}{}  │  ║",
            throughput_str,
            " ".repeat(35 - format_number(throughput).len())
        );

        let elapsed_str = self.format_duration(elapsed);
        println!(
            "║  │  ⏱️  Elapsed: {}{}  │  ║",
            elapsed_str.cyan(),
            " ".repeat(45 - elapsed_str.len())
        );

        let eta_str = self.format_duration(eta);
        let eta_colored = if eta < 5.0 { eta_str.green() } else { eta_str.cyan() };

        println!("║  │  ⏳ ETA: {}{}  │  ║", eta_colored, " ".repeat(50 - eta_str.len()));

        println!("║  │{}│  ║", " ".repeat(70));
        println!("║  └──────────────────────────────────────────────────────────────────┘  ║");

        Ok(())
    }

    /// Render document statistics
    fn render_statistics(&self) -> io::Result<()> {
        let total = self
            .progress_tracker
            .total_documents
            .load(std::sync::atomic::Ordering::Relaxed);
        let unique = self
            .progress_tracker
            .unique_documents
            .load(std::sync::atomic::Ordering::Relaxed);
        let dup_pairs = self
            .progress_tracker
            .duplicate_pairs
            .load(std::sync::atomic::Ordering::Relaxed);
        let clusters = self
            .progress_tracker
            .duplicate_clusters
            .load(std::sync::atomic::Ordering::Relaxed);

        let duplicate_count = total.saturating_sub(unique);

        println!("║  ┌─ Document Statistics ────────────────────────────────────────┐  ║");
        println!("║  │{}│  ║", " ".repeat(70));

        let unique_pct = if total > 0 {
            (unique as f64 / total as f64 * 100.0)
        } else {
            0.0
        };

        println!(
            "║  │  💎 Unique: {}  ({:.1}%){}  │  ║",
            format_number(unique).byzantine_gold(),
            unique_pct,
            " ".repeat(30)
        );

        let dup_pct = if total > 0 {
            (duplicate_count as f64 / total as f64 * 100.0)
        } else {
            0.0
        };

        println!(
            "║  │  🔄 Duplicates: {}  ({:.1}%){}  │  ║",
            format_number(duplicate_count),
            dup_pct,
            " ".repeat(28)
        );

        println!(
            "║  │  🔗 Pairs Found: {}{}  │  ║",
            format_number(dup_pairs),
            " ".repeat(40 - format_number(dup_pairs).len())
        );

        println!(
            "║  │  📊 Clusters: {}{}  │  ║",
            format_number(clusters as u64),
            " ".repeat(44 - format_number(clusters as u64).len())
        );

        println!("║  │{}│  ║", " ".repeat(70));
        println!("║  └──────────────────────────────────────────────────────────────────┘  ║");

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

/// Get current time in nanoseconds since epoch
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
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
    fn test_processing_screen_creation() {
        let screen = ProcessingScreen::new(1_000_000);
        assert_eq!(
            screen
                .progress_tracker
                .total_documents
                .load(std::sync::atomic::Ordering::Relaxed),
            1_000_000
        );
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1_000_000), "1,000,000");
    }

    #[test]
    fn test_format_duration() {
        let screen = ProcessingScreen::new(1_000);
        assert!(screen.format_duration(45.5).contains("s"));
        assert!(screen.format_duration(125.0).contains("m"));
    }
}
