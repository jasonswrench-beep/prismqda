//! The output notebook of the Statistics page as a file: a Word document with APA 7 tables, an Excel workbook, or a web page.
//!
//! The interface words every sentence and label (in the language of the window) and draws the charts, and sends both here; this turns
//! what it sent into a file, so the file says what the screen said and no number or word is added on the way. A chart arrives as a
//! picture (PNG, for Word and Excel) and as drawing instructions (SVG, for the web page), each with the text alternative that
//! someone who cannot see it needs.

use serde::Deserialize;

use crate::docx::{write_docx_with_media, Block, Crop, Media, Run};
use crate::xlsx::{write_xlsx_with_media, Cell, Sheet, SheetPicture};

/// Escapes a text for the page, after the list of names has been applied to it while one is in force.
fn escape(text: &str) -> String {
    crate::xml::escape(&crate::scrub::text(text))
}
use crate::{base64, ExportError};

const MAX_PAGES: usize = 2_000;
/// The largest a picture may be, in pixels on a side.
const MAX_PIXELS: u32 = 8_000;
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
/// The widest a picture is set in a document, in English metric units (6 inches).
const MAX_PICTURE_EMU: i64 = 5_486_400;
const EMU_PER_PIXEL: i64 = 9_525;

/// The words the file adds round what the interface sent, in the language of the window.
#[derive(Debug, Clone, Deserialize)]
pub struct Labels {
    /// "Table", numbered in the order the tables come.
    pub table: String,
    /// "Figure".
    pub figure: String,
    /// "Note.", the first word of the note under a table.
    pub note: String,
    /// "Contents".
    pub contents: String,
    /// "What the test depends on".
    pub checks: String,
    /// "Text alternative", the label of the words that stand for a chart in a workbook.
    pub alternative: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotebookTable {
    pub title: String,
    pub head: Vec<String>,
    pub rows: Vec<Vec<String>>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotebookChart {
    pub title: String,
    /// What a person who cannot see the chart is told.
    pub alt: String,
    /// The chart as a PNG, base 64. Word and Excel show it; left out, they show the text alternative alone.
    #[serde(default)]
    pub png: String,
    /// The chart as an SVG, text. The web page shows it; left out, the page shows the PNG, or the text alternative alone.
    #[serde(default)]
    pub svg: String,
    /// How large it is shown, in pixels.
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotebookPage {
    pub title: String,
    #[serde(default)]
    pub sentence: String,
    #[serde(default)]
    pub reading: String,
    /// Lines of fact under the sentence: the cases used, and the rule that picked them.
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub tables: Vec<NotebookTable>,
    /// The conditions the test depends on, one sentence each.
    #[serde(default)]
    pub checks: Vec<String>,
    #[serde(default)]
    pub charts: Vec<NotebookChart>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Notebook {
    pub title: String,
    /// The language of the words, as a code such as `en`.
    #[serde(default = "english")]
    pub language: String,
    pub labels: Labels,
    /// A line at the end ("Made with PrismQDA on ..."), or nothing.
    #[serde(default)]
    pub footer: String,
    pub pages: Vec<NotebookPage>,
}

fn english() -> String {
    "en".to_string()
}

/// The kinds of file a notebook can be made into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotebookFormat {
    Docx,
    Xlsx,
    Html,
}

impl NotebookFormat {
    pub fn from_name(name: &str) -> Option<NotebookFormat> {
        match name {
            "docx" => Some(NotebookFormat::Docx),
            "xlsx" => Some(NotebookFormat::Xlsx),
            "html" => Some(NotebookFormat::Html),
            _ => None,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            NotebookFormat::Docx => "docx",
            NotebookFormat::Xlsx => "xlsx",
            NotebookFormat::Html => "html",
        }
    }
}

/// The notebook as the bytes of a file of the format asked for.
pub fn export_notebook(format: NotebookFormat, notebook: &Notebook) -> Result<Vec<u8>, ExportError> {
    if notebook.pages.is_empty() {
        return Err(ExportError::Invalid("there is nothing in the notebook to export".to_string()));
    }
    if notebook.pages.len() > MAX_PAGES {
        return Err(ExportError::TooLarge(format!("the notebook has {} pages, and at most {MAX_PAGES} can be exported at once", notebook.pages.len())));
    }
    let pictures = pictures(notebook)?;
    match format {
        NotebookFormat::Docx => word(notebook, &pictures),
        NotebookFormat::Xlsx => excel(notebook, &pictures),
        NotebookFormat::Html => Ok(html(notebook).into_bytes()),
    }
}

/// The PNG of every chart in the order the charts come, checked: `None` for a chart sent without one.
fn pictures(notebook: &Notebook) -> Result<Vec<Option<Vec<u8>>>, ExportError> {
    let mut out = Vec::new();
    for page in &notebook.pages {
        for chart in &page.charts {
            if chart.width == 0 || chart.height == 0 || chart.width > MAX_PIXELS || chart.height > MAX_PIXELS {
                return Err(ExportError::Invalid(format!("the chart '{}' has no usable size", chart.title)));
            }
            if !chart.svg.trim().is_empty() && !chart.svg.trim_start().starts_with("<svg") {
                return Err(ExportError::Invalid(format!("the drawing of the chart '{}' is not an SVG", chart.title)));
            }
            if chart.png.trim().is_empty() {
                out.push(None);
                continue;
            }
            let bytes = base64::decode(&chart.png).ok_or_else(|| ExportError::Invalid(format!("the picture of the chart '{}' is not valid base 64", chart.title)))?;
            if !bytes.starts_with(&PNG_SIGNATURE) {
                return Err(ExportError::Invalid(format!("the picture of the chart '{}' is not a PNG", chart.title)));
            }
            out.push(Some(bytes));
        }
    }
    Ok(out)
}

/// The size a chart is set at in a document, in English metric units, no wider than the page.
fn emu_size(chart: &NotebookChart) -> (i64, i64) {
    let (w, h) = (i64::from(chart.width) * EMU_PER_PIXEL, i64::from(chart.height) * EMU_PER_PIXEL);
    if w <= MAX_PICTURE_EMU {
        (w, h)
    } else {
        (MAX_PICTURE_EMU, (h * MAX_PICTURE_EMU / w).max(1))
    }
}

fn full() -> Crop {
    Crop { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 }
}

// ---------------------------------------------------------------- Word

fn word(nb: &Notebook, pictures: &[Option<Vec<u8>>]) -> Result<Vec<u8>, ExportError> {
    let mut blocks = vec![Block::Heading(1, nb.title.clone())];
    let mut media: Vec<Media> = Vec::new();
    let (mut tables, mut figures, mut picture) = (0, 0, 0);
    for page in &nb.pages {
        blocks.push(Block::Heading(2, page.title.clone()));
        if !page.sentence.is_empty() {
            blocks.push(Block::paragraph(page.sentence.clone()));
        }
        if !page.reading.is_empty() {
            blocks.push(Block::Paragraph(vec![Run::italic(page.reading.clone())]));
        }
        for note in &page.notes {
            blocks.push(Block::paragraph(note.clone()));
        }
        for table in &page.tables {
            tables += 1;
            blocks.push(Block::Caption { label: format!("{} {tables}", nb.labels.table), title: table.title.clone() });
            blocks.push(Block::Apa { header: table.head.clone(), rows: table.rows.clone(), note_label: nb.labels.note.clone(), note: table.note.clone() });
        }
        for chart in &page.charts {
            figures += 1;
            blocks.push(Block::Caption { label: format!("{} {figures}", nb.labels.figure), title: chart.title.clone() });
            match &pictures[picture] {
                Some(bytes) => {
                    media.push(Media { extension: "png", content_type: "image/png", bytes: bytes.clone() });
                    let (width, height) = emu_size(chart);
                    blocks.push(Block::Picture { media: media.len() - 1, crop: full(), width, height, alt: chart.alt.clone() });
                }
                // No picture was sent, so the words that stand for it are shown instead.
                None => blocks.push(Block::paragraph(chart.alt.clone())),
            }
            picture += 1;
        }
        if !page.checks.is_empty() {
            blocks.push(Block::Heading(3, nb.labels.checks.clone()));
            for check in &page.checks {
                blocks.push(Block::paragraph(format!("\u{2022} {check}")));
            }
        }
    }
    if !nb.footer.is_empty() {
        blocks.push(Block::Paragraph(vec![Run::italic(nb.footer.clone())]));
    }
    write_docx_with_media(&nb.title, &blocks, &media)
}

// ---------------------------------------------------------------- Excel

/// A table's text as a number when it is one written plainly (an optional minus sign, digits, and a decimal point), and as text
/// otherwise: a p-value of "< .001", a count with a thousands comma, and an interval stay as they were written.
pub(crate) fn plain_number(text: &str) -> Option<f64> {
    let t = text.trim().replace('\u{2212}', "-");
    let digits = t.strip_prefix('-').unwrap_or(&t);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit() || c == '.') || digits.matches('.').count() > 1 || digits == "." {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn cell(text: &str) -> Cell {
    match plain_number(text) {
        Some(n) => Cell::Number(n),
        None => Cell::Text(text.to_string()),
    }
}

fn excel(nb: &Notebook, pictures: &[Option<Vec<u8>>]) -> Result<Vec<u8>, ExportError> {
    let mut sheets = vec![Sheet {
        name: nb.labels.contents.clone(),
        header: vec!["#".to_string(), nb.title.clone()],
        rows: nb.pages.iter().enumerate().map(|(i, p)| vec![Cell::Number((i + 1) as f64), Cell::Text(p.title.clone())]).collect(),
        widths: vec![6.0, 80.0],
        pictures: Vec::new(),
    }];
    let mut media: Vec<Media> = Vec::new();
    let mut picture = 0;
    for (n, page) in nb.pages.iter().enumerate() {
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut shown: Vec<SheetPicture> = Vec::new();
        let mut widest = 1;
        let text = |rows: &mut Vec<Vec<Cell>>, s: &str| {
            if !s.is_empty() {
                rows.push(vec![Cell::Text(s.to_string())]);
            }
        };
        text(&mut rows, &page.sentence);
        text(&mut rows, &page.reading);
        for note in &page.notes {
            text(&mut rows, note);
        }
        for table in &page.tables {
            rows.push(vec![Cell::Empty]);
            rows.push(vec![Cell::Head(table.title.clone())]);
            rows.push(table.head.iter().cloned().map(Cell::Head).collect());
            for row in &table.rows {
                rows.push(row.iter().map(|c| cell(c)).collect());
            }
            if !table.note.is_empty() {
                rows.push(vec![Cell::Text(format!("{} {}", nb.labels.note, table.note))]);
            }
            widest = widest.max(table.head.len());
        }
        for chart in &page.charts {
            rows.push(vec![Cell::Empty]);
            rows.push(vec![Cell::Head(chart.title.clone())]);
            if let Some(bytes) = &pictures[picture] {
                media.push(Media { extension: "png", content_type: "image/png", bytes: bytes.clone() });
                let (width, height) = emu_size(chart);
                rows.push(vec![Cell::Empty]);
                shown.push(SheetPicture { media: media.len() - 1, crop: full(), row: rows.len() - 1, col: 0, width, height, alt: chart.alt.clone() });
            }
            rows.push(vec![Cell::Text(format!("{}: {}", nb.labels.alternative, chart.alt))]);
            picture += 1;
        }
        if !page.checks.is_empty() {
            rows.push(vec![Cell::Empty]);
            rows.push(vec![Cell::Head(nb.labels.checks.clone())]);
            for check in &page.checks {
                rows.push(vec![Cell::Text(check.clone())]);
            }
        }
        let mut widths = vec![44.0];
        widths.resize(widest, 14.0);
        sheets.push(Sheet { name: format!("{} {}", n + 1, page.title), header: vec![page.title.clone()], rows, widths, pictures: shown });
    }
    write_xlsx_with_media(&sheets, &media)
}

// ---------------------------------------------------------------- web page

const STYLE: &str = "body{font:16px/1.5 system-ui,-apple-system,'Segoe UI',Roboto,sans-serif;margin:0;background:#fff;color:#1a1a1a}\
main{max-width:60rem;margin:0 auto;padding:1.5rem 1rem 4rem}\
h1{font-size:1.7rem;margin:0 0 1rem}h2{font-size:1.3rem;margin:2.5rem 0 .5rem;padding-top:1rem;border-top:1px solid #c8c8c8}h3{font-size:1.05rem;margin:1.5rem 0 .25rem}\
nav ol{padding-left:1.4rem}a{color:#0b4fa8}\
.sentence{font-size:1.05rem}.reading,.muted{color:#4a4a4a}.note{font-size:.9rem}\
table{border-collapse:collapse;margin:.25rem 0;min-width:50%}caption{text-align:left;margin-top:1.25rem;padding-bottom:.25rem}\
thead th{border-top:2px solid #1a1a1a;border-bottom:1px solid #1a1a1a;font-weight:600}\
tbody tr:last-child>*{border-bottom:2px solid #1a1a1a}th,td{padding:.25rem .75rem;text-align:center}th[scope=row],th:first-child,td:first-child{text-align:left}th[scope=row]{font-weight:400}\
figure{margin:1.25rem 0}figcaption{margin-bottom:.25rem}figure img{max-width:100%;height:auto;background:#fff}\
footer{margin-top:3rem;color:#4a4a4a;font-size:.9rem}\
@media (prefers-color-scheme:dark){body{background:#161616;color:#eaeaea}a{color:#8ab8ff}h2{border-color:#444}.reading,.muted,footer{color:#b8b8b8}thead th{border-color:#eaeaea}tbody tr:last-child>*{border-color:#eaeaea}}\
@media print{body{background:#fff;color:#000}h2{break-after:avoid}table,figure{break-inside:avoid}a{color:#000}}";

fn html(nb: &Notebook) -> String {
    let lang = if nb.language.trim().is_empty() { "en" } else { nb.language.trim() };
    let mut out = format!(
        "<!doctype html>\n<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<title>{}</title>\n<style>{STYLE}</style>\n</head>\n<body>\n<main>\n<h1>{}</h1>\n",
        escape(lang),
        escape(&nb.title),
        escape(&nb.title)
    );
    out.push_str(&format!("<nav aria-label=\"{}\">\n<h2 style=\"border:0;margin:0;padding:0\">{}</h2>\n<ol>\n", escape(&nb.labels.contents), escape(&nb.labels.contents)));
    for (i, page) in nb.pages.iter().enumerate() {
        out.push_str(&format!("<li><a href=\"#page-{0}\">{1}</a></li>\n", i + 1, escape(&page.title)));
    }
    out.push_str("</ol>\n</nav>\n");
    let (mut tables, mut figures) = (0, 0);
    for (i, page) in nb.pages.iter().enumerate() {
        let n = i + 1;
        out.push_str(&format!("<section id=\"page-{n}\" aria-labelledby=\"page-{n}-title\">\n<h2 id=\"page-{n}-title\">{}</h2>\n", escape(&page.title)));
        if !page.sentence.is_empty() {
            out.push_str(&format!("<p class=\"sentence\">{}</p>\n", escape(&page.sentence)));
        }
        if !page.reading.is_empty() {
            out.push_str(&format!("<p class=\"reading\">{}</p>\n", escape(&page.reading)));
        }
        for note in &page.notes {
            out.push_str(&format!("<p class=\"muted\">{}</p>\n", escape(note)));
        }
        for table in &page.tables {
            tables += 1;
            out.push_str(&format!("<table>\n<caption><strong>{} {tables}</strong><br><em>{}</em></caption>\n<thead><tr>", escape(&nb.labels.table), escape(&table.title)));
            let columns = table.head.len().max(table.rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
            for c in 0..columns {
                out.push_str(&format!("<th scope=\"col\">{}</th>", escape(table.head.get(c).map(String::as_str).unwrap_or(""))));
            }
            out.push_str("</tr></thead>\n<tbody>\n");
            for row in &table.rows {
                out.push_str("<tr>");
                for c in 0..columns {
                    let text = escape(row.get(c).map(String::as_str).unwrap_or(""));
                    if c == 0 {
                        out.push_str(&format!("<th scope=\"row\">{text}</th>"));
                    } else {
                        out.push_str(&format!("<td>{text}</td>"));
                    }
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</tbody>\n</table>\n");
            if !table.note.is_empty() {
                out.push_str(&format!("<p class=\"note\"><em>{}</em> {}</p>\n", escape(&nb.labels.note), escape(&table.note)));
            }
        }
        for chart in &page.charts {
            figures += 1;
            out.push_str(&format!("<figure>\n<figcaption><strong>{} {figures}</strong><br><em>{}</em></figcaption>\n", escape(&nb.labels.figure), escape(&chart.title)));
            let source = if !chart.svg.trim().is_empty() {
                Some(format!("data:image/svg+xml;base64,{}", base64::encode(chart.svg.as_bytes())))
            } else if !chart.png.trim().is_empty() {
                Some(format!("data:image/png;base64,{}", chart.png.split_whitespace().collect::<String>()))
            } else {
                None
            };
            match source {
                Some(src) => out.push_str(&format!("<img src=\"{src}\" width=\"{}\" height=\"{}\" alt=\"{}\">\n", chart.width, chart.height, escape(&chart.alt))),
                None => out.push_str(&format!("<p>{}</p>\n", escape(&chart.alt))),
            }
            out.push_str("</figure>\n");
        }
        if !page.checks.is_empty() {
            out.push_str(&format!("<h3>{}</h3>\n<ul>\n", escape(&nb.labels.checks)));
            for check in &page.checks {
                out.push_str(&format!("<li>{}</li>\n", escape(check)));
            }
            out.push_str("</ul>\n");
        }
        out.push_str("</section>\n");
    }
    if !nb.footer.is_empty() {
        out.push_str(&format!("<footer>{}</footer>\n", escape(&nb.footer)));
    }
    out.push_str("</main>\n</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn png() -> String {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        base64::encode(&bytes)
    }

    fn sample() -> Notebook {
        Notebook {
            title: "Notebook <1> & more".into(),
            language: "en".into(),
            labels: Labels { table: "Table".into(), figure: "Figure".into(), note: "Note.".into(), contents: "Contents".into(), checks: "What the test depends on".into(), alternative: "Text alternative".into() },
            footer: "Made with PrismQDA".into(),
            pages: vec![
                NotebookPage {
                    title: "Page 1: t test of Score".into(),
                    sentence: "Score was higher in B, p < .001.".into(),
                    reading: "A large effect.".into(),
                    notes: vec!["Cases used: 12 of 12.".into()],
                    tables: vec![
                        NotebookTable { title: "Groups".into(), head: vec!["Group".into(), "n".into(), "M".into(), "p".into()], rows: vec![vec!["A".into(), "4".into(), "11.50".into(), "< .001".into()], vec!["B".into(), "4".into(), "\u{2212}0.25".into(), "1,200".into()]], note: "M is the mean.".into() },
                        NotebookTable { title: "Second".into(), head: vec!["x".into()], rows: vec![vec!["1".into()]], note: String::new() },
                    ],
                    checks: vec!["Fine: 12 cases.".into()],
                    charts: vec![NotebookChart { title: "Box plot of Score".into(), alt: "Two boxes, \"A\" lower than B.".into(), png: png(), svg: "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\"><rect width=\"10\" height=\"10\" fill=\"#123456\"/></svg>".into(), width: 640, height: 360 }],
                },
                NotebookPage { title: "Page 2: Frequencies".into(), sentence: "N was 6.".into(), reading: String::new(), notes: vec![], tables: vec![], checks: vec![], charts: vec![NotebookChart { title: "Bars".into(), alt: "Six bars.".into(), png: String::new(), svg: String::new(), width: 400, height: 200 }] },
            ],
        }
    }

    fn read(bytes: Vec<u8>, name: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut s = String::new();
        zip.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    fn names(bytes: Vec<u8>) -> Vec<String> {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect()
    }

    #[test]
    fn plain_numbers_become_numbers_and_the_rest_stays_text() {
        assert_eq!(plain_number("11.50"), Some(11.5));
        assert_eq!(plain_number(".62"), Some(0.62));
        assert_eq!(plain_number("\u{2212}0.25"), Some(-0.25));
        assert_eq!(plain_number("-3"), Some(-3.0));
        for text in ["< .001", "1,200", "[0.1, 0.2]", "1e5", "inf", "NaN", "", ".", "-", "1.2.3", "12 ", "+4", "50%"] {
            let trimmed_ok = text == "12 ";
            assert_eq!(plain_number(text).is_some(), trimmed_ok, "{text:?}");
        }
    }

    #[test]
    fn the_word_file_has_numbered_apa_tables_figures_and_the_checks() {
        let bytes = export_notebook(NotebookFormat::Docx, &sample()).unwrap();
        assert!(names(bytes.clone()).iter().any(|n| n == "word/media/image1.png"));
        assert_eq!(names(bytes.clone()).iter().filter(|n| n.starts_with("word/media/")).count(), 1, "the chart sent without a picture has none");
        let doc = read(bytes, "word/document.xml");
        assert!(doc.contains("Notebook &lt;1&gt; &amp; more"));
        assert!(doc.contains("Score was higher in B, p &lt; .001."));
        assert!(doc.contains(">Table 1<") && doc.contains(">Table 2<") && doc.contains(">Figure 1<") && doc.contains(">Figure 2<"), "numbered in the order they come, through the pages");
        assert!(doc.contains("descr=\"Two boxes, &quot;A&quot; lower than B.\""), "the picture carries its text alternative");
        assert!(doc.contains("Six bars."), "a chart with no picture shows its words");
        assert!(doc.contains("\u{2022} Fine: 12 cases.") && doc.contains("What the test depends on"));
        assert!(doc.contains(">Note.<") && doc.contains("M is the mean."));
        assert!(doc.contains("Made with PrismQDA"));
        assert!(doc.contains("<w:pStyle w:val=\"Heading2\"/>"), "each page is a heading, so Word's navigation pane lists them");
    }

    #[test]
    fn the_workbook_has_a_contents_sheet_a_sheet_for_each_page_numbers_as_numbers_and_the_picture() {
        let bytes = export_notebook(NotebookFormat::Xlsx, &sample()).unwrap();
        let all = names(bytes.clone());
        assert!(all.iter().any(|n| n == "xl/worksheets/sheet3.xml") && !all.iter().any(|n| n == "xl/worksheets/sheet4.xml"), "contents and two pages");
        assert!(all.iter().any(|n| n == "xl/media/image1.png") || all.iter().any(|n| n.starts_with("xl/media/")), "{all:?}");
        let workbook = read(bytes.clone(), "xl/workbook.xml");
        assert!(workbook.contains("name=\"Contents\"") && workbook.contains("name=\"1 Page 1- t test of Score\""), "{workbook}");
        let sheet = read(bytes.clone(), "xl/worksheets/sheet2.xml");
        assert!(sheet.contains("<v>11.5</v>") && sheet.contains("<v>-0.25</v>"), "plain numbers are numbers: {sheet}");
        assert!(sheet.contains("&lt; .001") && sheet.contains("1,200"), "a p-value and a count with a comma stay as written");
        assert!(sheet.contains("Text alternative: Two boxes"), "the words that stand for the chart are in the sheet");
        assert!(sheet.contains("M is the mean."));
        assert!(sheet.contains("<drawing r:id=\"rId1\"/>"));
        let second = read(bytes, "xl/worksheets/sheet3.xml");
        assert!(second.contains("Text alternative: Six bars.") && !second.contains("<drawing"));
    }

    #[test]
    fn the_web_page_is_a_document_with_a_language_headings_captions_and_described_images() {
        let page = String::from_utf8(export_notebook(NotebookFormat::Html, &sample()).unwrap()).unwrap();
        assert!(page.starts_with("<!doctype html>") && page.contains("<html lang=\"en\">") && page.contains("<title>Notebook &lt;1&gt; &amp; more</title>"));
        assert!(page.contains("<nav aria-label=\"Contents\">") && page.contains("<a href=\"#page-2\">Page 2: Frequencies</a>"));
        assert!(page.contains("<section id=\"page-1\" aria-labelledby=\"page-1-title\">") && page.contains("<h2 id=\"page-1-title\">Page 1: t test of Score</h2>"));
        assert!(page.contains("<caption><strong>Table 1</strong><br><em>Groups</em></caption>"));
        assert!(page.contains("<th scope=\"col\">Group</th>") && page.contains("<th scope=\"row\">A</th>") && page.contains("<td>&lt; .001</td>"));
        assert!(page.contains("<p class=\"note\"><em>Note.</em> M is the mean.</p>"));
        assert!(page.contains("<img src=\"data:image/svg+xml;base64,") && page.contains("alt=\"Two boxes, &quot;A&quot; lower than B.\""), "the drawing is shown, with its words");
        assert!(page.contains("<p>Six bars.</p>"), "a chart sent with no picture or drawing shows its words");
        assert!(!page.contains("<script"), "nothing in the page runs");
        let src = page.split("data:image/svg+xml;base64,").nth(1).unwrap().split('"').next().unwrap();
        assert!(String::from_utf8(base64::decode(src).unwrap()).unwrap().contains("fill=\"#123456\""), "the drawing itself is inside");
    }

    #[test]
    fn a_page_with_only_a_png_shows_the_png_and_text_is_escaped_everywhere() {
        let mut nb = sample();
        nb.pages[0].charts[0].svg.clear();
        nb.pages[0].title = "A <b>bold</b> title".into();
        let page = String::from_utf8(export_notebook(NotebookFormat::Html, &nb).unwrap()).unwrap();
        assert!(page.contains("<img src=\"data:image/png;base64,"));
        assert!(page.contains("A &lt;b&gt;bold&lt;/b&gt; title") && !page.contains("<b>bold</b>"));
    }

    #[test]
    fn a_notebook_that_cannot_be_made_is_refused_with_a_reason() {
        let mut nb = sample();
        nb.pages.clear();
        assert!(export_notebook(NotebookFormat::Docx, &nb).unwrap_err().to_string().contains("nothing in the notebook"));
        let mut bad = sample();
        bad.pages[0].charts[0].png = base64::encode(b"not a picture");
        assert!(export_notebook(NotebookFormat::Docx, &bad).unwrap_err().to_string().contains("not a PNG"));
        bad.pages[0].charts[0].png = "!!!".into();
        assert!(export_notebook(NotebookFormat::Xlsx, &bad).unwrap_err().to_string().contains("base 64"));
        let mut size = sample();
        size.pages[0].charts[0].width = 0;
        assert!(export_notebook(NotebookFormat::Html, &size).unwrap_err().to_string().contains("no usable size"));
        let mut svg = sample();
        svg.pages[0].charts[0].svg = "<script>alert(1)</script>".into();
        assert!(export_notebook(NotebookFormat::Html, &svg).unwrap_err().to_string().contains("not an SVG"));
    }

    #[test]
    fn a_wide_chart_is_scaled_to_the_page() {
        let c = NotebookChart { title: String::new(), alt: String::new(), png: String::new(), svg: String::new(), width: 1_200, height: 600 };
        let (w, h) = emu_size(&c);
        assert_eq!(w, MAX_PICTURE_EMU);
        assert_eq!(h, 600 * 9_525 * MAX_PICTURE_EMU / (1_200 * 9_525));
        let small = NotebookChart { width: 300, height: 100, ..c };
        assert_eq!(emu_size(&small), (300 * 9_525, 100 * 9_525));
    }
}
