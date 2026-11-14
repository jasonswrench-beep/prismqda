//! Theme development (decision 167), through the command layer: the overview of the themes with their codes, counts, and quotations, the codes
//! still under no theme, a theme's stage (and Undo), moving a code under a theme, the Themes section of the report, and the thematic map.

use prismqda_api::{Api, ApiError};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ok(api: &mut Api, cmd: &str, args: Value) -> Value {
    api.call(cmd, args).unwrap_or_else(|e| panic!("{cmd} failed: {e}"))
}

fn err(api: &mut Api, cmd: &str, args: Value) -> ApiError {
    api.call(cmd, args).expect_err(&format!("{cmd} should have failed"))
}

struct Study {
    api: Api,
    belonging: i64,
    obstacles: i64,
    welcome: i64,
    weather: i64,
    anchored: i64,
}

/// Two themes, four codes (one under no theme), and passages in two documents.
fn study(dir: &TempDir) -> Study {
    let mut a = Api::new("Ada").without_backup_on_open();
    ok(&mut a, "project.create", json!({ "path": dir.path().join("study.pqda"), "name": "Study" }));
    let one = dir.path().join("one.txt");
    let two = dir.path().join("two.txt");
    std::fs::write(&one, "The market was friendly. Everyone said hello. It rained all day.").unwrap();
    std::fs::write(&two, "The bus was late again. Fares keep rising. A driver waved at me.").unwrap();
    let s1 = ok(&mut a, "import.run", json!({ "path": one }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let s2 = ok(&mut a, "import.run", json!({ "path": two }))["outcome"]["source_ids"][0].as_i64().unwrap();
    let belonging = ok(&mut a, "code.add", json!({ "name": "Belonging", "theme": true, "definition": "Feeling part of the place.\nInclude when: a welcome is described." }))["id"].as_i64().unwrap();
    let obstacles = ok(&mut a, "code.add", json!({ "name": "Obstacles", "theme": true }))["id"].as_i64().unwrap();
    let welcome = ok(&mut a, "code.add", json!({ "name": "Welcome", "parent_id": belonging }))["id"].as_i64().unwrap();
    let greeting = ok(&mut a, "code.add", json!({ "name": "Greeting", "parent_id": belonging }))["id"].as_i64().unwrap();
    let cost = ok(&mut a, "code.add", json!({ "name": "Cost", "parent_id": obstacles }))["id"].as_i64().unwrap();
    let weather = ok(&mut a, "code.add", json!({ "name": "Weather" }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": s1, "start": 0, "end": 24, "code_id": welcome }));
    let anchored = ok(&mut a, "coding.apply", json!({ "source_id": s1, "start": 25, "end": 45, "code_id": greeting }))["id"].as_i64().unwrap();
    ok(&mut a, "coding.apply", json!({ "source_id": s2, "start": 43, "end": 64, "code_id": welcome }));
    ok(&mut a, "coding.apply", json!({ "source_id": s2, "start": 24, "end": 42, "code_id": cost }));
    ok(&mut a, "coding.apply", json!({ "source_id": s1, "start": 46, "end": 64, "code_id": weather }));
    ok(&mut a, "coding.anchor", json!({ "id": anchored, "anchor": true }));
    Study { api: a, belonging, obstacles, welcome, weather, anchored }
}

fn theme<'a>(overview: &'a Value, name: &str) -> &'a Value {
    overview["themes"].as_array().unwrap().iter().find(|t| t["name"] == name).unwrap_or_else(|| panic!("no theme {name} in {overview}"))
}

#[test]
fn the_overview_lists_each_theme_with_its_codes_counts_and_quotations_and_the_codes_still_to_place() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let o = ok(&mut s.api, "theme.overview", json!({}));
    let names: Vec<&str> = o["themes"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Belonging", "Obstacles"], "in the order of the codebook");
    let b = theme(&o, "Belonging");
    assert_eq!(b["stage"], "candidate");
    assert_eq!(b["definition"], "Feeling part of the place.\nInclude when: a welcome is described.");
    let codes: Vec<(&str, i64)> = b["codes"].as_array().unwrap().iter().map(|c| (c["name"].as_str().unwrap(), c["passages"].as_i64().unwrap())).collect();
    assert_eq!(codes, [("Greeting", 1), ("Welcome", 2)], "in the order of the codebook: {b}");
    assert_eq!(b["passages"], 3);
    assert_eq!(b["documents"], 2);
    // the passage marked as an example comes first, then the first passages
    let quotes = b["quotes"].as_array().unwrap();
    assert_eq!(quotes[0]["text"], "Everyone said hello.");
    assert_eq!(quotes[0]["example"], true);
    assert!(quotes.iter().skip(1).all(|q| q["example"] == false));
    assert_eq!(quotes.len(), 3, "up to three");
    assert_eq!(quotes.iter().filter(|q| q["coding_id"] == s.anchored).count(), 1, "no passage twice");
    let ob = theme(&o, "Obstacles");
    assert_eq!(ob["codes"].as_array().unwrap().len(), 1);
    assert_eq!(ob["quotes"][0]["example"], false, "none is marked, so the first passage is shown");
    // the codes that are under no theme are the ones left to place
    let left: Vec<&str> = o["unassigned"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(left, ["Weather"]);
    assert_eq!(o["unassigned"][0]["passages"], 1);
}

#[test]
fn moving_a_code_under_a_theme_places_it_and_a_scope_narrows_the_counts() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "code.move", json!({ "id": s.weather, "parent_id": s.obstacles }));
    let o = ok(&mut s.api, "theme.overview", json!({}));
    assert_eq!(o["unassigned"], json!([]), "nothing is left to place");
    assert_eq!(theme(&o, "Obstacles")["codes"].as_array().unwrap().len(), 2);
    assert_eq!(theme(&o, "Obstacles")["passages"], 2);
    // narrowed to one document, the counts follow
    let sources = ok(&mut s.api, "source.list", json!({}));
    let first = sources[0]["id"].as_i64().unwrap();
    let narrow = ok(&mut s.api, "theme.overview", json!({ "scope": { "source_ids": [first] } }));
    assert_eq!(theme(&narrow, "Belonging")["documents"], 1);
    assert_eq!(theme(&narrow, "Belonging")["passages"], 2, "two of the three passages are in the first document");
    // moving it back out
    ok(&mut s.api, "code.move", json!({ "id": s.weather, "parent_id": null }));
    assert_eq!(ok(&mut s.api, "theme.overview", json!({}))["unassigned"][0]["name"], "Weather");
}

#[test]
fn a_theme_has_a_stage_that_undo_takes_back_and_that_a_code_may_not_be_given_nonsense() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let r = ok(&mut s.api, "code.update", json!({ "id": s.belonging, "theme_stage": "reviewed" }));
    assert_eq!(r["theme_stage"], "reviewed");
    assert_eq!(theme(&ok(&mut s.api, "theme.overview", json!({})), "Belonging")["stage"], "reviewed");
    let bad = err(&mut s.api, "code.update", json!({ "id": s.belonging, "theme_stage": "finished" }));
    assert_eq!(bad.code, "invalid");
    assert!(bad.message.contains("candidate, reviewed, or defined"), "{}", bad.message);
    ok(&mut s.api, "code.update", json!({ "id": s.belonging, "theme_stage": "defined", "definition": "Feeling at home." }));
    ok(&mut s.api, "undo", json!({}));
    let back = theme(&ok(&mut s.api, "theme.overview", json!({})), "Belonging").clone();
    assert_eq!(back["stage"], "reviewed", "Undo took the stage back with the definition");
    assert_eq!(back["definition"], "Feeling part of the place.\nInclude when: a welcome is described.");
    // a code that is not a theme can carry the field and ignores it
    let listed = ok(&mut s.api, "code.list", json!({}));
    assert_eq!(listed.as_array().unwrap().iter().find(|c| c["id"] == s.welcome).unwrap()["theme_stage"], "candidate");
}

