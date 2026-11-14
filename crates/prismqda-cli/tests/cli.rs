//! End-to-end: drive the real `prismqda` program the way a script would.

use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_prismqda"))
        .args(args)
        .env("PRISMQDA_ACTOR", "scripted")
        .output()
        .expect("the prismqda program should start")
}

fn ok(args: &[&str]) -> String {
    let out = run(args);
    assert!(
        out.status.success(),
        "prismqda {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn fails(args: &[&str]) -> String {
    let out = run(args);
    assert!(!out.status.success(), "prismqda {args:?} should have failed");
    String::from_utf8(out.stderr).unwrap()
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn a_whole_small_study_from_the_command_line() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("study.pqda");
    let text = dir.path().join("interview.txt");
    std::fs::write(&text, "\u{feff}I felt welcome from the first day.\nThe cost was the hard part.").unwrap();
    let (p, t) = (s(&project), s(&text));

    assert!(ok(&["new", p, "--name", "Welcome study"]).contains("created"));
    assert_eq!(ok(&["import-text", p, t, "--type", "transcript"]).trim(), "source 1");
    assert_eq!(ok(&["add-code", p, "Welcome", "--definition", "Feeling accepted"]).trim(), "code 1");
    assert_eq!(ok(&["add-coder", p, "Ada"]).trim(), "coder 1");

    // "I felt welcome" is characters 0..14: the byte-order mark was stripped, so offsets start at "I".
    let coded = ok(&["code", p, "--source", "1", "--start", "0", "--end", "14", "--code", "1", "--coder", "1"]);
    assert!(coded.contains("\"I felt welcome\""), "{coded}");

    let info = ok(&["info", p]);
    assert!(info.contains("Welcome study"), "{info}");
    assert!(info.contains("sources 1  units 0  codes 1  coders 1  codings 1"), "{info}");

    ok(&["annotate", p, "Pilot coding done"]);
    let events = ok(&["events", p]);
    for action in ["project.create", "source.import", "code.add", "coder.add", "coding.add", "annotation"] {
        assert!(events.contains(action), "missing {action} in:\n{events}");
    }
    assert!(events.contains("scripted"), "the actor is recorded");

    ok(&["uncode", p, "1"]);
    assert!(ok(&["info", p]).contains("codings 0"));
    assert!(ok(&["events", p]).contains("coding.remove"));
}

#[test]
fn problems_are_reported_in_plain_words_with_a_failing_exit_code() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("study.pqda");
    let text = dir.path().join("a.txt");
    std::fs::write(&text, "short").unwrap();
    let (p, t) = (s(&project), s(&text));
    ok(&["new", p, "--name", "Study"]);

    assert!(fails(&["new", p, "--name", "Again"]).contains("already exists"));
    assert!(fails(&["info", s(&dir.path().join("missing.pqda"))]).contains("does not exist"));

    ok(&["import-text", p, t]);
    assert!(fails(&["import-text", p, t]).contains("already imported"), "duplicate import is refused");
    ok(&["import-text", p, t, "--force"]);

    ok(&["add-code", p, "Code"]);
    ok(&["add-coder", p, "Ada"]);
    let bad = fails(&["code", p, "--source", "1", "--start", "0", "--end", "99", "--code", "1", "--coder", "1"]);
    assert!(bad.contains("does not fit source 1"), "{bad}");

    let bad_type = fails(&["import-text", p, t, "--type", "novel", "--force"]);
    assert!(bad_type.contains("document"), "the allowed values are listed: {bad_type}");

    let not_text = dir.path().join("binary.txt");
    std::fs::write(&not_text, [0xff, 0xfe, 0x00, 0x80]).unwrap();
    assert!(fails(&["import-text", p, s(&not_text)]).contains("not UTF-8"));
}

#[test]
fn a_model_coder_is_recorded_with_its_model() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("study.pqda");
    let p = s(&project);
    ok(&["new", p, "--name", "Study"]);
    assert_eq!(ok(&["add-coder", p, "Assistant", "--model", "example-model", "--provider", "local"]).trim(), "coder 1");
    let events = ok(&["events", p, "--details"]);
    assert!(events.contains("\"role\":\"model\""), "{events}");
    assert!(events.contains("\"model_name\":\"example-model\""), "{events}");
    assert!(events.contains("\"provider\":\"local\""), "{events}");

    // A person has no model recorded.
    ok(&["add-coder", p, "Ada"]);
    let events = ok(&["events", p, "--details"]);
    assert!(events.contains("\"role\":\"human\""), "{events}");
}

#[test]
fn the_schema_command_prints_the_published_schema() {
    let schema = ok(&["schema"]);
    assert!(schema.contains("CREATE TABLE coding"));
    assert!(schema.contains("CREATE TRIGGER event_no_update"));
}

#[test]
fn a_backup_command_makes_a_copy_beside_the_project() {
    let dir = TempDir::new().unwrap();
    let project = dir.path().join("study.pqda");
    let p = s(&project);
    ok(&["new", p, "--name", "Study"]);
    let out = ok(&["backup", p]);
    assert!(out.starts_with("backup "), "{out}");
    assert_eq!(std::fs::read_dir(dir.path().join("study.backups")).unwrap().count(), 1);
}
