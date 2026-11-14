//! Social media and comment imports, using small invented exports (tests/fixtures/social) and tables built here.

use prismqda_engine::{
    AppliesTo, AttributeSpec, ImportBatch, ImportedSource, ImportedUnit, Level, NewProject, OpenOptions, Project,
    SourceType, UnitKind,
};
use prismqda_import::social::{from_table, parse_reddit_json, parse_youtube_json, SocialMapping};
use prismqda_import::{open_bytes, Cell, Column, ImportError, ImportOptions, Opened, Table};
use unicode_normalization::is_nfc;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/social/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// The text between two offsets that count Unicode scalar values.
fn slice(text: &str, start: i64, end: i64) -> String {
    text.chars().skip(start as usize).take((end - start) as usize).collect()
}

fn texts(s: &ImportedSource) -> Vec<String> {
    s.units.iter().map(|u| slice(&s.text, u.start, u.end)).collect()
}

fn attr<'a>(u: &'a ImportedUnit, name: &str) -> Option<&'a str> {
    u.attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

fn attrs_of(s: &ImportedSource, name: &str) -> Vec<Option<String>> {
    s.units.iter().map(|u| attr(u, name).map(str::to_string)).collect()
}

fn spec<'a>(specs: &'a [AttributeSpec], name: &str) -> Option<&'a AttributeSpec> {
    specs.iter().find(|s| s.name == name)
}

fn kinds(s: &ImportedSource) -> Vec<&str> {
    s.warnings.iter().map(|w| w.kind.as_str()).collect()
}

fn warning<'a>(s: &'a ImportedSource, kind: &str) -> &'a prismqda_engine::ImportWarning {
    s.warnings.iter().find(|w| w.kind == kind).unwrap_or_else(|| panic!("no '{kind}' warning in {:?}", s.warnings))
}

/// What every source must satisfy: NFC text, units in text order, inside the text, separated by a blank line
/// (so a unit is exactly one comment), parents before children, depth one more than the parent's.
fn check_shape(s: &ImportedSource) {
    assert!(is_nfc(&s.text));
    let len = s.text.chars().count() as i64;
    for (i, u) in s.units.iter().enumerate() {
        assert!(0 <= u.start && u.start < u.end && u.end <= len, "unit {i} {}..{}", u.start, u.end);
        if i > 0 {
            let before = &s.units[i - 1];
            assert_eq!(slice(&s.text, before.end, u.start), "\n\n", "units {} and {i} are a blank line apart", i - 1);
        }
        let text = slice(&s.text, u.start, u.end);
        assert_eq!(text, text.trim(), "unit {i} has no surrounding space");
        assert!(!text.is_empty());
        let depth: usize = attr(u, "depth").expect("every unit has a depth").parse().unwrap();
        match u.parent {
            None => assert_eq!(depth, 0, "unit {i}"),
            Some(p) => {
                assert!(p < i, "unit {i}: parent {p} must come first");
                assert_eq!(depth, attr(&s.units[p], "depth").unwrap().parse::<usize>().unwrap() + 1, "unit {i}");
            }
        }
        // Every attribute used has a definition.
        for (name, _) in &u.attrs {
            assert!(
                spec(&s.unit_attributes, name).is_some_and(|a| a.applies_to == AppliesTo::Unit),
                "{name} is defined"
            );
        }
    }
    assert_eq!(s.units.first().map(|u| u.start), Some(0));
    assert_eq!(s.units.last().map(|u| u.end), Some(len));
}

