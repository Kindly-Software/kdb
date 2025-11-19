//! Generic constraint capsule (T0 Auditable - compile-time proc macro)
//!
//! Provides compile-time type constraint handling for generic types in #[derive(CapsuleSerialize)].
//! Enables automatic trait bound propagation for generic parameters.
//!
//! # Purpose
//!
//! When deriving CapsuleSerialize for generic structs, all type parameters must be
//! constrained to implement CapsuleSerialize. This module automates that process.
//!
//! # Example
//!
//! ```rust,ignore
//! // Before: Manual bounds required on every use site
//! #[derive(CapsuleSerialize)]
//! struct Wrapper<T: CapsuleSerialize> {
//!     value: T,
//! }
//!
//! // After: GenericConstraintCapsule handles bounds automatically
//! #[derive(CapsuleSerialize)]
//! struct Wrapper<T> {
//!     value: T,
//! }
//! // Macro automatically adds: T: CapsuleSerialize
//! ```
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_GENERICS_EXTRACTABLE`: syn::Generics can be safely cloned and modified
//! - `#VERIFY_GENERICS`: Proc-macro errors if invalid generic syntax found
//! - `#ASSUME_BOUNDS_PROPAGATE`: All type parameters should have CapsuleSerialize bound
//! - `#VERIFY_BOUNDS`: Compile error if type parameter lacks required trait
//! - `#ASSUME_WHERE_CLAUSE_MERGE`: Where clause predicates can be safely merged
//! - `#VERIFY_WHERE_CLAUSE`: Compile-time verification of where clause syntax
//!
//! # Design Philosophy (IMPL-2 V3.0)
//!
//! - **Zero runtime cost**: All constraint handling at compile-time only
//! - **Transparent to user**: Automatic bound injection, no explicit annotations needed
//! - **Minimal dependencies**: Uses only syn + quote (no external constraint libraries)
//! - **Conservative**: Only adds constraints to type parameters (not lifetime bounds)
//! - **Composable**: Works with multiple generics, bounds, and where clauses

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    punctuated::Punctuated, parse_quote, GenericParam, Generics, Token,
    TypeParamBound, WherePredicate,
};

/// Generic constraint capsule (T0 Auditable - compile-time)
///
/// Handles all constraint generation for generic types in capsule serialization.
/// No runtime overhead - all work done during proc-macro expansion.
#[derive(Debug, Clone)]
pub struct GenericConstraintCapsule;