#[test]
fn the_themes_section_of_the_report_gives_each_theme_its_stage_codes_and_quotations() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    ok(&mut s.api, "code.update", json!({ "id": s.belonging, "theme_stage": "reviewed" }));
    let out = ok(&mut s.api, "export.report", json!({ "path": dir.path().join("themes"), "sections": ["themes"] }));
    let text = docx_text(&std::fs::read(out["path"].as_str().unwrap()).unwrap());
    assert!(text.contains("Belonging (reviewed)") && text.contains("Obstacles (candidate)"), "{text}");
    assert!(text.contains("2 themes in all: 1 candidate, 1 reviewed, 0 defined"), "{text}");
    assert!(text.contains("Feeling part of the place."), "{text}");
    assert!(text.contains("Include when: a welcome is described."), "{text}");
    assert!(text.contains("Everyone said hello.") && text.contains("marked as an example"), "{text}");
    assert!(text.contains("Codes under no theme") && text.contains("Weather"), "{text}");
    assert!(text.contains("No definition has been written for this theme yet."), "Obstacles has none: {text}");
}

#[test]
fn a_thematic_map_is_made_from_the_themes_and_the_codes_under_them() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let r = ok(&mut s.api, "theme.map", json!({}));
    let map = ok(&mut s.api, "map.get", json!({ "id": r["map_id"] }));
    assert_eq!(map["name"], "Thematic map");
    assert_eq!(map["nodes"].as_array().unwrap().len(), 5, "two themes and the three codes under them, not Weather");
    assert_eq!(map["links"].as_array().unwrap().len(), 3, "each code linked to its theme: {map}");
    let again = ok(&mut s.api, "theme.map", json!({}));
    assert_eq!(ok(&mut s.api, "map.get", json!({ "id": again["map_id"] }))["name"], "Thematic map (2)", "a second map does not replace the first");
    // with no theme there is nothing to draw
    let dir2 = TempDir::new().unwrap();
    let mut bare = Api::new("Ada").without_backup_on_open();
    ok(&mut bare, "project.create", json!({ "path": dir2.path().join("p.pqda"), "name": "Bare" }));
    let e = err(&mut bare, "theme.map", json!({}));
    assert_eq!(e.code, "invalid");
    assert!(e.message.contains("no theme yet"), "{}", e.message);
    let _ = s.obstacles;
}

/// The words of a .docx, joined, enough to look for sentences.
fn docx_text(bytes: &[u8]) -> String {
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn a_passage_coded_twice_under_a_theme_is_one_passage() {
    let dir = TempDir::new().unwrap();
    let mut s = study(&dir);
    let sources = ok(&mut s.api, "source.list", json!({}));
    let first = sources[0]["id"].as_i64().unwrap();
    // the same words as the first Welcome passage, with the other code of the theme
    let greeting = ok(&mut s.api, "code.list", json!({})).as_array().unwrap().iter().find(|c| c["name"] == "Greeting").unwrap()["id"].as_i64().unwrap();
    ok(&mut s.api, "coding.apply", json!({ "source_id": first, "start": 0, "end": 24, "code_id": greeting }));
    let o = ok(&mut s.api, "theme.overview", json!({}));
    assert_eq!(theme(&o, "Belonging")["passages"], 3, "four codings on three passages: {o}");
}
