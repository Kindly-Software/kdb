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

// =============================================================================
// Q35 FIELD DETECTION FUNCTIONS
// =============================================================================

/// Detect if struct has DualAtomicU64 field(s)
///
/// # UCE35 Q35 (Self-Destruction Mandate)
/// DualAtomicU64 fields provide dual-channel coordination with built-in poison
/// tracking support. Capsules with DualAtomicU64 can leverage secondary channel
/// for poison state propagation.
///
/// # ASSUM Framework
/// - `#ASSUME_DUAL_ATOMIC_DETECTION`: Field type detection via syn::Type::Path
/// - `#VERIFY_DUAL_ATOMIC_DETECTION`: Unit tests verify detection accuracy
///
/// # Example
/// ```ignore
/// let fields = get_struct_fields(&input);
/// if has_dual_atomic_field(&fields) {
///     // Generate DualAtomicU64-aware poison tracking
/// }
/// ```
pub fn has_dual_atomic_field(fields: &Fields) -> bool {
    match fields {
        Fields::Named(fields_named) => {
            fields_named.named.iter().any(|f| is_dual_atomic_type(&f.ty))
        }
        Fields::Unnamed(fields_unnamed) => {
            fields_unnamed.unnamed.iter().any(|f| is_dual_atomic_type(&f.ty))
        }
        Fields::Unit => false,
    }
}

/// Get all DualAtomicU64 field names
///
/// # UCE35 Q35 (Self-Destruction Mandate)
/// Returns identifiers for all DualAtomicU64 fields, enabling targeted poison
/// injection via `terminate_secondary()` and poison state checking via
/// `is_poisoned()`.
///
/// # ASSUM Framework
/// - `#ASSUME_FIELD_IDENT_PRESENT`: Named fields have identifiers
/// - `#VERIFY_FIELD_IDENT_PRESENT`: Skip unnamed fields gracefully
///
/// # Returns
/// Vec of field identifiers for DualAtomicU64 fields (empty if none found)
pub fn get_dual_atomic_fields(fields: &Fields) -> Vec<syn::Ident> {
    match fields {
        Fields::Named(fields_named) => {
            fields_named.named.iter()
                .filter_map(|f| {
                    if is_dual_atomic_type(&f.ty) {
                        f.ident.clone()
                    } else {
                        None
                    }
                })
                .collect()
        }
        // Unnamed fields don't have identifiers we can use
        Fields::Unnamed(_) | Fields::Unit => Vec::new(),
    }
}

/// Get all atomic field names (AtomicU64, AtomicU32, AtomicBool, etc.)
///
/// # UCE35 Q35 (Self-Destruction Mandate)
/// Returns identifiers for all standard atomic fields (excluding AtomicPtr which
/// requires special handling). These fields can be zeroed during corrupt_state().
///
/// # Detected Types
/// - AtomicU64, AtomicI64, AtomicU32, AtomicI32, AtomicU16, AtomicI16
/// - AtomicU8, AtomicI8, AtomicUsize, AtomicIsize, AtomicBool
///
/// # Excluded Types
/// - AtomicPtr<T> (requires special handling for pointer invalidation)
///
/// # ASSUM Framework
/// - `#ASSUME_ATOMIC_PREFIX`: Atomic types start with "Atomic" prefix
/// - `#VERIFY_ATOMIC_PREFIX`: Excludes AtomicPtr, verified via tests
///
/// # Returns
/// Vec of field identifiers for atomic fields (empty if none found)
pub fn get_atomic_fields(fields: &Fields) -> Vec<syn::Ident> {
    match fields {
        Fields::Named(fields_named) => {
            fields_named.named.iter()
                .filter_map(|f| {
                    if is_atomic_type(&f.ty) {
                        f.ident.clone()
                    } else {
                        None
                    }
                })
                .collect()
        }
        // Unnamed fields don't have identifiers we can use
        Fields::Unnamed(_) | Fields::Unit => Vec::new(),
    }
}