impl GenericConstraintCapsule {
    /// Extract generic parameters from struct definition.
    ///
    /// Returns vector of generic parameter names (e.g., ["T", "U", "V"]).
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GENERICS_VALID`: Input generics are syntactically correct
    /// - `#VERIFY_GENERICS`: syn validates during parsing
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// struct Pair<T, U> { first: T, second: U }
    /// // Returns: vec!["T", "U"]
    /// ```
    pub fn extract_type_params(generics: &Generics) -> Vec<String> {
        generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(type_param) => Some(type_param.ident.to_string()),
                GenericParam::Lifetime(_) => None, // Skip lifetime params
                GenericParam::Const(_) => None,     // Skip const params
            })
            .collect()
    }

    /// Add CapsuleSerialize bound to all type parameters.
    ///
    /// Creates a new Generics with trait bounds added to each type parameter.
    /// Preserves existing bounds and where clauses.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_BOUNDS_ADDITIVE`: New bounds can be safely added to existing bounds
    /// - `#VERIFY_BOUNDS`: Compile error if trait path invalid
    /// - `#ASSUME_CLONE_SAFE`: Generics can be cloned and modified (immutable copy)
    /// - `#VERIFY_CLONE`: syn guarantees clone fidelity
    ///
    /// # Generated Code
    ///
    /// ```text
    /// Input:  <T>
    /// Output: <T: CapsuleSerialize>
    ///
    /// Input:  <T: Clone>
    /// Output: <T: Clone + CapsuleSerialize>
    ///
    /// Input:  <T, U>
    /// Output: <T: CapsuleSerialize, U: CapsuleSerialize>
    /// ```
    pub fn add_serialize_bounds(generics: &Generics) -> Generics {
        // #ASSUME_GENERICS_CLONEABLE: Generics is cloneable
        // #VERIFY_GENERICS: syn validates during clone
        let mut bounded_generics = generics.clone();

        // Iterate and bound all type parameters
        for param in &mut bounded_generics.params {
            if let GenericParam::Type(type_param) = param {
                // Create CapsuleSerialize bound: T: CapsuleSerialize
                let bound: TypeParamBound = parse_quote!(
                    ::atomic_capsule::serialize::CapsuleSerialize
                );

                // #ASSUME_BOUNDS_MERGE: Bounds can be pushed to existing vec
                // #VERIFY_BOUNDS: compile_fail tests validate merging
                type_param.bounds.push(bound);
            }
        }

        bounded_generics
    }

    /// Add CapsuleDeserialize bound to all type parameters.
    ///
    /// Identical to add_serialize_bounds but for deserialization constraint.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_BOUNDS_ADDITIVE`: Deserialization bounds independent of serialize
    /// - `#VERIFY_BOUNDS`: Compile error if trait path invalid
    ///
    /// # Generated Code
    ///
    /// ```text
    /// Input:  <T>
    /// Output: <T: CapsuleDeserialize>
    ///
    /// Input:  <T, U>
    /// Output: <T: CapsuleDeserialize, U: CapsuleDeserialize>
    /// ```
    pub fn add_deserialize_bounds(generics: &Generics) -> Generics {
        let mut bounded_generics = generics.clone();

        for param in &mut bounded_generics.params {
            if let GenericParam::Type(type_param) = param {
                // Create CapsuleDeserialize bound
                let bound: TypeParamBound = parse_quote!(
                    ::atomic_capsule::serialize::CapsuleDeserialize
                );

                type_param.bounds.push(bound);
            }
        }

        bounded_generics
    }

    /// Generate code for qualified generic parameter list.
    ///
    /// Produces `<T: CapsuleSerialize, U: CapsuleSerialize>` syntax suitable
    /// for impl blocks.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_QUOTE_OUTPUT`: quote! macro produces valid Rust syntax
    /// - `#VERIFY_QUOTE`: Compiler validates generated code
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let generics = parse_quote!(<T, U>);
    /// let bounded = GenericConstraintCapsule::add_serialize_bounds(&generics);
    /// let code = GenericConstraintCapsule::qualified_generic_params(&bounded);
    /// // Outputs: <T: CapsuleSerialize, U: CapsuleSerialize>
    /// ```
    pub fn qualified_generic_params(generics: &Generics) -> TokenStream {
        // Extract only type parameters (not lifetimes, const params)
        let type_params: Vec<_> = generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(tp) => Some({
                    let ident = &tp.ident;
                    let bounds = &tp.bounds;
                    quote! {
                        #ident: #bounds
                    }
                }),
                _ => None,
            })
            .collect();

        if type_params.is_empty() {
            quote!()
        } else {
            quote! {
                <#(#type_params),*>
            }
        }
    }

    /// Check if generics contains any type parameters.
    ///
    /// Returns true if struct is generic (has at least one <T>).
    /// Returns false if struct is non-generic.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GENERIC_DETECTION`: Generics::params is accurate
    /// - `#VERIFY_GENERIC`: Tests validate detection
    pub fn has_generics(generics: &Generics) -> bool {
        generics.params.iter().any(|p| matches!(p, GenericParam::Type(_)))
    }

    /// Create default CapsuleSerialize bound as TypeParamBound.
    ///
    /// Encapsulates the trait bound definition for reusability.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_BOUND_SYNTAX`: parse_quote! produces valid TypeParamBound
    /// - `#VERIFY_BOUND`: Compiler validates syntax
    pub fn serialize_bound() -> TypeParamBound {
        // #ASSUME_PARSE_QUOTE_SAFE: parse_quote! is statically guaranteed safe
        // #VERIFY_PARSE_QUOTE: Compilation fails if syntax invalid
        parse_quote!(::atomic_capsule::serialize::CapsuleSerialize)
    }

    /// Create default CapsuleDeserialize bound as TypeParamBound.
    pub fn deserialize_bound() -> TypeParamBound {
        parse_quote!(::atomic_capsule::serialize::CapsuleDeserialize)
    }

    /// Extract custom where clause predicates (if any).
    ///
    /// Returns vector of existing where clause predicates.
    /// Useful for merging with auto-generated bounds.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_WHERE_CLAUSE_PRESENT`: where clause may or may not exist
    /// - `#VERIFY_WHERE_CLAUSE`: Extraction is optional operation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Input: struct Wrapper<T> where T: Clone { value: T }
    /// // Returns: [T: Clone]
    /// ```
    pub fn extract_where_predicates(
        generics: &Generics,
    ) -> Option<Vec<WherePredicate>> {
        generics.where_clause.as_ref().map(|where_clause| {
            where_clause
                .predicates
                .iter()
                .cloned()
                .collect()
        })
    }

    /// Merge custom where clause predicates with auto-generated bounds.
    ///
    /// Creates a new where clause combining:
    /// 1. Original where clause predicates (if any)
    /// 2. Auto-generated bounds for all type parameters
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_WHERE_MERGE_SAFE`: Predicates can be combined without conflict
    /// - `#VERIFY_WHERE_MERGE`: Compiler validates merged where clause
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Input generics: <T> where T: Clone
    /// // Returns: T: Clone, T: CapsuleSerialize
    /// ```
    pub fn merge_where_clauses(
        generics: &Generics,
        custom_predicates: Vec<WherePredicate>,
    ) -> Option<syn::WhereClause> {
        if custom_predicates.is_empty() {
            return None;
        }

        let mut all_predicates = Punctuated::<WherePredicate, Token![,]>::new();

        // Add custom predicates first
        for pred in custom_predicates {
            all_predicates.push(pred);
        }

        // Add auto-generated type parameter bounds
        let serialize_bound = Self::serialize_bound();
        for param in &generics.params {
            if let GenericParam::Type(type_param) = param {
                let ident = &type_param.ident;

                // Create: T: CapsuleSerialize
                let pred: WherePredicate = parse_quote! {
                    #ident: #serialize_bound
                };

                all_predicates.push(pred);
            }
        }

        Some(syn::WhereClause {
            where_token: Default::default(),
            predicates: all_predicates,
        })
    }

    /// Generate complete impl block signature with bounds.
    ///
    /// Produces: `impl<T: CapsuleSerialize, U: CapsuleSerialize> TraitName for StructName<T, U>`
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_IMPL_SYNTAX`: Generated syntax is valid Rust
    /// - `#VERIFY_IMPL`: Compiler validates complete impl block
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let struct_name = parse_quote!(MyStruct);
    /// let ty_generics = quote!(<T, U>);
    /// let generics = parse_quote!(<T, U>);
    /// let trait_name = parse_quote!(MyTrait);
    ///
    /// GenericConstraintCapsule::generate_impl_signature(
    ///     &struct_name,
    ///     &ty_generics,
    ///     &generics,
    ///     &trait_name,
    /// );
    /// // Outputs: impl<T: CapsuleSerialize, U: CapsuleSerialize> MyTrait for MyStruct<T, U>
    /// ```
    pub fn generate_impl_signature(
        struct_name: &syn::Ident,
        ty_generics: &TokenStream,
        impl_generics: &Generics,
        trait_name: &TokenStream,
    ) -> TokenStream {
        let bounded = Self::add_serialize_bounds(impl_generics);
        let (gen_impl, _, where_clause) = bounded.split_for_impl();

        quote! {
            impl #gen_impl #trait_name for #struct_name #ty_generics #where_clause
        }
    }

    /// Helper: Extract type parameter bounds as comma-separated list.
    ///
    /// Returns formatted string suitable for error messages.
    ///
    /// # Example Output
    /// "T: CapsuleSerialize, U: CapsuleSerialize"
    pub fn bounds_as_string(generics: &Generics) -> String {
        let bounds: Vec<String> = generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(type_param) => {
                    let ident = &type_param.ident;
                    let bound_strs: Vec<String> = type_param
                        .bounds
                        .iter()
                        .map(|b| quote!(#b).to_string())
                        .collect();
                    let bounds_str = bound_strs.join(" + ");
                    Some(format!("{}: {}", ident, bounds_str))
                }
                _ => None,
            })
            .collect();

        bounds.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_extract_type_params_single() {
        let generics: Generics = parse_quote!(<T>);
        let params = GenericConstraintCapsule::extract_type_params(&generics);
        assert_eq!(params, vec!["T"]);
    }

    #[test]
    fn test_extract_type_params_multiple() {
        let generics: Generics = parse_quote!(<T, U, V>);
        let params = GenericConstraintCapsule::extract_type_params(&generics);
        assert_eq!(params, vec!["T", "U", "V"]);
    }

    #[test]
    fn test_extract_type_params_empty() {
        let generics: Generics = parse_quote!();
        let params = GenericConstraintCapsule::extract_type_params(&generics);
        assert!(params.is_empty());
    }

    #[test]
    fn test_extract_type_params_with_lifetimes() {
        let generics: Generics = parse_quote!(<'a, T, 'b, U>);
        let params = GenericConstraintCapsule::extract_type_params(&generics);
        assert_eq!(params, vec!["T", "U"]);
    }

    #[test]
    fn test_add_serialize_bounds_single() {
        let generics: Generics = parse_quote!(<T>);
        let bounded = GenericConstraintCapsule::add_serialize_bounds(&generics);

        // Verify bound was added
        let param = bounded.params.first().unwrap();
        if let GenericParam::Type(type_param) = param {
            assert!(!type_param.bounds.is_empty());
        } else {
            panic!("Expected type parameter");
        }
    }

    #[test]
    fn test_add_serialize_bounds_multiple() {
        let generics: Generics = parse_quote!(<T, U>);
        let bounded = GenericConstraintCapsule::add_serialize_bounds(&generics);

        // Verify bounds added to both parameters
        assert_eq!(bounded.params.len(), 2);
        for param in bounded.params.iter() {
            if let GenericParam::Type(type_param) = param {
                assert!(!type_param.bounds.is_empty());
            }
        }
    }

    #[test]
    fn test_add_deserialize_bounds_single() {
        let generics: Generics = parse_quote!(<T>);
        let bounded = GenericConstraintCapsule::add_deserialize_bounds(&generics);

        // Verify bound was added
        let param = bounded.params.first().unwrap();
        if let GenericParam::Type(type_param) = param {
            assert!(!type_param.bounds.is_empty());
        } else {
            panic!("Expected type parameter");
        }
    }

    #[test]
    fn test_add_deserialize_bounds_preserves_existing() {
        let generics: Generics = parse_quote!(<T: Clone>);
        let bounded = GenericConstraintCapsule::add_deserialize_bounds(&generics);

        // Verify original bound is preserved
        let param = bounded.params.first().unwrap();
        if let GenericParam::Type(type_param) = param {
            assert!(type_param.bounds.len() >= 2); // Clone + CapsuleDeserialize
        }
    }

    #[test]
    fn test_has_generics_with_types() {
        let generics: Generics = parse_quote!(<T>);
        assert!(GenericConstraintCapsule::has_generics(&generics));
    }

    #[test]
    fn test_has_generics_multiple() {
        let generics: Generics = parse_quote!(<T, U, V>);
        assert!(GenericConstraintCapsule::has_generics(&generics));
    }

    #[test]
    fn test_has_generics_empty() {
        let generics: Generics = parse_quote!();
        assert!(!GenericConstraintCapsule::has_generics(&generics));
    }

    #[test]
    fn test_has_generics_only_lifetimes() {
        let generics: Generics = parse_quote!(<'a, 'b>);
        assert!(!GenericConstraintCapsule::has_generics(&generics));
    }

    #[test]
    fn test_serialize_bound_creates_valid_bound() {
        let bound = GenericConstraintCapsule::serialize_bound();
        let bound_str = quote!(#bound).to_string();
        assert!(bound_str.contains("CapsuleSerialize"));
    }

    #[test]
    fn test_deserialize_bound_creates_valid_bound() {
        let bound = GenericConstraintCapsule::deserialize_bound();
        let bound_str = quote!(#bound).to_string();
        assert!(bound_str.contains("CapsuleDeserialize"));
    }

    #[test]
    fn test_extract_where_predicates_empty() {
        let generics: Generics = parse_quote!(<T>);
        let predicates = GenericConstraintCapsule::extract_where_predicates(&generics);
        assert!(predicates.is_none());
    }

    #[test]
    fn test_extract_where_predicates_with_clause() {
        let generics: Generics = parse_quote!(<T> where T: Clone);
        let predicates = GenericConstraintCapsule::extract_where_predicates(&generics);
        assert!(predicates.is_some());
        assert_eq!(predicates.unwrap().len(), 1);
    }

    #[test]
    fn test_bounds_as_string_single() {
        let generics = GenericConstraintCapsule::add_serialize_bounds(&parse_quote!(<T>));
        let bounds_str = GenericConstraintCapsule::bounds_as_string(&generics);
        assert!(bounds_str.contains("T"));
        assert!(bounds_str.contains("CapsuleSerialize"));
    }

    #[test]
    fn test_bounds_as_string_multiple() {
        let generics = GenericConstraintCapsule::add_serialize_bounds(&parse_quote!(<T, U>));
        let bounds_str = GenericConstraintCapsule::bounds_as_string(&generics);
        assert!(bounds_str.contains("T"));
        assert!(bounds_str.contains("U"));
        assert!(bounds_str.contains("CapsuleSerialize"));
    }

    #[test]
    fn test_qualified_generic_params_empty() {
        let generics: Generics = parse_quote!();
        let code = GenericConstraintCapsule::qualified_generic_params(&generics);
        assert_eq!(code.to_string(), "");
    }

    #[test]
    fn test_qualified_generic_params_single() {
        let generics = GenericConstraintCapsule::add_serialize_bounds(&parse_quote!(<T>));
        let code = GenericConstraintCapsule::qualified_generic_params(&generics);
        let code_str = code.to_string();
        assert!(code_str.contains("T"));
        assert!(code_str.contains("CapsuleSerialize"));
    }

    #[test]
    fn test_qualified_generic_params_multiple() {
        let generics =
            GenericConstraintCapsule::add_serialize_bounds(&parse_quote!(<T, U>));
        let code = GenericConstraintCapsule::qualified_generic_params(&generics);
        let code_str = code.to_string();
        assert!(code_str.contains("T"));
        assert!(code_str.contains("U"));
    }

    #[test]
    fn test_merge_where_clauses_empty() {
        let generics: Generics = parse_quote!(<T>);
        let merged =
            GenericConstraintCapsule::merge_where_clauses(&generics, vec![]);
        assert!(merged.is_none());
    }

    #[test]
    fn test_merge_where_clauses_with_predicates() {
        let generics: Generics = parse_quote!(<T>);
        let custom_pred: WherePredicate = parse_quote!(T: Clone);
        let merged = GenericConstraintCapsule::merge_where_clauses(
            &generics,
            vec![custom_pred],
        );

        assert!(merged.is_some());
        let where_clause = merged.unwrap();
        assert!(where_clause.predicates.len() >= 2); // Original + generated
    }

    #[test]
    fn test_bounds_preserve_existing_constraints() {
        let generics: Generics = parse_quote!(<T: Clone + Default>);
        let bounded = GenericConstraintCapsule::add_serialize_bounds(&generics);

        let param = bounded.params.first().unwrap();
        if let GenericParam::Type(type_param) = param {
            // Should have: Clone + Default + CapsuleSerialize (3 bounds)
            assert!(type_param.bounds.len() >= 3);
        }
    }

    #[test]
    fn test_multiple_generics_with_mixed_constraints() {
        let generics: Generics = parse_quote!(<T: Clone, U: Default>);
        let bounded = GenericConstraintCapsule::add_serialize_bounds(&generics);

        assert_eq!(bounded.params.len(), 2);

        // First parameter: Clone + CapsuleSerialize
        if let GenericParam::Type(tp1) = bounded.params.first().unwrap() {
            assert!(tp1.bounds.len() >= 2);
        }

        // Second parameter: Default + CapsuleSerialize
        if let GenericParam::Type(tp2) = bounded.params.last().unwrap() {
            assert!(tp2.bounds.len() >= 2);
        }
    }
}
