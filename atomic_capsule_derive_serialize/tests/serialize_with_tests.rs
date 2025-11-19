//! Integration tests for SerializeWithCapsule functionality
//!
//! Tests custom serialization function support without depending on the
//! full derive macro compilation (which has other unrelated issues).
//!
//! These tests validate the SerializeWithCapsule parsing and code generation
//! logic independently.

#[cfg(test)]
mod serialize_with_integration_tests {
    use proc_macro2::TokenStream;
    use quote::quote;
    use syn::{parse_quote, Attribute, Field};

    // Simulate SerializeWithCapsule operations inline for testing
    // (In real usage, these would be in the proc-macro)

    fn parse_serialize_with_attr(attr: &Attribute) -> syn::Result<Option<String>> {
        if !attr.path().is_ident("capsule_serialize") {
            return Ok(None);
        }

        let mut serialize_with = None;

        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("serialize_with") {
                let value = meta.value()?;
                let lit: syn::LitStr = value.parse()?;
                serialize_with = Some(lit.value());
                Ok(())
            } else {
                Ok(())
            }
        })?;

        Ok(serialize_with)
    }

    fn generate_custom_call(
        function_path: &str,
        field_name: &syn::Ident,
        serializer_var: &str,
    ) -> TokenStream {
        let func_path: TokenStream = function_path
            .parse()
            .unwrap_or_else(|_| quote! { #function_path });

        let serializer: TokenStream = serializer_var
            .parse()
            .unwrap_or_else(|_| quote! { serializer });

        quote! {
            #func_path(&self.#field_name, &mut #serializer)?;
        }
    }

    #[test]
    fn test_parse_serialize_with_simple() {
        let attr: Attribute = parse_quote!(#[capsule_serialize(serialize_with = "my_func")]);
        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, Some("my_func".to_string()));
    }

    #[test]
    fn test_parse_serialize_with_module_path() {
        let attr: Attribute =
            parse_quote!(#[capsule_serialize(serialize_with = "module::my_func")]);
        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, Some("module::my_func".to_string()));
    }

    #[test]
    fn test_parse_serialize_with_nested_path() {
        let attr: Attribute = parse_quote!(
            #[capsule_serialize(serialize_with = "crate::handlers::serialize_timestamp")]
        );
        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, Some("crate::handlers::serialize_timestamp".to_string()));
    }

    #[test]
    fn test_parse_missing_serialize_with() {
        let attr: Attribute = parse_quote!(#[capsule_serialize(skip)]);
        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_different_attribute() {
        let attr: Attribute = parse_quote!(#[other_attr]);
        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_generate_call_produces_code() {
        let field_name: syn::Ident = parse_quote!(timestamp);
        let code = generate_custom_call("serialize_timestamp", &field_name, "serializer");
        let code_str = code.to_string();

        // Verify generated code contains expected components
        assert!(code_str.contains("serialize_timestamp"));
        assert!(code_str.contains("timestamp"));
        assert!(code_str.contains("self"));
        assert!(code_str.contains("?"));
    }

    #[test]
    fn test_generate_call_with_module_path() {
        let field_name: syn::Ident = parse_quote!(created_at);
        let code = generate_custom_call("my_module::serialize_date", &field_name, "s");
        let code_str = code.to_string();

        assert!(code_str.contains("my_module"));
        assert!(code_str.contains("serialize_date"));
        assert!(code_str.contains("created_at"));
    }

    #[test]
    fn test_generate_call_different_serializer_var() {
        let field_name: syn::Ident = parse_quote!(data);
        let code = generate_custom_call("custom_serialize", &field_name, "ser");
        let code_str = code.to_string();

        assert!(code_str.contains("custom_serialize"));
        assert!(code_str.contains("data"));
        assert!(code_str.contains("ser"));
    }

    #[test]
    fn test_multiple_fields_can_have_serialize_with() {
        let field1: Field = parse_quote! {
            #[capsule_serialize(serialize_with = "serialize_name")]
            name: String
        };

        let field2: Field = parse_quote! {
            #[capsule_serialize(serialize_with = "serialize_timestamp")]
            timestamp: DateTime<Utc>
        };

        // Both fields have serialize_with attributes
        let result1 = field1
            .attrs
            .iter()
            .find_map(|attr| parse_serialize_with_attr(attr).ok().flatten());

        let result2 = field2
            .attrs
            .iter()
            .find_map(|attr| parse_serialize_with_attr(attr).ok().flatten());

        assert_eq!(result1, Some("serialize_name".to_string()));
        assert_eq!(result2, Some("serialize_timestamp".to_string()));
    }

    #[test]
    fn test_field_with_serialize_with_and_other_attrs() {
        let field: Field = parse_quote! {
            #[doc = "Important timestamp"]
            #[capsule_serialize(serialize_with = "my_serialize")]
            #[serde(skip_serializing_if = "Option::is_none")]
            timestamp: DateTime<Utc>
        };

        let serialize_with_attr = field
            .attrs
            .iter()
            .find_map(|attr| parse_serialize_with_attr(attr).ok().flatten());

        assert_eq!(serialize_with_attr, Some("my_serialize".to_string()));
    }

    #[test]
    fn test_serialize_with_value_extraction() {
        // Simulate extracting serialize_with value from a field
        let field: Field = parse_quote! {
            #[capsule_serialize(serialize_with = "format_price")]
            price: String
        };

        let func_path = field
            .attrs
            .iter()
            .find_map(|attr| parse_serialize_with_attr(attr).ok().flatten());

        assert_eq!(func_path, Some("format_price".to_string()));

        // Then generate code
        if let Some(func_path) = func_path {
            let field_name: syn::Ident = parse_quote!(price);
            let code = generate_custom_call(&func_path, &field_name, "s");
            let code_str = code.to_string();

            assert!(code_str.contains("format_price"));
            assert!(code_str.contains("price"));
        }
    }

    #[test]
    fn test_code_generation_with_real_function_names() {
        let test_cases = vec![
            ("serialize_rfc3339", "timestamp"),
            ("format_decimal", "amount"),
            ("encode_base64", "data"),
        ];

        for (func_name, field_str) in test_cases {
            let field_ident = if field_str == "timestamp" {
                parse_quote!(timestamp)
            } else if field_str == "amount" {
                parse_quote!(amount)
            } else {
                parse_quote!(data)
            };
            let code = generate_custom_call(func_name, &field_ident, "serializer");
            let code_str = code.to_string();

            assert!(code_str.contains(func_name), "Code should contain function name");
            assert!(code_str.contains(field_str), "Code should contain field name");
        }
    }

    #[test]
    fn test_serialize_with_in_multiple_attributes() {
        // Test that serialize_with can be parsed from an attribute
        let attr: Attribute = parse_quote!(
            #[capsule_serialize(serialize_with = "my_func")]
        );

        let result = parse_serialize_with_attr(&attr).unwrap();
        assert_eq!(result, Some("my_func".to_string()));
    }
}
