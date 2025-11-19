//! Conditional field serialization via #[capsule_serialize(skip_if = "...")]
//!
//! **NOT EXPORTED** from proc_macro crate (internal module only).
//! Use SkipPredicate from field_parser module instead.
//!
//! Implements conditional skip predicates for compile-time field exclusion.
//!
//! # Architecture
//!
//! **Tier**: T0 (Auditable - compile-time derive macro)
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_PREDICATE_PATH`: Predicate path parses correctly (verified by syn)
//! - `#VERIFY_PREDICATE_PATH`: Quote! generates valid TokenStream
//! - `#ASSUME_FIELD_TYPE_MATCH`: Field type compatible with predicate (verified at compile)
//! - `#VERIFY_FIELD_TYPE_MATCH`: Compile error if type mismatch occurs
//! - `#ASSUME_DETERMINISTIC_CONDITION`: Predicate returns consistent bool (user's responsibility)
//! - `#VERIFY_DETERMINISTIC_CONDITION`: Tests validate for common cases

// Re-export from field_parser (internal use only)
pub use crate::field_parser::SkipPredicate;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skip_predicate_parse_option_is_none() {
        let pred = SkipPredicate::parse("Option::is_none");
        assert!(matches!(pred, SkipPredicate::OptionIsNone));
    }

    #[test]
    fn test_skip_predicate_parse_vec_is_empty() {
        let pred = SkipPredicate::parse("Vec::is_empty");
        assert!(matches!(pred, SkipPredicate::VecIsEmpty));
    }

    #[test]
    fn test_skip_predicate_parse_string_is_empty() {
        let pred = SkipPredicate::parse("String::is_empty");
        assert!(matches!(pred, SkipPredicate::StringIsEmpty));
    }

    #[test]
    fn test_skip_predicate_parse_is_zero() {
        let pred = SkipPredicate::parse("is_zero");
        assert!(matches!(pred, SkipPredicate::IsZero));
    }

    #[test]
    fn test_skip_predicate_parse_is_false() {
        let pred = SkipPredicate::parse("is_false");
        assert!(matches!(pred, SkipPredicate::IsFalse));
    }

    #[test]
    fn test_skip_predicate_parse_custom_path() {
        let pred = SkipPredicate::parse("my_module::is_special");
        match pred {
            SkipPredicate::CustomPath(path) => assert_eq!(path, "my_module::is_special"),
            _ => panic!("Expected CustomPath"),
        }
    }

    #[test]
    fn test_skip_predicate_validate_builtin_option_is_none() {
        assert!(SkipPredicate::validate("Option::is_none").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_builtin_vec_is_empty() {
        assert!(SkipPredicate::validate("Vec::is_empty").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_builtin_string_is_empty() {
        assert!(SkipPredicate::validate("String::is_empty").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_builtin_is_zero() {
        assert!(SkipPredicate::validate("is_zero").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_builtin_is_false() {
        assert!(SkipPredicate::validate("is_false").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_custom_path_single_segment() {
        assert!(SkipPredicate::validate("my_func").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_custom_path_multi_segment() {
        assert!(SkipPredicate::validate("my_module::is_special").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_custom_path_nested() {
        assert!(SkipPredicate::validate("a::b::c::d").is_ok());
    }

    #[test]
    fn test_skip_predicate_validate_empty_string() {
        let result = SkipPredicate::validate("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_skip_predicate_validate_empty_segment() {
        let result = SkipPredicate::validate("my_module::is_special::");
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("empty") || err_msg.contains("segment"));
    }

    #[test]
    fn test_skip_predicate_validate_invalid_start_digit() {
        let result = SkipPredicate::validate("9invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must start with"));
    }

    #[test]
    fn test_skip_predicate_validate_invalid_characters() {
        let result = SkipPredicate::validate("invalid-func");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid characters"));
    }
}
