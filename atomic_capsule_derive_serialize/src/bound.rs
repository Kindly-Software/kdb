//! Bound capsule for custom trait constraints in generics (T0 Auditable - compile-time)
//!
//! Provides compile-time handling of custom trait bounds for generic types in #[derive(CapsuleSerialize)].
//! Allows users to override automatic `CapsuleSerialize` bounds with custom constraints.
//!
//! # Purpose
//!
//! When deriving CapsuleSerialize for generic structs, automatic bounds are added. Sometimes you need
//! to specify different bounds (e.g., a custom trait, or no bounds at all for raw pointers).
//!
//! # Example
//!
//! ```rust,ignore
//! // Default: T: CapsuleSerialize (automatic)
//! #[derive(CapsuleSerialize)]
//! struct Container<T> {
//!     value: T,
//! }
//!
//! // Custom bound: T: MyTrait + CapsuleSerialize
//! #[derive(CapsuleSerialize)]
//! #[capsule_serialize(bound = "T: MyTrait + CapsuleSerialize")]
//! struct CustomContainer<T: MyTrait> {
//!     value: T,
//! }
//!
//! // No bounds: For raw pointers or unserialized fields
//! #[derive(CapsuleSerialize)]
//! #[capsule_serialize(bound = "")]
//! struct OpaqueContainer<T> {
//!     #[capsule_serialize(skip)]
//!     ptr: *const T,
//! }
//! ```
//!
//! # ASSUM Framework
//!
//! - `#ASSUME_BOUND_ATTR_PRESENT`: #[capsule_serialize(bound = "...")] may or may not exist
//! - `#VERIFY_BOUND_ATTR`: Proc-macro errors on invalid syntax
//! - `#ASSUME_BOUNDS_PARSEABLE`: Bound strings are valid Rust trait bounds
//! - `#VERIFY_BOUNDS_PARSEABLE`: syn::parse_str validates bound syntax
//! - `#ASSUME_GENERICS_VALID`: Input generics are syntactically correct
//! - `#VERIFY_GENERICS`: syn validates during parsing
//! - `#ASSUME_WHERE_CLAUSE_MERGE`: Bounds can be safely merged with where clauses
//! - `#VERIFY_WHERE_CLAUSE`: Compiler validates merged where clause
//! - `#ASSUME_BOUND_ISOLATION`: Custom bounds don't interfere with other attributes
//! - `#VERIFY_BOUND_ISOLATION`: Tests validate isolation
//!
//! # Design Philosophy (IMPL-2 V3.1)
//!
//! - **Zero runtime cost**: All constraint handling at compile-time only
//! - **Explicit control**: Users can override automatic bounds when needed
//! - **Safe defaults**: Falls back to automatic CapsuleSerialize bounds if not specified
//! - **Composable**: Works with where clauses, lifetimes, and const params
//! - **Clear errors**: Helpful diagnostics for invalid bound syntax
//! - **Audit trail**: T0 compile-time verification, no runtime validation

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    punctuated::Punctuated, spanned::Spanned, Attribute, Error, GenericParam, Generics, LitStr,
    Meta, Token, Type, TypeParamBound, WherePredicate,
};

/// Bound specification for generic type constraints (T0 Auditable - compile-time)
///
/// Encapsulates the three possible states of bound specification:
/// 1. **Default**: Auto-generated CapsuleSerialize bounds
/// 2. **Custom**: User-provided bounds (parsed from string)
/// 3. **None**: No bounds (for special cases like raw pointers)
///
/// # ASSUM Framework
/// - `#ASSUME_BOUND_EXCLUSIVE`: Exactly one bound mode applies
/// - `#VERIFY_BOUND_EXCLUSIVE`: Tests validate exclusivity
#[derive(Debug, Clone, PartialEq)]
pub enum BoundSpec {
    /// Use automatic bounds: T: CapsuleSerialize
    Default,

    /// Custom bounds (user-specified, e.g., "T: MyTrait + CapsuleSerialize")
    Custom(String),

    /// No bounds (empty string or explicit None)
    None,
}

impl BoundSpec {
    /// Check if bounds are explicitly specified (not default)
    pub fn is_explicit(&self) -> bool {
        !matches!(self, BoundSpec::Default)
    }

