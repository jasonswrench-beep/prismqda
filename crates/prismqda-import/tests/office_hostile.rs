//! Damaged and hostile input. Every importer must answer with a plain error (or a sound result) and
//! never panic or loop. The inputs here are truncated zips, missing parts, malformed XML, invalid
//! UTF-8, absurd nesting, entity bombs, and thousands of random mutations of good files.

mod common;

use common::*;
use prismqda_engine::RunKind;
use prismqda_import::{docx, odt, rtf, ParsedText};

/// How many runs of a kind there are, without cutting out the text of each (slow on big texts).
fn count_runs(p: &ParsedText, kind: RunKind) -> usize {
    p.runs.iter().filter(|r| r.kind == kind).count()
}

// ---------------------------------------------------------------------------------------------
// Not the right kind of file
// ---------------------------------------------------------------------------------------------

#[test]
fn bytes_that_are_not_a_package_are_refused_in_plain_words() {
    for bytes in [&b""[..], b"hello", b"PK\x03\x04", b"%PDF-1.7 not a word file at all", &[0u8; 64][..]] {
        for result in [docx::parse(bytes, &opts()), odt::parse(bytes, &opts())] {
            let message = format_error(result);
            assert!(message.contains("could not be opened") || message.contains("damaged"), "{message}");
        }
    }
}