fn check_batch(b: &ImportBatch) {
    assert_eq!(b.sources.len(), b.cases.len());
    for (i, s) in b.sources.iter().enumerate() {
        check_shape(s);
        assert_eq!(s.source_type, SourceType::Social);
        assert_eq!(s.default_unit_kind, UnitKind::Comment);
        assert_eq!(b.cases[i].members.len(), 1);
        assert_eq!((b.cases[i].members[0].source, b.cases[i].members[0].unit), (i, None));
        for (name, _) in &b.cases[i].attrs {
            assert!(spec(&b.case_attributes, name).is_some_and(|a| a.applies_to == AppliesTo::Case));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// YouTube, yt-dlp layout
// ---------------------------------------------------------------------------------------------

#[test]
fn ytdlp_threads_come_out_depth_first_with_exact_offsets() {
    let batch = parse_youtube_json(&fixture("ytdlp.json"), &ImportOptions::default()).unwrap();
    check_batch(&batch);
    let s = &batch.sources[0];

    assert_eq!(s.name, "Garden tour: spring beds");
    assert_eq!(s.default_unit_kind, UnitKind::Comment);
    assert!(s.runs.is_empty());
    assert_eq!(
        texts(s),
        [
            "Great tour, merci beaucoup! \u{1F331}",
            "Merci \u{00E0} toi \u{4F60}\u{597D}",
            "Nested reply",
            "Reply two",
            // Windows line breaks became plain ones and the space around the comment is gone.
            "First line\nsecond line",
            "Reply to a comment that is not in the file",
            "Reply to the comment without text",
        ]
    );
    assert_eq!(s.text, texts(s).join("\n\n"));

    // Offsets count scalar values: the emoji is one position and so is each CJK character.
    assert_eq!((s.units[0].start, s.units[0].end), (0, 29));
    assert_eq!((s.units[1].start, s.units[1].end), (31, 45));

    // Replies follow their parent, siblings keep the file's order (c1.r1 before c1.r2), and a reply to a
    // comment without text moves up to the top level.
    let parents: Vec<Option<usize>> = s.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(1), Some(0), None, None, None]);
    assert_eq!(attrs_of(s, "depth"), ["0", "1", "2", "1", "0", "0", "0"].map(|d| Some(d.to_string())));

    assert!(s.units.iter().all(|u| u.kind == UnitKind::Comment));
    let speakers: Vec<Option<&str>> = s.units.iter().map(|u| u.speaker.as_deref()).collect();
    assert_eq!(
        speakers,
        [Some("@zoe"), Some("@marc"), Some("@zoe"), Some("@ben"), Some("@ana"), Some("@cy"), Some("@eli")]
    );
    assert_eq!(attrs_of(s, "author"), speakers.iter().map(|x| x.map(str::to_string)).collect::<Vec<_>>());
    assert_eq!(
        attrs_of(s, "external_id"),
        ["c1", "c1.r1", "c1.r1.r1", "c1.r2", "c2", "c3", "c5"].map(|d| Some(d.to_string()))
    );
    // A null like count is simply absent.
    assert_eq!(
        attrs_of(s, "likes"),
        [Some("12"), Some("3"), Some("1"), Some("0"), None, Some("2"), Some("4")].map(|d| d.map(str::to_string))
    );
    // Author role is user-mapped only, never guessed.
    assert!(attrs_of(s, "author_role").iter().all(Option::is_none));
    assert!(spec(&s.unit_attributes, "author_role").is_none());

    // Definitions.
    let level = |n: &str| spec(&s.unit_attributes, n).map(|a| a.level);
    assert_eq!(level("author"), Some(Level::Nominal));
    assert_eq!(level("depth"), Some(Level::Ratio));
    assert_eq!(level("likes"), Some(Level::Ratio));
    assert_eq!(level("external_id"), Some(Level::Text));
    assert_eq!(level("timestamp"), Some(Level::Date));

    // yt-dlp works the timestamps out from "1 year ago", so they are dates but flagged.
    assert_eq!(attr(&s.units[0], "timestamp"), Some("2024-03-05T12:00:00Z"));
    assert_eq!(attr(&s.units[1], "timestamp"), Some("2024-03-05T14:46:40Z"));
    assert_eq!(attr(&s.units[5], "timestamp"), Some("2024-03-06T01:53:20Z"));
}

#[test]
fn ytdlp_warnings_are_counted_once_each() {
    let batch = parse_youtube_json(&fixture("ytdlp.json"), &ImportOptions::default()).unwrap();
    let s = &batch.sources[0];
    assert_eq!(kinds(s), ["skipped_entries", "orphan_reply", "empty_comments", "relative_timestamps"]);
    assert!(warning(s, "skipped_entries").message.starts_with("1 entry"));
    assert!(warning(s, "orphan_reply").message.starts_with("1 reply points to a comment that is not in the file"));
    assert!(warning(s, "empty_comments").message.starts_with("1 comment had no text"));
    let relative = warning(s, "relative_timestamps");
    assert!(
        relative.message.contains("'1 year ago'") && relative.message.contains("published date"),
        "{}",
        relative.message
    );
    assert!(relative.message.contains("7 comments"), "{}", relative.message);
}

#[test]
fn ytdlp_makes_one_case_for_the_video() {
    let batch = parse_youtube_json(&fixture("ytdlp.json"), &ImportOptions::default()).unwrap();
    assert_eq!(batch.cases.len(), 1);
    let case = &batch.cases[0];
    assert_eq!(case.name, "Garden tour: spring beds");
    assert_eq!(case.members, [prismqda_engine::CaseMember { source: 0, unit: None }]);
    assert_eq!(
        case.attrs,
        [
            ("container".to_string(), "vid001".to_string()),
            ("channel".to_string(), "Green Thumb Club".to_string()),
            ("published".to_string(), "2024-03-05".to_string()),
        ]
    );
    let level = |n: &str| spec(&batch.case_attributes, n).map(|a| a.level);
    assert_eq!(
        (level("container"), level("channel"), level("published")),
        (Some(Level::Nominal), Some(Level::Nominal), Some(Level::Date))
    );
    let settings = &batch.sources[0].settings;
    assert_eq!(settings["profile"], "youtube_ytdlp");
    assert_eq!(settings["container_id"], "vid001");
    assert_eq!(settings["comments"], 7);
}

#[test]
fn options_name_folder_hash_and_overrides_are_honoured() {
    let opts = ImportOptions {
        name: Some("My video".to_string()),
        folder: "YouTube".to_string(),
        file_hash: Some("ab".repeat(32)),
        unit_kind: Some(UnitKind::Sentence),
        source_type: Some(SourceType::Other),
        ..Default::default()
    };
    let batch = parse_youtube_json(&fixture("ytdlp.json"), &opts).unwrap();
    let s = &batch.sources[0];
    assert_eq!(s.name, "My video");
    assert_eq!(s.folder, "YouTube");
    assert_eq!(s.original_file_hash.as_deref(), Some("ab".repeat(32).as_str()));
    assert_eq!(s.default_unit_kind, UnitKind::Sentence);
    assert_eq!(s.source_type, SourceType::Other);
    // The case keeps the video's own title.
    assert_eq!(batch.cases[0].name, "Garden tour: spring beds");
}

fn ytdlp_with(comments: &str) -> Vec<u8> {
    format!(r#"{{"id": "v", "title": "T", "comments": {comments}}}"#).into_bytes()
}

#[test]
fn exact_times_are_dates_and_relative_text_makes_the_attribute_text() {
    // Exact: unix seconds only.
    let b = parse_youtube_json(&ytdlp_with(r#"[{"id":"a","parent":"root","text":"x","timestamp":1700000000},{"id":"b","parent":"a","text":"y","timestamp":1700003600}]"#), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Date);
    assert_eq!(
        attrs_of(s, "timestamp"),
        [Some("2023-11-14T22:13:20Z".to_string()), Some("2023-11-14T23:13:20Z".to_string())]
    );
    assert!(!kinds(s).contains(&"relative_timestamps"));

    // Only relative text: kept as text, with one warning.
    let b = parse_youtube_json(&ytdlp_with(r#"[{"id":"a","parent":"root","text":"x","timestamp":null,"_time_text":"2 years ago"},{"id":"b","parent":"a","text":"y","_time_text":"3 weeks ago"}]"#), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Text);
    assert_eq!(attrs_of(s, "timestamp"), [Some("2 years ago".to_string()), Some("3 weeks ago".to_string())]);
    assert_eq!(kinds(s), ["relative_timestamps"]);
    assert!(warning(s, "relative_timestamps").message.contains("2 comments' times"));

    // A mix: one level for the whole source, so the exact time is kept as ISO text next to the relative one.
    let b = parse_youtube_json(&ytdlp_with(r#"[{"id":"a","parent":"root","text":"x","timestamp":1700000000},{"id":"b","parent":"a","text":"y","_time_text":"3 weeks ago"}]"#), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Text);
    assert_eq!(attrs_of(s, "timestamp"), [Some("2023-11-14T22:13:20Z".to_string()), Some("3 weeks ago".to_string())]);

    // ISO text in the time text counts as exact.
    let b = parse_youtube_json(
        &ytdlp_with(r#"[{"id":"a","parent":"root","text":"x","_time_text":"2024-01-02 03:04:05"}]"#),
        &ImportOptions::default(),
    )
    .unwrap();
    assert_eq!(attrs_of(&b.sources[0], "timestamp"), [Some("2024-01-02T03:04:05".to_string())]);
    assert!(b.sources[0].warnings.is_empty());
}

#[test]
fn text_is_normalised_to_nfc_and_offsets_follow_the_stored_text() {
    // "e" + combining acute is two scalar values in the file and one once composed.
    let b = parse_youtube_json(
        &ytdlp_with(
            r#"[{"id":"a","parent":"root","text":"Café society"},{"id":"b","parent":"a","text":"Réponse 😀"}]"#,
        ),
        &ImportOptions::default(),
    )
    .unwrap();
    let s = &b.sources[0];
    assert_eq!(s.text, "Caf\u{00E9} society\n\nR\u{00E9}ponse \u{1F600}");
    assert_eq!((s.units[0].start, s.units[0].end), (0, 12));
    assert_eq!((s.units[1].start, s.units[1].end), (14, 23));
    assert_eq!(slice(&s.text, 14, 23), "R\u{00E9}ponse \u{1F600}");
    check_batch(&b);
}

#[test]
fn loops_of_replies_are_cut_open_and_never_hang() {
    let b = parse_youtube_json(
        &ytdlp_with(
            r#"[{"id":"a","parent":"b","text":"A"},{"id":"b","parent":"a","text":"B"},{"id":"c","parent":"root","text":"C"},{"id":"d","parent":"d","text":"D"},{"id":"e","parent":"a","text":"E hangs off the loop"}]"#,
        ),
        &ImportOptions::default(),
    )
    .unwrap();
    let s = &b.sources[0];
    check_batch(&b);
    // Real top-level comments first (c, then d which replied to itself), then the loop a-b cut at a.
    assert_eq!(texts(s), ["C", "D", "A", "B", "E hangs off the loop"]);
    let parents: Vec<Option<usize>> = s.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, None, None, Some(2), Some(2)]);
    assert!(warning(s, "parent_cycle").message.starts_with("2 loops of comments"));
}

#[test]
fn duplicate_ids_send_replies_to_the_first_comment_with_that_id() {
    let b = parse_youtube_json(
        &ytdlp_with(r#"[{"id":"a","parent":"root","text":"first"},{"id":"a","parent":"root","text":"second with the same id"},{"id":"r","parent":"a","text":"reply"}]"#),
        &ImportOptions::default(),
    )
    .unwrap();
    let s = &b.sources[0];
    assert_eq!(texts(s), ["first", "reply", "second with the same id"]);
    assert_eq!(s.units[1].parent, Some(0));
    assert!(warning(s, "duplicate_ids").message.starts_with("1 comment has an ID"));
}

#[test]
fn a_long_chain_of_replies_does_not_overflow_the_stack() {
    let n = 20_000;
    let mut json = String::from(r#"{"id":"v","comments":["#);
    for i in 0..n {
        if i > 0 {
            json.push(',');
        }
        let parent = if i == 0 { "root".to_string() } else { format!("c{}", i - 1) };
        json.push_str(&format!(r#"{{"id":"c{i}","parent":"{parent}","text":"comment {i}"}}"#));
    }
    json.push_str("]}");
    let b = parse_youtube_json(json.as_bytes(), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(s.units.len(), n);
    assert_eq!(s.units[n - 1].parent, Some(n - 2));
    assert_eq!(attr(&s.units[n - 1], "depth"), Some("19999"));
    assert!(s.units.windows(2).all(|w| w[0].end < w[1].start));
}

// ---------------------------------------------------------------------------------------------
// YouTube, Data API layout
// ---------------------------------------------------------------------------------------------

#[test]
fn the_api_layout_is_detected_and_read() {
    let batch = parse_youtube_json(&fixture("youtube_api.json"), &ImportOptions::default()).unwrap();
    check_batch(&batch);
    assert_eq!(batch.sources.len(), 1);
    let s = &batch.sources[0];
    assert_eq!(s.name, "vidAPI");
    assert_eq!(
        texts(s),
        [
            // The API's HTML became text: <br> a new line, the link keeps its words, character references are replaced.
            "Nice & clear\nsecond line the link it's fine",
            // textOriginal wins and is not unescaped.
            "plain reply \u{1F600} with <3 and &amp; left alone",
            "Second thread",
        ]
    );
    let parents: Vec<Option<usize>> = s.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, Some(0), None]);
    assert_eq!(attrs_of(s, "author"), ["Ana", "Ben", "Cy"].map(|a| Some(a.to_string())));
    assert_eq!(attrs_of(s, "likes"), ["5", "0", "2"].map(|a| Some(a.to_string())));
    assert_eq!(attrs_of(s, "external_id"), ["T1", "T1.R1", "T2"].map(|a| Some(a.to_string())));
    assert_eq!(
        attrs_of(s, "timestamp"),
        ["2024-03-05T10:20:30Z", "2024-03-06T08:00:00Z", "2024-03-07T09:00:00.123+01:00"].map(|a| Some(a.to_string()))
    );
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Date);
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    assert_eq!(batch.cases[0].attrs, [("container".to_string(), "vidAPI".to_string())]);
    assert_eq!(s.settings["profile"], "youtube_api");
}

#[test]
fn an_api_response_that_mixes_videos_makes_one_source_per_video() {
    let json = r#"{"items":[
        {"snippet":{"videoId":"v1","topLevelComment":{"id":"a","snippet":{"textOriginal":"one"}}}},
        {"snippet":{"videoId":"v2","topLevelComment":{"id":"b","snippet":{"textOriginal":"two"}}}},
        {"snippet":{"videoId":"v1","topLevelComment":{"id":"c","snippet":{"textOriginal":"three"}}}},
        {"id":"d","snippet":{"videoId":"v2","parentId":"b","textOriginal":"a plain comment resource"}},
        17
    ]}"#;
    let b = parse_youtube_json(json.as_bytes(), &ImportOptions::default()).unwrap();
    check_batch(&b);
    assert_eq!(b.sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["v1", "v2"]);
    assert_eq!(texts(&b.sources[0]), ["one", "three"]);
    assert_eq!(texts(&b.sources[1]), ["two", "a plain comment resource"]);
    assert_eq!(b.sources[1].units[1].parent, Some(0));
    // Both sources define the same attributes the same way.
    assert_eq!(b.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["v1", "v2"]);
    // The entry that was not a comment is reported once, on the first source.
    assert!(kinds(&b.sources[0]).contains(&"skipped_entries"));
    // With a given name and several videos, the name is a prefix.
    let named =
        parse_youtube_json(json.as_bytes(), &ImportOptions { name: Some("Study".to_string()), ..Default::default() })
            .unwrap();
    assert_eq!(named.sources[1].name, "Study - v2");
}

// ---------------------------------------------------------------------------------------------
// Reddit
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reddit_thread_keeps_its_post_replies_and_unloaded_markers() {
    let batch = parse_reddit_json(&fixture("reddit.json"), &ImportOptions::default()).unwrap();
    check_batch(&batch);
    let s = &batch.sources[0];
    assert_eq!(s.name, "Which tomato & why?");
    assert_eq!(
        texts(s),
        [
            "Which tomato & why?\n\nI planted 12 <3 plants.\n\nAny tips?",
            "Try Roma",
            "Seconded, \u{597D}\u{756A}\u{8304} \u{1F345}",
            "[deleted]",
            "Reply to a deleted comment",
            "Another top-level comment",
            "[removed]",
            "Reply to a comment that is not in the file",
        ]
    );
    // The opening post is a Post; the rest are comments.
    let unit_kinds: Vec<UnitKind> = s.units.iter().map(|u| u.kind).collect();
    assert_eq!(unit_kinds[0], UnitKind::Post);
    assert!(unit_kinds[1..].iter().all(|k| *k == UnitKind::Comment));
    // Top-level comments reply to the post, so the post is the root with depth 0.
    let parents: Vec<Option<usize>> = s.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(1), Some(1), Some(3), Some(0), Some(0), None]);
    assert_eq!(attrs_of(s, "depth"), ["0", "1", "2", "2", "3", "1", "1", "0"].map(|d| Some(d.to_string())));
    assert_eq!(attr(&s.units[0], "timestamp"), Some("2023-11-14T22:13:20Z"));
    assert_eq!(attr(&s.units[0], "likes"), Some("42"));
    assert_eq!(attr(&s.units[3], "likes"), Some("-1"));
    assert_eq!(attr(&s.units[0], "external_id"), Some("abc123"));
    assert_eq!(s.units[3].speaker.as_deref(), Some("[deleted]"));
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Date);

    assert_eq!(kinds(s), ["deleted_comments", "unloaded_replies", "orphan_reply"]);
    let deleted = warning(s, "deleted_comments");
    assert!(deleted.message.starts_with("2 comments show '[deleted]' or '[removed]'"), "{}", deleted.message);
    let unloaded = warning(s, "unloaded_replies");
    assert!(
        unloaded.message.contains("3 placeholders") && unloaded.message.contains("about 19 replies"),
        "{}",
        unloaded.message
    );
    assert!(warning(s, "orphan_reply").message.starts_with("1 reply points"));
    assert!(!kinds(s).contains(&"relative_timestamps"));

    let case = &batch.cases[0];
    assert_eq!(case.name, "Which tomato & why?");
    assert_eq!(
        case.attrs,
        [
            ("container".to_string(), "abc123".to_string()),
            ("subreddit".to_string(), "gardening".to_string()),
            ("published".to_string(), "2023-11-14".to_string()),
        ]
    );
    assert_eq!(s.settings["profile"], "reddit");
    assert_eq!(s.settings["permalink"], "/r/gardening/comments/abc123/which_tomato_why/");
}

#[test]
fn reddit_replies_without_a_parent_id_use_their_nesting_and_comments_without_a_post_are_top_level() {
    let json = r#"[
        {"kind":"Listing","data":{"children":[]}},
        {"kind":"Listing","data":{"children":[
          {"kind":"t1","data":{"id":"a","body":"outer","replies":{"kind":"Listing","data":{"children":[
             {"kind":"t1","data":{"id":"b","body":"inner","replies":""}}]}}}},
          {"kind":"t1","data":{"id":"c","body":"second outer","parent_id":"t3_gone","replies":""}}
        ]}}]"#;
    let b = parse_reddit_json(json.as_bytes(), &ImportOptions::default()).unwrap();
    check_batch(&b);
    let s = &b.sources[0];
    assert_eq!(s.name, "Reddit thread");
    assert_eq!(texts(s), ["outer", "inner", "second outer"]);
    let parents: Vec<Option<usize>> = s.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, Some(0), None]);
    assert!(s.warnings.is_empty(), "{:?}", s.warnings);
    assert!(s.unit_attributes.iter().all(|a| a.name != "timestamp" && a.name != "likes"));
}

#[test]
fn a_deleted_post_text_is_counted_too() {
    let json = r#"[{"kind":"Listing","data":{"children":[{"kind":"t3","data":{"id":"p","title":"A title","selftext":"[removed]","author":"x","created_utc":1700000000}}]}},
                   {"kind":"Listing","data":{"children":[]}}]"#;
    let b = parse_reddit_json(json.as_bytes(), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(texts(s), ["A title\n\n[removed]"]);
    assert!(warning(s, "deleted_comments").message.starts_with("The post's text shows"));
}

// ---------------------------------------------------------------------------------------------
// Generic table
// ---------------------------------------------------------------------------------------------

fn table_from_csv(bytes: &[u8], name: &str) -> Table {
    let mut reader = csv::ReaderBuilder::new().has_headers(true).from_reader(bytes);
    let columns = reader.headers().unwrap().iter().map(Column::new).collect();
    let rows = reader
        .records()
        .map(|r| {
            r.unwrap()
                .iter()
                .map(|f| {
                    if f.is_empty() {
                        Cell::Empty
                    } else if let Ok(n) = f.parse::<f64>() {
                        Cell::Number(n)
                    } else {
                        Cell::Text(f.to_string())
                    }
                })
                .collect()
        })
        .collect();
    Table { name: name.to_string(), columns, rows, warnings: vec![] }
}

fn full_mapping() -> SocialMapping {
    SocialMapping {
        id_column: "id".into(),
        text_column: "text".into(),
        parent_column: Some("reply_to".into()),
        author_column: Some("who".into()),
        author_role_column: Some("role".into()),
        timestamp_column: Some("posted".into()),
        container_column: Some("thread".into()),
        likes_column: Some("votes".into()),
    }
}

#[test]
fn a_table_becomes_one_source_per_container_with_threads_and_roles() {
    let table = table_from_csv(&fixture("comments.csv"), "comments");
    let batch = from_table(&table, &full_mapping(), &ImportOptions::default()).unwrap();
    check_batch(&batch);
    assert_eq!(batch.sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["Thread A", "Thread B"]);

    let a = &batch.sources[0];
    assert_eq!(
        texts(a),
        [
            "Welcome to the thread, \u{00E7}a va?",
            "Thanks! \u{1F600} and \u{65E5}\u{672C}\u{8A9E}",
            "Reply to Bob",
            "Second reply to the host",
            "Reply to a comment that is not in the table",
        ]
    );
    // Numeric IDs match numeric parents; the empty row is skipped; the orphan is top level.
    let parents: Vec<Option<usize>> = a.units.iter().map(|u| u.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(1), Some(0), None]);
    assert_eq!(attrs_of(a, "depth"), ["0", "1", "2", "1", "0"].map(|d| Some(d.to_string())));
    assert_eq!(attrs_of(a, "author"), ["ann", "bob", "ann", "dee", "eli"].map(|d| Some(d.to_string())));
    assert_eq!(
        attrs_of(a, "author_role"),
        ["host", "audience", "host", "audience", "audience"].map(|d| Some(d.to_string()))
    );
    assert_eq!(attrs_of(a, "likes"), [Some("3"), Some("1"), Some("2"), None, Some("0")].map(|d| d.map(str::to_string)));
    assert_eq!(attrs_of(a, "external_id"), ["1", "2", "4", "3", "5"].map(|d| Some(d.to_string())));
    assert_eq!(
        attrs_of(a, "timestamp"),
        [
            "2024-03-05T10:00:00Z",
            "2024-03-05T11:30:00Z",
            "2024-03-06T09:00:00Z",
            "2024-03-06T08:00:00Z",
            "2024-03-06T10:00:00Z"
        ]
        .map(|d| Some(d.to_string()))
    );
    assert_eq!(spec(&a.unit_attributes, "author_role").unwrap().level, Level::Nominal);
    assert_eq!(spec(&a.unit_attributes, "timestamp").unwrap().level, Level::Date);
    assert_eq!(kinds(a), ["orphan_reply", "empty_comments"]);
    assert!(warning(a, "empty_comments").message.starts_with("1 comment had no text"));

    let b = &batch.sources[1];
    assert_eq!(texts(b), ["A different thread starts here", "Reply in thread B"]);
    assert_eq!(b.units[1].parent, Some(0));
    // Dates without a time are kept as they are, and both sources agree on the level.
    assert_eq!(attrs_of(b, "timestamp"), [Some("2024-04-01".to_string()), Some("2024-04-02".to_string())]);
    assert_eq!(spec(&b.unit_attributes, "timestamp").unwrap().level, Level::Date);
    assert!(b.warnings.is_empty());

    // One case per container, each holding its whole source.
    assert_eq!(batch.cases.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Thread A", "Thread B"]);
    assert_eq!(batch.cases[1].attrs, [("container".to_string(), "Thread B".to_string())]);
    assert_eq!(a.settings["profile"], "table");
    assert_eq!(a.settings["mapping"]["parent_column"], "reply_to");
}

#[test]
fn author_role_is_only_stored_when_a_column_was_mapped_for_it() {
    let table = table_from_csv(&fixture("comments.csv"), "comments");
    let mapping = SocialMapping { author_role_column: None, ..full_mapping() };
    let batch = from_table(&table, &mapping, &ImportOptions::default()).unwrap();
    for s in &batch.sources {
        assert!(spec(&s.unit_attributes, "author_role").is_none());
        assert!(s.units.iter().all(|u| attr(u, "author_role").is_none()));
    }
}

#[test]
fn without_a_container_column_the_whole_table_is_one_source() {
    let table = table_from_csv(&fixture("comments.csv"), "comments");
    let mapping = SocialMapping { container_column: None, ..full_mapping() };
    let batch = from_table(&table, &mapping, &ImportOptions::default()).unwrap();
    check_batch(&batch);
    assert_eq!(batch.sources.len(), 1);
    assert_eq!(batch.sources[0].name, "comments");
    assert_eq!(batch.sources[0].units.len(), 7);
    assert_eq!(batch.cases[0].name, "comments");
    assert!(batch.cases[0].attrs.is_empty());
    let named =
        from_table(&table, &mapping, &ImportOptions { name: Some("Study".into()), ..Default::default() }).unwrap();
    assert_eq!(named.sources[0].name, "Study");
    assert_eq!(named.cases[0].name, "Study");
}

#[test]
fn mistakes_in_the_mapping_are_explained_in_plain_words() {
    let table = table_from_csv(&fixture("comments.csv"), "comments");
    let message = |m: SocialMapping| match from_table(&table, &m, &ImportOptions::default()).unwrap_err() {
        ImportError::Format(msg) => msg,
        other => panic!("expected a format error, got {other:?}"),
    };

    let m = message(SocialMapping { text_column: "nope".into(), author_column: Some("who2".into()), ..full_mapping() });
    assert!(m.starts_with("these are not a column of this file: 'nope', 'who2'"), "{m}");
    assert!(m.contains("The columns are 'thread', 'id', 'reply_to'"), "{m}");

    let m = message(SocialMapping { timestamp_column: Some("when".into()), ..full_mapping() });
    assert!(m.starts_with("this is not a column of this file: 'when'"), "{m}");

    let m = message(SocialMapping { text_column: "  ".into(), ..full_mapping() });
    assert!(m.contains("choose the column that holds the text"), "{m}");

    let m = message(SocialMapping { id_column: String::new(), ..full_mapping() });
    assert!(m.contains("choose both the column that holds each comment's own ID"), "{m}");

    // Without replies the ID column is optional, and empty names mean "not mapped".
    let ok = SocialMapping {
        id_column: String::new(),
        text_column: "text".into(),
        parent_column: Some(String::new()),
        author_column: Some(" ".into()),
        ..Default::default()
    };
    let batch = from_table(&table, &ok, &ImportOptions::default()).unwrap();
    assert_eq!(batch.sources[0].units.len(), 7);
    assert!(batch.sources[0].units.iter().all(|u| u.parent.is_none()));
}

#[test]
fn a_table_with_no_text_at_all_is_an_error() {
    let table = Table {
        name: "t".into(),
        columns: vec![Column::new("id"), Column::new("text")],
        rows: vec![vec![Cell::Number(1.0), Cell::Empty], vec![Cell::Number(2.0), Cell::Text("   ".into())]],
        warnings: vec![],
    };
    let mapping = SocialMapping { id_column: "id".into(), text_column: "text".into(), ..Default::default() };
    let err = from_table(&table, &mapping, &ImportOptions::default()).unwrap_err();
    assert!(matches!(err, ImportError::Format(_)));
    assert!(err.to_string().contains("none of the comments"), "{err}");
}

fn table_of(rows: Vec<Vec<Cell>>) -> Table {
    Table {
        name: "t".into(),
        columns: vec![Column::new("id"), Column::new("text"), Column::new("when"), Column::new("box")],
        rows,
        warnings: vec![],
    }
}

fn simple_mapping() -> SocialMapping {
    SocialMapping {
        id_column: "id".into(),
        text_column: "text".into(),
        timestamp_column: Some("when".into()),
        container_column: Some("box".into()),
        ..Default::default()
    }
}

#[test]
fn table_times_are_dates_when_they_are_dates_and_text_otherwise() {
    let t = |s: &str| Cell::Text(s.to_string());
    // Date cells, ISO text, and unix seconds are all exact.
    let table = table_of(vec![
        vec![Cell::Number(1.0), t("a"), Cell::Date("2024-05-06T07:08:09Z".into()), t("x")],
        vec![Cell::Number(2.0), t("b"), t("2024-05-06 07:08"), t("x")],
        vec![Cell::Number(3.0), t("c"), Cell::Number(1700000000.0), t("x")],
        vec![Cell::Number(4.0), t("d"), t("1700000000"), t("x")],
        vec![Cell::Number(5.0), t("e"), Cell::Empty, t("x")],
    ]);
    let b = from_table(&table, &simple_mapping(), &ImportOptions::default()).unwrap();
    let s = &b.sources[0];
    assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Date);
    assert_eq!(
        attrs_of(s, "timestamp"),
        [
            Some("2024-05-06T07:08:09Z"),
            Some("2024-05-06T07:08"),
            Some("2023-11-14T22:13:20Z"),
            Some("2023-11-14T22:13:20Z"),
            None
        ]
        .map(|d| d.map(str::to_string))
    );
    assert!(s.warnings.is_empty());

    // A number that is not plausibly a time is not turned into one.
    let table = table_of(vec![vec![Cell::Number(1.0), t("a"), Cell::Number(42.0), t("x")]]);
    let b = from_table(&table, &simple_mapping(), &ImportOptions::default()).unwrap();
    assert_eq!(spec(&b.sources[0].unit_attributes, "timestamp").unwrap().level, Level::Text);

    // Relative text in one container makes the attribute text in every source, so they agree.
    let table = table_of(vec![
        vec![Cell::Number(1.0), t("a"), t("2 years ago"), t("x")],
        vec![Cell::Number(2.0), t("b"), t("2024-05-06"), t("y")],
    ]);
    let b = from_table(&table, &simple_mapping(), &ImportOptions::default()).unwrap();
    for s in &b.sources {
        assert_eq!(spec(&s.unit_attributes, "timestamp").unwrap().level, Level::Text);
    }
    assert_eq!(attrs_of(&b.sources[0], "timestamp"), [Some("2 years ago".to_string())]);
    assert_eq!(attrs_of(&b.sources[1], "timestamp"), [Some("2024-05-06".to_string())]);
    assert_eq!(kinds(&b.sources[0]), ["relative_timestamps"]);
    assert!(b.sources[1].warnings.is_empty());
}

