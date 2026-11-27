//! `phono-build --format-version`: what the release workflow compares a tag's major against.

use std::process::Command;

#[test]
fn prints_the_data_format_and_builds_nothing() {
    let out = Command::new(env!("CARGO_BIN_EXE_phono-build"))
        // A path that doesn't exist: the flag must exit before reading any input.
        .args(["--format-version", "--cmudict", "no/such/cmudict.dict"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        format!("{}\n", phono_core::data::FORMAT_VERSION)
    );
}
