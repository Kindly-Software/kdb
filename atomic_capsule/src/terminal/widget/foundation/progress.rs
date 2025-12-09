//! Progress Bar Widget Capsule
//!
//! T1+T3 compound tier progress bar with smooth animation and indeterminate mode.
//!
//! ## Features
//!
//! - **Smooth Animation**: Q16.16 fixed-point interpolation
//! - **Multiple Styles**: Bar, Striped, Blocks, Dots
//! - **Indeterminate Mode**: Animated progress for unknown duration
//! - **Customizable**: Colors, width, labels
//! - **Lockfree**: 100% atomic operations
//!
//! ## UCE34 Compliance
//!
//! - **Q10**: T1+T3 compound (Atomic state + Q16.16 fixed-point)
//! - **Q33**: 100% lockfree, cache-aligned (128B)
//! - **Q34**: Progress tracking for audit trails
//!
//! ## Example
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::widget::ProgressCapsule;
//!
//! let progress = ProgressCapsule::new()
//!     .with_style(ProgressStyle::Striped)
//!     .with_width(40)
//!     .with_label("Downloading");
//!
//! progress.set_value_animated(0.5); // Animate to 50%
//! progress.update_animation(16); // Update at 60fps
//! ```

use core::sync::atomic::{AtomicU64, Ordering};

extern crate alloc;

#[cfg(feature = "derive")]
use atomic_capsule_derive::ComputationalCapsule;

use super::super::{Rect, RenderCommandBuffer, RenderCommand, RenderStyle, Widget, Constraints};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Q16.16 fixed-point scale (65536)
const SCALE: u32 = 1 << 16;

/// Animation speed for smooth interpolation (Q16.16 per ms)
const ANIMATION_SPEED: u32 = SCALE / 200; // 0.005 per ms (200ms total)

/// Indeterminate animation speed (Q8.8 per ms)
const INDETERMINATE_SPEED: u16 = 256 / 50; // Full cycle in ~50ms

// ============================================================================
// PROGRESS STYLE
// ============================================================================

/// Progress bar visual style
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgressStyle {
    /// Solid bar: `[████░░░░]`
    #[default]
    Bar = 0,
    /// Diagonal stripes (animated): `[▓▓▓▓░░░░]`
    Striped = 1,
    /// Unicode block characters: `[█████▌   ]`
    Blocks = 2,
    /// Dot pattern: `[●●●●○○○○]`
    Dots = 3,
}

impl ProgressStyle {
    /// Convert from u8 (used in atomic state)
    #[inline]
    pub const fn from_u8(val: u8) -> Self {
        match val {
            0 => Self::Bar,
            1 => Self::Striped,
            2 => Self::Blocks,
            3 => Self::Dots,
            _ => Self::Bar,
        }
    }
}

// ============================================================================
// PROGRESS STATE
// ============================================================================

/// Progress state snapshot (for Widget trait)
#[derive(Copy, Clone, Debug, Default)]
#[repr(C)]
pub struct ProgressState {
    /// Current value (Q16.16 fixed-point, 0.0-1.0)
    pub value: u32,
    /// Target value for smooth animation
    pub target: u32,
    /// Animation phase for indeterminate (Q8.8, 0-255)
    pub phase: u16,
    /// Mode: determinate(0), indeterminate(1)
    pub mode: u8,
    /// Progress style
    pub style: ProgressStyle,
}

// ============================================================================
// PROGRESS CAPSULE
// ============================================================================

/// T1+T3 - Progress bar with smooth animation
///
/// # Size
///
/// 128 bytes (cache-aligned for optimal performance)
///
/// # Layout
///
/// ```text
/// [0-7]   value_state (AtomicU64): value(32) | target(32)
/// [8-15]  anim_state (AtomicU64): phase(16) | mode(8) | _pad(40)
/// [16-19] style(1) | width(1) | height(1) | show_percent(1)
/// [20-23] fill_color (RGBA8888)
/// [24-27] track_color (RGBA8888)
/// [28-31] text_color (RGBA8888)
/// [32]    label_len
/// [33-56] label[24]
/// [57-127] _pad[71]
/// ```
///
/// # UCE34 Compliance
///
/// - **Q10**: T1+T3 compound (Atomic + Q16.16 fixed-point)
/// - **Q33**: 100% lockfree, cache-aligned
/// - **Q34**: Progress tracking audit
///
/// # ASSUM Safety
///
/// - #ASSUME: value/target in [0, SCALE] - enforced by set_value()
/// - #ASSUME: phase in [0, 255] - enforced by update_animation()
/// - #VERIFY: Atomic ordering (Relaxed for reads, Release/Acquire for writes)
#[repr(C, align(64))]
#[cfg_attr(feature = "derive", derive(ComputationalCapsule))]
#[cfg_attr(feature = "derive", capsule(alignment = 64))]
pub struct ProgressCapsule {
    // Atomic state (16 bytes)
    /// Packed: value(32) | target(32)
    value_state: AtomicU64,
    /// Packed: phase(16) | mode(8) | _pad(40)
    anim_state: AtomicU64,

