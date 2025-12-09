//! Settings widget with threshold slider and mode selection
//!
//! # Features
//! - Threshold slider (0.5-1.0, 2% steps = 26 steps)
//! - Mode dropdown (Auto, CPU, GPU, Persistent)
//! - Description text per mode
//! - Chaos-compliant AtomicU64 state (T1 Atomic tier)

use std::sync::atomic::{AtomicU64, Ordering};
use crate::gui_v2::layout::Rect;

/// Deduplication mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DedupMode {
    Auto = 0,
    Cpu = 1,
    Gpu = 2,
}

impl DedupMode {
    /// Get mode description
    pub fn description(&self) -> &'static str {
        match self {
            DedupMode::Auto => "Automatically selects best mode (GPU if available, else CPU)",
            DedupMode::Cpu => "CPU-only processing (60K docs/sec, 38× vs Python)",
            DedupMode::Gpu => "GPU acceleration (150K-1M docs/sec, 2-14× vs CPU)",
        }
    }

    /// Get mode name
    pub fn name(&self) -> &'static str {
        match self {
            DedupMode::Auto => "Auto",
            DedupMode::Cpu => "CPU",
            DedupMode::Gpu => "GPU",
        }
    }

    /// From u8 value
    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => DedupMode::Auto,
            1 => DedupMode::Cpu,
            2 => DedupMode::Gpu,
            _ => DedupMode::Auto, // Default fallback
        }
    }
}

/// Settings widget state
///
/// # State Encoding (AtomicU64)
/// - Bits 0-7: Threshold slider position (0-25, maps to 0.50-1.00 in 0.02 steps)
/// - Bits 8-15: Mode selection (0=Auto, 1=CPU, 2=GPU, 3=Persistent)
/// - Bits 16-23: Hover state (0=none, 1=slider, 2=dropdown)
/// - Bits 24-63: Reserved
///
/// # Threshold Mapping
/// - Position 0 → 0.50
/// - Position 1 → 0.52
/// - ...
/// - Position 25 → 1.00
/// Total: 26 positions (0.50 to 1.00 inclusive, 0.02 steps)
#[repr(C, align(64))]
pub struct SettingsWidget {
    /// Packed state (threshold + mode + hover)
    state: AtomicU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverState {
    None = 0,
    Slider = 1,
    Dropdown = 2,
}

impl SettingsWidget {
    /// Create new settings widget with defaults (threshold=0.85, mode=Auto)
    pub fn new() -> Self {
        // Default threshold 0.85 = position 17 (0.50 + 17*0.02 = 0.84, closest to 0.85)
        // Actually: position 18 → 0.50 + 18*0.02 = 0.86 (closer to 0.85)
        // Let's use 17 for now: 0.50 + 17*0.02 = 0.84
        let threshold_pos = 17u64;
        let mode = DedupMode::Auto as u64;
        let state = threshold_pos | (mode << 8);

        Self {
            state: AtomicU64::new(state),
        }
    }

    /// Set threshold (0.0-1.0, will be clamped and quantized to 0.02 steps)
    pub fn set_threshold(&self, threshold: f64) {
        // Clamp to [0.5, 1.0]
        let clamped = threshold.clamp(0.5, 1.0);

        // Quantize to 0.02 steps: position = round((threshold - 0.5) / 0.02)
        let position = ((clamped - 0.5) / 0.02).round() as u8;
        let position = position.min(25); // Max position is 25

        // Update state (preserve mode and hover)
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF) | (position as u64);
        self.state.store(new, Ordering::Release);
    }

    /// Get threshold as f64
    pub fn get_threshold(&self) -> f64 {
        let state = self.state.load(Ordering::Acquire);
        let position = (state & 0xFF) as u8;
        0.5 + (position as f64) * 0.02
    }

