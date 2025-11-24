# UCE34 Framework Analysis: Clippy Lint Enhancement for Phase 3

**Date**: 2025-10-20
**Framework**: UCE34 (34 Questions for Systematic Discovery)
**Task**: Enhance `clippy::missing_capsule_verification` lint to detect missing `#[derive(ComputationalCapsule)]` on `#[derive(CapsuleSerialize)]` structs

---

## Q1-Q9: Problem Definition & Context

### Q1: What problem are we solving?
**Problem**: Structs using `#[derive(CapsuleSerialize)]` for audit trails MUST also use `#[derive(ComputationalCapsule)]` for compile-time verification, but this is not enforced.

**Impact**:
- Missing alignment verification → false sharing, cache line violations
- Missing size verification → memory layout mismatches
- Broken hash chains → audit trail integrity failures (SOX/SOC2/GDPR compliance risk)

### Q2: Who needs this and why?
**Users**:
- clapi_core developers (Phase 3: audit-trail serialization)
- Future capsule developers using CapsuleSerialize
- Compliance teams (SOX 404, SOC2 Type II, GDPR Article 30)

**Why**:
- Prevent production bugs from unverified capsules
- Ensure hash chain integrity for audit trails
- Compile-time safety (catch errors before runtime)

### Q3: What is the current state?
**Existing**:
- `clippy::missing_capsule_verification` detects `#[repr(C, align(N))]` without verification
- Detects missing `#[derive(ComputationalCapsule)]`
- Does NOT detect `#[derive(CapsuleSerialize)]` dual-derivation requirement

**Gap**:
- CapsuleSerialize structs can bypass verification if they lack ComputationalCapsule
- No size constraint checks (<= 256B for T1, <128B for hot path)
- No hash chain integrity warnings

### Q4: What constraints exist?
**Technical**:
- Clippy plugin architecture (rustc_private, nightly compiler)
- Module-level detection only (~95% accuracy, cannot cross modules)
- Macro expansion happens before lint analysis (post-HIR)

**Performance**:
- Lint must execute < 5ms per crate build (acceptable overhead)
- Zero false positives (actionable errors only)

**Compatibility**:
- Must work with existing clippy infrastructure
- Stable Rust support for end users (lint runs on nightly, targets stable code)

### Q5: What is success?
**Metrics**:
- 100% detection of CapsuleSerialize without ComputationalCapsule
- Zero false positives on correctly dual-derived structs
- < 5ms lint execution overhead (B32 benchmark)
- 20+ unit tests pass, 15+ UI tests pass
- Actionable error messages (tell user exactly what to fix)

### Q6: What existing solutions exist?
**Current lint**:
- Detects `#[repr(C, align(N))]` without verification
- Checks for `#[derive(ComputationalCapsule)]`
- Scans for manual verification macros (`verify_capsule_properties!`)

**Gaps**:
- No CapsuleSerialize detection
- No dual-derivation enforcement
- No size constraint validation

### Q7: What makes this hard?
**Challenges**:
1. **Macro expansion timing**: Derive macros expand before lint analysis (post-HIR)
2. **Cross-crate detection**: CapsuleSerialize defined in atomic_capsule, lint in clippy-capsule-verify
3. **Attribute parsing**: Must detect `#[derive(CapsuleSerialize)]` in attribute list
4. **Size calculation**: Need compile-time size_of::<T>() for constraint checks

### Q8: What are the failure modes?
**False Positives**:
- Struct has both derives but lint still fires → User frustration
- External FFI types trigger warning → Noise

**False Negatives**:
- CapsuleSerialize in different module → Missed detection
- Custom trait impl instead of derive → Not caught

**Performance**:
- Lint takes > 5ms → Slows down builds significantly
- O(n²) algorithm → Exponential slowdown on large codebases

### Q9: What are we NOT solving?
**Out of Scope**:
- Auto-fix suggestions (future work, requires syn AST manipulation)
- Cross-module verification detection (95% accuracy acceptable)
- Runtime validation (compile-time only)
- Non-Rust serialization formats (JSON, protobuf, etc.)

---