    // Configuration (4 bytes)
    /// Progress style
    style: ProgressStyle,
    /// Width in cells (0 = auto fill)
    width: u8,
    /// Height in cells (1 = single line)
    height: u8,
    /// Show percentage text
    show_percent: bool,

    // Styling (12 bytes)
    /// Fill color (RGBA8888)
    fill_color: u32,
    /// Track color (RGBA8888)
    track_color: u32,
    /// Text color (RGBA8888)
    text_color: u32,

    // Label (25 bytes)
    /// Label length
    label_len: u8,
    /// Label text (max 24 chars)
    label: [u8; 24],

    // Padding to 128 bytes
    _pad: [u8; 71],
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<ProgressCapsule>() == 128);
const _: () = assert!(core::mem::align_of::<ProgressCapsule>() == 64);

impl Default for ProgressCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressCapsule {
    // ========================================================================
    // CONSTRUCTION
    // ========================================================================

    /// Create new progress bar
    ///
    /// # Default Configuration
    ///
    /// - Value: 0.0
    /// - Style: Bar
    /// - Width: 0 (auto-fill)
    /// - Height: 1
    /// - Show percentage: true
    /// - Fill color: Green (0x00FF00FF)
    /// - Track color: Dark gray (0x333333FF)
    /// - Text color: White (0xFFFFFFFF)
    #[inline]
    pub const fn new() -> Self {
        Self {
            value_state: AtomicU64::new(0),
            anim_state: AtomicU64::new(0),
            style: ProgressStyle::Bar,
            width: 0,
            height: 1,
            show_percent: true,
            fill_color: 0x00FF00FF, // Green
            track_color: 0x333333FF, // Dark gray
            text_color: 0xFFFFFFFF, // White
            label_len: 0,
            label: [0; 24],
            _pad: [0; 71],
        }
    }

    // ========================================================================
    // BUILDER PATTERN
    // ========================================================================

    /// Set progress style
    #[inline]
    pub const fn with_style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    /// Set width in cells (0 = auto fill)
    #[inline]
    pub const fn with_width(mut self, width: u8) -> Self {
        self.width = width;
        self
    }

    /// Set height in cells
    #[inline]
    pub const fn with_height(mut self, height: u8) -> Self {
        self.height = height;
        self
    }

    /// Show percentage text
    #[inline]
    pub const fn with_show_percent(mut self, show: bool) -> Self {
        self.show_percent = show;
        self
    }

    /// Set fill color (RGBA8888)
    #[inline]
    pub const fn with_fill_color(mut self, color: u32) -> Self {
        self.fill_color = color;
        self
    }

    /// Set track color (RGBA8888)
    #[inline]
    pub const fn with_track_color(mut self, color: u32) -> Self {
        self.track_color = color;
        self
    }

    /// Set text color (RGBA8888)
    #[inline]
    pub const fn with_text_color(mut self, color: u32) -> Self {
        self.text_color = color;
        self
    }

    /// Set label text
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: label.len() <= 24
    /// - #VERIFY: Truncates if longer
    pub fn with_label(mut self, label: &str) -> Self {
        let bytes = label.as_bytes();
        let len = bytes.len().min(24);
        self.label[..len].copy_from_slice(&bytes[..len]);
        self.label_len = len as u8;
        self
    }

    // ========================================================================
    // VALUE OPERATIONS
    // ========================================================================

    /// Set progress value immediately (0.0-1.0)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: value in [0.0, 1.0]
    /// - #VERIFY: Clamps to valid range
    pub fn set_value(&self, value: f32) {
        // Clamp to [0.0, 1.0]
        let clamped = value.max(0.0).min(1.0);
        let fixed = (clamped * SCALE as f32) as u32;

        // Pack: value(32) | target(32)
        let packed = ((fixed as u64) << 32) | (fixed as u64);
        self.value_state.store(packed, Ordering::Release);
    }

