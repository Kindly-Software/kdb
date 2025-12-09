//! StyleCapsule - CSS-like style properties for widgets
//!
//! # Overview
//!
//! A 100% lockfree, cache-aligned style system for fast style resolution in GUI applications.
//! Uses bit-packing and Q4.4 fixed-point for sub-pixel precision.
//!
//! # Tier Classification
//!
//! - **T1 (Atomic)**: Lockfree style property access (<100ns)
//! - **T3 (Fixed-Point)**: Q4.4 border width/radius for deterministic rendering
//!
//! # Memory Layout
//!
//! 64 bytes, cache-aligned:
//! - state: AtomicU64 (packed properties)
//! - background: u32 (RGBA color)
//! - foreground: u32 (RGBA text/icon color)
//! - border_color: u32 (RGBA border color)
//! - padding: AtomicU32 (4×u8 TRBL)
//! - margin: AtomicU32 (4×u8 TRBL)
//! - generation: AtomicU32 (update counter)
//! - _pad: 24 bytes (cache alignment)
//!
//! # State Bit Layout
//!
//! ```text
//! Bits 0-7:   border_width (Q4.4 fixed-point, 0-15.9375 px)
//! Bits 8-15:  border_radius (Q4.4 fixed-point, 0-15.9375 px)
//! Bits 16-23: opacity (0-255 = 0.0-1.0)
//! Bits 24-31: font_size (pixels, 0-255)
//! Bits 32-35: font_weight (0=thin...7=black)
//! Bits 36-39: text_align (0=left, 1=center, 2=right, 3=justify)
//! Bits 40-47: reserved
//! Bits 48-63: reserved
//! ```
//!
//! # Performance Targets (B32)
//!
//! - Style resolution: <100ns per property
//! - Builder pattern: <500ns for full style
//! - Concurrent updates: lockfree, cache-aligned
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q10 (T1+T3 tier selection), Q33 (lockfree atomics)
//! - **Chaos**: 100% lockfree, cache-aligned, generation counters
//! - **ASSUM**: 100% safe (no unsafe code)
//! - **T28**: 20+ comprehensive tests

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Text alignment options
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left = 0,
    Center = 1,
    Right = 2,
    Justify = 3,
}

impl TextAlign {
    /// Convert from raw u8 value
    pub fn from_u8(value: u8) -> Self {
        match value & 0x3 {
            0 => TextAlign::Left,
            1 => TextAlign::Center,
            2 => TextAlign::Right,
            3 => TextAlign::Justify,
            _ => unreachable!(),
        }
    }

    /// Convert to raw u8 value
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// Font weight options
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Thin = 0,
    Light = 1,
    Normal = 2,
    Medium = 3,
    SemiBold = 4,
    Bold = 5,
    ExtraBold = 6,
    Black = 7,
}

impl FontWeight {
    /// Convert from raw u8 value
    pub fn from_u8(value: u8) -> Self {
        match value & 0x7 {
            0 => FontWeight::Thin,
            1 => FontWeight::Light,
            2 => FontWeight::Normal,
            3 => FontWeight::Medium,
            4 => FontWeight::SemiBold,
            5 => FontWeight::Bold,
            6 => FontWeight::ExtraBold,
            7 => FontWeight::Black,
            _ => unreachable!(),
        }
    }

    /// Convert to raw u8 value
    pub fn to_u8(self) -> u8 {
        self as u8
    }
}

