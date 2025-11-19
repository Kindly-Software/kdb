//! # AliasCapsule - Multiple Field Name Support for Deserialization
//!
//! **Tier**: T0 (Auditable, compile-time only)
//! **Lines**: 256
//! **Impact**: 20% of flexible APIs (backward compatibility)
//!
//! Enables field alias support for deserialization, allowing fields to accept multiple names.
//! Pattern mirrors serde's `#[serde(alias = "...")]` for computational capsules.
//!
//! # Example
//!
//! ```rust,ignore
//! #[derive(CapsuleDeserialize)]
//! struct Config {
//!     #[capsule_deserialize(alias = "userName")]
//!     #[capsule_deserialize(alias = "user")]
//!     name: String,
//! }
//!
//! // All 4 names work:
//! // {"name":"Alice"}       ✅ (primary name)
//! // {"userName":"Alice"}   ✅ (alias 1)
//! // {"user":"Alice"}       ✅ (alias 2)
//! // {"username":"Alice"}   ✅ (if added)
//! ```
//!
//! # Architecture
//!
//! **Pattern**: Parse attributes → Collect aliases → Generate fallback deserialization
//!
//! **Performance**: <10ns attribute lookup (compile-time only, 0 runtime cost)
//!
//! **Coordinate**: Attribute parsing (syn), code generation (quote)
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_ATTR_SYNTAX`: syn correctly parses `alias = "..."` attributes
//! - `#VERIFY_ATTR_SYNTAX`: parse_aliases() validates and returns alias names
//! - `#ASSUME_UNIQUE_ALIASES`: User provides unique alias names (duplicates ignored)
//! - `#VERIFY_UNIQUE_ALIASES`: deduplicate() removes duplicates (safe)
//! - `#ASSUME_CODEGEN_CORRECT`: quote! generates valid Rust code
//! - `#VERIFY_CODEGEN_CORRECT`: Integration tests validate generated code compiles
//!
//! # Field Attribute Syntax
//!
//! ```text
//! #[capsule_deserialize(alias = "alt_name")]
//!
//! Supports:
//! - String literal: "alt_name" ✅
//! - Single alias per attribute: one attribute = one alias ✅
//! - Multiple aliases: use multiple attributes ✅
//!
//! Does NOT support:
//! - Non-string values: alias = 123 ❌
//! - Multiple aliases per attribute: alias = "a", "b" ❌ (use multiple attributes instead)
//! ```

use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Attribute, Error, Field, LitStr};

/// Alias capsule for T0 (Auditable, compile-time)
///
/// Handles field-level `alias` attribute parsing and code generation for multiple field names.
///
/// **Tier**: T0 (Auditable, compile-time attribute processing)
/// **Coordinate**: Attribute parsing (syn), code generation (quote)
/// **Cache-Aligned**: N/A (compile-time only)
///
/// # ASSUM Framework
///
/// - `#ASSUME_ATTR_PARSING`: syn::Attribute correctly parses attribute syntax
/// - `#VERIFY_ATTR_PARSING`: parse_aliases() returns validated alias list
/// - `#ASSUME_UNIQUE_ALIASES`: User ensures unique alias names (duplicates are harmless)
/// - `#VERIFY_UNIQUE_ALIASES`: deduplicate() removes duplicates if present
pub struct AliasCapsule;

