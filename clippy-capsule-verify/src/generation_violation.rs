//! # Missing Generation Counter Lint Implementation
//!
//! **P0.3 CRITICAL**: Detects T1 (Atomic) capsules without generation counters.
//!
//! ## UCE34 Q10 (Tier Selection) & COCA Mandate
//!
//! Generation counters are **mandatory** for T1 (Atomic) tier capsules to prevent
//! TOCTOU (time-of-check-time-of-use) races:
//! - Load value → check condition → value changed (ABA problem)
//! - Two-phase commits require generation tracking
//! - Atomic snapshots need versioning for determinism
//! - DualAtomicU64 pattern: data(32) | generation(32) per field
//!
//! ## Why This Matters (B32 Framework)
//!
//! Without generation counters:
//! - Race window: Check value at T1, value changes, use stale value at T2
//! - ABA problem: Can't detect value changed back to original
//! - 3-10× latency spike when retry loops triggered by races
//! - Affects multi-field coordination, snapshots, migrations
//!
//! ## Example
//!
//! ```rust,ignore
//! // BAD: T1 without generation → TOCTOU race risk
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct BadCapsule {
//!     state: AtomicU64,
//!     _padding: [u8; 56],  // Missing generation!
//! }
//!
//! // GOOD: DualAtomicU64 pattern with generation
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(128))]
//! struct GoodCapsule {
//!     primary: AtomicU64,    // state(32) | generation(32)
//!     secondary: AtomicU64,  // metadata(32) | generation(32)
//! }
//!
//! // ALSO GOOD: Standalone generation
//! #[derive(ComputationalCapsule)]
//! #[capsule(tier = "Atomic")]
//! #[repr(C, align(64))]
//! struct SimpleCapsule {
//!     state: AtomicU64,
//!     generation: AtomicU64,  // TOCTOU detection
//! }
//! ```
//!
//! ## ASSUM Framework
//!
//! - `#ASSUME_TIER_DETECTION_ACCURATE`: infer_tier_from_attributes returns correct tier
//! - `#VERIFY_GENERATION_FIELD_PRESENT`: UI tests validate correct/incorrect detection
//! - `#ASSUME_FIELD_IDENT_AVAILABLE`: All struct fields have accessible identifiers
//!
//! ## References
//!
//! - `/home/samuel/Docs/The Atomic Capsule.md` (DualAtomicU64 pattern)
//! - `/home/samuel/Primitives/Docs/KEY_INNOVATIONS.md` (generation counter design)

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};
use rustc_span::Symbol;

use crate::size_validation::infer_tier_from_attributes;

declare_lint! {
    /// **Detects T1 (Atomic) capsules without generation counters.**
    ///
    /// ## What it does
    /// Checks if structs with `#[capsule(tier = "Atomic")]` or inferred T1 tier
    /// (via 64B/128B alignment) have a field with "generation" or "gen" in the name.
    ///
    /// ## Why is this bad?
    /// Generation counters prevent TOCTOU (time-of-check-time-of-use) races:
    /// - Load value → check condition → load again (value might have changed) = race
    /// - ABA problem: Same value after modification (naive comparison fails)
    /// - Atomic snapshots need versioning to detect stale reads
    /// - Two-phase commits require odd/even generation for synchronization
    ///
    /// ## Common patterns
    /// - **DualAtomicU64**: data(32) | generation(32) packed in AtomicU64
    /// - **Standalone**: generation: AtomicU64 (simple T1 capsules)
    /// - **Implied**: Some readonly status capsules may not need it
    ///
    /// ## Example
    /// ```rust,ignore
    /// // Bad: T1 capsule without generation
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(64))]
    /// struct BadCapsule {
    ///     state: AtomicU64,
    /// }
    ///
    /// // Good: With generation (DualAtomicU64)
    /// #[capsule(tier = "Atomic")]
    /// #[repr(C, align(128))]
    /// struct GoodCapsule {
    ///     primary: AtomicU64,    // data(32) | generation(32)
    ///     secondary: AtomicU64,  // metadata(32) | generation(32)
    /// }
    /// ```
    ///
    /// ## Lint level
    /// **WARN** (some simple capsules may not need it, allow with `#[allow(...)]`)
    pub CAPSULE_MISSING_GENERATION,
    Warn,
    "T1 (Atomic) capsule should have generation counter for TOCTOU prevention"
}

declare_lint_pass!(CapsuleGenerationViolation => [CAPSULE_MISSING_GENERATION]);

impl<'tcx> LateLintPass<'tcx> for CapsuleGenerationViolation {
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

        // Check if struct has field with "generation" or "gen" in name
        if let ItemKind::Struct(_, _, variant_data) = &item.kind {
            let has_generation = variant_data.fields().iter().any(|field| {
                // #ASSUME_FIELD_IDENT_AVAILABLE: All fields have accessible ident
                is_generation_field(field.ident.name)
            });

            if !has_generation {
                emit_missing_generation_diagnostic(cx, item);
            }
        }
    }
}

/// Check if field name indicates generation counter
///
/// Matches patterns:
/// - generation, generation_counter, gen, gen_counter
/// - generation_u64, gen_u32, etc.
/// - Case-insensitive matching
fn is_generation_field(field_name: Symbol) -> bool {
    let name = field_name.as_str().to_lowercase();
    name.contains("generation") || name.contains("gen")
}

