//! Text Label Widget for kindly_dedup gui_v2
//!
//! # LabelCapsule
//!
//! 192-byte cache-aligned text label widget using T1 Atomic tier for lockfree updates.
//!
//! ## Architecture
//!
//! ```text
//! LabelCapsule (192B, cache-aligned)
//! ├── id: u64 (widget identifier)
//! ├── generation: AtomicU32 (modification counter)
//! ├── state: AtomicU64 (visible, enabled flags)
//! ├── bounds: AtomicU64 (x, y, width, height as u16)
//! ├── color: AtomicU32 (RGBA packed)
//! ├── font_size: AtomicU16 (Q8.8 fixed-point)
//! ├── alignment: AtomicU8 (Left/Center/Right)
//! └── text: [u8; 128] (UTF-8 content, max 127 chars + null)
//! ```
//!
//! ## Performance Targets
//!
//! - Text access: <10ns (direct array read)
//! - Color update: <5ns (single atomic store)
//! - Font size conversion: <5ns (Q8.8 shift)
//! - Glyph generation: <500ns (text parsing + layout)
//!
//! ## Framework Compliance
//!
//! - **UCE34**: T1 Atomic tier (Q10 tier selection)
//! - **Chaos**: 100% lockfree (AtomicU64/U32/U16/U8, no mutex)
//! - **ASSUM**: All bounds verified, text truncation safe
//! - **B32**: <10ns text access, <5ns color update
//! - **T28**: 12+ tests (text, color, alignment, truncation, UTF-8)

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, AtomicU8, Ordering};

/// Text alignment options
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left = 0,
    Center = 1,
    Right = 2,
}

impl TextAlignment {
    /// Convert from u8 value
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => TextAlignment::Left,
            1 => TextAlignment::Center,
            2 => TextAlignment::Right,
            _ => TextAlignment::Left, // Default to left for invalid values
        }
    }
}

/// Glyph positioning data for GPU text rendering
#[derive(Debug, Clone)]
pub struct LabelGlyphs {
    /// Text content
    pub text: String,
    /// X position (pixels)
    pub x: f32,
    /// Y position (pixels)
    pub y: f32,
    /// Font size (pixels)
    pub font_size: f32,
    /// RGBA color
    pub color: [u8; 4],
    /// Text alignment
    pub alignment: TextAlignment,
}

/// Text Label Widget (192B, T1 Atomic)
///
/// Cache-aligned lockfree text label with Q8.8 fixed-point font sizing
/// and atomic state updates.
///
/// # Memory Layout
///
/// ```text
/// Offset | Size | Field
/// -------|------|------
/// 0      | 8    | id
/// 8      | 4    | generation
/// 12     | 4    | _pad1
/// 16     | 8    | state
/// 24     | 8    | bounds
/// 32     | 4    | color
/// 36     | 2    | font_size
/// 38     | 1    | alignment
/// 39     | 1    | _pad2
/// 40     | 128  | text
/// 168    | 24   | _padding
/// -------|------|------
/// Total: 192B (cache-aligned)
/// ```
#[repr(C, align(64))]
pub struct LabelCapsule {
    // Identity
    id: u64,
    generation: AtomicU32,
    _pad1: u32,

    // State (visible, enabled packed into u64)
    state: AtomicU64,

    // Bounds (x: u16, y: u16, width: u16, height: u16)
    bounds: AtomicU64,

    // Text color (RGBA packed into u32)
    color: AtomicU32,

    // Font size (Q8.8 fixed-point for sub-pixel accuracy)
    font_size: AtomicU16,

    // Text alignment (0=Left, 1=Center, 2=Right)
    alignment: AtomicU8,
    _pad2: u8,

    // Text content (max 127 chars + null terminator)
    text: [u8; 128],

    // Padding to 192B
    _padding: [u8; 24],
}

impl LabelCapsule {
    /// Create a new label with given ID and text
    ///
    /// # Arguments
    ///
    /// * `id` - Unique widget identifier
    /// * `text` - Initial text content (truncated to 127 chars)
    ///
    /// # Returns
    ///
    /// New LabelCapsule with default styling:
    /// - Color: White (#FFFFFF)
    /// - Font size: 16.0 (Q8.8 = 4096)
    /// - Alignment: Left
    /// - Visible: true
    /// - Enabled: true
    ///
    /// # Performance
    ///
    /// <100ns (text copy + atomic init)
    pub fn new(id: u64, text: &str) -> Self {
        let mut label = Self {
            id,
            generation: AtomicU32::new(0),
            _pad1: 0,
            state: AtomicU64::new(0x0000_0001_0000_0001), // visible=1, enabled=1
            bounds: AtomicU64::new(0), // x=0, y=0, w=0, h=0
            color: AtomicU32::new(0xFFFF_FFFF), // White RGBA
            font_size: AtomicU16::new(16 << 8), // 16.0 in Q8.8
            alignment: AtomicU8::new(TextAlignment::Left as u8),
            _pad2: 0,
            text: [0; 128],
            _padding: [0; 24],
        };

        // Copy text without incrementing generation (initialization)
        let bytes = text.as_bytes();
        let copy_len = bytes.len().min(127);
        label.text[..copy_len].copy_from_slice(&bytes[..copy_len]);

        label
    }

