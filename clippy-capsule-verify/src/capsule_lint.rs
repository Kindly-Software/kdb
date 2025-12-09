//! # Missing Capsule Verification Lint Implementation
//!
//! Detects capsules without compile-time verification macros.

use rustc_hir::{Item, ItemKind};
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_lint, declare_lint_pass};

use crate::utils::{
    has_repr_c_align, get_alignment_value, has_verification_macro,
    has_derive_computational_capsule,
    check_dual_derivation, DualDerivationError
};
use crate::size_validation::{
    validate_size_constraints, infer_tier_from_attributes,
    SizeConstraintViolation
};
use crate::diagnostics::{
    format_verification_benefits_p10,
    format_capsule_diagrams_p10,
    format_verification_importance_p10,
    format_uce34_q33_reference_p10,
    format_verification_fix_suggestion_p10,
};

declare_lint! {
    /// **Detects capsules missing compile-time verification.**
    ///
    /// ## What it does
    /// Checks if structs with `#[repr(C, align(N))]` have verification macros.
    ///
    /// ## Why is this bad?
    /// Capsules without verification can have alignment/size mismatches that cause:
    /// - False sharing (performance degradation)
    /// - Undefined behavior (incorrect atomic operations)
    /// - Cache line violations (unpredictable latency)
    ///
    /// ## Known problems
    /// May trigger false positives for:
    /// - External FFI types (suppress with #[allow])
    /// - Testing/example code (suppress with #[allow])
    ///
    /// ## Example
    /// ```rust,ignore
    /// // Bad: No verification
    /// #[repr(C, align(64))]
    /// struct MyCapsule {
    ///     state: AtomicU64,
    /// }
    ///
    /// // Good: Has verification
    /// #[repr(C, align(64))]
    /// struct MyCapsule {
    ///     state: AtomicU64,
    /// }
    /// verify_capsule_properties!(MyCapsule, 64, 8);
    ///
    /// // Also good: Derive macro
    /// #[derive(ComputationalCapsule)]
    /// #[repr(C, align(64))]
    /// struct MyCapsule {
    ///     state: AtomicU64,
    /// }
    /// ```
    pub MISSING_CAPSULE_VERIFICATION,
    Warn,
    "capsule struct missing compile-time verification"
}

declare_lint_pass!(MissingCapsuleVerification => [MISSING_CAPSULE_VERIFICATION]);

impl<'tcx> LateLintPass<'tcx> for MissingCapsuleVerification {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx Item<'tcx>) {
        // Only check structs
        if !matches!(item.kind, ItemKind::Struct(..)) {
            return;
        }

        // Get attributes from tcx
        let attrs = cx.tcx.hir_attrs(item.hir_id());
        let item_name = cx.tcx.item_name(item.owner_id.to_def_id());

        // Phase 3 Enhancement: Check dual-derivation (CapsuleSerialize + ComputationalCapsule)
        if let Err(DualDerivationError::MissingComputationalCapsule) = check_dual_derivation(attrs) {
            emit_missing_computational_capsule_diagnostic(cx, item, item_name);
            return;
        }

        // Original lint: Check if has #[repr(C, align(N))]
        if !has_repr_c_align(attrs) {
            return;
        }

        // Get alignment value for better diagnostics
        let alignment = get_alignment_value(attrs);

        // Check if has #[derive(ComputationalCapsule)]
        if has_derive_computational_capsule(attrs) {
            return; // OK: Derive macro provides verification
        }

        // Check if verification macro exists in module
        let def_id = item.owner_id.def_id;
        if has_verification_macro_in_module(cx.tcx, def_id) {
            return; // OK: Manual verification macro exists
        }

        // Phase 3 Enhancement: Size constraint validation
        // Check size constraints for capsule structs
        let tier = infer_tier_from_attributes(attrs);
        let def_id = item.owner_id.to_def_id();
        if let Err(violation) = validate_size_constraints(cx.tcx, def_id, tier) {
            emit_size_constraint_diagnostic(cx, item, violation);
        }

        // Emit warning: Missing verification
        emit_missing_verification_diagnostic(cx, item, item_name, alignment);
    }
}