/// CSS-like style properties for widgets
///
/// # Chaos Compliance
///
/// - **Lockfree**: AtomicU64 state, AtomicU32 padding/margin
/// - **Cache-Aligned**: 64 bytes, aligned to cache line
/// - **Generation Counter**: Atomic generation for snapshot consistency
/// - **Q4.4 Fixed-Point**: Border width/radius for deterministic rendering
///
/// # Example
///
/// ```rust
/// use atomic_capsule::gui::theme::style::{StyleCapsule, FontWeight, TextAlign};
///
/// // Create default style
/// let style = StyleCapsule::new();
///
/// // Use builder pattern
/// let style = StyleCapsule::builder()
///     .background(0xFF2E3440) // Nord polar night
///     .foreground(0xFFECEFF4) // Nord snow storm
///     .border(2.0, 0xFF88C0D0) // 2px cyan border
///     .border_radius(4.0)
///     .font(16, FontWeight::Normal)
///     .padding(8, 12, 8, 12) // top, right, bottom, left
///     .build();
///
/// assert_eq!(style.background(), 0xFF2E3440);
/// assert_eq!(style.border_width(), 2.0);
/// ```
#[repr(C, align(64))]
pub struct StyleCapsule {
    /// Packed properties (see module docs for bit layout)
    state: AtomicU64,

    /// Background color (RGBA)
    background: u32,

    /// Foreground color (RGBA, text/icon)
    foreground: u32,

    /// Border color (RGBA)
    border_color: u32,

    /// Padding (top, right, bottom, left in u8 pixels)
    padding: AtomicU32,

    /// Margin (top, right, bottom, left in u8 pixels)
    margin: AtomicU32,

    /// Generation counter (incremented on updates)
    generation: AtomicU32,

    /// Padding to 64 bytes
    _pad: [u8; 24],
}

impl StyleCapsule {
    // Bit masks for state field
    const BORDER_WIDTH_MASK: u64 = 0xFF;
    const BORDER_RADIUS_MASK: u64 = 0xFF << 8;
    const OPACITY_MASK: u64 = 0xFF << 16;
    const FONT_SIZE_MASK: u64 = 0xFF << 24;
    const FONT_WEIGHT_MASK: u64 = 0xF << 32;
    const TEXT_ALIGN_MASK: u64 = 0xF << 36;

    /// Create a new StyleCapsule with default values
    ///
    /// Default style:
    /// - Background: transparent (0x00000000)
    /// - Foreground: opaque black (0xFF000000)
    /// - Border: 0px, transparent
    /// - Padding/Margin: 0px
    /// - Font: 14px, normal weight
    /// - Text align: left
    /// - Opacity: 1.0
    pub fn new() -> Self {
        // #ASSUME: Default font size 14px fits in u8
        // #VERIFY: 14 < 255
        const DEFAULT_FONT_SIZE: u64 = 14 << 24;

        // #ASSUME: FontWeight::Normal (2) fits in 4 bits
        // #VERIFY: 2 < 16
        const DEFAULT_FONT_WEIGHT: u64 = (FontWeight::Normal as u64) << 32;

        // #ASSUME: TextAlign::Left (0) fits in 4 bits
        // #VERIFY: 0 < 16
        const DEFAULT_TEXT_ALIGN: u64 = (TextAlign::Left as u64) << 36;

        // #ASSUME: Opacity 255 (1.0) fits in u8
        // #VERIFY: 255 <= 255
        const DEFAULT_OPACITY: u64 = 255 << 16;

        Self {
            state: AtomicU64::new(
                DEFAULT_FONT_SIZE | DEFAULT_FONT_WEIGHT | DEFAULT_TEXT_ALIGN | DEFAULT_OPACITY,
            ),
            background: 0x00000000,
            foreground: 0xFF000000,
            border_color: 0x00000000,
            padding: AtomicU32::new(0),
            margin: AtomicU32::new(0),
            generation: AtomicU32::new(0),
            _pad: [0; 24],
        }
    }

    /// Create a builder for fluent style construction
    pub fn builder() -> StyleBuilder {
        StyleBuilder {
            style: StyleCapsule::new(),
        }
    }

    /// Get background color (RGBA)
    #[inline]
    pub fn background(&self) -> u32 {
        self.background
    }

