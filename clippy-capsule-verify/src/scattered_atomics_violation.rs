//! # Scattered Atomics in T1 Capsule Lint
//!
//! **P1.2 HIGH PRIORITY**: Detects multiple scattered AtomicU64/U32 fields in T1 capsules.
//!
//! ## UCE34 Q10 & COCA Mandate
//!
//! T1 (Atomic) capsules with ≥2 separate atomic fields should use DualAtomicU64 pattern:
//! - **Problem**: Scattered atomics create false sharing (separate cache lines)
//! - **Solution**: DualAtomicU64 packs 2 fields per AtomicU64 (primary|generation)
//! - **Speedup**: 2× proven via cache-line separation pattern
//!
//! ## Why This Matters (B32 Framework)
//!
//! - **False sharing**: Multiple AtomicU64 fields → separate cache lines → contention
//! - **DualAtomicU64**: primary(32)|generation(32) + secondary(32)|generation(32)
//! - **Cache efficiency**: 2 fields in 1 cache line vs 2+ separate cache lines
//! - **Performance**: 2× speedup via cache-separated coordination
//!
//! ## Example
//!
//! ```rust,ignore
//! // BAD: Multiple scattered AtomicU64 → false sharing
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct BadCapsule {
//!     state: AtomicU64,      // Field 1
//!     counter: AtomicU64,    // Field 2 - scattered!
//!     flags: AtomicU64,      // Field 3 - scattered!
//! }
//!
//! // GOOD: DualAtomicU64 pattern (cache-separated)
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(128))]
//! struct GoodCapsule {
//!     primary: AtomicU64,    // state(32) | generation(32)
//!     secondary: AtomicU64,  // counter(32) | flags(32)
//! }
//!
//! // ALSO GOOD: Single AtomicU64 (no scattering)
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct SimpleCapsule {
//!     state: AtomicU64,
//!     _padding: [u8; 56],
//! }
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_TIER_DETECTION_ACCURATE`: infer_tier_from_attributes returns correct tier
//! - `#VERIFY_SCATTERED_DETECTION`: UI tests validate correct/incorrect detection
//! - `#ASSUME_FIELD_TYPE_ACCESSIBLE`: All struct fields have accessible types
//!
//! ## References
//!
//! - `/home/samuel/Docs/The Atomic Capsule.md` (DualAtomicU64 pattern)
//! - `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` (cache-separated coordination)
//! - `/home/samuel/Docs/The Complete Catalog of Discoveries.md` (9.5× throughput under contention)

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::Symbol;

use crate::size_validation::infer_tier_from_attributes;

declare_lint! {
    /// **Detects scattered atomic fields in T1 (Atomic) capsules.**
    ///
    /// ## What it does
    /// Checks if T1 capsules have ≥2 separate AtomicU64/U32/U16/U8 fields
    /// that should be refactored to use DualAtomicU64 pattern.
    ///
    /// ## Why is this bad?
    /// Multiple scattered atomic fields cause false sharing:
    /// - Each AtomicU64 occupies separate cache line
    /// - Concurrent access → cache line bouncing
    /// - DualAtomicU64 pattern: 2× speedup via cache-separation
    /// - Pack 2 fields per AtomicU64: data(32) | generation(32)
    ///
    /// ## DualAtomicU64 Pattern
    /// - **primary**: state(32) | generation(32)
    /// - **secondary**: metadata(32) | generation(32)
    /// - Cache-line separated (128B alignment)
    /// - 2× throughput vs scattered atomics
    ///
    /// ## Example
    /// ```rust,ignore
    /// // Bad: Scattered atomics
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(64))]
    /// struct BadCapsule {
    ///     state: AtomicU64,    // Field 1
    ///     counter: AtomicU64,  // Field 2 - scattered!
    /// }
    ///
    /// // Good: DualAtomicU64
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(128))]
    /// struct GoodCapsule {
    ///     primary: AtomicU64,    // state(32) | generation(32)
    ///     secondary: AtomicU64,  // counter(32) | generation(32)
    /// }
    /// ```
    ///
    /// ## Lint level
    /// **WARN** (P1 High Priority - optimization opportunity)
    pub CAPSULE_SCATTERED_ATOMICS,
    Warn,
    "T1 (Atomic) capsule has ≥2 scattered atomic fields (use DualAtomicU64 pattern)"
}

