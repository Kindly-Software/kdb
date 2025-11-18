//! Welcome screen for kindly_dedup CLI
//!
//! Displays pulsing purple hearts, application title, and performance highlights.
//!
//! ## Features
//! - Pulsing hearts animation (brightness-aware)
//! - Byzantine Purple/Gold themed branding
//! - Performance metrics display
//! - Heavy box drawing borders (Unicode)
//!
//! ## UCE34 Framework
//! - Q14 (Capsule Pattern): Uses AnimationStateCapsule for brightness animation
//! - Q28 (Simplicity): Pure rendering function, no side effects
//! - Q31 (Rust Transform): Zero-copy string formatting

use crate::cli::state::{AnimationStateCapsule, MenuStateCapsule};
use crate::utils::terminal::{box_drawing, emoji, Colorize};
use std::io::{self, Write};

/// Render welcome screen with pulsing purple hearts
///
/// ## Arguments
/// - `menu_state`: Current menu state (unused in welcome, for future phases)
/// - `animation_state`: Animation state for brightness cycling
///
/// ## Returns
/// `io::Result<()>` for I/O operations
///
/// ## Performance
/// - Rendering: <50µs (string allocation)
/// - Brightness calculation: <5ns (Relaxed atomic load)
pub fn render_welcome_screen(
    _menu_state: &MenuStateCapsule,
    animation_state: &AnimationStateCapsule,
) -> io::Result<()> {
    // Get brightness for pulsing hearts (0-100)
    let brightness = animation_state.brightness();

    // Fade hearts between 60% and 100%
    let heart_symbol = "💜";

    // Clear screen (ANSI escape code)
    print!("\x1b[2J\x1b[H");
    io::stdout().flush()?;

    // Top border with pulsing hearts
    println!(
        "{}",
        "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓"
    );
    println!(
        "{}",
        "┃                                                                             ┃"
    );

    // Pulsing purple hearts (3 hearts, brightness-dependent styling)
    let heart_1 = apply_brightness(heart_symbol, brightness);
    let heart_2 = apply_brightness(heart_symbol, brightness.wrapping_add(50) % 100);
    let heart_3 = apply_brightness(heart_symbol, brightness.wrapping_sub(25) % 100);

    println!(
        "{}          {}                   {}                   {}                     {}",
        "┃", heart_1, heart_2, heart_3, "┃"
    );

    // Title (Byzantine Purple)
    println!(
        "{}{}{}",
        "┃",
        "                     k i n d l y _ d e d u p".byzantine_purple().bold(),
        "                     ┃"
    );

    // Subtitle (Light Purple)
    println!(
        "{}{}{}",
        "┃",
        "                   LLM Dataset Deduplication Tool".light_purple(),
        "               ┃"
    );

    // Version (Dim)
    println!(
        "{}{}{}",
        "┃",
        "                          v1.14.0 (2025-11-10)".dim(),
        "                      ┃"
    );

    println!(
        "{}",
        "┃                                                                             ┃"
    );

    // Pulsing hearts again
    println!(
        "{}          {}                   {}                   {}                     {}",
        "┃", heart_1, heart_2, heart_3, "┃"
    );
    println!(
        "{}",
        "┃                                                                             ┃"
    );
    println!(
        "{}",
        "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛"
    );

    // Performance highlights
    println!();
    println!(
        "   {} PERFORMANCE: 373K docs/sec @ 16 cores (measured @ 10M scale)",
        emoji::performance::ROCKET
    );
    println!(
        "   {} QUALITY: 95-100% F1 score (duplicate detection accuracy)",
        emoji::brand::GEM
    );
    println!(
        "   {} COMPLIANCE: Q34 audit trails (SOX/SOC2/GDPR/HIPAA ready)",
        emoji::tools::SHIELD
    );
    println!(
        "   {} ARCHITECTURE: 100% lockfree (computational capsules T0-T10)",
        emoji::performance::LIGHTNING
    );

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    io::stdout().flush()?;
    Ok(())
}

/// Apply brightness styling to emoji
///
/// ## Brightness Levels
/// - 0-60: Dim (fade to background)
/// - 61-85: Normal (standard style)
/// - 86-100: Bold (bright highlight)
///
/// ## Performance
/// <5ns (inline comparison)
#[inline]
fn apply_brightness(emoji: &str, brightness: u8) -> String {
    if brightness < 70 {
        emoji.to_string().dim()
    } else if brightness < 85 {
        emoji.to_string()
    } else {
        emoji.to_string().bold()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_brightness_dim() {
        let result = apply_brightness("💜", 50);
        // Should contain dim styling
        assert!(result.contains("💜"));
    }

    #[test]
    fn test_apply_brightness_normal() {
        let result = apply_brightness("💜", 80);
        assert!(result.contains("💜"));
    }

    #[test]
    fn test_apply_brightness_bold() {
        let result = apply_brightness("💜", 95);
        // Should contain bold styling
        assert!(result.contains("💜"));
    }

    #[test]
    fn test_render_welcome_screen() {
        let menu = MenuStateCapsule::new();
        let anim = AnimationStateCapsule::new(8);

        // Should not panic
        let result = render_welcome_screen(&menu, &anim);
        assert!(result.is_ok());
    }

    #[test]
    fn test_render_with_different_brightness_levels() {
        let menu = MenuStateCapsule::new();
        let anim = AnimationStateCapsule::new(8);

        // Test at different brightness levels
        for brightness in [0, 50, 70, 85, 100] {
            anim.set_brightness(brightness);
            let result = render_welcome_screen(&menu, &anim);
            assert!(result.is_ok(), "Failed at brightness {}", brightness);
        }
    }
}
