//! # Terminal User Interface (TUI) Capsules
//!
//! High-performance TUI components built on atomic capsule architecture.
//!
//! ## UCE34 Framework Integration
//!
//! - **Tier**: T0 (Auditable) + T1 (Atomic) - Hash-chained audit logging with sub-50ns event logging
//! - **Architecture**: 100% lockfree, 512-byte cache-aligned, hash-chained tamper detection
//! - **Testing**: 25+ tests (Q1-Q28 T28 framework) covering hash chains, verification, compliance
//!
//! ## Modules
//!
//! - [`audit_log`]: AuditLogCapsule - Q34 compliance audit logging with hash-chaining
//! - [`file_navigator`]: Atomic file system navigator with Blake3 hashing
//! - [`screen_state`]: Screen state machine with navigation history and timeout management
//! - [`terminal_capabilities`]: TerminalCapabilityCapsule - T1 Atomic terminal capability detection
//! - [`keyboard_input`]: KeyboardInputHistoryCapsule - T1 Atomic keyboard input tracking with idle detection
//!

pub mod audit_log;
pub mod file_navigator;
pub mod screen_state;
pub mod terminal_capabilities;
pub mod render_buffer;
pub mod configuration;
pub mod keyboard_input;

pub use audit_log::AuditLogCapsule;
pub use file_navigator::{FileNavigatorCapsule, filter_flags};
pub use screen_state::{ScreenStateCapsule, ScreenId};
pub use terminal_capabilities::TerminalCapabilityCapsule;
pub use render_buffer::RenderBufferCapsule;
pub use configuration::{ConfigurationCapsule, Q16Fixed};
pub use keyboard_input::KeyboardInputHistoryCapsule;
