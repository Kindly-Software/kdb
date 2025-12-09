#![cfg(feature = "serde")]

use assert_cmd::Command;

#[test]
fn runs_with_json_config() {
    let mut cmd = Command::cargo_bin("offline_gateway").expect("binary built");
    cmd.args([
        "--config",
        "docs/config.sample.json",
        "--cycles",
        "1000",
        "--threads",
        "2",
    ]);
    cmd.assert().success();
}
