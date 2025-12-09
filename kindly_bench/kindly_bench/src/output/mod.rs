//! Output formatting for benchmark results
//!
//! Provides XML (machine-readable) and terminal (human-readable) output

pub mod xml;
pub mod terminal;

pub use terminal::print_results;
