//! A small Word (.docx) writer for reports: real heading styles, paragraphs with bold and italic runs, block
//! quotes, and tables with a marked header row, so the file reads well with a screen reader and in Word's
//! navigation pane. The document language and title are set.

use std::io::Write;

use crate::xml::escape;
use crate::{zip_package, ExportError};

/// A stretch of text with one look.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Run {
        Run { text: text.into(), ..Run::default() }
    }
    pub fn bold(text: impl Into<String>) -> Run {
        Run { text: text.into(), bold: true, italic: false }
    }
    pub fn italic(text: impl Into<String>) -> Run {
        Run { text: text.into(), bold: false, italic: true }
    }
}

/// A picture file kept once in the document; any number of blocks may show a part of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// `png`, `jpg`, `gif` or `bmp`.
    pub extension: &'static str,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

/// The part of a picture to show: its left, top, right and bottom edges as fractions of the picture (0 is the left or top edge, 1 the
/// right or bottom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crop {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Crop {
    /// The four edges cut away, in the hundred-thousandths of the picture that Office files use: left, top, right, bottom.
    pub(crate) fn cut(&self) -> (i64, i64, i64, i64) {
        let pct = |v: f64| (v.clamp(0.0, 1.0) * 100_000.0).round() as i64;
        (pct(self.x0), pct(self.y0), pct(1.0 - self.x1), pct(1.0 - self.y1))
    }

    /// True when the edges leave a part of the picture to show.
    pub(crate) fn is_valid(&self) -> bool {
        [self.x0, self.y0, self.x1, self.y1].iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)) && self.x1 > self.x0 && self.y1 > self.y0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// Level 1 to 3.
    Heading(u8, String),
    Paragraph(Vec<Run>),
    /// An indented quotation, with an optional attribution line beneath it.
    Quote { text: String, source: Option<String> },
    /// A table whose first row is a header. Cells hold plain text; a line break inside is kept.
    Table { header: Vec<String>, rows: Vec<Vec<String>> },
    /// Part of a picture kept in the document (`media` is its place in the list given to `write_docx_with_media`), shown `width` by
    /// `height` English metric units large (914,400 to the inch), with `alt` read out for a person who cannot see it.
    Picture { media: usize, crop: Crop, width: i64, height: i64, alt: String },
    /// A number line in bold and, beneath it, a title in italics, as APA 7 sets above a table or a figure.
    Caption { label: String, title: String },
    /// A table in APA 7 style: rules above and below the header and below the last row, no vertical lines, the first column left
    /// aligned and the others centered, and a note beneath (its first word, `note_label`, in italics). The caption above it is a
    /// `Block::Caption`.
    Apa { header: Vec<String>, rows: Vec<Vec<String>>, note_label: String, note: String },
}

impl Block {
    pub fn paragraph(text: impl Into<String>) -> Block {
        Block::Paragraph(vec![Run::plain(text)])
    }
}

fn runs_xml(runs: &[Run]) -> String {
    let mut xml = String::new();
    for run in runs {
        // A line break in the text becomes a Word line break, and a tab a Word tab.
        for (i, line) in run.text.split('\n').enumerate() {
            let mut props = String::new();
            if run.bold {
                props.push_str("<w:b/>");
            }
            if run.italic {
                props.push_str("<w:i/>");
            }
            let rpr = if props.is_empty() { String::new() } else { format!("<w:rPr>{props}</w:rPr>") };
            if i > 0 {
                xml.push_str(&format!("<w:r>{rpr}<w:br/></w:r>"));
            }
            for (j, piece) in line.trim_end_matches('\r').split('\t').enumerate() {
                if j > 0 {
                    xml.push_str(&format!("<w:r>{rpr}<w:tab/></w:r>"));
                }
                if !piece.is_empty() {
                    xml.push_str(&format!("<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>", escape(piece)));
                }
            }
        }
    }
    xml
}

