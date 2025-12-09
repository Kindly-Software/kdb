//! # Field Type Diagnostics
//!
//! Analyzes struct fields and generates compile-time warnings for potential issues.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Fields, Type};

/// Analyze struct fields and generate COMPILE ERRORS for violations
///
/// # UCE33 Q11 (Rust Transform)
/// Capsules MUST use atomic types for coordination, never mutexes.
///
/// # ASSUM Framework
/// - `#ASSUME_ATOMIC_FIELDS`: Capsules use atomic primitives (not Mutex/RwLock)
/// - `#VERIFY_ATOMIC_FIELDS`: Generate COMPILE ERRORS for non-atomic types
///
/// # Errors Generated
///
/// - Mutex<T> fields → COMPILE ERROR (must use AtomicU64 instead)
/// - RwLock<T> fields → COMPILE ERROR (must use atomic operations)
/// - Cell<T>/RefCell<T> fields → COMPILE ERROR (must use atomic equivalents)
///
/// # Chaos Mandate
/// 100% lockfree architecture - NO mutex/RwLock/Cell allowed.
pub fn generate_field_diagnostics(input: &DeriveInput) -> TokenStream {
    let fields = match &input.data {
        syn::Data::Struct(data_struct) => match &data_struct.fields {
            Fields::Named(fields_named) => &fields_named.named,
            _ => return quote! {}, // No diagnostics for unnamed fields
        },
        _ => return quote! {}, // No diagnostics for non-structs
    };

    let mut errors = Vec::new();

    for field in fields.iter() {
        if let Some(field_name) = &field.ident {
            // Skip padding fields
            let name_str = field_name.to_string();
            if name_str.starts_with("_padding") || name_str.starts_with("_pad") {
                continue;
            }

            // Analyze field type - will generate COMPILE ERROR if forbidden type detected
            if let Some(error) = analyze_field_type(&field.ty, field_name) {
                errors.push(error);
            }
        }
    }

    // Generate all compile errors
    if errors.is_empty() {
        quote! {}
    } else {
        quote! {
            // Field diagnostics (compile-time errors for Chaos violations)
            const _: () = {
                #(#errors)*
            };
        }
    }
}

