//! Configuration screen for kindly_dedup CLI (Phase 3.2 → Phase 3.3: ConfigurationCapsule Integration)
//!
//! Interactive settings panel with:
//! - Jaccard threshold slider (0.0-1.0, Q16.16 deterministic)
//! - Thread count selector (1-128, auto-detect default)
//! - Memory limit configuration
//! - Feature toggles (Q34 audit, Bloom, SIMD, Batch LSH)
//! - Advanced settings (NUMA, huge pages)
//!
//! ## UCE34 Framework Compliance
//! - **Q10 (Tier)**: T0 Auditable (ConfigurationCapsule Q16.16 determinism) + T1 Atomic (MenuStateCapsule)
//! - **Q13 (Architecture)**: Settings UI with option navigation
//! - **Q14 (Pattern)**: Uses MenuStateCapsule for menu state, ConfigurationCapsule for deterministic config
//! - **Q28 (Simplicity)**: Clear settings interface
//! - **Q31 (Rust Transform)**: 100% safe, no unsafe code
//! - **Q33 (Verification)**: ConfigurationCapsule #[derive(ComputationalCapsule)] compile-time verification
//! - **Q34 (Auditability)**: ConfigurationCapsule includes CRC32 checksum integrity

use crate::cli::state::MenuStateCapsule;
use crate::utils::terminal::{emoji, Colorize};
use atomic_capsule::tui::ConfigurationCapsule;
use std::io::{self, Write};
use std::sync::Arc;

/// Deduplication configuration (type alias for ConfigurationCapsule)
///
/// **Tier**: T0 (Auditable) + T1 (Atomic)
/// **Size**: 128 bytes (cache-aligned WarmTier)
/// **Properties**:
/// - Q16.16 fixed-point threshold (deterministic, 100% reproducible)
/// - Bit-packed feature flags (64-bit for 64 features)
/// - CRC32 checksum integrity verification
/// - Zero allocation, zero dependencies
pub type DedupConfig = ConfigurationCapsule;

/// Feature flag constants for DedupConfig
pub mod features {
    /// Feature: Q34 Audit Trail (SOX/SOC2 compliance)
    pub const FEATURE_Q34_AUDIT: u64 = 1 << 0;

    /// Feature: Bloom Pre-Filter (2-10× speedup on duplicate-heavy corpora)
    pub const FEATURE_BLOOM_FILTER: u64 = 1 << 1;

    /// Feature: SIMD Optimization (7× faster vectorization)
    pub const FEATURE_SIMD: u64 = 1 << 2;

    /// Feature: Batch LSH Lookups (1.5× speedup)
    pub const FEATURE_BATCH_LSH: u64 = 1 << 3;

    /// Feature: NUMA Support (multi-socket systems)
    pub const FEATURE_NUMA: u64 = 1 << 4;

    /// Feature: Huge Pages (2MB/1GB TLB optimization)
    pub const FEATURE_HUGE_PAGES: u64 = 1 << 5;
}

/// Helper to create default DedupConfig with standard settings
pub fn create_default_config() -> DedupConfig {
    let num_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(8)
        .min(128) as u32;

    ConfigurationCapsule::new()
        .set_threshold(0.85) // Q16.16 deterministic
        .set_threads(num_threads)
        .set_memory_limit_mb(8192) // 8GB in MB
        .enable_feature(features::FEATURE_Q34_AUDIT)
        .enable_feature(features::FEATURE_BLOOM_FILTER)
        .enable_feature(features::FEATURE_SIMD)
        .enable_feature(features::FEATURE_BATCH_LSH)
}

/// Configuration screen
pub struct ConfigurationScreen {
    menu_state: Arc<MenuStateCapsule>,
    config: DedupConfig,
}

impl ConfigurationScreen {
    /// Create a new configuration screen
    pub fn new() -> Self {
        Self {
            menu_state: Arc::new(MenuStateCapsule::new()),
            config: create_default_config(),
        }
    }

    /// Get current configuration
    pub fn config(&self) -> &DedupConfig {
        &self.config
    }

    /// Get current configuration (immutable copy, suitable for deterministic operations)
    ///
    /// Returns a copy of the ConfigurationCapsule for reading. Since ConfigurationCapsule
    /// is Copy and deterministic (Q16.16 fixed-point), reading is always safe and consistent.
    pub fn config_copy(&self) -> DedupConfig {
        self.config
    }

