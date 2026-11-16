//! The case table for PrismStatistics (PSTAT-1 section 7.3): a small project whose every figure is worked out by hand, exported, unzipped
//! and read back as JSON. The same files are checked against the shared core's own checker by `tools/pstat-check` (it needs Node and the
//! core's package), which is the independent check; these tests are the ones that run with every build.
//!
//!   cases (by id):   Maria            Tomas             Priya (no document)
//!   Age group:       2                3                 -99        ordinal, labels 1 2 3, missing code -99
//!   Years in post:   12               3                 (none)     ratio
//!   Visit date:      2026-03-04       3/4/2026          (none)     date; the second is not year-month-day
//!   Site:            North            South             North      nominal words
//!   Staff id:        007              12                (none)     nominal, but "007" would meet 7 as a number
//!   Age Group:       a                b                 c          a second attribute whose variable name collides
//!
//! Codes: Belonging (a theme) > Welcome, and Cost; Maria's document has Welcome twice and Cost once, Tomas's has Cost once, and a code
//! that was retired is left out.

use std::io::Read;

use prismqda_engine::{
    AppliesTo, AttributeSpec, CaseMember, DeidRule, ImportBatch, ImportedCase, ImportedSource, Level, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, Scrubber, SourceType,
    UnitKind,
};
use prismqda_export::{attribute_names, export_case_table, scrub_scope, PstatOptions};
use serde_json::{json, Value};
use tempfile::TempDir;

fn source(name: &str) -> ImportedSource {
    ImportedSource {
        name: name.to_string(),
        source_type: SourceType::Document,
        folder: String::new(),
        text: "Warm welcome here. Fees were a worry. Welcome again.".to_string(),
        runs: vec![],
        units: vec![],
        default_unit_kind: UnitKind::Sentence,
        unit_attributes: vec![],
        original_file_hash: None,
        settings: Value::Null,
        warnings: vec![],
    }
}

struct World {
    p: Project,
    _dir: TempDir,
    welcome: i64,
    belonging: i64,
    cost: i64,
}

fn world() -> World {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("Caregiver interviews"), &o).unwrap();
    let mut age = AttributeSpec::new("Age group", AppliesTo::Case, Level::Ordinal);
    age.value_labels.insert("1".into(), "Under 30".into());
    age.value_labels.insert("2".into(), "30 to 49".into());
    age.value_labels.insert("3".into(), "50 or over".into());
    age.missing_codes = vec!["-99".into()];
    let batch = ImportBatch {
        sources: vec![source("Maria"), source("Tomas")],
        cases: vec![
            ImportedCase {
                name: "Maria".into(),
                attrs: vec![
                    ("Age group".into(), "2".into()),
                    ("Years in post".into(), "12".into()),
                    ("Visit date".into(), "2026-03-04".into()),
                    ("Site".into(), "North".into()),
                    ("Staff id".into(), "007".into()),
                    ("Age Group".into(), "a".into()),
                ],
                members: vec![CaseMember { source: 0, unit: None }],
            },
            ImportedCase {
                name: "Tomas".into(),
                attrs: vec![
                    ("Age group".into(), "3".into()),
                    ("Years in post".into(), "3".into()),
                    ("Visit date".into(), "3/4/2026".into()),
                    ("Site".into(), "South".into()),
                    ("Staff id".into(), "12".into()),
                    ("Age Group".into(), "b".into()),
                ],
                members: vec![CaseMember { source: 1, unit: None }],
            },
            ImportedCase {
                name: "Priya".into(),
                attrs: vec![("Age group".into(), "-99".into()), ("Site".into(), "North".into()), ("Age Group".into(), "c".into())],
                members: vec![],
            },
        ],
        images: Vec::new(),
        pdfs: Vec::new(),
        case_attributes: vec![
            age,
            AttributeSpec::new("Years in post", AppliesTo::Case, Level::Ratio),
            AttributeSpec::new("Visit date", AppliesTo::Case, Level::Date),
            AttributeSpec::new("Site", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Staff id", AppliesTo::Case, Level::Nominal),
            AttributeSpec::new("Age Group", AppliesTo::Case, Level::Nominal),
        ],
    };
    let out = p.apply_import(&batch).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let mut theme = NewCode::new("Belonging");
    theme.theme = true;
    let belonging = p.add_code(&theme).unwrap();
    let mut welcome_code = NewCode::new("Welcome");
    welcome_code.parent_id = Some(belonging);
    let welcome = p.add_code(&welcome_code).unwrap();
    let cost = p.add_code(&NewCode::new("Cost")).unwrap();
    let retired = p.add_code(&NewCode::new("Old idea")).unwrap();
    p.retire_code(retired).unwrap();
    // Maria's document: Welcome on two sentences, Cost on one. Tomas's: Cost on one.
    p.apply_coding(&NewCoding::new(out.source_ids[0], 0, 18, welcome, ada)).unwrap();
    p.apply_coding(&NewCoding::new(out.source_ids[0], 32, 51, welcome, ada)).unwrap();
    p.apply_coding(&NewCoding::new(out.source_ids[0], 19, 31, cost, ada)).unwrap();
    p.apply_coding(&NewCoding::new(out.source_ids[1], 19, 31, cost, ada)).unwrap();
    World { p, _dir: dir, welcome, belonging, cost }
}