## Q10-Q12: Computational Capsule Foundation

### Q10: Which tier transforms this problem?
**Tier**: **Meta-Infrastructure (Verification Tier)**

**Rationale**:
- Not a runtime capsule, but a compile-time verification tool
- Ensures ALL other tiers (T1-T6) are correctly verified
- Foundation for audit trail integrity (Q34)

**Transformation**:
- Before: Manual dual-derivation checking (human error-prone)
- After: Automated clippy lint (compile-time enforcement)

### Q11: What is the Rust transformation?
**Implementation**:
- `rustc_hir` for HIR traversal (struct analysis)
- `rustc_lint` for diagnostic emission
- `syn` attribute parsing (detect derive macros)

**Key Patterns**:
- Attribute inspection: `has_derive_capsule_serialize(attrs)`
- Dual-derivation check: `has_both_derives(attrs)`
- Size constraint check: `validate_size_constraints(tcx, def_id)`

### Q12: What nightly features are needed?
**Required**:
- `rustc_private` - Access to compiler internals (clippy plugin requirement)

**Not Required**:
- No portable_simd (not a runtime capsule)
- No const_trait_impl (lint logic is runtime)
- No generic_const_exprs (no const generics needed)

**Note**: Lint runs on nightly compiler, but lints stable Rust code.

---

## Q13-Q20: Implementation Strategy

### Q13: What are the key data structures?
**Input**:
- `Item<'tcx>` - Struct being analyzed
- `&[Attribute]` - Derive macros, repr attributes
- `TyCtxt<'tcx>` - Type context for size_of calculations

**Detection**:
```rust
struct DualDerivationCheck {
    has_capsule_serialize: bool,
    has_computational_capsule: bool,
    has_repr_c_align: bool,
    alignment: Option<u64>,
    size: Option<u64>,
}
```

### Q14: What are the critical algorithms?
**Algorithm 1: Dual-Derivation Detection**
```rust
fn check_dual_derivation(attrs: &[Attribute]) -> DualDerivationCheck {
    let has_serialize = has_derive_capsule_serialize(attrs);
    let has_capsule = has_derive_computational_capsule(attrs);
    let has_repr = has_repr_c_align(attrs);

    DualDerivationCheck {
        has_capsule_serialize: has_serialize,
        has_computational_capsule: has_capsule,
        has_repr_c_align: has_repr,
        alignment: get_alignment_value(attrs),
        size: None, // Computed later via TyCtxt
    }
}
```

**Algorithm 2: Size Constraint Validation**
```rust
fn validate_size_constraints(tcx: TyCtxt, def_id: DefId, tier: Tier) -> Result<(), SizeError> {
    let size = tcx.layout_of(tcx.param_env(def_id).and(tcx.type_of(def_id)))
        .map(|layout| layout.size.bytes())?;

    match tier {
        Tier::Atomic => assert!(size <= 256, "T1 capsules must be <= 256B"),
        Tier::HotPath => assert!(size <= 128, "Hot path capsules must be <= 128B"),
        _ => Ok(())
    }
}
```

### Q15: What are the performance characteristics?
**Complexity**:
- Attribute scan: O(n) where n = number of attributes (typically 2-5)
- Module scan: O(m) where m = items in module (module-level only)
- Size calculation: O(1) via TyCtxt

**Targets** (B32 Framework):
- Total lint execution: < 5ms per crate
- Per-struct analysis: < 100μs
- Attribute parsing: < 10μs

### Q16: What are the resource requirements?
**Memory**:
- Zero heap allocations (all stack-based)
- Attribute refs borrowed from HIR (zero-copy)

**CPU**:
- Single-pass attribute scan
- No recursive traversal (struct-level only)

**Disk**:
- No I/O (all in-memory lint)

### Q17: What are the failure modes?
**Detection Failures**:
- False negative: CapsuleSerialize in different module (out of scope)
- False positive: External FFI types (user must suppress)

**Performance Failures**:
- > 5ms lint time → Unacceptable build slowdown

**Compatibility Failures**:
- rustc_private API changes between nightly versions

