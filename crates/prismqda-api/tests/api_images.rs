//! Pictures through the command layer: importing one, coding areas of it, the numbers areas share with words, what is refused, and what
//! a window is answered when it asks for the picture.

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

fn png(w: u32, h: u32, salt: u8) -> Vec<u8> {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 2, 0, 0, 0, salt, salt, salt, salt]);
    b
}

fn project(dir: &TempDir) -> Api {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("p.pqda"), "name": "S" }));
    a
}

fn picture(dir: &TempDir, name: &str, bytes: Vec<u8>) -> String {
    let path = dir.path().join(name);
    fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

#[test]
fn a_picture_is_imported_as_a_document_and_served_as_it_was() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let path = picture(&dir, "Front Door.png", png(640, 480, 7));

    let shown = ok(&mut a, "import.inspect", json!({ "path": path }));
    assert_eq!(shown["kind"], "batch");
    assert_eq!(shown["summary"]["sources"][0]["format"], "image");
    assert!(shown["summary"]["sources"][0]["preview"].as_str().unwrap().contains("640 by 480 pixels"));

    let done = ok(&mut a, "import.run", json!({ "path": path }));
    let id = done["outcome"]["source_ids"][0].as_i64().unwrap();
    let list = ok(&mut a, "source.list", json!({}));
    assert_eq!(list[0]["name"], "Front Door");
    assert_eq!((list[0]["image"]["width"].as_i64(), list[0]["image"]["height"].as_i64(), list[0]["image"]["mime"].as_str()), (Some(640), Some(480), Some("image/png")));
    assert_eq!(ok(&mut a, "image.get", json!({ "source_id": id }))["file_name"], "Front Door.png");
    assert_eq!(ok(&mut a, "image.list", json!({})).as_array().unwrap().len(), 1);
    assert!(ok(&mut a, "source.get", json!({ "id": id }))["text"].as_str().unwrap().contains("Front Door.png"));

    let reply = a.image_reply(id);
    assert_eq!((reply.status, reply.content_type.as_str(), reply.cacheable), (200, "image/png", true));
    assert_eq!(reply.body, png(640, 480, 7), "the bytes are the file's, unchanged");
    assert_eq!(a.image_reply(9999).status, 404);
    let words = ok(&mut a, "import.run", json!({ "path": picture(&dir, "n.txt", b"Some words.".to_vec()) }));
    assert_eq!(a.image_reply(words["outcome"]["source_ids"][0].as_i64().unwrap()).status, 404, "a document of words has no picture");

    // The same file is not imported twice by accident.
    assert_eq!(err(&mut a, "import.run", json!({ "path": path })).code, "duplicate");
}

#[test]
fn a_picture_that_cannot_be_kept_says_what_to_do() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let e = err(&mut a, "import.run", json!({ "path": picture(&dir, "scan.tiff", vec![1, 2, 3]) }));
    assert!(e.message.contains("Save scan.tiff as a PNG or JPEG"), "{}", e.message);
    let e = err(&mut a, "import.run", json!({ "path": picture(&dir, "fake.png", b"words, not a picture".to_vec()) }));
    assert!(e.message.contains("not a PNG"), "{}", e.message);
    let e = err(&mut a, "import.run", json!({ "path": picture(&dir, "wrong.jpg", png(10, 10, 1)) }));
    assert!(e.message.contains("named as jpeg but its bytes are png"), "{}", e.message);
    assert_eq!(ok(&mut a, "source.list", json!({})).as_array().unwrap().len(), 0, "nothing was imported");
}