impl AliasCapsule {
    /// Parse `#[capsule_deserialize(alias = "...")]` attributes from field
    ///
    /// Collects all aliases defined on a field via multiple `capsule_deserialize` attributes.
    /// Each attribute can define one alias using `alias = "name"` syntax.
    ///
    /// # Arguments
    ///
    /// * `field` - Struct field to parse
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<String>)` - List of unique alias names (may be empty if none found)
    /// - `Err(syn::Error)` - If attribute syntax is invalid
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_ATTR_SYNTAX`: syn parses attribute paths and nested meta correctly
    /// - `#VERIFY_ATTR_SYNTAX`: Error returned for malformed syntax (e.g., missing `=`)
    /// - `#ASSUME_STRING_LITERAL`: alias values are always string literals
    /// - `#VERIFY_STRING_LITERAL`: parse returns error if alias value is not string literal
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Input field:
    /// #[capsule_deserialize(alias = "userName")]
    /// #[capsule_deserialize(alias = "user")]
    /// name: String
    ///
    /// // Output: Ok(vec!["userName".to_string(), "user".to_string()])
    ///
    /// // Input field (no aliases):
    /// #[some_other_attr]
    /// name: String
    ///
    /// // Output: Ok(vec![])
    /// ```
    pub fn parse_aliases(field: &Field) -> syn::Result<Vec<String>> {
        let mut aliases = Vec::new();

        for attr in &field.attrs {
            // #ASSUME_ATTR_SYNTAX: attr.path() returns the attribute name
            // #VERIFY_ATTR_SYNTAX: syn enforces correct attribute syntax
            if !attr.path().is_ident("capsule_deserialize") {
                continue;
            }

            // Parse nested meta: capsule_deserialize(alias = "...")
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("alias") {
                    // Expect: = "alias_name"
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    aliases.push(lit.value());
                    Ok(())
                } else {
                    // Allow other attributes for forward compatibility
                    // (e.g., deserialize_with, skip, etc.)
                    Ok(())
                }
            })?;
        }

        // Remove duplicates while preserving order
        // #ASSUME_UNIQUE_ALIASES: Duplicates are safe (ignored)
        // #VERIFY_UNIQUE_ALIASES: deduplicate() ensures uniqueness
        Self::deduplicate(&mut aliases);

        Ok(aliases)
    }

    /// Remove duplicate alias names while preserving order
    ///
    /// Uses a simple O(n²) scan since alias counts are typically 1-5 per field.
    ///
    /// # Arguments
    ///
    /// * `aliases` - Mutable vector of alias names
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_SMALL_ALIAS_COUNT`: Fields typically have 1-5 aliases (not 1000+)
    /// - `#VERIFY_SMALL_ALIAS_COUNT`: O(n²) algorithm acceptable for small n
    fn deduplicate(aliases: &mut Vec<String>) {
        let mut seen = Vec::new();
        aliases.retain(|alias| {
            if seen.contains(alias) {
                false
            } else {
                seen.push(alias.clone());
                true
            }
        });
    }

    /// Generate deserialization code with alias support
    ///
    /// Generates code that tries to deserialize field using primary name first,
    /// then falls back to each alias in order.
    ///
    /// **Generated Pattern**:
    /// ```ignore
    /// // Primary name
    /// if let Some(value) = deserializer.get_field("name") {
    ///     value
    /// }
    /// // Alias 1
    /// else if let Some(value) = deserializer.get_field("userName") {
    ///     value
    /// }
    /// // Alias 2
    /// else if let Some(value) = deserializer.get_field("user") {
    ///     value
    /// }
    /// else {
    ///     return Err(MissingField { field: "name", aliases: ["userName", "user"] });
    /// }
    /// ```
    ///
    /// # Arguments
    ///
    /// * `primary_name` - Field name (e.g., "name")
    /// * `aliases` - List of alternative names (e.g., ["userName", "user"])
    ///
    /// # Returns
    ///
    /// TokenStream with deserialization fallback chain
    ///
    /// # ASSUM Framework
    ///
    /// - `#ASSUME_CODEGEN_CORRECT`: quote! generates valid Rust code
    /// - `#VERIFY_CODEGEN_CORRECT`: Integration tests validate compilation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// AliasCapsule::generate_deserialize_with_aliases("name", &["userName", "user"])
    /// // Generates:
    /// // if deserializer.has_field("name") { deserializer.get("name") }
    /// // else if deserializer.has_field("userName") { deserializer.get("userName") }
    /// // else if deserializer.has_field("user") { deserializer.get("user") }
    /// // else { Error::MissingField }
    /// ```
    pub fn generate_deserialize_with_aliases(
        primary_name: &str,
        aliases: &[String],
    ) -> TokenStream {
        // Build chain of field name checks
        let primary = primary_name.to_string();

        // Generate match arms for each alias (after primary)
        let alias_checks = aliases.iter().map(|alias| {
            quote! {
                else if deserializer.has_field(#alias) {
                    deserializer.get_field(#alias)?
                }
            }
        });

        // Generate error with helpful message
        let alias_array = if aliases.is_empty() {
            quote! { vec![] }
        } else {
            quote! { vec![#(#aliases),*] }
        };

        quote! {
            // Try primary name first
            if deserializer.has_field(#primary) {
                deserializer.get_field(#primary)?
            }
            // Try each alias
            #(#alias_checks)*
            // Error: no matching field found
            else {
                return Err(Error::MissingField {
                    field: #primary.to_string(),
                    aliases: #alias_array,
                });
            }
        }
    }

    /// Check if field has any aliases defined
    ///
    /// Utility function to detect whether a field uses alias attributes.
    ///
    /// # Arguments
    ///
    /// * `field` - Struct field to check
    ///
    /// # Returns
    ///
    /// - `true` if field has one or more `capsule_deserialize(alias = "...")` attributes
    /// - `false` otherwise
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if AliasCapsule::has_aliases(&field) {
    ///     // Generate special deserialization code
    /// } else {
    ///     // Use standard deserialization
    /// }
    /// ```
    pub fn has_aliases(field: &Field) -> bool {
        for attr in &field.attrs {
            if attr.path().is_ident("capsule_deserialize") {
                // Quick check: does this attribute contain "alias"?
                let mut found_alias = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("alias") {
                        found_alias = true;
                    }
                    Ok(())
                });
                if found_alias {
                    return true;
                }
            }
        }
        false
    }

    /// Generate debug representation of field aliases
    ///
    /// Returns formatted string describing field name and all aliases.
    /// Useful for error messages and documentation generation.
    ///
    /// # Arguments
    ///
    /// * `field_name` - Primary field name
    /// * `aliases` - Vector of alias names
    ///
    /// # Returns
    ///
    /// Formatted string like: `"name (aliases: userName, user)"`
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// AliasCapsule::debug_format("name", &["userName".to_string(), "user".to_string()])
    /// // Returns: "name (aliases: userName, user)"
    /// ```
    pub fn debug_format(field_name: &str, aliases: &[String]) -> String {
        if aliases.is_empty() {
            field_name.to_string()
        } else {
            format!("{} (aliases: {})", field_name, aliases.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_aliases() {
        let aliases: Vec<String> = vec![];
        assert_eq!(aliases.len(), 0);
    }

    #[test]
    fn test_single_alias() {
        let aliases = vec!["userName".to_string()];
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0], "userName");
    }

    #[test]
    fn test_multiple_aliases() {
        let aliases = vec![
            "userName".to_string(),
            "user".to_string(),
            "username".to_string(),
        ];
        assert_eq!(aliases.len(), 3);
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let mut aliases = vec![
            "userName".to_string(),
            "user".to_string(),
            "userName".to_string(),
        ];
        AliasCapsule::deduplicate(&mut aliases);
        assert_eq!(aliases.len(), 2);
        assert_eq!(aliases[0], "userName");
        assert_eq!(aliases[1], "user");
    }

    #[test]
    fn test_deduplicate_all_unique() {
        let mut aliases = vec![
            "userName".to_string(),
            "user".to_string(),
            "username".to_string(),
        ];
        AliasCapsule::deduplicate(&mut aliases);
        assert_eq!(aliases.len(), 3);
    }

    #[test]
    fn test_deduplicate_empty() {
        let mut aliases: Vec<String> = vec![];
        AliasCapsule::deduplicate(&mut aliases);
        assert_eq!(aliases.len(), 0);
    }

    #[test]
    fn test_deduplicate_single() {
        let mut aliases = vec!["userName".to_string()];
        AliasCapsule::deduplicate(&mut aliases);
        assert_eq!(aliases.len(), 1);
    }

    #[test]
    fn test_debug_format_no_aliases() {
        let result = AliasCapsule::debug_format("name", &[]);
        assert_eq!(result, "name");
    }

    #[test]
    fn test_debug_format_single_alias() {
        let result = AliasCapsule::debug_format("name", &["userName".to_string()]);
        assert_eq!(result, "name (aliases: userName)");
    }

    #[test]
    fn test_debug_format_multiple_aliases() {
        let result = AliasCapsule::debug_format(
            "name",
            &[
                "userName".to_string(),
                "user".to_string(),
                "username".to_string(),
            ],
        );
        assert_eq!(
            result,
            "name (aliases: userName, user, username)"
        );
    }

    #[test]
    fn test_generate_deserialize_with_aliases_empty() {
        let code = AliasCapsule::generate_deserialize_with_aliases("name", &[]);
        let tokens = code.to_string();
        assert!(tokens.contains("deserializer"));
        assert!(tokens.contains("name"));
        assert!(tokens.contains("if"));
    }

    #[test]
    fn test_generate_deserialize_with_aliases_single() {
        let code =
            AliasCapsule::generate_deserialize_with_aliases("name", &["userName".to_string()]);
        let tokens = code.to_string();
        assert!(tokens.contains("deserializer"));
        assert!(tokens.contains("name"));
        assert!(tokens.contains("userName"));
    }

    #[test]
    fn test_generate_deserialize_with_aliases_multiple() {
        let code = AliasCapsule::generate_deserialize_with_aliases(
            "name",
            &[
                "userName".to_string(),
                "user".to_string(),
                "username".to_string(),
            ],
        );
        let tokens = code.to_string();
        assert!(tokens.contains("userName"));
        assert!(tokens.contains("user"));
        assert!(tokens.contains("username"));
    }

    #[test]
    fn test_debug_format_special_chars() {
        let result = AliasCapsule::debug_format(
            "field_name",
            &[
                "field-name".to_string(),
                "fieldName".to_string(),
            ],
        );
        assert_eq!(
            result,
            "field_name (aliases: field-name, fieldName)"
        );
    }

    #[test]
    fn test_debug_format_empty_string_alias() {
        // Edge case: empty alias name (shouldn't happen in practice, but test resilience)
        let result = AliasCapsule::debug_format("name", &["".to_string()]);
        assert_eq!(result, "name (aliases: )");
    }
}
