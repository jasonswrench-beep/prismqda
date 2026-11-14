//! Reference libraries (RIS, BibTeX, CSL JSON), e-mail (.eml, .mbox), saved web pages (.mhtml), and news feeds. All text here is invented.

use prismqda_engine::{AppliesTo, ImportBatch, Level, NewProject, OpenOptions, Project, SourceType};
use prismqda_import::{open_bytes, Format, ImportError, ImportOptions, Opened};
use tempfile::TempDir;

fn batch(name: &str, bytes: &[u8]) -> ImportBatch {
    match open_bytes(name, bytes, &ImportOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}")) {
        Opened::Batch(b) => b,
        Opened::Table(_) => panic!("{name} should give documents, not a table"),
    }
}

fn error(name: &str, bytes: &[u8]) -> ImportError {
    open_bytes(name, bytes, &ImportOptions::default()).err().unwrap_or_else(|| panic!("{name} should have failed"))
}

fn case_value<'a>(b: &'a ImportBatch, case: usize, name: &str) -> Option<&'a str> {
    b.cases[case].attrs.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

fn kinds(b: &ImportBatch, source: usize) -> Vec<&str> {
    b.sources[source].warnings.iter().map(|w| w.kind.as_str()).collect()
}

fn stored(b: &ImportBatch) -> (TempDir, Project) {
    let dir = TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut p = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    p.apply_import(b).unwrap();
    (dir, p)
}

// ---------------------------------------------------------------- formats are told apart by name

#[test]
fn the_new_kinds_of_file_are_known_by_their_endings() {
    for (name, format) in [
        ("a.ris", Format::Ris),
        ("A.BIB", Format::Bib),
        ("a.bibtex", Format::Bib),
        ("a.eml", Format::Eml),
        ("a.mbox", Format::Mbox),
        ("a.mht", Format::Mhtml),
        ("a.mhtml", Format::Mhtml),
        ("a.rss", Format::Feed),
        ("a.atom", Format::Feed),
        ("a.xml", Format::Feed),
    ] {
        assert_eq!(Format::from_name(name), Some(format), "{name}");
    }
}

// ---------------------------------------------------------------- RIS

const RIS: &str = "\
TY  - JOUR
AU  - Hernandez, Maria
AU  - Li, Wei
TI  - Bus reliability and daily stress
JO  - Journal of Urban Mobility
PY  - 2021///
VL  - 12
IS  - 3
SP  - 45
EP  - 62
DO  - https://doi.org/10.1234/jum.2021.045
AB  - Riders describe delays as a source of stress.
 The effect is strongest in the morning.
KW  - reliability
KW  - stress; commuting
ER  -

TY  - BOOK
AU  - Okafor, Ngozi
T1  - Cities in motion
PB  - Harbor Press
DA  - 2019/05/01/
ER  -

TY  - GEN
TI  - A last record with no closing line
";

#[test]
fn a_ris_file_gives_a_source_and_a_case_for_each_reference() {
    let b = batch("library.ris", RIS.as_bytes());
    assert_eq!(b.sources.len(), 3, "the last record counts even with no ER line");
    assert_eq!(b.cases.len(), 3);
    assert_eq!(b.sources[0].name, "Hernandez & Li 2021 \u{2013} Bus reliability and daily stress");
    assert_eq!(b.sources[1].name, "Okafor 2019 \u{2013} Cities in motion");
    assert_eq!(b.sources[2].name, "A last record with no closing line");
    assert!(b.sources.iter().all(|s| s.folder == "References" && s.source_type == SourceType::Document));

    let text = &b.sources[0].text;
    assert!(text.starts_with("Bus reliability and daily stress\n\nAuthors: Maria Hernandez; Wei Li\nYear: 2021\nPublished in: Journal of Urban Mobility (volume 12, issue 3, pages 45\u{2013}62)\nType: Journal article\nDOI: 10.1234/jum.2021.045"), "{text}");
    assert!(text.contains("Abstract\n\nRiders describe delays as a source of stress. The effect is strongest in the morning."), "a continued line joins the abstract: {text}");
    assert!(text.ends_with("Keywords: reliability; stress; commuting"), "{text}");
    assert_eq!(kinds(&b, 0), Vec::<&str>::new());
    assert_eq!(kinds(&b, 1), ["no_abstract"], "a reference with no abstract says so");

    assert_eq!(case_value(&b, 0, "Authors"), Some("Maria Hernandez; Wei Li"));
    assert_eq!(case_value(&b, 0, "Year"), Some("2021"));
    assert_eq!(case_value(&b, 0, "Reference type"), Some("Journal article"));
    assert_eq!(case_value(&b, 0, "DOI"), Some("10.1234/jum.2021.045"));
    assert_eq!(case_value(&b, 1, "Year"), Some("2019"), "a year is found in a DA date too");
    assert_eq!(case_value(&b, 1, "DOI"), None, "a value that is not there is not made up");
    let year = b.case_attributes.iter().find(|a| a.name == "Year").unwrap();
    assert_eq!((year.applies_to, year.level), (AppliesTo::Case, Level::Interval));

    let (_dir, p) = stored(&b);
    assert_eq!(p.list_sources().unwrap().len(), 3);
    assert_eq!(p.list_cases().unwrap().len(), 3);
}