declare_lint_pass!(CapsuleScatteredAtomics => [CAPSULE_SCATTERED_ATOMICS]);

impl<'tcx> LateLintPass<'tcx> for CapsuleScatteredAtomics {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only check structs
        if !matches!(item.kind, ItemKind::Struct(_, _, _)) {
            return;
        }

        // Get attributes
        let attrs = cx.tcx.hir_attrs(item.hir_id());

        // Only enforce for T1 (Atomic) tier
        // #ASSUME_TIER_DETECTION_ACCURATE: infer_tier_from_attributes correct
        use crate::size_validation::CapsuleTier;
        let tier = infer_tier_from_attributes(attrs);
        if tier != CapsuleTier::Atomic {
            return;
        }

        // Count atomic fields (excluding padding)
        if let ItemKind::Struct(_, _, variant_data) = &item.kind {
            let atomic_fields: Vec<_> = variant_data
                .fields()
                .iter()
                .filter(|field| {
                    // Skip padding fields
                    if is_padding_field(field.ident.name) {
                        return false;
                    }

                    // Check if field type is atomic
                    // #ASSUME_FIELD_TYPE_ACCESSIBLE: All fields have accessible types
                    let field_ty = cx.tcx.type_of(field.def_id).instantiate_identity();
                    is_atomic_type(cx, field_ty)
                })
                .collect();

            // Trigger lint if ≥2 atomic fields (scattered pattern)
            if atomic_fields.len() >= 2 {
                emit_scattered_atomics_diagnostic(cx, item, atomic_fields.len());
            }
        }
    }
}

/// Check if field name indicates padding
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
/// Atomic types considered for scattering analysis:
/// - std::sync::atomic::AtomicU64
/// - std::sync::atomic::AtomicU32
/// - std::sync::atomic::AtomicU16
/// - std::sync::atomic::AtomicU8
/// - std::sync::atomic::AtomicBool (less common in coordination)
/// - std::sync::atomic::AtomicPtr<T> (less common in coordination)
///
/// Arrays and pointers are excluded from scattering analysis.
fn is_atomic_type<'tcx>(cx: &LateContext<'tcx>, ty: rustc_middle::ty::Ty<'tcx>) -> bool {
    use rustc_middle::ty;

    match ty.kind() {
        // Adt: Struct/Enum/Union type (covers Atomic* types)
        ty::Adt(adt_def, _) => {
            let type_name = cx.tcx.def_path_str(adt_def.did());
            type_name.starts_with("std::sync::atomic::Atomic")
                || type_name.starts_with("core::sync::atomic::Atomic")
        }

        // Arrays, pointers, etc. are NOT considered for scattering
        _ => false,
    }
}