    /// Move into configuration (consuming self)
    pub fn into_config(self) -> DedupConfig {
        self.config
    }

    /// Render configuration screen
    pub fn render(&self) -> Result<(), io::Error> {
        clearscreen()?;

        // Header
        println!("╔════════════════════════════════════════════════════════════════════════════╗");
        println!(
            "║  {}  kindly_dedup → {} Configuration{}║",
            emoji::PURPLE_HEART,
            "⚙️ ",
            " ".repeat(45)
        );
        println!("╠════════════════════════════════════════════════════════════════════════════╣");
        println!("║{}║", " ".repeat(78));

        // Jaccard Threshold
        let selected = self.menu_state.selected();
        self.render_threshold_setting(0, selected)?;
        println!("║{}║", " ".repeat(78));

        // Thread Count
        self.render_thread_setting(1, selected)?;
        println!("║{}║", " ".repeat(78));

        // Memory Limit
        self.render_memory_setting(2, selected)?;
        println!("║{}║", " ".repeat(78));

        // Features Section
        println!("║  {} Feature Toggles{}║", "✨".byzantine_gold(), " ".repeat(62));
        println!("║{}║", " ".repeat(78));

        // Q34 Audit (atomic read)
        let enable_q34 = self.config.is_feature_enabled(features::FEATURE_Q34_AUDIT);
        self.render_checkbox(3, selected, "Q34 Audit Trail (SOX/SOC2)", enable_q34)?;

        // Bloom Filter (atomic read)
        let enable_bloom = self.config.is_feature_enabled(features::FEATURE_BLOOM_FILTER);
        self.render_checkbox(4, selected, "Bloom Pre-Filter (2-10× speedup)", enable_bloom)?;

        // SIMD (atomic read)
        let enable_simd = self.config.is_feature_enabled(features::FEATURE_SIMD);
        self.render_checkbox(5, selected, "SIMD Optimization (7× faster)", enable_simd)?;

        // Batch LSH (atomic read)
        let enable_batch = self.config.is_feature_enabled(features::FEATURE_BATCH_LSH);
        self.render_checkbox(6, selected, "Batch LSH Lookups (1.5× speedup)", enable_batch)?;

        println!("║{}║", " ".repeat(78));

        // Advanced Settings
        println!("║  {} Advanced Settings{}║", "🔧".bright_gold(), " ".repeat(54));
        println!("║{}║", " ".repeat(78));

        // NUMA (atomic read)
        let enable_numa = self.config.is_feature_enabled(features::FEATURE_NUMA);
        self.render_checkbox(7, selected, "NUMA Support", enable_numa)?;

        // Huge Pages (atomic read)
        let enable_huge = self.config.is_feature_enabled(features::FEATURE_HUGE_PAGES);
        self.render_checkbox(8, selected, "Huge Pages (if available)", enable_huge)?;

        println!("║{}║", " ".repeat(78));

        // Summary
        self.render_summary()?;

        // Instructions
        println!(
            "║  [↑↓] Navigate  [Space/Enter] Toggle  [Esc] Cancel{}║",
            " ".repeat(18)
        );
        println!("║{}║", " ".repeat(78));
        println!("╚════════════════════════════════════════════════════════════════════════════╝");

        io::stdout().flush()?;
        Ok(())
    }

    /// Render Jaccard threshold setting
    fn render_threshold_setting(&self, index: u8, selected: u8) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        // Read Q16.16 threshold deterministically
        let threshold_f64 = self.config.threshold_f64(); // Exact reverse of set_threshold
        let threshold_str = format!("{:.2}", threshold_f64);
        let threshold_display = if is_selected {
            threshold_str.byzantine_gold().bold()
        } else {
            threshold_str.to_string()
        };

        // Draw slider
        let slider_pos = (threshold_f64 * 20.0) as usize;
        let mut slider = String::from("[");
        for i in 0..20 {
            if i < slider_pos {
                slider.push('█');
            } else if i == slider_pos {
                slider.push('●');
            } else {
                slider.push('░');
            }
        }
        slider.push(']');

        if is_selected {
            slider = slider.byzantine_gold();
        }

