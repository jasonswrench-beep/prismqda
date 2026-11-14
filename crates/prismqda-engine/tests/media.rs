//! Recordings linked to documents: what is kept (where the file is, not the file), what is refused, how a player is given parts
//! of a recording, what happens when the file moves or changes, undo, and the time a unit is spoken.

use std::fs;
use std::path::PathBuf;

use prismqda_engine::{
    media_kind, mime_for, AppliesTo, AttributeSpec, ImportBatch, ImportedSource, ImportedUnit, Level, NewProject, NewSource, OpenOptions, Project, SourceType, UnitKind, MAX_CHUNK,
};
use tempfile::TempDir;

fn project(dir: &TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

fn recording(dir: &TempDir, name: &str, len: usize) -> PathBuf {
    let path = dir.path().join(name);
    let bytes: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
    fs::write(&path, bytes).unwrap();
    path
}

fn interview(p: &mut Project) -> i64 {
    p.add_source(&NewSource::new("Interview", SourceType::Transcript, "Ann: Hello there.\nBo: Fine, thanks.")).unwrap()
}

#[test]
fn a_recording_is_linked_by_where_it_is_and_what_kind_it_is() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    let path = recording(&dir, "interview-1.MP3", 3000);

    let row = p.link_media(src, path.to_str().unwrap()).unwrap();
    assert_eq!((row.kind.as_str(), row.file_name.as_str(), row.size_bytes), ("audio", "interview-1.MP3", 3000));
    assert!(row.found && row.unchanged);
    assert_eq!(row.duration_ms, None, "this transcript has no times, so the length is not known");
    assert_eq!(p.get_media(src).unwrap().unwrap(), row);
    assert_eq!(p.list_media().unwrap().len(), 1);
    assert_eq!(p.get_media(9999).unwrap(), None);

    // The project holds the path, not the recording: the file is where it was and no copy was made.
    assert_eq!(fs::read_dir(dir.path()).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".MP3")).count(), 1);

    assert_eq!((media_kind("a.wav"), media_kind("a.MP4"), media_kind("a.txt"), media_kind("noext")), (Some("audio"), Some("video"), None, None));
    assert_eq!((mime_for("a.mp3"), mime_for("a.webm"), mime_for("a.xyz")), ("audio/mpeg", "video/webm", "application/octet-stream"));
}

#[test]
fn what_cannot_be_a_recording_is_refused_in_words() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    let good = recording(&dir, "ok.wav", 100);
    let text = dir.path().join("notes.txt");
    fs::write(&text, "words").unwrap();
    let empty = dir.path().join("empty.wav");
    fs::write(&empty, "").unwrap();

    let why = |r: prismqda_engine::Result<prismqda_engine::MediaRow>| r.unwrap_err().to_string();
    assert!(why(p.link_media(src, "ok.wav")).contains("full path"));
    assert!(why(p.link_media(src, text.to_str().unwrap())).contains("MP3, WAV"));
    assert!(why(p.link_media(src, empty.to_str().unwrap())).contains("empty"));
    assert!(why(p.link_media(src, dir.path().join("gone.wav").to_str().unwrap())).contains("cannot be opened"));
    assert!(why(p.link_media(src, dir.path().to_str().unwrap())).contains("MP3, WAV"), "a folder has no recording ending");
    assert!(p.link_media(9999, good.to_str().unwrap()).is_err(), "there is no such document");
    assert!(p.get_media(src).unwrap().is_none(), "every refusal left the document without a recording");
}