#[test]
fn rows_without_a_container_value_go_to_their_own_source() {
    let t = |s: &str| Cell::Text(s.to_string());
    let table = table_of(vec![
        vec![Cell::Number(1.0), t("a"), Cell::Empty, t("x")],
        vec![Cell::Number(2.0), t("b"), Cell::Empty, Cell::Empty],
    ]);
    let b = from_table(&table, &simple_mapping(), &ImportOptions::default()).unwrap();
    assert_eq!(b.sources.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["x", "No container"]);
    assert!(kinds(&b.sources[0]).contains(&"missing_container"));
}

#[test]
fn a_container_with_no_text_is_left_out_and_reported() {
    let t = |s: &str| Cell::Text(s.to_string());
    let table = table_of(vec![
        vec![Cell::Number(1.0), Cell::Empty, Cell::Empty, t("empty one")],
        vec![Cell::Number(2.0), t("kept"), Cell::Empty, t("full one")],
    ]);
    let b = from_table(&table, &simple_mapping(), &ImportOptions::default()).unwrap();
    assert_eq!(b.sources.len(), 1);
    assert_eq!(b.cases.len(), 1);
    assert_eq!(kinds(&b.sources[0]), ["no_text"]);
    assert!(warning(&b.sources[0], "no_text").message.contains("'empty one'"));
}