fn scrub_blocks(blocks: &[Block]) -> Vec<Block> {
    let t = |s: &String| crate::scrub::text(s).into_owned();
    blocks
        .iter()
        .map(|b| match b {
            Block::Heading(level, s) => Block::Heading(*level, t(s)),
            Block::Paragraph(runs) => Block::Paragraph(runs.iter().map(|r| Run { text: t(&r.text), ..r.clone() }).collect()),
            Block::Quote { text, source } => Block::Quote { text: t(text), source: source.as_ref().map(t) },
            Block::Table { header, rows } => Block::Table { header: header.iter().map(t).collect(), rows: rows.iter().map(|r| r.iter().map(t).collect()).collect() },
            Block::Picture { media, crop, width, height, alt } => Block::Picture { media: *media, crop: *crop, width: *width, height: *height, alt: t(alt) },
            Block::Caption { label, title } => Block::Caption { label: t(label), title: t(title) },
            Block::Apa { header, rows, note_label, note } => {
                Block::Apa { header: header.iter().map(t).collect(), rows: rows.iter().map(|r| r.iter().map(t).collect()).collect(), note_label: t(note_label), note: t(note) }
            }
        })
        .collect()
}

fn paragraph_xml(style: Option<&str>, runs: &[Run]) -> String {
    let ppr = style.map(|s| format!("<w:pPr><w:pStyle w:val=\"{s}\"/></w:pPr>")).unwrap_or_default();
    format!("<w:p>{ppr}{}</w:p>", runs_xml(runs))
}

fn cell_xml(text: &str, width: u32, bold: bool) -> String {
    let run = Run { text: text.to_string(), bold, italic: false };
    format!("<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>", paragraph_xml(Some("TableText"), &[run]))
}

