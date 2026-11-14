//! A small Excel (.xlsx) writer: sheets of text and numbers with a bold, frozen header row.
//!
//! Written from the Office Open XML spreadsheet format directly, so there is nothing to audit beyond the
//! zip library. Text goes in as inline strings; long text wraps. Limits that Excel enforces are checked
//! here so a file is never produced that Excel would call corrupt.

use std::io::Write;

use crate::docx::{Crop, Media};
use crate::xml::escape;
use crate::{zip_package, ExportError};

/// Excel's own limits.
pub const MAX_ROWS: usize = 1_048_576;
pub const MAX_CELL_CHARS: usize = 32_767;
const MAX_SHEET_NAME: usize = 31;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Text(String),
    /// Text in bold, for a title or a row of column names inside a sheet.
    Head(String),
    Number(f64),
    Empty,
}

impl From<&str> for Cell {
    fn from(s: &str) -> Self {
        Cell::Text(s.to_string())
    }
}

impl From<String> for Cell {
    fn from(s: String) -> Self {
        Cell::Text(s)
    }
}

impl From<i64> for Cell {
    fn from(n: i64) -> Self {
        Cell::Number(n as f64)
    }
}

/// Part of a picture shown in a cell of a sheet. The row is made tall enough for it.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetPicture {
    /// The picture's place in the list given to `write_xlsx_with_media`.
    pub media: usize,
    pub crop: Crop,
    /// The place in `rows` (0 is the first row under the header) and the column (0 is A).
    pub row: usize,
    pub col: usize,
    /// How large it is shown, in English metric units (914,400 to the inch, 12,700 to the point).
    pub width: i64,
    pub height: i64,
    /// What a person who cannot see the picture hears.
    pub alt: String,
}

/// The tallest a row can be in Excel, in points.
const MAX_ROW_POINTS: f64 = 409.0;

/// One sheet: a header row and the rows beneath it.
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub header: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
    /// Column widths in characters; columns past the end of this list get a default.
    pub widths: Vec<f64>,
    /// Parts of pictures shown on the sheet.
    pub pictures: Vec<SheetPicture>,
}

/// `A`, `B`, ... `Z`, `AA`, ...
fn column_name(mut index: usize) -> String {
    let mut name = String::new();
    loop {
        name.insert(0, (b'A' + (index % 26) as u8) as char);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    name
}

/// A sheet name Excel accepts: at most 31 characters, none of `: \ / ? * [ ]`, not empty, and not starting or ending with an apostrophe.
fn sheet_name(wanted: &str, taken: &[String]) -> String {
    let mut base: String = wanted.chars().map(|c| if ":\\/?*[]".contains(c) { '-' } else { c }).collect::<String>().trim_matches('\'').trim().to_string();
    if base.is_empty() {
        base = "Sheet".to_string();
    }
    let mut name: String = base.chars().take(MAX_SHEET_NAME).collect();
    let mut n = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
        let suffix = format!(" ({n})");
        name = format!("{}{suffix}", base.chars().take(MAX_SHEET_NAME - suffix.chars().count()).collect::<String>());
        n += 1;
    }
    name
}

fn cell_xml(reference: &str, cell: &Cell, style: u8) -> String {
    match cell {
        Cell::Empty => String::new(),
        Cell::Number(n) if n.is_finite() => format!("<c r=\"{reference}\" s=\"{style}\"><v>{n}</v></c>"),
        Cell::Number(_) => String::new(),
        Cell::Text(t) | Cell::Head(t) => {
            let style = if matches!(cell, Cell::Head(_)) { 1 } else { style };
            let cut: String = if t.chars().count() > MAX_CELL_CHARS { t.chars().take(MAX_CELL_CHARS - 1).chain(std::iter::once('\u{2026}')).collect() } else { t.clone() };
            format!("<c r=\"{reference}\" s=\"{style}\" t=\"inlineStr\"><is><t xml:space=\"preserve\">{}</t></is></c>", escape(&cut))
        }
    }
}

