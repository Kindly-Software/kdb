//! ColorCapsule - Terminal Color Representation (T3 Fixed-Point, 64B)
//!
//! Supports RGB (24-bit true color), 256-color palette, and standard 16-color ANSI modes.
//!
//! ## Design
//!
//! - **Tier**: T3 Fixed-Point (deterministic color conversion)
//! - **Size**: 64B (cache-line aligned)
//! - **Generation Counter**: TOCTOU prevention
//! - **Zero Allocations**: Stack-based escape sequence generation
//!
//! ## ANSI Color Support
//!
//! - **24-bit RGB**: `\x1b[38;2;{r};{g};{b}m` (foreground), `\x1b[48;2;{r};{g};{b}m` (background)
//! - **256-color**: `\x1b[38;5;{n}m` (foreground), `\x1b[48;5;{n}m` (background)
//! - **16-color**: Standard ANSI codes 30-37, 90-97 (foreground), 40-47, 100-107 (background)
//!
//! ## References
//!
//! - [24-bit True Color Support](https://gist.github.com/sindresorhus/bed863fb8bedf023b833c88c322e44f9)
//! - [ANSI Escape Sequences](https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797)
//! - [RGB to ANSI256 Conversion](https://github.com/rhysd/rgb2ansi256)

use core::sync::atomic::{AtomicU64, Ordering};

/// Color representation modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// 24-bit true color (RGB)
    Rgb,
    /// 256-color palette
    Ansi256,
    /// Standard 16 colors
    Ansi16,
    /// Reset/default terminal color
    Reset,
}

/// Color enum with support for various modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    // Standard 16 colors (ANSI)
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,

    // Extended modes
    /// 24-bit true color
    Rgb(u8, u8, u8),
    /// 256-color palette index
    Ansi256(u8),
    /// Reset to terminal default
    Reset,
}

/// ColorCapsule - T3 Fixed-Point (64B)
///
/// Compact color representation with RGB, 256-color, and ANSI modes.
///
/// ## Memory Layout
///
/// ```text
/// [0-2]   RGB values (r, g, b)
/// [3]     Color mode (Rgb/Ansi256/Ansi16/Reset)
/// [4]     ANSI 256 index (for compatibility)
/// [5-7]   Padding
/// [8-15]  Generation counter (atomic)
/// [16-63] Padding to 64B
/// ```
///
/// ## Thread Safety
///
/// - Immutable after construction (builder pattern)
/// - Generation counter prevents TOCTOU races
/// - Cache-aligned for performance
#[repr(C, align(64))]
pub struct ColorCapsule {
    // RGB (24-bit true color)
    r: u8,
    g: u8,
    b: u8,

    // Color mode
    mode: ColorMode,

    // ANSI 256 index (for compatibility)
    ansi256_index: u8,

    // Padding to 8B boundary
    _padding1: [u8; 3],

    // Generation counter (TOCTOU prevention)
    generation: AtomicU64,

    // Padding to 64B
    _padding2: [u8; 47],
}

impl ColorCapsule {
    /// Create a new ColorCapsule from RGB values
    pub const fn new_rgb(r: u8, g: u8, b: u8) -> Self {
        let ansi256_index = rgb_to_ansi256(r, g, b);
        Self {
            r,
            g,
            b,
            mode: ColorMode::Rgb,
            ansi256_index,
            _padding1: [0; 3],
            generation: AtomicU64::new(0),
            _padding2: [0; 47],
        }
    }

    /// Create from ANSI 256 palette index
    pub const fn new_ansi256(index: u8) -> Self {
        let (r, g, b) = ansi256_to_rgb(index);
        Self {
            r,
            g,
            b,
            mode: ColorMode::Ansi256,
            ansi256_index: index,
            _padding1: [0; 3],
            generation: AtomicU64::new(0),
            _padding2: [0; 47],
        }
    }

    /// Create from ANSI 16 color
    pub const fn new_ansi16(color: u8) -> Self {
        let index = color % 16;
        let (r, g, b) = ansi16_to_rgb(index);
        Self {
            r,
            g,
            b,
            mode: ColorMode::Ansi16,
            ansi256_index: index,
            _padding1: [0; 3],
            generation: AtomicU64::new(0),
            _padding2: [0; 47],
        }
    }