#[test]
fn table_warnings_from_the_reader_reach_the_first_source() {
    let mut table = table_from_csv(&fixture("comments.csv"), "comments");
    table.warnings.push(prismqda_engine::ImportWarning::new("odd_encoding", "some characters were replaced"));
    let b = from_table(&table, &full_mapping(), &ImportOptions::default()).unwrap();
    assert!(kinds(&b.sources[0]).contains(&"odd_encoding"));
    assert!(!kinds(&b.sources[1]).contains(&"odd_encoding"));
}

// ---------------------------------------------------------------------------------------------
// Bad input
// ---------------------------------------------------------------------------------------------

fn must_be_a_plain_error(result: Result<ImportBatch, ImportError>, what: &str) {
    match result {
        Ok(_) => {}
        Err(ImportError::Format(m)) | Err(ImportError::Unsupported(m)) => assert!(!m.trim().is_empty(), "{what}"),
        Err(other) => panic!("{what}: unexpected error kind {other:?}"),
    }
}

#[test]
fn bad_json_is_a_plain_error_for_every_entry_point() {
    let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
    let samples: Vec<&[u8]> = vec![
        b"",
        b"   ",
        b"null",
        b"true",
        b"12",
        b"\"text\"",
        b"[]",
        b"{}",
        b"[1,2,3]",
        b"[[],[]]",
        b"[{},{}]",
        b"[{\"kind\":\"Listing\"},{\"kind\":\"Listing\",\"data\":5}]",
        b"[{\"data\":{\"children\":[{\"kind\":\"t1\"},{\"kind\":\"t1\",\"data\":5},{\"kind\":\"t3\",\"data\":[]}]}}]",
        b"{\"comments\":5}",
        b"{\"comments\":null}",
        b"{\"comments\":[]}",
        b"{\"comments\":[1,\"a\",null,[],{}]}",
        b"{\"comments\":[{\"text\":5,\"id\":{\"a\":1},\"parent\":[],\"like_count\":\"x\",\"timestamp\":\"soon\",\"author\":false}]}",
        b"{\"items\":5}",
        b"{\"items\":[]}",
        b"{\"items\":[null,5,{},{\"snippet\":5},{\"snippet\":{\"topLevelComment\":5}}]}",
        b"{\"comments\":[{\"text\":\"x\"",
        b"\xff\xfe\x00not json",
        deep.as_bytes(),
    ];
    for (i, bytes) in samples.iter().enumerate() {
        must_be_a_plain_error(parse_youtube_json(bytes, &ImportOptions::default()), &format!("youtube sample {i}"));
        must_be_a_plain_error(parse_reddit_json(bytes, &ImportOptions::default()), &format!("reddit sample {i}"));
    }
}

