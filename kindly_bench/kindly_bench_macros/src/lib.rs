//! Procedural macros for kindly_bench
//!
//! Provides the `#[bench_capsule]` attribute macro for simplified benchmarking.
//!
//! # Phase 1 MVP
//!
//! Phase 1 provides a minimal macro that expands to the manual API.
//! Full AST-based baseline generation will be implemented in Phase 2.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Attribute macro for simplified benchmarking
///
/// # Example
///
/// ```rust,ignore
/// use kindly_bench::bench_capsule;
///
/// #[bench_capsule(tier = "T1", baseline = "RwLock")]
/// fn bench_circuit_breaker() {
///     let breaker = CircuitBreaker::new(State::Closed);
///     breaker.transition(State::Open);
/// }
/// ```
///
/// # Phase 1 Limitation
///
/// In Phase 1, this macro simply marks the function for future expansion.
/// Users should use the manual `run_benchmark` API for now.
#[proc_macro_attribute]
pub fn bench_capsule(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;

    // Phase 1: Pass through with compile-time note
    let expanded = quote! {
        #[allow(dead_code)]
        #input

        // Phase 1: Macro expansion not yet implemented
        // Use kindly_bench::run_benchmark API directly
        compile_error!(concat!(
            "Phase 1: #[bench_capsule] macro not yet implemented. ",
            "Please use kindly_bench::run_benchmark API directly for function: ",
            stringify!(#fn_name)
        ));
    };

    TokenStream::from(expanded)
}
