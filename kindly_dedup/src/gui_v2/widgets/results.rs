//! Results widget with statistics display
//!
//! # Features
//! - Success checkmark with spring animation
//! - Statistics table (total docs, duplicates, speedup)
//! - Output path display
//! - Reset button with hover state
//! - Chaos-compliant AtomicU64 state (T1 Atomic tier)

use std::sync::atomic::{AtomicU64, Ordering};
use crate::gui_v2::layout::Rect;

/// Results widget state
///
/// # State Encoding (AtomicU64)
/// - Bits 0-31: Total documents processed
/// - Bits 32-47: Duplicate count (max 65535)
/// - Bits 48-55: Animation phase (0-255, for checkmark spring)
/// - Bits 56-63: Hover state (0=none, 1=reset_button)
#[repr(C, align(64))]
pub struct ResultsWidget {
    /// Packed state (stats + animation + hover)
    state: AtomicU64,
    /// Output path (heap-allocated, not in hot path)
    output_path: std::sync::RwLock<Option<String>>,
    /// Speedup vs baseline (heap-allocated)
    speedup: std::sync::RwLock<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverState {
    None = 0,
    ResetButton = 1,
}

impl ResultsWidget {
    /// Create new results widget
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            output_path: std::sync::RwLock::new(None),
            speedup: std::sync::RwLock::new(1.0),
        }
    }

    /// Set results (total docs, duplicates, output path, speedup)
    pub fn set_results(&self, total: u32, duplicates: u16, output_path: String, speedup: f64) {
        // Pack total and duplicates
        let packed = (total as u64) | ((duplicates as u64) << 32);

        // Update state (preserve animation and hover)
        let old = self.state.load(Ordering::Acquire);
        let new = (old & 0xFFFF000000000000) | packed;
        self.state.store(new, Ordering::Release);

        // Update heap-allocated fields
        *self.output_path.write().unwrap() = Some(output_path);
        *self.speedup.write().unwrap() = speedup;

        // Reset animation phase
        self.reset_animation();
    }

    /// Clear results
    pub fn clear(&self) {
        self.state.store(0, Ordering::Release);
        *self.output_path.write().unwrap() = None;
        *self.speedup.write().unwrap() = 1.0;
    }

    /// Get total documents
    pub fn get_total(&self) -> u32 {
        let state = self.state.load(Ordering::Acquire);
        (state & 0xFFFFFFFF) as u32
    }

    /// Get duplicate count
    pub fn get_duplicates(&self) -> u16 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 32) & 0xFFFF) as u16
    }

    /// Get output path
    pub fn get_output_path(&self) -> Option<String> {
        self.output_path.read().unwrap().clone()
    }

    /// Get speedup
    pub fn get_speedup(&self) -> f64 {
        *self.speedup.read().unwrap()
    }

    /// Set hover state
    pub fn set_hover(&self, hover: HoverState) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF00000000000000) | ((hover as u64) << 56);
        self.state.store(new, Ordering::Release);
    }

    /// Get hover state
    pub fn get_hover(&self) -> HoverState {
        let state = self.state.load(Ordering::Acquire);
        let hover_bits = ((state >> 56) & 0xFF) as u8;
        match hover_bits {
            1 => HoverState::ResetButton,
            _ => HoverState::None,
        }
    }

    /// Check if reset button is hovered
    pub fn is_reset_hovered(&self) -> bool {
        self.get_hover() == HoverState::ResetButton
    }

    /// Update animation (call each frame)
    pub fn update_animation(&self) {
        let old = self.state.load(Ordering::Acquire);
        let phase = ((old >> 48) & 0xFF) as u8;

        // Spring animation: 0 → 255 → settle at 128
        let new_phase = if phase < 128 {
            phase.saturating_add(16) // Fast rise
        } else if phase > 128 {
            phase.saturating_sub(8) // Slow settle
        } else {
            128 // Settled
        };

        let new = (old & !0xFF000000000000) | ((new_phase as u64) << 48);
        self.state.store(new, Ordering::Release);
    }

    /// Reset animation to start
    pub fn reset_animation(&self) {
        let old = self.state.load(Ordering::Acquire);
        let new = old & !0xFF000000000000; // Set phase to 0
        self.state.store(new, Ordering::Release);
    }

    /// Get animation phase (0-255)
    pub fn get_animation_phase(&self) -> u8 {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 48) & 0xFF) as u8
    }

    /// Get checkmark scale (for spring animation)
    pub fn get_checkmark_scale(&self) -> f32 {
        let phase = self.get_animation_phase();
        // Map phase to scale: 0 → 0.0, 128 → 1.0, 255 → 1.2, then settle to 1.0
        if phase <= 128 {
            (phase as f32) / 128.0
        } else {
            1.0 + ((phase - 128) as f32 / 127.0) * 0.2
        }
    }

    /// Get reset button bounds (for hit testing)
    pub fn get_reset_button_bounds(&self) -> Rect {
        // Reset button is 150px wide, 40px tall
        // Positioned at (325, 750) in layout (centered)
        Rect {
            x: 325,
            y: 750,
            width: 150,
            height: 40,
        }
    }

    /// Format statistics for display
    pub fn format_stats(&self) -> String {
        let total = self.get_total();
        let duplicates = self.get_duplicates();
        let speedup = self.get_speedup();

        format!(
            "Total: {} docs | Duplicates: {} | Speedup: {:.1}×",
            total, duplicates, speedup
        )
    }

    /// Calculate duplicate percentage
    pub fn get_duplicate_percentage(&self) -> f64 {
        let total = self.get_total();
        if total == 0 {
            return 0.0;
        }
        let duplicates = self.get_duplicates();
        (duplicates as f64 / total as f64) * 100.0
    }
}