fn table_xml(header: &[String], rows: &[Vec<String>]) -> String {
    let columns = header.len().max(rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
    // The page is 9,360 twentieths of a point wide inside its margins.
    let width = 9360 / columns as u32;
    let mut xml = String::from("<w:tbl><w:tblPr><w:tblStyle w:val=\"TableGrid\"/><w:tblW w:w=\"9360\" w:type=\"dxa\"/><w:tblLook w:val=\"0420\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>");
    for _ in 0..columns {
        xml.push_str(&format!("<w:gridCol w:w=\"{width}\"/>"));
    }
    xml.push_str("</w:tblGrid>");
    // The header row repeats on each page and is marked as a header for assistive technology.
    xml.push_str("<w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>");
    for c in 0..columns {
        xml.push_str(&cell_xml(header.get(c).map(String::as_str).unwrap_or(""), width, true));
    }
    xml.push_str("</w:tr>");
    for row in rows {
        xml.push_str("<w:tr><w:trPr><w:cantSplit/></w:trPr>");
        for c in 0..columns {
            xml.push_str(&cell_xml(row.get(c).map(String::as_str).unwrap_or(""), width, false));
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

fn apa_cell_xml(text: &str, width: u32, first: bool, header: bool, last_row: bool) -> String {
    let mut borders = String::new();
    if header {
        borders.push_str("<w:top w:val=\"single\" w:sz=\"8\" w:space=\"0\" w:color=\"000000\"/>");
    }
    if header || last_row {
        let size = if last_row { 8 } else { 4 };
        borders.push_str(&format!("<w:bottom w:val=\"single\" w:sz=\"{size}\" w:space=\"0\" w:color=\"000000\"/>"));
    }
    let borders = if borders.is_empty() { String::new() } else { format!("<w:tcBorders>{borders}</w:tcBorders>") };
    let jc = if first { "" } else { "<w:jc w:val=\"center\"/>" };
    let run = Run { text: text.to_string(), bold: false, italic: false };
    format!("<w:tc><w:tcPr><w:tcW w:w=\"{width}\" w:type=\"dxa\"/>{borders}</w:tcPr><w:p><w:pPr><w:pStyle w:val=\"TableText\"/>{jc}</w:pPr>{}</w:p></w:tc>", runs_xml(&[run]))
}

/// An APA table: no table style (so no grid), the rules set on the cells, and the header row marked as a header.
fn apa_table_xml(header: &[String], rows: &[Vec<String>]) -> String {
    let columns = header.len().max(rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
    // The first column holds the names of the rows, and gets more room when there are others.
    let total = 9360u32;
    let first_width = if columns > 2 { total * 3 / 10 } else { total / columns as u32 };
    let other = if columns > 1 { (total - first_width) / (columns as u32 - 1) } else { total };
    let width = |c: usize| if c == 0 { first_width } else { other };
    let mut xml = String::from("<w:tbl><w:tblPr><w:tblW w:w=\"9360\" w:type=\"dxa\"/><w:tblLook w:val=\"0420\" w:firstRow=\"1\" w:lastRow=\"0\" w:firstColumn=\"0\" w:lastColumn=\"0\" w:noHBand=\"1\" w:noVBand=\"1\"/></w:tblPr><w:tblGrid>");
    for c in 0..columns {
        xml.push_str(&format!("<w:gridCol w:w=\"{}\"/>", width(c)));
    }
    xml.push_str("</w:tblGrid><w:tr><w:trPr><w:cantSplit/><w:tblHeader/></w:trPr>");
    for c in 0..columns {
        xml.push_str(&apa_cell_xml(header.get(c).map(String::as_str).unwrap_or(""), width(c), c == 0, true, rows.is_empty()));
    }
    xml.push_str("</w:tr>");
    for (r, row) in rows.iter().enumerate() {
        xml.push_str("<w:tr><w:trPr><w:cantSplit/></w:trPr>");
        for c in 0..columns {
            xml.push_str(&apa_cell_xml(row.get(c).map(String::as_str).unwrap_or(""), width(c), c == 0, false, r + 1 == rows.len()));
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
}

/// The XML of a part of a picture, set in a paragraph by itself. `id` numbers it among the drawings of the page; `rid` is the
/// relationship that names the picture's file.
fn picture_xml(id: usize, rid: &str, crop: &Crop, width: i64, height: i64, alt: &str) -> String {
    let (l, t, r, b) = crop.cut();
    let alt = escape(alt);
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"PictureArea\"/></w:pPr><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{width}\" cy=\"{height}\"/><wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{alt}\"/><wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture {id}\" descr=\"{alt}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed=\"{rid}\"/><a:srcRect l=\"{l}\" t=\"{t}\" r=\"{r}\" b=\"{b}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"
    )
}

fn body_xml(blocks: &[Block]) -> String {
    let mut xml = String::new();
    let mut drawings = 0;
    for block in blocks {
        match block {
            Block::Picture { media, crop, width, height, alt } => {
                drawings += 1;
                xml.push_str(&picture_xml(drawings, &format!("rIdM{}", media + 1), crop, *width, *height, alt));
            }
            Block::Heading(level, text) => xml.push_str(&paragraph_xml(Some(&format!("Heading{}", (*level).clamp(1, 3))), &[Run::plain(text.clone())])),
            Block::Paragraph(runs) => xml.push_str(&paragraph_xml(None, runs)),
            Block::Quote { text, source } => {
                xml.push_str(&paragraph_xml(Some("Quote"), &[Run::plain(text.clone())]));
                if let Some(s) = source {
                    xml.push_str(&paragraph_xml(Some("QuoteSource"), &[Run::plain(s.clone())]));
                }
            }
            Block::Table { header, rows } => {
                xml.push_str(&table_xml(header, rows));
                // Word needs a paragraph after a table, and the gap keeps the next block from touching it.
                xml.push_str("<w:p/>");
            }
            Block::Caption { label, title } => {
                xml.push_str(&paragraph_xml(Some("CaptionNumber"), &[Run::bold(label.clone())]));
                if !title.is_empty() {
                    xml.push_str(&paragraph_xml(Some("CaptionTitle"), &[Run::italic(title.clone())]));
                }
            }
            Block::Apa { header, rows, note_label, note } => {
                xml.push_str(&apa_table_xml(header, rows));
                // The note, or when there is none the paragraph Word needs after a table.
                if note.is_empty() {
                    xml.push_str("<w:p/>");
                } else {
                    xml.push_str(&paragraph_xml(Some("TableNote"), &[Run::italic(note_label.clone()), Run::plain(format!(" {note}"))]));
                }
            }
        }
    }
    xml
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:eastAsia="Calibri" w:cs="Calibri"/><w:sz w:val="22"/><w:szCs w:val="22"/><w:lang w:val="en-US" w:eastAsia="en-US" w:bidi="ar-SA"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:spacing w:after="120" w:line="276" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="36"/><w:szCs w:val="36"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="280" w:after="80"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="30"/><w:szCs w:val="30"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="200" w:after="60"/><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/><w:szCs w:val="26"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Quote"><w:name w:val="Quote"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="120" w:after="0"/><w:ind w:left="567" w:right="567"/></w:pPr><w:rPr><w:i/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="PictureArea"><w:name w:val="Picture Area"/><w:basedOn w:val="Normal"/><w:next w:val="Quote"/><w:pPr><w:keepNext/><w:spacing w:before="120" w:after="60"/><w:ind w:left="567"/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="QuoteSource"><w:name w:val="Quote Source"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:spacing w:after="160"/><w:ind w:left="567" w:right="567"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="TableText"><w:name w:val="Table Text"/><w:basedOn w:val="Normal"/><w:pPr><w:spacing w:before="40" w:after="40" w:line="240" w:lineRule="auto"/></w:pPr><w:rPr><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr></w:style>
<w:style w:type="table" w:default="1" w:styleId="TableNormal"><w:name w:val="Normal Table"/><w:uiPriority w:val="99"/><w:semiHidden/><w:tblPr><w:tblInd w:w="0" w:type="dxa"/><w:tblCellMar><w:top w:w="0" w:type="dxa"/><w:left w:w="108" w:type="dxa"/><w:bottom w:w="0" w:type="dxa"/><w:right w:w="108" w:type="dxa"/></w:tblCellMar></w:tblPr></w:style>
<w:style w:type="table" w:styleId="TableGrid"><w:name w:val="Table Grid"/><w:basedOn w:val="TableNormal"/><w:uiPriority w:val="39"/><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:left w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:right w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="808080"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="808080"/></w:tblBorders></w:tblPr></w:style>
<w:style w:type="paragraph" w:styleId="CaptionNumber"><w:name w:val="Caption Number"/><w:basedOn w:val="Normal"/><w:next w:val="CaptionTitle"/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="0"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="CaptionTitle"><w:name w:val="Caption Title"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:keepNext/><w:spacing w:before="0" w:after="80"/></w:pPr><w:rPr><w:i/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="TableNote"><w:name w:val="Table Note"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:pPr><w:spacing w:before="60" w:after="200"/></w:pPr><w:rPr><w:sz w:val="20"/></w:rPr></w:style>
</w:styles>"#;

/// The document as `.docx` bytes. `title` becomes the document's title property.
pub fn write_docx(title: &str, blocks: &[Block]) -> Result<Vec<u8>, ExportError> {
    write_docx_with_media(title, blocks, &[])
}

/// The document with pictures kept in it. Each file in `media` is stored once, however many blocks show a part of it.
pub fn write_docx_with_media(title: &str, blocks: &[Block], media: &[Media]) -> Result<Vec<u8>, ExportError> {
    // While a list of names is applied to what leaves the project, every text is passed through it here, where the file is made.
    let scrubbed;
    let (blocks, title): (&[Block], String) = if crate::scrub::active() {
        scrubbed = scrub_blocks(blocks);
        (&scrubbed, crate::scrub::text(title).into_owned())
    } else {
        (blocks, title.to_string())
    };
    let title = title.as_str();
    for block in blocks {
        if let Block::Picture { media: m, crop, width, height, .. } = block {
            if *m >= media.len() {
                return Err(ExportError::Invalid("a picture in the document names a file that was not given".to_string()));
            }
            if !crop.is_valid() || *width <= 0 || *height <= 0 {
                return Err(ExportError::Invalid("a picture in the document has no part to show or no size".to_string()));
            }
        }
    }
    let document = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"><w:body>{}<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>",
        body_xml(blocks)
    );
    // Each kind of picture file is named once, as the default for its ending.
    let mut kinds: Vec<(&str, &str)> = Vec::new();
    for m in media {
        if !kinds.iter().any(|(e, _)| *e == m.extension) {
            kinds.push((m.extension, m.content_type));
        }
    }
    let defaults: String = kinds.iter().map(|(e, c)| format!("<Default Extension=\"{e}\" ContentType=\"{c}\"/>")).collect();
    let content_types = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>{defaults}<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/><Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/><Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/></Types>");
    let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/><Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/></Relationships>";
    let mut document_rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>");
    for (i, m) in media.iter().enumerate() {
        document_rels.push_str(&format!("<Relationship Id=\"rIdM{0}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image{0}.{1}\"/>", i + 1, m.extension));
    }
    document_rels.push_str("</Relationships>");
    let core = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"><dc:title>{}</dc:title><dc:creator>PrismQDA</dc:creator><dc:language>en-US</dc:language></cp:coreProperties>",
        escape(title)
    );
    zip_package(|zip, options| {
        let mut put = |name: &str, content: &[u8]| -> Result<(), ExportError> {
            zip.start_file(name, options).map_err(|e| ExportError::Io(e.to_string()))?;
            zip.write_all(content).map_err(|e| ExportError::Io(e.to_string()))
        };
        put("[Content_Types].xml", content_types.as_bytes())?;
        put("_rels/.rels", root_rels.as_bytes())?;
        put("word/document.xml", document.as_bytes())?;
        put("word/_rels/document.xml.rels", document_rels.as_bytes())?;
        put("word/styles.xml", STYLES.as_bytes())?;
        for (i, m) in media.iter().enumerate() {
            put(&format!("word/media/image{}.{}", i + 1, m.extension), &m.bytes)?;
        }
        put("docProps/core.xml", core.as_bytes())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_breaks_and_tabs_become_word_breaks_and_tabs() {
        let xml = runs_xml(&[Run::bold("a\nb\tc")]);
        assert_eq!(xml, "<w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">a</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:br/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">b</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:tab/></w:r><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">c</w:t></w:r>");
    }

    #[test]
    fn a_table_marks_its_header_row_and_pads_short_rows() {
        let xml = table_xml(&["A".into(), "B".into()], &[vec!["1".into()]]);
        assert!(xml.contains("<w:tblHeader/>"));
        assert_eq!(xml.matches("<w:tc>").count(), 4, "two header cells and two cells in the padded row");
    }

    #[test]
    fn a_picture_is_kept_once_and_each_block_crops_its_own_part() {
        let crop_a = Crop { x0: 0.25, y0: 0.1, x1: 0.75, y1: 0.6 };
        let crop_b = Crop { x0: 0.0, y0: 0.0, x1: 1.0, y1: 0.5 };
        let media = [Media { extension: "png", content_type: "image/png", bytes: vec![1, 2, 3] }];
        let blocks = [
            Block::Picture { media: 0, crop: crop_a, width: 1_000_000, height: 800_000, alt: "A \"quoted\" <area>".into() },
            Block::Picture { media: 0, crop: crop_b, width: 2_000_000, height: 500_000, alt: "Another".into() },
        ];
        let xml = body_xml(&blocks);
        assert!(xml.contains("<a:srcRect l=\"25000\" t=\"10000\" r=\"25000\" b=\"40000\"/>"), "{xml}");
        assert!(xml.contains("<a:srcRect l=\"0\" t=\"0\" r=\"0\" b=\"50000\"/>"));
        assert_eq!(xml.matches("r:embed=\"rIdM1\"").count(), 2, "both blocks point at the one file");
        assert!(xml.contains("descr=\"A &quot;quoted&quot; &lt;area&gt;\""), "the description cannot break the markup");
        assert!(xml.contains("<wp:docPr id=\"1\"") && xml.contains("<wp:docPr id=\"2\""), "each drawing has its own number");
        let bytes = write_docx_with_media("T", &blocks, &media).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
        assert_eq!(names.iter().filter(|n| n.starts_with("word/media/")).count(), 1);
        let mut rels = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("word/_rels/document.xml.rels").unwrap(), &mut rels).unwrap();
        assert!(rels.contains("Id=\"rIdM1\"") && rels.contains("media/image1.png"));
        let mut types = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("[Content_Types].xml").unwrap(), &mut types).unwrap();
        assert!(types.contains("Extension=\"png\" ContentType=\"image/png\""));
    }

    #[test]
    fn a_picture_with_nothing_to_show_or_no_file_is_refused() {
        let media = [Media { extension: "png", content_type: "image/png", bytes: vec![0] }];
        let good = Crop { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 };
        let block = |media: usize, crop: Crop, width: i64| Block::Picture { media, crop, width, height: 10, alt: String::new() };
        assert!(write_docx_with_media("T", &[block(1, good, 10)], &media).is_err(), "no such file");
        assert!(write_docx_with_media("T", &[block(0, Crop { x0: 0.5, y0: 0.0, x1: 0.5, y1: 1.0 }, 10)], &media).is_err(), "no width to show");
        assert!(write_docx_with_media("T", &[block(0, Crop { x0: 0.0, y0: f64::NAN, x1: 1.0, y1: 1.0 }, 10)], &media).is_err());
        assert!(write_docx_with_media("T", &[block(0, good, 0)], &media).is_err(), "no size");
        assert!(write_docx_with_media("T", &[block(0, good, 10)], &media).is_ok());
    }

    #[test]
    fn an_apa_table_has_rules_and_no_grid_and_a_note() {
        let xml = body_xml(&[
            Block::Caption { label: "Table 1".into(), title: "Scores by <group>".into() },
            Block::Apa { header: vec!["Group".into(), "M".into(), "SD".into()], rows: vec![vec!["A".into(), "1.00".into(), "0.50".into()], vec!["B".into(), "2.00".into()]], note_label: "Note.".into(), note: "n = 8 & more".into() },
        ]);
        assert!(xml.contains("<w:pStyle w:val=\"CaptionNumber\"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Table 1</w:t>"), "{xml}");
        assert!(xml.contains("Scores by &lt;group&gt;"));
        assert!(!xml.contains("TableGrid") && !xml.contains("w:insideV") && !xml.contains("<w:left "), "no grid and no vertical lines");
        assert_eq!(xml.matches("<w:tblHeader/>").count(), 1);
        assert_eq!(xml.matches("<w:top ").count(), 3, "a rule above each cell of the header, and nowhere else");
        assert_eq!(xml.matches("w:sz=\"8\"").count(), 3 + 3, "the top rule and the closing rule, three cells wide");
        assert_eq!(xml.matches("w:sz=\"4\"").count(), 3, "a thin rule under the header");
        assert_eq!(xml.matches("<w:jc w:val=\"center\"/>").count(), 2 * 3, "all but the first column is centered");
        assert_eq!(xml.matches("<w:tc>").count(), 9, "a short row is padded");
        assert!(xml.contains("<w:t xml:space=\"preserve\">Note.</w:t>") && xml.contains("n = 8 &amp; more"));
    }

    #[test]
    fn an_apa_table_with_no_rows_closes_under_its_header() {
        let xml = body_xml(&[Block::Apa { header: vec!["A".into(), "B".into()], rows: vec![], note_label: "Note.".into(), note: String::new() }]);
        assert_eq!(xml.matches("w:sz=\"8\"").count(), 2 + 2);
        assert!(xml.ends_with("<w:p/>"), "Word needs a paragraph after a table");
    }

    #[test]
    fn markup_in_text_cannot_break_the_document() {
        let xml = body_xml(&[Block::paragraph("<w:p>&</w:p>")]);
        assert!(xml.contains("&lt;w:p&gt;&amp;&lt;/w:p&gt;"));
    }
}