#[test]
fn a_player_is_given_parts_of_the_recording_and_never_more_than_the_limit() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    let len = MAX_CHUNK as usize + 1_000_000;
    let path = recording(&dir, "long.wav", len);
    p.link_media(src, path.to_str().unwrap()).unwrap();
    let all = fs::read(&path).unwrap();

    let a = p.read_media(src, 0, Some(9)).unwrap();
    assert_eq!((a.bytes.as_slice(), a.start, a.end, a.total, a.mime), (&all[0..10], 0, 9, len as u64, "audio/wav"));
    let b = p.read_media(src, 1000, Some(1999)).unwrap();
    assert_eq!(b.bytes, all[1000..2000]);
    let whole = p.read_media(src, 0, None).unwrap();
    assert_eq!((whole.bytes.len() as u64, whole.end), (MAX_CHUNK, MAX_CHUNK - 1), "a request for everything is answered in a piece");
    let rest = p.read_media(src, MAX_CHUNK, None).unwrap();
    assert_eq!((rest.bytes.len(), rest.end), (1_000_000, len as u64 - 1));
    assert_eq!(rest.bytes, all[MAX_CHUNK as usize..]);
    let past = p.read_media(src, 0, Some(len as u64 * 2)).unwrap();
    assert_eq!(past.end, MAX_CHUNK - 1, "an end past the file is cut to the file and to the limit");
    assert!(p.read_media(src, len as u64, None).unwrap_err().to_string().contains("nothing from byte"));
    assert!(p.read_media(9999, 0, None).unwrap_err().to_string().contains("no recording"));
}

#[test]
fn a_recording_that_moved_or_changed_is_told_not_guessed() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    let path = recording(&dir, "interview.wav", 5000);
    p.link_media(src, path.to_str().unwrap()).unwrap();

    fs::write(&path, vec![1u8; 4000]).unwrap();
    let changed = p.get_media(src).unwrap().unwrap();
    assert!(changed.found && !changed.unchanged, "it is there, but not the size it was");

    let moved = dir.path().join("moved.wav");
    fs::rename(&path, &moved).unwrap();
    let gone = p.get_media(src).unwrap().unwrap();
    assert!(!gone.found && !gone.unchanged);
    let why = p.read_media(src, 0, None).unwrap_err().to_string();
    assert!(why.contains("not at") && why.contains("link it again"), "{why}");

    // Linking it again from its new place is the way back, and keeps the one link a document has.
    let again = p.link_media(src, moved.to_str().unwrap()).unwrap();
    assert!(again.found && again.unchanged);
    assert_eq!(p.list_media().unwrap().len(), 1);
}

#[test]
fn linking_and_unlinking_can_be_undone() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    let first = recording(&dir, "one.wav", 100);
    let second = recording(&dir, "two.mp4", 200);

    p.link_media(src, first.to_str().unwrap()).unwrap();
    p.link_media(src, second.to_str().unwrap()).unwrap();
    assert_eq!(p.get_media(src).unwrap().unwrap().file_name, "two.mp4");
    p.undo().unwrap();
    let back = p.get_media(src).unwrap().unwrap();
    assert_eq!((back.file_name.as_str(), back.kind.as_str(), back.size_bytes), ("one.wav", "audio", 100), "the earlier recording is linked again");
    p.undo().unwrap();
    assert!(p.get_media(src).unwrap().is_none(), "and with another undo, none");

    p.link_media(src, first.to_str().unwrap()).unwrap();
    p.unlink_media(src).unwrap();
    assert!(p.get_media(src).unwrap().is_none());
    assert!(p.unlink_media(src).is_err(), "there is nothing to take away now");
    p.undo().unwrap();
    assert_eq!(p.get_media(src).unwrap().unwrap().file_name, "one.wav", "undoing the unlink links it again");
}

