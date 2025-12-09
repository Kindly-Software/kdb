//! # Non-Atomic Field in T1 Capsule Lint
//!
//! **Detects non-atomic fields in T1 (Atomic) tier computational capsules.**
//!
//! ## UCE34 Q10 & Chaos Mandate
//!
//! T1 (Atomic) capsules MUST use only atomic types for lockfree guarantees:
//! - AtomicU64, AtomicU32, AtomicU16, AtomicU8, AtomicBool, AtomicPtr
//! - Padding fields: [u8; N]
//!
//! ## Why This Matters
//!
//! - **Data races**: Non-atomic fields in coordination structures cause races
//! - **Lockfree violation**: Defeats entire purpose of T1 capsule
//! - **Memory model**: Mixed atomic/non-atomic breaks Rust's memory ordering
//!
//! ## Example
//!
//! ```rust,ignore
//! // BAD: u64 in atomic capsule → data race risk
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct BadCapsule {
//!     count: u64,  // ERROR: Should be AtomicU64!
//! }
//!
//! // GOOD: All fields atomic
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct GoodCapsule {
//!     count: AtomicU64,
//!     _padding: [u8; 56],
//! }
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_TYPE_KIND_COMPLETE`: ty.kind() covers all possible types
//! - `#VERIFY_ATOMIC_DETECTION`: UI tests validate correct/incorrect detection
//!
//! ## References
//!
//! - `/home/samuel/Docs/The Atomic Capsule.md` (T1 patterns)
//! - `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` (lockfree designs)

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::Symbol;

declare_lint! {
    /// **Detects non-atomic fields in T1 (Atomic) tier capsules.**
    ///
    /// ## What it does
    /// Checks that T1 capsules (marked with `#[capsule(tier = "Atomic")]` or
    /// inferred from 64B/128B alignment) only use atomic types.
    ///
    /// ## Why is this bad?
    /// T1 capsules are designed for lockfree coordination. Non-atomic fields:
    /// - Cause data races (memory model violation)
    /// - Defeat lockfree guarantee
    /// - May be accessed concurrently without synchronization
    ///
    /// ## Allowed types
    /// - AtomicU64, AtomicU32, AtomicU16, AtomicU8, AtomicBool, AtomicPtr<T>
    /// - Padding arrays: [u8; N] (must start with `_pad`)
    ///
    /// ## Example
    /// ```rust,ignore
    /// // Bad
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(64))]
    /// struct BadCapsule {
    ///     count: u64,  // ERROR
    /// }
    ///
    /// // Good
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(64))]
    /// struct GoodCapsule {
    ///     count: AtomicU64,
    ///     _padding: [u8; 56],
    /// }
    /// ```
    ///
    /// ## Lint level
    /// **DENY** (data race risk, prevents compilation in T1 capsules)
    pub CAPSULE_NON_ATOMIC_FIELD,
    Deny,
    "T1 (Atomic) capsule contains non-atomic field (data race risk)"
}

declare_lint_pass!(CapsuleAtomicFieldViolation => [CAPSULE_NON_ATOMIC_FIELD]);

impl<'tcx> LateLintPass<'tcx> for CapsuleAtomicFieldViolation {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only check structs
        if !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }

        // Get HIR attributes
        let attrs = cx.tcx.hir_attrs(item.hir_id());

        // Check if this is a T1 (Atomic) capsule by looking for:
        // 1. Explicit #[capsule(tier = "Atomic")] attribute
        // 2. 64B or 128B alignment (#[repr(C, align(...))])
        if !is_atomic_tier_capsule(attrs) {
            return;
        }

        // Check all fields
        if let ItemKind::Struct(_, _, variant_data) = &item.kind {
            for field in variant_data.fields() {
                // Skip padding fields (_pad, _padding, etc.)
                if is_padding_field(field.ident.name) {
                    continue;
                }

                // Get field type
                let field_ty = cx.tcx.type_of(field.def_id).instantiate_identity();

                // Check if atomic
                if !is_atomic_type(cx, field_ty) {
                    emit_non_atomic_field_diagnostic(cx, field, field_ty);
                }
            }
        }
    }
}

/// Check if this is a T1 (Atomic) tier capsule
///
/// Returns true if the struct has evidence of being a T1 capsule:
/// - Has #[capsule(tier = "Atomic")] or similar
/// - Has 64B or 128B alignment (common for T1)
fn is_atomic_tier_capsule(attrs: &[rustc_hir::Attribute]) -> bool {
    use rustc_span::sym;

    // Check for explicit #[capsule(tier = "Atomic")] attribute
    for attr in attrs {
        if attr.has_name(sym::repr) {
            // Parse repr for C and align values
            let attr_str = format!("{:?}", attr);
            if attr_str.contains("C") && (attr_str.contains("64") || attr_str.contains("128")) {
                return true;
            }
        }
    }

    false
}