/// Emit diagnostic for missing generation counter in T1 capsule
///
/// # Enhanced Message Strategy (v2.0)
///
/// - **Primary**: Clear TOCTOU risk statement
/// - **Visual Timeline**: Shows the race condition happening
/// - **Two Solutions**: DualAtomicU64 (production) vs standalone (simple)
/// - **Performance**: <1% overhead prevents 3-10× latency spikes
/// - **Allow Exceptions**: Clear guidance on when to suppress
///
/// # UCE34 Q10 (Tier Selection)
///
/// Generation counters are TOCTOU prevention mechanism for atomic coordination:
/// - **DualAtomicU64 pattern**: data(32) | generation(32) packed bits (production)
/// - **Standalone field**: generation: AtomicU64 (simple cases)
/// - **Exceptions**: Readonly status capsules (document with allow attribute)
/// - Performance impact: <1% overhead for 10-100× race condition prevention
///
/// # ASSUM Framework
///
/// - `#ASSUME_LINT_MESSAGE_ACTIONABLE`: User knows exactly what to add
/// - `#VERIFY_LINT_HELPFUL`: UI tests validate message clarity
fn emit_missing_generation_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
) {
    use crate::diagnostics::*;

    let item_name = cx.tcx.item_name(item.owner_id.to_def_id());

    let msg = format!(
        "TOCTOU race risk: T1 capsule `{}` missing generation counter (3-10× latency spikes possible)",
        item_name
    );

    cx.lint(
        CAPSULE_MISSING_GENERATION,
        |lint| {
            lint.primary_message(msg);
            lint.span(item.span);

            // WHAT IS TOCTOU (visual timeline)
            lint.help("Add generation counter to prevent Time-Of-Check-Time-Of-Use races:");
            lint.note("");
            lint.note("━━━ TOCTOU Race Scenario ━━━");
            lint.note("");
            for line in format_toctou_explanation() {
                lint.note(line);
            }

            // SOLUTION 1: DualAtomicU64 (production recommended)
            lint.note("");
            lint.note("━━━ Solution 1: DualAtomicU64 Pattern (RECOMMENDED) ━━━");
            lint.note("");
            lint.note("Production-grade pattern with built-in versioning:");
            lint.note("");
            for line in format_dual_atomic_pattern() {
                lint.note(line);
            }
            lint.note("");
            lint.note("Benefits:");
            lint.note("  • TOCTOU prevention: generation increments on every update");
            lint.note("  • ABA safety: detect value changed back to original");
            lint.note("  • Atomic snapshots: read both fields + generations in one CAS");
            lint.note("  • <1% overhead: packed in existing AtomicU64 fields");

            // SOLUTION 2: Standalone field (simple cases)
            lint.note("");
            lint.note("━━━ Solution 2: Standalone Generation Field (SIMPLE) ━━━");
            lint.note("");
            lint.note("For simple capsules with single atomic field:");
            lint.note("");
            lint.note("    generation: AtomicU64,  // Increment on every state change");
            lint.note("");
            lint.note("Use when:");
            lint.note("  • Only one data field to track");
            lint.note("  • Simplicity preferred over cache efficiency");
            lint.note("  • Total size ≤64 bytes with padding");

            // PERFORMANCE IMPACT
            lint.note("");
            lint.note("━━━ Performance Impact ━━━");
            lint.note("");
            lint.note(format_speedup(
                "Without generation (retry storms)",
                "30-100ns",
                "With generation (clean detection)",
                "<10ns",
                3.0
            ));
            lint.note("");
            lint.note("Race condition costs:");
            lint.note("  • Retry loop triggered: 3-10× latency spike");
            lint.note("  • Cascading retries: exponential backoff");
            lint.note("  • Silent corruption: undetected ABA problem");
            lint.note("");
            lint.note("Generation counter overhead:");
            lint.note("  • DualAtomicU64: 0 bytes (packed in existing field)");
            lint.note("  • Standalone: 8 bytes (AtomicU64)");
            lint.note("  • CAS cost: <1ns increment per update");

            // WHEN TO SUPPRESS
            lint.note("");
            lint.note("━━━ When to Suppress (Use With Caution) ━━━");
            lint.note("");
            lint.note("Acceptable to suppress (#[allow(clippy::capsule_missing_generation)]):");
            lint.note("  • Read-only status capsules (never modified)");
            lint.note("  • Single-threaded coordination (documented)");
            lint.note("  • Generation tracking external to capsule");
            lint.note("");
            lint.note("MUST document safety proof in comment!");

            // FRAMEWORK COMPLIANCE
            lint.note("");
            lint.note("━━━ Framework Compliance ━━━");
            lint.note("");
            for line in format_framework_compliance(&[
                ("COCA", "TOCTOU prevention (T1 tier requirement)"),
                ("UCE34 Q10", "Generation counter mandate"),
                ("ASSUM", "Document exceptions with safety proof"),
                ("B32", "3-10× proven latency prevention"),
            ]) {
                lint.note(line);
            }

            // DOCUMENTATION
            lint.note("");
            lint.note("━━━ Documentation ━━━");
            lint.note("");
            lint.note("• /home/samuel/Docs/The Atomic Capsule.md (DualAtomicU64 pattern)");
            lint.note("• /home/samuel/Primitives/Docs/KEY_INNOVATIONS.md (TOCTOU prevention)");
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generation_field_detection() {
        let generation_names = vec![
            "generation",
            "generation_counter",
            "gen",
            "gen_counter",
            "Generation",
            "GEN",
            "generation_u64",
        ];

        for name in generation_names {
            let sym = Symbol::intern(name);
            assert!(is_generation_field(sym), "Should detect generation field: {}", name);
        }
    }

    #[test]
    fn test_non_generation_field_detection() {
        let non_generation = vec![
            "state",
            "data",
            "count",
            "value",
            "_padding",
            "_pad",
        ];

        for name in non_generation {
            let sym = Symbol::intern(name);
            assert!(!is_generation_field(sym), "Should not detect as generation: {}", name);
        }
    }
}