#[test]
fn nothing_but_empty_comments_is_an_error_not_an_empty_source() {
    let err = parse_youtube_json(
        &ytdlp_with(r#"[{"id":"a","parent":"root","text":"  "},{"id":"b","parent":"a","text":""}]"#),
        &ImportOptions::default(),
    )
    .unwrap_err();
    assert!(err.to_string().contains("none of the comments"), "{err}");
}

#[test]
fn files_with_the_wrong_shape_say_what_was_expected() {
    let err = parse_reddit_json(b"{\"comments\": []}", &ImportOptions::default()).unwrap_err();
    assert!(err.to_string().contains("list of two listings"), "{err}");
    let err = parse_youtube_json(b"[1]", &ImportOptions::default()).unwrap_err();
    assert!(err.to_string().contains("one JSON object"), "{err}");
    let err = parse_youtube_json(b"{\"comments\": null}", &ImportOptions::default()).unwrap_err();
    assert!(err.to_string().contains("--write-comments"), "{err}");
    let err = parse_youtube_json(b"{\"title\": \"x\"}", &ImportOptions::default()).unwrap_err();
    assert!(err.to_string().contains("neither"), "{err}");
}

/// Replaces one randomly chosen value somewhere inside a JSON document with a value of another type.
fn confuse(value: &mut serde_json::Value, next: &mut impl FnMut() -> u64) {
    use serde_json::Value;
    let replacements = [
        Value::Null,
        Value::Bool(true),
        serde_json::json!(5),
        serde_json::json!(-1.5e300),
        serde_json::json!("str\u{0}x"),
        serde_json::json!([]),
        serde_json::json!({}),
        serde_json::json!([[1]]),
    ];
    // Choose a path down the document first, then follow it.
    let mut path: Vec<usize> = Vec::new();
    let mut look: &Value = value;
    loop {
        let len = match look {
            Value::Array(a) => a.len(),
            Value::Object(o) => o.len(),
            _ => 0,
        };
        if len == 0 || next().is_multiple_of(4) {
            break;
        }
        let i = (next() % len as u64) as usize;
        look = match look {
            Value::Array(a) => &a[i],
            Value::Object(o) => o.values().nth(i).expect("in range"),
            _ => unreachable!("only containers have children"),
        };
        path.push(i);
    }
    let mut here: &mut Value = value;
    for i in path {
        here = match here {
            Value::Array(a) => &mut a[i],
            Value::Object(o) => o.values_mut().nth(i).expect("in range"),
            _ => unreachable!("only containers have children"),
        };
    }
    *here = replacements[(next() % replacements.len() as u64) as usize].clone();
}

#[test]
fn documents_with_values_of_the_wrong_type_never_panic() {
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let (mut imported, mut refused) = (0, 0);
    for (name, reddit) in [("ytdlp.json", false), ("youtube_api.json", false), ("reddit.json", true)] {
        let original: serde_json::Value = serde_json::from_slice(&fixture(name)).unwrap();
        for _ in 0..400 {
            let mut doc = original.clone();
            for _ in 0..(1 + next() % 3) {
                confuse(&mut doc, &mut next);
            }
            let bytes = serde_json::to_vec(&doc).unwrap();
            let result = if reddit {
                parse_reddit_json(&bytes, &ImportOptions::default())
            } else {
                parse_youtube_json(&bytes, &ImportOptions::default())
            };
            match result {
                Ok(batch) => {
                    check_batch(&batch);
                    imported += 1;
                }
                Err(_) => refused += 1,
            }
        }
    }
    // The test is only worth something if it reaches both outcomes often.
    assert!(imported > 100 && refused > 20, "imported {imported}, refused {refused}");
}

#[test]
fn a_byte_order_mark_in_front_of_the_json_is_ignored() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(fixture("reddit.json"));
    assert_eq!(parse_reddit_json(&bytes, &ImportOptions::default()).unwrap().sources[0].units.len(), 8);
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend(fixture("ytdlp.json"));
    assert_eq!(parse_youtube_json(&bytes, &ImportOptions::default()).unwrap().sources[0].units.len(), 7);
}

