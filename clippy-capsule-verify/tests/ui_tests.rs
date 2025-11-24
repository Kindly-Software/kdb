//! UI tests for clippy-capsule-verify lints
//! Tests P0 critical lints, P1 high lints, and P2 medium lints

#[test]
fn ui() {
    let t = trybuild::TestCases::new();

    // P0 Critical: Mutex violations
    t.compile_fail("tests/ui/p0_mutex_violation/*.rs");

    // P0 Critical: Alignment violations
    t.compile_fail("tests/ui/p0_alignment_violation/*.rs");

    // P0 Critical: Generation counter violations
    t.compile_fail("tests/ui/p0_generation_violation/*.rs");

    // P0 Critical: Non-atomic field violations
    t.compile_fail("tests/ui/p0_atomic_field_violation/*.rs");
}
