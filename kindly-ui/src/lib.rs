//! Kindly UI - Shared Leptos Components
//!
//! Byzantine Royal Purple + Gold design system for Kindly products.

pub mod theme;
pub mod components;
pub mod effects;

// Re-exports for convenience
pub use theme::{colors, *};
pub use components::*;
pub use effects::MeshGradient;
