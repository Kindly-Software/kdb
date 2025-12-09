//! ANSI Escape Sequence Parser Module
//!
//! High-performance SIMD-accelerated ANSI/VT100 escape sequence parsing.
//!
//! # Components
//! - `tables`: Const lookup tables (0ns compile-time)
//! - `ansi`: AnsiParserCapsule (T2 SIMD, 256B cache-aligned)
//!
//! # References
//! - [VT100 State Machine](https://vt100.net/emu/dec_ansi_parser)
//! - [ANSI Escape Codes](https://en.wikipedia.org/wiki/ANSI_escape_code)
//! - [Xterm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)

pub mod tables;
pub mod ansi;

pub use ansi::{AnsiParserCapsule, ParserState, ParsedSequence};
