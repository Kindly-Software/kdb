//! Error box widget for displaying error messages
//!
//! # Features
//! - Error message display with wrapping
//! - Report button with hover state
//! - Close button with hover state
//! - Chaos-compliant AtomicU64 state (T1 Atomic tier)

use std::sync::atomic::{AtomicU64, Ordering};
use crate::gui_v2::layout::Rect;

/// Error box widget state
///
/// # State Encoding (AtomicU64)
/// - Bits 0-7: Hover state (0=none, 1=report_button, 2=close_button)
/// - Bits 8-15: Visibility (0=hidden, 1=visible)
/// - Bits 16-63: Reserved
#[repr(C, align(64))]
pub struct ErrorBoxWidget {
    /// Packed state (hover + visibility)
    state: AtomicU64,
    /// Error message (heap-allocated, not in hot path)
    message: std::sync::RwLock<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverState {
    None = 0,
    ReportButton = 1,
    CloseButton = 2,
}

impl ErrorBoxWidget {
    /// Create new error box widget (initially hidden)
    pub fn new() -> Self {
        Self {
            state: AtomicU64::new(0),
            message: std::sync::RwLock::new(None),
        }
    }

    /// Show error message
    pub fn show_error(&self, message: String) {
        *self.message.write().unwrap() = Some(message);
        self.set_visible(true);
    }

    /// Hide error box
    pub fn hide(&self) {
        self.set_visible(false);
        self.set_hover(HoverState::None);
    }

    /// Clear error message
    pub fn clear(&self) {
        *self.message.write().unwrap() = None;
        self.hide();
    }

    /// Set visibility
    pub fn set_visible(&self, visible: bool) {
        let old = self.state.load(Ordering::Acquire);
        let visibility_bits = if visible { 1u64 } else { 0u64 };
        let new = (old & !0xFF00) | (visibility_bits << 8);
        self.state.store(new, Ordering::Release);
    }

    /// Get visibility
    pub fn is_visible(&self) -> bool {
        let state = self.state.load(Ordering::Acquire);
        ((state >> 8) & 0xFF) == 1
    }

    /// Set hover state
    pub fn set_hover(&self, hover: HoverState) {
        let old = self.state.load(Ordering::Acquire);
        let new = (old & !0xFF) | (hover as u64);
        self.state.store(new, Ordering::Release);
    }

    /// Get hover state
    pub fn get_hover(&self) -> HoverState {
        let state = self.state.load(Ordering::Acquire);
        let hover_bits = (state & 0xFF) as u8;
        match hover_bits {
            1 => HoverState::ReportButton,
            2 => HoverState::CloseButton,
            _ => HoverState::None,
        }
    }

    /// Check if report button is hovered
    pub fn is_report_hovered(&self) -> bool {
        self.get_hover() == HoverState::ReportButton
    }

    /// Check if close button is hovered
    pub fn is_close_hovered(&self) -> bool {
        self.get_hover() == HoverState::CloseButton
    }

    /// Get error message
    pub fn get_message(&self) -> Option<String> {
        self.message.read().unwrap().clone()
    }

    /// Get report button bounds (for hit testing)
    pub fn get_report_button_bounds(&self) -> Rect {
        // Report button is 120px wide, 35px tall
        // Positioned at (220, 400) in error box (offset from box top-left)
        Rect {
            x: 220,
            y: 400,
            width: 120,
            height: 35,
        }
    }

    /// Get close button bounds (for hit testing)
    pub fn get_close_button_bounds(&self) -> Rect {
        // Close button is 100px wide, 35px tall
        // Positioned at (360, 400) in error box (offset from box top-left)
        Rect {
            x: 360,
            y: 400,
            width: 100,
            height: 35,
        }
    }

    /// Get error box bounds (for layout)
    pub fn get_box_bounds(&self) -> Rect {
        // Error box is 600px wide, 200px tall
        // Centered horizontally at (100, 300)
        Rect {
            x: 100,
            y: 300,
            width: 600,
            height: 200,
        }
    }

    /// Wrap error message for display (max 60 chars per line)
    pub fn wrap_message(&self, max_width: usize) -> Vec<String> {
        let message = match self.get_message() {
            Some(msg) => msg,
            None => return vec![],
        };

        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in message.split_whitespace() {
            if current_line.len() + word.len() + 1 > max_width {
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                }
            }

            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        lines
    }
}

impl Default for ErrorBoxWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_widget() {
        let widget = ErrorBoxWidget::new();
        assert!(!widget.is_visible());
        assert!(widget.get_message().is_none());
        assert_eq!(widget.get_hover(), HoverState::None);
    }

