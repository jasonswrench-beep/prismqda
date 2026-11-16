//! Runs the shared core's conformance files through PrismQDA's `.pstat` reader.
//!
//! The core (`specs/pstat/conformance/check_reader.mjs`) makes 33 files, some sound and some that each break one rule of PSTAT-1, and
//! says what a reader must do with each. This test only reads them: it does nothing unless `PRISMQDA_PSTAT_FILES` names the folder those
//! files are in, and then writes `results.txt` there, one line for each file (`01 open`, or `02 refused: the message`).
//! `tools/pstat-check/conformance.mjs` makes the files, runs this, and compares the answers with the core's.

use std::fs;
use std::path::PathBuf;

use prismqda_import::pstat::read_table;

#[test]
fn the_cores_conformance_files_are_opened_and_refused_as_the_specification_says() {
    let Ok(dir) = std::env::var("PRISMQDA_PSTAT_FILES") else { return };
    let dir = PathBuf::from(dir);
    let mut files: Vec<PathBuf> = fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "pstat")).collect();
    files.sort();
    assert!(!files.is_empty(), "no .pstat file in {}", dir.display());
    let mut lines = Vec::new();
    for file in files {
        let number = file.file_stem().unwrap().to_string_lossy().to_string();
        let bytes = fs::read(&file).unwrap();
        lines.push(match read_table(&bytes, "x") {
            Ok(_) => format!("{number} open"),
            Err(e) => format!("{number} refused: {}", e.to_string().replace('\n', " ")),
        });
    }
    fs::write(dir.join("results.txt"), lines.join("\n") + "\n").unwrap();
}
