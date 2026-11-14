//! Text analytics: the words the documents use and where a word is used.

use prismqda_engine::{
    read_image, sha256_hex, ContextSpec, ImageAttachment, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SourceType,
    UnitKind, WordSpec,
};
use serde_json::Value;
use tempfile::TempDir;

const RIDERS: &str = "Fares are too high. Fares are a worry, and I don't like the fares.";
const NOTES: &str = "The staff were kind. The staff were helpful.";

struct W {
    p: Project,
    _dir: TempDir,
    riders: i64,
    notes: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("t.pqda"), &NewProject::new("T"), &o).unwrap();
    let riders = p.add_source(&NewSource::new("Riders", SourceType::Document, RIDERS)).unwrap();
    let notes = p.add_source(&NewSource::new("Notes", SourceType::Document, NOTES)).unwrap();
    W { p, _dir: dir, riders, notes }
}

fn names(w: &prismqda_engine::WordFrequencies) -> Vec<(&str, usize)> {
    w.words.iter().map(|x| (x.word.as_str(), x.count)).collect()
}

#[test]
fn the_commonest_words_come_first_with_common_and_short_words_left_out() {
    let w = world();
    let f = w.p.word_frequencies(&WordSpec::default()).unwrap();
    assert_eq!(names(&f), [("fares", 3), ("staff", 2), ("helpful", 1), ("high", 1), ("kind", 1), ("like", 1), ("worry", 1)]);
    assert_eq!((f.total_words, f.counted_words, f.distinct_words, f.documents), (22, 10, 7, 2));
    assert_eq!(f.words[0].documents, 1, "fares is in one document");

    // Everything counted, without leaving out the common words or the short ones.
    let all = w.p.word_frequencies(&WordSpec { leave_out_common: false, min_length: 1, ..Default::default() }).unwrap();
    assert_eq!(all.counted_words, all.total_words);
    let the = all.words.iter().find(|x| x.word == "the").unwrap();
    assert_eq!((the.count, the.documents), (3, 2));
    assert!(all.words.iter().any(|x| x.word == "don't"), "an apostrophe stays inside a word");

    // Only the words used often, the commonest few, and one document.
    assert_eq!(names(&w.p.word_frequencies(&WordSpec { min_count: 2, ..Default::default() }).unwrap()), [("fares", 3), ("staff", 2)]);
    let one = w.p.word_frequencies(&WordSpec { limit: 1, ..Default::default() }).unwrap();
    assert_eq!((names(&one), one.distinct_words), (vec![("fares", 3)], 7), "the list is cut, the count of different words is not");
    let notes = w.p.word_frequencies(&WordSpec { source_ids: Some(vec![w.notes]), ..Default::default() }).unwrap();
    assert_eq!(names(&notes), [("staff", 2), ("helpful", 1), ("kind", 1)]);
    assert_eq!(notes.documents, 1);
}

#[test]
fn only_the_words_inside_coded_passages_can_be_counted_and_a_code_includes_its_sub_codes() {
    let mut w = world();
    let money = w.p.add_code(&NewCode::new("Money")).unwrap();
    let mut price = NewCode::new("Price");
    price.parent_id = Some(money);
    let price = w.p.add_code(&price).unwrap();
    let ada = w.p.add_coder(&NewCoder::human("Ada")).unwrap();
    // "Fares are too high." is the first nineteen characters of the first document.
    w.p.apply_coding(&NewCoding::new(w.riders, 0, 19, price, ada)).unwrap();
    for chosen in [price, money] {
        let f = w.p.word_frequencies(&WordSpec { code_ids: vec![chosen], ..Default::default() }).unwrap();
        assert_eq!(names(&f), [("fares", 1), ("high", 1)], "code {chosen}");
        assert_eq!((f.total_words, f.counted_words), (4, 2));
    }
    assert!(w.p.word_frequencies(&WordSpec { code_ids: vec![9999], ..Default::default() }).is_err(), "a code that is not there");
}

#[test]
fn a_picture_is_not_material_to_count() {
    let mut w = world();
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    bytes.extend_from_slice(&100u32.to_be_bytes());
    bytes.extend_from_slice(&50u32.to_be_bytes());
    bytes.extend_from_slice(&[8, 2, 0, 0, 0, 1, 1, 1, 1]);
    let image = read_image("street.png", bytes.clone()).unwrap();
    let text = prismqda_engine::default_description(&image.file_name, image.width, image.height);
    let len = text.chars().count() as i64;
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Street".into(),
            source_type: SourceType::Other,
            folder: String::new(),
            text,
            runs: vec![],
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: Some(sha256_hex(&bytes)),
            settings: Value::Null,
            warnings: vec![],
        }],
        images: vec![ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    w.p.apply_import(&batch).unwrap();
    let f = w.p.word_frequencies(&WordSpec::default()).unwrap();
    assert_eq!((f.documents, f.total_words), (2, 22), "the picture's description is not counted");
    assert!(!f.words.iter().any(|x| x.word == "pixels" || x.word == "description"));
    assert!(w.p.keywords_in_context(&ContextSpec { query: "pixels".into(), source_ids: None, case_id: None, words_around: 3, limit: 10 }).unwrap().items.is_empty());
}

fn spec(query: &str, around: usize, limit: usize) -> ContextSpec {
    ContextSpec { query: query.into(), source_ids: None, case_id: None, words_around: around, limit }
}

#[test]
fn a_word_is_found_wherever_it_is_used_with_the_words_round_it() {
    let w = world();
    let found = w.p.keywords_in_context(&spec("FARES", 2, 10)).unwrap();
    assert_eq!((found.total, found.items.len(), found.documents), (3, 3, 1), "capitals do not matter, and the word is used in one document");
    let first = &found.items[0];
    assert_eq!((first.source_name.as_str(), first.start, first.end), ("Riders", 0, 5));
    assert_eq!((first.left.as_str(), first.hit.as_str(), first.right.as_str()), ("", "Fares", "are too"));
    let second = &found.items[1];
    assert_eq!((second.left.as_str(), second.hit.as_str(), second.right.as_str()), ("too high.", "Fares", "are a"));
    let third = &found.items[2];
    assert_eq!((third.left.as_str(), third.right.as_str()), ("like the", ""), "the end of the document ends the context");
    assert_eq!(&RIDERS.chars().skip(third.start as usize).take((third.end - third.start) as usize).collect::<String>(), "fares", "the offsets point at the word itself");

    // A phrase is its words in order; a part of a word is not the word; too many are counted but not all returned.
    let phrase = w.p.keywords_in_context(&spec("worry and", 1, 10)).unwrap();
    assert_eq!((phrase.total, phrase.items[0].hit.as_str(), phrase.items[0].left.as_str(), phrase.items[0].right.as_str()), (1, "worry, and", "a", "I"));
    assert_eq!(w.p.keywords_in_context(&spec("fare", 2, 10)).unwrap().total, 0, "fare is not fares");
    let cut = w.p.keywords_in_context(&spec("fares", 2, 2)).unwrap();
    assert_eq!((cut.items.len(), cut.total), (2, 3));
    assert!(w.p.keywords_in_context(&spec("  ,, ", 2, 10)).is_err(), "nothing to look for");
}

#[test]
fn contexts_can_be_limited_to_some_documents() {
    let w = world();
    let mut s = spec("staff", 1, 10);
    assert_eq!(w.p.keywords_in_context(&s).unwrap().total, 2);
    s.source_ids = Some(vec![w.riders]);
    let none = w.p.keywords_in_context(&s).unwrap();
    assert_eq!((none.total, none.documents), (0, 0));
}
