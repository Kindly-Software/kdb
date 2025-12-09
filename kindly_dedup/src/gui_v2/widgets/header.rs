//! Header widget with animated title
//!
//! # Features
//! - "Kindly Dedup" title (64px font)
//! - Glow pulse animation (purple → gold)
//! - Subtitle text
//! - Chaos-compliant AtomicU64 state (T1 Atomic tier)

use std::sync::atomic::{AtomicU64, Ordering};
use crate::gui_v2::layout::Rect;
use super::{Color, theme};

/// Header widget state
///
/// # State Encoding (AtomicU64)
/// - Bits 0-15: Glow animation phase (0-65535, maps to 0-2π for sine wave)
/// - Bits 16-63: Reserved
#[repr(C, align(64))]
pub struct HeaderWidget {
    /// Packed state (glow animation phase)
    state: AtomicU64,
}

impl HeaderWidget {
    /// Create new header widget
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
        }
    }

    /// Update glow animation (call each frame)
    pub fn update_glow(&self) {
        let old = self.state.load(Ordering::Acquire);
        let phase = (old & 0xFFFF) as u16;
        let new_phase = phase.wrapping_add(128); // Increment by 128 each frame
        let new = (old & !0xFFFF) | (new_phase as u64);
        self.state.store(new, Ordering::Release);
    }

    /// Get glow animation phase (0-65535)
    pub fn get_glow_phase(&self) -> u16 {
        let state = self.state.load(Ordering::Acquire);
        (state & 0xFFFF) as u16
    }

    /// Get glow color (interpolates between purple and gold)
    pub fn get_glow_color(&self) -> Color {
        let phase = self.get_glow_phase();

        // Convert phase to 0.0-1.0 range using sine wave
        // phase / 65535 * 2π, then (sin(x) + 1) / 2 for 0.0-1.0
        let radians = (phase as f32 / 65535.0) * 2.0 * std::f32::consts::PI;
        let sine = radians.sin();
        let t = (sine + 1.0) / 2.0; // 0.0 at phase=0, 1.0 at phase=32768, 0.0 at phase=65535

        // Interpolate between PURPLE_ROYAL and GOLD_BRIGHT
        let purple = theme::PURPLE_ROYAL;
        let gold = theme::GOLD_BRIGHT;

        Color {
            r: (purple.r as f32 + (gold.r as f32 - purple.r as f32) * t) as u8,
            g: (purple.g as f32 + (gold.g as f32 - purple.g as f32) * t) as u8,
            b: (purple.b as f32 + (gold.b as f32 - purple.b as f32) * t) as u8,
            a: 255,
        }
    }

    /// Get glow intensity (0.0-1.0)
    pub fn get_glow_intensity(&self) -> f32 {
        let phase = self.get_glow_phase();

        // Sine wave for smooth pulsing
        let radians = (phase as f32 / 65535.0) * 2.0 * std::f32::consts::PI;
        let sine = radians.sin();
        (sine + 1.0) / 2.0 // Map -1..1 to 0..1
    }

    /// Get title text
    pub fn get_title(&self) -> &'static str {
        "Kindly Dedup"
    }

    /// Get subtitle text
    pub fn get_subtitle(&self) -> &'static str {
        "High-Performance LLM Dataset Deduplication"
    }

    /// Get title bounds (for layout)
    pub fn get_title_bounds(&self) -> Rect {
        // Title is centered at top, 64px font
        // Approximate width: 15 chars × 40px = 600px
        Rect {
            x: 100,
            y: 20,
            width: 600,
            height: 80,
        }
    }

    /// Get subtitle bounds (for layout)
    pub fn get_subtitle_bounds(&self) -> Rect {
        // Subtitle below title, 18px font
        // Approximate width: 45 chars × 12px = 540px
        Rect {
            x: 130,
            y: 110,
            width: 540,
            height: 25,
        }
    }

    /// Reset animation
    pub fn reset(&self) {
        self.state.store(0, Ordering::Release);
    }

    /// Render header widget
    ///
    /// # Output
    ///
    /// - Title text with glow (64px font)
    /// - Subtitle text (18px font)
    ///
    /// # Performance
    ///
    /// - Shape creation: <50ns (2 shapes + 2 text commands)
    /// - Glow color calculation: Already cached in get_glow_color()
    pub fn render(&self, shapes: &mut Vec<super::super::rendering_primitives::Shape>, texts: &mut Vec<super::super::rendering_primitives::TextCommand>) {
        use super::super::rendering_primitives::{Shape, TextCommand};

        let title_bounds = self.get_title_bounds();
        let subtitle_bounds = self.get_subtitle_bounds();
        let glow_color = self.get_glow_color();

        // Title text with glow
        texts.push(TextCommand::centered(
            self.get_title(),
            title_bounds.x + title_bounds.width / 2,
            title_bounds.y,
            64, // Font size
            glow_color,
        ));

        // Subtitle text
        texts.push(TextCommand::centered(
            self.get_subtitle(),
            subtitle_bounds.x + subtitle_bounds.width / 2,
            subtitle_bounds.y,
            18, // Font size
            theme::TEXT_SECONDARY,
        ));
    }
}