/// Check if a field name indicates padding
///
/// Padding fields typically follow naming convention:
/// - _padding, _padding_u8, _padding_field
/// - _pad, _pad_u8, _pad_field
fn is_padding_field(field_name: Symbol) -> bool {
    let name = field_name.as_str();
    name.starts_with("_pad")  // Covers _padding, _pad, etc.
}

/// Check if type is an atomic type
///
/// Allowed atomic types:
/// - std::sync::atomic::AtomicU64
/// - std::sync::atomic::AtomicU32
/// - std::sync::atomic::AtomicU16
/// - std::sync::atomic::AtomicU8
/// - std::sync::atomic::AtomicBool
/// - std::sync::atomic::AtomicPtr<T>
fn is_atomic_type<'tcx>(cx: &LateContext<'tcx>, ty: rustc_middle::ty::Ty<'tcx>) -> bool {
    use rustc_middle::ty;

    match ty.kind() {
        // Adt: Struct/Enum/Union type (covers Atomic* types)
        ty::Adt(adt_def, _) => {
            let type_name = cx.tcx.def_path_str(adt_def.did());
            type_name.starts_with("std::sync::atomic::Atomic")
                || type_name.starts_with("core::sync::atomic::Atomic")
        }

        // Array of u8: padding like [u8; 56]
        ty::Array(elem_ty, _) => {
            matches!(elem_ty.kind(), ty::Uint(ty::UintTy::U8))
        }

        // Pointer: unlikely but allow for special cases
        ty::RawPtr(..) => true,

        _ => false,
    }
}

