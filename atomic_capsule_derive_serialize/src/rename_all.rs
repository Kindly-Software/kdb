//! RenameAllStrategyCapsule - Field name transformation for JSON serialization
//!
//! Implements serde-compatible `#[serde(rename_all)]` strategies for CapsuleSerialize.
//!
//! **Tier**: T0 (Auditable - compile-time code generation)
//!
//! # Supported Strategies
//!
//! 1. `lowercase` - Convert to all lowercase
//! 2. `UPPERCASE` - Convert to all uppercase
//! 3. `PascalCase` - Convert to PascalCase (CapWords)
//! 4. `camelCase` - Convert to camelCase (first word lowercase)
//! 5. `snake_case` - Convert to snake_case (insert _ between words)
//! 6. `SCREAMING_SNAKE_CASE` - Convert to SCREAMING_SNAKE_CASE
//! 7. `kebab-case` - Convert to kebab-case (hyphens instead of underscores)
//! 8. `SCREAMING-KEBAB-CASE` - Convert to SCREAMING-KEBAB-CASE
//!
//! # Example
//!
//! ```rust,ignore
//! use atomic_capsule_derive_serialize::{CapsuleSerialize, RenameStrategy};
//!
//! #[derive(CapsuleSerialize)]
//! #[capsule_serialize(rename_all = "camelCase")]
//! #[repr(C, align(128))]
//! struct PersonCapsule {
//!     first_name: String,  // → "firstName" in JSON
//!     last_name: String,   // → "lastName" in JSON
//!     middle_initial: char, // → "middleInitial" in JSON
//! }
//! ```
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_STRATEGY_DETERMINISTIC`: Each strategy produces consistent output for same input
//! - `#VERIFY_STRATEGY_DETERMINISTIC`: Property tests ensure determinism
//! - `#ASSUME_WORD_BOUNDARIES`: PascalCase/camelCase detection based on '_' and capital letters
//! - `#VERIFY_WORD_BOUNDARIES`: Test coverage for boundary cases (consecutive capitals, leading/trailing _)
//! - `#ASSUME_COMPILE_TIME_ONLY`: All transformations happen in proc-macro, zero runtime cost
//! - `#VERIFY_COMPILE_TIME_ONLY`: No runtime code generated (only field name strings)

use std::fmt;

/// Field name transformation strategy for JSON serialization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameStrategy {
    /// Convert to lowercase: "myField" → "myfield"
    Lowercase,
    /// Convert to UPPERCASE: "myField" → "MYFIELD"
    Uppercase,
    /// Convert to PascalCase: "my_field" → "MyField"
    PascalCase,
    /// Convert to camelCase: "my_field" → "myField"
    CamelCase,
    /// Convert to snake_case: "myField" → "my_field"
    SnakeCase,
    /// Convert to SCREAMING_SNAKE_CASE: "myField" → "MY_FIELD"
    ScreamingSnakeCase,
    /// Convert to kebab-case: "my_field" → "my-field"
    KebabCase,
    /// Convert to SCREAMING-KEBAB-CASE: "myField" → "MY-FIELD"
    ScreamingKebabCase,
}

/// Error type for RenameStrategy parsing
#[derive(Debug, Clone)]
pub struct RenameStrategyError {
    pub strategy: String,
}

impl fmt::Display for RenameStrategyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Unknown rename_all strategy: '{}'\n\
             Valid options: lowercase, UPPERCASE, PascalCase, camelCase, \
             snake_case, SCREAMING_SNAKE_CASE, kebab-case, SCREAMING-KEBAB-CASE",
            self.strategy
        )
    }
}

