//! PDF pages: a PDF is a document of the words read from it with the file kept beside them, and a rectangle on one of its pages can be
//! coded as a stretch of words is. The engine never draws a page (that is `prismqda-pdfview`); these tests give it files that only look
//! like PDFs to it and check what it keeps, which areas it accepts, and how an area on one page is kept apart from the same shape on another.

use prismqda_engine::{
    CodeEdit, FormatRun, ImportBatch, ImportedSource, ImportedUnit, NewCode, NewCoder, NewProject, NewRegion, NewSource, OpenOptions, PdfAttachment, PdfData, Project, QuerySpec,
    RunKind, SegmentFilter, Shape, SourceType, UnitKind,
};
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

const FILE: &[u8] = b"%PDF-1.4 a file this test never draws, three pages long";

/// The words of a three-page PDF whose second page has no words (a picture of a table, say).
const WORDS: &str = "First page words.\n\nThird page words.";

fn pdf_data(name: &str, pages: usize) -> PdfData {
    PdfData { file_name: name.to_string(), pages: (0..pages).map(|i| (612.0, 792.0 + i as f64)).collect(), bytes: FILE.to_vec() }
}

fn pdf_batch(name: &str, with_file: bool) -> ImportBatch {
    let len = WORDS.chars().count() as i64;
    let first_end = "First page words.".chars().count() as i64;
    ImportBatch {
        sources: vec![ImportedSource {
            name: name.to_string(),
            source_type: SourceType::Pdf,
            folder: String::new(),
            text: WORDS.to_string(),
            runs: vec![
                FormatRun { start: 0, end: first_end, kind: RunKind::Page, level: None, page: Some(1) },
                FormatRun { start: first_end + 2, end: len, kind: RunKind::Page, level: None, page: Some(3) },
            ],
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: None,
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        pdfs: if with_file { vec![PdfAttachment { source: 0, pdf: pdf_data(&format!("{name}.pdf"), 3) }] } else { vec![] },
        ..Default::default()
    }
}

struct W {
    p: Project,
    _dir: TempDir,
    report: i64,
    old: i64,
    words: i64,
    joy: i64,
    fear: i64,
    ada: i64,
}

fn world() -> W {
    let dir = TempDir::new().unwrap();
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &opts()).unwrap();
    let report = p.apply_import(&pdf_batch("Report", true)).unwrap().source_ids[0];
    let old = p.apply_import(&pdf_batch("Old report", false)).unwrap().source_ids[0];
    let words = p.add_source(&NewSource::new("Notes", SourceType::Document, "Some words.")).unwrap();
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let fear = p.add_code(&NewCode::new("Fear")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    W { p, _dir: dir, report, old, words, joy, fear, ada }
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Shape {
    Shape::Rectangle { x, y, w, h }
}

fn on_page(source: i64, page: i64, shape: Shape, code: i64, coder: i64) -> NewRegion {
    let mut r = NewRegion::new(source, shape, code, coder);
    r.page = page;
    r
}

#[test]
fn a_pdf_is_kept_exactly_as_the_file_was_with_the_size_of_every_page() {
    let w = world();
    let row = w.p.get_pdf(w.report).unwrap().unwrap();
    assert_eq!((row.file_name.as_str(), row.page_count, row.size_bytes), ("Report.pdf", 3, FILE.len() as i64));
    assert_eq!(row.pages.len(), 3);
    assert_eq!((row.pages[0].page, row.pages[0].width, row.pages[0].height), (1, 612.0, 792.0));
    assert_eq!(row.pages[2].height, 794.0);
    assert_eq!(w.p.read_pdf_bytes(w.report).unwrap(), FILE, "byte for byte");
    // The listing of documents says which keep a PDF; a document that does not keep one says nothing, and so does a document of words.
    let list = w.p.list_sources().unwrap();
    let find = |id: i64| list.iter().find(|s| s.id == id).unwrap();
    assert_eq!(find(w.report).pdf.as_ref().map(|p| p.page_count), Some(3));
    assert!(find(w.old).pdf.is_none() && find(w.words).pdf.is_none());
    assert!(w.p.get_pdf(w.old).unwrap().is_none());
    assert!(w.p.read_pdf_bytes(w.old).unwrap_err().to_string().contains("no PDF kept"));
    assert_eq!(w.p.source_detail(w.report).unwrap().text, WORDS, "its words are the words of an ordinary document");
}

#[test]
fn an_area_is_coded_on_a_page_the_pdf_has_and_nowhere_else() {
    let mut w = world();
    let id = w.p.apply_region(&on_page(w.report, 2, rect(0.1, 0.2, 0.5, 0.3), w.joy, w.ada)).unwrap();
    let row = w.p.get_region(id).unwrap();
    assert_eq!((row.page, row.source_id), (2, w.report));
    assert!(row.text.starts_with("Page 2, Rectangle, 10% to 60% across and 20% to 50% down"), "{}", row.text);
    assert!(row.text.ends_with("of the page"), "{}", row.text);

    let e = w.p.apply_region(&on_page(w.report, 4, rect(0.1, 0.1, 0.2, 0.2), w.joy, w.ada)).unwrap_err().to_string();
    assert!(e.contains("3 pages") && e.contains("page 4"), "{e}");
    assert!(w.p.apply_region(&on_page(w.report, 0, rect(0.1, 0.1, 0.2, 0.2), w.joy, w.ada)).is_err(), "pages count from 1");
    let e = w.p.apply_region(&on_page(w.report, 1, Shape::Ellipse { x: 0.1, y: 0.1, w: 0.2, h: 0.2 }, w.joy, w.ada)).unwrap_err().to_string();
    assert!(e.contains("rectangle"), "{e}");
    // A PDF imported with no file kept has no pages to be on, and a document of words never has.
    let e = w.p.apply_region(&on_page(w.old, 1, rect(0.1, 0.1, 0.2, 0.2), w.joy, w.ada)).unwrap_err().to_string();
    assert!(e.contains("not a picture or a PDF whose pages are kept"), "{e}");
    assert!(w.p.apply_region(&on_page(w.words, 1, rect(0.1, 0.1, 0.2, 0.2), w.joy, w.ada)).is_err());
    assert_eq!(w.p.list_regions(w.report).unwrap().len(), 1, "only the first was kept");
}

#[test]
fn the_same_shape_on_two_pages_is_two_areas_in_page_order_and_the_page_never_moves() {
    let mut w = world();
    let shape = rect(0.1, 0.1, 0.4, 0.4);
    let third = w.p.apply_region(&on_page(w.report, 3, shape.clone(), w.joy, w.ada)).unwrap();
    let first = w.p.apply_region(&on_page(w.report, 1, shape.clone(), w.joy, w.ada)).unwrap();
    let pages: Vec<(i64, i64)> = w.p.list_regions(w.report).unwrap().iter().map(|r| (r.id, r.page)).collect();
    assert_eq!(pages, [(first, 1), (third, 3)], "a PDF's areas are listed by page, then top to bottom");
    // Moving an area, giving it another code, or rating it leaves it on its page.
    w.p.reshape_region(third, &rect(0.2, 0.2, 0.3, 0.3)).unwrap();
    w.p.change_region_code(third, w.fear).unwrap();
    assert_eq!(w.p.get_region(third).unwrap().page, 3);
    // The history of an area says its page.
    assert_eq!(w.p.region_history(third).unwrap()[0].after.as_ref().unwrap()["page"], 3);
}

#[test]
fn a_mutually_exclusive_group_is_kept_page_by_page() {
    let mut w = world();
    w.p.update_code(w.joy, &CodeEdit { exclusive_group: Some("mood".to_string()), ..CodeEdit::default() }).unwrap();
    w.p.update_code(w.fear, &CodeEdit { exclusive_group: Some("mood".to_string()), ..CodeEdit::default() }).unwrap();
    w.p.apply_region(&on_page(w.report, 1, rect(0.1, 0.1, 0.4, 0.4), w.joy, w.ada)).unwrap();
    // The same ground on another page is another place, so the other code of the group may be given there ...
    w.p.apply_region(&on_page(w.report, 2, rect(0.1, 0.1, 0.4, 0.4), w.fear, w.ada)).unwrap();
    // ... and not on the page that already has Joy there.
    let e = w.p.apply_region(&on_page(w.report, 1, rect(0.2, 0.2, 0.4, 0.4), w.fear, w.ada)).unwrap_err();
    assert!(matches!(e, prismqda_engine::Error::ExclusiveGroup(_)), "{e}");
}

#[test]
fn undo_puts_a_removed_area_back_on_its_page() {
    let mut w = world();
    let id = w.p.apply_region(&on_page(w.report, 3, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    w.p.remove_region(id).unwrap();
    assert!(w.p.list_regions(w.report).unwrap().is_empty());
    assert_eq!(w.p.undo().unwrap().unwrap().action, "region.remove");
    let back = w.p.get_region(id).unwrap();
    assert_eq!(back.page, 3, "it comes back where it was");
}

#[test]
fn coded_segments_and_queries_say_which_page_an_area_is_on() {
    let mut w = world();
    w.p.apply_region(&on_page(w.report, 2, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    w.p.apply_region(&on_page(w.report, 3, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    w.p.apply_coding(&prismqda_engine::NewCoding::new(w.report, 0, 5, w.joy, w.ada)).unwrap();
    let segments = w.p.coded_segments(&SegmentFilter::default()).unwrap();
    let areas: Vec<_> = segments.iter().filter(|s| s.kind == "image").collect();
    assert_eq!(areas.len(), 2);
    assert_eq!(areas.iter().map(|s| s.page).collect::<Vec<_>>(), [Some(2), Some(3)]);
    assert!(areas[0].text.starts_with("Page 2, Rectangle"), "{}", areas[0].text);
    assert!(segments.iter().filter(|s| s.kind == "text").all(|s| s.page.is_none()), "words have no page of this kind");

    // A query counts the same shape on two pages as two excerpts, and tells each in words with its page.
    let spec: QuerySpec = serde_json::from_value(serde_json::json!({ "rule": { "type": "code", "code_ids": [w.joy] }, "result_unit": "excerpts" })).unwrap();
    let result = w.p.run_query(&spec).unwrap();
    let areas: Vec<_> = result.items.iter().filter(|i| i.kind == "image").collect();
    assert_eq!(areas.len(), 2, "two pages, two excerpts");
    assert_eq!(areas.iter().map(|i| i.page).collect::<Vec<_>>(), [Some(2), Some(3)]);
    assert!(areas.iter().all(|i| i.text.contains("of the page")), "{:?}", areas.iter().map(|i| &i.text).collect::<Vec<_>>());
}

#[test]
fn a_pdf_attached_later_must_be_the_file_the_words_came_from() {
    let mut w = world();
    // The old report was imported with no file kept; its words came from page 3, so a file of two pages is not the same file.
    let e = w.p.attach_pdf(w.old, &pdf_data("short.pdf", 2)).unwrap_err().to_string();
    assert!(e.contains("2 pages") && e.contains("page 3"), "{e}");
    assert!(w.p.get_pdf(w.old).unwrap().is_none(), "nothing was kept");
    let row = w.p.attach_pdf(w.old, &pdf_data("old.pdf", 3)).unwrap();
    assert_eq!((row.page_count, row.file_name.as_str()), (3, "old.pdf"));
    w.p.apply_region(&on_page(w.old, 2, rect(0.1, 0.1, 0.3, 0.3), w.joy, w.ada)).unwrap();
    assert!(w.p.events(10_000).unwrap().iter().any(|e| e.action == "pdf.attach"), "attaching a file is in the log");
    // A document that already keeps its PDF, and one that was never a PDF, cannot have another attached.
    assert!(w.p.attach_pdf(w.report, &pdf_data("again.pdf", 3)).unwrap_err().to_string().contains("already keeps"));
    assert!(w.p.attach_pdf(w.words, &pdf_data("x.pdf", 3)).unwrap_err().to_string().contains("imported from a PDF"));
}

#[test]
fn a_pdf_with_no_pages_or_no_bytes_or_a_page_with_no_size_is_refused_whole() {
    let mut w = world();
    for (what, data) in [
        ("no pages", PdfData { file_name: "a.pdf".into(), pages: vec![], bytes: FILE.to_vec() }),
        ("no bytes", PdfData { file_name: "a.pdf".into(), pages: vec![(612.0, 792.0)], bytes: vec![] }),
        ("a page with no size", PdfData { file_name: "a.pdf".into(), pages: vec![(612.0, 0.0)], bytes: FILE.to_vec() }),
        ("a page of NaN", PdfData { file_name: "a.pdf".into(), pages: vec![(f64::NAN, 792.0)], bytes: FILE.to_vec() }),
    ] {
        let mut batch = pdf_batch("Bad", false);
        batch.pdfs = vec![PdfAttachment { source: 0, pdf: data }];
        assert!(w.p.apply_import(&batch).is_err(), "{what}");
    }
    assert_eq!(w.p.list_sources().unwrap().len(), 3, "an import that is refused leaves nothing behind");
}

#[test]
fn merging_a_copy_brings_the_pdf_and_its_areas_on_their_pages_and_twice_adds_nothing() {
    let mut w = world();
    let copy_dir = TempDir::new().unwrap();
    let copy_path = copy_dir.path().join("copy.pqda");
    {
        // The copy is a project that imported the same words, with the file, and coded two areas.
        let mut theirs = Project::create(&copy_path, &NewProject::new("Copy"), &opts()).unwrap();
        let src = theirs.apply_import(&pdf_batch("Old report", true)).unwrap().source_ids[0];
        let joy = theirs.add_code(&NewCode::new("Joy")).unwrap();
        let ben = theirs.add_coder(&NewCoder::human("Ben")).unwrap();
        theirs.apply_region(&on_page(src, 2, rect(0.1, 0.1, 0.3, 0.3), joy, ben)).unwrap();
        theirs.apply_region(&on_page(src, 3, rect(0.1, 0.1, 0.3, 0.3), joy, ben)).unwrap();
    }
    // Here "Old report" matches by name and words but keeps no file: the file comes with the merge, so the areas have pages to be on.
    assert!(w.p.get_pdf(w.old).unwrap().is_none());
    let report = w.p.merge_copy(&copy_path).unwrap();
    assert_eq!(report.areas_added, 2, "{report:?}");
    let pdf = w.p.get_pdf(w.old).unwrap().expect("the file was brought");
    assert_eq!(pdf.page_count, 3);
    let areas = w.p.list_regions(w.old).unwrap();
    assert_eq!(areas.iter().map(|r| r.page).collect::<Vec<_>>(), [2, 3]);
    let again = w.p.merge_copy(&copy_path).unwrap();
    assert_eq!((again.areas_added, again.areas_already_here), (0, 2));
    assert_eq!(w.p.list_regions(w.old).unwrap().len(), 2);
}
