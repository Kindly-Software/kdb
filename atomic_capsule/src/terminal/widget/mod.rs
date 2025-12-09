//! Terminal Widget System
//!
//! High-performance widget primitives for terminal UI applications with computational
//! capsule architecture.
//!
//! ## Design Principles
//!
//! - **UCE34 Framework**: T1+T3 compound tier (Atomic state + Fixed-point animation)
//! - **Chaos Compliant**: 100% lockfree, cache-aligned capsules
//! - **Zero-Copy State**: Atomic operations for state transitions
//! - **Fixed-Point Animation**: Q8.8 for smooth sub-pixel precision
//!
//! ## Module Organization
//!
//! - `foundation`: Core widget primitives (ButtonCapsule, TextInputCapsule, etc.)
//! - `layout`: Layout primitives (FlexCapsule, GridCapsule, etc.)
//! - `container`: Container widgets (PanelCapsule, ScrollCapsule, etc.)
//!
//! ## Core Traits
//!
//! - `Widget`: Base trait for all widgets (measure, layout, handle_event, render)
//! - `Focusable`: Trait for widgets that can receive focus
//! - `Scrollable`: Trait for widgets with scrollable content
//!
//! ## Feature Flags
//!
//! - `terminal-widget`: Enable widget system
//! - `terminal-widget-foundation`: Core widgets (button, text, label)
//! - `terminal-widget-layout`: Layout containers
//! - `terminal-widget-advanced`: Advanced widgets (tree, table, chart)

pub mod foundation;
pub mod container;
pub mod complex;
pub mod style;
pub mod types;
pub mod focus;
pub mod layout;
pub mod tree;
pub mod error;

// Re-exports for convenience
pub use foundation::ButtonCapsule;
pub use container::panel::{PanelCapsule, BorderStyle, ShadowDirection};
pub use complex::{DropdownCapsule, TreeCapsule, TreeNodeState, ListCapsule, SelectionMode, ListItemState};
pub use style::{
    CacheStats, StyleCacheCapsule,
    ComputedStyleCapsule, PseudoState, flags as style_flags
};

// Stub types for missing widgets (TODO: implement)
pub struct TabsCapsule;
pub struct TableCapsule;
pub enum TabPosition {}
pub enum TabStyle {}
pub enum TabAction {}
pub struct TabInfo;
pub use types::{
    Rect, Color, RenderCommandBuffer, RenderCommand, RenderStyle, Constraints, Widget
};
pub use focus::FocusManagerCapsule;

use crate::terminal::event::Event;

// Type aliases for compatibility with old code
pub type RenderContext<'a> = &'a mut RenderCommandBuffer;
pub type EventResult = bool;

// Need alloc for String/Vec in RenderCommandBuffer
extern crate alloc;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_contains() {
        let r = Rect::new(10, 10, 20, 20);
        assert!(r.contains(15, 15));
        assert!(r.contains(10, 10));
        assert!(!r.contains(9, 15));
        assert!(!r.contains(30, 15));
    }

    #[test]
    fn test_rect_area() {
        let rect = Rect::new(0, 0, 10, 20);
        assert_eq!(rect.area(), 200);
    }

    #[test]
    fn test_render_command_buffer() {
        let mut cmd = RenderCommandBuffer::new();
        cmd.draw_char(0, 0, 'A', Color::RED);
        cmd.draw_text(1, 0, "Hello", Color::GREEN);

        assert_eq!(cmd.commands().len(), 2);
    }
}