#[test]
fn old_binary_office_files_and_password_protected_files_get_a_helpful_message() {
    let mut ole = vec![0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    ole.extend_from_slice(&[0; 200]);
    let message = format_error(docx::parse(&ole, &opts()));
    assert!(message.contains("older binary Office file or a password-protected one"), "{message}");
    let message = format_error(rtf::parse(&ole, &opts()));
    assert!(message.contains("older binary Word file"), "{message}");
}

#[test]
fn a_zip_that_is_some_other_kind_of_file_is_named_as_such() {
    let sheet = zip_bytes(&[("[Content_Types].xml", CONTENT_TYPES), ("xl/workbook.xml", "<workbook/>")]);
    let message = format_error(docx::parse(&sheet, &opts()));
    assert!(message.contains("not a Word document"), "{message}");
    let message = format_error(odt::parse(&sheet, &opts()));
    assert!(message.contains("not an OpenDocument text file"), "{message}");
}

#[test]
fn rtf_that_is_not_rtf_says_what_it_looks_like() {
    assert!(format_error(rtf::parse(b"just some text", &opts())).contains("does not start the way an RTF file does"));
    assert!(format_error(rtf::parse(&docx("<w:p/>"), &opts())).contains("DOCX"));
    assert!(format_error(rtf::parse(b"", &opts())).contains("not an RTF file"));
}

#[test]
fn a_password_protected_zip_entry_is_reported_as_such() {
    // Set the "encrypted" flag in the local and the central header of every part.
    fn encrypt(mut bytes: Vec<u8>) -> Vec<u8> {
        for (sig, flag_at) in [(b"PK\x03\x04", 6usize), (b"PK\x01\x02", 8)] {
            let mut from = 0;
            while let Some(rel) = bytes[from..].windows(4).position(|w| w == sig) {
                let at = from + rel;
                bytes[at + flag_at] |= 1;
                from = at + 4;
            }
        }
        bytes
    }
    let message = format_error(docx::parse(&encrypt(docx(&p("Secret"))), &opts()));
    assert!(message.contains("password"), "{message}");
    let message = format_error(odt::parse(&encrypt(odt("", "<text:p>Secret</text:p>")), &opts()));
    assert!(message.contains("password"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// Damaged zips
// ---------------------------------------------------------------------------------------------

#[test]
fn truncated_zips_are_refused_at_every_length() {
    let good_docx = docx(&format!("{}{}", p("First paragraph."), p("Second paragraph.")));
    let good_odt = odt("", "<text:p>First paragraph.</text:p><text:p>Second paragraph.</text:p>");
    for good in [&good_docx, &good_odt] {
        // Every prefix of the file, so the cut falls in a local header, in the data, and in the directory.
        for len in (0..good.len()).step_by(7) {
            let cut = &good[..len];
            let a = docx::parse(cut, &opts());
            let b = odt::parse(cut, &opts());
            assert!(a.is_err() && b.is_err(), "a zip cut to {len} of {} bytes must not import", good.len());
        }
    }
}

#[test]
fn flipped_bytes_in_a_package_never_panic() {
    let good = docx(&format!("{}{}", p("First paragraph."), p("Second paragraph.")));
    for i in (0..good.len()).step_by(3) {
        let mut bad = good.clone();
        bad[i] ^= 0xFF;
        if let Ok(parsed) = docx::parse(&bad, &opts()) {
            assert_sound(&parsed);
        }
    }
}

#[test]
fn a_damaged_compressed_stream_is_reported() {
    // Store the part uncompressed, then damage a byte of its text: the checksum no longer matches.
    use std::io::Write;
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    w.start_file("word/document.xml", options).unwrap();
    w.write_all(document_xml(&p("Checksum text")).as_bytes()).unwrap();
    let mut bytes = w.finish().unwrap().into_inner();
    let at = bytes.windows(8).position(|w| w == b"Checksum").unwrap();
    bytes[at] = b'X';
    let message = format_error(docx::parse(&bytes, &opts()));
    assert!(message.contains("damaged"), "{message}");
}

// Packages whose own headers disagree: the "zip cloaking" advisory against the zip crate (GHSA-m665-v7pm-7grj, fixed in 9.0.5). The advisory is
// about the streaming reader, which unpacks to disk by the names in each file's header while `ZipArchive` goes by the directory at the end of the
// package. PrismQDA uses `ZipArchive` only, reads parts into memory by name, and writes none of them to disk, so there is no file to overwrite.

#[test]
fn a_package_is_read_by_the_names_in_its_directory_as_the_zip_format_says() {
    // The name in a file's own header and the name in the directory at the end of the package disagree. The header comes first in the
    // package, so the first copy of the name is the one in the header. The directory is the authority (Word and every reader that follows
    // the format do the same), so the document is found under the directory's name; a change of that in a later zip version fails here.
    let good = docx(&p("Cloaked paragraph."));
    let name = b"word/document.xml";
    let at = good.windows(name.len()).position(|w| w == name).unwrap();
    let mut bad = good.clone();
    bad[at + name.len() - 1] = b'x'; // "word/document.xmx" in the header, "word/document.xml" in the directory
    let parsed = docx::parse(&bad, &opts()).expect("the directory names the document, so it is read");
    assert!(parsed.text.contains("Cloaked paragraph."), "{:?}", parsed.text);
}

#[test]
fn two_entries_with_one_name_are_refused_or_read_as_one_and_never_mixed() {
    let first = document_xml(&p("First copy."));
    let second = document_xml(&p("Second copy."));
    let mut bad = zip_bytes(&[("[Content_Types].xml", CONTENT_TYPES), ("_rels/.rels", ROOT_RELS), ("word/document.xml", &first), ("word/document.xmx", &second)]);
    // Rename the second entry to the first one's name, in its header and in the directory (the writer itself refuses a duplicate).
    let from = b"document.xmx";
    while let Some(at) = bad.windows(from.len()).position(|w| w == from) {
        bad[at..at + from.len()].copy_from_slice(b"document.xml");
    }
    if let Ok(parsed) = docx::parse(&bad, &opts()) {
        let (a, b) = (parsed.text.contains("First copy."), parsed.text.contains("Second copy."));
        assert!(a != b, "the two copies were mixed, or neither was read: {:?}", parsed.text);
    }
}

// ---------------------------------------------------------------------------------------------
// Missing and damaged parts
// ---------------------------------------------------------------------------------------------

#[test]
fn a_package_with_no_main_part_is_refused() {
    let no_doc = zip_bytes(&[("[Content_Types].xml", CONTENT_TYPES), ("_rels/.rels", ROOT_RELS)]);
    assert!(format_error(docx::parse(&no_doc, &opts())).contains("no main document part"));
    let no_content = zip_bytes(&[("mimetype", TEXT_MIMETYPE), ("meta.xml", "<x/>")]);
    assert!(format_error(odt::parse(&no_content, &opts())).contains("no content.xml"));
}

#[test]
fn malformed_xml_is_refused_in_every_way_it_can_be_malformed() {
    let cases: [(&str, String); 7] = [
        ("mismatched end tag", "<w:document xmlns:w=\"x\"><w:body><w:p></w:q></w:body></w:document>".to_string()),
        ("unclosed element", document_xml(&p("cut off")).replace("</w:document>", "")),
        ("cut in the middle of a tag", document_xml(&p("cut off"))[..120].to_string()),
        ("empty part", String::new()),
        ("text only", "this is not xml".to_string()),
        ("dangling ampersand", document_xml("<w:p><w:r><w:t>AT&T</w:t></w:r></w:p>")),
        ("stray end tag", "</w:p>".to_string()),
    ];
    for (what, xml) in cases {
        let bytes = zip_bytes(&[
            ("[Content_Types].xml", CONTENT_TYPES),
            ("_rels/.rels", ROOT_RELS),
            ("word/document.xml", &xml),
        ]);
        let message = format_error(docx::parse(&bytes, &opts()));
        assert!(!message.is_empty(), "{what}");
        let bytes = zip_bytes(&[("mimetype", TEXT_MIMETYPE), ("content.xml", &xml)]);
        let message = format_error(odt::parse(&bytes, &opts()));
        assert!(!message.is_empty(), "{what}");
    }
}

#[test]
fn a_damaged_styles_part_costs_headings_but_not_the_document() {
    let bytes = zip_bytes(&[
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", &document_xml(&p("Readable text"))),
        ("word/styles.xml", "<w:styles><w:style"),
    ]);
    let parsed = docx::parse(&bytes, &opts()).unwrap();
    assert_eq!(parsed.text, "Readable text");
    assert!(warning(&parsed, "unreadable_styles").is_some());
}

#[test]
fn invalid_utf8_in_a_part_is_refused() {
    let mut xml = document_xml(&p("good")).into_bytes();
    let at = xml.windows(4).position(|w| w == b"good").unwrap();
    xml[at] = 0xFF;
    xml[at + 1] = 0xC0;
    let bytes = zip_raw(&[("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", &xml)]);
    let message = format_error(docx::parse(&bytes, &opts()));
    assert!(message.contains("not valid text"), "{message}");

    let bytes = zip_raw(&[("mimetype", TEXT_MIMETYPE.as_bytes()), ("content.xml", &[0x3C, 0xFF, 0xFE, 0x80, 0x3E])]);
    assert!(format_error(odt::parse(&bytes, &opts())).len() > 10);
}

#[test]
fn utf16_parts_with_a_broken_pair_are_refused() {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend_from_slice(&[b'<', 0, 0x00, 0xD8, b'>', 0]); // a lone high surrogate
    let package = zip_raw(&[("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", &bytes)]);
    assert!(format_error(docx::parse(&package, &opts())).contains("not valid text"));
}

// ---------------------------------------------------------------------------------------------
// Absurd structure
// ---------------------------------------------------------------------------------------------

#[test]
fn xml_nested_a_thousand_levels_deep_is_refused() {
    let deep = format!("{}{}{}", "<w:sdt><w:sdtContent>".repeat(500), p("deep"), "</w:sdtContent></w:sdt>".repeat(500));
    let message = format_error(docx::parse(&docx(&deep), &opts()));
    assert!(message.contains("levels deep"), "{message}");

    let deep = format!("{}x{}", "<text:span>".repeat(1000), "</text:span>".repeat(1000));
    let message = format_error(odt::parse(&odt("", &format!("<text:p>{deep}</text:p>")), &opts()));
    assert!(message.contains("levels deep"), "{message}");
}

#[test]
fn tables_nested_too_deep_are_refused() {
    let open = "<w:tbl><w:tr><w:tc>";
    let close = "</w:tc></w:tr></w:tbl>";
    let ok = format!("{}{}{}", open.repeat(50), p("inside"), close.repeat(50));
    let parsed = docx::parse(&docx(&ok), &opts()).unwrap();
    assert_eq!(parsed.text, "inside");
    assert_eq!(count_runs(&parsed, prismqda_engine::RunKind::TableCell), 50);
    let too_deep = format!("{}{}{}", open.repeat(100), p("inside"), close.repeat(100));
    let message = format_error(docx::parse(&docx(&too_deep), &opts()));
    assert!(message.contains("nests tables too deeply"), "{message}");

    let open = "<table:table><table:table-row><table:table-cell>";
    let close = "</table:table-cell></table:table-row></table:table>";
    let too_deep = format!("{}<text:p>inside</text:p>{}", open.repeat(100), close.repeat(100));
    let message = format_error(odt::parse(&odt("", &too_deep), &opts()));
    assert!(message.contains("nests tables too deeply"), "{message}");
}

#[test]
fn rtf_groups_nested_too_deep_are_refused() {
    let hostile = format!("{{\\rtf1 {}text{}}}", "{".repeat(5000), "}".repeat(5000));
    assert!(format_error(rtf::parse(hostile.as_bytes(), &opts())).contains("levels deep"));
    let fine = format!("{{\\rtf1 {}text{}}}", "{".repeat(100), "}".repeat(100));
    assert_eq!(rtf::parse(fine.as_bytes(), &opts()).unwrap().text, "text");
}

#[test]
fn entity_bombs_are_never_expanded() {
    // "Billion laughs": each entity is ten of the one before. Expanding it would need gigabytes.
    let mut dtd = String::from("<!DOCTYPE w:document [<!ENTITY a0 \"lol\">");
    for i in 1..10 {
        let prev = format!("&a{};", i - 1);
        dtd.push_str(&format!("<!ENTITY a{i} \"{}\">", prev.repeat(10)));
    }
    dtd.push_str("]>");
    let xml = document_xml("<w:p><w:r><w:t>before &a9; after</w:t></w:r></w:p>")
        .replace("<w:document", &format!("{dtd}<w:document"));
    let bytes = zip_bytes(&[("_rels/.rels", ROOT_RELS), ("word/document.xml", &xml)]);
    match docx::parse(&bytes, &opts()) {
        Ok(parsed) => {
            assert!(parsed.text.len() < 100, "an entity must not be expanded");
            assert!(!parsed.text.contains("lol"));
        }
        Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_))),
    }
}

#[test]
fn impossible_characters_in_text_are_dropped() {
    let body = "<w:p><w:r><w:t>a&#0;b&#1;c&#x1F;d&#xD800;e&#x110000;f&#233;g</w:t></w:r></w:p>";
    if let Ok(parsed) = docx::parse(&docx(body), &opts()) {
        assert_sound(&parsed);
        assert!(parsed.text.contains('a') && parsed.text.contains('g'));
    }
    // A real NUL character cannot be written in XML, but a damaged file may hold one.
    let mut xml = document_xml(&p("nul")).into_bytes();
    let at = xml.windows(3).position(|w| w == b"nul").unwrap();
    xml[at + 1] = 0;
    let bytes = zip_raw(&[("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", &xml)]);
    if let Ok(parsed) = docx::parse(&bytes, &opts()) {
        assert_sound(&parsed);
    }
}

#[test]
fn duplicate_and_odd_attributes_do_not_break_reading() {
    let body = r#"<w:p><w:r><w:rPr><w:b w:val="1" w:val="0"/></w:rPr><w:t w:x="1" w:x="2">dup</w:t></w:r></w:p>"#;
    // Whether the parser accepts the duplicates or calls the file damaged, it must not panic.
    match docx::parse(&docx(body), &opts()) {
        Ok(parsed) => assert_eq!(parsed.text, "dup"),
        Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_)), "{e}"),
    }
}