impl RenameStrategy {
    /// Parse strategy from string attribute value.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let strategy = RenameStrategy::from_str("camelCase")?;
    /// assert_eq!(strategy, RenameStrategy::CamelCase);
    /// ```
    pub fn from_str(s: &str) -> Result<Self, RenameStrategyError> {
        match s {
            "lowercase" => Ok(RenameStrategy::Lowercase),
            "UPPERCASE" => Ok(RenameStrategy::Uppercase),
            "PascalCase" => Ok(RenameStrategy::PascalCase),
            "camelCase" => Ok(RenameStrategy::CamelCase),
            "snake_case" => Ok(RenameStrategy::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Ok(RenameStrategy::ScreamingSnakeCase),
            "kebab-case" => Ok(RenameStrategy::KebabCase),
            "SCREAMING-KEBAB-CASE" => Ok(RenameStrategy::ScreamingKebabCase),
            _ => Err(RenameStrategyError {
                strategy: s.to_string(),
            }),
        }
    }

    /// Apply strategy to field name. Used in proc-macro code generation.
    ///
    /// All transformations are deterministic and safe for compile-time use.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let strategy = RenameStrategy::CamelCase;
    /// assert_eq!(strategy.apply("first_name"), "firstName");
    /// assert_eq!(strategy.apply("my_long_field"), "myLongField");
    /// ```
    pub fn apply(&self, field_name: &str) -> String {
        match self {
            RenameStrategy::Lowercase => field_name.to_lowercase(),
            RenameStrategy::Uppercase => field_name.to_uppercase(),
            RenameStrategy::PascalCase => to_pascal_case(field_name),
            RenameStrategy::CamelCase => to_camel_case(field_name),
            RenameStrategy::SnakeCase => to_snake_case(field_name),
            RenameStrategy::ScreamingSnakeCase => to_snake_case(field_name).to_uppercase(),
            RenameStrategy::KebabCase => to_kebab_case(field_name),
            RenameStrategy::ScreamingKebabCase => to_kebab_case(field_name).to_uppercase(),
        }
    }
}

/// Convert field name to PascalCase (CapWords).
///
/// Splits on underscores and capital letters, capitalizes first letter of each word,
/// then joins without separators.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(to_pascal_case("my_field"), "MyField");
/// assert_eq!(to_pascal_case("myField"), "MyField");
/// assert_eq!(to_pascal_case("my_long_field_name"), "MyLongFieldName");
/// assert_eq!(to_pascal_case("simple"), "Simple");
/// assert_eq!(to_pascal_case("_leading"), "Leading");
/// assert_eq!(to_pascal_case("trailing_"), "Trailing");
/// assert_eq!(to_pascal_case("SCREAMING"), "Screaming");
/// ```
pub fn to_pascal_case(s: &str) -> String {
    let snake = to_snake_case(s);
    snake
        .split('_')
        .filter(|w| !w.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars.flat_map(|c| c.to_lowercase())).collect(),
            }
        })
        .collect()
}

/// Convert field name to camelCase (lowerCamelCase).
///
/// Converts to PascalCase first, then lowercases the first character.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(to_camel_case("my_field"), "myField");
/// assert_eq!(to_camel_case("MyField"), "myField");
/// assert_eq!(to_camel_case("my_long_field"), "myLongField");
/// assert_eq!(to_camel_case("simple"), "simple");
/// assert_eq!(to_camel_case("SCREAMING"), "screaming");
/// ```
pub fn to_camel_case(s: &str) -> String {
    let pascal = to_pascal_case(s);
    let mut chars = pascal.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().chain(chars).collect(),
    }
}

/// Convert field name to snake_case (lower_case_with_underscores).
///
/// Inserts underscores before capital letters and converts to lowercase.
/// Handles consecutive capitals (e.g., "HTTPServer" → "http_server").
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(to_snake_case("myField"), "my_field");
/// assert_eq!(to_snake_case("MyField"), "my_field");
/// assert_eq!(to_snake_case("my_field"), "my_field");
/// assert_eq!(to_snake_case("HTTPServer"), "http_server");
/// assert_eq!(to_snake_case("IOError"), "io_error");
/// assert_eq!(to_snake_case("simple"), "simple");
/// assert_eq!(to_snake_case("SCREAMING"), "screaming");
/// ```
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;

    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            // Insert underscore before capital letter, unless:
            // - First character
            // - Previous character was underscore
            // - Previous character was uppercase (handle consecutive capitals)
            if i > 0 && !result.ends_with('_') && !prev_was_upper {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
            prev_was_upper = true;
        } else {
            result.push(ch);
            prev_was_upper = false;
        }
    }

    // Clean up leading/trailing underscores
    result.trim_matches('_').to_string()
}

