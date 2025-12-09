//! # Terminal Event Types Module
//!
//! **UCE34 Framework: T0 Auditable tier - Core event types for terminal applications**
//!
//! This module provides comprehensive terminal event types compatible with crossterm
//! for easy migration while maintaining Chaos compliance (100% lockfree, cache-aligned).
//!
//! ## Module Organization
//! - `types`: Core event types (Event, KeyCode, KeyEvent, MouseEvent, etc.)
//! - `queue`: T5 Streaming lockfree event queue (SPSC ring buffer)
//!
//! ## Key Features
//! - **Crossterm API compatibility**: Drop-in replacement for crossterm::event types
//! - **Copy + Clone**: All types are Copy for zero-cost passing
//! - **Compact representation**: KeyCode uses u16, modifiers use u8 bitflags
//! - **Comprehensive key coverage**: 100+ key codes including F1-F24, media keys, modifiers
//!
//! ## Design Principles (Chaos Compliance)
//! - **T0 Auditable**: Simple enums and structs, no atomic operations needed
//! - **Copy types**: Zero-cost event passing (no heap allocation)
//! - **Bitflag modifiers**: Efficient multi-modifier representation
//! - **#[repr(u16)]**: Compact storage for key codes
//!
//! ## Framework Compliance
//! - **UCE34**: Q10 (T0 Auditable tier - core types only)
//! - **Chaos**: 100% safe, no atomic operations (simple data types)
//! - **ASSUM**: 99.99% safe (all types are Copy, no unsafe code)
//! - **T28**: Inline tests for all type conversions
//!
//! ## References
//! - Crossterm: <https://docs.rs/crossterm/latest/crossterm/event/>
//! - VT100 sequences: <https://vt100.net/docs/vt100-ug/chapter3.html>
//! - ANSI escape codes: <https://en.wikipedia.org/wiki/ANSI_escape_code>
//!
//! ## Usage Example
//! ```rust
//! use atomic_capsule::terminal::event::{Event, KeyCode, KeyEvent, KeyModifiers};
//!
//! // Create a key event
//! let event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
//!
//! // Pattern match on events
//! match event.code {
//!     KeyCode::Char(ch) if event.modifiers.contains(KeyModifiers::CONTROL) => {
//!         println!("Ctrl+{}", ch);
//!     }
//!     _ => {}
//! }
//! ```

pub mod queue;
pub mod types;

// Re-export all types for convenient access
pub use types::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MediaKeyCode, ModifierKeyCode,
    MouseButton, MouseEvent, MouseEventKind,
};

// Re-export queue types
pub use queue::{EventQueueCapsule, EventQueueWithStorage};
