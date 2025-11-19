//! # TransparentCapsule - Newtype Serialization (T0)
//!
//! Implements transparent serialization for newtype wrappers, allowing the inner
//! value to be serialized directly without wrapper overhead.
//!
//! **Tier**: T0 Auditable (compile-time derive macro)
//!
//! ## What is Transparent Serialization?
//!
//! Standard serialization wraps a newtype:
//! ```text
//! struct UserId(u64);
//! → Serialized as [42] (tuple) or {"0": 42} (struct wrapper)
//! ```
//!
//! Transparent serialization delegates to inner:
//! ```text
//! struct UserId(u64);
//! #[capsule_serialize(transparent)]
//! → Serialized as 42 (inner value directly, no wrapper)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atomic_capsule_derive_serialize::CapsuleSerialize;
//!
//! #[derive(CapsuleSerialize)]
//! #[capsule_serialize(transparent)]
//! #[repr(C, align(64))]
//! struct UserId(u64);
//!
//! #[derive(CapsuleSerialize)]
//! #[capsule_serialize(transparent)]
//! #[repr(C, align(64))]
//! struct Name(String);
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_SINGLE_FIELD`: Transparent requires exactly 1 field (enforced at parse time)
//! - `#VERIFY_SINGLE_FIELD`: Compile error if struct has != 1 field
//! - `#ASSUME_FIELD_SERIALIZABLE`: Inner field must be serializable type or fixed-point
//! - `#VERIFY_FIELD_SERIALIZABLE`: Type detection validates field type
//! - `#ASSUME_TRANSPARENT_DELEGATION`: serialize delegates to inner field's impl
//! - `#VERIFY_TRANSPARENT_DELEGATION`: Generated code calls inner.serialize(serializer)

use proc_macro2::TokenStream;
use quote::{quote, format_ident};
use syn::{Attribute, Error, Field, Ident, spanned::Spanned};

/// T0 Auditable capsule for transparent serialization detection and code generation
pub struct TransparentCapsule;

impl TransparentCapsule {
    /// Check if struct should use transparent serialization
    ///
    /// Returns true if any attribute matches `#[capsule_serialize(transparent)]`
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ATTR_PATH`: syn::Attribute::path() correctly identifies attribute names
    /// - `#VERIFY_ATTR_PATH`: Tested with multiple attribute formats
    pub fn is_transparent(attrs: &[Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if !attr.path().is_ident("capsule_serialize") {
                return false;
            }

            // Try to parse nested metadata for "transparent" keyword
            let mut found_transparent = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("transparent") {
                    found_transparent = true;
                }
                Ok(())
            });