/// Emit diagnostic for CapsuleSerialize missing ComputationalCapsule (Phase 3)
///
/// # UCE34 Q34 (Auditability)
///
/// CapsuleSerialize requires ComputationalCapsule for audit trail integrity:
/// - Missing alignment verification → false sharing → UB in concurrent hash updates
/// - Missing size verification → layout mismatches → corrupted audit trails
/// - Result: SOX/SOC2/GDPR compliance failures
///
/// # ASSUM Framework
///
/// - `#ASSUME_LINT_MESSAGE_ACTIONABLE`: User knows exactly what to fix
/// - `#VERIFY_LINT_HELPFUL`: UI tests validate message clarity
fn emit_missing_computational_capsule_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
    item_name: rustc_span::Symbol,
) {
    let msg = format!(
        "struct `{}` uses #[derive(CapsuleSerialize)] but missing #[derive(ComputationalCapsule)]",
        item_name
    );

    cx.lint(
        MISSING_CAPSULE_VERIFICATION,
        |lint| {
            lint.primary_message(msg);
            lint.span(item.span);
            lint.help("add `#[derive(ComputationalCapsule)]` above `#[derive(CapsuleSerialize)]`");
            lint.note("CapsuleSerialize requires compile-time verification for audit trail integrity");
            lint.note("missing verification causes:");
            lint.note("  - Alignment mismatches → false sharing → UB in concurrent hash updates");
            lint.note("  - Size mismatches → layout corruption → broken audit trails");
            lint.note("  - Compliance failures: SOX 404, SOC2 Type II, GDPR Article 30");
        },
    );
}

/// Emit diagnostic for missing verification (P1.0 Enhanced)
///
/// # Haiku v0.1.0 Enhancement
///
/// Provides comprehensive error message with:
/// - Why verification matters (false sharing, alignment bugs, size mismatches)
/// - Real-world impact (3-10× slowdown, UB, cache line violations)
/// - Compile-time benefits (0ns runtime, <20ms compile)
/// - ASCII diagrams (verified vs unverified capsules)
/// - Exact fix suggestions (derive macro vs manual verification)
/// - UCE34 Q33 framework reference (canonical Chaos verification)
fn emit_missing_verification_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
    item_name: rustc_span::Symbol,
    alignment: Option<u64>,
) {
    let msg = format!(
        "capsule struct `{}` is missing compile-time verification",
        item_name
    );

    // P1.0: Collect all diagnostic lines
    let mut all_notes = vec![];
    all_notes.extend(format_verification_benefits_p10());
    all_notes.extend(format_capsule_diagrams_p10());
    all_notes.extend(format_verification_importance_p10());
    all_notes.extend(format_verification_fix_suggestion_p10(&item_name.to_string(), alignment));
    all_notes.extend(format_uce34_q33_reference_p10());

    cx.lint(
        MISSING_CAPSULE_VERIFICATION,
        move |lint| {
            lint.primary_message(msg);
            lint.span(item.span);

            // P1.0: Primary help message
            lint.help("add #[derive(ComputationalCapsule)] for automatic compile-time verification (0ns runtime, <20ms compile-time)");

            // P1.0: Emit all diagnostic notes
            for line in all_notes {
                if line.is_empty() {
                    lint.note("");
                } else {
                    lint.note(line);
                }
            }
        },
    );
}

/// Emit diagnostic for size constraint violation (Phase 3)
///
/// # UCE34 Q10 (Tier Selection)
///
/// Different tiers have different size limits for optimal performance:
/// - T1 (Atomic): <= 256B (4× cache lines)
/// - Hot Path: <= 128B (2× cache lines, <100ns critical)
/// - T2 (SIMD): <= 512B (8× cache lines)
///
/// # B32 Performance Reality
///
/// Oversized capsules cause:
/// - More cache misses → higher latency
/// - Memory bandwidth contention → throughput degradation
/// - False sharing across adjacent capsules
fn emit_size_constraint_diagnostic<'tcx>(
    cx: &LateContext<'tcx>,
    item: &'tcx Item<'tcx>,
    violation: SizeConstraintViolation,
) {
    match violation {
        SizeConstraintViolation::ExceedsLimit { tier, actual_size, max_size } => {
            let item_name = cx.tcx.item_name(item.owner_id.to_def_id());
            let msg = format!(
                "capsule struct `{}` exceeds {:?} tier size limit ({} bytes > {} bytes max)",
                item_name, tier, actual_size, max_size
            );

            cx.lint(
                MISSING_CAPSULE_VERIFICATION,
                |lint| {
                    lint.primary_message(msg);
                    lint.span(item.span);
                    lint.help(format!(
                        "reduce struct size to {} bytes or use a larger tier",
                        max_size
                    ));
                    lint.note(format!("{:?} tier limit: {} bytes (actual: {} bytes)", tier, max_size, actual_size));
                    lint.note("oversized capsules cause:");
                    lint.note("  - More cache misses → higher latency");
                    lint.note("  - Memory bandwidth contention → throughput degradation");
                    lint.note("  - False sharing across adjacent capsules");
                },
            );
        }
        SizeConstraintViolation::LayoutError => {
            // Silently skip (TyCtxt layout computation failed, likely external type)
        }
    }
}

/// Check if verification macro exists in the same module as the capsule.
///
/// This is a simplified heuristic that assumes verification is handled
/// by the derive macro or explicit allow attributes.
fn has_verification_macro_in_module(tcx: rustc_middle::ty::TyCtxt<'_>, def_id: rustc_hir::def_id::LocalDefId) -> bool {
    has_verification_macro(tcx, def_id)
}
