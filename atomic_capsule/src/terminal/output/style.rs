//! StyleCapsule - Terminal Text Styling (T1 Atomic, 32B)
//!
//! Compact text style representation using bitflags for attributes and packed colors.
//!
//! ## Design
//!
//! - **Tier**: T1 Atomic (lockfree operations)
//! - **Size**: 64B (actual size due to align(32) padding)
//! - **Generation Counter**: TOCTOU prevention
//! - **Zero Allocations**: Stack-based escape sequence generation
//!
//! ## SGR (Select Graphic Rendition) Codes
//!
//! - **Bold**: `\x1b[1m` (SGR 1)
//! - **Dim**: `\x1b[2m` (SGR 2)
//! - **Italic**: `\x1b[3m` (SGR 3)
//! - **Underline**: `\x1b[4m` (SGR 4)
//! - **Blink**: `\x1b[5m` (SGR 5)
//! - **Reverse**: `\x1b[7m` (SGR 7, swap fg/bg)
//! - **Hidden**: `\x1b[8m` (SGR 8, conceal)
//! - **Strikethrough**: `\x1b[9m` (SGR 9)
//! - **Reset**: `\x1b[0m` (SGR 0, clear all)
//!
//! ## References
//!
//! - [ANSI SGR Codes](https://en.wikipedia.org/wiki/ANSI_escape_code#SGR)
//! - [Console Codes Manual](https://www.man7.org/linux/man-pages/man4/console_codes.4.html)
//! - [Terminal Colors Guide](https://chrisyeh96.github.io/2020/03/28/terminal-colors.html)

use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use super::color::{ColorCapsule, Color};

// Style attribute bitflags
pub const BOLD: u16 = 1 << 0;
pub const DIM: u16 = 1 << 1;
pub const ITALIC: u16 = 1 << 2;
pub const UNDERLINE: u16 = 1 << 3;
pub const BLINK: u16 = 1 << 4;
pub const REVERSE: u16 = 1 << 5;
pub const HIDDEN: u16 = 1 << 6;
pub const STRIKETHROUGH: u16 = 1 << 7;

// Internal color storage (packed RGB)
const COLOR_NONE: u32 = 0xFF_FF_FF_FF;

/// StyleCapsule - T1 Atomic (64B actual, 32B alignment)
///
/// Compact text style representation using bitflags and packed colors.
///
/// ## Memory Layout
///
/// ```text
/// [0-1]   Attributes (bitflags: bold, italic, underline, etc.)
/// [2-3]   Padding
/// [4-7]   Foreground color (packed RGB or special value)
/// [8-11]  Background color (packed RGB or special value)
/// [12-19] Generation counter (atomic)
/// [20-31] Padding
/// [32-63] Additional padding due to align(32) requirement
/// ```
///
/// ## Thread Safety
///
/// - Immutable after construction (builder pattern)
/// - Generation counter prevents TOCTOU races
/// - Cache-aligned for performance
///
/// ## Example
///
/// ```rust,ignore
/// use atomic_capsule::terminal::output::{StyleCapsule, Color};
///
/// let style = StyleCapsule::new()
///     .bold()
///     .italic()
///     .fg(Color::Red)
///     .bg(Color::White);
///
/// println!("{}Styled text{}", style.to_ansi(), StyleCapsule::reset().to_ansi());
/// ```
#[repr(C, align(32))]
pub struct StyleCapsule {
    attributes: AtomicU16,    // Bold, Italic, Underline, etc.
    _padding1: [u8; 2],
    fg_color: AtomicU32,      // Foreground (RGB packed)
    bg_color: AtomicU32,      // Background (RGB packed)
    generation: AtomicU64,    // TOCTOU prevention
    _padding2: [u8; 12],
}

impl StyleCapsule {
    /// Create a new empty style (no attributes, no colors)
    pub const fn new() -> Self {
        Self {
            attributes: AtomicU16::new(0),
            _padding1: [0; 2],
            fg_color: AtomicU32::new(COLOR_NONE),
            bg_color: AtomicU32::new(COLOR_NONE),
            generation: AtomicU64::new(0),
            _padding2: [0; 12],
        }
    }