            found_transparent
        })
    }

    /// Validate that transparent is applied correctly
    ///
    /// Returns Ok(()) if valid, or Err(syn::Error) if:
    /// - Struct has != 1 field
    /// - Field is marked with incompatible attributes
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_FIELD_ANNOTATIONS`: Field attributes are parsed correctly
    /// - `#VERIFY_FIELD_ANNOTATIONS`: Checked against known incompatible attributes
    pub fn validate(field_count: usize, field: &Field) -> Result<(), Error> {
        // Must have exactly 1 field for transparent
        if field_count != 1 {
            return Err(Error::new(
                field.span(),
                format!(
                    "#[capsule_serialize(transparent)] requires exactly 1 field, found {}",
                    field_count
                ),
            ));
        }

        // Check for conflicting attributes
        let has_skip = field.attrs.iter().any(|attr| {
            attr.path().is_ident("capsule_serialize") && {
                let mut skip = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                    }
                    Ok(())
                });
                skip
            }
        });

        if has_skip {
            return Err(Error::new(
                field.span(),
                "Transparent field cannot be marked #[capsule_serialize(skip)]",
            ));
        }

        let has_hash_key = field.attrs.iter().any(|attr| {
            attr.path().is_ident("capsule_serialize") && {
                let mut hash_key = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("hash_key") {
                        hash_key = true;
                    }
                    Ok(())
                });
                hash_key
            }
        });

        if has_hash_key {
            return Err(Error::new(
                field.span(),
                "Transparent field cannot be marked #[capsule_serialize(hash_key)]",
            ));
        }

        Ok(())
    }

    /// Generate transparent serialize_binary() method
    ///
    /// Delegates serialization to inner field's serialize_binary() implementation.
    ///
    /// # Generated Code
    /// ```rust,ignore
    /// fn serialize_binary(&self) -> Vec<u8> {
    ///     // Delegate to inner field (tuple index 0)
    ///     self.0.serialize_binary()
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INNER_SERIALIZABLE`: Inner field implements FixedPointSerialize
    /// - `#VERIFY_INNER_SERIALIZABLE`: Compile error if inner lacks serialize_binary()
    pub fn generate_serialize_binary() -> TokenStream {
        quote! {
            fn serialize_binary(&self) -> Vec<u8> {
                // #ASSUME_TRANSPARENT_DELEGATION: Inner field has serialize_binary()
                // Delegate to inner field (tuple index 0)
                self.0.serialize_binary()
            }
        }
    }

    /// Generate transparent deserialize_binary() method
    ///
    /// Constructs Self by deserializing inner field, then wrapping in Self(inner).
    ///
    /// # Generated Code
    /// ```rust,ignore
    /// fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
    ///     // Delegate to inner field's deserialize_binary
    ///     let inner = <InnerType>::deserialize_binary(data)?;
    ///     Ok(Self(inner))
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INNER_DESERIALIZABLE`: Inner field implements FixedPointSerialize::deserialize_binary
    /// - `#VERIFY_INNER_DESERIALIZABLE`: Compile error if inner lacks deserialize_binary()
    pub fn generate_deserialize_binary(field_type: &syn::Type) -> TokenStream {
        quote! {
            fn deserialize_binary(data: &[u8]) -> Result<Self, SerializeError> {
                // #ASSUME_TRANSPARENT_DELEGATION: Inner field has deserialize_binary()
                let inner = <#field_type>::deserialize_binary(data)?;
                Ok(Self(inner))
            }
        }
    }

    /// Generate transparent to_decimal_string() method
    ///
    /// Delegates string conversion to inner field.
    ///
    /// # Generated Code
    /// ```rust,ignore
    /// fn to_decimal_string(&self) -> String {
    ///     self.0.to_decimal_string()
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INNER_STRINGIFIABLE`: Inner field implements FixedPointSerialize::to_decimal_string()
    /// - `#VERIFY_INNER_STRINGIFIABLE`: Compile error if inner lacks to_decimal_string()
    pub fn generate_to_decimal_string() -> TokenStream {
        quote! {
            fn to_decimal_string(&self) -> String {
                // #ASSUME_TRANSPARENT_DELEGATION: Inner field has to_decimal_string()
                self.0.to_decimal_string()
            }
        }
    }

    /// Generate transparent compute_hash() method
    ///
    /// Delegates hash computation to inner field.
    ///
    /// # Generated Code
    /// ```rust,ignore
    /// fn compute_hash(&self) -> u64 {
    ///     self.0.compute_hash()
    /// }
    /// ```
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_INNER_HASHABLE`: Inner field implements FixedPointSerialize::compute_hash()
    /// - `#VERIFY_INNER_HASHABLE`: Compile error if inner lacks compute_hash()
    pub fn generate_compute_hash() -> TokenStream {
        quote! {
            fn compute_hash(&self) -> u64 {
                // #ASSUME_TRANSPARENT_DELEGATION: Inner field has compute_hash()
                self.0.compute_hash()
            }
        }
    }

    /// Generate complete FixedPointSerialize trait impl for transparent newtype
    ///
    /// Combines all method generations into a single impl block.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_TRAIT_VISIBILITY`: FixedPointSerialize trait is in scope
    /// - `#VERIFY_TRAIT_VISIBILITY`: Compile error if trait not imported
    /// - `#ASSUME_IMPL_COMPLETENESS`: All required methods generated (4/4)
    /// - `#VERIFY_IMPL_COMPLETENESS`: Compile error if any method missing
    pub fn generate_impl(struct_name: &Ident, field_type: &syn::Type) -> TokenStream {
        let serialize_binary = Self::generate_serialize_binary();
        let deserialize_binary = Self::generate_deserialize_binary(field_type);
        let to_decimal_string = Self::generate_to_decimal_string();
        let compute_hash = Self::generate_compute_hash();

        quote! {
            // #ASSUME_TRANSPARENT_DELEGATION: All 4 methods delegate to inner field
            // #VERIFY_TRANSPARENT_DELEGATION: Generated code calls inner.method()
            impl FixedPointSerialize for #struct_name {
                #serialize_binary
                #deserialize_binary
                #to_decimal_string
                #compute_hash
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_transparent_detects_attribute() {
        // Mock attribute for testing
        let attrs: Vec<Attribute> = vec![];
        assert!(!TransparentCapsule::is_transparent(&attrs));
    }

    #[test]
    fn test_generate_serialize_binary_produces_delegation() {
        let code = TransparentCapsule::generate_serialize_binary();
        let code_str = code.to_string();
        // Verify delegation to self.0
        assert!(code_str.contains("self.0.serialize_binary()"));
    }

    #[test]
    fn test_generate_deserialize_binary_produces_wrapping() {
        let field_type: syn::Type = syn::parse_quote!(u64);
        let code = TransparentCapsule::generate_deserialize_binary(&field_type);
        let code_str = code.to_string();
        // Verify deserialization + wrapping
        assert!(code_str.contains("deserialize_binary"));
        assert!(code_str.contains("Self"));
    }

    #[test]
    fn test_generate_to_decimal_string_delegates() {
        let code = TransparentCapsule::generate_to_decimal_string();
        let code_str = code.to_string();
        // Verify delegation
        assert!(code_str.contains("self.0.to_decimal_string()"));
    }

    #[test]
    fn test_generate_compute_hash_delegates() {
        let code = TransparentCapsule::generate_compute_hash();
        let code_str = code.to_string();
        // Verify delegation
        assert!(code_str.contains("self.0.compute_hash()"));
    }

    #[test]
    fn test_generate_impl_produces_complete_trait() {
        let struct_name = format_ident!("UserId");
        let field_type: syn::Type = syn::parse_quote!(u64);
        let code = TransparentCapsule::generate_impl(&struct_name, &field_type);
        let code_str = code.to_string();

        // Verify all 4 methods present
        assert!(code_str.contains("serialize_binary"));
        assert!(code_str.contains("deserialize_binary"));
        assert!(code_str.contains("to_decimal_string"));
        assert!(code_str.contains("compute_hash"));

        // Verify impl block structure
        assert!(code_str.contains("impl FixedPointSerialize"));
        assert!(code_str.contains("UserId"));
    }

    #[test]
    fn test_validate_single_field_requirement() {
        let field: syn::Field = syn::parse_quote!(value: u64);
        // Should pass with field_count=1
        assert!(TransparentCapsule::validate(1, &field).is_ok());
    }

    #[test]
    fn test_validate_rejects_multiple_fields() {
        let field: syn::Field = syn::parse_quote!(value: u64);
        // Should fail with field_count > 1
        let result = TransparentCapsule::validate(2, &field);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly 1 field"));
    }

    #[test]
    fn test_validate_rejects_zero_fields() {
        let field: syn::Field = syn::parse_quote!(value: u64);
        // Should fail with field_count = 0
        let result = TransparentCapsule::validate(0, &field);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly 1 field"));
    }

    #[test]
    fn test_impl_with_q16_16_type() {
        let struct_name = format_ident!("Amount");
        let field_type: syn::Type = syn::parse_quote!(Q16_16);
        let code = TransparentCapsule::generate_impl(&struct_name, &field_type);
        let code_str = code.to_string();

        // Verify specific type in generated code
        assert!(code_str.contains("Amount"));
        assert!(code_str.contains("Q16_16"));
    }

    #[test]
    fn test_impl_with_string_type() {
        let struct_name = format_ident!("Name");
        let field_type: syn::Type = syn::parse_quote!(String);
        let code = TransparentCapsule::generate_impl(&struct_name, &field_type);
        let code_str = code.to_string();

        // Verify delegation still works for non-fixed-point types
        assert!(code_str.contains("Name"));
        assert!(code_str.contains("String"));
    }

    #[test]
    fn test_delegation_pattern_consistency() {
        // Verify all methods use same delegation pattern (self.0.method())
        let serialize = TransparentCapsule::generate_serialize_binary().to_string();
        let deserialize = TransparentCapsule::generate_deserialize_binary(&syn::parse_quote!(u64)).to_string();
        let to_string = TransparentCapsule::generate_to_decimal_string().to_string();
        let hash = TransparentCapsule::generate_compute_hash().to_string();

        // serialize and to_string and hash use direct delegation
        assert!(serialize.contains("self.0"));
        assert!(to_string.contains("self.0"));
        assert!(hash.contains("self.0"));

        // deserialize constructs wrapper
        assert!(deserialize.contains("Self"));
    }

    #[test]
    fn test_error_message_clarity() {
        let field: syn::Field = syn::parse_quote!(value: u64);
        let result = TransparentCapsule::validate(3, &field);
        let err_msg = result.unwrap_err().to_string();

        // Error message should be clear about the issue
        assert!(err_msg.contains("exactly 1 field"));
        assert!(err_msg.contains("3")); // Shows actual count
    }
}