fn sheet_xml(sheet: &Sheet) -> String {
    let columns = sheet.header.len().max(sheet.rows.iter().map(Vec::len).max().unwrap_or(0)).max(1);
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
    // A row that holds a picture is as tall as the picture.
    let mut heights: std::collections::HashMap<usize, f64> = std::collections::HashMap::new();
    for pic in &sheet.pictures {
        let points = (pic.height as f64 / 12_700.0 + 4.0).min(MAX_ROW_POINTS);
        let h = heights.entry(pic.row).or_insert(points);
        *h = h.max(points);
    }
    xml.push_str("<sheetViews><sheetView workbookViewId=\"0\"><pane ySplit=\"1\" topLeftCell=\"A2\" activePane=\"bottomLeft\" state=\"frozen\"/></sheetView></sheetViews>");
    xml.push_str("<cols>");
    for c in 0..columns {
        let width = sheet.widths.get(c).copied().unwrap_or(18.0).clamp(4.0, 80.0);
        xml.push_str(&format!("<col min=\"{0}\" max=\"{0}\" width=\"{width}\" customWidth=\"1\"/>", c + 1));
    }
    xml.push_str("</cols><sheetData>");
    xml.push_str("<row r=\"1\">");
    for (c, h) in sheet.header.iter().enumerate() {
        xml.push_str(&cell_xml(&format!("{}1", column_name(c)), &Cell::Text(h.clone()), 1));
    }
    xml.push_str("</row>");
    for (r, row) in sheet.rows.iter().enumerate() {
        let number = r + 2;
        match heights.get(&r) {
            Some(points) => xml.push_str(&format!("<row r=\"{number}\" ht=\"{points:.2}\" customHeight=\"1\">")),
            None => xml.push_str(&format!("<row r=\"{number}\">")),
        }
        for (c, cell) in row.iter().enumerate() {
            xml.push_str(&cell_xml(&format!("{}{number}", column_name(c)), cell, 2));
        }
        xml.push_str("</row>");
    }
    xml.push_str("</sheetData>");
    if !sheet.header.is_empty() {
        xml.push_str(&format!("<autoFilter ref=\"A1:{}1\"/>", column_name(sheet.header.len() - 1)));
    }
    if !sheet.pictures.is_empty() {
        xml.push_str("<drawing r:id=\"rId1\"/>");
    }
    xml.push_str("</worksheet>");
    xml
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="2"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="3"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0" applyAlignment="1"><alignment vertical="top" wrapText="1"/></xf></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#;

/// The XML of the pictures on one sheet. Each is set in the cell it names, at the size it names, cropped to its part.
fn drawing_xml(pictures: &[SheetPicture]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">");
    for (i, pic) in pictures.iter().enumerate() {
        let id = i + 2;
        let (l, t, r, b) = pic.crop.cut();
        let alt = escape(&pic.alt);
        // The header is the first row, so a picture for row 0 of the data sits in the sheet's second row (counted from 0, row 1).
        xml.push_str(&format!(
            "<xdr:oneCellAnchor><xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>38100</xdr:colOff><xdr:row>{row}</xdr:row><xdr:rowOff>25400</xdr:rowOff></xdr:from><xdr:ext cx=\"{w}\" cy=\"{h}\"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"{id}\" name=\"Picture {n}\" descr=\"{alt}\"/><xdr:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></xdr:cNvPicPr></xdr:nvPicPr><xdr:blipFill><a:blip r:embed=\"rIdM{m}\"/><a:srcRect l=\"{l}\" t=\"{t}\" r=\"{r}\" b=\"{b}\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{w}\" cy=\"{h}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:oneCellAnchor>",
            col = pic.col,
            row = pic.row + 1,
            w = pic.width,
            h = pic.height,
            n = i + 1,
            m = pic.media + 1,
        ));
    }
    xml.push_str("</xdr:wsDr>");
    xml
}

fn scrub_sheets(sheets: &[Sheet]) -> Vec<Sheet> {
    let t = |s: &String| crate::scrub::text(s).into_owned();
    sheets
        .iter()
        .map(|s| Sheet {
            name: t(&s.name),
            header: s.header.iter().map(t).collect(),
            rows: s
                .rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|c| match c {
                            Cell::Text(x) => Cell::Text(t(x)),
                            Cell::Head(x) => Cell::Head(t(x)),
                            other => other.clone(),
                        })
                        .collect()
                })
                .collect(),
            widths: s.widths.clone(),
            pictures: s.pictures.iter().map(|p| SheetPicture { alt: t(&p.alt), ..p.clone() }).collect(),
        })
        .collect()
}

/// The workbook as `.xlsx` bytes.
pub fn write_xlsx(sheets: &[Sheet]) -> Result<Vec<u8>, ExportError> {
    write_xlsx_with_media(sheets, &[])
}