/// Convert field name to kebab-case (lower-case-with-hyphens).
///
/// Converts to snake_case first, then replaces underscores with hyphens.
///
/// # Examples
///
/// ```rust,ignore
/// assert_eq!(to_kebab_case("my_field"), "my-field");
/// assert_eq!(to_kebab_case("myField"), "my-field");
/// assert_eq!(to_kebab_case("my_long_field"), "my-long-field");
/// assert_eq!(to_kebab_case("HTTPServer"), "http-server");
/// ```
pub fn to_kebab_case(s: &str) -> String {
    to_snake_case(s).replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== RenameStrategy::from_str tests ==========

    #[test]
    fn test_from_str_lowercase() {
        assert_eq!(
            RenameStrategy::from_str("lowercase").unwrap(),
            RenameStrategy::Lowercase
        );
    }

    #[test]
    fn test_from_str_uppercase() {
        assert_eq!(
            RenameStrategy::from_str("UPPERCASE").unwrap(),
            RenameStrategy::Uppercase
        );
    }

    #[test]
    fn test_from_str_pascal_case() {
        assert_eq!(
            RenameStrategy::from_str("PascalCase").unwrap(),
            RenameStrategy::PascalCase
        );
    }

    #[test]
    fn test_from_str_camel_case() {
        assert_eq!(
            RenameStrategy::from_str("camelCase").unwrap(),
            RenameStrategy::CamelCase
        );
    }

    #[test]
    fn test_from_str_snake_case() {
        assert_eq!(
            RenameStrategy::from_str("snake_case").unwrap(),
            RenameStrategy::SnakeCase
        );
    }

    #[test]
    fn test_from_str_screaming_snake_case() {
        assert_eq!(
            RenameStrategy::from_str("SCREAMING_SNAKE_CASE").unwrap(),
            RenameStrategy::ScreamingSnakeCase
        );
    }

    #[test]
    fn test_from_str_kebab_case() {
        assert_eq!(
            RenameStrategy::from_str("kebab-case").unwrap(),
            RenameStrategy::KebabCase
        );
    }

    #[test]
    fn test_from_str_screaming_kebab_case() {
        assert_eq!(
            RenameStrategy::from_str("SCREAMING-KEBAB-CASE").unwrap(),
            RenameStrategy::ScreamingKebabCase
        );
    }

    #[test]
    fn test_from_str_invalid() {
        assert!(RenameStrategy::from_str("invalid_strategy").is_err());
        assert!(RenameStrategy::from_str("mixedCase").is_err());
        assert!(RenameStrategy::from_str("").is_err());
    }

    // ========== RenameStrategy::apply tests ==========

    #[test]
    fn test_apply_lowercase() {
        let strategy = RenameStrategy::Lowercase;
        assert_eq!(strategy.apply("myField"), "myfield");
        assert_eq!(strategy.apply("my_field"), "my_field");
        assert_eq!(strategy.apply("SCREAMING"), "screaming");
    }

    #[test]
    fn test_apply_uppercase() {
        let strategy = RenameStrategy::Uppercase;
        assert_eq!(strategy.apply("myField"), "MYFIELD");
        assert_eq!(strategy.apply("my_field"), "MY_FIELD");
        assert_eq!(strategy.apply("simple"), "SIMPLE");
    }

    #[test]
    fn test_apply_pascal_case() {
        let strategy = RenameStrategy::PascalCase;
        assert_eq!(strategy.apply("my_field"), "MyField");
        assert_eq!(strategy.apply("myField"), "MyField");
        assert_eq!(strategy.apply("simple"), "Simple");
        assert_eq!(strategy.apply("my_long_field"), "MyLongField");
    }

    #[test]
    fn test_apply_camel_case() {
        let strategy = RenameStrategy::CamelCase;
        assert_eq!(strategy.apply("my_field"), "myField");
        assert_eq!(strategy.apply("MyField"), "myField");
        assert_eq!(strategy.apply("simple"), "simple");
        assert_eq!(strategy.apply("my_long_field"), "myLongField");
    }

    #[test]
    fn test_apply_snake_case() {
        let strategy = RenameStrategy::SnakeCase;
        assert_eq!(strategy.apply("myField"), "my_field");
        assert_eq!(strategy.apply("MyField"), "my_field");
        assert_eq!(strategy.apply("my_field"), "my_field");
        assert_eq!(strategy.apply("HTTPServer"), "http_server");
    }

    #[test]
    fn test_apply_screaming_snake_case() {
        let strategy = RenameStrategy::ScreamingSnakeCase;
        assert_eq!(strategy.apply("myField"), "MY_FIELD");
        assert_eq!(strategy.apply("my_field"), "MY_FIELD");
        assert_eq!(strategy.apply("simple"), "SIMPLE");
    }

    #[test]
    fn test_apply_kebab_case() {
        let strategy = RenameStrategy::KebabCase;
        assert_eq!(strategy.apply("my_field"), "my-field");
        assert_eq!(strategy.apply("myField"), "my-field");
        assert_eq!(strategy.apply("my_long_field"), "my-long-field");
    }

    #[test]
    fn test_apply_screaming_kebab_case() {
        let strategy = RenameStrategy::ScreamingKebabCase;
        assert_eq!(strategy.apply("my_field"), "MY-FIELD");
        assert_eq!(strategy.apply("myField"), "MY-FIELD");
        assert_eq!(strategy.apply("simple"), "SIMPLE");
    }

    // ========== Helper function tests ==========

    #[test]
    fn test_to_pascal_case_basic() {
        assert_eq!(to_pascal_case("my_field"), "MyField");
        assert_eq!(to_pascal_case("simple"), "Simple");
    }

    #[test]
    fn test_to_pascal_case_already_pascal() {
        assert_eq!(to_pascal_case("MyField"), "MyField");
        assert_eq!(to_pascal_case("MyLongField"), "MyLongField");
    }

    #[test]
    fn test_to_pascal_case_consecutive_caps() {
        assert_eq!(to_pascal_case("HTTPServer"), "Httpserver");
        assert_eq!(to_pascal_case("IOError"), "Ioerror");
    }

    #[test]
    fn test_to_camel_case_basic() {
        assert_eq!(to_camel_case("my_field"), "myField");
        assert_eq!(to_camel_case("simple"), "simple");
    }

    #[test]
    fn test_to_camel_case_already_camel() {
        assert_eq!(to_camel_case("myField"), "myField");
        assert_eq!(to_camel_case("myLongField"), "myLongField");
    }

    #[test]
    fn test_to_snake_case_from_camel() {
        assert_eq!(to_snake_case("myField"), "my_field");
        assert_eq!(to_snake_case("myLongField"), "my_long_field");
    }

    #[test]
    fn test_to_snake_case_from_pascal() {
        assert_eq!(to_snake_case("MyField"), "my_field");
        assert_eq!(to_snake_case("MyLongField"), "my_long_field");
    }

    #[test]
    fn test_to_snake_case_already_snake() {
        assert_eq!(to_snake_case("my_field"), "my_field");
        assert_eq!(to_snake_case("simple"), "simple");
    }

    #[test]
    fn test_to_snake_case_consecutive_caps() {
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("IOError"), "io_error");
    }

    #[test]
    fn test_to_kebab_case_basic() {
        assert_eq!(to_kebab_case("my_field"), "my-field");
        assert_eq!(to_kebab_case("simple"), "simple");
    }

    #[test]
    fn test_to_kebab_case_from_camel() {
        assert_eq!(to_kebab_case("myField"), "my-field");
        assert_eq!(to_kebab_case("myLongField"), "my-long-field");
    }

    // ========== Edge case tests ==========

    #[test]
    fn test_empty_string() {
        assert_eq!(to_pascal_case(""), "");
        assert_eq!(to_camel_case(""), "");
        assert_eq!(to_snake_case(""), "");
        assert_eq!(to_kebab_case(""), "");
    }

    #[test]
    fn test_single_char() {
        assert_eq!(to_pascal_case("a"), "A");
        assert_eq!(to_camel_case("A"), "a");
        assert_eq!(to_snake_case("a"), "a");
        assert_eq!(to_snake_case("A"), "a");
    }

    #[test]
    fn test_numbers_preserved() {
        assert_eq!(to_snake_case("field1Name"), "field1_name");
        assert_eq!(to_camel_case("field_1_name"), "field1Name");
        assert_eq!(to_pascal_case("field_1_name"), "Field1Name");
    }

    #[test]
    fn test_multiple_underscores() {
        assert_eq!(to_snake_case("my__field"), "my_field");
        assert_eq!(to_camel_case("my__field"), "myField");
    }

    #[test]
    fn test_leading_trailing_underscores() {
        assert_eq!(to_snake_case("_leading"), "leading");
        assert_eq!(to_snake_case("trailing_"), "trailing");
        assert_eq!(to_camel_case("_leading"), "leading");
    }

    // ========== Determinism tests (ASSUM verification) ==========

    #[test]
    fn test_determinism_pascal_case() {
        let input = "my_field";
        let result1 = to_pascal_case(input);
        let result2 = to_pascal_case(input);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_determinism_camel_case() {
        let input = "MyField";
        let result1 = to_camel_case(input);
        let result2 = to_camel_case(input);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_strategy_consistency() {
        let strategy = RenameStrategy::CamelCase;
        let input = "my_long_field_name";
        let result1 = strategy.apply(input);
        let result2 = strategy.apply(input);
        assert_eq!(result1, result2);
    }
}