/// Determine default priority from tier (100% PROTECTION - P0 DEFAULT)
///
/// # UCE35 Q35 (Self-Destruction Mandate)
/// Maps capsule tiers to self-destruction priority levels. Following user mandate
/// "100% capsules protected 100%", defaults to P0 (Critical) for maximum protection.
///
/// # Priority Levels
/// - **P0 (Critical)**: Data integrity critical, immediate self-destruction
/// - **P1 (Important)**: Can degrade gracefully, ordered self-destruction
/// - **P2 (Enhanced)**: Auxiliary systems, deferred self-destruction
///
/// # Tier → Priority Mapping
/// | Tier | Priority | Rationale |
/// |------|----------|-----------|
/// | T0 Auditable | P0 | Audit integrity is critical |
/// | T1 Atomic | P0 | Coordination state is critical |
/// | T2 SIMD | P0 | Data processing is critical |
/// | T3 FixedPoint | P0 | Deterministic calculations are critical |
/// | T4 Batch | P0 | Batch processing integrity is critical |
/// | T5 Streaming | P0 | Stream state is critical |
/// | T6 Mixed | P1 | Can fall back to subset of capabilities |
/// | T7 Heterogeneous | P1 | GPU fallback available |
/// | T8 Network | P1 | Can retry/reconnect |
/// | T9 Persistent | P1 | Can recover from disk |
/// | T10 Probabilistic | P1 | Approximate data, can rebuild |
/// | T11 QuantumHybrid | P1 | Classical fallback available |
/// | Unknown/None | P0 | Maximum protection by default |
///
/// # ASSUM Framework
/// - `#ASSUME_P0_DEFAULT`: Unknown tiers default to P0 for safety
/// - `#VERIFY_P0_DEFAULT`: Explicit P1/P2 requires tier justification
///
/// # Arguments
/// * `tier` - Optional tier string from `#[capsule(tier = "...")]`
///
/// # Returns
/// Static string "P0" for ALL tiers (T0-T11) - 100% MAXIMUM PROTECTION
///
/// User mandate: "100% capsules protected 100%" - ALL tiers are CRITICAL
/// ANY capsule failure triggers TERMINAL cascade
pub fn infer_priority_from_tier(tier: &Option<String>) -> &'static str {
    // ALL TIERS ARE P0 (Critical) - MAXIMUM PROTECTION MODE
    // User decision: "i would put P0 for any tier from t0 to t11"
    //
    // This means ANY capsule poisoning triggers TERMINAL cascade:
    // - T0 Auditable: Audit integrity failure = TERMINAL
    // - T1 Atomic: Coordination state failure = TERMINAL
    // - T2 SIMD: Data processing failure = TERMINAL
    // - T3 FixedPoint: Calculation integrity failure = TERMINAL
    // - T4 Batch: Batch processing failure = TERMINAL
    // - T5 Streaming: Stream state failure = TERMINAL
    // - T6 Mixed: Composite failure = TERMINAL
    // - T7 Heterogeneous: GPU/accelerator failure = TERMINAL
    // - T8 Network: Network integrity failure = TERMINAL
    // - T9 Persistent: Storage integrity failure = TERMINAL
    // - T10 Probabilistic: Approximate data failure = TERMINAL
    // - T11 QuantumHybrid: Quantum computation failure = TERMINAL
    //
    // Result: Binary becomes unusable if ANY capsule detects tampering
    "P0"
}

/// Check if a type is DualAtomicU64
///
/// # Implementation Detail
/// Checks if the type path's last segment is "DualAtomicU64".
/// Handles both qualified (atomic_capsule::DualAtomicU64) and unqualified paths.
fn is_dual_atomic_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.last()
            .map(|seg| seg.ident == "DualAtomicU64")
            .unwrap_or(false)
    } else {
        false
    }
}

