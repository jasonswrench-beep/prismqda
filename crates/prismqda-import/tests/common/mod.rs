//! Helpers shared by the office document tests. All text in these tests is invented.

#![allow(dead_code)]

use std::io::Write;

use prismqda_engine::{FormatRun, ImportWarning, RunKind};
use prismqda_import::{ImportError, ImportOptions, ParsedText};
use unicode_normalization::UnicodeNormalization;

pub fn opts() -> ImportOptions {
    ImportOptions::default()
}

/// A zip package holding the given parts, compressed the way real documents are.
pub fn zip_bytes(parts: &[(&str, &str)]) -> Vec<u8> {
    let owned: Vec<(&str, &[u8])> = parts.iter().map(|(n, c)| (*n, c.as_bytes())).collect();
    zip_raw(&owned)
}

/// Like [`zip_bytes`] but the parts are raw bytes, so a part can hold something that is not text.
pub fn zip_raw(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, content) in parts {
        w.start_file(*name, options).unwrap();
        w.write_all(content).unwrap();
    }
    w.finish().unwrap().into_inner()
}

/// The text a run covers, counted in Unicode scalar values.
pub fn covered(p: &ParsedText, run: &FormatRun) -> String {
    p.text.chars().skip(run.start as usize).take((run.end - run.start) as usize).collect()
}

pub fn runs_of(p: &ParsedText, kind: RunKind) -> Vec<String> {
    p.runs.iter().filter(|r| r.kind == kind).map(|r| covered(p, r)).collect()
}

pub fn warning<'a>(p: &'a ParsedText, kind: &str) -> Option<&'a ImportWarning> {
    p.warnings.iter().find(|w| w.kind == kind)
}

pub fn is_nfc(s: &str) -> bool {
    s.nfc().collect::<String>() == s
}

/// What every importer promises about its result, whatever the input was.
pub fn assert_sound(p: &ParsedText) {
    assert!(is_nfc(&p.text), "text must be NFC");
    assert!(!p.text.contains('\0'));
    let len = p.text.chars().count() as i64;
    for r in &p.runs {
        assert!(0 <= r.start && r.start < r.end && r.end <= len, "run {r:?} lies outside the text (length {len})");
    }
}

// --- DOCX packages -----------------------------------------------------------------------------

pub const WORD_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
pub const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#;
pub const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

pub fn document_xml(body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="{WORD_NS}" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body>{body}</w:body></w:document>"#
    )
}

pub fn docx_with_styles(body: &str, styles: &str) -> Vec<u8> {
    let styles_xml =
        format!(r#"<?xml version="1.0" encoding="UTF-8"?><w:styles xmlns:w="{WORD_NS}">{styles}</w:styles>"#);
    zip_bytes(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", &document_xml(body)),
        ("word/styles.xml", &styles_xml),
    ])
}

pub fn docx(body: &str) -> Vec<u8> {
    zip_bytes(&[
        ("[Content_Types].xml", CONTENT_TYPES),
        ("_rels/.rels", ROOT_RELS),
        ("word/document.xml", &document_xml(body)),
    ])
}

/// A plain paragraph.
pub fn p(text: &str) -> String {
    format!(r#"<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#)
}

// --- ODT packages ------------------------------------------------------------------------------

pub const ODF_NS: &str = r#"xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0" xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0" xmlns:svg="urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:math="http://www.w3.org/1998/Math/MathML""#;

pub fn content_xml(automatic_styles: &str, body: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-content {ODF_NS} office:version="1.2"><office:font-face-decls><style:font-face style:name="Liberation Serif"/></office:font-face-decls><office:automatic-styles>{automatic_styles}</office:automatic-styles><office:body><office:text><text:sequence-decls><text:sequence-decl text:display-outline-level="0" text:name="Illustration"/></text:sequence-decls>{body}</office:text></office:body></office:document-content>"#
    )
}

pub fn styles_xml(named_styles: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><office:document-styles {ODF_NS} office:version="1.2"><office:styles>{named_styles}</office:styles></office:document-styles>"#
    )
}

pub const TEXT_MIMETYPE: &str = "application/vnd.oasis.opendocument.text";

pub fn odt(automatic_styles: &str, body: &str) -> Vec<u8> {
    zip_bytes(&[("mimetype", TEXT_MIMETYPE), ("content.xml", &content_xml(automatic_styles, body))])
}

pub fn odt_with_styles(automatic_styles: &str, named_styles: &str, body: &str) -> Vec<u8> {
    zip_bytes(&[
        ("mimetype", TEXT_MIMETYPE),
        ("content.xml", &content_xml(automatic_styles, body)),
        ("styles.xml", &styles_xml(named_styles)),
    ])
}

// --- RTF ---------------------------------------------------------------------------------------

/// A Word-style RTF file around a body: header, font table, and color table come first.
pub fn rtf_doc(body: &str) -> String {
    let mut s = String::from(
        r"{\rtf1\ansi\ansicpg1252\deff0\nouicompat\deflang1033{\fonttbl{\f0\fnil\fcharset0 Arial;}}{\colortbl ;\red0\green0\blue0;}",
    );
    s.push_str(body);
    s.push('}');
    s
}

/// The message of a failed import, which must be a plain "damaged or wrong file" error.
pub fn format_error<T: std::fmt::Debug>(result: Result<T, ImportError>) -> String {
    match result {
        Err(ImportError::Format(message)) => {
            assert!(!message.is_empty());
            message
        }
        other => panic!("expected a format error, got {other:?}"),
    }
}
