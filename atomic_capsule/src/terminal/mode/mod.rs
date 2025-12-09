//! # Terminal Mode Management - T1 Atomic Capsules
//!
//! **High-performance terminal mode management with atomic state tracking and RAII cleanup.**
//!
//! ## Framework: UCE34 Q10-Q34 (Tier 1 Atomic)
//!
//! ## Modules
//!
//! - [`raw`]: RawModeCapsule - Atomic raw mode management with automatic cleanup
//! - [`screen`]: AlternateScreenCapsule - Atomic alternate screen buffer management
//! - [`cursor`]: CursorCapsule - Atomic cursor visibility and position management

pub mod raw;
pub mod screen;
pub mod cursor;

pub use raw::{RawModeCapsule, RawModeError};
pub use screen::{AlternateScreenCapsule, ScreenError};
pub use cursor::{CursorCapsule, CursorError};