/// The workbook with pictures kept in it. Each file in `media` is stored once, however many cells show a part of it.
pub fn write_xlsx_with_media(sheets: &[Sheet], media: &[Media]) -> Result<Vec<u8>, ExportError> {
    // While a list of names is applied to what leaves the project, every text is passed through it here, where the file is made.
    let scrubbed;
    let sheets: &[Sheet] = if crate::scrub::active() {
        scrubbed = scrub_sheets(sheets);
        &scrubbed
    } else {
        sheets
    };
    if sheets.is_empty() {
        return Err(ExportError::Invalid("a workbook needs at least one sheet".to_string()));
    }
    for s in sheets {
        if s.rows.len() + 1 > MAX_ROWS {
            return Err(ExportError::TooLarge(format!("the sheet '{}' would have {} rows, but Excel allows {MAX_ROWS}; export as CSV instead", s.name, s.rows.len() + 1)));
        }
        for pic in &s.pictures {
            if pic.media >= media.len() {
                return Err(ExportError::Invalid("a picture on a sheet names a file that was not given".to_string()));
            }
            if !pic.crop.is_valid() || pic.width <= 0 || pic.height <= 0 || pic.row >= s.rows.len() || pic.col >= 16_384 {
                return Err(ExportError::Invalid("a picture on a sheet has no part to show, no size, or no cell".to_string()));
            }
        }
    }
    let mut names: Vec<String> = Vec::new();
    for s in sheets {
        let n = sheet_name(&s.name, &names);
        names.push(n);
    }

    // Each kind of picture file is named once, as the default for its ending.
    let mut kinds: Vec<(&str, &str)> = Vec::new();
    for m in media {
        if !kinds.iter().any(|(e, _)| *e == m.extension) {
            kinds.push((m.extension, m.content_type));
        }
    }
    let defaults: String = kinds.iter().map(|(e, c)| format!("<Default Extension=\"{e}\" ContentType=\"{c}\"/>")).collect();
    let mut content_types = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/>{defaults}<Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/><Override PartName=\"/xl/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml\"/>");
    let mut workbook = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><sheets>");
    let mut rels = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">");
    for (i, name) in names.iter().enumerate() {
        let n = i + 1;
        content_types.push_str(&format!("<Override PartName=\"/xl/worksheets/sheet{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>"));
        if !sheets[i].pictures.is_empty() {
            content_types.push_str(&format!("<Override PartName=\"/xl/drawings/drawing{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawing+xml\"/>"));
        }
        workbook.push_str(&format!("<sheet name=\"{}\" sheetId=\"{n}\" r:id=\"rId{n}\"/>", escape(name)));
        rels.push_str(&format!("<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet{n}.xml\"/>"));
    }
    content_types.push_str("</Types>");
    workbook.push_str("</sheets></workbook>");
    rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/></Relationships>", names.len() + 1));
    let root_rels = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/></Relationships>";

    zip_package(|zip, options| {
        let mut put = |name: &str, content: &[u8]| -> Result<(), ExportError> {
            zip.start_file(name, options).map_err(|e| ExportError::Io(e.to_string()))?;
            zip.write_all(content).map_err(|e| ExportError::Io(e.to_string()))
        };
        put("[Content_Types].xml", content_types.as_bytes())?;
        put("_rels/.rels", root_rels.as_bytes())?;
        put("xl/workbook.xml", workbook.as_bytes())?;
        put("xl/_rels/workbook.xml.rels", rels.as_bytes())?;
        put("xl/styles.xml", STYLES.as_bytes())?;
        for (i, s) in sheets.iter().enumerate() {
            let n = i + 1;
            put(&format!("xl/worksheets/sheet{n}.xml"), sheet_xml(s).as_bytes())?;
            if !s.pictures.is_empty() {
                put(
                    &format!("xl/worksheets/_rels/sheet{n}.xml.rels"),
                    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing\" Target=\"../drawings/drawing{n}.xml\"/></Relationships>").as_bytes(),
                )?;
                put(&format!("xl/drawings/drawing{n}.xml"), drawing_xml(&s.pictures).as_bytes())?;
                let mut used: Vec<usize> = s.pictures.iter().map(|p| p.media).collect();
                used.sort_unstable();
                used.dedup();
                let targets: String = used
                    .iter()
                    .map(|m| format!("<Relationship Id=\"rIdM{0}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/image{0}.{1}\"/>", m + 1, media[*m].extension))
                    .collect();
                put(
                    &format!("xl/drawings/_rels/drawing{n}.xml.rels"),
                    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{targets}</Relationships>").as_bytes(),
                )?;
            }
        }
        for (i, m) in media.iter().enumerate() {
            put(&format!("xl/media/image{}.{}", i + 1, m.extension), &m.bytes)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters_run_past_z() {
        let names: Vec<String> = [0, 1, 25, 26, 27, 51, 52, 701, 702].iter().map(|i| column_name(*i)).collect();
        assert_eq!(names, ["A", "B", "Z", "AA", "AB", "AZ", "BA", "ZZ", "AAA"]);
    }

    #[test]
    fn sheet_names_are_made_legal_and_unique() {
        let taken: Vec<String> = Vec::new();
        assert_eq!(sheet_name("Codes: a/b [x]?", &taken), "Codes- a-b -x--");
        assert_eq!(sheet_name("", &taken), "Sheet");
        assert_eq!(sheet_name("'quoted'", &taken), "quoted");
        let long = "x".repeat(40);
        assert_eq!(sheet_name(&long, &taken).chars().count(), 31);
        let taken = vec!["Codes".to_string()];
        assert_eq!(sheet_name("codes", &taken), "codes (2)");
        let taken = vec!["x".repeat(31)];
        let second = sheet_name(&"x".repeat(40), &taken);
        assert_eq!(second.chars().count(), 31);
        assert!(second.ends_with(" (2)"));
    }

    #[test]
    fn a_cropped_picture_sits_in_its_cell_and_makes_its_row_tall() {
        let media = [Media { extension: "png", content_type: "image/png", bytes: vec![9, 9, 9] }];
        let crop = Crop { x0: 0.2, y0: 0.25, x1: 0.7, y1: 0.75 };
        let picture = |row: usize| SheetPicture { media: 0, crop, row, col: 2, width: 1_905_000, height: 1_270_000, alt: "An area & more".into() };
        let sheet = Sheet {
            name: "S".into(),
            header: vec!["a".into(), "b".into(), "Picture".into()],
            rows: vec![vec![Cell::Text("x".into())], vec![Cell::Text("y".into())], vec![Cell::Text("z".into())]],
            widths: vec![],
            pictures: vec![picture(0), picture(2)],
        };
        let xml = sheet_xml(&sheet);
        assert!(xml.contains("<row r=\"2\" ht=\"104.00\" customHeight=\"1\">"), "100 points for the picture and 4 to breathe: {xml}");
        assert!(xml.contains("<row r=\"3\">"), "a row without a picture keeps its height");
        assert!(xml.contains("<row r=\"4\" ht=\"104.00\" customHeight=\"1\">"));
        assert!(xml.contains("<drawing r:id=\"rId1\"/>") && xml.contains("xmlns:r="));
        let drawing = drawing_xml(&sheet.pictures);
        assert!(drawing.contains("<xdr:col>2</xdr:col>") && drawing.contains("<xdr:row>1</xdr:row>") && drawing.contains("<xdr:row>3</xdr:row>"), "{drawing}");
        assert!(drawing.contains("<a:srcRect l=\"20000\" t=\"25000\" r=\"30000\" b=\"25000\"/>"));
        assert!(drawing.contains("descr=\"An area &amp; more\""));
        assert_eq!(drawing.matches("r:embed=\"rIdM1\"").count(), 2);
        let bytes = write_xlsx_with_media(&[sheet], &media).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect();
        for want in ["xl/drawings/drawing1.xml", "xl/drawings/_rels/drawing1.xml.rels", "xl/worksheets/_rels/sheet1.xml.rels", "xl/media/image1.png"] {
            assert!(names.iter().any(|n| n == want), "{want} is in {names:?}");
        }
        assert_eq!(names.iter().filter(|n| n.starts_with("xl/media/")).count(), 1, "kept once");
        let mut types = String::new();
        std::io::Read::read_to_string(&mut zip.by_name("[Content_Types].xml").unwrap(), &mut types).unwrap();
        assert!(types.contains("/xl/drawings/drawing1.xml") && types.contains("Extension=\"png\""));
    }

    #[test]
    fn a_picture_on_a_sheet_with_no_file_or_no_cell_is_refused() {
        let media = [Media { extension: "png", content_type: "image/png", bytes: vec![0] }];
        let good = Crop { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 };
        let sheet = |media: usize, row: usize| Sheet {
            name: "S".into(),
            header: vec!["a".into()],
            rows: vec![vec![Cell::Empty]],
            widths: vec![],
            pictures: vec![SheetPicture { media, crop: good, row, col: 0, width: 10, height: 10, alt: String::new() }],
        };
        assert!(write_xlsx_with_media(&[sheet(1, 0)], &media).is_err(), "no such file");
        assert!(write_xlsx_with_media(&[sheet(0, 1)], &media).is_err(), "no such row");
        assert!(write_xlsx_with_media(&[sheet(0, 0)], &media).is_ok());
    }

    #[test]
    fn an_empty_workbook_and_an_over_long_sheet_are_refused() {
        assert!(matches!(write_xlsx(&[]), Err(ExportError::Invalid(_))));
        let big = Sheet { name: "Big".into(), header: vec!["a".into()], rows: vec![vec![Cell::Empty]; MAX_ROWS], widths: vec![], pictures: vec![] };
        assert!(matches!(write_xlsx(&[big]), Err(ExportError::TooLarge(_))));
    }
}