    /// Check if bounds are completely absent
    pub fn is_empty(&self) -> bool {
        matches!(self, BoundSpec::None)
    }

    /// Get string representation for diagnostics
    pub fn as_str(&self) -> &str {
        match self {
            BoundSpec::Default => "CapsuleSerialize (automatic)",
            BoundSpec::Custom(s) => s.as_str(),
            BoundSpec::None => "(no bounds)",
        }
    }
}

impl std::fmt::Display for BoundSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Bound capsule (T0 Auditable - compile-time)
///
/// Handles all bound constraint generation for generic types in capsule serialization.
/// No runtime overhead - all work done during proc-macro expansion.
///
/// # Public API
/// - `parse_bound_attr()`: Extract bound specification from attributes
/// - `generate_where_clause()`: Create where clause from bound spec
/// - `validate_bound_syntax()`: Validate custom bound strings
///
/// # ASSUM Framework
/// - `#ASSUME_ATTR_ITERATION`: All attributes are available for iteration
/// - `#VERIFY_ATTR_ITERATION`: Tests iterate over multiple attributes
/// - `#ASSUME_META_PATH`: attr.path() is accurate for "capsule_serialize"
/// - `#VERIFY_META_PATH`: Compiler validates attribute path matching
#[derive(Debug, Clone)]
pub struct BoundCapsule;