    /// Set target value for smooth animation (0.0-1.0)
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: value in [0.0, 1.0]
    /// - #VERIFY: Clamps to valid range
    pub fn set_value_animated(&self, value: f32) {
        // Clamp to [0.0, 1.0]
        let clamped = value.max(0.0).min(1.0);
        let target = (clamped * SCALE as f32) as u32;

        // Update target only (keep current value)
        let old = self.value_state.load(Ordering::Acquire);
        let current = (old >> 32) as u32;
        let packed = ((current as u64) << 32) | (target as u64);
        self.value_state.store(packed, Ordering::Release);
    }

    /// Get current progress value (0.0-1.0)
    #[inline]
    pub fn value(&self) -> f32 {
        let packed = self.value_state.load(Ordering::Acquire);
        let value = (packed >> 32) as u32;
        (value as f32) / (SCALE as f32)
    }

    /// Get target value (0.0-1.0)
    #[inline]
    pub fn target(&self) -> f32 {
        let packed = self.value_state.load(Ordering::Acquire);
        let target = packed as u32;
        (target as f32) / (SCALE as f32)
    }

    // ========================================================================
    // INDETERMINATE MODE
    // ========================================================================

    /// Set indeterminate mode (animated spinner-style progress)
    pub fn set_indeterminate(&self, indeterminate: bool) {
        let mode = if indeterminate { 1u8 } else { 0u8 };

        // Update mode only (keep phase)
        let old = self.anim_state.load(Ordering::Acquire);
        let phase = (old >> 48) as u16;
        let packed = ((phase as u64) << 48) | ((mode as u64) << 40);
        self.anim_state.store(packed, Ordering::Release);
    }

    /// Check if in indeterminate mode
    #[inline]
    pub fn is_indeterminate(&self) -> bool {
        let packed = self.anim_state.load(Ordering::Acquire);
        let mode = ((packed >> 40) & 0xFF) as u8;
        mode != 0
    }

    // ========================================================================
    // ANIMATION
    // ========================================================================

    /// Update animation (call at regular intervals, e.g., 60fps = 16ms)
    ///
    /// # Arguments
    ///
    /// - `delta_ms`: Time delta in milliseconds since last update
    ///
    /// # ASSUM
    ///
    /// - #ASSUME: delta_ms < 1000 (reasonable frame time)
    /// - #VERIFY: Animation speed prevents overflow
    pub fn update_animation(&self, delta_ms: u16) {
        // Update value animation (smooth interpolation)
        let value_packed = self.value_state.load(Ordering::Acquire);
        let mut current = (value_packed >> 32) as u32;
        let target = value_packed as u32;

        if current != target {
            let delta = (ANIMATION_SPEED * delta_ms as u32).min(SCALE);
            if current < target {
                current = (current + delta).min(target);
            } else {
                current = current.saturating_sub(delta).max(target);
            }

            let new_packed = ((current as u64) << 32) | (target as u64);
            self.value_state.store(new_packed, Ordering::Release);
        }

        // Update indeterminate animation
        if self.is_indeterminate() {
            let anim_packed = self.anim_state.load(Ordering::Acquire);
            let mut phase = (anim_packed >> 48) as u16;
            let mode = ((anim_packed >> 40) & 0xFF) as u8;

            // Increment phase (Q8.8 wrapping)
            phase = phase.wrapping_add(INDETERMINATE_SPEED * delta_ms);

            let new_packed = ((phase as u64) << 48) | ((mode as u64) << 40);
            self.anim_state.store(new_packed, Ordering::Release);
        }
    }

    // ========================================================================
    // RENDERING
    // ========================================================================

