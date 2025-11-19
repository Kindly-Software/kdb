//! Integration test for DeserializeWithCapsule
//!
//! Tests custom field-level deserialization via #[capsule_deserialize(deserialize_with = "...")]
//!
//! Note: Testing DeserializeWithCapsule patterns inline since proc-macro crates
//! cannot export public modules. These tests validate the parsing and code generation
//! logic independently.

#[cfg(test)]
mod deserialize_with_tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::{parse_quote, spanned::Spanned, Attribute, Error, Field, LitStr, Type};

    /// Copy of DeserializeWithCapsule for testing (mirrors src/deserialize_with.rs)
    struct DeserializeWithCapsule;

    impl DeserializeWithCapsule {
        fn parse_attr(attr: &Attribute) -> syn::Result<Option<String>> {
            if !attr.path().is_ident("capsule_deserialize") {
                return Ok(None);
            }

            let mut deserialize_with = None;

            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("deserialize_with") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    deserialize_with = Some(lit.value());
                    Ok(())
                } else {
                    Ok(())
                }
            })?;

            Ok(deserialize_with)
        }

        fn has_deserialize_with(field: &Field) -> bool {
            field
                .attrs
                .iter()
                .any(|attr| {
                    if let Ok(Some(_)) = Self::parse_attr(attr) {
                        true
                    } else {
                        false
                    }
                })
        }

        fn extract_function_path(field: &Field) -> syn::Result<Option<String>> {
            for attr in &field.attrs {
                if let Some(path) = Self::parse_attr(attr)? {
                    return Ok(Some(path));
                }
            }
            Ok(None)
        }

        fn generate_call(
            function_path: &str,
            _field_type: &Type,
            deserializer_var: &TokenStream,
        ) -> TokenStream {
            let func: TokenStream = match function_path.parse() {
                Ok(ts) => ts,
                Err(_) => {
                    return quote! {
                        compile_error!(concat!("Invalid function path for deserialize_with: ", #function_path))
                    };
                }
            };

            quote! {
                {
                    #func(#deserializer_var)?
                }
            }
        }

        fn generate_field_deserialization(
            field_name: &syn::Ident,
            function_path: &str,
            field_type: &Type,
            deserializer: &TokenStream,
        ) -> TokenStream {
            let call = Self::generate_call(function_path, field_type, deserializer);

            quote! {
                let #field_name = #call;
            }
        }

        fn validate_compatibility(
            field: &Field,
            function_path: &str,
        ) -> syn::Result<()> {
            if function_path.is_empty() {
                return Err(Error::new(
                    field.span(),
                    "deserialize_with function path cannot be empty",
                ));
            }

            if !function_path
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
            {
                return Err(Error::new(
                    field.span(),
                    format!(
                        "Invalid function path '{}': contains invalid characters. Use 'module::function' format.",
                        function_path
                    ),
                ));
            }

            Ok(())
        }
    }

    // ────────────────────────────────────────────────────────────────────────
    // Unit Tests (4): Attribute parsing
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_deserialize_with_basic_parsing() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "parse_value")]
            value: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("parse_value".to_string()));
    }

    #[test]
    fn test_deserialize_with_module_path() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "utils::parse_timestamp")]
            timestamp: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("utils::parse_timestamp".to_string()));
    }

    #[test]
    fn test_deserialize_with_nested_module_path() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "api::utils::time::parse_rfc3339")]
            timestamp: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("api::utils::time::parse_rfc3339".to_string()));
    }

    #[test]
    fn test_has_deserialize_with_detection() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "custom")]
            value: u64
        };

        assert!(DeserializeWithCapsule::has_deserialize_with(&field));
    }

    // ────────────────────────────────────────────────────────────────────────
    // Property Tests (4): Attribute detection and validation
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_no_deserialize_with_returns_false() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(skip)]
            value: u64
        };

        assert!(!DeserializeWithCapsule::has_deserialize_with(&field));
    }

    #[test]
    fn test_extract_none_when_missing() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(skip)]
            value: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_validate_valid_path() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "parse_fn")]
            value: u64
        };

        let result = DeserializeWithCapsule::validate_compatibility(&field, "parse_fn");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_characters() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "parse!invalid")]
            value: u64
        };

        let result = DeserializeWithCapsule::validate_compatibility(&field, "parse!invalid");
        assert!(result.is_err());
    }

    // ────────────────────────────────────────────────────────────────────────
    // Integration Tests (6): Code generation + type compatibility
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_generate_call_simple() {
        let deser = quote! { deserializer };
        let field_type: Type = parse_quote! { u64 };

        let result = DeserializeWithCapsule::generate_call("parse_value", &field_type, &deser);
        let result_str = result.to_string();

        assert!(result_str.contains("parse_value"));
        assert!(result_str.contains("deserializer"));
        assert!(result_str.contains("?"));
    }

    #[test]
    fn test_generate_call_module_path() {
        let deser = quote! { buffer };
        let field_type: Type = parse_quote! { String };

        let result = DeserializeWithCapsule::generate_call("utils::parse_string", &field_type, &deser);
        let result_str = result.to_string();

        assert!(result_str.contains("utils"));
        assert!(result_str.contains("parse_string"));
    }

    #[test]
    fn test_generate_field_deserialization() {
        let field_name: syn::Ident = parse_quote! { timestamp };
        let field_type: Type = parse_quote! { u64 };
        let deser = quote! { deserializer };

        let result = DeserializeWithCapsule::generate_field_deserialization(
            &field_name,
            "parse_timestamp",
            &field_type,
            &deser,
        );

        let result_str = result.to_string();

        assert!(result_str.contains("let timestamp"));
        assert!(result_str.contains("parse_timestamp"));
    }

    #[test]
    fn test_generate_code_preserves_deserializer_context() {
        let deser = quote! { d };
        let field_type: Type = parse_quote! { i32 };

        let result = DeserializeWithCapsule::generate_call("parse_i32", &field_type, &deser);
        let result_str = result.to_string();

        assert!(result_str.contains("d"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // Edge Case Tests (4): Boundary conditions
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_empty_function_path_validation() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "")]
            value: u64
        };

        let result = DeserializeWithCapsule::validate_compatibility(&field, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_underscore_in_function_name() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "parse_my_value")]
            value: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("parse_my_value".to_string()));

        let validate = DeserializeWithCapsule::validate_compatibility(&field, result.unwrap().as_str());
        assert!(validate.is_ok());
    }

    #[test]
    fn test_numeric_suffix_in_function() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "parse64")]
            value: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("parse64".to_string()));

        let validate = DeserializeWithCapsule::validate_compatibility(&field, result.unwrap().as_str());
        assert!(validate.is_ok());
    }

    #[test]
    fn test_multiple_consecutive_colons() {
        let field: Field = parse_quote! {
            #[capsule_deserialize(deserialize_with = "a::b::c::d::parser")]
            value: u64
        };

        let result = DeserializeWithCapsule::extract_function_path(&field).unwrap();
        assert_eq!(result, Some("a::b::c::d::parser".to_string()));

        let validate = DeserializeWithCapsule::validate_compatibility(&field, result.unwrap().as_str());
        assert!(validate.is_ok());
    }
}