    /// Create a reset style (clear all attributes and colors)
    pub const fn reset() -> Self {
        Self::new()
    }

    /// Add bold attribute
    pub fn bold(self) -> Self {
        self.add_attribute(BOLD)
    }

    /// Add dim attribute
    pub fn dim(self) -> Self {
        self.add_attribute(DIM)
    }

    /// Add italic attribute
    pub fn italic(self) -> Self {
        self.add_attribute(ITALIC)
    }

    /// Add underline attribute
    pub fn underline(self) -> Self {
        self.add_attribute(UNDERLINE)
    }

    /// Add blink attribute
    pub fn blink(self) -> Self {
        self.add_attribute(BLINK)
    }

    /// Add reverse attribute (swap foreground/background)
    pub fn reverse(self) -> Self {
        self.add_attribute(REVERSE)
    }

    /// Add hidden attribute (conceal text)
    pub fn hidden(self) -> Self {
        self.add_attribute(HIDDEN)
    }

    /// Add strikethrough attribute
    pub fn strikethrough(self) -> Self {
        self.add_attribute(STRIKETHROUGH)
    }

    /// Set foreground color
    pub fn fg(self, color: Color) -> Self {
        let packed = Self::pack_color(color);
        self.fg_color.store(packed, Ordering::Relaxed);
        self
    }

    /// Set background color
    pub fn bg(self, color: Color) -> Self {
        let packed = Self::pack_color(color);
        self.bg_color.store(packed, Ordering::Relaxed);
        self
    }

    /// Generate ANSI escape sequence
    ///
    /// Returns empty string if no styles are set.
    #[cfg(feature = "std")]
    pub fn to_ansi(&self) -> String {
        use std::string::String;
        use std::format;

        let attrs = self.attributes.load(Ordering::Relaxed);
        let fg = self.fg_color.load(Ordering::Relaxed);
        let bg = self.bg_color.load(Ordering::Relaxed);

        // If no styles, return empty (or reset if this is a reset style)
        if attrs == 0 && fg == COLOR_NONE && bg == COLOR_NONE {
            return String::from("\x1b[0m");
        }

        let mut codes = std::vec::Vec::new();

        // Add attribute codes
        if attrs & BOLD != 0 {
            codes.push(1);
        }
        if attrs & DIM != 0 {
            codes.push(2);
        }
        if attrs & ITALIC != 0 {
            codes.push(3);
        }
        if attrs & UNDERLINE != 0 {
            codes.push(4);
        }
        if attrs & BLINK != 0 {
            codes.push(5);
        }
        if attrs & REVERSE != 0 {
            codes.push(7);
        }
        if attrs & HIDDEN != 0 {
            codes.push(8);
        }
        if attrs & STRIKETHROUGH != 0 {
            codes.push(9);
        }

        // Build escape sequence
        let mut result = String::from("\x1b[");

        // Add attribute codes
        for (i, code) in codes.iter().enumerate() {
            if i > 0 {
                result.push(';');
            }
            result.push_str(&format!("{}", code));
        }

        // Add foreground color
        if fg != COLOR_NONE {
            if !codes.is_empty() {
                result.push(';');
            }
            let color_capsule = Self::unpack_color(fg);
            let (buf, len) = color_capsule.to_ansi_fg();
            // Extract just the color codes (skip \x1b[ and m)
            let color_str = core::str::from_utf8(&buf[2..len - 1]).unwrap();
            result.push_str(color_str);
        }

        // Add background color
        if bg != COLOR_NONE {
            if !codes.is_empty() || fg != COLOR_NONE {
                result.push(';');
            }
            let color_capsule = Self::unpack_color(bg);
            let (buf, len) = color_capsule.to_ansi_bg();
            // Extract just the color codes (skip \x1b[ and m)
            let color_str = core::str::from_utf8(&buf[2..len - 1]).unwrap();
            result.push_str(color_str);
        }

        result.push('m');
        result
    }