#[test]
fn a_ris_file_with_no_references_is_refused() {
    assert!(error("empty.ris", b"nothing here\n").to_string().contains("no references"));
}

// ---------------------------------------------------------------- BibTeX

const BIB: &str = r#"
@comment{ this is skipped @article{ignored, title={x}} }
@string{jum = "Journal of Urban Mobility"}
@article{hernandez2021,
  author  = {Hern{\'a}ndez, Mar{\'\i}a and M{\"u}ller, J{\"o}rg and {World Health Organization}},
  title   = {Bus {R}eliability and {Daily} Stress},
  journal = jum,
  year    = 2021,
  volume  = {12},
  number  = {3},
  pages   = {45--62},
  doi     = {10.1234/jum.2021.045},
  keywords = {reliability, stress; commuting},
  abstract = "Riders describe delays as stress. " # "It is worst in the morning."
}
@book{okafor2019, author = "Okafor, Ngozi", title = "Cities in motion", publisher = "Harbor Press", year = {2019}}
"#;

#[test]
fn a_bibtex_file_is_read_with_macros_concatenation_and_latex() {
    let b = batch("library.bib", BIB.as_bytes());
    assert_eq!(b.sources.len(), 2, "the comment and the macro are not references");
    let text = &b.sources[0].text;
    assert!(text.starts_with("Bus Reliability and Daily Stress\n\nAuthors: Mar\u{ed}a Hern\u{e1}ndez; J\u{f6}rg M\u{fc}ller; World Health Organization\nYear: 2021\nPublished in: Journal of Urban Mobility (volume 12, issue 3, pages 45\u{2013}62)"), "{text}");
    assert!(text.contains("Riders describe delays as stress. It is worst in the morning."), "{text}");
    assert!(text.ends_with("Keywords: reliability; stress; commuting"));
    assert_eq!(b.sources[0].name, "Hern\u{e1}ndez et al. 2021 \u{2013} Bus Reliability and Daily Stress");
    assert_eq!(case_value(&b, 0, "Published in"), Some("Journal of Urban Mobility"));
    assert_eq!(b.sources[1].name, "Okafor 2019 \u{2013} Cities in motion");
    assert_eq!(case_value(&b, 1, "Reference type"), Some("Book"));
    assert!(unicode_normalization::is_nfc(&b.sources[0].text), "the text is NFC, as every offset needs");
}

#[test]
fn bibtex_that_is_cut_off_still_gives_what_was_complete() {
    let b = batch("cut.bib", b"@article{one, title={Whole one}, year=2020}\n@article{two, title={Cut o");
    assert_eq!(b.sources.len(), 2);
    assert_eq!(b.sources[0].name, "2020 \u{2013} Whole one", "no authors, so the year leads");
    assert!(error("no.bib", b"just words").to_string().contains("no references"));
}

// ---------------------------------------------------------------- CSL JSON

#[test]
fn a_csl_json_library_is_read_and_a_reddit_export_is_still_a_reddit_export() {
    let csl = r#"[
      {"id":"a","type":"article-journal","title":"Bus reliability","author":[{"family":"Hernandez","given":"Maria"},{"literal":"Transit Lab"}],
       "issued":{"date-parts":[[2021,3,15]]},"container-title":"Journal of Urban Mobility","DOI":"10.1/x","keyword":"reliability, stress","abstract":"Delays are stressful."},
      {"id":"b","type":"book","title":"Cities in motion","issued":{"raw":"May 2019"}}
    ]"#;
    let b = batch("library.json", csl.as_bytes());
    assert_eq!(b.sources.len(), 2);
    assert_eq!(b.sources[0].name, "Hernandez & Transit Lab 2021 \u{2013} Bus reliability");
    assert_eq!(case_value(&b, 0, "Authors"), Some("Maria Hernandez; Transit Lab"));
    assert_eq!(case_value(&b, 1, "Year"), Some("2019"));
    assert_eq!(kinds(&b, 1), ["no_abstract"]);

    let reddit = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/social/reddit.json")).unwrap();
    let social = batch("thread.json", &reddit);
    assert!(social.sources.iter().all(|s| s.source_type == SourceType::Social), "a Reddit listing is not mistaken for references");
}

// ---------------------------------------------------------------- e-mail

const SIMPLE_EML: &str = "From: =?UTF-8?B?TWFyw61hIEzDs3Bleg==?= <maria@example.org>\r\nTo: Wei Li <wei@example.org>\r\nDate: Tue, 3 Nov 2026 14:05:00 +0000\r\nSubject: =?utf-8?Q?Caf=C3=A9_visit?=\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: quoted-printable\r\n\r\nHello Wei,=0D\n\r\nThe caf=C3=A9 was lovely and the staff were kind.\r\n\r\n> You wrote earlier that it was noisy.\r\n";

#[test]
fn a_single_message_becomes_a_source_with_its_header_lines_and_a_case_with_variables() {
    let b = batch("visit.eml", SIMPLE_EML.as_bytes());
    assert_eq!(b.sources.len(), 1);
    let s = &b.sources[0];
    assert_eq!(s.name, "Caf\u{e9} visit \u{2013} Mar\u{ed}a L\u{f3}pez \u{2013} 2026-11-03");
    assert_eq!(s.folder, "Email");
    assert!(s.text.starts_with("From: Mar\u{ed}a L\u{f3}pez <maria@example.org>\nTo: Wei Li <wei@example.org>\nDate: Tue, 3 Nov 2026 14:05:00 +0000\nSubject: Caf\u{e9} visit\n\nHello Wei,"), "{}", s.text);
    assert!(s.text.contains("The caf\u{e9} was lovely and the staff were kind."), "{}", s.text);
    assert!(s.text.contains("> You wrote earlier that it was noisy."), "quoted lines are kept");
    assert_eq!(case_value(&b, 0, "Date"), Some("2026-11-03"));
    assert_eq!(case_value(&b, 0, "Subject"), Some("Caf\u{e9} visit"));
    let date = b.case_attributes.iter().find(|a| a.name == "Date").unwrap();
    assert_eq!(date.level, Level::Date);
    let (_dir, p) = stored(&b);
    assert_eq!(p.list_sources().unwrap()[0].name, s.name);
}

#[test]
fn alternatives_give_the_plain_text_and_attachments_are_named_not_imported() {
    let eml = "From: a@example.org\nTo: b@example.org\nSubject: Report\nMIME-Version: 1.0\nContent-Type: multipart/mixed; boundary=\"outer\"\n\n--outer\nContent-Type: multipart/alternative; boundary=\"inner\"\n\n--inner\nContent-Type: text/plain; charset=iso-8859-1\nContent-Transfer-Encoding: quoted-printable\n\nThe r=E9sum=E9 is attached.\n--inner\nContent-Type: text/html\n\n<p>The <b>HTML</b> copy of the same words.</p>\n--inner--\n--outer\nContent-Type: application/pdf; name=\"resume.pdf\"\nContent-Disposition: attachment; filename=\"resume.pdf\"\nContent-Transfer-Encoding: base64\n\nJVBERi0xLjQK\n--outer--\n";
    let b = batch("report.eml", eml.as_bytes());
    let s = &b.sources[0];
    assert!(s.text.contains("The r\u{e9}sum\u{e9} is attached."), "the plain text part, read in its own character set: {}", s.text);
    assert!(!s.text.contains("HTML copy"), "not the same words twice");
    let w = s.warnings.iter().find(|w| w.kind == "skipped_attachment").expect("the attachment is reported");
    assert!(w.message.contains("resume.pdf") && w.message.starts_with("1 attachment was not imported"), "{}", w.message);
}

#[test]
fn a_message_with_only_html_is_read_as_text() {
    let eml = "From: a@example.org\nSubject: Newsletter\nContent-Type: text/html; charset=utf-8\n\n<html><body><h1>News</h1><p>Hello <b>reader</b>, the library is open.</p></body></html>\n";
    let b = batch("news.eml", eml.as_bytes());
    assert!(b.sources[0].text.contains("Hello reader, the library is open."), "{}", b.sources[0].text);
    assert!(!b.sources[0].text.contains('<'));
}

#[test]
fn something_that_is_not_a_message_is_refused() {
    let e = error("note.eml", b"This is only a note with no header lines at all.\n");
    assert!(e.to_string().contains("does not look like an e-mail message"), "{e}");
}

#[test]
fn a_mailbox_gives_one_source_per_message_and_a_from_line_in_a_body_does_not_split_one() {
    let mbox = "From maria@example.org Tue Nov  3 14:05:00 2026\nFrom: maria@example.org\nSubject: One\nDate: Tue, 3 Nov 2026 14:05:00 +0000\n\nFirst body.\nFrom the start, it was good.\n\nFrom wei@example.org Wed Nov  4 09:00:00 2026\nFrom: wei@example.org\nSubject: Two\n\nSecond body.\n\nFrom nobody Thu Nov  5 09:00:00 2026\nJust some text with no headers.\n\nFrom lee@example.org Fri Nov  6 09:00:00 2026\nFrom: lee@example.org\nSubject: Three\n\nThird body.\n";
    let b = batch("box.mbox", mbox.as_bytes());
    assert_eq!(b.sources.len(), 3, "three messages; the entry with no headers is left out");
    assert!(b.sources[0].text.contains("From the start, it was good."), "a line starting 'From ' inside a body stays in it");
    assert_eq!(b.sources.iter().map(|s| s.name.starts_with(['O', 'T'])).collect::<Vec<_>>(), [true, true, true]);
    assert!(b.sources[0].warnings.iter().any(|w| w.kind == "skipped_message" && w.message.starts_with("1 entry in the mailbox")), "{:?}", b.sources[0].warnings);
    assert_eq!(b.cases.len(), 3);
}

// ---------------------------------------------------------------- a saved web page

#[test]
fn a_saved_web_page_gives_its_text_and_keeps_its_address_and_date_as_variables() {
    let page = "From: <Saved by a browser>\nSnapshot-Content-Location: https://example.org/news/library-opens\nSubject: =?utf-8?Q?Library_opens_late?=\nDate: Tue, 3 Nov 2026 14:05:00 +0000\nMIME-Version: 1.0\nContent-Type: multipart/related; type=\"text/html\"; boundary=\"b\"\n\n--b\nContent-Type: text/html; charset=utf-8\nContent-Transfer-Encoding: quoted-printable\nContent-Location: https://example.org/news/library-opens\n\n<html><body><h1>Library opens late</h1><p>The library will open at ten on Mondays.</p><img src=3D\"x.png\"></body></html>\n--b\nContent-Type: image/png\nContent-Transfer-Encoding: base64\nContent-Location: https://example.org/x.png\n\niVBORw0KGgo=\n--b--\n";
    let b = batch("page.mhtml", page.as_bytes());
    assert_eq!(b.sources.len(), 1);
    assert_eq!(b.sources[0].name, "Library opens late");
    assert_eq!(b.sources[0].folder, "Web pages");
    assert!(b.sources[0].text.contains("The library will open at ten on Mondays."), "{}", b.sources[0].text);
    assert_eq!(case_value(&b, 0, "Address"), Some("https://example.org/news/library-opens"));
    assert_eq!(case_value(&b, 0, "Saved"), Some("2026-11-03"));
    assert!(b.sources[0].warnings.iter().any(|w| w.kind == "skipped_resource"), "the picture saved with the page is reported");
}

// ---------------------------------------------------------------- news feeds

const RSS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:content="http://purl.org/rss/1.0/modules/content/">
 <channel>
  <title>City Transit News</title>
  <item>
   <title>Fares &amp; passes change</title>
   <link>https://example.org/fares</link>
   <dc:creator>Maya Lopez</dc:creator>
   <pubDate>Tue, 3 Nov 2026 14:05:00 GMT</pubDate>
   <description><![CDATA[<p>Short summary.</p>]]></description>
   <content:encoded><![CDATA[<p>The monthly pass costs more from January.</p><p>Students keep a discount.</p>]]></content:encoded>
  </item>
  <item>
   <title>Night buses</title>
   <description>Plain words in the description.</description>
  </item>
 </channel>
</rss>"#;

#[test]
fn an_rss_feed_gives_an_item_per_source_with_its_variables() {
    let b = batch("feed.rss", RSS.as_bytes());
    assert_eq!(b.sources.len(), 2);
    assert_eq!(b.sources[0].name, "Fares & passes change \u{2013} 2026-11-03");
    assert_eq!(b.sources[0].folder, "News");
    let text = &b.sources[0].text;
    assert!(text.starts_with("Fares & passes change\n\nAuthor: Maya Lopez\nPublished: 2026-11-03\nFeed: City Transit News\nLink: https://example.org/fares\n\nThe monthly pass costs more from January."), "{text}");
    assert!(text.contains("Students keep a discount.") && !text.contains("Short summary"), "the whole article is preferred to its summary");
    assert_eq!(case_value(&b, 0, "Feed"), Some("City Transit News"));
    assert_eq!(case_value(&b, 0, "Published"), Some("2026-11-03"));
    assert_eq!(b.sources[1].name, "Night buses");
    assert!(b.sources[1].text.contains("Plain words in the description."));
}

#[test]
fn an_atom_feed_is_read_too() {
    let atom = r#"<?xml version="1.0"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Campus Voices</title>
  <entry>
    <title>Students on parking</title>
    <link href="https://example.org/parking" rel="alternate"/>
    <link href="https://example.org/parking.json" rel="enclosure"/>
    <updated>2026-10-30T08:00:00Z</updated>
    <author><name>Jo Park</name></author>
    <summary>&lt;p&gt;Parking is hard &amp;#8212; and costly.&lt;/p&gt;</summary>
  </entry>
</feed>"#;
    let b = batch("voices.atom", atom.as_bytes());
    assert_eq!(b.sources.len(), 1);
    assert_eq!(case_value(&b, 0, "Link"), Some("https://example.org/parking"), "the alternate link, not the enclosure");
    assert_eq!(case_value(&b, 0, "Author"), Some("Jo Park"));
    assert_eq!(case_value(&b, 0, "Published"), Some("2026-10-30"));
    assert!(b.sources[0].text.contains("Parking is hard \u{2014} and costly."), "{}", b.sources[0].text);
}

#[test]
fn xml_that_is_not_a_feed_and_hostile_xml_are_refused_or_defanged() {
    let e = error("data.xml", b"<?xml version=\"1.0\"?><catalog><book>x</book></catalog>");
    assert!(e.to_string().contains("not a news feed"), "{e}");
    assert!(matches!(error("empty.rss", b"<rss><channel><title>t</title></channel></rss>"), ImportError::Format(_)));
    // A custom entity is never expanded, so a "billion laughs" file cannot grow.
    let hostile = "<?xml version=\"1.0\"?><!DOCTYPE rss [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;\">]><rss><channel><title>t</title><item><title>Safe</title><description>&b; text</description></item></channel></rss>";
    let b = batch("hostile.rss", hostile.as_bytes());
    assert!(!b.sources[0].text.contains("aaaa"), "{}", b.sources[0].text);
}