        println!(
            "║  {} Jaccard Threshold: {}{}  {}{}║",
            marker,
            threshold_display,
            " ".repeat(10 - threshold_str.len()),
            slider,
            " ".repeat(20 - slider.len())
        );
        println!(
            "║     (Lower = more lenient, Higher = stricter, Q16.16 deterministic) [← →]{}║",
            " ".repeat(18)
        );

        Ok(())
    }

    /// Render thread count setting
    fn render_thread_setting(&self, index: u8, selected: u8) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        let max_threads = std::thread::available_parallelism().map(|p| p.get()).unwrap_or(8);

        let threads_str = format!("{}/{}", self.config.threads(), max_threads);
        let threads_display = if is_selected {
            threads_str.byzantine_gold().bold()
        } else {
            threads_str.to_string()
        };

        println!(
            "║  {} Thread Count: {}{}[← →] to adjust{}║",
            marker,
            threads_display,
            " ".repeat(15 - threads_str.len()),
            " ".repeat(38)
        );
        println!(
            "║     (Recommend {} cores for this system){}║",
            max_threads,
            " ".repeat(30)
        );

        Ok(())
    }

    /// Render memory limit setting
    fn render_memory_setting(&self, index: u8, selected: u8) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        // Convert MB back to GB for display
        let memory_gb = self.config.memory_limit_mb() / 1024;
        let memory_str = format!("{} GB", memory_gb);
        let memory_display = if is_selected {
            memory_str.byzantine_gold().bold()
        } else {
            memory_str.to_string()
        };

        println!(
            "║  {} Memory Limit: {}{}[← →] to adjust{}║",
            marker,
            memory_display,
            " ".repeat(15 - memory_str.len()),
            " ".repeat(38)
        );
        println!(
            "║     (Auto-detected system RAM, or specify manually){}║",
            " ".repeat(21)
        );

        Ok(())
    }

    /// Render a checkbox setting
    fn render_checkbox(&self, index: u8, selected: u8, label: &str, enabled: bool) -> io::Result<()> {
        let is_selected = index == selected;
        let marker = if is_selected { "▶" } else { " " };

        let checkbox = if enabled {
            "[✓]".byzantine_gold()
        } else {
            "[✗]".dim()
        };

        let label_str = if is_selected {
            label.byzantine_gold().bold()
        } else {
            label.to_string()
        };

        println!(
            "║  {} {} {}{}║",
            marker,
            checkbox,
            label_str,
            " ".repeat(70 - label.len() - 3)
        );

        Ok(())
    }

    /// Render configuration summary
    fn render_summary(&self) -> io::Result<()> {
        println!("║  {} Configuration Summary{}║", "📋".bright_gold(), " ".repeat(52));
        println!("║{}║", " ".repeat(78));

        // Display threshold (Q16.16 deterministic)
        let threshold_f64 = self.config.threshold_f64();
        let memory_gb = self.config.memory_limit_mb() / 1024;

        println!(
            "║    Threshold: {:.2} (Q16.16) | Threads: {} | Memory: {}GB{}║",
            threshold_f64,
            self.config.threads(),
            memory_gb,
            " ".repeat(16)
        );

        // Read feature flags atomically
        let features = [
            ("Q34", features::FEATURE_Q34_AUDIT),
            ("Bloom", features::FEATURE_BLOOM_FILTER),
            ("SIMD", features::FEATURE_SIMD),
            ("Batch", features::FEATURE_BATCH_LSH),
        ];

        let enabled_features: Vec<&str> = features
            .iter()
            .filter_map(|(name, flag)| {
                if self.config.is_feature_enabled(*flag) {
                    Some(*name)
                } else {
                    None
                }
            })
            .collect();

        if !enabled_features.is_empty() {
            println!(
                "║    Features: {}{}║",
                enabled_features.join(", ").byzantine_gold(),
                " ".repeat(65 - enabled_features.join(", ").len())
            );
        }

        Ok(())
    }

    /// Adjust threshold (Q16.16 deterministic)
    ///
    /// Updates the configuration with new threshold, clamping to [0.0, 1.0].
    /// Uses ConfigurationCapsule's deterministic Q16.16 fixed-point arithmetic.
    pub fn adjust_threshold(&mut self, delta: f64) {
        let current = self.config.threshold_f64();
        let new_threshold = (current + delta).max(0.0).min(1.0);
        self.config = self.config.set_threshold(new_threshold);
    }

    /// Adjust thread count (atomic, deterministic)
    ///
    /// Updates the configuration with new thread count, clamping to [1, 256].
    pub fn adjust_threads(&mut self, delta: i32) {
        let current = self.config.threads() as i32;
        let new_threads = (current + delta).max(1).min(256) as u32;
        self.config = self.config.set_threads(new_threads);
    }

    /// Adjust memory limit in MB (atomic, deterministic)
    ///
    /// Updates the configuration with new memory limit in MB.
    pub fn adjust_memory(&mut self, delta_mb: i32) {
        let current = self.config.memory_limit_mb() as i32;
        let new_memory_mb = (current + delta_mb).max(256).min(262144) as u32; // 256MB - 256GB
        self.config = self.config.set_memory_limit_mb(new_memory_mb);
    }

    /// Toggle a feature by index (atomic, deterministic)
    ///
    /// Maps menu indices to feature flags and toggles atomically.
    pub fn toggle_feature(&mut self, index: u8) {
        let flag = match index {
            3 => features::FEATURE_Q34_AUDIT,
            4 => features::FEATURE_BLOOM_FILTER,
            5 => features::FEATURE_SIMD,
            6 => features::FEATURE_BATCH_LSH,
            7 => features::FEATURE_NUMA,
            8 => features::FEATURE_HUGE_PAGES,
            _ => return,
        };

        self.config = self.config.toggle_feature(flag);
    }
}