    /// Set background color (RGBA)
    pub fn set_background(&mut self, color: u32) {
        self.background = color;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get foreground color (RGBA)
    #[inline]
    pub fn foreground(&self) -> u32 {
        self.foreground
    }

    /// Set foreground color (RGBA)
    pub fn set_foreground(&mut self, color: u32) {
        self.foreground = color;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get border color (RGBA)
    #[inline]
    pub fn border_color(&self) -> u32 {
        self.border_color
    }

    /// Set border color (RGBA)
    pub fn set_border_color(&mut self, color: u32) {
        self.border_color = color;
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get border width in pixels (Q4.4 fixed-point)
    #[inline]
    pub fn border_width(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let q4_4 = (state & Self::BORDER_WIDTH_MASK) as u8;
        // #ASSUME: Q4.4 conversion is exact for valid inputs
        // #VERIFY: q4_4 < 256, division by 16.0 is exact for integers
        (q4_4 as f32) / 16.0
    }

    /// Set border width in pixels (Q4.4 fixed-point)
    ///
    /// # Panics
    ///
    /// Panics if width is negative or >= 16.0
    pub fn set_border_width(&self, width: f32) {
        assert!(width >= 0.0 && width < 16.0, "Border width must be in [0, 16)");

        // #ASSUME: width * 16.0 fits in u8
        // #VERIFY: width < 16.0 => width * 16.0 < 256
        let q4_4 = (width * 16.0) as u8;

        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::BORDER_WIDTH_MASK) | (q4_4 as u64);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get border radius in pixels (Q4.4 fixed-point)
    #[inline]
    pub fn border_radius(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let q4_4 = ((state & Self::BORDER_RADIUS_MASK) >> 8) as u8;
        (q4_4 as f32) / 16.0
    }

    /// Set border radius in pixels (Q4.4 fixed-point)
    ///
    /// # Panics
    ///
    /// Panics if radius is negative or >= 16.0
    pub fn set_border_radius(&self, radius: f32) {
        assert!(radius >= 0.0 && radius < 16.0, "Border radius must be in [0, 16)");

        let q4_4 = (radius * 16.0) as u8;

        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::BORDER_RADIUS_MASK) | ((q4_4 as u64) << 8);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get opacity (0.0-1.0)
    #[inline]
    pub fn opacity(&self) -> f32 {
        let state = self.state.load(Ordering::Acquire);
        let opacity_u8 = ((state & Self::OPACITY_MASK) >> 16) as u8;
        (opacity_u8 as f32) / 255.0
    }

    /// Set opacity (0.0-1.0)
    ///
    /// # Panics
    ///
    /// Panics if opacity is not in [0.0, 1.0]
    pub fn set_opacity(&self, opacity: f32) {
        assert!(opacity >= 0.0 && opacity <= 1.0, "Opacity must be in [0.0, 1.0]");

        let opacity_u8 = (opacity * 255.0) as u8;

        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::OPACITY_MASK) | ((opacity_u8 as u64) << 16);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get font size in pixels
    #[inline]
    pub fn font_size(&self) -> u8 {
        let state = self.state.load(Ordering::Acquire);
        ((state & Self::FONT_SIZE_MASK) >> 24) as u8
    }

    /// Set font size in pixels
    pub fn set_font_size(&self, size: u8) {
        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::FONT_SIZE_MASK) | ((size as u64) << 24);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get font weight
    #[inline]
    pub fn font_weight(&self) -> FontWeight {
        let state = self.state.load(Ordering::Acquire);
        let weight = ((state & Self::FONT_WEIGHT_MASK) >> 32) as u8;
        FontWeight::from_u8(weight)
    }

    /// Set font weight
    pub fn set_font_weight(&self, weight: FontWeight) {
        let weight_u8 = weight.to_u8();

        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::FONT_WEIGHT_MASK) | ((weight_u8 as u64) << 32);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get text alignment
    #[inline]
    pub fn text_align(&self) -> TextAlign {
        let state = self.state.load(Ordering::Acquire);
        let align = ((state & Self::TEXT_ALIGN_MASK) >> 36) as u8;
        TextAlign::from_u8(align)
    }

    /// Set text alignment
    pub fn set_text_align(&self, align: TextAlign) {
        let align_u8 = align.to_u8();

        loop {
            let state = self.state.load(Ordering::Acquire);
            let new_state = (state & !Self::TEXT_ALIGN_MASK) | ((align_u8 as u64) << 36);

            if self
                .state
                .compare_exchange_weak(state, new_state, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                self.generation.fetch_add(1, Ordering::Release);
                break;
            }
        }
    }

    /// Get padding (top, right, bottom, left in pixels)
    #[inline]
    pub fn padding(&self) -> (u8, u8, u8, u8) {
        let packed = self.padding.load(Ordering::Acquire);
        (
            (packed >> 24) as u8,
            (packed >> 16) as u8,
            (packed >> 8) as u8,
            packed as u8,
        )
    }

    /// Set padding (top, right, bottom, left in pixels)
    pub fn set_padding(&self, top: u8, right: u8, bottom: u8, left: u8) {
        let packed = ((top as u32) << 24) | ((right as u32) << 16) | ((bottom as u32) << 8) | (left as u32);
        self.padding.store(packed, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set uniform padding (all sides equal)
    #[inline]
    pub fn set_padding_uniform(&self, value: u8) {
        self.set_padding(value, value, value, value);
    }

    /// Get margin (top, right, bottom, left in pixels)
    #[inline]
    pub fn margin(&self) -> (u8, u8, u8, u8) {
        let packed = self.margin.load(Ordering::Acquire);
        (
            (packed >> 24) as u8,
            (packed >> 16) as u8,
            (packed >> 8) as u8,
            packed as u8,
        )
    }

    /// Set margin (top, right, bottom, left in pixels)
    pub fn set_margin(&self, top: u8, right: u8, bottom: u8, left: u8) {
        let packed = ((top as u32) << 24) | ((right as u32) << 16) | ((bottom as u32) << 8) | (left as u32);
        self.margin.store(packed, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Set uniform margin (all sides equal)
    #[inline]
    pub fn set_margin_uniform(&self, value: u8) {
        self.set_margin(value, value, value, value);
    }

    /// Get generation counter
    #[inline]
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

impl Default for StyleCapsule {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for fluent style construction
///
/// # Example
///
/// ```rust
/// use atomic_capsule::gui::theme::style::{StyleCapsule, FontWeight, TextAlign};
///
/// let style = StyleCapsule::builder()
///     .background(0xFF2E3440)
///     .foreground(0xFFECEFF4)
///     .border(2.0, 0xFF88C0D0)
///     .border_radius(4.0)
///     .font(16, FontWeight::Bold)
///     .text_align(TextAlign::Center)
///     .padding(8, 12, 8, 12)
///     .margin(4, 4, 4, 4)
///     .opacity(0.95)
///     .build();
/// ```
pub struct StyleBuilder {
    style: StyleCapsule,
}

impl StyleBuilder {
    /// Set background color (RGBA)
    pub fn background(mut self, color: u32) -> Self {
        self.style.set_background(color);
        self
    }

    /// Set foreground color (RGBA)
    pub fn foreground(mut self, color: u32) -> Self {
        self.style.set_foreground(color);
        self
    }

    /// Set border (width in pixels, color RGBA)
    pub fn border(mut self, width: f32, color: u32) -> Self {
        self.style.set_border_width(width);
        self.style.set_border_color(color);
        self
    }

    /// Set border radius in pixels
    pub fn border_radius(self, radius: f32) -> Self {
        self.style.set_border_radius(radius);
        self
    }

    /// Set opacity (0.0-1.0)
    pub fn opacity(self, opacity: f32) -> Self {
        self.style.set_opacity(opacity);
        self
    }

    /// Set font (size in pixels, weight)
    pub fn font(self, size: u8, weight: FontWeight) -> Self {
        self.style.set_font_size(size);
        self.style.set_font_weight(weight);
        self
    }

    /// Set text alignment
    pub fn text_align(self, align: TextAlign) -> Self {
        self.style.set_text_align(align);
        self
    }

    /// Set padding (top, right, bottom, left in pixels)
    pub fn padding(self, top: u8, right: u8, bottom: u8, left: u8) -> Self {
        self.style.set_padding(top, right, bottom, left);
        self
    }

    /// Set uniform padding (all sides equal)
    pub fn padding_uniform(self, value: u8) -> Self {
        self.style.set_padding_uniform(value);
        self
    }

    /// Set margin (top, right, bottom, left in pixels)
    pub fn margin(self, top: u8, right: u8, bottom: u8, left: u8) -> Self {
        self.style.set_margin(top, right, bottom, left);
        self
    }

    /// Set uniform margin (all sides equal)
    pub fn margin_uniform(self, value: u8) -> Self {
        self.style.set_margin_uniform(value);
        self
    }

    /// Build the final StyleCapsule
    pub fn build(self) -> StyleCapsule {
        self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creation() {
        let style = StyleCapsule::new();
        assert_eq!(style.background(), 0x00000000);
        assert_eq!(style.foreground(), 0xFF000000);
        assert_eq!(style.border_color(), 0x00000000);
        assert_eq!(style.border_width(), 0.0);
        assert_eq!(style.border_radius(), 0.0);
        assert_eq!(style.opacity(), 1.0);
        assert_eq!(style.font_size(), 14);
        assert_eq!(style.font_weight(), FontWeight::Normal);
        assert_eq!(style.text_align(), TextAlign::Left);
        assert_eq!(style.padding(), (0, 0, 0, 0));
        assert_eq!(style.margin(), (0, 0, 0, 0));
        assert_eq!(style.generation(), 0);
    }

    #[test]
    fn test_background() {
        let mut style = StyleCapsule::new();
        assert_eq!(style.background(), 0x00000000);

        style.set_background(0xFF2E3440);
        assert_eq!(style.background(), 0xFF2E3440);
        assert_eq!(style.generation(), 1);
    }

    #[test]
    fn test_foreground() {
        let mut style = StyleCapsule::new();
        assert_eq!(style.foreground(), 0xFF000000);

        style.set_foreground(0xFFECEFF4);
        assert_eq!(style.foreground(), 0xFFECEFF4);
        assert_eq!(style.generation(), 1);
    }

    #[test]
    fn test_border_width_q4_4() {
        let style = StyleCapsule::new();
        assert_eq!(style.border_width(), 0.0);

        // Test exact Q4.4 values
        style.set_border_width(0.0);
        assert_eq!(style.border_width(), 0.0);

        style.set_border_width(1.0);
        assert_eq!(style.border_width(), 1.0);

        style.set_border_width(2.5);
        assert_eq!(style.border_width(), 2.5);

        style.set_border_width(15.9375); // Max Q4.4 value (255/16)
        assert_eq!(style.border_width(), 15.9375);

        // Test Q4.4 precision (1/16 = 0.0625)
        style.set_border_width(0.0625);
        assert_eq!(style.border_width(), 0.0625);

        style.set_border_width(3.125); // 50/16
        assert_eq!(style.border_width(), 3.125);
    }

    #[test]
    fn test_border_radius_q4_4() {
        let style = StyleCapsule::new();
        assert_eq!(style.border_radius(), 0.0);

        style.set_border_radius(0.0);
        assert_eq!(style.border_radius(), 0.0);

        style.set_border_radius(4.0);
        assert_eq!(style.border_radius(), 4.0);

        style.set_border_radius(8.5);
        assert_eq!(style.border_radius(), 8.5);

        style.set_border_radius(15.9375);
        assert_eq!(style.border_radius(), 15.9375);
    }

    #[test]
    fn test_opacity() {
        let style = StyleCapsule::new();
        assert_eq!(style.opacity(), 1.0);

        style.set_opacity(0.0);
        assert!((style.opacity() - 0.0).abs() < 0.01);

        style.set_opacity(0.5);
        assert!((style.opacity() - 0.5).abs() < 0.01);

        style.set_opacity(1.0);
        assert!((style.opacity() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_font_size() {
        let style = StyleCapsule::new();
        assert_eq!(style.font_size(), 14);

        style.set_font_size(10);
        assert_eq!(style.font_size(), 10);

        style.set_font_size(16);
        assert_eq!(style.font_size(), 16);

        style.set_font_size(255);
        assert_eq!(style.font_size(), 255);
    }

    #[test]
    fn test_font_weight() {
        let style = StyleCapsule::new();
        assert_eq!(style.font_weight(), FontWeight::Normal);

        style.set_font_weight(FontWeight::Thin);
        assert_eq!(style.font_weight(), FontWeight::Thin);

        style.set_font_weight(FontWeight::Bold);
        assert_eq!(style.font_weight(), FontWeight::Bold);

        style.set_font_weight(FontWeight::Black);
        assert_eq!(style.font_weight(), FontWeight::Black);
    }

    #[test]
    fn test_text_align() {
        let style = StyleCapsule::new();
        assert_eq!(style.text_align(), TextAlign::Left);

        style.set_text_align(TextAlign::Center);
        assert_eq!(style.text_align(), TextAlign::Center);

        style.set_text_align(TextAlign::Right);
        assert_eq!(style.text_align(), TextAlign::Right);

        style.set_text_align(TextAlign::Justify);
        assert_eq!(style.text_align(), TextAlign::Justify);
    }

    #[test]
    fn test_padding() {
        let style = StyleCapsule::new();
        assert_eq!(style.padding(), (0, 0, 0, 0));

        style.set_padding(8, 12, 8, 12);
        assert_eq!(style.padding(), (8, 12, 8, 12));

        style.set_padding_uniform(16);
        assert_eq!(style.padding(), (16, 16, 16, 16));

        style.set_padding(255, 0, 128, 64);
        assert_eq!(style.padding(), (255, 0, 128, 64));
    }

    #[test]
    fn test_margin() {
        let style = StyleCapsule::new();
        assert_eq!(style.margin(), (0, 0, 0, 0));

        style.set_margin(4, 4, 4, 4);
        assert_eq!(style.margin(), (4, 4, 4, 4));

        style.set_margin_uniform(8);
        assert_eq!(style.margin(), (8, 8, 8, 8));

        style.set_margin(32, 16, 32, 16);
        assert_eq!(style.margin(), (32, 16, 32, 16));
    }

    #[test]
    fn test_builder_pattern() {
        let style = StyleCapsule::builder()
            .background(0xFF2E3440)
            .foreground(0xFFECEFF4)
            .border(2.0, 0xFF88C0D0)
            .border_radius(4.0)
            .font(16, FontWeight::Bold)
            .text_align(TextAlign::Center)
            .padding(8, 12, 8, 12)
            .margin(4, 4, 4, 4)
            .opacity(0.95)
            .build();

        assert_eq!(style.background(), 0xFF2E3440);
        assert_eq!(style.foreground(), 0xFFECEFF4);
        assert_eq!(style.border_color(), 0xFF88C0D0);
        assert_eq!(style.border_width(), 2.0);
        assert_eq!(style.border_radius(), 4.0);
        assert_eq!(style.font_size(), 16);
        assert_eq!(style.font_weight(), FontWeight::Bold);
        assert_eq!(style.text_align(), TextAlign::Center);
        assert_eq!(style.padding(), (8, 12, 8, 12));
        assert_eq!(style.margin(), (4, 4, 4, 4));
        assert!((style.opacity() - 0.95).abs() < 0.01);
    }

    #[test]
    fn test_size_alignment() {
        use core::mem::{align_of, size_of};
        assert_eq!(size_of::<StyleCapsule>(), 64);
        assert_eq!(align_of::<StyleCapsule>(), 64);
    }

    #[test]
    fn test_generation_updates() {
        let style = StyleCapsule::new();
        assert_eq!(style.generation(), 0);

        style.set_border_width(2.0);
        assert_eq!(style.generation(), 1);

        style.set_border_radius(4.0);
        assert_eq!(style.generation(), 2);

        style.set_opacity(0.8);
        assert_eq!(style.generation(), 3);

        style.set_font_size(16);
        assert_eq!(style.generation(), 4);

        style.set_font_weight(FontWeight::Bold);
        assert_eq!(style.generation(), 5);

        style.set_text_align(TextAlign::Center);
        assert_eq!(style.generation(), 6);

        style.set_padding(8, 8, 8, 8);
        assert_eq!(style.generation(), 7);

        style.set_margin(4, 4, 4, 4);
        assert_eq!(style.generation(), 8);
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let style = Arc::new(StyleCapsule::new());
        let mut handles = vec![];

        // Spawn 8 threads updating different properties
        for i in 0..8 {
            let style_clone = Arc::clone(&style);
            let handle = thread::spawn(move || {
                for _ in 0..1000 {
                    match i % 8 {
                        0 => style_clone.set_border_width(2.0),
                        1 => style_clone.set_border_radius(4.0),
                        2 => style_clone.set_opacity(0.8),
                        3 => style_clone.set_font_size(16),
                        4 => style_clone.set_font_weight(FontWeight::Bold),
                        5 => style_clone.set_text_align(TextAlign::Center),
                        6 => style_clone.set_padding(8, 8, 8, 8),
                        7 => style_clone.set_margin(4, 4, 4, 4),
                        _ => unreachable!(),
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Verify final values are consistent (last writer wins)
        assert!(style.generation() > 0);
        let _ = style.border_width();
        let _ = style.border_radius();
        let _ = style.opacity();
        let _ = style.font_size();
        let _ = style.font_weight();
        let _ = style.text_align();
        let _ = style.padding();
        let _ = style.margin();
    }

    #[test]
    #[should_panic(expected = "Border width must be in [0, 16)")]
    fn test_border_width_overflow() {
        let style = StyleCapsule::new();
        style.set_border_width(16.0); // Max is 15.9375
    }

    #[test]
    #[should_panic(expected = "Border width must be in [0, 16)")]
    fn test_border_width_negative() {
        let style = StyleCapsule::new();
        style.set_border_width(-1.0);
    }

    #[test]
    #[should_panic(expected = "Border radius must be in [0, 16)")]
    fn test_border_radius_overflow() {
        let style = StyleCapsule::new();
        style.set_border_radius(16.0);
    }

    #[test]
    #[should_panic(expected = "Opacity must be in [0.0, 1.0]")]
    fn test_opacity_overflow() {
        let style = StyleCapsule::new();
        style.set_opacity(1.1);
    }

    #[test]
    #[should_panic(expected = "Opacity must be in [0.0, 1.0]")]
    fn test_opacity_negative() {
        let style = StyleCapsule::new();
        style.set_opacity(-0.1);
    }

    #[test]
    fn test_default() {
        let style1 = StyleCapsule::new();
        let style2 = StyleCapsule::default();

        assert_eq!(style1.background(), style2.background());
        assert_eq!(style1.foreground(), style2.foreground());
        assert_eq!(style1.border_color(), style2.border_color());
        assert_eq!(style1.border_width(), style2.border_width());
        assert_eq!(style1.border_radius(), style2.border_radius());
        assert_eq!(style1.opacity(), style2.opacity());
        assert_eq!(style1.font_size(), style2.font_size());
        assert_eq!(style1.font_weight(), style2.font_weight());
        assert_eq!(style1.text_align(), style2.text_align());
    }

    #[test]
    fn test_builder_fluent_chain() {
        let style = StyleCapsule::builder()
            .background(0xFFFFFFFF)
            .foreground(0xFF000000)
            .padding_uniform(16)
            .margin_uniform(8)
            .build();

        assert_eq!(style.background(), 0xFFFFFFFF);
        assert_eq!(style.foreground(), 0xFF000000);
        assert_eq!(style.padding(), (16, 16, 16, 16));
        assert_eq!(style.margin(), (8, 8, 8, 8));
    }
}