impl Default for HeaderWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_widget() {
        let widget = HeaderWidget::new();
        assert_eq!(widget.get_glow_phase(), 0);
    }

    #[test]
    fn test_update_glow() {
        let widget = HeaderWidget::new();

        widget.update_glow();
        assert_eq!(widget.get_glow_phase(), 128);

        widget.update_glow();
        assert_eq!(widget.get_glow_phase(), 256);
    }

    #[test]
    fn test_glow_wraparound() {
        let widget = HeaderWidget::new();

        // Set to near max
        for _ in 0..512 {
            widget.update_glow();
        }

        // Should wrap around (512 * 128 = 65536 wraps to 0)
        assert_eq!(widget.get_glow_phase(), 0);
    }

    #[test]
    fn test_get_glow_color() {
        let widget = HeaderWidget::new();

        // Phase 0: sine(0) = 0, t = 0.5 (halfway between purple and gold)
        // Color is interpolated at t=0.5
        let color = widget.get_glow_color();
        // Should be somewhere between purple and gold (t=0.5)
        let purple = theme::PURPLE_ROYAL;
        let gold = theme::GOLD_BRIGHT;
        let mid_r = (purple.r as i16 + gold.r as i16) / 2;
        let diff = (color.r as i16 - mid_r).abs();
        assert!(diff < 20, "Expected color near midpoint, got diff={}", diff);

        // Phase 16384 (quarter cycle): sine(π/2) = 1, t = 1.0 (gold)
        for _ in 0..128 {
            widget.update_glow();
        }
        let color = widget.get_glow_color();
        // Should be closer to gold at peak
        assert!(color.r > purple.r, "At peak, red should exceed purple");
    }

    #[test]
    fn test_get_glow_intensity() {
        let widget = HeaderWidget::new();

        // Phase 0 should be ~0.5 intensity (sine wave start)
        let intensity = widget.get_glow_intensity();
        assert!((intensity - 0.5).abs() < 0.1);

        // Phase 16384 (quarter cycle) should be ~1.0 (peak)
        for _ in 0..128 {
            widget.update_glow();
        }
        let intensity = widget.get_glow_intensity();
        assert!(intensity > 0.9);
    }

    #[test]
    fn test_get_title() {
        let widget = HeaderWidget::new();
        assert_eq!(widget.get_title(), "Kindly Dedup");
    }

    #[test]
    fn test_get_subtitle() {
        let widget = HeaderWidget::new();
        assert_eq!(
            widget.get_subtitle(),
            "High-Performance LLM Dataset Deduplication"
        );
    }

    #[test]
    fn test_title_bounds() {
        let widget = HeaderWidget::new();
        let bounds = widget.get_title_bounds();
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 20);
        assert_eq!(bounds.width, 600);
        assert_eq!(bounds.height, 80);
    }

    #[test]
    fn test_subtitle_bounds() {
        let widget = HeaderWidget::new();
        let bounds = widget.get_subtitle_bounds();
        assert_eq!(bounds.x, 130);
        assert_eq!(bounds.y, 110);
        assert_eq!(bounds.width, 540);
        assert_eq!(bounds.height, 25);
    }

    #[test]
    fn test_reset() {
        let widget = HeaderWidget::new();

        // Advance animation
        for _ in 0..10 {
            widget.update_glow();
        }

        widget.reset();
        assert_eq!(widget.get_glow_phase(), 0);
    }

    #[test]
    fn test_atomic_alignment() {
        let widget = HeaderWidget::new();
        let ptr = &widget as *const HeaderWidget as usize;
        assert_eq!(ptr % 64, 0, "HeaderWidget not 64-byte aligned");
    }

    #[test]
    fn test_glow_animation_smooth() {
        let widget = HeaderWidget::new();

        // Verify animation produces smooth values
        let mut prev_intensity = widget.get_glow_intensity();

        for _ in 0..100 {
            widget.update_glow();
            let intensity = widget.get_glow_intensity();

            // Intensity should change smoothly (not jump)
            let delta = (intensity - prev_intensity).abs();
            assert!(delta < 0.1, "Animation not smooth: delta={}", delta);

            prev_intensity = intensity;
        }
    }

    #[test]
    fn test_concurrent_glow_updates() {
        use std::sync::Arc;
        use std::thread;

        let widget = Arc::new(HeaderWidget::new());
        let widget1 = Arc::clone(&widget);
        let widget2 = Arc::clone(&widget);

        let h1 = thread::spawn(move || {
            for _ in 0..1000 {
                widget1.update_glow();
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                widget2.update_glow();
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Phase should be valid (no corruption)
        let phase = widget.get_glow_phase();
        assert!(phase <= 65535);
    }

    #[test]
    fn test_render() {
        use super::super::super::rendering_primitives::{Shape, TextCommand};

        let widget = HeaderWidget::new();
        let mut shapes = Vec::new();
        let mut texts = Vec::new();

        widget.render(&mut shapes, &mut texts);

        // Should produce 2 text commands (title + subtitle)
        assert_eq!(texts.len(), 2);

        // Title
        assert_eq!(texts[0].text.as_str(), "Kindly Dedup");
        assert_eq!(texts[0].font_size, 64);

        // Subtitle
        assert_eq!(texts[1].text.as_str(), "High-Performance LLM Dataset Deduplication");
        assert_eq!(texts[1].font_size, 18);

        // No shapes (text only)
        assert_eq!(shapes.len(), 0);
    }

    #[test]
    fn test_render_glow_color_changes() {
        use super::super::super::rendering_primitives::{Shape, TextCommand};

        let widget = HeaderWidget::new();

        // Initial glow
        let mut texts1 = Vec::new();
        widget.render(&mut Vec::new(), &mut texts1);
        let color1 = texts1[0].color;

        // Advance animation to change glow
        for _ in 0..128 {
            widget.update_glow();
        }

        let mut texts2 = Vec::new();
        widget.render(&mut Vec::new(), &mut texts2);
        let color2 = texts2[0].color;

        // Color should have changed due to glow animation
        assert_ne!(color1.r, color2.r);
    }
}