#[test]
fn a_unit_knows_when_in_the_recording_it_is_spoken_and_the_length_follows() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let text = "Ann: Hello there.\nBo: Fine, thanks.\n";
    let turn = |words: &str, label: &str, from: i64, to: i64| {
        let start = text.find(words).unwrap() as i64;
        ImportedUnit { speaker: Some(label.to_string()), time_ms: Some((from, to)), ..ImportedUnit::new(start, start + words.len() as i64, UnitKind::SpeakerTurn) }
    };
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Talk".into(),
            source_type: SourceType::Transcript,
            folder: String::new(),
            text: text.into(),
            runs: vec![],
            units: vec![turn("Hello there.", "Ann", 1000, 3200), turn("Fine, thanks.", "Bo", 3500, 6100)],
            default_unit_kind: UnitKind::SpeakerTurn,
            unit_attributes: vec![AttributeSpec::new("speaker", AppliesTo::Unit, Level::Nominal)],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    };
    let src = p.apply_import(&batch).unwrap().source_ids[0];
    let units = p.list_units(src).unwrap();
    assert_eq!(units.iter().map(|u| (u.time_start_ms, u.time_end_ms)).collect::<Vec<_>>(), [(Some(1000), Some(3200)), (Some(3500), Some(6100))]);

    let path = recording(&dir, "talk.m4a", 500);
    assert_eq!(p.link_media(src, path.to_str().unwrap()).unwrap().duration_ms, Some(6100), "the recording is as long as the last turn says");

    // A unit with no times says so, rather than 0.
    let plain = interview(&mut p);
    assert!(p.list_units(plain).unwrap().iter().all(|u| u.time_start_ms.is_none() && u.time_end_ms.is_none()));
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// A path that came from inside a project (someone else's, opened or merged) may be a network address. Asking about it connects to that
// server and offers this computer's sign-in hash, so a path that is not on a drive with a letter is never looked at.
// ---------------------------------------------------------------------------------------------------------------------------------------------

#[cfg(windows)]
#[test]
fn a_recording_is_never_linked_from_a_network_address_and_the_words_say_what_to_do() {
    let dir = TempDir::new().unwrap();
    let mut p = project(&dir);
    let src = interview(&mut p);
    for bad in [r"\\example.invalid\s\a.mp3", "//example.invalid/s/a.mp3", r"\\?\UNC\example.invalid\s\a.mp3", r"\\.\pipe\a.mp3", r"\\?\C:\a.mp3"] {
        let message = p.link_media(src, bad).unwrap_err().to_string();
        assert!(message.contains("drive with a letter"), "{bad}: {message}");
        assert!(!message.contains("cannot be opened"), "{bad}: refused before the file was opened: {message}");
    }
    assert!(p.get_media(src).unwrap().is_none());
    assert!(prismqda_engine::recording_fingerprint(r"\\example.invalid\s\a.mp3").unwrap_err().to_string().contains("drive with a letter"));
}

/// The recording at `file` by a network-style name that needs no network (`\\localhost\C$\...`), when this computer reaches it that way.
#[cfg(windows)]
fn through_the_admin_share(file: &std::path::Path) -> Option<String> {
    let text = file.to_string_lossy().to_string();
    let (drive, rest) = (text.chars().next()?, text.get(3..)?);
    let unc = format!(r"\\localhost\{drive}$\{rest}");
    fs::metadata(&unc).ok().map(|_| unc)
}

#[cfg(windows)]
#[test]
fn a_project_whose_recording_row_names_a_network_address_says_not_found_without_asking_the_disk() {
    let dir = TempDir::new().unwrap();
    let real = recording(&dir, "real.wav", 500);
    let (path, src) = {
        let mut p = project(&dir);
        let src = interview(&mut p);
        p.link_media(src, real.to_str().unwrap()).unwrap();
        (dir.path().join("p.pqda"), src)
    };
    // What a sent project could hold: the same row, with the place written as a network address. Where this computer reaches its own disk
    // that way, the file is really there, so only the check can make the answer "not found".
    let alias = through_the_admin_share(&real);
    for hostile in alias.into_iter().chain([r"\\example.invalid\s\real.wav".to_string(), "//example.invalid/s/real.wav".to_string()]) {
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute("UPDATE media SET path = ?1 WHERE source_id = ?2", rusqlite::params![hostile, src]).unwrap();
        }
        let mut o = OpenOptions::new("tester");
        o.backup_on_open = false;
        let p = Project::open(&path, &o).unwrap();
        let row = p.get_media(src).unwrap().unwrap();
        assert!(!row.found && !row.unchanged, "{hostile}: the recording is reported as not there");
        let message = p.read_media(src, 0, None).unwrap_err().to_string();
        assert!(message.contains("is not at"), "{hostile}: {message}");
        assert!(p.list_media().unwrap().iter().all(|m| !m.found));
    }
}
