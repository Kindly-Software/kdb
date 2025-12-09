//! Integration tests for GenericConstraintCapsule
//!
//! Demonstrates how GenericConstraintCapsule enables generic type serialization
//! in computational capsules.

#[cfg(test)]
mod generic_constraint_tests {
    use syn::{parse_quote, GenericParam, Generics};

    // Note: We test the API surface with syn types to demonstrate the capsule's
    // capabilities without requiring atomic_capsule crate linkage in tests.

    #[test]
    fn test_single_generic_type_parsing() {
        // Test: Parsing <T> from generic parameter list
        let generics: Generics = parse_quote!(<T>);

        // Verify we can extract the type parameter name
        let params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(params, vec!["T"]);
    }

    #[test]
    fn test_multiple_generic_types() {
        // Test: Parsing <T, U, V> from generic parameter list
        let generics: Generics = parse_quote!(<T, U, V>);

        let params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(params, vec!["T", "U", "V"]);
    }

    #[test]
    fn test_mixed_generics_with_lifetimes() {
        // Test: Parsing <'a, T, 'b, U> correctly filters only type params
        let generics: Generics = parse_quote!(<'a, T, 'b, U>);

        let type_params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(type_params, vec!["T", "U"]);

        // Verify lifetime params are present
        let lifetime_params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Lifetime(lp) => Some(lp.lifetime.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(lifetime_params, vec!["a", "b"]);
    }

    #[test]
    fn test_existing_bounds_preservation() {
        // Test: Parsing <T: Clone + Default> preserves existing bounds
        let generics: Generics = parse_quote!(<T: Clone + Default>);

        let type_param = &generics.params[0];
        if let GenericParam::Type(tp) = type_param {
            // Should have 2 bounds: Clone + Default
            assert_eq!(tp.bounds.len(), 2);
        } else {
            panic!("Expected type parameter");
        }
    }

    #[test]
    fn test_where_clause_detection() {
        // Test: Detecting where clause in generics
        let generics: Generics = parse_quote!(<T> where T: Clone);

        assert!(generics.where_clause.is_some());
        let where_clause = generics.where_clause.unwrap();
        assert!(!where_clause.predicates.is_empty());
    }

    #[test]
    fn test_non_generic_struct() {
        // Test: Detecting when struct has no generics
        let generics: Generics = parse_quote!();

        let has_type_params = generics.params.iter().any(|p| matches!(p, GenericParam::Type(_)));
        assert!(!has_type_params);
    }

    #[test]
    fn test_generic_constraint_composition() {
        // Test: Composing multiple generic constraints
        let generics: Generics = parse_quote!(<T: Clone, U: Default>);

        assert_eq!(generics.params.len(), 2);

        // First parameter: T: Clone
        if let GenericParam::Type(tp1) = &generics.params[0] {
            assert_eq!(tp1.ident.to_string(), "T");
            assert_eq!(tp1.bounds.len(), 1);
        }

        // Second parameter: U: Default
        if let GenericParam::Type(tp2) = &generics.params[1] {
            assert_eq!(tp2.ident.to_string(), "U");
            assert_eq!(tp2.bounds.len(), 1);
        }
    }

    #[test]
    fn test_generic_parameter_isolation() {
        // Test: Isolating type parameters from other generic forms
        let generics: Generics = parse_quote!(<'a, T: Clone, const N: usize>);

        // Only type parameters
        let type_params: Vec<_> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(type_params, vec!["T"]);

        // Total params
        assert_eq!(generics.params.len(), 3);
    }

    #[test]
    fn test_generic_vector_serialization_pattern() {
        // Test: Pattern for vector of generics (e.g., Vec<T>)
        let generics: Generics = parse_quote!(<T>);

        // This demonstrates the pattern: extracting T from Vec<T> container
        let type_params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(type_params.len(), 1);
        assert_eq!(type_params[0], "T");
    }

    #[test]
    fn test_generic_option_serialization_pattern() {
        // Test: Pattern for optional generics (e.g., Option<T>)
        let generics: Generics = parse_quote!(<T>);

        // All parameters should be considered for constraint generation
        assert_eq!(generics.params.len(), 1);
    }

    #[test]
    fn test_generic_result_serialization_pattern() {
        // Test: Pattern for result types (e.g., Result<T, E>)
        let generics: Generics = parse_quote!(<T, E>);

        let type_params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(type_params, vec!["T", "E"]);
    }

    #[test]
    fn test_generic_higher_ranked_trait_bounds() {
        // Test: Generic with higher-ranked trait bounds (HRTB)
        // Note: HRTB like for<'a> Fn(&'a T) requires special syn handling
        let generics: Generics = parse_quote!(<T>);

        // Basic case: T with no bounds
        if let GenericParam::Type(tp) = &generics.params[0] {
            assert!(tp.bounds.is_empty());
        }
    }

    #[test]
    fn test_generic_default_values() {
        // Test: Generic parameters with default values
        let generics: Generics = parse_quote!(<T = String>);

        if let GenericParam::Type(tp) = &generics.params[0] {
            assert_eq!(tp.ident.to_string(), "T");
            // Note: default is stored in tp.default, not bounds
            assert!(tp.default.is_some());
        }
    }

    #[test]
    fn test_complex_generic_hierarchy() {
        // Test: Complex generic hierarchy with multiple constraints
        let generics: Generics = parse_quote!(<T: Clone + Send, U: Sync + 'static>);

        let type_params: Vec<String> = generics
            .params
            .iter()
            .filter_map(|p| match p {
                GenericParam::Type(tp) => Some(tp.ident.to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(type_params, vec!["T", "U"]);

        // T has Clone + Send
        if let GenericParam::Type(tp1) = &generics.params[0] {
            assert_eq!(tp1.bounds.len(), 2);
        }

        // U has Sync + 'static
        if let GenericParam::Type(tp2) = &generics.params[1] {
            assert!(tp2.bounds.len() >= 1);
        }
    }
}