### Q18: What are the concurrency patterns?
**Not Applicable**: Lint is single-threaded (runs in clippy pass).

### Q19: What are the testing requirements?
**T28 Framework**:
- **Unit tests (Q1-Q7)**: 20+ tests for detection logic
- **UI tests (Q8-Q14)**: 15+ tests for error message formatting
- **Integration tests (Q15-Q21)**: End-to-end lint behavior
- **Stress tests (Q22-Q28)**: Large codebase performance

### Q20: What are the monitoring needs?
**Build Metrics**:
- Lint execution time (< 5ms target)
- False positive rate (0% target)
- Detection accuracy (100% for same-module, 95% cross-module)

**CI/CD**:
- GitHub Actions: `cargo clippy -- -D clippy::missing_capsule_verification`
- Fail build on lint warnings

---

## Q21-Q27: Advanced Topics (Not Applicable for Lint)

**Q21-Q24**: Resource constraints, composition, migration, dependencies → Not applicable (lint is meta-infrastructure, not runtime capsule)

**Q25-Q27**: Advanced concurrency, streaming, persistence → Not applicable

---

## Q28-Q33: Optimization & Validation

### Q28: How do we simplify?
**Simplification**:
- Reuse existing `has_derive_computational_capsule()` logic
- Add single new function: `has_derive_capsule_serialize()`
- Combine checks in existing `check_item()` flow

**Avoiding Over-Engineering**:
- No cross-module analysis (diminishing returns, 5% edge cases)
- No auto-fix suggestions (future work, not MVP)
- No custom lint configuration (use existing clippy infra)

### Q29: What are the edge cases?
**Edge Cases**:
1. **Dual serde + CapsuleSerialize**: OK, both allowed
2. **Generic structs**: Must check monomorphized instances (TyCtxt)
3. **Feature-gated derives**: Lint must respect `#[cfg(...)]`
4. **FFI types**: User must suppress with `#[allow(...)]`

### Q30: What validation is needed?
**Compile-Time**:
- Derive macro presence (attribute parsing)
- Size constraints (TyCtxt layout calculations)
- Alignment correctness (repr validation)

**Testing**:
- UI tests verify error messages
- Compile-fail tests verify lint fires correctly
- Compile-pass tests verify no false positives

### Q31: How do we leverage Rust?
**Type Safety**:
- `rustc_hir::Item` type-safe struct representation
- `rustc_lint::Diagnostic` type-safe error emission

