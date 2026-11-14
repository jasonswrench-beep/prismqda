//! The build's license rule (specification 2.2): every dependency must be under a permissive
//! license. GPL, LGPL, AGPL, and non-commercial licenses are blocked; anything else that is not
//! on the allow list is held for review. Both fail the check until a person decides.

use std::path::Path;
use std::process::Command;

/// Licenses accepted without review (SPDX identifiers).
pub const ALLOWED: &[&str] = &[
    "Apache-2.0",
    "MIT",
    "MIT-0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "BSL-1.0",
    "CC0-1.0",
    "Unlicense",
    "Unicode-3.0",
    "Unicode-DFS-2016",
];

/// A dependency a person looked at and accepted although its license is not on the allow list. The match is on the
/// crate's name and its exact license text, so a new version that changes the license is held for review again.
pub struct Reviewed {
    pub name: &'static str,
    pub license: &'static str,
    pub why: &'static str,
}

/// The reviewed exceptions. Every one of them is also written up in docs/DECISIONS.md.
///
/// All five are MPL-2.0, a license that applies to the files of the library itself and not to the program that
/// uses it. They come in with Tauri (the window toolkit the specification chooses), they are used unmodified
/// from crates.io, and the specification blocks GPL, LGPL in static builds, AGPL, and non-commercial licenses, which
/// MPL-2.0 is none of. The installer ships THIRD-PARTY.md, which names each one and where its source is.
pub const REVIEWED: &[Reviewed] = &[
    Reviewed { name: "cssparser", license: "MPL-2.0", why: "CSS parser used by Tauri's HTML handling; used unmodified" },
    Reviewed { name: "cssparser-macros", license: "MPL-2.0", why: "macros for cssparser; used unmodified" },
    Reviewed { name: "dtoa-short", license: "MPL-2.0", why: "number printing used by cssparser; used unmodified" },
    Reviewed { name: "selectors", license: "MPL-2.0", why: "CSS selector matching used by Tauri's HTML handling; used unmodified" },
    Reviewed { name: "option-ext", license: "MPL-2.0", why: "small Option helpers used by Tauri's directory lookup; used unmodified" },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Some way of reading the license expression uses only allowed licenses.
    Allowed,
    /// Every way of reading it involves a license the spec blocks.
    Blocked(String),
    /// Not blocked, but not on the allow list (or no readable license): a person must decide.
    NeedsReview(String),
    /// A person decided, and the decision is in [`REVIEWED`]. Carries the reason.
    Reviewed(String),
}

impl Verdict {
    pub fn passes(&self) -> bool {
        matches!(self, Verdict::Allowed | Verdict::Reviewed(_))
    }
}

/// One dependency and what the rule says about it.
#[derive(Debug, Clone)]
pub struct Row {
    pub name: String,
    pub version: String,
    pub license: String,
    pub verdict: Verdict,
}

fn is_blocked_name(name: &str) -> bool {
    name.starts_with("GPL-")
        || name.starts_with("LGPL-")
        || name.starts_with("AGPL-")
        || name.starts_with("SSPL")
        || name.contains("-NC")
}

/// Judges one SPDX license expression, such as `MIT OR Apache-2.0`.
pub fn judge(license: Option<&str>) -> Verdict {
    let Some(text) = license.map(str::trim).filter(|t| !t.is_empty()) else {
        return Verdict::NeedsReview("no license field".to_string());
    };
    let expr = match spdx::Expression::parse_mode(text, spdx::ParseMode::LAX) {
        Ok(e) => e,
        Err(e) => return Verdict::NeedsReview(format!("cannot read the license expression: {e}")),
    };
    // An OR lets the user pick, so one fully allowed branch is enough; an AND needs both sides.
    if expr.evaluate(|req| req.license.id().is_some_and(|id| ALLOWED.contains(&id.name))) {
        return Verdict::Allowed;
    }
    let blocked: Vec<&str> = expr
        .requirements()
        .filter_map(|r| r.req.license.id())
        .map(|id| id.name)
        .filter(|name| is_blocked_name(name))
        .collect();
    if blocked.is_empty() {
        Verdict::NeedsReview(format!("{text} is not on the allow list"))
    } else {
        Verdict::Blocked(format!("{text} involves {}", blocked.join(", ")))
    }
}

