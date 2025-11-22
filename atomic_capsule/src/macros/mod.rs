//! Macro utilities for capsule definitions
//!
//! This module provides high-level macros for defining capsules with automatic boilerplate:
//! - `define_capsule!`: Complete capsule definition with Send/Sync/verification
//!
//! Note: The `define_capsule!` macro is exported at the crate root due to #[macro_export]

pub mod define_capsule;