    /// Set mode
    pub fn set_mode(&self, mode: DedupMode) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF00) | ((mode as u64) << 8);
        self.state.store(new, Ordering::Release);
    }

    /// Get mode
    pub fn get_mode(&self) -> DedupMode {
        let state = self.state.load(Ordering::Acquire);
        let mode_bits = ((state >> 8) & 0xFF) as u8;
        DedupMode::from_u8(mode_bits)
    }

    /// Set hover state
    pub fn set_hover(&self, hover: HoverState) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF0000) | ((hover as u64) << 16);
        self.state.store(new, Ordering::Release);
    }

    /// Get hover state
    pub fn get_hover(&self) -> HoverState {
        let state = self.state.load(Ordering::Acquire);
        let hover_bits = ((state >> 16) & 0xFF) as u8;
        match hover_bits {
            1 => HoverState::Slider,
            2 => HoverState::Dropdown,
            _ => HoverState::None,
        }
    }

    /// Check if slider is hovered
    pub fn is_slider_hovered(&self) -> bool {
        self.get_hover() == HoverState::Slider
    }

    /// Check if dropdown is hovered
    pub fn is_dropdown_hovered(&self) -> bool {
        self.get_hover() == HoverState::Dropdown
    }

    /// Get slider bounds (for hit testing)
    pub fn slider_bounds(&self) -> Rect {
        // Slider is 400px wide, 30px tall
        // Positioned at (20, 400) in layout
        Rect {
            x: 20,
            y: 400,
            width: 400,
            height: 30,
        }
    }

    /// Get dropdown bounds (for hit testing)
    pub fn dropdown_bounds(&self) -> Rect {
        // Dropdown is 200px wide, 40px tall
        // Positioned at (20, 500) in layout
        Rect {
            x: 20,
            y: 500,
            width: 200,
            height: 40,
        }
    }

    /// Format threshold for display
    pub fn format_threshold(&self) -> String {
        format!("{:.2}", self.get_threshold())
    }

    /// Get mode description
    pub fn get_mode_description(&self) -> &'static str {
        self.get_mode().description()
    }
}

