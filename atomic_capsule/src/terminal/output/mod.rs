//! Terminal Output Capsules
//!
//! Text styling and color formatting for terminal output.
//!
//! ## Design Principles
//!
//! - **UCE34 Framework**: T1 Atomic (StyleCapsule 32B), T3 Fixed-Point (ColorCapsule 64B)
//! - **Chaos Compliant**: 100% lockfree, compact representation, generation counters
//! - **Multi-Mode Support**: RGB (24-bit), 256-color, 16-color ANSI
//! - **Zero Allocations**: All escape sequences built with stack buffers
//!
//! ## Capsules
//!
//! - `StyleCapsule`: Text attributes (bold, italic, underline) + colors (32B, T1)
//! - `ColorCapsule`: Color representation with mode detection (64B, T3)
//!
//! ## ANSI Escape Sequences
//!
//! Based on ECMA-48 / ISO/IEC 6429 (Select Graphic Rendition - SGR):
//!
//! - Foreground RGB: `\x1b[38;2;{r};{g};{b}m`
//! - Background RGB: `\x1b[48;2;{r};{g};{b}m`
//! - Foreground 256: `\x1b[38;5;{n}m`
//! - Background 256: `\x1b[48;5;{n}m`
//! - Bold: `\x1b[1m`, Italic: `\x1b[3m`, Underline: `\x1b[4m`
//! - Reset: `\x1b[0m`
//!
//! ## References
//!
//! - [ANSI Escape Codes (Wikipedia)](https://en.wikipedia.org/wiki/ANSI_escape_code)
//! - [ANSI Escape Sequences Cheatsheet](https://gist.github.com/fnky/458719343aabd01cfb17a3a4f7296797)
//! - [True Color Terminal Support](https://gist.github.com/sindresorhus/bed863fb8bedf023b833c88c322e44f9)
//! - [Terminal Colors Guide](https://chrisyeh96.github.io/2020/03/28/terminal-colors.html)
//!
//! ## Examples
//!
//! ```rust,ignore
//! use atomic_capsule::terminal::output::{StyleCapsule, Color};
//!
//! // Create a bold red text on white background
//! let style = StyleCapsule::new()
//!     .bold()
//!     .fg(Color::Red)
//!     .bg(Color::White);
//!
//! // Generate ANSI escape sequence
//! let ansi = style.to_ansi();
//! println!("{}Hello, World!{}", ansi, StyleCapsule::reset().to_ansi());
//!
//! // RGB colors
//! let rgb_style = StyleCapsule::new()
//!     .fg(Color::Rgb(255, 100, 0))
//!     .italic();
//! ```

pub mod style;
pub mod color;
pub mod writer;

pub use style::{StyleCapsule, BOLD, DIM, ITALIC, UNDERLINE, BLINK, REVERSE, HIDDEN, STRIKETHROUGH};
pub use color::{ColorCapsule, Color, ColorMode};
pub use writer::TerminalWriterCapsule;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_import() {
        let style = StyleCapsule::new();
        // Empty style returns reset sequence
        assert_eq!(style.to_ansi(), "\x1b[0m");
    }

    #[test]
    fn test_color_import() {
        let color = Color::Red;
        assert_eq!(ColorCapsule::from(color).ansi256_index(), 1);
    }

    #[test]
    fn test_rgb_color() {
        let color = Color::Rgb(255, 128, 64);
        let capsule = ColorCapsule::from(color);
        assert_eq!(capsule.mode(), ColorMode::Rgb);
    }

    #[test]
    fn test_style_bold() {
        let style = StyleCapsule::new().bold();
        assert!(style.to_ansi().contains("1m"));
    }

    #[test]
    fn test_style_reset() {
        let reset = StyleCapsule::reset();
        assert_eq!(reset.to_ansi(), "\x1b[0m");
    }
}