**Zero-Cost**:
- All analysis at compile-time
- Zero runtime overhead (lint doesn't ship with binary)

### Q32: What nightly optimizations exist?
**Not Applicable**: Lint uses rustc_private (nightly-only), but targets stable code.

### Q33: How do we verify correctness?
**ASSUM Framework**:
```rust
// #ASSUME_DERIVE_DETECTED: syn correctly parses #[derive(...)]
// #VERIFY_DERIVE_DETECTED: UI tests prove lint fires on missing derive

// #ASSUME_SIZE_ACCURATE: TyCtxt layout_of() returns correct size
// #VERIFY_SIZE_ACCURATE: Compile-fail tests check size constraints

// #ASSUME_NO_FALSE_POSITIVES: Dual-derived structs pass lint
// #VERIFY_NO_FALSE_POSITIVES: Compile-pass tests validate correctness
```

**Verification**:
- `verify_lint_detection!` macro (custom test harness)
- UI tests: 15+ error message checks
- Property tests: 1000+ random struct generations

---

## Q34: Auditability

### Q34: How do we ensure auditability?
**Audit Trail**:
- Git commits track all lint changes
- UI test snapshots preserve historical error messages
- CHANGELOG.md documents all lint behavior changes

**Compliance**:
- Lint ensures capsules meet SOX/SOC2/GDPR audit requirements
- Hash chain integrity depends on dual-derivation enforcement
- Tamper-detection requires ComputationalCapsule verification

**Reproducibility**:
- Deterministic lint behavior (same code → same warnings)
- Version pinning: clippy-capsule-verify version in Cargo.lock
- CI/CD enforcement: `-D clippy::missing_capsule_verification`

---

## Implementation Plan

### Phase 1: Detection Enhancement (300 LOC)
- Add `has_derive_capsule_serialize()` function
- Add dual-derivation check in `check_item()`
- Emit new diagnostic for CapsuleSerialize without ComputationalCapsule

### Phase 2: Size Constraints (200 LOC)
- Add `validate_size_constraints()` using TyCtxt
- Check T1 (<= 256B), hot path (<= 128B)
- Emit diagnostic for oversized capsules

### Phase 3: Testing (20 unit + 15 UI tests)
- Unit tests: Attribute parsing, dual-derivation logic
- UI tests: Error message formatting, suggestions
- Compile-fail: Missing ComputationalCapsule
- Compile-pass: Correct dual-derivation

### Phase 4: Documentation
- ASSUM framework tags for all assumptions
- README updates with dual-derivation examples
- CHANGELOG.md entry

---

## B32 Performance Analysis

**Baseline**: Existing lint: < 1ms per struct
**Target**: Enhanced lint: < 5ms per crate
**Measurement**: `cargo clippy --timings`

**Expected**:
- Attribute scan: +10μs (negligible)
- Size calculation: +50μs (TyCtxt overhead)
- Total: < 100μs per struct (acceptable)

---

## ASSUM Safety Documentation

**Assumptions**:
1. `#ASSUME_SYN_PARSES_CORRECTLY`: syn library correctly parses attributes
2. `#ASSUME_TYCTXT_SIZE_ACCURATE`: TyCtxt layout_of() returns correct size
3. `#ASSUME_MODULE_LEVEL_SUFFICIENT`: 95% detection accuracy acceptable

**Verification**:
1. `#VERIFY_SYN`: UI tests prove attribute parsing correctness
2. `#VERIFY_SIZE`: Compile-fail tests validate size constraints
3. `#VERIFY_DETECTION`: Property tests measure accuracy

---

## Success Criteria

**Functional**:
- ✅ Detects CapsuleSerialize without ComputationalCapsule (100%)
- ✅ Zero false positives on dual-derived structs
- ✅ Actionable error messages (tell user what to fix)

**Performance**:
- ✅ < 5ms lint execution per crate (B32 benchmark)
- ✅ < 100μs per struct analysis

**Testing**:
- ✅ 20+ unit tests pass
- ✅ 15+ UI tests pass
- ✅ Zero test failures

**Quality**:
- ✅ ASSUM framework tags on all assumptions
- ✅ Documentation complete (README, CHANGELOG)
- ✅ CI/CD integration (GitHub Actions)

---

## Deliverables

1. **Enhanced Lint Code**: 400-600 LOC
   - `src/utils.rs`: `has_derive_capsule_serialize()`
   - `src/capsule_lint.rs`: Dual-derivation check
   - `src/size_validation.rs`: Size constraint checks

2. **Tests**: 35+ tests
   - Unit tests: `tests/unit/dual_derivation_tests.rs`
   - UI tests: `tests/ui/capsule_serialize_*.rs`

3. **Documentation**:
   - `README.md`: Updated examples
   - `CHANGELOG.md`: Phase 3 entry
   - `ASSUM.md`: Safety assumptions

4. **Integration**:
   - CapsuleSerialize macro hook (if possible)
   - CI/CD GitHub Actions workflow

---

## Risk Mitigation

**Risk**: False positives on external FFI types
**Mitigation**: Document suppression pattern, add examples

**Risk**: Performance regression (> 5ms)
**Mitigation**: Benchmark before merge, optimize if needed

**Risk**: rustc_private API changes
**Mitigation**: Pin nightly version, test on multiple nightly releases

---

## Conclusion

This enhancement provides **compile-time enforcement** of dual-derivation requirements, ensuring all `CapsuleSerialize` structs have proper verification. This is critical for:
- **Phase 3 audit trails**: Hash chain integrity depends on verified capsules
- **Compliance**: SOX/SOC2/GDPR audit requirements
- **Production safety**: Prevent alignment/size bugs before deployment

**Next Steps**: Implement Phase 1 (detection enhancement) → Write tests → Document → Deploy.
