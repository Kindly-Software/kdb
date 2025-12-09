//! Theme module for kindly-gui framework
//!
//! Provides T1 Atomic theme capsules with Byzantine purple + gold branding.

pub mod style;
mod theme;

pub use style::{FontWeight, StyleBuilder, StyleCapsule, TextAlign};
pub use theme::{
    ThemeCapsule, ThemeMode,
    // Byzantine Purple Palette
    PURPLE_DEEP, PURPLE_ROYAL, PURPLE_MEDIUM, PURPLE_LIGHT,
    // Gold Palette
    GOLD_DARK, GOLD_BRIGHT, GOLD_LIGHT,
    // Neutral Palette
    BG_DARK, BG_LIGHT, TEXT_PRIMARY, TEXT_SECONDARY, TEXT_TERTIARY,
    // Semantic Colors
    SUCCESS, WARNING, ERROR,
    // Color utilities
    rgba, from_rgba,
};
