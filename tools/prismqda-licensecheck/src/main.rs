use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use prismqda_licensecheck::{check_workspaces, manifests_under, third_party_markdown, Verdict};

fn main() -> ExitCode {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let write = std::env::args().any(|a| a == "--write-third-party");

    let rows = match check_workspaces(&manifests_under(&root)) {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut failures = 0;
    for row in &rows {
        match &row.verdict {
            Verdict::Allowed | Verdict::Reviewed(_) => {}
            Verdict::Blocked(why) => {
                failures += 1;
                println!("BLOCKED       {} {}: {why}", row.name, row.version);
            }
            Verdict::NeedsReview(why) => {
                failures += 1;
                println!("NEEDS REVIEW  {} {}: {why}", row.name, row.version);
            }
        }
    }
    println!("{} third-party crates checked, {failures} not accepted", rows.len());

    if write {
        let path = root.join("THIRD-PARTY.md");
        if let Err(e) = fs::write(&path, third_party_markdown(&rows)) {
            eprintln!("error: could not write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("wrote THIRD-PARTY.md");
    }
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