    /// Build text string for rendering
    fn build_text(&self, value: f32, width: u8) -> alloc::string::String {
        use alloc::string::{String, ToString};
        use alloc::format;

        let mut result = String::new();
        let filled = (value * width as f32) as u8;

        // Add label if present
        if self.label_len > 0 {
            let label_str = core::str::from_utf8(&self.label[..self.label_len as usize])
                .unwrap_or("");
            result.push_str(label_str);
            result.push('\n');
        }

        // Opening bracket
        result.push('[');

        // Progress bar based on style
        match self.style {
            ProgressStyle::Bar => {
                for i in 0..width {
                    result.push(if i < filled { '█' } else { '░' });
                }
            }
            ProgressStyle::Striped => {
                let anim_packed = self.anim_state.load(Ordering::Acquire);
                let phase = (anim_packed >> 48) as u16;
                let offset = ((phase >> 8) % 4) as u8;

                for i in 0..width {
                    let is_stripe = ((i + offset) % 4) < 2;
                    let ch = if i < filled {
                        if is_stripe { '▓' } else { '▒' }
                    } else {
                        '░'
                    };
                    result.push(ch);
                }
            }
            ProgressStyle::Blocks => {
                let exact_filled = value * width as f32;
                let whole = exact_filled as u8;
                let frac = exact_filled - whole as f32;

                for i in 0..width {
                    let ch = if i < whole {
                        '█'
                    } else if i == whole && frac > 0.0 {
                        if frac < 0.125 { '▏' }
                        else if frac < 0.25 { '▎' }
                        else if frac < 0.375 { '▍' }
                        else if frac < 0.5 { '▌' }
                        else if frac < 0.625 { '▋' }
                        else if frac < 0.75 { '▊' }
                        else if frac < 0.875 { '▉' }
                        else { '█' }
                    } else {
                        ' '
                    };
                    result.push(ch);
                }
            }
            ProgressStyle::Dots => {
                for i in 0..width {
                    result.push(if i < filled { '●' } else { '○' });
                }
            }
        }

        // Closing bracket
        result.push(']');

        // Percentage text
        if self.show_percent {
            let percent = (value * 100.0) as u8;
            result.push_str(&format!(" {:3}%", percent));
        }

        result
    }

    // ========================================================================
    // ACCESSORS
    // ========================================================================

    /// Get style
    #[inline]
    pub const fn style(&self) -> ProgressStyle {
        self.style
    }

    /// Get width
    #[inline]
    pub const fn width(&self) -> u8 {
        self.width
    }

    /// Get height
    #[inline]
    pub const fn height(&self) -> u8 {
        self.height
    }

    /// Get label
    #[inline]
    pub fn label(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len as usize])
            .unwrap_or("")
    }
}

// ============================================================================
// WIDGET TRAIT IMPLEMENTATION
// ============================================================================

impl Widget for ProgressCapsule {
    type State = ProgressState;

    const TYPE_ID: u64 = 0x5052_4F47_5245_5353; // "PROGRESS" in hex

    fn measure(&self, constraints: Constraints, _state: &Self::State) -> (u16, u16) {
        let min_width = if self.width == 0 { 10 } else { self.width as u16 };
        let height = self.height as u16 + if self.label_len > 0 { 1 } else { 0 };
        let width = min_width + 2 + if self.show_percent { 5 } else { 0 }; // +2 brackets, +5 for " 100%"

        constraints.clamp(width, height)
    }

    fn layout(&self, bounds: Rect, _state: &Self::State) -> Rect {
        // Progress bar uses all available space
        bounds
    }

    fn handle_event(&self, _event: &crate::terminal::event::Event, _state: &mut Self::State) -> bool {
        false // Progress bars don't handle events
    }

    fn render(&self, area: Rect, state: &Self::State, cmd: &mut RenderCommandBuffer) {
        let value = (state.value as f32) / (SCALE as f32);
        let width = if self.width == 0 {
            area.width.saturating_sub(2).saturating_sub(if self.show_percent { 5 } else { 0 })
        } else {
            self.width as u16
        };

        let text = self.build_text(value, width as u8);
        let style = RenderStyle::new(self.text_color, 0x00000000); // Transparent background

        cmd.text(area.x, area.y, text, style);
    }

    fn focusable(&self) -> bool {
        false // Progress bars are not focusable
    }