#[test]
fn rtf_odd_tokens_do_not_break_reading() {
    let huge = "99999999999";
    let files: Vec<String> = vec![
        format!(r"{{\rtf1 \u{huge} x}}"),
        format!(r"{{\rtf1 \u-{huge} x}}"),
        format!(r"{{\rtf1 \uc{huge} \u233 abc}}"),
        format!(r"{{\rtf1 \uc{huge} \u{huge}\u{huge} abc}}"),
        r"{\rtf1 \ansicpg99999 \'e9\'e9\'e9}".to_string(),
        r"{\rtf1 \'}".to_string(),
        r"{\rtf1 \'e}".to_string(),
        r"{\rtf1 {\*\*\*\* x}}".to_string(),
        r"{\rtf1 {\fonttbl{\f1\fcharset999999 x;}} \f1 \'e9}".to_string(),
        r"{\rtf1 \cell\cell\row\row\nestcell\nestrow\cell}".to_string(),
        r"{\rtf1 \trowd \intbl\itap999999 a\cell \intbl\itap0 b\cell \row}".to_string(),
        r"{\rtf1 \par\par\par \sect\page}".to_string(),
        r"{\rtf1 {\pict {\pict {\pict }}}}".to_string(),
        r"{\rtf1 {\shp {\shp {\shptxt a}}}}".to_string(),
        r"{\rtf1 {\stylesheet {\s1 {\s2 {\s3 a;}}}} \s1 \s2 \s3 x}".to_string(),
        r"{\rtf1 \b\b0\b\b0 x \ab \ai \v \v0 y}".to_string(),
        r"{\rtf1 }}}}{{{ x}".to_string(),
        "{\\rtf1 \\bin 5 abc}".to_string(),
        "{\\rtf1 \\bin-5 abc}".to_string(),
        "{\\rtf1 \\bin999999999999999999999 abc}".to_string(),
    ];
    for file in files {
        match rtf::parse(file.as_bytes(), &opts()) {
            Ok(parsed) => assert_sound(&parsed),
            Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_)), "{file}: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Size: nothing should be slow in the size of the document
// ---------------------------------------------------------------------------------------------

#[test]
fn big_documents_are_read_in_one_pass() {
    // 30,000 paragraphs with mixed emphasis, then a table of 3,000 rows.
    let mut body = String::new();
    for i in 0..30_000 {
        body.push_str(&format!(
            "<w:p><w:r><w:t xml:space=\"preserve\">Paragraph {i} </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r></w:p>"
        ));
    }
    body.push_str("<w:tbl>");
    for i in 0..3_000 {
        body.push_str(&format!("<w:tr><w:tc><w:p><w:r><w:t>r{i}</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>c{i}</w:t></w:r></w:p></w:tc></w:tr>"));
    }
    body.push_str("</w:tbl>");
    let started = std::time::Instant::now();
    let parsed = docx::parse(&docx(&body), &opts()).unwrap();
    assert!(started.elapsed().as_secs() < 30, "reading took {:?}", started.elapsed());
    assert_sound(&parsed);
    assert_eq!(count_runs(&parsed, prismqda_engine::RunKind::Bold), 30_000);
    assert_eq!(count_runs(&parsed, prismqda_engine::RunKind::TableCell), 6_000);
    assert!(parsed.text.starts_with("Paragraph 0 bold\n\nParagraph 1 bold"));
    assert!(parsed.text.ends_with("r2999\tc2999"));
}

#[test]
fn one_paragraph_with_a_hundred_thousand_runs_is_read_in_one_pass() {
    let mut body = String::from("<w:p>");
    for i in 0..100_000 {
        let props = if i % 2 == 0 { "<w:rPr><w:b/></w:rPr>" } else { "" };
        body.push_str(&format!("<w:r>{props}<w:t xml:space=\"preserve\">w{i} </w:t></w:r>"));
    }
    body.push_str("</w:p>");
    let started = std::time::Instant::now();
    let parsed = docx::parse(&docx(&body), &opts()).unwrap();
    assert!(started.elapsed().as_secs() < 30, "reading took {:?}", started.elapsed());
    assert_sound(&parsed);
    assert_eq!(count_runs(&parsed, prismqda_engine::RunKind::Bold), 50_000);
}

#[test]
fn big_rtf_is_read_in_one_pass() {
    let mut body = String::new();
    for i in 0..30_000 {
        body.push_str(&format!("\\pard Paragraph {i} {{\\b bold}}\\par\n"));
    }
    let started = std::time::Instant::now();
    let parsed = rtf::parse(rtf_doc(&body).as_bytes(), &opts()).unwrap();
    assert!(started.elapsed().as_secs() < 30, "reading took {:?}", started.elapsed());
    assert_sound(&parsed);
    assert_eq!(count_runs(&parsed, prismqda_engine::RunKind::Bold), 30_000);
}

// ---------------------------------------------------------------------------------------------
// Mutations: thousands of damaged variants of good files, with a fixed seed so any failure repeats
// ---------------------------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn mutate(rng: &mut Rng, good: &[u8]) -> Vec<u8> {
    let mut bytes = good.to_vec();
    for _ in 0..1 + rng.below(4) {
        if bytes.is_empty() {
            break;
        }
        match rng.below(5) {
            0 => {
                let i = rng.below(bytes.len());
                bytes[i] = rng.next() as u8;
            }
            1 => {
                let i = rng.below(bytes.len());
                bytes.remove(i);
            }
            2 => {
                let i = rng.below(bytes.len());
                bytes.insert(i, b"{}\\<>&\"'/"[rng.below(9)]);
            }
            3 => {
                let at = rng.below(bytes.len());
                bytes.truncate(at);
            }
            _ => {
                let from = rng.below(bytes.len());
                let to = (from + rng.below(40)).min(bytes.len());
                let piece: Vec<u8> = bytes[from..to].to_vec();
                let at = rng.below(bytes.len());
                for (k, b) in piece.into_iter().enumerate() {
                    bytes.insert((at + k).min(bytes.len()), b);
                }
            }
        }
    }
    bytes
}

