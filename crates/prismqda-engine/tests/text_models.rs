//! Text analytics, the second part (decision 202): the forms of a word counted together, topics, and sentiment.

use prismqda_engine::{Error, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, OpenOptions, Project, SentimentSpec, SourceType, TopicSpec, UnitKind, WordSpec};
use tempfile::TempDir;

fn options() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

fn batch(name: &str, paragraphs: &[String]) -> ImportBatch {
    let mut text = String::new();
    let mut units = Vec::new();
    for para in paragraphs {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    ImportBatch {
        sources: vec![ImportedSource {
            name: name.into(),
            source_type: SourceType::Document,
            folder: String::new(),
            text,
            runs: vec![],
            units,
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        ..Default::default()
    }
}

fn project(paragraphs: &[String]) -> (Project, TempDir, i64) {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("t.pqda"), &NewProject::new("Text"), &options()).unwrap();
    let source = p.apply_import(&batch("Doc", paragraphs)).unwrap().source_ids[0];
    (p, dir, source)
}

fn paragraphs(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn the_forms_of_a_word_are_counted_together_when_asked_and_listed() {
    let (p, _dir, _) = project(&paragraphs(&["I connect the lines.", "They connected it.", "Connecting is hard.", "Connections matter.", "Buses are late."]));
    let plain = p.word_frequencies(&WordSpec::default()).unwrap();
    assert!(plain.words.iter().all(|w| w.forms.is_empty()), "without the option nothing is grouped");
    assert_eq!(plain.words.iter().filter(|w| w.word.starts_with("connect")).count(), 4);
    let stemmed = p.word_frequencies(&WordSpec { stem: true, ..Default::default() }).unwrap();
    let connect = stemmed.words.iter().find(|w| w.word.starts_with("connect")).unwrap();
    assert_eq!((connect.word.as_str(), connect.count, connect.documents), ("connect", 4, 1), "four forms of one word, each used once, in one document, shown by the first alphabetically");
    assert_eq!(connect.forms.iter().map(|f| f.word.as_str()).collect::<Vec<_>>(), ["connect", "connected", "connecting", "connections"]);
    assert!(stemmed.words.iter().filter(|w| w.word.starts_with("connect")).count() == 1);
    assert!(stemmed.distinct_words < plain.distinct_words, "the forms are one word");
    // A word with one form has no list of forms.
    assert!(stemmed.words.iter().filter(|w| w.word == "lines" || w.word == "line").all(|w| w.forms.is_empty()));
}

/// Forty passages: twenty about travel and twenty about food, each with six of ten words of its own kind.
fn two_subjects() -> Vec<String> {
    let travel = ["bus", "train", "station", "platform", "ticket", "driver", "route", "timetable", "commute", "journey"];
    let food = ["soup", "bread", "kitchen", "dinner", "recipe", "flavour", "breakfast", "lunch", "cooking", "dessert"];
    let mut out = Vec::new();
    for i in 0..20 {
        let words: Vec<&str> = (0..6).map(|j| travel[(i * 3 + j) % 10]).collect();
        out.push(format!("The {} was very late today.", words.join(" and the ")));
        let words: Vec<&str> = (0..6).map(|j| food[(i * 3 + j) % 10]).collect();
        out.push(format!("The {} was very tasty today.", words.join(" and the ")));
    }
    out
}

#[test]
fn two_subjects_are_found_as_two_topics_and_the_same_text_gives_the_same_topics() {
    let (p, _dir, _) = project(&two_subjects());
    let spec = TopicSpec { topics: 2, ..Default::default() };
    let report = p.topics(&spec).unwrap();
    assert_eq!(report.topics.len(), 2);
    assert_eq!(report.passages_used, 40);
    // "late" and "tasty" are in every passage of their subject, so they belong to it as much as the other words do.
    let travel = ["bus", "train", "station", "platform", "ticket", "driver", "route", "timetable", "commute", "journey", "late"];
    let food = ["soup", "bread", "kitchen", "dinner", "recipe", "flavour", "breakfast", "lunch", "cooking", "dessert", "tasty"];
    let kind = |t: &prismqda_engine::Topic| -> &'static str {
        let top: Vec<&str> = t.words.iter().take(5).map(|w| w.word.as_str()).collect();
        if top.iter().all(|w| travel.contains(w)) {
            "travel"
        } else if top.iter().all(|w| food.contains(w)) {
            "food"
        } else {
            "mixed"
        }
    };
    let kinds: Vec<&str> = report.topics.iter().map(kind).collect();
    assert!(kinds.contains(&"travel") && kinds.contains(&"food"), "each topic is one subject: {kinds:?} {:?}", report.topics.iter().map(|t| &t.label).collect::<Vec<_>>());
    for t in &report.topics {
        assert!((18..=22).contains(&t.passages), "{} has {} passages", t.label, t.passages);
        assert!((t.share - t.passages as f64 / 40.0).abs() < 1e-12);
        assert_eq!(t.examples.len(), 3);
        for e in &t.examples {
            let about_travel = e.text.contains("late");
            assert_eq!(kind(t) == "travel", about_travel, "an example of a topic is a passage of its subject: {} / {}", t.label, e.text);
            assert!(e.strength > 0.5 && e.strength <= 1.0);
        }
        assert!((t.words[0].weight - 1.0).abs() < 1e-12 && t.words.windows(2).all(|w| w[0].weight >= w[1].weight), "the strongest word first");
    }
    assert_eq!(report.topics.iter().map(|t| t.number).collect::<Vec<_>>(), [1, 2]);
    assert_eq!(p.topics(&spec).unwrap(), report, "the same text and settings give the same topics");
    assert!(report.notes.iter().any(|n| n.contains("not a finding")));
}

#[test]
fn topics_are_refused_when_the_number_or_the_text_makes_no_sense() {
    let (p, _dir, _) = project(&paragraphs(&["A short one about buses.", "Another about trains.", "A third about stations."]));
    for k in [0, 1, 21] {
        assert!(matches!(p.topics(&TopicSpec { topics: k, ..Default::default() }), Err(Error::Invalid(_))), "{k}");
    }
    let little = p.topics(&TopicSpec { topics: 5, ..Default::default() });
    assert!(matches!(little, Err(Error::Invalid(ref m)) if m.contains("too little text")), "{little:?}");
}

#[test]
fn sentiment_counts_positive_negative_mixed_and_neutral_passages_and_turns_a_denied_word_round() {
    let (mut p, _dir, source) = project(&paragraphs(&[
        "The bus was great and the driver was kind.",
        "The fares are terrible.",
        "The staff were helpful but the wait was awful.",
        "It was not good.",
        "We went to the shop.",
        "I do not hate it.",
    ]));
    let report = p.sentiment(&SentimentSpec { examples: 5, ..Default::default() }).unwrap();
    assert_eq!(report.passages, 6);
    assert_eq!((report.counts.positive, report.counts.negative, report.counts.mixed, report.counts.neutral), (2, 2, 1, 1));
    assert_eq!(report.most_positive[0].positive_words, ["great", "kind"]);
    assert_eq!(report.most_positive[0].net, 2);
    assert!(report.most_positive.iter().any(|m| m.positive_words == ["not hate"]), "a denied negative word is counted as positive: {:?}", report.most_positive);
    assert!(report.most_negative.iter().any(|m| m.negative_words == ["not good"]), "a denied positive word is counted as negative: {:?}", report.most_negative);
    assert_eq!(report.by_document.len(), 1);
    assert_eq!(report.by_document[0].counts, report.counts);
    assert!(report.positive_list > 100 && report.negative_list > 100);
    assert!(report.notes.iter().any(|n| n.contains("read a sample")));

    // The passages by what they were coded with.
    let me = p.add_coder(&NewCoder::human("Reader")).unwrap();
    p.add_code(&NewCode::new("Cost")).unwrap();
    let cost = p.list_codes().unwrap()[0].id;
    let units = p.list_units(source).unwrap();
    let mut c = NewCoding::new(source, units[1].start, units[1].end, cost, me);
    c.unit_id = Some(units[1].id);
    p.apply_coding(&c).unwrap();
    let coded = p.sentiment(&SentimentSpec::default()).unwrap();
    assert_eq!(coded.by_code.len(), 1);
    assert_eq!(coded.by_code[0].name, "Cost");
    assert_eq!((coded.by_code[0].counts.negative, coded.by_code[0].counts.positive), (1, 0));
}

#[test]
fn a_persons_own_words_override_the_list_and_a_contradiction_is_refused() {
    let (p, _dir, _) = project(&paragraphs(&["The bus was great and the driver was kind.", "We went to the shop."]));
    let own = p.sentiment(&SentimentSpec { positive_words: vec!["Shop".into()], negative_words: vec!["great".into()], ..Default::default() }).unwrap();
    // "great" is now negative, "kind" still positive: the first passage has both; "shop" is positive.
    assert_eq!((own.counts.positive, own.counts.negative, own.counts.mixed, own.counts.neutral), (1, 0, 1, 0));
    let only = p.sentiment(&SentimentSpec { own_words_only: true, positive_words: vec!["shop".into()], ..Default::default() }).unwrap();
    assert_eq!((only.counts.positive, only.counts.neutral), (1, 1), "only the person's words are looked for");
    assert!(only.notes.iter().all(|n| !n.contains("built-in list is a short one")));
    assert!(matches!(p.sentiment(&SentimentSpec { positive_words: vec!["fine".into()], negative_words: vec!["Fine".into()], ..Default::default() }), Err(Error::Invalid(_))));
    assert!(matches!(p.sentiment(&SentimentSpec { own_words_only: true, ..Default::default() }), Err(Error::Invalid(_))), "no words to look for");
}
