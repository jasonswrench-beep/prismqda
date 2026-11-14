//! Recordings through the command layer: importing a recording with the captions beside it, linking one to a document later, what the
//! player is answered, and every way it is refused.

use std::fs;

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

const CAPTIONS: &str = "WEBVTT\n\n1\n00:00:00.500 --> 00:00:02.000\n<v Ann>Hello there.\n\n2\n00:00:02.500 --> 00:00:04.000\n<v Bo>Fine, thanks.\n\n3\n00:00:04.500 --> 00:00:06.000\n<v Ann>Good to hear.\n";

fn project(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    a
}

fn recording(dir: &TempDir, name: &str, len: usize) -> std::path::PathBuf {
    let path = dir.path().join(name);
    fs::write(&path, (0..len).map(|i| (i % 253) as u8).collect::<Vec<u8>>()).unwrap();
    path
}

#[test]
fn a_recording_is_imported_through_the_captions_beside_it_and_linked() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let wav = recording(&dir, "interview-1.wav", 4000);
    fs::write(dir.path().join("interview-1.vtt"), CAPTIONS).unwrap();

    let shown = ok(&mut a, "import.inspect", json!({ "path": wav }));
    assert_eq!(shown["media"]["kind"], "audio");
    assert_eq!((shown["media"]["captions"].as_str(), shown["media"]["file_name"].as_str(), shown["media"]["size_bytes"].as_i64()), (Some("interview-1.vtt"), Some("interview-1.wav"), Some(4000)));
    assert_eq!(shown["kind"], "batch", "what comes in is the transcript read from the captions");

    let done = ok(&mut a, "import.run", json!({ "path": wav }));
    let source = done["outcome"]["source_ids"][0].as_i64().unwrap();
    assert_eq!((done["media"]["kind"].as_str(), done["media"]["found"].as_bool(), done["media"]["duration_ms"].as_i64()), (Some("audio"), Some(true), Some(6000)));
    assert_eq!(ok(&mut a, "media.get", json!({ "source_id": source }))["file_name"], "interview-1.wav");
    assert_eq!(ok(&mut a, "media.list", json!({})).as_array().unwrap().len(), 1);

    let units = ok(&mut a, "unit.list", json!({ "source_id": source }));
    let times: Vec<(i64, i64)> = units.as_array().unwrap().iter().map(|u| (u["time_start_ms"].as_i64().unwrap(), u["time_end_ms"].as_i64().unwrap())).collect();
    assert_eq!(times, [(500, 2000), (2500, 4000), (4500, 6000)], "each turn knows when in the recording it is spoken");
}

#[test]
fn a_recording_without_captions_comes_in_as_a_document_of_its_own() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let wav = recording(&dir, "lonely.mp3", 3000);

    // Looking first changes nothing, and says it is a recording with no captions.
    let shown = ok(&mut a, "import.inspect", json!({ "path": wav }));
    assert_eq!(shown["kind"], "batch");
    assert_eq!((shown["media"]["kind"].as_str(), shown["media"]["captions"].clone(), shown["media"]["stands_alone"].as_bool()), (Some("audio"), Value::Null, Some(true)));
    assert_eq!(shown["media"]["size_bytes"], 3000);
    assert_eq!(shown["duplicate_of"], Value::Null);
    assert_eq!(ok(&mut a, "source.list", json!({})).as_array().unwrap().len(), 0, "inspecting imported nothing");

    let done = ok(&mut a, "import.run", json!({ "path": wav, "name": "Interview 9", "folder": "Audio" }));
    let source = done["outcome"]["source_ids"][0].as_i64().unwrap();
    assert_eq!((done["media"]["stands_alone"].as_bool(), done["media"]["kind"].as_str(), done["media"]["found"].as_bool()), (Some(true), Some("audio"), Some(true)));
    let list = ok(&mut a, "source.list", json!({}));
    let row = &list.as_array().unwrap()[0];
    assert_eq!((row["name"].as_str(), row["folder"].as_str(), row["source_type"].as_str()), (Some("Interview 9"), Some("Audio"), Some("other")));
    assert_eq!((row["recording"]["stands_alone"].as_bool(), row["recording"]["file_name"].as_str()), (Some(true), Some("lonely.mp3")));
    let detail = ok(&mut a, "source.get", json!({ "id": source }));
    assert!(detail["text"].as_str().unwrap().starts_with("Audio recording lonely.mp3, 3 KB."), "{}", detail["text"]);

    // The same recording is not imported twice by mistake; saying so on purpose is allowed.
    let again = err(&mut a, "import.run", json!({ "path": wav }));
    assert_eq!(again.code, "duplicate", "{}", again.message);
    assert_eq!(ok(&mut a, "import.inspect", json!({ "path": wav }))["duplicate_of"], source);
    ok(&mut a, "import.run", json!({ "path": wav, "force": true }));
    assert_eq!(ok(&mut a, "source.list", json!({})).as_array().unwrap().len(), 2);

    assert_eq!(err(&mut a, "import.run", json!({ "path": "relative.mp3" })).code, "invalid");
    assert_eq!(err(&mut a, "import.run", json!({ "path": dir.path().join("missing.mp3") })).code, "io");
}