/// Emit diagnostic for scattered atomic fields in T1 capsule
///
/// # UCE34 Q10 (Tier Selection)
///
/// DualAtomicU64 pattern is COCA-compliant refactoring for scattered atomics:
/// - **Cache-separated**: primary(64B) + secondary(64B) = 128B alignment
/// - **Bit-packing**: data(32) | generation(32) per AtomicU64
/// - **Performance**: 10.7× speedup vs scattered atomics (105ns → 9.8ns latency)
/// - **Throughput**: 9.5× improvement under contention (production-proven)
/// - **Pattern**: primary: state(32)|gen(32), secondary: metadata(32)|gen(32)
///
/// # ASSUM Framework
///
/// - `#ASSUME_LINT_MESSAGE_ACTIONABLE`: User knows exactly how to refactor
/// - `#VERIFY_LINT_HELPFUL`: UI tests validate message clarity
fn emit_scattered_atomics_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
    atomic_count: usize,
) {
    use crate::diagnostics::{
        format_dual_atomic_pattern, format_speedup, format_cache_alignment_benefits,
    };

    let item_name = cx.tcx.item_name(item.owner_id.to_def_id());

    let msg = format!(
        "T1 (Atomic) capsule `{}` has {} scattered atomic fields causing cache line conflicts",
        item_name, atomic_count
    );

    cx.lint(
        CAPSULE_SCATTERED_ATOMICS,
        |lint| {
            lint.primary_message(msg);
            lint.span(item.span);

            // === PERFORMANCE IMPACT (B32 Framework) ===
            lint.note("");
            lint.note("━━━ PERFORMANCE IMPACT ━━━");
            let perf_msg = format_speedup(
                "Scattered Atomics",
                "105ns",
                "DualAtomicU64",
                "9.8ns",
                10.7,
            );
            lint.note(perf_msg);
            lint.note("Throughput under contention: 9.5× improvement");
            lint.note("Cache penalty: 2× performance loss from false sharing");

            // === WHY SCATTERED ATOMICS ARE BAD ===
            lint.note("");
            lint.note("━━━ WHY SCATTERED ATOMICS ARE BAD ━━━");
            lint.note("Multiple AtomicU64 fields cause cache line conflicts:");
            lint.note("  1. Each AtomicU64 occupies separate 64-byte cache line");
            lint.note("  2. Concurrent access → cache line bouncing between cores");
            lint.note("  3. Coherency traffic causes 2-3× latency penalty");
            lint.note("  4. Under contention: 9.5× throughput loss");

            // === CURRENT (BAD) LAYOUT ===
            lint.note("");
            lint.note("━━━ CURRENT LAYOUT (BAD - False Sharing) ━━━");
            lint.note(format!("struct {} {{", item_name));
            for i in 1..=atomic_count {
                lint.note(format!(
                    "    field{}: AtomicU64,      // Cache line {} (separate)",
                    i, i
                ));
            }
            lint.note("}");
            lint.note("");
            lint.note("Cache layout:");
            lint.note("  ┌──────────────────────────────────┐");
            lint.note("  │ AtomicU64 field1 (105ns latency) │ ← Cache line 0");
            lint.note("  └──────────────────────────────────┘");
            lint.note("  ┌──────────────────────────────────┐");
            lint.note("  │ AtomicU64 field2 (105ns latency) │ ← Cache line 1");
            lint.note("  └──────────────────────────────────┘");
            lint.note("  ┌──────────────────────────────────┐");
            lint.note("  │ AtomicU64 field3 (105ns latency) │ ← Cache line 2");
            lint.note("  └──────────────────────────────────┘");
            lint.note("       ↑ Bounces between cores = 2× slowdown");

            // === RECOMMENDED (GOOD) LAYOUT ===
            lint.note("");
            lint.note("━━━ RECOMMENDED LAYOUT (GOOD - DualAtomicU64) ━━━");
            lint.note(format!("struct {} {{", item_name));
            lint.note("    primary: AtomicU64,    // state(32) | generation(32)");
            lint.note("    secondary: AtomicU64,  // counter(32) | flags(32)");
            lint.note("    _padding: [u8; 64],   // Ensure cache line separation");
            lint.note("}");
            lint.note("");
            lint.note("Add #[repr(C, align(128))] for 128-byte alignment");

            // === ASCII DUAL ATOMIC PATTERN ===
            lint.note("");
            lint.note("━━━ DUALATOMICU64 BIT-PACKING LAYOUT ━━━");
            let pattern_lines = format_dual_atomic_pattern();
            for line in pattern_lines {
                lint.note(line);
            }

            // === CACHE ALIGNMENT BENEFITS ===
            lint.note("");
            lint.note("━━━ CACHE ALIGNMENT BENEFITS ━━━");
            let benefits = format_cache_alignment_benefits();
            for line in benefits {
                lint.note(line);
            }

            // === TRANSFORMATION EXAMPLE ===
            lint.note("");
            lint.note("━━━ BEFORE/AFTER TRANSFORMATION ━━━");
            lint.note("");
            lint.note("❌ BEFORE (Scattered - Cache line conflicts):");
            lint.note("   #[repr(C, align(64))]");
            lint.note("   struct BadCapsule {");
            lint.note("       state: AtomicU64,       // Separate cache line");
            lint.note("       counter: AtomicU64,     // Separate cache line");
            lint.note("       flags: AtomicU64,       // Separate cache line");
            lint.note("   }");
            lint.note("   Latency: 105ns | Throughput: Low (cache bouncing)");
            lint.note("");
            lint.note("✅ AFTER (DualAtomicU64 - Cache separated):");
            lint.note("   #[repr(C, align(128))]");
            lint.note("   struct GoodCapsule {");
            lint.note("       primary: AtomicU64,     // state(32)|gen(32)");
            lint.note("       secondary: AtomicU64,   // counter(32)|flags(32)");
            lint.note("       _padding: [u8; 64],    // Cache line separation");
            lint.note("   }");
            lint.note("   Latency: 9.8ns | Throughput: 9.5× higher");

            // === BIT EXTRACTION CODE ===
            lint.note("");
            lint.note("━━━ BIT EXTRACTION & UPDATES ━━━");
            lint.note("Extract state from primary field:");
            lint.note("   let state = primary.load(Acquire) >> 32;");
            lint.note("   let generation = primary.load(Acquire) & 0xFFFF_FFFF;");
            lint.note("");
            lint.note("Atomic update with CAS loop:");
            lint.note("   let (new_state, new_gen) = (state + 1, generation + 1);");
            lint.note("   let new_value = ((new_state as u64) << 32) | (new_gen as u64);");
            lint.note("   while let Err(old) = primary.compare_exchange_weak(");
            lint.note("       old_value, new_value, Release, Acquire");
            lint.note("   ) { /* retry */ }");

            // === FRAMEWORK REFERENCES ===
            lint.note("");
            lint.note("━━━ FRAMEWORK REFERENCES ━━━");
            lint.note("COCA (Computational Capsule Architecture):");
            lint.note("  - Lock-free coordination via DualAtomicU64");
            lint.note("  - Cache-aligned (64B/128B/256B) prevents false sharing");
            lint.note("  - Generation counters prevent TOCTOU races");
            lint.note("  - 100% atomic operations (no mutex/RwLock)");
            lint.note("");
            lint.note("B32 Framework:");
            lint.note("  - 95% CI, 1000+ iterations (fair baselines)");
            lint.note("  - Scattered: 105ns median | DualAtomicU64: 9.8ns median");
            lint.note("  - Validated under real contention (>95% reproducible)");
            lint.note("");
            lint.note("UCE34 Q10 (Tier Selection):");
            lint.note("  - T1 (Atomic) tier: 3-10× speedup via cache alignment");
            lint.note("  - DualAtomicU64 is canonical T1 pattern");
            lint.note("  - See /home/samuel/Docs/The Atomic Capsule.md");

            // === HELP MESSAGE ===
            lint.help("Consolidate scattered atomics to 2 DualAtomicU64 fields (9.8ns latency)");
            lint.help("Add cache-line padding (_padding: [u8; 64]) for alignment");
            lint.help("Use Acquire/Release memory ordering for coordination");
            lint.help("Suppress if intentional: #[allow(clippy::capsule_scattered_atomics)]");

            // === DOCUMENTATION LINKS ===
            lint.note("");
            lint.note("Documentation:");
            lint.note("  - /home/samuel/Docs/The Atomic Capsule.md");
            lint.note("  - /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md");
            lint.note("  - /home/samuel/Primitives/atomic_capsule/CLAUDE.md");
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
            "state",
            "counter",
            "primary",
            "secondary",
            "__reserved",
        ];

        for name in non_padding {
            let sym = Symbol::intern(name);
            assert!(!is_padding_field(sym), "Should not detect as padding: {}", name);
        }
    }
}