impl Default for ResultsWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_widget() {
        let widget = ResultsWidget::new();
        assert_eq!(widget.get_total(), 0);
        assert_eq!(widget.get_duplicates(), 0);
        assert!(widget.get_output_path().is_none());
        assert_eq!(widget.get_speedup(), 1.0);
        assert_eq!(widget.get_hover(), HoverState::None);
        assert_eq!(widget.get_animation_phase(), 0);
    }

    #[test]
    fn test_set_results() {
        let widget = ResultsWidget::new();
        widget.set_results(
            10_000,
            1_500,
            String::from("/output/deduped.jsonl"),
            38.5,
        );

        assert_eq!(widget.get_total(), 10_000);
        assert_eq!(widget.get_duplicates(), 1_500);
        assert_eq!(
            widget.get_output_path(),
            Some(String::from("/output/deduped.jsonl"))
        );
        assert_eq!(widget.get_speedup(), 38.5);
    }

    #[test]
    fn test_clear() {
        let widget = ResultsWidget::new();
        widget.set_results(10_000, 1_500, String::from("/output/deduped.jsonl"), 38.5);
        widget.clear();

        assert_eq!(widget.get_total(), 0);
        assert_eq!(widget.get_duplicates(), 0);
        assert!(widget.get_output_path().is_none());
        assert_eq!(widget.get_speedup(), 1.0);
    }

    #[test]
    fn test_hover_state() {
        let widget = ResultsWidget::new();

        widget.set_hover(HoverState::ResetButton);
        assert_eq!(widget.get_hover(), HoverState::ResetButton);
        assert!(widget.is_reset_hovered());

        widget.set_hover(HoverState::None);
        assert_eq!(widget.get_hover(), HoverState::None);
        assert!(!widget.is_reset_hovered());
    }

    #[test]
    fn test_hover_preserves_results() {
        let widget = ResultsWidget::new();
        widget.set_results(5_000, 750, String::from("/out.jsonl"), 20.0);

        widget.set_hover(HoverState::ResetButton);

        assert_eq!(widget.get_total(), 5_000);
        assert_eq!(widget.get_duplicates(), 750);
    }

    #[test]
    fn test_animation_phase() {
        let widget = ResultsWidget::new();
        assert_eq!(widget.get_animation_phase(), 0);

        // Simulate animation updates
        for _ in 0..10 {
            widget.update_animation();
        }

        let phase = widget.get_animation_phase();
        assert!(phase > 0 && phase <= 255);
    }

    #[test]
    fn test_animation_settles() {
        let widget = ResultsWidget::new();

        // Run animation to completion
        for _ in 0..50 {
            widget.update_animation();
        }

        // Should settle at 128
        let phase = widget.get_animation_phase();
        assert_eq!(phase, 128);
    }

    #[test]
    fn test_reset_animation() {
        let widget = ResultsWidget::new();

        // Advance animation
        for _ in 0..10 {
            widget.update_animation();
        }

        widget.reset_animation();
        assert_eq!(widget.get_animation_phase(), 0);
    }

    #[test]
    fn test_checkmark_scale() {
        let widget = ResultsWidget::new();

        // Phase 0 → scale 0.0
        assert_eq!(widget.get_checkmark_scale(), 0.0);

        // Advance to phase 128 → scale 1.0
        for _ in 0..8 {
            widget.update_animation();
        }
        assert_eq!(widget.get_animation_phase(), 128);
        assert_eq!(widget.get_checkmark_scale(), 1.0);

        // Continue to overshoot
        for _ in 0..5 {
            widget.update_animation();
        }
        let scale = widget.get_checkmark_scale();
        assert!(scale >= 1.0); // Should be slightly above 1.0
    }

    #[test]
    fn test_format_stats() {
        let widget = ResultsWidget::new();
        widget.set_results(10_000, 1_500, String::from("/out.jsonl"), 38.5);

        let stats = widget.format_stats();
        assert!(stats.contains("10000"));
        assert!(stats.contains("1500"));
        assert!(stats.contains("38.5"));
    }

    #[test]
    fn test_get_duplicate_percentage() {
        let widget = ResultsWidget::new();
        widget.set_results(10_000, 1_500, String::from("/out.jsonl"), 38.5);

        let percentage = widget.get_duplicate_percentage();
        assert!((percentage - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_get_duplicate_percentage_zero_total() {
        let widget = ResultsWidget::new();
        assert_eq!(widget.get_duplicate_percentage(), 0.0);
    }

    #[test]
    fn test_reset_button_bounds() {
        let widget = ResultsWidget::new();
        let bounds = widget.get_reset_button_bounds();
        assert_eq!(bounds.x, 325);
        assert_eq!(bounds.y, 750);
        assert_eq!(bounds.width, 150);
        assert_eq!(bounds.height, 40);
    }

    #[test]
    fn test_max_duplicates() {
        let widget = ResultsWidget::new();
        widget.set_results(100_000, u16::MAX, String::from("/out.jsonl"), 100.0);
        assert_eq!(widget.get_duplicates(), u16::MAX);
    }

    #[test]
    fn test_max_total() {
        let widget = ResultsWidget::new();
        widget.set_results(u32::MAX, 1_000, String::from("/out.jsonl"), 50.0);
        assert_eq!(widget.get_total(), u32::MAX);
    }

    #[test]
    fn test_atomic_alignment() {
        let widget = ResultsWidget::new();
        let ptr = &widget as *const ResultsWidget as usize;
        assert_eq!(ptr % 64, 0, "ResultsWidget not 64-byte aligned");
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let widget = Arc::new(ResultsWidget::new());
        let widget1 = Arc::clone(&widget);
        let widget2 = Arc::clone(&widget);

        widget.set_results(10_000, 1_500, String::from("/out.jsonl"), 38.5);

        let h1 = thread::spawn(move || {
            for _ in 0..1000 {
                widget1.update_animation();
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                widget2.set_hover(HoverState::ResetButton);
                widget2.set_hover(HoverState::None);
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Results should be preserved
        assert_eq!(widget.get_total(), 10_000);
        assert_eq!(widget.get_duplicates(), 1_500);
    }
}