#[test]
fn a_stretch_of_a_recording_is_coded_listed_moved_and_taken_away_through_the_commands() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let wav = recording(&dir, "talk.wav", 3000);
    let source = ok(&mut a, "import.run", json!({ "path": wav }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let code = ok(&mut a, "code.add", json!({ "name": "Humor" }))["id"].as_i64().unwrap();

    let added = ok(&mut a, "region.add", json!({ "source_id": source, "shape": "span", "start_ms": 12_000, "end_ms": 45_000, "code_id": code }));
    let id = added["id"].as_i64().unwrap();
    assert_eq!(added["coding_id"], -id);
    assert_eq!((added["region"]["shape"].as_str(), added["region"]["start_ms"].as_i64(), added["region"]["end_ms"].as_i64()), (Some("span"), Some(12_000), Some(45_000)));
    assert_eq!(added["region"]["text"], "Recording, 0:12 to 0:45 (0:33 long)");
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": source })).as_array().unwrap().len(), 1);

    let moved = ok(&mut a, "region.reshape", json!({ "id": id, "shape": "span", "start_ms": 10_000, "end_ms": 20_000 }));
    assert_eq!((moved["start_ms"].as_i64(), moved["end_ms"].as_i64()), (Some(10_000), Some(20_000)));

    // A bad time is told in words and nothing is stored.
    let e = err(&mut a, "region.add", json!({ "source_id": source, "shape": "span", "start_ms": 5000, "end_ms": 5000, "code_id": code }));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("end after it starts"), "{}", e.message);
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": source })).as_array().unwrap().len(), 1);

    // The segment list tells it as a clip, and a document with no recording refuses one.
    let segments = ok(&mut a, "segments.list", json!({}));
    let clip = segments["segments"].as_array().unwrap().iter().find(|s| s["kind"] == "clip").expect("a clip");
    assert_eq!((clip["region"]["start_ms"].as_i64(), clip["page"].clone()), (Some(10_000), Value::Null));
    let notes_file = dir.path().join("notes.txt");
    fs::write(&notes_file, "Some words.").unwrap();
    let notes = ok(&mut a, "import.run", json!({ "path": notes_file }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let no = err(&mut a, "region.add", json!({ "source_id": notes, "shape": "span", "start_ms": 0, "end_ms": 1000, "code_id": code }));
    assert!(no.message.contains("no recording linked"), "{}", no.message);

    ok(&mut a, "coding.remove", json!({ "id": -id }));
    assert!(ok(&mut a, "region.list", json!({ "source_id": source })).as_array().unwrap().is_empty());
}

#[test]
fn a_recording_can_be_linked_to_a_transcript_afterwards_taken_away_and_undone() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let captions = dir.path().join("talk.srt");
    fs::write(&captions, "1\n00:00:01,000 --> 00:00:03,000\nFirst words.\n\n2\n00:00:03,500 --> 00:00:05,000\nMore words.\n").unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": captions }))["outcome"]["source_ids"][0].as_i64().unwrap();
    assert_eq!(ok(&mut a, "media.get", json!({ "source_id": source })), Value::Null, "a transcript starts with no recording");

    let video = recording(&dir, "talk.MP4", 300);
    let linked = ok(&mut a, "media.link", json!({ "source_id": source, "path": video }));
    assert_eq!((linked["kind"].as_str(), linked["duration_ms"].as_i64()), (Some("video"), Some(5000)));
    assert_eq!(err(&mut a, "media.link", json!({ "source_id": source, "path": dir.path().join("notes.txt") })).code, "invalid");
    assert_eq!(err(&mut a, "media.link", json!({ "source_id": 9999, "path": video })).code, "not_found");
    assert_eq!(err(&mut a, "media.link", json!({ "source_id": source })).code, "bad_args");

    ok(&mut a, "media.unlink", json!({ "source_id": source }));
    assert_eq!(ok(&mut a, "media.get", json!({ "source_id": source })), Value::Null);
    assert_eq!(err(&mut a, "media.unlink", json!({ "source_id": source })).code, "invalid");
    ok(&mut a, "undo", json!({}));
    assert_eq!(ok(&mut a, "media.get", json!({ "source_id": source }))["file_name"], "talk.MP4", "undo links it again");
}

#[test]
fn a_player_is_answered_with_the_recording_and_parts_of_it() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let wav = recording(&dir, "answer.wav", 6000);
    fs::write(dir.path().join("answer.vtt"), CAPTIONS).unwrap();
    let source = ok(&mut a, "import.run", json!({ "path": wav }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let bytes = fs::read(&wav).unwrap();

    let whole = a.media_reply(source, None);
    assert_eq!((whole.status, whole.content_range.as_deref(), whole.body.len(), whole.content_type.as_str()), (200, None, 6000, "audio/wav"));
    let part = a.media_reply(source, Some("bytes=100-199"));
    assert_eq!((part.status, part.content_range.as_deref()), (206, Some("bytes 100-199/6000")));
    assert_eq!(part.body, bytes[100..200]);
    let from = a.media_reply(source, Some("bytes=5990-"));
    assert_eq!((from.status, from.body.len(), from.content_range.as_deref()), (206, 10, Some("bytes 5990-5999/6000")));
    let tail = a.media_reply(source, Some("bytes=-50"));
    assert_eq!((tail.status, tail.body), (206, bytes[5950..].to_vec()));
    let past = a.media_reply(source, Some("bytes=6000-"));
    assert_eq!((past.status, past.content_range.as_deref()), (416, Some("bytes */6000")));
    assert_eq!(a.media_reply(9999, None).status, 404, "no recording for that document");

    // The recording moves: the player is told, not given something else.
    fs::rename(&wav, dir.path().join("elsewhere.wav")).unwrap();
    let gone = a.media_reply(source, Some("bytes=0-9"));
    assert_eq!(gone.status, 404);
    assert!(String::from_utf8_lossy(&gone.body).contains("link it again"), "{}", String::from_utf8_lossy(&gone.body));
    assert_eq!(ok(&mut a, "media.get", json!({ "source_id": source }))["found"], false);
    ok(&mut a, "media.link", json!({ "source_id": source, "path": dir.path().join("elsewhere.wav") }));
    assert_eq!(a.media_reply(source, Some("bytes=0-9")).status, 206, "linking it again from its new place is the way back");
}

#[test]
fn with_no_project_open_a_player_is_told_so() {
    let mut a = Api::new("Ada").without_backup_on_open();
    assert_eq!(a.media_reply(1, None).status, 404);
}

// ---------------------------------------------------------------- a coded stretch saved as a file (decision 182)

/// A tiny WAV (8000 Hz, one channel, 16 bits) of `samples` samples, as base 64.
fn wav_base64(samples: usize) -> String {
    let data_len = (samples * 2) as u32;
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&[1, 0, 1, 0]);
    b.extend_from_slice(&8000u32.to_le_bytes());
    b.extend_from_slice(&16000u32.to_le_bytes());
    b.extend_from_slice(&[2, 0, 16, 0]);
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    b.extend(std::iter::repeat_n(0u8, samples * 2));
    prismqda_export::base64::encode(&b)
}

#[test]
fn a_clip_cut_by_the_window_is_saved_as_a_wav_file_and_nothing_else_is() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let saved = ok(&mut a, "export.clip", json!({ "path": dir.path().join("Interview 1 0-02 to 0-05"), "data": wav_base64(24000) }));
    let path = saved["path"].as_str().unwrap();
    assert!(path.ends_with("Interview 1 0-02 to 0-05.wav"), "the ending is added: {path}");
    let bytes = fs::read(path).unwrap();
    assert_eq!((&bytes[0..4], &bytes[8..12], bytes.len()), (&b"RIFF"[..], &b"WAVE"[..], 44 + 48000), "the file is what the window sent");
    assert!(saved.get("names_not_replaced").is_none(), "no list of names is applied to exports");

    // A file is never replaced unless it was asked for, and what is not a WAV is refused.
    assert_eq!(err(&mut a, "export.clip", json!({ "path": path, "data": wav_base64(10) })).code, "already_exists");
    assert_eq!(fs::read(path).unwrap().len(), 44 + 48000);
    let png = prismqda_export::base64::encode(b"\x89PNG\r\n\x1a\n and so on, long enough to be a file");
    assert_eq!(err(&mut a, "export.clip", json!({ "path": dir.path().join("nope"), "data": png })).code, "invalid");
    assert_eq!(err(&mut a, "export.clip", json!({ "path": dir.path().join("nope"), "data": "***" })).code, "invalid");
    assert!(!dir.path().join("nope.wav").exists());

    // While a list of names is applied to what is exported, the sound cannot be changed, and the person is told.
    ok(&mut a, "deid.add", json!({ "terms": ["Sarah"], "replacement": "Participant 1" }));
    ok(&mut a, "deid.set_exports", json!({ "on": true }));
    let told = ok(&mut a, "export.clip", json!({ "path": dir.path().join("second"), "data": wav_base64(100) }));
    assert_eq!(told["names_not_replaced"], true);
}
