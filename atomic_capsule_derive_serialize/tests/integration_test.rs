//! Integration tests using trybuild for compile-pass/fail tests

#[test]
fn compile_tests() {
    let t = trybuild::TestCases::new();

    // Compile-pass tests (should succeed)
    t.pass("tests/compile_pass/basic_capsule.rs");
    t.pass("tests/compile_pass/skip_field.rs");
    t.pass("tests/compile_pass/hash_key_field.rs");
    t.pass("tests/compile_pass/transparent_newtype.rs");

    // Compile-fail tests (should fail with specific errors)
    t.compile_fail("tests/compile_fail/missing_repr.rs");
    t.compile_fail("tests/compile_fail/invalid_type.rs");
    t.compile_fail("tests/compile_fail/no_serializable_fields.rs");
    t.compile_fail("tests/compile_fail/transparent_multiple_fields.rs");
    t.compile_fail("tests/compile_fail/transparent_with_skip.rs");
}