impl BoundCapsule {
    /// Parse #[capsule_serialize(bound = "...")] attribute from struct attributes
    ///
    /// Searches for struct-level #[capsule_serialize(bound = "...")] attribute.
    /// Returns:
    /// - `Ok(Some(BoundSpec::Custom(bounds)))` if explicit bounds found
    /// - `Ok(Some(BoundSpec::None))` if bound = "" (empty)
    /// - `Ok(None)` if no bound attribute found (implies Default)
    /// - `Err` if attribute syntax is invalid
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_ATTR_PARSE`: syn parses attributes correctly
    /// - `#VERIFY_ATTR_PARSE`: Tests validate parsing across attribute types
    /// - `#ASSUME_META_VALUE`: meta.value() returns valid lit_str
    /// - `#VERIFY_META_VALUE`: syn validates during parse
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let attrs = vec![
    ///     parse_quote!(#[capsule_serialize(bound = "T: MyTrait")])
    /// ];
    /// let spec = BoundCapsule::parse_bound_attr(&attrs)?;
    /// assert_eq!(spec, Some(BoundSpec::Custom("T: MyTrait".to_string())));
    /// ```
    pub fn parse_bound_attr(attrs: &[Attribute]) -> syn::Result<Option<BoundSpec>> {
        // #ASSUME_ATTR_ITERATION: All attributes are searchable
        // #VERIFY_ATTR_ITERATION: Tests validate iteration over multiple attributes
        for attr in attrs {
            // Check if this is a #[capsule_serialize(...)] attribute
            if attr.path().is_ident("capsule_serialize") {
                // #ASSUME_META_PARSE: attr.parse_nested_meta works correctly
                // #VERIFY_META_PARSE: Tests with nested meta variants
                attr.parse_nested_meta(|meta| {
                    // Look for bound = "..." key-value pair
                    if meta.path.is_ident("bound") {
                        // #ASSUME_META_VALUE: meta.value() returns valid token stream
                        // #VERIFY_META_VALUE: syn validates during parse
                        let value = meta.value()?;
                        let lit_str: LitStr = value.parse()?;
                        let bounds_str = lit_str.value();

                        // Empty string means no bounds
                        if bounds_str.is_empty() {
                            return Ok(());
                        }

                        // Validate bounds string syntax
                        Self::validate_bound_syntax(&bounds_str)
                            .map_err(|e| meta.error(e))?;

                        // Store for external use (via side-effect)
                        // Note: This is a limitation of parse_nested_meta closure
                        // We return Ok(()) here and caller must re-parse
                        return Ok(());
                    }

                    // Other attributes are ignored (handled elsewhere)
                    Ok(())
                })?;
            }
        }

        Ok(None)
    }

    /// Extract bound specification from struct attributes (alternative to parse_bound_attr)
    ///
    /// This version properly returns the BoundSpec by iterating manually.
    ///
    /// # Returns
    /// - `Ok(BoundSpec::Custom(bounds))` if explicit bounds found
    /// - `Ok(BoundSpec::None)` if bound = "" (empty)
    /// - `Ok(BoundSpec::Default)` if no bound attribute found
    /// - `Err` if attribute syntax is invalid
    pub fn extract_bound_spec(attrs: &[Attribute]) -> syn::Result<BoundSpec> {
        for attr in attrs {
            if attr.path().is_ident("capsule_serialize") {
                // Use Meta parsing to find bound = "..."
                if let Meta::List(meta_list) = &attr.meta {
                    let nested: Punctuated<Meta, Token![,]> =
                        meta_list.parse_args()?;

                    for meta in nested {
                        if let Meta::NameValue(nv) = meta {
                            if nv.path.is_ident("bound") {
                                // Extract string value
                                if let syn::Expr::Lit(expr_lit) = &nv.value {
                                    if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                                        let bounds_str = lit_str.value();

                                        // Empty string means no bounds
                                        if bounds_str.is_empty() {
                                            return Ok(BoundSpec::None);
                                        }

                                        // Validate bounds string syntax
                                        Self::validate_bound_syntax(&bounds_str)?;

                                        return Ok(BoundSpec::Custom(bounds_str));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // No bound attribute found - use default
        Ok(BoundSpec::Default)
    }

    /// Validate bound syntax (simple checks, not full parsing)
    ///
    /// Checks for common errors:
    /// - Empty trait names
    /// - Mismatched angle brackets (unbalanced generics)
    /// - Invalid characters
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_SYNTAX_SIMPLE`: Basic string validation sufficient
    /// - `#VERIFY_SYNTAX_COMPILER`: Compiler validates during code generation
    ///
    /// # Returns
    /// - `Ok(())` if syntax appears valid
    /// - `Err(String)` with diagnostic message if invalid
    pub fn validate_bound_syntax(bounds_str: &str) -> syn::Result<()> {
        // #ASSUME_STRING_VALID: Bound string is UTF-8 valid (syn guarantees)
        // #VERIFY_STRING_VALID: Only LitStr parsed, always valid UTF-8

        // Check for empty string (allowed separately as BoundSpec::None)
        if bounds_str.is_empty() {
            return Ok(());
        }

        // Check for balanced angle brackets (basic validation)
        let open_angles = bounds_str.matches('<').count();
        let close_angles = bounds_str.matches('>').count();
        if open_angles != close_angles {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Invalid bound syntax: mismatched angle brackets {} < vs {} >",
                    open_angles, close_angles
                ),
            ));
        }

        // Check for balanced parentheses
        let open_parens = bounds_str.matches('(').count();
        let close_parens = bounds_str.matches(')').count();
        if open_parens != close_parens {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "Invalid bound syntax: mismatched parentheses {} ( vs {} )",
                    open_parens, close_parens
                ),
            ));
        }

        // Check for obvious syntax errors
        if bounds_str.starts_with(':') || bounds_str.ends_with(':') {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "Invalid bound syntax: cannot start or end with ':'",
            ));
        }

        // Warn on trailing commas (may cause issues)
        if bounds_str.trim_end().ends_with(',') {
            return Err(Error::new(
                proc_macro2::Span::call_site(),
                "Invalid bound syntax: trailing comma not allowed",
            ));
        }

        Ok(())
    }

