//! Widget Components for Chaos-Compliant GUI Framework
//!
//! # Overview
//!
//! Lockfree widget primitives using computational capsule architecture.
//!
//! # Tier Classification
//!
//! - **T1 (Atomic)**: Lockfree state coordination
//! - **T3 (Fixed-Point)**: Q8.8 animation for smooth interpolation
//!
//! # Modules
//!
//! - `button`: ButtonCapsule (T1+T3, <10ns state access, Q8.8 animation)
//! - `slider`: SliderCapsule (T1+T3, <5ns value access, Q8.8 fixed-point values)
//! - `text`: LabelCapsule, TextCapsule (T1, <10ns text access, inline storage)

pub mod button;
pub mod slider;
pub mod text;

pub use button::{ButtonCapsule, ButtonState, ButtonStyle, PressState};
pub use slider::{DragState, Orientation, SliderCapsule};
pub use text::{FontWeight, LabelCapsule, TextAlign, TextCapsule, TextRun};
