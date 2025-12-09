//! # Compile-Pass and Compile-Fail Tests
//!
//! Uses trybuild to verify proc-macro error messages.

#[test]
fn compile_pass_tests() {
    let t = trybuild::TestCases::new();
    t.pass("tests/compile_pass/*.rs");
}

#[test]
fn compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
