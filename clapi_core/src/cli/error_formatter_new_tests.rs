#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_cents() {
        assert_eq!(format_cents(100), "$1.00");
        assert_eq!(format_cents(10_000), "$100.00");
        assert_eq!(format_cents(1), "$0.01");
        assert_eq!(format_cents(0), "$0.00");
    }

    #[test]
    fn test_format_budget_exhausted() {
        let formatter = ErrorFormatter::new(true, true, Verbosity::Default);
        let error = ClapiError::BudgetExhausted {
            requested: 1000,
            available: 500,
        };
        let output = formatter.format_error(&error);

        assert!(output.contains("Budget Exhausted"));
        assert!(output.contains("CLAPI-E001"));
        assert!(output.contains("clapi budget add"));
        assert!(output.contains("docs.clapi.dev"));
    }

    #[test]
    fn test_format_config_error() {
        let formatter = ErrorFormatter::new(true, true, Verbosity::Default);
        let error = ClapiError::ConfigError("Missing required field: listen_addr".to_string());
        let output = formatter.format_error(&error);

        assert!(output.contains("Configuration Error"));  // Updated
        assert!(output.contains("CLAPI-E013"));  // Added
        assert!(output.contains("Missing required field"));
        assert!(output.contains("clapi config"));
        assert!(output.contains("docs.clapi.dev"));
    }

    #[test]
    fn test_format_all_providers_unavailable() {
        let formatter = ErrorFormatter::new(true, true, Verbosity::Default);
        let error = ClapiError::AllProvidersUnavailable;
        let output = formatter.format_error(&error);

        assert!(output.contains("No Providers Available"));  // Updated
        assert!(output.contains("CLAPI-E003"));  // Added
        assert!(output.contains("clapi providers"));
        assert!(output.contains("docs.clapi.dev"));
    }

    #[test]
    fn test_verbose_mode() {
        use crate::cli::error_formatter::Verbosity;

        let formatter = ErrorFormatter::new(true, true, Verbosity::Verbose);
        assert_eq!(formatter.verbosity, Verbosity::Verbose);

        let formatter = ErrorFormatter::new(true, true, Verbosity::Default);
        assert_eq!(formatter.verbosity, Verbosity::Default);
    }
}
