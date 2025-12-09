//! TUI Framework - Terminal User Interface
//!
//! # Purpose
//! Provides a lockfree, Byzantine Purple-themed TUI for clapi:
//! - Custom widgets (SelectWidget, InputWidget, ConfirmWidget)
//! - Event loop with <50ms input latency
//! - Atomic state updates via WizardStateCapsule
//!
//! # Design Principles
//! - Lockfree Updates: All state changes via atomic operations
//! - Byzantine Purple Theme: Primary highlight color (#663399)
//! - Zero Blocking: Event loop never blocks on user input
//! - <50ms Latency: Input processed immediately
//!
//! # UCE34 Framework
//! - Q10: T1 (Atomic) - State management via WizardStateCapsule
//! - Q13: Ratatui widgets for rendering
//! - Q25: <50ms input latency target
//! - Q33: Input validation at widget level
//!
//! # Performance Targets
//! - Widget render: <5ms
//! - Input handling: <50ms
//! - State update: <100ns (atomic operations)

pub mod capsules;
pub mod layout;
pub mod widgets;
pub mod wizard_app;

pub use capsules::{
    CtrlCHandlerCapsule,
    LogoAnimationCapsule,
    WizardStateCapsule,
};
pub use layout::{render_split_screen, render_logo, render_wizard_form};
pub use widgets::{ConfirmWidget, InputWidget, SelectWidget};
pub use wizard_app::TuiWizardApp;
