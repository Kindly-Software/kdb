//! Widget Error Types
//!
//! **UCE34 Framework: T0 Auditable tier - Widget error types**
//!
//! Error types for widget operations, compatible with the terminal error
//! hierarchy and supporting Q34 audit trails.
//!
//! ## Framework Compliance
//! - **UCE34**: Q10 (T0 Auditable tier)
//! - **Chaos**: 100% safe, simple error types
//! - **Q34**: Context-rich errors for audit trails
//! - **ASSUM**: 99.99% safe (no unsafe code)

use core::fmt;

/// Widget operation error types
///
/// Represents errors that can occur during widget operations:
/// - Invalid layout constraints
/// - Render failures
/// - Focus management errors
/// - Type mismatches in widget slots
///
/// # Q34 Audit Trail Support
/// All errors include contextual information for debugging and audit logging.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetError {
    /// Invalid constraints (min > max)
    InvalidConstraints {
        min_width: u16,
        max_width: u16,
        min_height: u16,
        max_height: u16,
    },

    /// Layout failed (widget doesn't fit in bounds)
    LayoutFailed {
        required_width: u16,
        required_height: u16,
        available_width: u16,
        available_height: u16,
    },

    /// Render buffer overflow (too many commands)
    RenderBufferOverflow {
        capacity: usize,
        attempted: usize,
    },

    /// Invalid widget ID (not found in tree)
    InvalidWidgetId(u64),

    /// Type mismatch (tried to downcast to wrong type)
    TypeMismatch {
        expected: u64,
        actual: u64,
    },

    /// Focus operation failed (widget not focusable)
    FocusNotAllowed {
        widget_id: u64,
        reason: &'static str,
    },

    /// Event not consumed (propagate up tree)
    EventNotConsumed,
}

impl fmt::Display for WidgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConstraints { min_width, max_width, min_height, max_height } => {
                write!(
                    f,
                    "Invalid constraints: width [{}, {}], height [{}, {}]",
                    min_width, max_width, min_height, max_height
                )
            }
            Self::LayoutFailed { required_width, required_height, available_width, available_height } => {
                write!(
                    f,
                    "Layout failed: required {}x{}, available {}x{}",
                    required_width, required_height, available_width, available_height
                )
            }
            Self::RenderBufferOverflow { capacity, attempted } => {
                write!(
                    f,
                    "Render buffer overflow: capacity {}, attempted {}",
                    capacity, attempted
                )
            }
            Self::InvalidWidgetId(id) => {
                write!(f, "Invalid widget ID: {:#x}", id)
            }
            Self::TypeMismatch { expected, actual } => {
                write!(f, "Type mismatch: expected {:#x}, got {:#x}", expected, actual)
            }
            Self::FocusNotAllowed { widget_id, reason } => {
                write!(f, "Focus not allowed for widget {:#x}: {}", widget_id, reason)
            }
            Self::EventNotConsumed => {
                write!(f, "Event not consumed by widget")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for WidgetError {}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_constraints() {
        let err = WidgetError::InvalidConstraints {
            min_width: 100,
            max_width: 50,
            min_height: 100,
            max_height: 50,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid constraints"));
        assert!(msg.contains("100"));
        assert!(msg.contains("50"));
    }

    #[test]
    fn test_layout_failed() {
        let err = WidgetError::LayoutFailed {
            required_width: 200,
            required_height: 100,
            available_width: 150,
            available_height: 80,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Layout failed"));
        assert!(msg.contains("200x100"));
        assert!(msg.contains("150x80"));
    }

    #[test]
    fn test_render_buffer_overflow() {
        let err = WidgetError::RenderBufferOverflow {
            capacity: 1024,
            attempted: 2048,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("overflow"));
        assert!(msg.contains("1024"));
        assert!(msg.contains("2048"));
    }

    #[test]
    fn test_invalid_widget_id() {
        let err = WidgetError::InvalidWidgetId(0xDEADBEEF);
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid widget ID"));
        assert!(msg.contains("0xdeadbeef"));
    }

    #[test]
    fn test_type_mismatch() {
        let err = WidgetError::TypeMismatch {
            expected: 0x1234,
            actual: 0x5678,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Type mismatch"));
        assert!(msg.contains("0x1234"));
        assert!(msg.contains("0x5678"));
    }

    #[test]
    fn test_focus_not_allowed() {
        let err = WidgetError::FocusNotAllowed {
            widget_id: 42,
            reason: "widget is disabled",
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Focus not allowed"));
        assert!(msg.contains("42"));
        assert!(msg.contains("disabled"));
    }

    #[test]
    fn test_event_not_consumed() {
        let err = WidgetError::EventNotConsumed;
        let msg = format!("{}", err);
        assert!(msg.contains("not consumed"));
    }
}
