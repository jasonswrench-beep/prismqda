//! The build gate: this test fails if any dependency, in the engine or in the desktop app, has a blocked or
//! unreviewed license, or if THIRD-PARTY.md has fallen out of date.

use std::path::PathBuf;

use prismqda_licensecheck::{check_workspaces, manifests_under, third_party_markdown};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

#[test]
fn every_dependency_has_a_permissive_license() {
    let rows = check_workspaces(&manifests_under(&root())).unwrap();
    assert!(rows.len() > 10, "expected the dependency list to be read, got {} rows", rows.len());
    let bad: Vec<String> =
        rows.iter().filter(|r| !r.verdict.passes()).map(|r| format!("{} {} ({}): {:?}", r.name, r.version, r.license, r.verdict)).collect();
    assert!(bad.is_empty(), "licenses that are not accepted:\n{}", bad.join("\n"));
}

#[test]
fn third_party_file_is_up_to_date() {
    let rows = check_workspaces(&manifests_under(&root())).unwrap();
    let expected = third_party_markdown(&rows);
    let actual = std::fs::read_to_string(root().join("THIRD-PARTY.md")).unwrap_or_default();
    // Compare ignoring line-ending style so a checkout with CRLF does not fail the test.
    assert_eq!(
        actual.replace("\r\n", "\n"),
        expected,
        "THIRD-PARTY.md is out of date: run `cargo run -p prismqda-licensecheck -- --write-third-party`"
    );
}
