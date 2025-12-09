// Copyright (C) 2025 Kindly Platform, Inc.
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Error types for Chaos-compliant GUI framework
//!
//! # Tier Classification
//!
//! T0 (Auditable): Error types with deterministic construction
//!
//! # Framework Compliance
//!
//! - **UCE34**: Q1-Q3 domain errors, no allocations
//! - **Chaos**: No mutex, no Arc, deterministic error construction
//! - **ASSUM**: 100% safe (no unsafe code)

use thiserror::Error;

/// GUI framework error types
///
/// All errors are deterministic and allocation-free where possible.
///
/// # Examples
///
/// ```
/// use atomic_capsule::gui::GuiError;
///
/// let err = GuiError::InvalidDimensions { width: 0, height: 100 };
/// assert!(matches!(err, GuiError::InvalidDimensions { .. }));
/// ```
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GuiError {
    /// Invalid dimensions (zero or negative size)
    #[error("Invalid dimensions: width={width}, height={height}")]
    InvalidDimensions {
        /// Width in pixels (Q16.16 raw value)
        width: u32,
        /// Height in pixels (Q16.16 raw value)
        height: u32,
    },

    /// Invalid color value (out of range)
    #[error("Invalid color component: {component}={value} (max 255)")]
    InvalidColor {
        /// Component name (R, G, B, or A)
        component: &'static str,
        /// Invalid value
        value: u32,
    },

    /// Overflow in coordinate calculation
    #[error("Coordinate overflow: {operation}")]
    CoordinateOverflow {
        /// Description of the operation that overflowed
        operation: &'static str,
    },

    /// Invalid rectangle (negative area or flipped coordinates)
    #[error("Invalid rectangle: x={x}, y={y}, width={width}, height={height}")]
    InvalidRect {
        /// X coordinate (Q16.16 raw value)
        x: u32,
        /// Y coordinate (Q16.16 raw value)
        y: u32,
        /// Width (Q16.16 raw value)
        width: u32,
        /// Height (Q16.16 raw value)
        height: u32,
    },

    /// Point outside valid bounds
    #[error("Point out of bounds: x={x}, y={y}")]
    OutOfBounds {
        /// X coordinate (Q16.16 raw value)
        x: u32,
        /// Y coordinate (Q16.16 raw value)
        y: u32,
    },

    /// Resource not found (texture, font, etc.)
    #[error("Resource not found: {resource_type} with id {id}")]
    ResourceNotFound {
        /// Type of resource (e.g., "texture", "font")
        resource_type: &'static str,
        /// Resource identifier
        id: u64,
    },

    /// Resource allocation failed
    #[error("Resource allocation failed: {resource_type}")]
    AllocationFailed {
        /// Type of resource that failed to allocate
        resource_type: &'static str,
    },

    /// Invalid state transition
    #[error("Invalid state transition: {from} -> {to}")]
    InvalidStateTransition {
        /// Current state
        from: &'static str,
        /// Attempted new state
        to: &'static str,
    },

    /// Render backend error
    #[error("Render backend error: {message}")]
    RenderError {
        /// Error message from backend
        message: &'static str,
    },

    /// Event queue full
    #[error("Event queue full: {capacity} events")]
    EventQueueFull {
        /// Queue capacity
        capacity: usize,
    },

    /// Invalid event
    #[error("Invalid event: {reason}")]
    InvalidEvent {
        /// Reason the event is invalid
        reason: &'static str,
    },
}

impl GuiError {
    /// Check if error is recoverable
    ///
    /// # Returns
    ///
    /// `true` if the application can continue after this error
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::GuiError;
    ///
    /// let err = GuiError::ResourceNotFound {
    ///     resource_type: "texture",
    ///     id: 42,
    /// };
    /// assert!(err.is_recoverable());
    ///
    /// let fatal = GuiError::AllocationFailed {
    ///     resource_type: "framebuffer",
    /// };
    /// assert!(!fatal.is_recoverable());
    /// ```
    #[inline]
    pub const fn is_recoverable(&self) -> bool {
        match self {
            // Recoverable errors (can continue with degraded functionality)
            Self::ResourceNotFound { .. } => true,
            Self::InvalidEvent { .. } => true,
            Self::EventQueueFull { .. } => true,
            Self::OutOfBounds { .. } => true,

            // Fatal errors (cannot continue safely)
            Self::AllocationFailed { .. } => false,
            Self::RenderError { .. } => false,

            // Input validation errors (caller bug, but not fatal)
            Self::InvalidDimensions { .. } => true,
            Self::InvalidColor { .. } => true,
            Self::CoordinateOverflow { .. } => true,
            Self::InvalidRect { .. } => true,
            Self::InvalidStateTransition { .. } => true,
        }
    }

    /// Get error severity level (0 = debug, 1 = warning, 2 = error, 3 = fatal)
    ///
    /// # Examples
    ///
    /// ```
    /// use atomic_capsule::gui::GuiError;
    ///
    /// let warn = GuiError::ResourceNotFound {
    ///     resource_type: "texture",
    ///     id: 42,
    /// };
    /// assert_eq!(warn.severity(), 1);
    ///
    /// let fatal = GuiError::AllocationFailed {
    ///     resource_type: "framebuffer",
    /// };
    /// assert_eq!(fatal.severity(), 3);
    /// ```
    #[inline]
    pub const fn severity(&self) -> u8 {
        match self {
            // Debug (0)
            Self::InvalidEvent { .. } => 0,

            // Warning (1)
            Self::ResourceNotFound { .. } => 1,
            Self::EventQueueFull { .. } => 1,
            Self::OutOfBounds { .. } => 1,

            // Error (2)
            Self::InvalidDimensions { .. } => 2,
            Self::InvalidColor { .. } => 2,
            Self::CoordinateOverflow { .. } => 2,
            Self::InvalidRect { .. } => 2,
            Self::InvalidStateTransition { .. } => 2,

            // Fatal (3)
            Self::AllocationFailed { .. } => 3,
            Self::RenderError { .. } => 3,
        }
    }
}

/// Result type for GUI operations
pub type GuiResult<T> = Result<T, GuiError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_construction() {
        let err = GuiError::InvalidDimensions {
            width: 0,
            height: 100,
        };
        assert_eq!(err.severity(), 2);
        assert!(err.is_recoverable());
    }

    #[test]
    fn test_error_display() {
        let err = GuiError::InvalidColor {
            component: "R",
            value: 256,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Invalid color"));
        assert!(msg.contains("R"));
        assert!(msg.contains("256"));
    }

    #[test]
    fn test_recoverable_classification() {
        let recoverable = GuiError::ResourceNotFound {
            resource_type: "texture",
            id: 42,
        };
        assert!(recoverable.is_recoverable());

        let fatal = GuiError::AllocationFailed {
            resource_type: "buffer",
        };
        assert!(!fatal.is_recoverable());
    }

    #[test]
    fn test_severity_levels() {
        let debug = GuiError::InvalidEvent {
            reason: "unknown type",
        };
        assert_eq!(debug.severity(), 0);

        let warning = GuiError::EventQueueFull { capacity: 1024 };
        assert_eq!(warning.severity(), 1);

        let error = GuiError::InvalidRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
        assert_eq!(error.severity(), 2);

        let fatal = GuiError::RenderError {
            message: "context lost",
        };
        assert_eq!(fatal.severity(), 3);
    }

    #[test]
    fn test_clone_and_equality() {
        let err1 = GuiError::OutOfBounds { x: 100, y: 200 };
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }
}