    #[test]
    fn test_show_error() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("Test error message"));

        assert!(widget.is_visible());
        assert_eq!(
            widget.get_message(),
            Some(String::from("Test error message"))
        );
    }

    #[test]
    fn test_hide() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("Test error"));
        widget.hide();

        assert!(!widget.is_visible());
        // Message should still be available
        assert!(widget.get_message().is_some());
    }

    #[test]
    fn test_clear() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("Test error"));
        widget.clear();

        assert!(!widget.is_visible());
        assert!(widget.get_message().is_none());
    }

    #[test]
    fn test_set_visible() {
        let widget = ErrorBoxWidget::new();

        widget.set_visible(true);
        assert!(widget.is_visible());

        widget.set_visible(false);
        assert!(!widget.is_visible());
    }

    #[test]
    fn test_hover_state() {
        let widget = ErrorBoxWidget::new();

        widget.set_hover(HoverState::ReportButton);
        assert_eq!(widget.get_hover(), HoverState::ReportButton);
        assert!(widget.is_report_hovered());
        assert!(!widget.is_close_hovered());

        widget.set_hover(HoverState::CloseButton);
        assert_eq!(widget.get_hover(), HoverState::CloseButton);
        assert!(!widget.is_report_hovered());
        assert!(widget.is_close_hovered());

        widget.set_hover(HoverState::None);
        assert_eq!(widget.get_hover(), HoverState::None);
    }

    #[test]
    fn test_hover_preserves_visibility() {
        let widget = ErrorBoxWidget::new();
        widget.set_visible(true);

        widget.set_hover(HoverState::ReportButton);
        assert!(widget.is_visible());
    }

    #[test]
    fn test_visibility_preserves_hover() {
        let widget = ErrorBoxWidget::new();
        widget.set_hover(HoverState::CloseButton);

        widget.set_visible(true);
        assert_eq!(widget.get_hover(), HoverState::CloseButton);
    }

    #[test]
    fn test_report_button_bounds() {
        let widget = ErrorBoxWidget::new();
        let bounds = widget.get_report_button_bounds();
        assert_eq!(bounds.x, 220);
        assert_eq!(bounds.y, 400);
        assert_eq!(bounds.width, 120);
        assert_eq!(bounds.height, 35);
    }

    #[test]
    fn test_close_button_bounds() {
        let widget = ErrorBoxWidget::new();
        let bounds = widget.get_close_button_bounds();
        assert_eq!(bounds.x, 360);
        assert_eq!(bounds.y, 400);
        assert_eq!(bounds.width, 100);
        assert_eq!(bounds.height, 35);
    }

    #[test]
    fn test_box_bounds() {
        let widget = ErrorBoxWidget::new();
        let bounds = widget.get_box_bounds();
        assert_eq!(bounds.x, 100);
        assert_eq!(bounds.y, 300);
        assert_eq!(bounds.width, 600);
        assert_eq!(bounds.height, 200);
    }

    #[test]
    fn test_wrap_message_short() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("Short message"));

        let lines = widget.wrap_message(60);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "Short message");
    }

    #[test]
    fn test_wrap_message_long() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from(
            "This is a very long error message that should be wrapped to multiple lines when displayed",
        ));

        let lines = widget.wrap_message(60);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.len() <= 60);
        }
    }

    #[test]
    fn test_wrap_message_exact_width() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("This is exactly sixty characters long for testing purposes"));

        let lines = widget.wrap_message(60);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_wrap_message_empty() {
        let widget = ErrorBoxWidget::new();
        let lines = widget.wrap_message(60);
        assert!(lines.is_empty());
    }

    #[test]
    fn test_wrap_message_single_long_word() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from(
            "supercalifragilisticexpialidocious",
        ));

        let lines = widget.wrap_message(20);
        // Single long word should still appear on one line
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_atomic_alignment() {
        let widget = ErrorBoxWidget::new();
        let ptr = &widget as *const ErrorBoxWidget as usize;
        assert_eq!(ptr % 64, 0, "ErrorBoxWidget not 64-byte aligned");
    }

    #[test]
    fn test_concurrent_updates() {
        use std::sync::Arc;
        use std::thread;

        let widget = Arc::new(ErrorBoxWidget::new());
        let widget1 = Arc::clone(&widget);
        let widget2 = Arc::clone(&widget);

        let h1 = thread::spawn(move || {
            for i in 0..1000 {
                widget1.show_error(format!("Error {}", i));
            }
        });

        let h2 = thread::spawn(move || {
            for _ in 0..1000 {
                widget2.set_hover(HoverState::ReportButton);
                widget2.set_hover(HoverState::CloseButton);
            }
        });

        h1.join().unwrap();
        h2.join().unwrap();

        // Should have some error message and valid hover state
        assert!(widget.get_message().is_some());
        let hover = widget.get_hover();
        assert!(matches!(
            hover,
            HoverState::None | HoverState::ReportButton | HoverState::CloseButton
        ));
    }

    #[test]
    fn test_hide_clears_hover() {
        let widget = ErrorBoxWidget::new();
        widget.show_error(String::from("Test"));
        widget.set_hover(HoverState::ReportButton);

        widget.hide();
        assert_eq!(widget.get_hover(), HoverState::None);
    }
}