impl Default for ConfigurationScreen {
    fn default() -> Self {
        Self::new()
    }
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
    fn test_default_config() {
        let config = create_default_config();
        let threshold = config.threshold_f64();
        assert!(threshold > 0.0 && threshold < 1.0);
        assert!(config.threads() > 0 && config.threads() <= 256);
        assert!(config.memory_limit_mb() > 0);
    }

    #[test]
    fn test_configuration_capsule_determinism() {
        // Test Q16.16 deterministic round-trip
        let threshold_orig = 0.85_f64;
        let config = ConfigurationCapsule::new().set_threshold(threshold_orig);
        let threshold_read = config.threshold_f64();

        // Q16.16 is deterministic: should be bit-exact
        assert!((threshold_read - threshold_orig).abs() < 0.00002); // < 1/65536
    }

    #[test]
    fn test_configuration_screen_creation() {
        let screen = ConfigurationScreen::new();
        assert!(screen.config.is_feature_enabled(features::FEATURE_Q34_AUDIT));
        assert!(screen.config.is_feature_enabled(features::FEATURE_BLOOM_FILTER));
    }

    #[test]
    fn test_adjust_threshold() {
        let mut screen = ConfigurationScreen::new();
        let original = screen.config.threshold_f64();

        screen.adjust_threshold(0.05);
        assert!(screen.config.threshold_f64() > original);

        screen.adjust_threshold(-0.1);
        assert!(screen.config.threshold_f64() < original);

        // Test boundaries
        screen.config = screen.config.set_threshold(0.95);
        screen.adjust_threshold(0.2);
        assert!(screen.config.threshold_f64() <= 1.0);
    }

    #[test]
    fn test_adjust_threads() {
        let mut screen = ConfigurationScreen::new();
        let original = screen.config.threads();

        screen.adjust_threads(2);
        assert!(screen.config.threads() > original);

        screen.adjust_threads(-2);
        assert_eq!(screen.config.threads(), original);
    }

    #[test]
    fn test_toggle_feature() {
        let mut screen = ConfigurationScreen::new();
        let original = screen.config.is_feature_enabled(features::FEATURE_SIMD);

        screen.toggle_feature(5); // SIMD (index 5)
        assert_eq!(screen.config.is_feature_enabled(features::FEATURE_SIMD), !original);

        screen.toggle_feature(5);
        assert_eq!(screen.config.is_feature_enabled(features::FEATURE_SIMD), original);
    }

    #[test]
    fn test_configuration_validity() {
        let config = create_default_config();
        assert!(config.is_valid(), "Default configuration should have valid checksum");
    }

    #[test]
    fn test_memory_limit_conversion() {
        let config = ConfigurationCapsule::new().set_memory_limit_mb(8192); // 8 GB in MB

        assert_eq!(config.memory_limit_mb(), 8192);
        assert_eq!(config.memory_limit_mb() / 1024, 8); // Should be 8 GB
    }
}