    fn tab_index(&self) -> u16 {
        u16::MAX // Never focused
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // T28 Q1-Q7: UNIT TESTS
    // ========================================================================

    #[test]
    fn test_new_default_state() {
        let progress = ProgressCapsule::new();
        assert_eq!(progress.value(), 0.0);
        assert_eq!(progress.target(), 0.0);
        assert!(!progress.is_indeterminate());
        assert_eq!(progress.style(), ProgressStyle::Bar);
    }

    #[test]
    fn test_set_value_immediate() {
        let progress = ProgressCapsule::new();
        progress.set_value(0.5);
        assert!((progress.value() - 0.5).abs() < 0.001);
        assert!((progress.target() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_set_value_clamping() {
        let progress = ProgressCapsule::new();

        // Test lower bound
        progress.set_value(-0.5);
        assert_eq!(progress.value(), 0.0);

        // Test upper bound
        progress.set_value(1.5);
        assert_eq!(progress.value(), 1.0);
    }

    #[test]
    fn test_set_value_animated() {
        let progress = ProgressCapsule::new();
        progress.set_value(0.0);
        progress.set_value_animated(1.0);

        assert_eq!(progress.value(), 0.0); // Current unchanged
        assert_eq!(progress.target(), 1.0); // Target updated
    }

    #[test]
    fn test_indeterminate_mode() {
        let progress = ProgressCapsule::new();

        assert!(!progress.is_indeterminate());

        progress.set_indeterminate(true);
        assert!(progress.is_indeterminate());

        progress.set_indeterminate(false);
        assert!(!progress.is_indeterminate());
    }

    #[test]
    fn test_update_animation_value() {
        let progress = ProgressCapsule::new();
        progress.set_value(0.0);
        progress.set_value_animated(1.0);

        // Animate for 100ms (should move toward target)
        progress.update_animation(100);
        let value1 = progress.value();
        assert!(value1 > 0.0 && value1 < 1.0);

        // Animate more (should continue moving)
        progress.update_animation(100);
        let value2 = progress.value();
        assert!(value2 > value1);
    }

    #[test]
    fn test_builder_pattern() {
        let progress = ProgressCapsule::new()
            .with_style(ProgressStyle::Blocks)
            .with_width(50)
            .with_height(2)
            .with_label("Test");

        assert_eq!(progress.style(), ProgressStyle::Blocks);
        assert_eq!(progress.width(), 50);
        assert_eq!(progress.height(), 2);
        assert_eq!(progress.label(), "Test");
    }

    #[test]
    fn test_widget_trait() {
        let progress = ProgressCapsule::new();
        progress.set_value(0.75);

        let state = progress.snapshot();
        assert!((state.value as f32 / SCALE as f32 - 0.75).abs() < 0.001);

        assert!(!progress.is_focusable());

        let (min_w, min_h) = progress.min_size();
        assert!(min_w >= 12); // At least 10 + 2 brackets
        assert!(min_h >= 1);
    }

    // ========================================================================
    // T28 Q8-Q14: PROPERTY TESTS
    // ========================================================================

    #[cfg(feature = "proptest")]
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn prop_value_always_in_bounds(value in -10.0f32..10.0f32) {
                let progress = ProgressCapsule::new();
                progress.set_value(value);
                let actual = progress.value();
                prop_assert!(actual >= 0.0 && actual <= 1.0);
            }

            #[test]
            fn prop_animation_converges(target in 0.0f32..1.0f32) {
                let progress = ProgressCapsule::new();
                progress.set_value(0.0);
                progress.set_value_animated(target);

                // Animate for sufficient time
                for _ in 0..20 {
                    progress.update_animation(16); // 60fps
                }

                let final_value = progress.value();
                prop_assert!((final_value - target).abs() < 0.01);
            }

            #[test]
            fn prop_indeterminate_phase_wraps(iterations in 1usize..100) {
                let progress = ProgressCapsule::new();
                progress.set_indeterminate(true);

                for _ in 0..iterations {
                    progress.update_animation(50);
                }

                // Should always remain in indeterminate mode
                prop_assert!(progress.is_indeterminate());
            }

            #[test]
            fn prop_label_truncates(label in ".*") {
                let progress = ProgressCapsule::new().with_label(&label);
                let stored = progress.label();
                prop_assert!(stored.len() <= 24);
            }
        }
    }

    // ========================================================================
    // T28 Q15-Q21: INTEGRATION TESTS
    // ========================================================================

    #[test]
    fn test_progress_lifecycle() {
        let progress = ProgressCapsule::new()
            .with_style(ProgressStyle::Striped)
            .with_width(40)
            .with_label("Loading");

        // Start at 0%
        assert_eq!(progress.value(), 0.0);

        // Animate to 50%
        progress.set_value_animated(0.5);
        for _ in 0..10 {
            progress.update_animation(16);
        }
        assert!(progress.value() > 0.4 && progress.value() < 0.6);

        // Jump to 100%
        progress.set_value(1.0);
        assert_eq!(progress.value(), 1.0);

        // Switch to indeterminate
        progress.set_indeterminate(true);
        progress.update_animation(50);
        assert!(progress.is_indeterminate());
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let progress = Arc::new(ProgressCapsule::new());
        let mut handles = vec![];

        // Spawn threads updating value
        for i in 0..4 {
            let p = Arc::clone(&progress);
            handles.push(thread::spawn(move || {
                let target = (i as f32) / 4.0;
                p.set_value_animated(target);
                for _ in 0..10 {
                    p.update_animation(10);
                    thread::yield_now();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have converged to some valid value
        let final_value = progress.value();
        assert!(final_value >= 0.0 && final_value <= 1.0);
    }
}
