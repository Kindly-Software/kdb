//! Integration tests for capsule verification lint
//!
//! These tests use compiletest_rs to run UI tests.

use std::path::PathBuf;

#[test]
fn ui_tests() {
    let config = compiletest_rs::Config {
        mode: compiletest_rs::common::Mode::Ui,
        src_base: PathBuf::from("tests/ui"),
        build_base: PathBuf::from("target/ui-tests"),
        rustc_path: PathBuf::from("rustc"),
        target_rustcflags: Some(String::from(
            "--emit=metadata -Dwarnings -Zui-testing \
             -L dependency=target/debug/deps"
        )),
        ..Default::default()
    };

    compiletest_rs::run_tests(&config);
}