#[test]
fn areas_are_coded_with_the_commands_words_use_and_a_negative_number_names_them() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let id = ok(&mut a, "import.run", json!({ "path": picture(&dir, "street.png", png(800, 600, 3)) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let joy = ok(&mut a, "code.add", json!({ "name": "Joy" }))["id"].as_i64().unwrap();
    let fear = ok(&mut a, "code.add", json!({ "name": "Fear" }))["id"].as_i64().unwrap();

    let made = ok(&mut a, "region.add", json!({ "source_id": id, "shape": "rectangle", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4, "code_id": joy, "rationale": "a child laughing" }));
    let area = made["id"].as_i64().unwrap();
    assert_eq!(made["coding_id"].as_i64().unwrap(), -area);
    assert_eq!(made["region"]["shape"], "rectangle");
    let poly = ok(&mut a, "region.add", json!({ "source_id": id, "shape": "polygon", "points": [[0.5, 0.5], [0.9, 0.5], [0.7, 0.9]], "code_id": fear }));
    assert_eq!(poly["region"]["points"].as_array().unwrap().len(), 3);

    let listed = ok(&mut a, "region.list", json!({ "source_id": id }));
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0]["rationale"], "a child laughing");
    assert_eq!(ok(&mut a, "source.list", json!({}))[0]["codings"], 2, "the document counts its coded areas as codings");
    assert_eq!(ok(&mut a, "code.list", json!({}))[0]["usage"], 1);

    // Segments list the areas beside the words, each with a negative number and its shape.
    let segs = ok(&mut a, "segments.list", json!({}))["segments"].clone();
    let kinds: Vec<&str> = segs.as_array().unwrap().iter().map(|s| s["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["image", "image"]);
    assert!(segs.as_array().unwrap().iter().all(|s| s["coding_id"].as_i64().unwrap() < 0));
    assert_eq!(segs[0]["region"]["shape"], "rectangle");

    // The commands that take a coding take a coded area by its negative number.
    let neg = -area;
    ok(&mut a, "coding.change_code", json!({ "id": neg, "code_id": fear }));
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": id }))[0]["code_id"].as_i64().unwrap(), fear);
    ok(&mut a, "coding.rationale", json!({ "id": neg, "rationale": "now fear" }));
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": id }))[0]["rationale"], "now fear");
    assert!(err(&mut a, "coding.rating", json!({ "id": neg, "rating": 3 })).message.contains("not a rated code"));
    assert_eq!(err(&mut a, "coding.anchor", json!({ "id": neg, "anchor": true })).code, "invalid");
    let history = ok(&mut a, "coding.history", json!({ "id": neg }));
    assert_eq!(history.as_array().unwrap().iter().map(|h| h["action"].as_str().unwrap()).collect::<Vec<_>>(), ["region.add", "region.change_code", "region.rationale"]);

    let moved = ok(&mut a, "region.reshape", json!({ "id": area, "shape": "rectangle", "x": 0.2, "y": 0.2, "w": 0.3, "h": 0.4 }));
    assert_eq!(moved["x"].as_f64().unwrap(), 0.2);
    assert!(err(&mut a, "region.reshape", json!({ "id": area, "shape": "ellipse", "x": 0.2, "y": 0.2, "w": 0.3, "h": 0.4 })).message.contains("cannot become"));

    ok(&mut a, "coding.remove", json!({ "id": neg }));
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": id })).as_array().unwrap().len(), 1);
    ok(&mut a, "undo", json!({}));
    assert_eq!(ok(&mut a, "region.list", json!({ "source_id": id })).as_array().unwrap().len(), 2, "undo puts the area back");
}

#[test]
fn what_cannot_be_coded_is_refused_with_a_code_the_screen_can_use() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let id = ok(&mut a, "import.run", json!({ "path": picture(&dir, "street.png", png(800, 600, 3)) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let words = ok(&mut a, "import.run", json!({ "path": picture(&dir, "n.txt", b"Some words here.".to_vec()) }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let joy = ok(&mut a, "code.add", json!({ "name": "Joy" }))["id"].as_i64().unwrap();
    assert_eq!(err(&mut a, "region.add", json!({ "source_id": id, "shape": "rectangle", "x": 0.9, "y": 0.2, "w": 0.3, "h": 0.4, "code_id": joy })).code, "invalid");
    assert_eq!(err(&mut a, "region.add", json!({ "source_id": words, "shape": "rectangle", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4, "code_id": joy })).code, "invalid");
    assert_eq!(err(&mut a, "region.add", json!({ "source_id": id, "shape": "star", "code_id": joy })).code, "bad_args");
    assert_eq!(err(&mut a, "region.add", json!({ "source_id": id, "shape": "rectangle", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4, "code_id": 999 })).code, "not_found");

    // A mutually exclusive group says so in the same way it does for words.
    ok(&mut a, "code.update", json!({ "id": joy, "exclusive_group": "Mood" }));
    let calm = ok(&mut a, "code.add", json!({ "name": "Calm" }))["id"].as_i64().unwrap();
    ok(&mut a, "code.update", json!({ "id": calm, "exclusive_group": "Mood" }));
    ok(&mut a, "region.add", json!({ "source_id": id, "shape": "ellipse", "x": 0.1, "y": 0.1, "w": 0.5, "h": 0.5, "code_id": joy }));
    let e = err(&mut a, "region.add", json!({ "source_id": id, "shape": "rectangle", "x": 0.2, "y": 0.2, "w": 0.2, "h": 0.2, "code_id": calm }));
    assert_eq!(e.code, "exclusive_group");

    // A picture cannot stand in for the new version of a document of words.
    let e = err(&mut a, "import.revise", json!({ "source_id": words, "path": picture(&dir, "other.png", png(20, 20, 9)) }));
    assert!(e.message.contains("cannot be the new version"), "{}", e.message);
}

#[test]
fn a_folder_of_photographs_comes_in_as_pictures() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let folder = dir.path().join("photos");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("a.png"), png(100, 100, 1)).unwrap();
    fs::write(folder.join("b.PNG"), png(200, 100, 2)).unwrap();
    fs::write(folder.join("c.tiff"), [1, 2, 3]).unwrap();
    let done = ok(&mut a, "import.folder", json!({ "path": folder }));
    assert_eq!(ok(&mut a, "image.list", json!({})).as_array().unwrap().len(), 2, "{done}");
    assert_eq!(done["skipped"].as_array().map_or(0, |s| s.len()) + done["failed"].as_array().map_or(0, |s| s.len()), 1, "the TIFF is reported, not dropped silently: {done}");
}

/// A JPEG whose camera block says only: Nikon, and the moment given (`YYYY:MM:DD HH:MM:SS`). Different moments make different files.
fn photograph(moment: &str) -> Vec<u8> {
    assert_eq!(moment.len(), 19);
    let mut t = b"MM\0\x2a".to_vec();
    t.extend_from_slice(&8u32.to_be_bytes());
    t.extend_from_slice(&2u16.to_be_bytes());
    for (tag, kind, count, offset) in [(0x010fu16, 2u16, 6u32, 38u32), (0x0132, 2, 20, 44)] {
        t.extend_from_slice(&tag.to_be_bytes());
        t.extend_from_slice(&kind.to_be_bytes());
        t.extend_from_slice(&count.to_be_bytes());
        t.extend_from_slice(&offset.to_be_bytes());
    }
    t.extend_from_slice(&0u32.to_be_bytes());
    t.extend_from_slice(b"Nikon\0");
    t.extend_from_slice(moment.as_bytes());
    t.push(0);
    let mut b = vec![0xff, 0xd8, 0xff, 0xe1];
    b.extend_from_slice(&((2 + 6 + t.len()) as u16).to_be_bytes());
    b.extend_from_slice(b"Exif\0\0");
    b.extend_from_slice(&t);
    b.extend_from_slice(&[0xff, 0xc0, 0, 17, 8, 0, 100, 0, 200, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    b
}

#[test]
fn the_cameras_facts_become_variables_only_when_asked_for_and_the_place_is_told_only_as_present() {
    let dir = TempDir::new().unwrap();
    let mut a = project(&dir);
    let door = picture(&dir, "Door.jpg", photograph("2023:12:25 08:30:00"));

    // Looking at the file says what it holds, and changes nothing.
    let shown = ok(&mut a, "import.inspect", json!({ "path": door }));
    assert_eq!(shown["picture"], json!({ "taken": "2023-12-25 08:30:00", "camera": "Nikon", "place": false }));
    assert_eq!(ok(&mut a, "case.list", json!({})).as_array().unwrap().len(), 0);
    // A picture that says nothing, and a document of words, give no such part.
    assert_eq!(ok(&mut a, "import.inspect", json!({ "path": picture(&dir, "plain.png", png(10, 10, 3)) }))["picture"], json!({ "taken": null, "camera": null, "place": false }));
    assert!(ok(&mut a, "import.inspect", json!({ "path": picture(&dir, "n.txt", b"Words.".to_vec()) }))["picture"].is_null());

    // Imported plainly, no case is made.
    ok(&mut a, "import.run", json!({ "path": door }));
    assert_eq!(ok(&mut a, "case.list", json!({})).as_array().unwrap().len(), 0, "nothing is kept unless asked for");

    // Asked for, the date and the camera become the variables of a case named for the picture.
    let two = picture(&dir, "Window.jpg", photograph("2024:01:02 09:00:00"));
    let done = ok(&mut a, "import.run", json!({ "path": two, "picture_variables": { "when": true, "camera": true } }));
    assert_eq!(done["outcome"]["case_ids"].as_array().unwrap().len(), 1);
    let cases = ok(&mut a, "case.list", json!({}));
    assert_eq!((cases[0]["name"].as_str(), cases[0]["sources"].as_i64()), (Some("Window"), Some(1)));
    let variables = ok(&mut a, "case.variables", json!({}));
    let names: Vec<&str> = variables.as_array().unwrap().iter().map(|v| v["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Camera", "Date taken", "Month taken", "Year taken"], "no place variables, because none was asked for");
    let month = variables.as_array().unwrap().iter().find(|v| v["name"] == "Month taken").unwrap();
    assert_eq!(month["values"], json!([["2024-01", 1]]));
    let date = variables.as_array().unwrap().iter().find(|v| v["name"] == "Date taken").unwrap();
    assert_eq!(date["dates"], 1, "the date reads as a date, so a range of dates can use it");

    // A whole folder of photographs, each with its own case.
    let folder = dir.path().join("trip");
    fs::create_dir(&folder).unwrap();
    fs::write(folder.join("one.jpg"), photograph("2024:02:03 10:00:00")).unwrap();
    fs::write(folder.join("two.jpg"), photograph("2024:03:04 11:00:00")).unwrap();
    fs::write(folder.join("plain.png"), png(30, 30, 5)).unwrap();
    ok(&mut a, "import.folder", json!({ "path": folder, "picture_variables": { "when": true } }));
    let cases = ok(&mut a, "case.list", json!({}));
    assert_eq!(cases.as_array().unwrap().len(), 3, "a case for each photograph that has a date, none for the picture that has none: {cases}");
}