const SAMPLE_BODY: &str = concat!(
    r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>"#,
    r#"<w:p><w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>Bold italic</w:t></w:r><w:r><w:drawing><wp:inline><a:graphic><pic:pic/></a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
    r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>b</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
    r#"<w:p><w:ins w:id="1"><w:r><w:t>kept</w:t></w:r></w:ins><w:del w:id="2"><w:r><w:delText>gone</w:delText></w:r></w:del></w:p>"#,
);

#[test]
fn mutated_docx_xml_never_panics() {
    let xml = document_xml(SAMPLE_BODY).into_bytes();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut accepted = 0;
    for _ in 0..1500 {
        let bad = mutate(&mut rng, &xml);
        let package = zip_raw(&[("_rels/.rels", ROOT_RELS.as_bytes()), ("word/document.xml", &bad)]);
        match docx::parse(&package, &opts()) {
            Ok(parsed) => {
                accepted += 1;
                assert_sound(&parsed);
            }
            Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_)), "{e}"),
        }
    }
    assert!(accepted > 0, "some mutations leave a readable file");
}

#[test]
fn mutated_odt_xml_never_panics() {
    let body = concat!(
        r#"<text:h text:outline-level="1">Title</text:h><text:p>Some <text:span text:style-name="Bold">bold</text:span> text<text:s text:c="2"/>and more</text:p>"#,
        r#"<table:table><table:table-row><table:table-cell><text:p>a</text:p></table:table-cell><table:table-cell><text:p>b</text:p></table:table-cell></table:table-row></table:table>"#,
        r#"<text:p><draw:frame><draw:image xlink:href="x.png"/></draw:frame></text:p>"#,
    );
    let style = r#"<style:style style:name="Bold" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>"#;
    let xml = content_xml(style, body).into_bytes();
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    let mut accepted = 0;
    for _ in 0..1500 {
        let bad = mutate(&mut rng, &xml);
        let package = zip_raw(&[("mimetype", TEXT_MIMETYPE.as_bytes()), ("content.xml", &bad)]);
        match odt::parse(&package, &opts()) {
            Ok(parsed) => {
                accepted += 1;
                assert_sound(&parsed);
            }
            Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_)), "{e}"),
        }
    }
    assert!(accepted > 0);
}

#[test]
fn mutated_rtf_never_panics() {
    let sample = include_bytes!("fixtures/interview_excerpt.rtf");
    let mut rng = Rng(0x2545_F491_4F6C_DD1D);
    let mut accepted = 0;
    for _ in 0..4000 {
        let bad = mutate(&mut rng, sample);
        match rtf::parse(&bad, &opts()) {
            Ok(parsed) => {
                accepted += 1;
                assert_sound(&parsed);
            }
            Err(e) => assert!(matches!(e, prismqda_import::ImportError::Format(_)), "{e}"),
        }
    }
    assert!(accepted > 0);
}

#[test]
fn mutated_zip_bytes_never_panic() {
    let good = docx(SAMPLE_BODY);
    let mut rng = Rng(0xA076_1D64_78BD_642F);
    for _ in 0..1000 {
        let bad = mutate(&mut rng, &good);
        if let Ok(parsed) = docx::parse(&bad, &opts()) {
            assert_sound(&parsed);
        }
        if let Ok(parsed) = odt::parse(&bad, &opts()) {
            assert_sound(&parsed);
        }
    }
}