/// The Cargo workspaces under a project root that exist: the engine's, and the desktop app's.
pub fn manifests_under(root: &Path) -> Vec<std::path::PathBuf> {
    ["Cargo.toml", "app/src-tauri/Cargo.toml"].iter().map(|m| root.join(m)).filter(|p| p.is_file()).collect()
}

/// Checks several Cargo workspaces (the engine's and the desktop app's) and merges the rows, one per crate and version.
pub fn check_workspaces(manifest_paths: &[std::path::PathBuf]) -> Result<Vec<Row>, String> {
    let mut rows: Vec<Row> = Vec::new();
    for path in manifest_paths {
        for row in check_workspace(path)? {
            if !rows.iter().any(|r| r.name == row.name && r.version == row.version) {
                rows.push(row);
            }
        }
    }
    rows.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(rows)
}

/// Asks Cargo for every package in the workspace's dependency set and judges each non-workspace one.
pub fn check_workspace(manifest_path: &Path) -> Result<Vec<Row>, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--manifest-path"])
        .arg(manifest_path)
        .output()
        .map_err(|e| format!("could not run cargo: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    rows_from_metadata(&output.stdout)
}

/// Reads `cargo metadata` JSON. Separate from `check_workspace` so it can be tested with fixed input.
pub fn rows_from_metadata(json: &[u8]) -> Result<Vec<Row>, String> {
    let value: serde_json::Value = serde_json::from_slice(json).map_err(|e| format!("bad metadata: {e}"))?;
    let packages = value["packages"].as_array().ok_or("metadata has no package list")?;
    let mut rows: Vec<Row> = packages
        .iter()
        // Packages with no source are the workspace's own (path) crates.
        .filter(|p| !p["source"].is_null())
        .map(|p| {
            let license = p["license"].as_str().map(str::to_string);
            let name = p["name"].as_str().unwrap_or("?").to_string();
            let mut verdict = judge(license.as_deref());
            if matches!(verdict, Verdict::NeedsReview(_)) {
                if let Some(r) = REVIEWED.iter().find(|r| r.name == name && Some(r.license) == license.as_deref()) {
                    verdict = Verdict::Reviewed(r.why.to_string());
                }
            }
            Row {
                name,
                version: p["version"].as_str().unwrap_or("?").to_string(),
                verdict,
                license: license.unwrap_or_else(|| "(none)".to_string()),
            }
        })
        .collect();
    rows.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(rows)
}

/// The text of THIRD-PARTY.md for these rows.
pub fn third_party_markdown(rows: &[Row]) -> String {
    let mut out = String::new();
    out.push_str("# Third-party components\n\n");
    out.push_str("Generated by `cargo run -p prismqda-licensecheck -- --write-third-party`. Do not edit by hand.\n");
    out.push_str("PrismQDA is Apache-2.0. These Rust crates are used to build and test it, each under the license shown.\n\n");
    out.push_str("| Crate | Version | License |\n|---|---|---|\n");
    for r in rows {
        out.push_str(&format!("| {} | {} | {} |\n", r.name, r.version, r.license));
    }
    let reviewed: Vec<&Row> = rows.iter().filter(|r| matches!(r.verdict, Verdict::Reviewed(_))).collect();
    if !reviewed.is_empty() {
        out.push_str("\n## Reviewed exceptions\n\n");
        out.push_str("These crates are under the Mozilla Public License 2.0, which is not on the plain allow list. A person reviewed each one and the decision is recorded in docs/DECISIONS.md. The MPL-2.0 applies to the files of these libraries only, not to PrismQDA. They are used exactly as published, so their source is the published crate at `https://crates.io/crates/<name>/<version>`, and the license text is at https://www.mozilla.org/MPL/2.0/.\n\n");
        out.push_str("| Crate | Version | Why it is accepted |\n|---|---|---|\n");
        for r in reviewed {
            if let Verdict::Reviewed(why) = &r.verdict {
                out.push_str(&format!("| {} | {} | {} |\n", r.name, r.version, why));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(s: &str) -> bool {
        judge(Some(s)).passes()
    }

    #[test]
    fn permissive_licenses_pass() {
        for s in [
            "MIT",
            "Apache-2.0",
            "MIT OR Apache-2.0",
            "MIT/Apache-2.0",
            "Apache-2.0 OR MIT",
            "0BSD OR MIT OR Apache-2.0",
            "Apache-2.0 WITH LLVM-exception",
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            "Zlib",
            "ISC",
            "BSD-3-Clause",
            "Unlicense OR MIT",
        ] {
            assert!(allowed(s), "{s} should pass: {:?}", judge(Some(s)));
        }
    }

    #[test]
    fn copyleft_and_non_commercial_licenses_are_blocked() {
        for s in ["GPL-3.0-only", "GPL-2.0-or-later", "LGPL-2.1-or-later", "AGPL-3.0-only", "CC-BY-NC-4.0", "MIT AND GPL-3.0-only"] {
            assert!(matches!(judge(Some(s)), Verdict::Blocked(_)), "{s} should be blocked: {:?}", judge(Some(s)));
        }
    }

    #[test]
    fn an_or_lets_the_user_avoid_a_blocked_branch_but_an_and_does_not() {
        assert!(allowed("GPL-3.0-only OR MIT"));
        assert!(!allowed("GPL-3.0-only AND MIT"));
    }

    #[test]
    fn unknown_missing_or_unlisted_licenses_wait_for_a_person() {
        assert!(matches!(judge(None), Verdict::NeedsReview(_)));
        assert!(matches!(judge(Some("")), Verdict::NeedsReview(_)));
        assert!(matches!(judge(Some("   ")), Verdict::NeedsReview(_)));
        assert!(matches!(judge(Some("MPL-2.0")), Verdict::NeedsReview(_)));
        assert!(matches!(judge(Some("this is not a license")), Verdict::NeedsReview(_)));
        assert!(matches!(judge(Some("LicenseRef-custom")), Verdict::NeedsReview(_)));
    }

    #[test]
    fn metadata_rows_skip_workspace_crates_and_flag_bad_ones() {
        let json = br#"{"packages":[
            {"name":"ours","version":"0.1.0","source":null,"license":"Apache-2.0"},
            {"name":"good","version":"1.0.0","source":"registry+x","license":"MIT OR Apache-2.0"},
            {"name":"bad","version":"2.0.0","source":"registry+x","license":"GPL-3.0-only"},
            {"name":"nolicense","version":"3.0.0","source":"registry+x","license":null}
        ]}"#;
        let rows = rows_from_metadata(json).unwrap();
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["bad", "good", "nolicense"]);
        assert!(matches!(rows[0].verdict, Verdict::Blocked(_)));
        assert!(rows[1].verdict.passes());
        assert!(matches!(rows[2].verdict, Verdict::NeedsReview(_)));
        assert_eq!(rows[2].license, "(none)");
    }

    #[test]
    fn a_reviewed_exception_passes_only_for_the_exact_license_it_was_reviewed_under() {
        let json = br#"{"packages":[
            {"name":"selectors","version":"0.38.0","source":"registry+x","license":"MPL-2.0"},
            {"name":"selectors","version":"0.99.0","source":"registry+x","license":"MPL-2.0 OR GPL-3.0-only"},
            {"name":"some-other-mpl","version":"1.0.0","source":"registry+x","license":"MPL-2.0"}
        ]}"#;
        let rows = rows_from_metadata(json).unwrap();
        assert!(matches!(rows[0].verdict, Verdict::Reviewed(_)) && rows[0].verdict.passes());
        assert!(!rows[1].verdict.passes(), "a changed license is reviewed again: {:?}", rows[1].verdict);
        assert!(matches!(rows[2].verdict, Verdict::NeedsReview(_)), "an unreviewed MPL crate still waits for a person");
    }

    #[test]
    fn reviewed_exceptions_are_written_up_in_the_third_party_file() {
        let rows = vec![Row { name: "selectors".into(), version: "0.38.0".into(), license: "MPL-2.0".into(), verdict: Verdict::Reviewed("used unmodified".into()) }];
        let md = third_party_markdown(&rows);
        assert!(md.contains("## Reviewed exceptions") && md.contains("| selectors | 0.38.0 | used unmodified |"));
    }

    #[test]
    fn third_party_table_lists_every_row() {
        let rows = vec![Row {
            name: "a".into(),
            version: "1.0.0".into(),
            license: "MIT".into(),
            verdict: Verdict::Allowed,
        }];
        let md = third_party_markdown(&rows);
        assert!(md.contains("| a | 1.0.0 | MIT |"));
        assert!(md.starts_with("# Third-party components"));
    }
}
