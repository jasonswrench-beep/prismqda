//! Crash safety (roadmap M1 gate): kill the process in the middle of a stream of writes, and the project must
//! reopen intact, with every stored coding matched by its audit event and nothing half-written.
//!
//! The test starts a copy of its own executable as the writer (the `PRISMQDA_CRASH_PATH` variable tells that
//! copy what to do), lets it write for a while, and ends exactly that process with a hard kill, the same as a
//! power cut or a crash would. It does this several times over one project, each time after a different delay.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use prismqda_engine::{NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType};
use rusqlite::Connection;
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("crash-test");
    o.backup_on_open = false;
    o
}

/// The writer. Runs only when started by the test below; under a plain `cargo test` it returns at once.
#[test]
fn writer_child() {
    let Some(path) = std::env::var_os("PRISMQDA_CRASH_PATH").map(PathBuf::from) else { return };
    let mut project = Project::open(&path, &opts()).expect("the writer opens the project");
    let source = project.list_sources().unwrap()[0].id;
    let code = project.list_codes().unwrap()[0].id;
    let coder = project.list_coders().unwrap()[0].id;
    let mut n: i64 = 0;
    loop {
        // Alternate between one coding at a time and a batch, so a kill can land inside either kind of transaction.
        let start = (n % 400) * 2;
        if n % 3 == 0 {
            let batch: Vec<NewCoding> = (0..25).map(|k| NewCoding::new(source, (start + k) % 700, (start + k) % 700 + 1, code, coder)).collect();
            project.apply_codings(&batch).unwrap();
        } else {
            project.apply_coding(&NewCoding::new(source, start, start + 3, code, coder)).unwrap();
        }
        n += 1;
    }
}

fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn killing_the_process_mid_write_leaves_a_project_that_reopens_intact() {
    if std::env::var_os("PRISMQDA_CRASH_PATH").is_some() {
        return; // this is the writer copy; its work is in `writer_child`
    }
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("crash.pqda");
    {
        let mut p = Project::create(&path, &NewProject::new("Crash safety"), &opts()).unwrap();
        p.add_coder(&NewCoder::human("Ada")).unwrap();
        p.add_code(&NewCode::new("A")).unwrap();
        p.add_source(&NewSource::new("Doc", SourceType::Document, "x".repeat(800))).unwrap();
    }

    let exe = std::env::current_exe().unwrap();
    let mut last_codings = 0;
    for (round, delay_ms) in [40u64, 90, 150, 25, 300, 60].into_iter().enumerate() {
        let mut child = Command::new(&exe)
            .args(["--exact", "writer_child", "--nocapture", "--test-threads=1"])
            .env("PRISMQDA_CRASH_PATH", &path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start the writer");
        // Give it time to start, then until it has really begun writing (the coding count grows past the last round).
        let started = Instant::now();
        loop {
            std::thread::sleep(Duration::from_millis(20));
            if let Ok(c) = Connection::open(&path) {
                if c.query_row("SELECT count(*) FROM coding", [], |r| r.get::<_, i64>(0)).is_ok_and(|n| n > last_codings) {
                    break;
                }
            }
            assert!(started.elapsed() < Duration::from_secs(60), "round {round}: the writer never wrote anything");
        }
        std::thread::sleep(Duration::from_millis(delay_ms));
        child.kill().expect("end the writer process");
        child.wait().unwrap();

        // Reopen the way a person would, then look inside with a separate connection.
        let project = Project::open(&path, &opts()).unwrap_or_else(|e| panic!("round {round}: the project would not reopen: {e}"));
        drop(project);
        let c = Connection::open(&path).unwrap();
        let integrity: String = c.query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
        assert_eq!(integrity, "ok", "round {round}");
        let codings = count(&c, "SELECT count(*) FROM coding");
        let events = count(&c, "SELECT count(*) FROM event WHERE action = 'coding.add'");
        assert_eq!(codings, events, "round {round}: every coding has its audit event and no event is left without its coding");
        assert!(codings > last_codings, "round {round}: writing made progress");
        let gaps = count(&c, "SELECT (max(id) - min(id) + 1) - count(*) FROM event");
        assert_eq!(gaps, 0, "round {round}: the event log has no holes");
        last_codings = codings;
    }
}