#[test]
fn byte_damage_never_panics() {
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for name in ["ytdlp.json", "reddit.json"] {
        let good = fixture(name);
        for _ in 0..300 {
            let mut bad = good.clone();
            for _ in 0..(1 + next() % 4) {
                let at = (next() % bad.len() as u64) as usize;
                bad[at] = (next() & 0xFF) as u8;
            }
            let _ = parse_youtube_json(&bad, &ImportOptions::default());
            let _ = parse_reddit_json(&bad, &ImportOptions::default());
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Through open_bytes and the engine
// ---------------------------------------------------------------------------------------------

fn open_batch(name: &str, bytes: &[u8]) -> ImportBatch {
    match open_bytes(name, bytes, &ImportOptions::default()).unwrap() {
        Opened::Batch(b) => b,
        other => panic!("expected a batch, got {other:?}"),
    }
}

fn temp_project(dir: &tempfile::TempDir) -> Project {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap()
}

/// (external id, parent's external id, comment text read back from the stored text, speaker, kind)
type Stored = (String, Option<String>, String, Option<String>, String);

fn stored_units(conn: &rusqlite::Connection, source_id: i64) -> Vec<Stored> {
    let mut stmt = conn
        .prepare(
            "SELECT
               (SELECT av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id
                 WHERE a.name = 'external_id' AND a.applies_to = 'unit' AND av.target_id = u.id),
               (SELECT av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id
                 WHERE a.name = 'external_id' AND a.applies_to = 'unit' AND av.target_id = u.parent_unit_id),
               substr(s.text, u.start_offset + 1, u.end_offset - u.start_offset),
               u.speaker,
               u.kind
             FROM unit u JOIN source s ON s.id = u.source_id
             WHERE u.source_id = ?1 ORDER BY u.start_offset",
        )
        .unwrap();
    stmt.query_map([source_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn a_reddit_file_goes_through_open_bytes_into_the_database_with_reply_links_intact() {
    let batch = open_batch("thread.json", &fixture("reddit.json"));
    let dir = tempfile::TempDir::new().unwrap();
    let mut project = temp_project(&dir);
    let outcome = project.apply_import(&batch).unwrap();
    assert_eq!(outcome.source_ids.len(), 1);
    assert_eq!(outcome.case_ids.len(), 1);
    assert_eq!(outcome.units_created, 8);

    let conn = rusqlite::Connection::open(project.path()).unwrap();
    let units = stored_units(&conn, outcome.source_ids[0]);
    let s = |x: &str| x.to_string();
    let expected: Vec<Stored> = vec![
        (
            s("abc123"),
            None,
            s("Which tomato & why?\n\nI planted 12 <3 plants.\n\nAny tips?"),
            Some(s("gardener1")),
            s("post"),
        ),
        (s("c1"), Some(s("abc123")), s("Try Roma"), Some(s("u1")), s("comment")),
        (s("c2"), Some(s("c1")), s("Seconded, \u{597D}\u{756A}\u{8304} \u{1F345}"), Some(s("u2")), s("comment")),
        (s("c3"), Some(s("c1")), s("[deleted]"), Some(s("[deleted]")), s("comment")),
        (s("c4"), Some(s("c3")), s("Reply to a deleted comment"), Some(s("u4")), s("comment")),
        (s("c5"), Some(s("abc123")), s("Another top-level comment"), Some(s("u5")), s("comment")),
        (s("c6"), Some(s("abc123")), s("[removed]"), Some(s("u6")), s("comment")),
        (s("c7"), None, s("Reply to a comment that is not in the file"), Some(s("u7")), s("comment")),
    ];
    assert_eq!(units, expected, "the text SQLite reads back at each stored offset is exactly the comment");

    // The case holds the whole source.
    let members: Vec<(Option<i64>, Option<i64>)> = conn
        .prepare("SELECT source_id, unit_id FROM case_member WHERE case_id = ?1")
        .unwrap()
        .query_map([outcome.case_ids[0]], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(members, [(Some(outcome.source_ids[0]), None)]);

    // Attribute levels and the import report are stored.
    let level: String = conn
        .query_row("SELECT level FROM attribute WHERE applies_to = 'unit' AND name = 'timestamp'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(level, "date");
    let (report, settings): (String, String) = conn
        .query_row(
            "SELECT import_report_json, import_settings_json FROM source WHERE id = ?1",
            [outcome.source_ids[0]],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(
        report.contains("deleted_comments") && report.contains("unloaded_replies") && report.contains("orphan_reply")
    );
    assert!(settings.contains("\"profile\":\"reddit\""));
}

#[test]
fn a_youtube_file_goes_through_open_bytes_and_the_engine_with_reply_links_and_a_case() {
    let batch = open_batch("video.json", &fixture("ytdlp.json"));
    let dir = tempfile::TempDir::new().unwrap();
    let mut project = temp_project(&dir);
    let outcome = project.apply_import(&batch).unwrap();
    let conn = rusqlite::Connection::open(project.path()).unwrap();

    let units = stored_units(&conn, outcome.source_ids[0]);
    let links: Vec<(String, Option<String>)> = units.iter().map(|u| (u.0.clone(), u.1.clone())).collect();
    let s = |x: &str| x.to_string();
    assert_eq!(
        links,
        [
            (s("c1"), None),
            (s("c1.r1"), Some(s("c1"))),
            (s("c1.r1.r1"), Some(s("c1.r1"))),
            (s("c1.r2"), Some(s("c1"))),
            (s("c2"), None),
            (s("c3"), None),
            (s("c5"), None),
        ]
    );
    assert_eq!(units[0].2, "Great tour, merci beaucoup! \u{1F331}");
    assert_eq!(units[4].2, "First line\nsecond line");

    let name: String =
        conn.query_row("SELECT name FROM research_case WHERE id = ?1", [outcome.case_ids[0]], |r| r.get(0)).unwrap();
    assert_eq!(name, "Garden tour: spring beds");
    let published: String = conn
        .query_row(
            "SELECT av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id WHERE a.name = 'published' AND av.target_id = ?1",
            [outcome.case_ids[0]],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(published, "2024-03-05");
}

#[test]
fn two_imports_of_the_same_kind_of_file_can_share_the_attribute_definitions() {
    // The engine requires an existing attribute to have the same level, so exact times in both files is fine.
    let dir = tempfile::TempDir::new().unwrap();
    let mut project = temp_project(&dir);
    let first = parse_youtube_json(
        &ytdlp_with(r#"[{"id":"a","parent":"root","text":"x","timestamp":1700000000}]"#),
        &ImportOptions::default(),
    )
    .unwrap();
    let second = parse_youtube_json(
        &ytdlp_with(r#"[{"id":"b","parent":"root","text":"y","timestamp":1700000500}]"#),
        &ImportOptions::default(),
    )
    .unwrap();
    project.apply_import(&first).unwrap();
    project.apply_import(&second).unwrap();
}

#[test]
fn a_table_batch_is_stored_with_one_source_and_case_per_container() {
    let table = table_from_csv(&fixture("comments.csv"), "comments");
    let batch = from_table(&table, &full_mapping(), &ImportOptions::default()).unwrap();
    let dir = tempfile::TempDir::new().unwrap();
    let mut project = temp_project(&dir);
    let outcome = project.apply_import(&batch).unwrap();
    assert_eq!(outcome.source_ids.len(), 2);
    assert_eq!(outcome.case_ids.len(), 2);
    let conn = rusqlite::Connection::open(project.path()).unwrap();
    let a = stored_units(&conn, outcome.source_ids[0]);
    assert_eq!(
        a.iter().map(|u| (u.0.as_str(), u.1.as_deref())).collect::<Vec<_>>(),
        [("1", None), ("2", Some("1")), ("4", Some("2")), ("3", Some("1")), ("5", None)]
    );
    let roles: Vec<String> = conn
        .prepare(
            "SELECT av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id
             JOIN unit u ON u.id = av.target_id WHERE a.name = 'author_role' AND u.source_id = ?1 ORDER BY u.start_offset",
        )
        .unwrap()
        .query_map([outcome.source_ids[0]], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(roles, ["host", "audience", "host", "audience", "audience"]);
}