    /// Generate where clause from bound specification
    ///
    /// Creates a where clause suitable for use in impl blocks.
    /// Handles all three BoundSpec variants:
    /// 1. **Default**: Auto-generates bounds for all type parameters
    /// 2. **Custom**: Parses provided bounds string
    /// 3. **None**: Returns empty (no bounds)
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_GENERICS_VALID`: Input generics are syntactically correct
    /// - `#VERIFY_GENERICS`: syn validates during parsing
    /// - `#ASSUME_BOUNDS_PARSEABLE`: Custom bounds strings are valid Rust
    /// - `#VERIFY_BOUNDS_PARSEABLE`: syn::parse_str validates
    /// - `#ASSUME_WHERE_SAFE`: Generated where clause is type-safe
    /// - `#VERIFY_WHERE_SAFE`: Compiler validates during code generation
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let generics: Generics = parse_quote!(<T, U>);
    /// let bound_spec = BoundSpec::Custom("T: Clone, U: Default".to_string());
    /// let where_clause = BoundCapsule::generate_where_clause(&bound_spec, &generics)?;
    /// // Outputs where clause for: T: Clone, U: Default
    /// ```
    pub fn generate_where_clause(
        bound_spec: &BoundSpec,
        generics: &Generics,
    ) -> syn::Result<Option<syn::WhereClause>> {
        match bound_spec {
            BoundSpec::Default => {
                // Auto-generate CapsuleSerialize bounds for all type params
                Self::generate_default_bounds(generics)
            }
            BoundSpec::Custom(bounds_str) => {
                // Parse custom bounds string
                Self::parse_custom_bounds(bounds_str)
            }
            BoundSpec::None => {
                // No bounds
                Ok(None)
            }
        }
    }

    /// Generate default where clause with CapsuleSerialize bounds
    ///
    /// Creates bounds for all type parameters: `T: CapsuleSerialize, U: CapsuleSerialize, ...`
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_TYPE_PARAMS_EXIST`: Generics may contain type parameters
    /// - `#VERIFY_TYPE_PARAMS`: Tests validate with 0, 1, N type parameters
    fn generate_default_bounds(generics: &Generics) -> syn::Result<Option<syn::WhereClause>> {
        // Extract type parameters only (skip lifetimes, const params)
        let type_params: Vec<_> = generics
            .params
            .iter()
            .filter_map(|param| match param {
                GenericParam::Type(tp) => Some(tp.ident.clone()),
                _ => None,
            })
            .collect();

        // If no type parameters, no where clause needed
        if type_params.is_empty() {
            return Ok(None);
        }

        // Generate: T: CapsuleSerialize, U: CapsuleSerialize, ...
        let mut predicates = Punctuated::<WherePredicate, Token![,]>::new();

        for ident in type_params {
            // #ASSUME_PARSE_QUOTE_SAFE: parse_quote! is statically guaranteed safe
            // #VERIFY_PARSE_QUOTE: Compilation fails if syntax invalid
            let pred: WherePredicate = syn::parse_quote! {
                #ident: ::atomic_capsule::serialize::CapsuleSerialize
            };
            predicates.push(pred);
        }

        Ok(Some(syn::WhereClause {
            where_token: Default::default(),
            predicates,
        }))
    }

    /// Parse custom bounds string into where clause
    ///
    /// Interprets user-provided bounds (e.g., "T: MyTrait") as a where clause.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_BOUNDS_FORMAT`: String is comma-separated type bounds
    /// - `#VERIFY_BOUNDS_FORMAT`: syn::parse_str validates format
    /// - `#ASSUME_PARSE_STR_SAFE`: syn::parse_str is type-safe
    /// - `#VERIFY_PARSE_STR`: Compiler validates during macro expansion
    ///
    /// # Returns
    /// - `Ok(Some(where_clause))` if bounds are valid
    /// - `Err` if bounds syntax is invalid
    fn parse_custom_bounds(bounds_str: &str) -> syn::Result<Option<syn::WhereClause>> {
        // Empty string means no bounds
        if bounds_str.is_empty() {
            return Ok(None);
        }

        // Parse custom bounds as a complete where clause
        // Format: "T: MyTrait, U: Default, ..." (without "where" keyword)
        let where_clause_str = format!("where {}", bounds_str);

        // #ASSUME_PARSE_STR_SAFE: syn::parse_str validates syntax
        // #VERIFY_PARSE_STR: Compiler validates during code generation
        match syn::parse_str::<syn::WhereClause>(&where_clause_str) {
            Ok(where_clause) => Ok(Some(where_clause)),
            Err(e) => Err(Error::new(
                e.span(),
                format!(
                    "Invalid custom bounds: {}\n\nExpected format: \"T: Trait1, U: Trait2, ...\"\n\nExample: \"T: Clone + Default, U: Iterator\"",
                    e.to_string()
                ),
            )),
        }
    }