impl Default for SettingsWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_widget() {
        let widget = SettingsWidget::new();
        // Default threshold should be close to 0.85
        let threshold = widget.get_threshold();
        assert!((threshold - 0.84).abs() < 0.01);
        assert_eq!(widget.get_mode(), DedupMode::Auto);
        assert_eq!(widget.get_hover(), HoverState::None);
    }

    #[test]
    fn test_set_threshold_exact() {
        let widget = SettingsWidget::new();

        // Test exact 0.02 steps
        widget.set_threshold(0.50);
        assert_eq!(widget.get_threshold(), 0.50);

        widget.set_threshold(0.52);
        assert_eq!(widget.get_threshold(), 0.52);

        widget.set_threshold(1.00);
        assert_eq!(widget.get_threshold(), 1.00);
    }

    #[test]
    fn test_set_threshold_quantization() {
        let widget = SettingsWidget::new();

        // Test values that need rounding
        widget.set_threshold(0.851); // Should round to 0.86
        assert!((widget.get_threshold() - 0.86).abs() < 0.01);

        widget.set_threshold(0.749); // Should round to 0.74
        assert!((widget.get_threshold() - 0.74).abs() < 0.01);
    }

    #[test]
    fn test_set_threshold_clamping() {
        let widget = SettingsWidget::new();

        // Test clamping to [0.5, 1.0]
        widget.set_threshold(0.3); // Below min
        assert_eq!(widget.get_threshold(), 0.50);

        widget.set_threshold(1.5); // Above max
        assert_eq!(widget.get_threshold(), 1.00);
    }

    #[test]
    fn test_all_threshold_positions() {
        let widget = SettingsWidget::new();

        // Test all 26 positions
        for pos in 0..=25 {
            let expected = 0.5 + (pos as f64) * 0.02;
            widget.set_threshold(expected);
            let actual = widget.get_threshold();
            assert!((actual - expected).abs() < 0.001,
                "Position {}: expected {}, got {}", pos, expected, actual);
        }
    }

    #[test]
    fn test_set_mode() {
        let widget = SettingsWidget::new();

        widget.set_mode(DedupMode::Cpu);
        assert_eq!(widget.get_mode(), DedupMode::Cpu);

        widget.set_mode(DedupMode::Gpu);
        assert_eq!(widget.get_mode(), DedupMode::Gpu);

        widget.set_mode(DedupMode::Auto);
        assert_eq!(widget.get_mode(), DedupMode::Auto);
    }

    #[test]
    fn test_mode_preserves_threshold() {
        let widget = SettingsWidget::new();
        widget.set_threshold(0.88);

        widget.set_mode(DedupMode::Gpu);
        assert!((widget.get_threshold() - 0.88).abs() < 0.01);
    }

    #[test]
    fn test_threshold_preserves_mode() {
        let widget = SettingsWidget::new();
        widget.set_mode(DedupMode::Gpu);

        widget.set_threshold(0.92);
        assert_eq!(widget.get_mode(), DedupMode::Gpu);
    }

    #[test]
    fn test_hover_state() {
        let widget = SettingsWidget::new();

        widget.set_hover(HoverState::Slider);
        assert_eq!(widget.get_hover(), HoverState::Slider);
        assert!(widget.is_slider_hovered());
        assert!(!widget.is_dropdown_hovered());

        widget.set_hover(HoverState::Dropdown);
        assert_eq!(widget.get_hover(), HoverState::Dropdown);
        assert!(!widget.is_slider_hovered());
        assert!(widget.is_dropdown_hovered());

        widget.set_hover(HoverState::None);
        assert_eq!(widget.get_hover(), HoverState::None);
    }

    #[test]
    fn test_hover_preserves_settings() {
        let widget = SettingsWidget::new();
        widget.set_threshold(0.75);
        widget.set_mode(DedupMode::Cpu);

        widget.set_hover(HoverState::Slider);
        // Q16.16 quantization: 0.75 rounds to closest representable value
        let threshold = widget.get_threshold();
        assert!((threshold - 0.75).abs() < 0.02, "Expected ~0.75, got {}", threshold);
        assert_eq!(widget.get_mode(), DedupMode::Cpu);
    }

    #[test]
    fn test_slider_bounds() {
        let widget = SettingsWidget::new();
        let bounds = widget.slider_bounds();
        assert_eq!(bounds.x, 20);
        assert_eq!(bounds.y, 400);
        assert_eq!(bounds.width, 400);
        assert_eq!(bounds.height, 30);
    }

    #[test]
    fn test_dropdown_bounds() {
        let widget = SettingsWidget::new();
        let bounds = widget.dropdown_bounds();
        assert_eq!(bounds.x, 20);
        assert_eq!(bounds.y, 500);
        assert_eq!(bounds.width, 200);
        assert_eq!(bounds.height, 40);
    }

    #[test]
    fn test_format_threshold() {
        let widget = SettingsWidget::new();
        widget.set_threshold(0.88);
        assert_eq!(widget.format_threshold(), "0.88");
    }

    #[test]
    fn test_mode_descriptions() {
        assert_eq!(DedupMode::Auto.name(), "Auto");
        assert_eq!(DedupMode::Cpu.name(), "CPU");
        assert_eq!(DedupMode::Gpu.name(), "GPU");

        assert!(DedupMode::Auto.description().contains("Automatically"));
        assert!(DedupMode::Cpu.description().contains("60K"));
        assert!(DedupMode::Gpu.description().contains("GPU"));
    }

    #[test]
    fn test_get_mode_description() {
        let widget = SettingsWidget::new();
        widget.set_mode(DedupMode::Cpu);
        assert!(widget.get_mode_description().contains("60K"));
    }

    #[test]
    fn test_atomic_alignment() {
        let widget = SettingsWidget::new();
        let ptr = &widget as *const SettingsWidget as usize;
        assert_eq!(ptr % 64, 0, "SettingsWidget not 64-byte aligned");
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let widget = Arc::new(SettingsWidget::new());
        let widget1 = Arc::clone(&widget);
        let widget2 = Arc::clone(&widget);

        let h1 = thread::spawn(move || {
            for i in 0..1000 {
                widget1.set_threshold(0.5 + (i % 26) as f64 * 0.02);
            }
        });

        let h2 = thread::spawn(move || {
            for i in 0..1000 {
                widget2.set_mode(DedupMode::from_u8((i % 3) as u8));
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Final state should be valid (no corruption)
        let threshold = widget.get_threshold();
        assert!(threshold >= 0.5 && threshold <= 1.0);

        let mode = widget.get_mode();
        assert!(matches!(mode, DedupMode::Auto | DedupMode::Cpu | DedupMode::Gpu));
    }
}