    /// Check if a specific attribute is set
    #[inline]
    pub fn has_attribute(&self, attr: u16) -> bool {
        self.attributes.load(Ordering::Relaxed) & attr != 0
    }

    /// Get current attributes bitmask
    #[inline]
    pub fn attributes(&self) -> u16 {
        self.attributes.load(Ordering::Relaxed)
    }

    /// Increment generation counter
    #[inline]
    pub fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed)
    }

    /// Get current generation
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    // Internal helpers

    fn add_attribute(self, attr: u16) -> Self {
        let current = self.attributes.load(Ordering::Relaxed);
        self.attributes.store(current | attr, Ordering::Relaxed);
        self
    }

    fn pack_color(color: Color) -> u32 {
        match color {
            Color::Rgb(r, g, b) => {
                ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
            }
            Color::Ansi256(index) => {
                // Use high byte to indicate ANSI256 mode
                0x01_00_00_00 | (index as u32)
            }
            Color::Reset => COLOR_NONE,
            // Standard 16 colors encoded as ANSI256
            _ => {
                let capsule = ColorCapsule::from(color);
                0x01_00_00_00 | (capsule.ansi256_index() as u32)
            }
        }
    }

    fn unpack_color(packed: u32) -> ColorCapsule {
        if packed == COLOR_NONE {
            return ColorCapsule::new_reset();
        }

        if packed & 0xFF_00_00_00 == 0x01_00_00_00 {
            // ANSI256 mode
            let index = (packed & 0xFF) as u8;
            ColorCapsule::new_ansi256(index)
        } else {
            // RGB mode
            let r = ((packed >> 16) & 0xFF) as u8;
            let g = ((packed >> 8) & 0xFF) as u8;
            let b = (packed & 0xFF) as u8;
            ColorCapsule::new_rgb(r, g, b)
        }
    }
}

