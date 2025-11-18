//! Utility modules (zero external dependencies)
//!
//! Replaces external dependencies with std-only implementations:
//! - terminal: Replaces `colored` + `atty` (-2 deps)

pub mod terminal;