/// Analyze a single field type and generate COMPILE ERROR if needed
///
/// # ASSUM Framework
/// - `#ASSUME_NO_MUTEX_FIELDS`: Capsules NEVER use Mutex/RwLock/Cell (100% lockfree mandate)
/// - `#VERIFY_NO_MUTEX_FIELDS`: Compile-time detection and hard error (not warning)
///
/// Returns `Some(error)` if field type violates capsule architecture
fn analyze_field_type(ty: &Type, field_name: &syn::Ident) -> Option<TokenStream> {
    let type_string = quote!(#ty).to_string();

    // Detect Mutex<T> - COMPILE ERROR (not warning)
    if type_string.contains("Mutex") {
        return Some(generate_mutex_error(field_name));
    }

    // Detect RwLock<T> - COMPILE ERROR (not warning)
    if type_string.contains("RwLock") {
        return Some(generate_rwlock_error(field_name));
    }

    // Detect Cell<T> or RefCell<T> - COMPILE ERROR (not warning)
    // Allow UnsafeCell (interior mutability primitive for SIMD with atomic coordination)
    // Forbid Cell/RefCell (not Send/Sync, Chaos violation)
    if !type_string.contains("Atomic") {
        if type_string.contains("RefCell") {
            return Some(generate_cell_error(field_name));
        }
        // Check for Cell but not UnsafeCell (note: quote! adds spaces, so "Cell < T >")
        if type_string.contains("Cell") && !type_string.contains("UnsafeCell") {
            return Some(generate_cell_error(field_name));
        }
    }

    None
}

/// Generate COMPILE ERROR for Mutex<T> field
///
/// # ASSUM Framework
/// - `#ASSUME_NO_MUTEX`: Capsules are 100% lockfree (Chaos mandate)
/// - `#VERIFY_NO_MUTEX`: Compile-time enforcement via compile_error!
fn generate_mutex_error(field_name: &syn::Ident) -> TokenStream {
    let msg = format!(
        "Field `{}` uses Mutex which is FORBIDDEN in capsule architecture.\n\
         \n\
         Chaos MANDATE: Capsules require 100% lockfree atomic operations.\n\
         (UCE33 Q10: Tier 1 Atomic, Chaos: No mutex/RwLock/scattered atomics)\n\
         \n\
         Replace Mutex with:\n\
         - AtomicU64 for packed state (3-10× faster)\n\
         - DualAtomicU64 for dual-channel coordination\n\
         - Atomic types with appropriate memory ordering\n\
         \n\
         Example:\n\
         // Before: state: Mutex<u64> (FORBIDDEN - blocking)\n\
         // After:  state: AtomicU64 (REQUIRED - lockfree)\n\
         \n\
         See: /home/samuel/Docs/The Atomic Capsule.md\n\
         See: /home/samuel/Primitives/CLAUDE.md (Chaos mandate)",
        field_name
    );

    quote! {
        compile_error!(#msg);
    }
}

/// Generate COMPILE ERROR for RwLock<T> field
///
/// # ASSUM Framework
/// - `#ASSUME_NO_RWLOCK`: Capsules are 100% lockfree (Chaos mandate)
/// - `#VERIFY_NO_RWLOCK`: Compile-time enforcement via compile_error!
fn generate_rwlock_error(field_name: &syn::Ident) -> TokenStream {
    let msg = format!(
        "Field `{}` uses RwLock which is FORBIDDEN in capsule architecture.\n\
         \n\
         Chaos MANDATE: Capsules require 100% lockfree atomic operations.\n\
         (UCE33 Q10: Tier 1 Atomic, Chaos: No mutex/RwLock/scattered atomics)\n\
         \n\
         Replace RwLock with:\n\
         - AtomicU64 for state coordination\n\
         - Atomic loads with Acquire ordering for reads\n\
         - Atomic stores with Release ordering for writes\n\
         \n\
         Example:\n\
         // Before: state: RwLock<State> (FORBIDDEN - writer-blocking)\n\
         // After:  state: AtomicU64 (REQUIRED - always lock-free)\n\
         \n\
         See: /home/samuel/Docs/The Atomic Capsule.md\n\
         See: /home/samuel/Primitives/CLAUDE.md (Chaos mandate)",
        field_name
    );

    quote! {
        compile_error!(#msg);
    }
}

/// Generate COMPILE ERROR for Cell<T>/RefCell<T> field
///
/// # ASSUM Framework
/// - `#ASSUME_NO_CELL`: Capsules are Send + Sync (thread-safe)
/// - `#VERIFY_NO_CELL`: Compile-time enforcement via compile_error!
fn generate_cell_error(field_name: &syn::Ident) -> TokenStream {
    let msg = format!(
        "Field `{}` uses Cell/RefCell which is FORBIDDEN in capsule architecture.\n\
         \n\
         Chaos MANDATE: Capsules are Send + Sync and require atomic operations.\n\
         (Cell/RefCell are NOT Send/Sync)\n\
         \n\
         Replace Cell/RefCell with:\n\
         - AtomicU64/AtomicI64/AtomicBool for primitive types\n\
         - Atomic operations with appropriate memory ordering\n\
         \n\
         Example:\n\
         // Before: state: Cell<u64> (FORBIDDEN - not Send/Sync)\n\
         // After:  state: AtomicU64 (REQUIRED - Send + Sync, lockfree)\n\
         \n\
         See: /home/samuel/Docs/The Atomic Capsule.md\n\
         See: /home/samuel/Primitives/CLAUDE.md (Chaos mandate)",
        field_name
    );

    quote! {
        compile_error!(#msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_no_errors_for_atomic_fields() {
        let input: DeriveInput = parse_quote! {
            struct GoodCapsule {
                state: AtomicU64,
                _padding: [u8; 56],
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should not generate errors for atomic fields
        assert!(!output.contains("compile_error"));
    }

    #[test]
    fn test_error_for_mutex_field() {
        let input: DeriveInput = parse_quote! {
            struct BadCapsule {
                state: Mutex<u64>,
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should generate Mutex COMPILE ERROR (not warning)
        assert!(output.contains("Mutex"));
        assert!(output.contains("compile_error"));
        assert!(output.contains("FORBIDDEN"));
    }

    #[test]
    fn test_error_for_rwlock_field() {
        let input: DeriveInput = parse_quote! {
            struct BadCapsule {
                state: RwLock<State>,
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should generate RwLock COMPILE ERROR (not warning)
        assert!(output.contains("RwLock"));
        assert!(output.contains("compile_error"));
        assert!(output.contains("FORBIDDEN"));
    }

    #[test]
    fn test_error_for_cell_field() {
        let input: DeriveInput = parse_quote! {
            struct BadCapsule {
                state: Cell<u64>,
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should generate Cell COMPILE ERROR (not warning)
        assert!(output.contains("Cell"));
        assert!(output.contains("compile_error"));
        assert!(output.contains("FORBIDDEN"));
    }

    #[test]
    fn test_no_error_for_unsafe_cell() {
        let input: DeriveInput = parse_quote! {
            struct ValidCapsule {
                data: UnsafeCell<[f32; 8]>,
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // UnsafeCell should NOT generate error (allowed for SIMD with atomic coordination)
        assert!(!output.contains("compile_error"));
    }

    #[test]
    fn test_error_for_refcell_field() {
        let input: DeriveInput = parse_quote! {
            struct BadCapsule {
                state: RefCell<u64>,
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should generate RefCell COMPILE ERROR (not warning)
        assert!(output.contains("Cell"));
        assert!(output.contains("compile_error"));
        assert!(output.contains("FORBIDDEN"));
    }

    #[test]
    fn test_no_error_for_padding() {
        let input: DeriveInput = parse_quote! {
            struct GoodCapsule {
                state: AtomicU64,
                _padding: Mutex<u64>,  // Weird but shouldn't error (it's padding)
            }
        };

        let diagnostics = generate_field_diagnostics(&input);
        let output = diagnostics.to_string();

        // Should not error about _padding fields (they're ignored)
        assert!(!output.contains("compile_error") || !output.contains("_padding"));
    }
}