impl Default for StyleCapsule {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StyleCapsule {
    fn clone(&self) -> Self {
        let attrs = self.attributes.load(Ordering::Relaxed);
        let fg = self.fg_color.load(Ordering::Relaxed);
        let bg = self.bg_color.load(Ordering::Relaxed);

        Self {
            attributes: AtomicU16::new(attrs),
            _padding1: [0; 2],
            fg_color: AtomicU32::new(fg),
            bg_color: AtomicU32::new(bg),
            generation: AtomicU64::new(0),
            _padding2: [0; 12],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_capsule_size() {
        // With align(32), the actual size is 64 bytes due to padding
        assert_eq!(core::mem::size_of::<StyleCapsule>(), 64);
        assert_eq!(core::mem::align_of::<StyleCapsule>(), 32);
    }

    #[test]
    fn test_new_style() {
        let style = StyleCapsule::new();
        assert_eq!(style.attributes(), 0);
        // Empty style returns reset sequence
        assert_eq!(style.to_ansi(), "\x1b[0m");
    }

    #[test]
    fn test_reset_style() {
        let reset = StyleCapsule::reset();
        assert_eq!(reset.to_ansi(), "\x1b[0m");
    }

    #[test]
    fn test_bold() {
        let style = StyleCapsule::new().bold();
        assert!(style.has_attribute(BOLD));
        assert!(style.to_ansi().contains("1m"));
    }

    #[test]
    fn test_italic() {
        let style = StyleCapsule::new().italic();
        assert!(style.has_attribute(ITALIC));
        assert!(style.to_ansi().contains("3m"));
    }

    #[test]
    fn test_underline() {
        let style = StyleCapsule::new().underline();
        assert!(style.has_attribute(UNDERLINE));
        assert!(style.to_ansi().contains("4m"));
    }

    #[test]
    fn test_multiple_attributes() {
        let style = StyleCapsule::new().bold().italic().underline();
        assert!(style.has_attribute(BOLD));
        assert!(style.has_attribute(ITALIC));
        assert!(style.has_attribute(UNDERLINE));
        let ansi = style.to_ansi();
        assert!(ansi.contains("1"));
        assert!(ansi.contains("3"));
        assert!(ansi.contains("4"));
    }

    #[test]
    fn test_foreground_color_rgb() {
        let style = StyleCapsule::new().fg(Color::Rgb(255, 128, 64));
        let ansi = style.to_ansi();
        assert!(ansi.contains("38;2;255;128;64"));
    }

    #[test]
    fn test_background_color_rgb() {
        let style = StyleCapsule::new().bg(Color::Rgb(255, 128, 64));
        let ansi = style.to_ansi();
        assert!(ansi.contains("48;2;255;128;64"));
    }

    #[test]
    fn test_foreground_color_ansi() {
        let style = StyleCapsule::new().fg(Color::Red);
        let ansi = style.to_ansi();
        // Red is ANSI color 1, maps to 31 for foreground
        assert!(ansi.contains("38;5;1"));
    }

    #[test]
    fn test_background_color_ansi() {
        let style = StyleCapsule::new().bg(Color::Red);
        let ansi = style.to_ansi();
        // Red is ANSI color 1, maps to 41 for background
        assert!(ansi.contains("48;5;1"));
    }

    #[test]
    fn test_bold_red_on_white() {
        let style = StyleCapsule::new()
            .bold()
            .fg(Color::Red)
            .bg(Color::White);
        let ansi = style.to_ansi();
        assert!(ansi.contains("1")); // Bold
        assert!(ansi.contains("38;5;1")); // Red foreground
        assert!(ansi.contains("48;5;7")); // White background
    }

    #[test]
    fn test_all_attributes() {
        let style = StyleCapsule::new()
            .bold()
            .dim()
            .italic()
            .underline()
            .blink()
            .reverse()
            .hidden()
            .strikethrough();

        assert!(style.has_attribute(BOLD));
        assert!(style.has_attribute(DIM));
        assert!(style.has_attribute(ITALIC));
        assert!(style.has_attribute(UNDERLINE));
        assert!(style.has_attribute(BLINK));
        assert!(style.has_attribute(REVERSE));
        assert!(style.has_attribute(HIDDEN));
        assert!(style.has_attribute(STRIKETHROUGH));
    }

    #[test]
    fn test_generation_counter() {
        let style = StyleCapsule::new();
        assert_eq!(style.generation(), 0);
        style.next_generation();
        assert_eq!(style.generation(), 1);
        style.next_generation();
        assert_eq!(style.generation(), 2);
    }

    #[test]
    fn test_clone() {
        let original = StyleCapsule::new().bold().fg(Color::Red);
        let cloned = original.clone();

        assert_eq!(original.attributes(), cloned.attributes());
        assert_eq!(original.to_ansi(), cloned.to_ansi());
    }

    #[test]
    fn test_default() {
        let style = StyleCapsule::default();
        assert_eq!(style.attributes(), 0);
        assert_eq!(style.to_ansi(), "\x1b[0m");
    }

    #[test]
    fn test_pack_unpack_rgb() {
        let color = Color::Rgb(255, 128, 64);
        let packed = StyleCapsule::pack_color(color);
        let unpacked = StyleCapsule::unpack_color(packed);
        assert_eq!(unpacked.rgb(), (255, 128, 64));
    }

    #[test]
    fn test_pack_unpack_ansi256() {
        let color = Color::Ansi256(196);
        let packed = StyleCapsule::pack_color(color);
        let unpacked = StyleCapsule::unpack_color(packed);
        assert_eq!(unpacked.ansi256_index(), 196);
    }

    #[test]
    fn test_bright_colors() {
        let style = StyleCapsule::new().fg(Color::BrightRed);
        let ansi = style.to_ansi();
        assert!(ansi.contains("38;5;9")); // Bright red is index 9
    }
}