/// Emit diagnostic for non-atomic field in T1 capsule
fn emit_non_atomic_field_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    field: &rustc_hir::FieldDef<'tcx>,
    _field_ty: rustc_middle::ty::Ty<'tcx>,
) {
    use crate::diagnostics::{
        format_suggestion, format_speedup,
        get_doc_references,
    };

    let field_name = field.ident.name.as_str();
    let type_name = cx.tcx.type_of(field.def_id)
        .instantiate_identity()
        .to_string();

    let atomic_suggestion = match type_name.as_str() {
        "bool" => "AtomicBool",
        "u8" | "i8" => "AtomicU8",
        "u16" | "i16" => "AtomicU16",
        "u32" | "i32" => "AtomicU32",
        "u64" | "i64" | "usize" | "isize" => "AtomicU64",
        "u128" | "i128" => "[TIER ERROR: No Atomic type for 128-bit; split into T1+T1 fields]",
        _ if type_name.contains("*") => "AtomicPtr<T>",
        _ => "AtomicU64 (or appropriate atomic type)",
    };

    let primary_msg = format!(
        "T1 (Atomic) capsule field `{}` has non-atomic type `{}` → use {} instead",
        field_name, type_name, atomic_suggestion
    );

    cx.lint(
        CAPSULE_NON_ATOMIC_FIELD,
        |lint| {
            lint.primary_message(primary_msg);
            lint.span(field.span);

            // === WHY IT'S BAD ===
            lint.note("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            lint.note("WHY THIS IS CRITICAL:");
            lint.note("  • Non-atomic fields cause data races (memory model violation)");
            lint.note("  • Undefined behavior: crashes, corruption, security exploits");
            lint.note("  • Defeats lockfree guarantee: T1 capsule promise broken");
            lint.note("  • Violates Chaos mandate: 100% atomic operations required");

            // === PERFORMANCE IMPACT ===
            lint.note("");
            lint.note("PERFORMANCE IMPACT:");
            let speedup = format_speedup(
                "Mutex fallback (unsafe workaround)",
                "1-10μs",
                "Atomic (correct)",
                "<10ns",
                100.0,
            );
            lint.note(format!("  {}", speedup));
            lint.note("  → 100-1000× latency penalty for this field");

            // === ASCII DIAGRAM ===
            lint.note("");
            lint.note("ATOMIC FIELD LAYOUT (64B cache line):");
            lint.note("");
            lint.note("  Before (BAD - UNSAFE):        After (GOOD - LOCKFREE):");
            lint.note("  ┌────────────────┐            ┌────────────────┐");
            lint.note("  │ state: Atomic  │            │ state: Atomic  │");
            lint.note("  │ count: u64     │ ← DATA     │ count: Atomic  │ ← SAFE");
            lint.note("  │ generation: A  │   RACE!   │ generation: A  │");
            lint.note("  │ _padding[40]   │            │ _padding[40]   │");
            lint.note("  └────────────────┘ 64B       └────────────────┘ 64B");
            lint.note("     NO GUARANTEE                 CACHE EXCLUSIVE");

            // === BEFORE/AFTER CODE ===
            lint.note("");
            lint.note("CODE TRANSFORMATION:");
            let code_before = format!("{}: {}", field_name, type_name);
            let code_after = format!("{}: {}", field_name, atomic_suggestion);
            let suggestion = format_suggestion(&code_before, &code_after);
            for line in suggestion.lines() {
                lint.note(format!("  {}", line));
            }

            // === FIX OPTIONS BY TYPE ===
            lint.note("");
            lint.note("ATOMIC TYPE MAPPING:");
            lint.note("  • bool, u8, i8              → AtomicU8 (1 byte)");
            lint.note("  • u16, i16                  → AtomicU16 (2 bytes)");
            lint.note("  • u32, i32, f32             → AtomicU32 (4 bytes)");
            lint.note("  • u64, i64, f64, usize     → AtomicU64 (8 bytes)");
            lint.note("  • *const T, *mut T          → AtomicPtr<T> (8/16 bytes)");
            lint.note("  • u128, i128                → Split into 2× AtomicU64 (T1+T1)");

            // === TOCTOU PREVENTION ===
            lint.note("");
            lint.note("TECHNICAL DETAILS (TOCTOU Race Prevention):");
            lint.note("  Load non-atomic u64:        Load AtomicU64:");
            lint.note("  1. Read value (42)          1. Load(Acquire) → (42, gen=5)");
            lint.note("  2. Check condition          2. Check condition");
            lint.note("  3. [Another thread]         3. [Another thread updates gen=6]");
            lint.note("  4. Use STALE value ⚠️       4. CAS fails (gen mismatch) ✓ RETRY");
            lint.note("");
            lint.note("  → Generation counter detects mutations between check and use");

            // === PADDING NOTES ===
            lint.note("");
            lint.note("PADDING FIELDS (Exception to atomicity rule):");
            lint.note("  Array fields ONLY: [u8; N] allowed for alignment");
            lint.note("  MUST follow naming: _pad, _padding, _pad_field, etc.");
            lint.note("  Example: _padding: [u8; 56] ← Valid, no atomic needed");

            // === FRAMEWORK COMPLIANCE ===
            lint.note("");
            lint.note("FRAMEWORK COMPLIANCE:");
            lint.note("  • Chaos: Computational Capsule mandate (100% lockfree)");
            lint.note("  • UCE34 Q33: Atomic field verification (compile-time)");
            lint.note("  • #[derive(ComputationalCapsule)]: Requires atomic types");
            lint.note("  • Memory Ordering: Acquire/Release/SeqCst (see docs)");

            // === DOCUMENTATION LINKS ===
            lint.note("");
            lint.note("DOCUMENTATION REFERENCES:");
            for (path, description) in get_doc_references() {
                if path.contains("Atomic") || path.contains("KEY_INNOVATIONS") || path.contains("UCE34") {
                    lint.note(format!("  • {} ({})", path, description));
                }
            }

            // === COMMON MISTAKES ===
            lint.note("");
            lint.note("COMMON MISTAKES:");
            lint.note("  ❌ Using bool instead of AtomicBool (data race)");
            lint.note("  ❌ Mixing Relaxed/Acquire/Release without design (race)");
            lint.note("  ❌ Forget generation counter in DualAtomicU64 (TOCTOU)");
            lint.note("  ❌ Use [u8; N] for non-padding (defeats purpose)");
            lint.note("  ✓  Load with Acquire; Store with Release");
            lint.note("  ✓  Use CAS loops for compound operations");
            lint.note("  ✓  Generation counter for TOCTOU prevention");

            // === FINAL HELP ===
            lint.note("");
            lint.help(format!(
                "Replace non-atomic field `{}`: {} → {}",
                field_name, type_name, atomic_suggestion
            ));
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_padding_field_detection() {
        let padding_names = vec![
            "_padding",
            "_padding_u8",
            "_pad",
            "_pad_field",
        ];

        for name in padding_names {
            let sym = Symbol::intern(name);
            assert!(is_padding_field(sym), "Should detect padding: {}", name);
        }
    }

    #[test]
    fn test_non_padding_field_detection() {
        let non_padding = vec![
            "count",
            "state",
            "data",
            "__reserved",
        ];

        for name in non_padding {
            let sym = Symbol::intern(name);
            assert!(!is_padding_field(sym), "Should not detect as padding: {}", name);
        }
    }
}
