//! # Utility Functions
//!
//! Common helper functions used across the derive macro implementation.

use syn::{DeriveInput, Field, Fields};

/// Extract named fields from a struct.
///
/// Returns `None` if the input is not a struct with named fields.
///
/// # Example
///
/// ```rust,ignore
/// if let Some(fields) = extract_named_fields(&input) {
///     for field in fields {
///         // Process field...
///     }
/// }
/// ```
pub fn extract_named_fields(
    input: &DeriveInput,
) -> Option<&syn::punctuated::Punctuated<Field, syn::token::Comma>> {
    match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => Some(&fields_named.named),
            _ => None,
        },
        _ => None,
    }
}

/// Check if a field name should be excluded from processing.
///
/// Returns `true` for:
/// - Padding fields (`_padding*`, `_pad*`)
/// - Hash metadata fields (for auditable capsules)
///
/// # Example
///
/// ```rust,ignore
/// for field in fields {
///     if is_excluded_field(&field.ident.as_ref().unwrap().to_string()) {
///         continue;
///     }
///     // Process field...
/// }
/// ```
pub fn is_excluded_field(field_name: &str) -> bool {
    is_padding_field(field_name) || is_hash_field(field_name)
}

/// Check if a field name is a padding field.
///
/// Returns `true` for fields starting with `_padding` or `_pad`.
pub fn is_padding_field(field_name: &str) -> bool {
    field_name.starts_with("_padding") || field_name.starts_with("_pad")
}

/// Check if a field name is a hash or metadata field.
///
/// Returns `true` for fields used in auditable capsules:
/// - `fast_hash`, `prev_fast_hash`
/// - `generation`, `timestamp_ns`
/// - `crypto_hash`, `prev_crypto_hash`
pub fn is_hash_field(field_name: &str) -> bool {
    matches!(
        field_name,
        "fast_hash"
            | "prev_fast_hash"
            | "generation"
            | "timestamp_ns"
            | "crypto_hash"
            | "prev_crypto_hash"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_extract_named_fields() {
        let input: DeriveInput = parse_quote! {
            struct TestStruct {
                field1: u64,
                field2: String,
            }
        };

        let fields = extract_named_fields(&input);
        assert!(fields.is_some());
        assert_eq!(fields.unwrap().len(), 2);
    }

    #[test]
    fn test_extract_named_fields_tuple_struct() {
        let input: DeriveInput = parse_quote! {
            struct TestStruct(u64, String);
        };

        let fields = extract_named_fields(&input);
        assert!(fields.is_none());
    }

    #[test]
    fn test_is_excluded_field_padding() {
        assert!(is_excluded_field("_padding"));
        assert!(is_excluded_field("_padding0"));
        assert!(is_excluded_field("_pad"));
        assert!(is_excluded_field("_pad_extra"));
        assert!(!is_excluded_field("padding")); // No underscore prefix
    }

    #[test]
    fn test_is_excluded_field_hash() {
        assert!(is_excluded_field("fast_hash"));
        assert!(is_excluded_field("prev_fast_hash"));
        assert!(is_excluded_field("generation"));
        assert!(is_excluded_field("timestamp_ns"));
        assert!(is_excluded_field("crypto_hash"));
        assert!(is_excluded_field("prev_crypto_hash"));
        assert!(!is_excluded_field("my_hash")); // Not a standard hash field
    }

    #[test]
    fn test_is_padding_field() {
        assert!(is_padding_field("_padding"));
        assert!(is_padding_field("_pad"));
        assert!(!is_padding_field("padding"));
        assert!(!is_padding_field("fast_hash"));
    }

    #[test]
    fn test_is_hash_field() {
        assert!(is_hash_field("fast_hash"));
        assert!(is_hash_field("generation"));
        assert!(!is_hash_field("_padding"));
        assert!(!is_hash_field("state"));
    }
}