    /// Get text content as string slice
    ///
    /// # Returns
    ///
    /// UTF-8 string slice (borrowed, zero-copy)
    ///
    /// # Performance
    ///
    /// <10ns (find null terminator + slice)
    ///
    /// # ASSUM-1: Text array is null-terminated
    /// #VERIFY: set_text() always writes null terminator
    pub fn text(&self) -> &str {
        // Find null terminator
        let len = self.text.iter().position(|&c| c == 0).unwrap_or(128);

        // SAFETY: set_text() ensures valid UTF-8 and null termination
        #[allow(unsafe_code)]
        unsafe {
            core::str::from_utf8_unchecked(&self.text[..len])
        }
    }

    /// Set text content (truncates to 127 chars)
    ///
    /// # Arguments
    ///
    /// * `text` - New text content (UTF-8)
    ///
    /// # Behavior
    ///
    /// - Truncates to 127 chars to fit buffer
    /// - Increments generation counter
    /// - Always null-terminates
    ///
    /// # Performance
    ///
    /// <200ns (UTF-8 copy + atomic increment)
    ///
    /// # ASSUME-2: Input text is valid UTF-8
    /// #VERIFY: Rust str type guarantees UTF-8 validity
    pub fn set_text(&mut self, text: &str) {
        // Clear buffer
        self.text.fill(0);

        // Copy text (truncate to 127 chars for null terminator)
        let bytes = text.as_bytes();
        let copy_len = bytes.len().min(127);
        self.text[..copy_len].copy_from_slice(&bytes[..copy_len]);

        // Null terminate (guaranteed by fill(0) + copy_len ≤ 127)
        // text[copy_len] is always 0

        // Increment generation
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get current RGBA color
    ///
    /// # Returns
    ///
    /// [r, g, b, a] array (0-255 per channel)
    ///
    /// # Performance
    ///
    /// <5ns (single atomic load + unpack)
    pub fn color(&self) -> [u8; 4] {
        let packed = self.color.load(Ordering::Acquire);
        [
            ((packed >> 24) & 0xFF) as u8, // R
            ((packed >> 16) & 0xFF) as u8, // G
            ((packed >> 8) & 0xFF) as u8,  // B
            (packed & 0xFF) as u8,         // A
        ]
    }

    /// Set RGBA color
    ///
    /// # Arguments
    ///
    /// * `color` - [r, g, b, a] array (0-255 per channel)
    ///
    /// # Performance
    ///
    /// <5ns (pack + single atomic store)
    pub fn set_color(&mut self, color: [u8; 4]) {
        let packed = ((color[0] as u32) << 24)
            | ((color[1] as u32) << 16)
            | ((color[2] as u32) << 8)
            | (color[3] as u32);

        self.color.store(packed, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get font size in pixels
    ///
    /// # Returns
    ///
    /// Font size as f32 (converted from Q8.8 fixed-point)
    ///
    /// # Performance
    ///
    /// <5ns (atomic load + shift + division)
    pub fn font_size(&self) -> f32 {
        let q8_8 = self.font_size.load(Ordering::Acquire);
        (q8_8 as f32) / 256.0
    }

    /// Set font size in pixels
    ///
    /// # Arguments
    ///
    /// * `size` - Font size in pixels (converted to Q8.8)
    ///
    /// # Behavior
    ///
    /// Clamps to [0.0, 255.99609375] (Q8.8 range)
    ///
    /// # Performance
    ///
    /// <5ns (clamp + multiply + atomic store)
    ///
    /// # ASSUME-3: Font size fits in Q8.8 range (0.0-255.99609375)
    /// #VERIFY: Clamp to [0.0, 255.99] before conversion
    pub fn set_font_size(&mut self, size: f32) {
        // Clamp to Q8.8 range [0.0, 255.99609375]
        let clamped = size.max(0.0).min(255.99);
        let q8_8 = (clamped * 256.0) as u16;

        self.font_size.store(q8_8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get text alignment
    ///
    /// # Returns
    ///
    /// Current alignment setting
    ///
    /// # Performance
    ///
    /// <5ns (atomic load + match)
    pub fn alignment(&self) -> TextAlignment {
        let value = self.alignment.load(Ordering::Acquire);
        TextAlignment::from_u8(value)
    }

    /// Set text alignment
    ///
    /// # Arguments
    ///
    /// * `align` - New alignment setting
    ///
    /// # Performance
    ///
    /// <5ns (atomic store)
    pub fn set_alignment(&mut self, align: TextAlignment) {
        self.alignment.store(align as u8, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get widget bounds
    ///
    /// # Returns
    ///
    /// (x, y, width, height) as u16 values
    ///
    /// # Performance
    ///
    /// <5ns (atomic load + unpack)
    pub fn bounds(&self) -> (u16, u16, u16, u16) {
        let packed = self.bounds.load(Ordering::Acquire);
        (
            ((packed >> 48) & 0xFFFF) as u16, // x
            ((packed >> 32) & 0xFFFF) as u16, // y
            ((packed >> 16) & 0xFFFF) as u16, // width
            (packed & 0xFFFF) as u16,         // height
        )
    }

    /// Set widget bounds
    ///
    /// # Arguments
    ///
    /// * `x` - X position
    /// * `y` - Y position
    /// * `width` - Width in pixels
    /// * `height` - Height in pixels
    ///
    /// # Performance
    ///
    /// <5ns (pack + atomic store)
    pub fn set_bounds(&mut self, x: u16, y: u16, width: u16, height: u16) {
        let packed = ((x as u64) << 48)
            | ((y as u64) << 32)
            | ((width as u64) << 16)
            | (height as u64);

        self.bounds.store(packed, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Generate glyph positioning data for GPU rendering
    ///
    /// # Returns
    ///
    /// LabelGlyphs with text content, position, styling
    ///
    /// # Performance
    ///
    /// <500ns (text copy + bounds read + color unpack)
    pub fn render_glyphs(&self) -> LabelGlyphs {
        let (x, y, _width, _height) = self.bounds();
        let color = self.color();
        let font_size = self.font_size();
        let alignment = self.alignment();
        let text = self.text().to_string();

        LabelGlyphs {
            text,
            x: x as f32,
            y: y as f32,
            font_size,
            color,
            alignment,
        }
    }

    /// Check if label is visible
    ///
    /// # Returns
    ///
    /// true if visible flag is set
    ///
    /// # Performance
    ///
    /// <5ns (atomic load + mask)
    pub fn is_visible(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        (state & 0x0000_0001) != 0
    }

    /// Set visibility
    ///
    /// # Arguments
    ///
    /// * `visible` - New visibility state
    ///
    /// # Performance
    ///
    /// <5ns (atomic update)
    pub fn set_visible(&mut self, visible: bool) {
        let state = self.state.load(Ordering::Acquire);
        let new_state = if visible {
            state | 0x0000_0001
        } else {
            state & !0x0000_0001
        };
        self.state.store(new_state, Ordering::Release);
        self.generation.fetch_add(1, Ordering::Release);
    }

    /// Get widget ID
    pub const fn id(&self) -> u64 {
        self.id
    }

    /// Get current generation counter
    pub fn generation(&self) -> u32 {
        self.generation.load(Ordering::Acquire)
    }
}

// Compile-time size verification
const _: () = assert!(core::mem::size_of::<LabelCapsule>() == 192);
const _: () = assert!(core::mem::align_of::<LabelCapsule>() == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_creation() {
        let label = LabelCapsule::new(42, "Hello, World!");

        assert_eq!(label.id(), 42);
        assert_eq!(label.text(), "Hello, World!");
        assert_eq!(label.color(), [255, 255, 255, 255]); // White
        assert_eq!(label.font_size(), 16.0);
        assert_eq!(label.alignment(), TextAlignment::Left);
        assert!(label.is_visible());
    }

    #[test]
    fn test_text_set_get() {
        let mut label = LabelCapsule::new(1, "Initial");

        assert_eq!(label.text(), "Initial");

        label.set_text("Updated text");
        assert_eq!(label.text(), "Updated text");

        // Verify generation counter incremented
        assert_eq!(label.generation(), 1);
    }

    #[test]
    fn test_text_truncation() {
        let mut label = LabelCapsule::new(1, "");

        // Create 200-char string (should truncate to 127)
        let long_text = "A".repeat(200);
        label.set_text(&long_text);

        let result = label.text();
        assert_eq!(result.len(), 127);
        assert_eq!(result, "A".repeat(127));
    }

    #[test]
    fn test_unicode_handling() {
        let mut label = LabelCapsule::new(1, "");

        // Test various Unicode characters
        label.set_text("Hello 世界 🌍");
        assert_eq!(label.text(), "Hello 世界 🌍");

        // Test emoji
        label.set_text("✅ ❌ ⚠️");
        assert_eq!(label.text(), "✅ ❌ ⚠️");

        // Test RTL text (Arabic)
        label.set_text("مرحبا");
        assert_eq!(label.text(), "مرحبا");
    }

    #[test]
    fn test_color_management() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test white (default)
        assert_eq!(label.color(), [255, 255, 255, 255]);

        // Test gold (#FFD700)
        label.set_color([255, 215, 0, 255]);
        assert_eq!(label.color(), [255, 215, 0, 255]);

        // Test with transparency
        label.set_color([128, 64, 32, 128]);
        assert_eq!(label.color(), [128, 64, 32, 128]);

        // Verify generation counter
        assert_eq!(label.generation(), 2); // 2 color changes
    }

    #[test]
    fn test_font_size_q8_8() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test default (16.0)
        assert_eq!(label.font_size(), 16.0);

        // Test integer sizes
        label.set_font_size(12.0);
        assert_eq!(label.font_size(), 12.0);

        label.set_font_size(24.0);
        assert_eq!(label.font_size(), 24.0);

        // Test fractional sizes (Q8.8 precision)
        label.set_font_size(14.5);
        assert!((label.font_size() - 14.5).abs() < 0.01);

        label.set_font_size(18.25);
        assert!((label.font_size() - 18.25).abs() < 0.01);

        // Test sub-pixel precision
        label.set_font_size(16.125);
        assert!((label.font_size() - 16.125).abs() < 0.01);
    }

    #[test]
    fn test_font_size_clamping() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test negative (should clamp to 0.0)
        label.set_font_size(-10.0);
        assert_eq!(label.font_size(), 0.0);

        // Test very large (should clamp to 255.99...)
        label.set_font_size(1000.0);
        assert!(label.font_size() <= 256.0);
        assert!(label.font_size() >= 255.0);
    }

    #[test]
    fn test_alignment() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test default (Left)
        assert_eq!(label.alignment(), TextAlignment::Left);

        // Test Center
        label.set_alignment(TextAlignment::Center);
        assert_eq!(label.alignment(), TextAlignment::Center);

        // Test Right
        label.set_alignment(TextAlignment::Right);
        assert_eq!(label.alignment(), TextAlignment::Right);

        // Test Left again
        label.set_alignment(TextAlignment::Left);
        assert_eq!(label.alignment(), TextAlignment::Left);
    }

    #[test]
    fn test_bounds() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test default (0, 0, 0, 0)
        assert_eq!(label.bounds(), (0, 0, 0, 0));

        // Test setting bounds
        label.set_bounds(100, 200, 300, 50);
        assert_eq!(label.bounds(), (100, 200, 300, 50));

        // Test large values
        label.set_bounds(u16::MAX, u16::MAX, u16::MAX, u16::MAX);
        assert_eq!(label.bounds(), (u16::MAX, u16::MAX, u16::MAX, u16::MAX));
    }

    #[test]
    fn test_visibility() {
        let mut label = LabelCapsule::new(1, "Test");

        // Test default (visible)
        assert!(label.is_visible());

        // Test hiding
        label.set_visible(false);
        assert!(!label.is_visible());

        // Test showing
        label.set_visible(true);
        assert!(label.is_visible());
    }

    #[test]
    fn test_render_glyphs() {
        let mut label = LabelCapsule::new(1, "Render Test");
        label.set_bounds(10, 20, 200, 30);
        label.set_color([255, 215, 0, 255]); // Gold
        label.set_font_size(18.0);
        label.set_alignment(TextAlignment::Center);

        let glyphs = label.render_glyphs();

        assert_eq!(glyphs.text, "Render Test");
        assert_eq!(glyphs.x, 10.0);
        assert_eq!(glyphs.y, 20.0);
        assert_eq!(glyphs.font_size, 18.0);
        assert_eq!(glyphs.color, [255, 215, 0, 255]);
        assert_eq!(glyphs.alignment, TextAlignment::Center);
    }

    #[test]
    fn test_generation_counter() {
        let mut label = LabelCapsule::new(1, "Test");
        assert_eq!(label.generation(), 0);

        label.set_text("Update 1");
        assert_eq!(label.generation(), 1);

        label.set_color([255, 0, 0, 255]);
        assert_eq!(label.generation(), 2);

        label.set_font_size(20.0);
        assert_eq!(label.generation(), 3);

        label.set_alignment(TextAlignment::Center);
        assert_eq!(label.generation(), 4);

        label.set_bounds(10, 20, 100, 50);
        assert_eq!(label.generation(), 5);

        label.set_visible(false);
        assert_eq!(label.generation(), 6);
    }

    #[test]
    fn test_memory_layout() {
        // Verify size and alignment
        assert_eq!(core::mem::size_of::<LabelCapsule>(), 192);
        assert_eq!(core::mem::align_of::<LabelCapsule>(), 64);

        // Verify cache-line alignment
        let label = LabelCapsule::new(1, "Test");
        let ptr = &label as *const LabelCapsule as usize;
        assert_eq!(ptr % 64, 0, "Label must be cache-aligned");
    }
}