fn options() -> PstatOptions {
    PstatOptions { leave_out_names: false, include_descendants: true, app_version: "PrismQDA 0.1.0-beta.2".into() }
}

/// The two entries of a file, parsed.
fn open(bytes: &[u8]) -> (Value, Value, Vec<String>) {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
    let mut read = |name: &str| -> Value {
        let mut text = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut text).unwrap();
        assert!(!text.starts_with('\u{feff}'), "no byte-order mark");
        serde_json::from_str(&text).unwrap()
    };
    let project = read("project.json");
    let part = read("case-table.json");
    (project, part, names)
}

fn variable<'a>(project: &'a Value, name: &str) -> (usize, &'a Value) {
    let vars = project["dataset"]["variables"].as_array().unwrap();
    let at = vars.iter().position(|v| v["name"] == name).unwrap_or_else(|| panic!("no variable {name}: {:?}", vars.iter().map(|v| v["name"].clone()).collect::<Vec<_>>()));
    (at, &vars[at])
}

fn values(project: &Value, name: &str) -> Value {
    let (at, _) = variable(project, name);
    project["dataset"]["columns"][at]["values"].clone()
}

#[test]
fn the_file_holds_exactly_the_two_entries_and_the_profile_names() {
    let w = world();
    let (bytes, report) = export_case_table(&w.p, &options()).unwrap();
    let (project, part, names) = open(&bytes);
    assert_eq!(names, ["project.json", "case-table.json"], "no other entry (rule Z2)");
    assert_eq!(project["format"], "PrismStatistics project");
    assert_eq!(project["version"], 1);
    assert_eq!(project["app_version"], "PrismQDA 0.1.0-beta.2");
    assert_eq!(project["profile"], "PrismQDA case table 1");
    assert_eq!(project["operations"], serde_json::json!([]));
    assert_eq!(project["results"], serde_json::json!([]));
    assert!(project.get("report").is_none() && project.get("data_settings").is_none(), "only PrismStatistics writes those");
    assert_eq!(project["dataset"]["name"], "Caregiver interviews");
    assert_eq!(project["source"]["file_name"], "Caregiver interviews case table");
    assert_eq!(project["source"]["report"]["rows"], 3);
    assert_eq!(project["source"]["report"]["variables"], report.variables);
    assert_eq!(project["source"]["report"]["short_rows"], 0);
    // The part starts with `part` and `version`, in that order.
    let first_keys: Vec<&String> = part.as_object().unwrap().keys().take(2).collect();
    assert_eq!(part["part"], "PrismQDA case table");
    assert_eq!(part["version"], 1);
    let _ = first_keys; // serde_json sorts keys when it reads; the order in the file is checked on the raw text below
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    let mut raw = String::new();
    zip.by_name("case-table.json").unwrap().read_to_string(&mut raw).unwrap();
    assert!(raw.starts_with(r#"{"part":"PrismQDA case table","version":1,"#), "{raw}");
    assert_eq!(part["project"]["name"], "Caregiver interviews");
    assert_eq!(part["project"]["id"].as_str().unwrap().len(), 32);
    assert!(part["exported"].as_str().unwrap().ends_with('Z') && part["exported"].as_str().unwrap().len() == 20);
}

#[test]
fn every_case_is_a_row_in_the_order_of_its_id_and_the_first_two_variables_are_the_profile_s() {
    let w = world();
    let (bytes, _) = export_case_table(&w.p, &options()).unwrap();
    let (project, part, _) = open(&bytes);
    let vars = project["dataset"]["variables"].as_array().unwrap();
    assert_eq!((vars[0]["name"].as_str().unwrap(), vars[0]["label"].as_str().unwrap(), vars[0]["measure"].as_str().unwrap()), ("case_id", "Case ID", "nominal"));
    assert_eq!((vars[1]["name"].as_str().unwrap(), vars[1]["label"].as_str().unwrap()), ("case_name", "Case name"));
    assert_eq!(vars[0]["id"], 1);
    assert_eq!(vars[1]["id"], 2);
    let ids: Vec<i64> = values(&project, "case_id").as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(ids.len(), 3);
    assert!(ids.windows(2).all(|p| p[0] < p[1]), "{ids:?}");
    assert_eq!(values(&project, "case_name"), json!(["Maria", "Tomas", "Priya"]));
    assert_eq!(part["case_id_variable"], 1);
    assert_eq!(part["case_name_variable"], 2);
    // Ids are numbered from 1 with no gap, and next_id is above them all.
    let all: Vec<i64> = vars.iter().map(|v| v["id"].as_i64().unwrap()).collect();
    assert_eq!(all, (1..=vars.len() as i64).collect::<Vec<_>>());
    assert_eq!(project["dataset"]["next_id"], vars.len() as i64 + 1);
}

#[test]
fn leaving_the_names_out_leaves_out_the_name_variable_and_frees_its_name_for_an_attribute() {
    let w = world();
    let mut o = options();
    o.leave_out_names = true;
    let (bytes, report) = export_case_table(&w.p, &o).unwrap();
    let (project, part, _) = open(&bytes);
    assert!(project["dataset"]["variables"].as_array().unwrap().iter().all(|v| v["name"] != "case_name"));
    assert_eq!(part["case_name_variable"], Value::Null);
    assert!(report.notes.iter().any(|n| n.contains("names were left out")), "{:?}", report.notes);
    assert!(!String::from_utf8_lossy(&bytes).contains("Maria"), "no case name is written (the file is compressed, so this only guards the plain case)");
    let (project_text, part_text) = {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
        let mut a = String::new();
        zip.by_name("project.json").unwrap().read_to_string(&mut a).unwrap();
        let mut b = String::new();
        zip.by_name("case-table.json").unwrap().read_to_string(&mut b).unwrap();
        (a, b)
    };
    assert!(!project_text.contains("Maria") && !project_text.contains("Tomas") && !part_text.contains("Maria"));
}

#[test]
fn an_attribute_becomes_a_variable_with_its_level_labels_and_missing_codes() {
    let w = world();
    let (bytes, report) = export_case_table(&w.p, &options()).unwrap();
    let (project, part, _) = open(&bytes);

    // Ordinal with labels and a missing code: numeric, the cell keeps the code. (Its name is Age_group_2 because "Age Group", another attribute, comes first.)
    let (at, age) = variable(&project, "Age_group_2");
    assert_eq!(age["label"], "Age group");
    assert_eq!(age["measure"], "ordinal");
    assert_eq!(project["dataset"]["columns"][at]["type"], "numeric");
    assert_eq!(values(&project, "Age_group_2"), json!([2, 3, -99]));
    assert_eq!(age["missing_values"], serde_json::json!([-99]));
    assert_eq!(
        age["value_labels"],
        serde_json::json!([{"value": 1, "label": "Under 30"}, {"value": 2, "label": "30 to 49"}, {"value": 3, "label": "50 or over"}])
    );

    // A ratio attribute is scale; a case without a value is null.
    let (at, years) = variable(&project, "Years_in_post");
    assert_eq!(years["measure"], "scale");
    assert_eq!(project["dataset"]["columns"][at]["type"], "numeric");
    assert_eq!(values(&project, "Years_in_post"), json!([12, 3, Value::Null]));

    // A date is a text column, written year-month-day; a value that is not read that way is left empty, and the file says so.
    let (at, date) = variable(&project, "Visit_date");
    assert_eq!(date["measure"], "nominal");
    assert_eq!(project["dataset"]["columns"][at]["type"], "text");
    assert_eq!(values(&project, "Visit_date"), json!(["2026-03-04", Value::Null, Value::Null]));
    assert!(report.notes.iter().any(|n| n.contains("Visit date") && n.contains("year-month-day")), "{:?}", report.notes);

    // Words are a text column with no labels.
    let (at, site) = variable(&project, "Site");
    assert_eq!(project["dataset"]["columns"][at]["type"], "text");
    assert_eq!(site["value_labels"], serde_json::json!([]));
    assert_eq!(values(&project, "Site"), json!(["North", "South", "North"]));

    // "007" would meet 7 as a number, so the identifier stays text.
    let (at, _) = variable(&project, "Staff_id");
    assert_eq!(project["dataset"]["columns"][at]["type"], "text");
    assert_eq!(values(&project, "Staff_id"), json!(["007", "12", Value::Null]));

    // The part names each attribute by variable id, with PrismQDA's own id and level.
    let attrs = part["attributes"].as_array().unwrap();
    assert_eq!(attrs.len(), 6);
    let age_entry = attrs.iter().find(|a| a["name"] == "Age group").unwrap();
    assert_eq!(age_entry["level"], "ordinal");
    assert_eq!(age_entry["variable"], variable(&project, "Age_group_2").1["id"]);
    assert!(age_entry["attribute_id"].as_i64().unwrap() >= 1);
}

#[test]
fn two_attributes_that_make_the_same_name_are_told_apart_and_the_change_is_listed() {
    let w = world();
    let (bytes, report) = export_case_table(&w.p, &options()).unwrap();
    let (project, _, _) = open(&bytes);
    // "Age Group" and "Age group" are the same name when case is ignored; the attributes are listed by name (BINARY order puts "Age Group" first).
    let (_, first) = variable(&project, "Age_Group");
    let (_, second) = variable(&project, "Age_group_2");
    assert_eq!(first["label"], "Age Group");
    assert_eq!(second["label"], "Age group");
    assert!(report.renamed.iter().any(|r| r[0] == "Age group" && r[1] == "Age_group_2"), "{:?}", report.renamed);
    assert!(report.renamed.iter().any(|r| r[0] == "Age Group" && r[1] == "Age_Group"), "{:?}", report.renamed);
    assert_eq!(project["source"]["report"]["renamed"].as_array().unwrap().len(), report.renamed.len());
}

#[test]
fn a_code_is_two_variables_the_excerpts_in_each_case_and_a_case_with_no_document_has_none() {
    let w = world();
    let (bytes, report) = export_case_table(&w.p, &options()).unwrap();
    let (project, part, _) = open(&bytes);

    // Sub-codes count with their parent: Belonging has Welcome's two excerpts.
    let welcome = format!("code_{}_count", w.welcome);
    let belonging = format!("code_{}_count", w.belonging);
    let cost = format!("code_{}_count", w.cost);
    assert_eq!(values(&project, &welcome), json!([2, 0, Value::Null]));
    assert_eq!(values(&project, &belonging), json!([2, 0, Value::Null]));
    assert_eq!(values(&project, &cost), json!([1, 1, Value::Null]));
    assert_eq!(values(&project, &format!("code_{}_present", w.welcome)), json!([1, 0, Value::Null]));
    assert_eq!(values(&project, &format!("code_{}_present", w.cost)), json!([1, 1, Value::Null]));

    // The labels are the path with " > " between the levels, and the count says it counts excerpts.
    let (_, present) = variable(&project, &format!("code_{}_present", w.welcome));
    assert_eq!(present["label"], "Belonging > Welcome");
    assert_eq!(present["measure"], "nominal");
    assert_eq!(present["value_labels"], serde_json::json!([{"value": 0, "label": "No"}, {"value": 1, "label": "Yes"}]));
    let (_, count) = variable(&project, &welcome);
    assert_eq!(count["label"], "Belonging > Welcome (excerpts)");
    assert_eq!(count["measure"], "scale");

    // Only active codes, in the order of their id; the retired one is left out and the file says so.
    let codes = part["codes"].as_array().unwrap();
    assert_eq!(codes.len(), 3);
    assert_eq!(codes.iter().map(|c| c["code_id"].as_i64().unwrap()).collect::<Vec<_>>(), [w.belonging, w.welcome, w.cost]);
    assert_eq!(codes[0]["kind"], "theme");
    assert_eq!(codes[1]["path"], serde_json::json!(["Belonging", "Welcome"]));
    assert_eq!(codes[1]["present_variable"], present["id"]);
    assert_eq!(codes[1]["count_variable"], count["id"]);
    assert_eq!(part["sub_codes_counted"], true);
    assert!(report.notes.iter().any(|n| n.contains("1 retired or merged code was left out")), "{:?}", report.notes);
}

#[test]
fn counting_only_the_code_itself_is_a_choice_and_the_file_says_which() {
    let w = world();
    let mut o = options();
    o.include_descendants = false;
    let (bytes, report) = export_case_table(&w.p, &o).unwrap();
    let (project, part, _) = open(&bytes);
    assert_eq!(values(&project, &format!("code_{}_count", w.belonging)), json!([0, 0, Value::Null]), "the theme has no excerpt of its own");
    assert_eq!(values(&project, &format!("code_{}_count", w.welcome)), json!([2, 0, Value::Null]));
    assert_eq!(part["sub_codes_counted"], false);
    assert!(report.notes.iter().any(|n| n.contains("only for the code it is coded with")));
}

#[test]
fn the_codebook_version_is_the_latest_saved_or_null() {
    let mut w = world();
    let (bytes, _) = export_case_table(&w.p, &options()).unwrap();
    assert_eq!(open(&bytes).1["codebook_version"], Value::Null);
    w.p.snapshot_codebook("first").unwrap();
    w.p.snapshot_codebook("second").unwrap();
    let (bytes, _) = export_case_table(&w.p, &options()).unwrap();
    assert_eq!(open(&bytes).1["codebook_version"], 2);
}

#[test]
fn a_project_with_no_cases_has_no_case_table() {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let p = Project::create(dir.path().join("p.pqda"), &NewProject::new("Empty"), &o).unwrap();
    let e = export_case_table(&p, &options()).unwrap_err();
    assert!(e.to_string().contains("no cases"), "{e}");
}

#[test]
fn nothing_but_numbers_and_names_is_written_no_excerpt_no_memo_no_coder() {
    let w = world();
    let (bytes, _) = export_case_table(&w.p, &options()).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
    let mut all = String::new();
    for name in ["project.json", "case-table.json"] {
        zip.by_name(name).unwrap().read_to_string(&mut all).unwrap();
    }
    for forbidden in ["Warm welcome", "Fees were", "Ada", "tester"] {
        assert!(!all.contains(forbidden), "{forbidden} must not be written");
    }
}

#[test]
fn the_list_of_names_to_replace_is_applied_to_the_names_and_text_written() {
    let w = world();
    let rule = DeidRule { id: 1, term: "Maria".into(), replacement: "Participant 1".into(), kind: "pseudonym".into(), match_case: false, whole_word: true, note: String::new(), created_at: String::new() };
    let _guard = scrub_scope(Some(Scrubber::new(&[rule])));
    let (bytes, report) = export_case_table(&w.p, &options()).unwrap();
    let (project, _, _) = open(&bytes);
    assert_eq!(values(&project, "case_name"), json!(["Participant 1", "Tomas", "Priya"]));
    assert!(report.notes.iter().any(|n| n.contains("list of names to replace")), "{:?}", report.notes);
}

#[test]
fn the_names_are_made_as_the_spec_says_for_a_few_awkward_attribute_names() {
    let names: Vec<String> = ["Age (years)", "age_years", "", "9 lives", "case_id", "Case Name"].iter().map(|s| s.to_string()).collect();
    assert_eq!(attribute_names(&names, &["case_id", "case_name"]), ["Age_years", "age_years_2", "a", "a_9_lives", "case_id_2", "Case_Name_2"]);
}

/// Writes the files this test project makes into the folder named by `PRISMQDA_PSTAT_OUT`, so `tools/pstat-check` can run the shared
/// core's own checker (`checkPstat` from `@openstage/pstat`) on them. Does nothing when the variable is not set.
#[test]
fn the_files_can_be_written_out_for_the_cores_checker() {
    let Ok(dir) = std::env::var("PRISMQDA_PSTAT_OUT") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let w = world();
    let variants: [(&str, bool, bool); 3] = [("sub-codes-counted", false, true), ("names-left-out", true, true), ("sub-codes-not-counted", false, false)];
    for (name, leave_out_names, include_descendants) in variants {
        let o = PstatOptions { leave_out_names, include_descendants, app_version: "PrismQDA 0.1.0-beta.2".into() };
        let (bytes, _) = export_case_table(&w.p, &o).unwrap();
        std::fs::write(dir.join(format!("{name}.pstat")), bytes).unwrap();
    }
}