/// Check if a type is an atomic type (excluding AtomicPtr)
///
/// # Implementation Detail
/// Checks if the type path's last segment starts with "Atomic" but is not "AtomicPtr".
/// AtomicPtr requires special handling for pointer invalidation during self-destruct.
fn is_atomic_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        type_path.path.segments.last()
            .map(|seg| {
                let ident_str = seg.ident.to_string();
                ident_str.starts_with("Atomic") && ident_str != "AtomicPtr"
            })
            .unwrap_or(false)
    } else {
        false
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

    // =========================================================================
    // Q35 FIELD DETECTION TESTS
    // =========================================================================

    #[test]
    fn test_has_dual_atomic_field_positive() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: DualAtomicU64,
                _padding: [u8; 56],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        assert!(has_dual_atomic_field(fields));
    }

    #[test]
    fn test_has_dual_atomic_field_negative() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: AtomicU64,
                counter: AtomicU32,
                _padding: [u8; 52],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        assert!(!has_dual_atomic_field(fields));
    }

    #[test]
    fn test_get_dual_atomic_fields_multiple() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                primary: DualAtomicU64,
                secondary: DualAtomicU64,
                counter: AtomicU64,
                _padding: [u8; 40],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        let dual_fields = get_dual_atomic_fields(fields);
        assert_eq!(dual_fields.len(), 2);
        assert!(dual_fields.iter().any(|f| f == "primary"));
        assert!(dual_fields.iter().any(|f| f == "secondary"));
    }

    #[test]
    fn test_get_atomic_fields_various_types() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: AtomicU64,
                counter: AtomicU32,
                flag: AtomicBool,
                index: AtomicUsize,
                _padding: [u8; 32],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        let atomic_fields = get_atomic_fields(fields);
        assert_eq!(atomic_fields.len(), 4);
        assert!(atomic_fields.iter().any(|f| f == "state"));
        assert!(atomic_fields.iter().any(|f| f == "counter"));
        assert!(atomic_fields.iter().any(|f| f == "flag"));
        assert!(atomic_fields.iter().any(|f| f == "index"));
    }

    #[test]
    fn test_get_atomic_fields_excludes_atomic_ptr() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: AtomicU64,
                ptr: AtomicPtr<u8>,
                _padding: [u8; 48],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        let atomic_fields = get_atomic_fields(fields);
        // Should only include AtomicU64, not AtomicPtr
        assert_eq!(atomic_fields.len(), 1);
        assert!(atomic_fields.iter().any(|f| f == "state"));
        assert!(!atomic_fields.iter().any(|f| f == "ptr"));
    }

    #[test]
    fn test_get_atomic_fields_empty_for_non_atomic() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                data: [f32; 8],
                _padding: [u8; 32],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        let atomic_fields = get_atomic_fields(fields);
        assert!(atomic_fields.is_empty());
    }

    #[test]
    fn test_infer_priority_t0_to_t5_are_p0() {
        // T0-T5: All should be P0 (Critical)
        assert_eq!(infer_priority_from_tier(&Some("Auditable".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Atomic".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("SIMD".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("FixedPoint".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Batch".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Streaming".to_string())), "P0");
    }

    #[test]
    fn test_infer_priority_all_tiers_are_p0() {
        // User decision: "i would put P0 for any tier from t0 to t11"
        // ALL tiers are P0 (Critical) - 100% MAXIMUM PROTECTION

        // T0-T5: Critical
        assert_eq!(infer_priority_from_tier(&Some("Auditable".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Atomic".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("SIMD".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("FixedPoint".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Batch".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Streaming".to_string())), "P0");

        // T6-T11: Also Critical (changed from P1)
        assert_eq!(infer_priority_from_tier(&Some("Mixed".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Heterogeneous".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Network".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Persistent".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Probabilistic".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("QuantumHybrid".to_string())), "P0");
    }

    #[test]
    fn test_infer_priority_unknown_defaults_to_p0() {
        // Unknown tier should default to P0 for maximum protection
        assert_eq!(infer_priority_from_tier(&None), "P0");
        assert_eq!(infer_priority_from_tier(&Some("Unknown".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("".to_string())), "P0");
        assert_eq!(infer_priority_from_tier(&Some("CustomTier".to_string())), "P0");
    }

    #[test]
    fn test_unit_struct_returns_empty() {
        let input: DeriveInput = parse_quote! {
            struct UnitStruct;
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        assert!(!has_dual_atomic_field(fields));
        assert!(get_dual_atomic_fields(fields).is_empty());
        assert!(get_atomic_fields(fields).is_empty());
    }

    #[test]
    fn test_qualified_path_dual_atomic() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: atomic_capsule::patterns::DualAtomicU64,
                _padding: [u8; 56],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        // Should detect DualAtomicU64 even with qualified path
        assert!(has_dual_atomic_field(fields));
        let dual_fields = get_dual_atomic_fields(fields);
        assert_eq!(dual_fields.len(), 1);
        assert!(dual_fields.iter().any(|f| f == "state"));
    }

    #[test]
    fn test_qualified_path_atomic() {
        let input: DeriveInput = parse_quote! {
            struct TestCapsule {
                state: core::sync::atomic::AtomicU64,
                flag: std::sync::atomic::AtomicBool,
                _padding: [u8; 48],
            }
        };

        let fields = match &input.data {
            syn::Data::Struct(data) => &data.fields,
            _ => panic!("Expected struct"),
        };

        // Should detect atomic types even with qualified paths
        let atomic_fields = get_atomic_fields(fields);
        assert_eq!(atomic_fields.len(), 2);
        assert!(atomic_fields.iter().any(|f| f == "state"));
        assert!(atomic_fields.iter().any(|f| f == "flag"));
    }
}
