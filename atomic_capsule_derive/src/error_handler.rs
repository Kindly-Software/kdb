//! # Error Handling and Diagnostics
//!
//! Provides clear, actionable error messages for proc-macro failures.

use syn::Error;

/// Create error with helpful diagnostic context.
///
/// Formats error messages with consistent structure:
/// - Main message describing the problem
/// - Help text providing actionable solutions
///
/// # ASSUM Framework
/// - `#ASSUME_ERROR_HELPFUL`: Errors include context and fixes
/// - `#VERIFY_ERROR_HELPFUL`: Manual review of error messages
///
/// # Example
///
/// ```rust,ignore
/// create_error_with_help(
///     span,
///     "Alignment mismatch",
///     "Update #[repr(C, align(64))]"
/// )
/// ```
#[allow(dead_code)] // Reserved for future Phase 4+ enhancements
pub fn create_error_with_help(span: proc_macro2::Span, message: &str, help: &str) -> Error {
    Error::new(span, format!("{}\nHelp: {}", message, help))
}

/// Create error with multi-line help text.
///
/// Similar to `create_error_with_help` but accepts multiple help lines for complex issues.
///
/// # Example
///
/// ```rust,ignore
/// create_error_with_multiline_help(
///     span,
///     "Alignment mismatch",
///     &["Option 1: Update repr", "Option 2: Update capsule"]
/// )
/// ```
#[allow(dead_code)] // Reserved for future Phase 4+ enhancements
pub fn create_error_with_multiline_help(
    span: proc_macro2::Span,
    message: &str,
    help_lines: &[&str],
) -> Error {
    let help = help_lines.join("\n");
    Error::new(span, format!("{}\n\n{}", message, help))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;

    #[test]
    fn test_create_error_with_help() {
        let error = create_error_with_help(
            Span::call_site(),
            "Alignment mismatch",
            "Update #[repr(C, align(64))]",
        );

        let message = error.to_string();
        assert!(message.contains("Alignment mismatch"));
        assert!(message.contains("Help:"));
        assert!(message.contains("Update #[repr"));
    }
}
