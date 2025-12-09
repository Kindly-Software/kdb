//! Foundation Widget Primitives
//!
//! Core widget building blocks for terminal UI applications.
//!
//! ## Widgets
//!
//! - `ButtonCapsule`: Interactive button with press animation (T1+T3)

pub mod button;
// TODO: Fix these to match Widget trait signature
// pub mod label;
// pub mod checkbox;
// pub mod spacer;
// pub mod progress;

pub use button::ButtonCapsule;
