use std::fmt;

/// Application error types for kindly-web
#[derive(Debug, Clone, PartialEq)]
pub enum AppError {
    /// Navigation error
    Navigation(String),

    /// State error
    State(String),

    /// Component error
    Component(String),

    /// Validation error
    Validation(String),

    /// Unknown error
    Unknown(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Navigation(msg) => write!(f, "Navigation error: {}", msg),
            AppError::State(msg) => write!(f, "State error: {}", msg),
            AppError::Component(msg) => write!(f, "Component error: {}", msg),
            AppError::Validation(msg) => write!(f, "Validation error: {}", msg),
            AppError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

/// Result type for application operations
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AppError::Navigation("Test error".to_string());
        assert_eq!(err.to_string(), "Navigation error: Test error");
    }

    #[test]
    fn test_error_types() {
        let errors = vec![
            AppError::Navigation("nav".to_string()),
            AppError::State("state".to_string()),
            AppError::Component("comp".to_string()),
            AppError::Validation("valid".to_string()),
            AppError::Unknown("unknown".to_string()),
        ];

        assert_eq!(errors.len(), 5);
    }
}