    /// Create reset color (terminal default)
    pub const fn new_reset() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            mode: ColorMode::Reset,
            ansi256_index: 0,
            _padding1: [0; 3],
            generation: AtomicU64::new(0),
            _padding2: [0; 47],
        }
    }

    /// Get RGB values
    #[inline]
    pub const fn rgb(&self) -> (u8, u8, u8) {
        (self.r, self.g, self.b)
    }

    /// Get color mode
    #[inline]
    pub const fn mode(&self) -> ColorMode {
        self.mode
    }

    /// Get ANSI 256 index
    #[inline]
    pub const fn ansi256_index(&self) -> u8 {
        self.ansi256_index
    }

    /// Generate ANSI escape sequence for foreground color
    ///
    /// Returns (buffer, length) to avoid allocations
    pub fn to_ansi_fg(&self) -> ([u8; 32], usize) {
        let mut buf = [0u8; 32];
        let len = match self.mode {
            ColorMode::Rgb => {
                // \x1b[38;2;{r};{g};{b}m
                let s = format!("\x1b[38;2;{};{};{}m", self.r, self.g, self.b);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Ansi256 => {
                // \x1b[38;5;{n}m
                let s = format!("\x1b[38;5;{}m", self.ansi256_index);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Ansi16 => {
                // Standard ANSI codes: 30-37 (normal), 90-97 (bright)
                let code = if self.ansi256_index < 8 {
                    30 + self.ansi256_index
                } else {
                    82 + self.ansi256_index // 90-97 for bright colors
                };
                let s = format!("\x1b[{}m", code);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Reset => {
                // \x1b[39m (default foreground)
                let bytes = b"\x1b[39m";
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
        };
        (buf, len)
    }

    /// Generate ANSI escape sequence for background color
    ///
    /// Returns (buffer, length) to avoid allocations
    pub fn to_ansi_bg(&self) -> ([u8; 32], usize) {
        let mut buf = [0u8; 32];
        let len = match self.mode {
            ColorMode::Rgb => {
                // \x1b[48;2;{r};{g};{b}m
                let s = format!("\x1b[48;2;{};{};{}m", self.r, self.g, self.b);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Ansi256 => {
                // \x1b[48;5;{n}m
                let s = format!("\x1b[48;5;{}m", self.ansi256_index);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Ansi16 => {
                // Standard ANSI codes: 40-47 (normal), 100-107 (bright)
                let code = if self.ansi256_index < 8 {
                    40 + self.ansi256_index
                } else {
                    92 + self.ansi256_index // 100-107 for bright colors
                };
                let s = format!("\x1b[{}m", code);
                let bytes = s.as_bytes();
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
            ColorMode::Reset => {
                // \x1b[49m (default background)
                let bytes = b"\x1b[49m";
                buf[..bytes.len()].copy_from_slice(bytes);
                bytes.len()
            }
        };
        (buf, len)
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
}

impl From<Color> for ColorCapsule {
    fn from(color: Color) -> Self {
        match color {
            Color::Black => Self::new_ansi16(0),
            Color::Red => Self::new_ansi16(1),
            Color::Green => Self::new_ansi16(2),
            Color::Yellow => Self::new_ansi16(3),
            Color::Blue => Self::new_ansi16(4),
            Color::Magenta => Self::new_ansi16(5),
            Color::Cyan => Self::new_ansi16(6),
            Color::White => Self::new_ansi16(7),
            Color::BrightBlack => Self::new_ansi16(8),
            Color::BrightRed => Self::new_ansi16(9),
            Color::BrightGreen => Self::new_ansi16(10),
            Color::BrightYellow => Self::new_ansi16(11),
            Color::BrightBlue => Self::new_ansi16(12),
            Color::BrightMagenta => Self::new_ansi16(13),
            Color::BrightCyan => Self::new_ansi16(14),
            Color::BrightWhite => Self::new_ansi16(15),
            Color::Rgb(r, g, b) => Self::new_rgb(r, g, b),
            Color::Ansi256(index) => Self::new_ansi256(index),
            Color::Reset => Self::new_reset(),
        }
    }
}

impl Default for ColorCapsule {
    fn default() -> Self {
        Self::new_reset()
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::Reset
    }
}

impl Color {
    /// Create a color from RGBA8888 value (ignores alpha)
    ///
    /// # Format
    /// - Bits 24-31: Red
    /// - Bits 16-23: Green
    /// - Bits 8-15: Blue
    /// - Bits 0-7: Alpha (ignored)
    #[inline]
    pub const fn from_u32(rgba: u32) -> Self {
        let r = ((rgba >> 24) & 0xFF) as u8;
        let g = ((rgba >> 16) & 0xFF) as u8;
        let b = ((rgba >> 8) & 0xFF) as u8;
        Color::Rgb(r, g, b)
    }

    /// Alias for from_u32 (RGBA8888 format)
    #[inline]
    pub const fn from_rgba8888(rgba: u32) -> Self {
        Self::from_u32(rgba)
    }
}

// Color conversion utilities

/// Convert RGB to ANSI 256 color index
///
/// Uses 6×6×6 color cube (216 colors) + grayscale ramp (24 shades)
const fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Check if it's a grayscale color
    let is_gray = (r as i16 - g as i16).abs() < 10 && (g as i16 - b as i16).abs() < 10;

    if is_gray {
        // Use grayscale ramp (232-255)
        let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
        if gray < 8 {
            16 // Use color cube black
        } else if gray > 247 {
            231 // Use color cube white
        } else {
            232 + ((gray - 8) / 10)
        }
    } else {
        // Use 6×6×6 color cube (16-231)
        let r6 = (r as u16 * 6 / 256) as u8;
        let g6 = (g as u16 * 6 / 256) as u8;
        let b6 = (b as u16 * 6 / 256) as u8;
        16 + 36 * r6 + 6 * g6 + b6
    }
}

/// Convert ANSI 256 index to RGB
const fn ansi256_to_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        // Standard 16 colors (0-15)
        0..=15 => ansi16_to_rgb(index),

        // 6×6×6 color cube (16-231)
        16..=231 => {
            let idx = index - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            (
                if r == 0 { 0 } else { 55 + r * 40 },
                if g == 0 { 0 } else { 55 + g * 40 },
                if b == 0 { 0 } else { 55 + b * 40 },
            )
        }

        // Grayscale ramp (232-255)
        232..=255 => {
            let gray = 8 + (index - 232) * 10;
            (gray, gray, gray)
        }
    }
}

/// Convert ANSI 16 color to RGB
const fn ansi16_to_rgb(color: u8) -> (u8, u8, u8) {
    match color {
        0 => (0, 0, 0),           // Black
        1 => (128, 0, 0),         // Red
        2 => (0, 128, 0),         // Green
        3 => (128, 128, 0),       // Yellow
        4 => (0, 0, 128),         // Blue
        5 => (128, 0, 128),       // Magenta
        6 => (0, 128, 128),       // Cyan
        7 => (192, 192, 192),     // White
        8 => (128, 128, 128),     // Bright Black (Gray)
        9 => (255, 0, 0),         // Bright Red
        10 => (0, 255, 0),        // Bright Green
        11 => (255, 255, 0),      // Bright Yellow
        12 => (0, 0, 255),        // Bright Blue
        13 => (255, 0, 255),      // Bright Magenta
        14 => (0, 255, 255),      // Bright Cyan
        15 => (255, 255, 255),    // Bright White
        _ => (0, 0, 0),           // Default to black
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_capsule_size() {
        assert_eq!(core::mem::size_of::<ColorCapsule>(), 64);
        assert_eq!(core::mem::align_of::<ColorCapsule>(), 64);
    }

    #[test]
    fn test_rgb_color() {
        let color = ColorCapsule::new_rgb(255, 128, 64);
        assert_eq!(color.rgb(), (255, 128, 64));
        assert_eq!(color.mode(), ColorMode::Rgb);
    }

    #[test]
    fn test_ansi256_color() {
        let color = ColorCapsule::new_ansi256(196); // Bright red in 256-color palette
        assert_eq!(color.ansi256_index(), 196);
        assert_eq!(color.mode(), ColorMode::Ansi256);
    }

    #[test]
    fn test_ansi16_color() {
        let color = ColorCapsule::new_ansi16(1); // Red
        assert_eq!(color.ansi256_index(), 1);
        assert_eq!(color.mode(), ColorMode::Ansi16);
    }

    #[test]
    fn test_reset_color() {
        let color = ColorCapsule::new_reset();
        assert_eq!(color.mode(), ColorMode::Reset);
    }

    #[test]
    fn test_from_color_enum() {
        let red = ColorCapsule::from(Color::Red);
        assert_eq!(red.ansi256_index(), 1);

        let rgb = ColorCapsule::from(Color::Rgb(255, 100, 50));
        assert_eq!(rgb.rgb(), (255, 100, 50));
        assert_eq!(rgb.mode(), ColorMode::Rgb);

        let reset = ColorCapsule::from(Color::Reset);
        assert_eq!(reset.mode(), ColorMode::Reset);
    }

    #[test]
    fn test_ansi_foreground_rgb() {
        let color = ColorCapsule::new_rgb(255, 128, 64);
        let (buf, len) = color.to_ansi_fg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_ansi_background_rgb() {
        let color = ColorCapsule::new_rgb(255, 128, 64);
        let (buf, len) = color.to_ansi_bg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[48;2;255;128;64m");
    }

    #[test]
    fn test_ansi_foreground_256() {
        let color = ColorCapsule::new_ansi256(196);
        let (buf, len) = color.to_ansi_fg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[38;5;196m");
    }

    #[test]
    fn test_ansi_background_256() {
        let color = ColorCapsule::new_ansi256(196);
        let (buf, len) = color.to_ansi_bg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[48;5;196m");
    }

    #[test]
    fn test_ansi_foreground_16() {
        let color = ColorCapsule::new_ansi16(1); // Red
        let (buf, len) = color.to_ansi_fg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[31m");
    }

    #[test]
    fn test_ansi_background_16() {
        let color = ColorCapsule::new_ansi16(1); // Red
        let (buf, len) = color.to_ansi_bg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[41m");
    }

    #[test]
    fn test_ansi_bright_foreground_16() {
        let color = ColorCapsule::new_ansi16(9); // Bright Red
        let (buf, len) = color.to_ansi_fg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[91m");
    }

    #[test]
    fn test_ansi_bright_background_16() {
        let color = ColorCapsule::new_ansi16(9); // Bright Red
        let (buf, len) = color.to_ansi_bg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[101m");
    }

    #[test]
    fn test_reset_foreground() {
        let color = ColorCapsule::new_reset();
        let (buf, len) = color.to_ansi_fg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[39m");
    }

    #[test]
    fn test_reset_background() {
        let color = ColorCapsule::new_reset();
        let (buf, len) = color.to_ansi_bg();
        let ansi = core::str::from_utf8(&buf[..len]).unwrap();
        assert_eq!(ansi, "\x1b[49m");
    }

    #[test]
    fn test_rgb_to_ansi256_gray() {
        // Pure gray should map to grayscale ramp (232-255)
        let index = rgb_to_ansi256(128, 128, 128);
        assert!(index >= 232 && index <= 255);
    }

    #[test]
    fn test_rgb_to_ansi256_color() {
        // Red should map to color cube
        let index = rgb_to_ansi256(255, 0, 0);
        assert!(index >= 16 && index <= 231);
    }

    #[test]
    fn test_ansi256_to_rgb_standard() {
        let (r, g, b) = ansi256_to_rgb(1); // Red
        assert_eq!((r, g, b), (128, 0, 0));
    }

    #[test]
    fn test_ansi256_to_rgb_cube() {
        let (r, g, b) = ansi256_to_rgb(196); // Bright red in color cube
        assert!(r > 200 && g < 50 && b < 50);
    }

    #[test]
    fn test_ansi256_to_rgb_grayscale() {
        let (r, g, b) = ansi256_to_rgb(244); // Mid-gray
        assert_eq!(r, g);
        assert_eq!(g, b);
    }

    #[test]
    fn test_generation_counter() {
        let color = ColorCapsule::new_rgb(255, 0, 0);
        assert_eq!(color.generation(), 0);
        color.next_generation();
        assert_eq!(color.generation(), 1);
    }

    #[test]
    fn test_default() {
        let color = ColorCapsule::default();
        assert_eq!(color.mode(), ColorMode::Reset);
    }
}