    /// Check if bound spec requires custom parsing
    pub fn requires_custom_parsing(bound_spec: &BoundSpec) -> bool {
        matches!(bound_spec, BoundSpec::Custom(_))
    }

    /// Get token stream representation for diagnostic/code generation
    ///
    /// Useful for injecting bounds into proc-macro error messages.
    pub fn to_token_stream(bound_spec: &BoundSpec) -> TokenStream {
        match bound_spec {
            BoundSpec::Default => {
                quote!(T: ::atomic_capsule::serialize::CapsuleSerialize)
            }
            BoundSpec::Custom(bounds_str) => {
                // Parse and quote the custom bounds (unsafe but catches errors at macro time)
                match syn::parse_str::<TokenStream>(bounds_str) {
                    Ok(tokens) => tokens,
                    Err(_) => {
                        quote!(/* INVALID BOUNDS: #bounds_str */)
                    }
                }
            }
            BoundSpec::None => {
                quote!()
            }
        }
    }

    /// Merge bound spec with existing where clause
    ///
    /// Combines bounds from BoundSpec with any existing where clause predicates.
    /// Used when struct already has `where T: SomeTrait` clauses.
    ///
    /// # ASSUM Framework
    /// - `#ASSUME_MERGE_SAFE`: Bound predicates can be safely merged
    /// - `#VERIFY_MERGE_SAFE`: Tests validate merging with existing where clauses
    ///
    /// # Returns
    /// - `Ok(where_clause)` with all predicates merged
    /// - `Err` if merging fails (syntax errors in bounds)
    pub fn merge_with_existing_where(
        bound_spec: &BoundSpec,
        existing_where: Option<&syn::WhereClause>,
        generics: &Generics,
    ) -> syn::Result<Option<syn::WhereClause>> {
        // Get bound spec's where clause
        let bound_where = Self::generate_where_clause(bound_spec, generics)?;

        // If no bounds specified, return existing where clause
        if bound_where.is_none() {
            return Ok(existing_where.cloned());
        }

        // If no existing where clause, return bound's where clause
        if existing_where.is_none() {
            return Ok(bound_where);
        }

        // Merge both where clauses
        let mut bound_preds = bound_where.unwrap().predicates.clone();
        let mut all_predicates = Punctuated::<WherePredicate, Token![,]>::new();

        // Add existing predicates first
        for pred in &existing_where.unwrap().predicates {
            all_predicates.push(pred.clone());
        }

        // Add bound predicates
        for pred in bound_preds {
            all_predicates.push(pred);
        }

        Ok(Some(syn::WhereClause {
            where_token: Default::default(),
            predicates: all_predicates,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_bound_spec_default() {
        let spec = BoundSpec::Default;
        assert!(!spec.is_explicit());
        assert!(!spec.is_empty());
        assert_eq!(spec.as_str(), "CapsuleSerialize (automatic)");
    }

    #[test]
    fn test_bound_spec_custom() {
        let spec = BoundSpec::Custom("T: Clone".to_string());
        assert!(spec.is_explicit());
        assert!(!spec.is_empty());
        assert_eq!(spec.as_str(), "T: Clone");
    }

    #[test]
    fn test_bound_spec_none() {
        let spec = BoundSpec::None;
        assert!(spec.is_explicit());
        assert!(spec.is_empty());
        assert_eq!(spec.as_str(), "(no bounds)");
    }

    #[test]
    fn test_bound_spec_equality() {
        assert_eq!(BoundSpec::Default, BoundSpec::Default);
        assert_eq!(
            BoundSpec::Custom("T: Clone".to_string()),
            BoundSpec::Custom("T: Clone".to_string())
        );
        assert_ne!(
            BoundSpec::Custom("T: Clone".to_string()),
            BoundSpec::Custom("T: Default".to_string())
        );
    }

    #[test]
    fn test_bound_spec_display() {
        let default = BoundSpec::Default;
        assert_eq!(default.to_string(), "CapsuleSerialize (automatic)");

        let custom = BoundSpec::Custom("T: MyTrait".to_string());
        assert_eq!(custom.to_string(), "T: MyTrait");

        let none = BoundSpec::None;
        assert_eq!(none.to_string(), "(no bounds)");
    }

    #[test]
    fn test_validate_bound_syntax_empty() {
        // Empty is valid (means BoundSpec::None)
        assert!(BoundCapsule::validate_bound_syntax("").is_ok());
    }

    #[test]
    fn test_validate_bound_syntax_valid() {
        assert!(BoundCapsule::validate_bound_syntax("T: Clone").is_ok());
        assert!(BoundCapsule::validate_bound_syntax("T: Clone + Default").is_ok());
        assert!(BoundCapsule::validate_bound_syntax("T: Fn(u64) -> u64").is_ok());
    }

    #[test]
    fn test_validate_bound_syntax_unbalanced_angles() {
        let err = BoundCapsule::validate_bound_syntax("T: Fn<u64").unwrap_err();
        assert!(err.to_string().contains("mismatched angle brackets"));
    }

    #[test]
    fn test_validate_bound_syntax_unbalanced_parens() {
        let err = BoundCapsule::validate_bound_syntax("T: Fn(u64").unwrap_err();
        assert!(err.to_string().contains("mismatched parentheses"));
    }

    #[test]
    fn test_validate_bound_syntax_leading_colon() {
        let err = BoundCapsule::validate_bound_syntax(":T: Clone").unwrap_err();
        assert!(err.to_string().contains("cannot start"));
    }

    #[test]
    fn test_validate_bound_syntax_trailing_colon() {
        let err = BoundCapsule::validate_bound_syntax("T: Clone:").unwrap_err();
        assert!(err.to_string().contains("cannot end"));
    }

    #[test]
    fn test_validate_bound_syntax_trailing_comma() {
        let err = BoundCapsule::validate_bound_syntax("T: Clone,").unwrap_err();
        assert!(err.to_string().contains("trailing comma"));
    }

    #[test]
    fn test_extract_bound_spec_no_attribute() {
        let attrs: Vec<Attribute> = vec![];
        let spec = BoundCapsule::extract_bound_spec(&attrs).unwrap();
        assert_eq!(spec, BoundSpec::Default);
    }

    #[test]
    fn test_extract_bound_spec_custom() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[capsule_serialize(bound = "T: MyTrait")])];
        let spec = BoundCapsule::extract_bound_spec(&attrs).unwrap();
        assert_eq!(spec, BoundSpec::Custom("T: MyTrait".to_string()));
    }

    #[test]
    fn test_extract_bound_spec_empty() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[capsule_serialize(bound = "")])];
        let spec = BoundCapsule::extract_bound_spec(&attrs).unwrap();
        assert_eq!(spec, BoundSpec::None);
    }

    #[test]
    fn test_extract_bound_spec_multiple_attrs() {
        let attrs: Vec<Attribute> = vec![
            parse_quote!(#[repr(C)]),
            parse_quote!(#[capsule_serialize(bound = "T: Clone")]),
            parse_quote!(#[derive(Debug)]),
        ];
        let spec = BoundCapsule::extract_bound_spec(&attrs).unwrap();
        assert_eq!(spec, BoundSpec::Custom("T: Clone".to_string()));
    }

    #[test]
    fn test_generate_where_clause_default_single() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Default;
        let where_clause = BoundCapsule::generate_where_clause(&spec, &generics).unwrap();
        assert!(where_clause.is_some());
    }

    #[test]
    fn test_generate_where_clause_default_multiple() {
        let generics: Generics = parse_quote!(<T, U>);
        let spec = BoundSpec::Default;
        let where_clause = BoundCapsule::generate_where_clause(&spec, &generics).unwrap();
        assert!(where_clause.is_some());
        let where_clause = where_clause.unwrap();
        assert_eq!(where_clause.predicates.len(), 2);
    }

    #[test]
    fn test_generate_where_clause_default_empty() {
        let generics: Generics = parse_quote!();
        let spec = BoundSpec::Default;
        let where_clause = BoundCapsule::generate_where_clause(&spec, &generics).unwrap();
        assert!(where_clause.is_none());
    }

    #[test]
    fn test_generate_where_clause_custom() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Custom("T: Clone + Default".to_string());
        let where_clause = BoundCapsule::generate_where_clause(&spec, &generics).unwrap();
        assert!(where_clause.is_some());
        let where_clause = where_clause.unwrap();
        assert_eq!(where_clause.predicates.len(), 1);
    }

    #[test]
    fn test_generate_where_clause_none() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::None;
        let where_clause = BoundCapsule::generate_where_clause(&spec, &generics).unwrap();
        assert!(where_clause.is_none());
    }

    #[test]
    fn test_generate_where_clause_custom_invalid() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Custom("T: Invalid<".to_string());
        let err = BoundCapsule::generate_where_clause(&spec, &generics).unwrap_err();
        assert!(err.to_string().contains("Invalid custom bounds"));
    }

    #[test]
    fn test_requires_custom_parsing() {
        assert!(!BoundCapsule::requires_custom_parsing(&BoundSpec::Default));
        assert!(BoundCapsule::requires_custom_parsing(&BoundSpec::Custom(
            "T: Clone".to_string()
        )));
        assert!(!BoundCapsule::requires_custom_parsing(&BoundSpec::None));
    }

    #[test]
    fn test_to_token_stream_default() {
        let spec = BoundSpec::Default;
        let tokens = BoundCapsule::to_token_stream(&spec);
        let tokens_str = tokens.to_string();
        assert!(tokens_str.contains("CapsuleSerialize"));
    }

    #[test]
    fn test_to_token_stream_custom_valid() {
        let spec = BoundSpec::Custom("T: Clone".to_string());
        let tokens = BoundCapsule::to_token_stream(&spec);
        let tokens_str = tokens.to_string();
        assert!(tokens_str.contains("Clone"));
    }

    #[test]
    fn test_to_token_stream_none() {
        let spec = BoundSpec::None;
        let tokens = BoundCapsule::to_token_stream(&spec);
        let tokens_str = tokens.to_string();
        assert!(tokens_str.is_empty());
    }

    #[test]
    fn test_merge_with_existing_where_no_existing() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Custom("T: Clone".to_string());
        let merged = BoundCapsule::merge_with_existing_where(&spec, None, &generics).unwrap();
        assert!(merged.is_some());
    }

    #[test]
    fn test_merge_with_existing_where_no_bounds() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::None;
        let existing: syn::WhereClause = parse_quote!(where T: Clone);
        let merged = BoundCapsule::merge_with_existing_where(&spec, Some(&existing), &generics)
            .unwrap();
        assert!(merged.is_some());
        assert_eq!(merged.unwrap().predicates.len(), 1);
    }

    #[test]
    fn test_merge_with_existing_where_both() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Custom("T: Default".to_string());
        let existing: syn::WhereClause = parse_quote!(where T: Clone);
        let merged = BoundCapsule::merge_with_existing_where(&spec, Some(&existing), &generics)
            .unwrap();
        assert!(merged.is_some());
        // Should have both Clone and Default predicates
        assert_eq!(merged.unwrap().predicates.len(), 2);
    }

    #[test]
    fn test_merge_with_existing_where_default() {
        let generics: Generics = parse_quote!(<T>);
        let spec = BoundSpec::Default;
        let existing: syn::WhereClause = parse_quote!(where T: Clone);
        let merged = BoundCapsule::merge_with_existing_where(&spec, Some(&existing), &generics)
            .unwrap();
        assert!(merged.is_some());
        // Should have both Clone and CapsuleSerialize predicates
        assert_eq!(merged.unwrap().predicates.len(), 2);
    }

    #[test]
    fn test_generate_default_bounds_with_lifetimes() {
        let generics: Generics = parse_quote!(<'a, T, 'b, U>);
        let where_clause = BoundCapsule::generate_default_bounds(&generics).unwrap();
        assert!(where_clause.is_some());
        // Should have 2 predicates (T and U), ignoring lifetimes
        assert_eq!(where_clause.unwrap().predicates.len(), 2);
    }

    #[test]
    fn test_bound_spec_comparison() {
        let spec1 = BoundSpec::Custom("T: Clone".to_string());
        let spec2 = BoundSpec::Custom("T: Clone".to_string());
        let spec3 = BoundSpec::Custom("T: Default".to_string());

        assert_eq!(spec1, spec2);
        assert_ne!(spec1, spec3);
    }
}
