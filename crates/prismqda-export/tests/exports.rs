//! The exporters, checked by reading their files back with the importers and an XML parser.

use prismqda_engine::{NewCode, NewCoder, NewCoding, NewProject, NewSource, OpenOptions, Project, SegmentFilter, SourceType};
use prismqda_export::{export_codebook, export_segments, CodebookOptions, Format};
use prismqda_import::{docx, open_bytes, Cell, ImportOptions, Opened, Table};
use quick_xml::events::Event;
use quick_xml::Reader;
use tempfile::TempDir;

fn opts() -> OpenOptions {
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    o
}

struct Fx {
    p: Project,
}

/// A project with a themed code tree, definitions, two coders (one a model), codings, and an anchor example.
fn fixture(dir: &TempDir) -> Fx {
    let mut p = Project::create(dir.path().join("e.pqda"), &NewProject::new("Exports & \"quotes\""), &opts()).unwrap();
    let source = p.add_source(&NewSource::new("Interview 1", SourceType::Document, "I felt welcome on day one.\nThe fees, though, were a worry <and a half>.")).unwrap();
    let mut theme = NewCode::new("Belonging");
    theme.theme = true;
    theme.definition = "How people feel about being part of the group.".into();
    let theme_id = p.add_code(&theme).unwrap();
    let mut welcome = NewCode::new("Welcome");
    welcome.parent_id = Some(theme_id);
    welcome.definition = "Feeling received warmly.".into();
    welcome.inclusion_criteria = "Explicit warmth".into();
    welcome.exclusion_criteria = "Mere politeness".into();
    welcome.color = Some("#336699".into());
    let welcome_id = p.add_code(&welcome).unwrap();
    let cost_id = p.add_code(&NewCode::new("Cost")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let model = p
        .add_coder(&NewCoder { name: "Model A".into(), role: prismqda_engine::CoderRole::Model, local_id: None, provider: Some("Provider A".into()), model_name: Some("Model A".into()), model_version: None })
        .unwrap();
    let a = p.apply_coding(&NewCoding::new(source, 2, 14, welcome_id, ada)).unwrap();
    p.apply_coding(&NewCoding::new(source, 31, 35, cost_id, ada)).unwrap();
    p.apply_coding(&NewCoding::new(source, 2, 14, welcome_id, model)).unwrap();
    p.set_anchor(a, true).unwrap();
    Fx { p }
}

fn table(bytes: &[u8], name: &str) -> Table {
    match open_bytes(name, bytes, &ImportOptions::default()).unwrap() {
        Opened::Table(t) => t,
        Opened::Batch(_) => panic!("expected a table"),
    }
}

fn text(c: &Cell) -> String {
    c.text()
}

#[test]
fn the_codebook_in_excel_reads_back_as_a_table_in_tree_order() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    let bytes = export_codebook(&f.p, Format::Xlsx, &CodebookOptions::default()).unwrap();
    let t = table(&bytes, "codebook.xlsx");
    let names: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names[..4], ["Code", "Name", "Parent", "Type"]);
    let rows: Vec<Vec<String>> = t.rows.iter().map(|r| r.iter().map(text).collect()).collect();
    assert_eq!(rows.len(), 3);
    // The theme, then its child beneath it, then the other top-level code.
    assert_eq!(rows[0][0], "Belonging");
    assert_eq!(rows[1][0], "Belonging > Welcome");
    assert_eq!(rows[1][2], "Belonging");
    assert_eq!(rows[2][0], "Cost");
    assert_eq!(rows[0][3], "theme");
    assert_eq!(rows[1][5], "Feeling received warmly.");
    assert_eq!(rows[1][6], "Explicit warmth");
    assert_eq!(rows[1][7], "Mere politeness");
    assert_eq!(rows[1][8], "\u{201C}felt welcome\u{201D} (Interview 1)", "the anchor example, with where it came from");
    assert_eq!(rows[1][9], "2", "two codings use Welcome");
    assert_eq!(rows[1][10], "#336699");
}

#[test]
fn retired_codes_are_left_out_unless_asked_for() {
    let dir = TempDir::new().unwrap();
    let mut f = fixture(&dir);
    let cost = f.p.list_codes().unwrap().into_iter().find(|c| c.name == "Cost").unwrap().id;
    f.p.retire_code(cost).unwrap();
    let rows = |include| table(&export_codebook(&f.p, Format::Xlsx, &CodebookOptions { include_inactive: include }).unwrap(), "c.xlsx").rows.len();
    assert_eq!(rows(false), 2);
    assert_eq!(rows(true), 3);
}

#[test]
fn the_codebook_in_word_has_headings_definitions_and_example_quotes() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    let bytes = export_codebook(&f.p, Format::Docx, &CodebookOptions::default()).unwrap();
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    for want in ["Codebook: Exports & \"quotes\"", "Belonging", "Welcome", "Definition: Feeling received warmly.", "Include when: Explicit warmth", "Do not include when: Mere politeness", "felt welcome", "Example from Interview 1", "A theme: it groups other codes.", "Used 2 times."] {
        assert!(parsed.text.contains(want), "missing {want:?} in:\n{}", parsed.text);
    }
    // Real heading styles, so Word's navigation pane and screen readers can move by heading.
    let headings = parsed.runs.iter().filter(|r| r.kind == prismqda_engine::RunKind::Heading).count();
    assert!(headings >= 4, "{headings} headings");
}

#[test]
fn the_codebook_in_qdc_is_wellformed_nested_and_stable() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    let a = export_codebook(&f.p, Format::Qdc, &CodebookOptions::default()).unwrap();
    let b = export_codebook(&f.p, Format::Qdc, &CodebookOptions::default()).unwrap();
    assert_eq!(a, b, "the same codebook gives the same file, including identifiers");
    let xml = String::from_utf8(a).unwrap();

    let mut reader = Reader::from_str(&xml);
    let mut depth = 0usize;
    let mut found: Vec<(usize, String, String, String)> = Vec::new();
    let mut desc_for: Vec<String> = Vec::new();
    let mut capture = false;
    loop {
        match reader.read_event().unwrap() {
            Event::Start(e) => {
                match e.name().as_ref() {
                    b"CodeBook" => {
                        let ns = e.attributes().flatten().find(|a| a.key.as_ref() == b"xmlns").map(|a| String::from_utf8_lossy(&a.value).to_string()).unwrap_or_default();
                        assert_eq!(ns, "urn:QDA-XML:codebook:1.0");
                    }
                    b"Code" => {
                        let get = |k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k.as_bytes()).map(|a| String::from_utf8_lossy(&a.value).to_string()).unwrap_or_default();
                        found.push((depth, get("name"), get("isCodable"), get("color")));
                        assert_eq!(get("guid").len(), 36);
                        depth += 1;
                    }
                    b"Description" => capture = true,
                    _ => {}
                }
            }
            Event::Empty(e) if e.name().as_ref() == b"Code" => {
                let get = |k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k.as_bytes()).map(|a| String::from_utf8_lossy(&a.value).to_string()).unwrap_or_default();
                found.push((depth, get("name"), get("isCodable"), get("color")));
            }
            Event::Text(t) if capture => desc_for.push(t.decode().unwrap().to_string()),
            Event::End(e) => match e.name().as_ref() {
                b"Code" => depth -= 1,
                b"Description" => capture = false,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(found.len(), 3);
    assert_eq!((found[0].0, found[0].1.as_str(), found[0].2.as_str()), (0, "Belonging", "false"), "a theme is not codable");
    assert_eq!((found[1].0, found[1].1.as_str(), found[1].2.as_str(), found[1].3.as_str()), (1, "Welcome", "true", "#336699"), "the child is nested one level in");
    assert_eq!((found[2].0, found[2].1.as_str()), (0, "Cost"));
    assert!(desc_for.iter().any(|d| d.contains("Feeling received warmly.") && d.contains("Include when: Explicit warmth") && d.contains("Do not include when: Mere politeness")), "{desc_for:?}");
}

#[test]
fn coded_segments_in_excel_have_the_words_and_a_summary_sheet() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    let bytes = export_segments(&f.p, Format::Xlsx, &SegmentFilter::default()).unwrap();
    let t = table(&bytes, "segments.xlsx");
    let col = |name: &str| t.columns.iter().position(|c| c.name == name).unwrap_or_else(|| panic!("no column {name}"));
    assert_eq!(t.rows.len(), 3);
    let texts: Vec<String> = t.rows.iter().map(|r| text(&r[col("Text")])).collect();
    assert_eq!(texts, ["felt welcome", "felt welcome", "fees"]);
    let coders: Vec<String> = t.rows.iter().map(|r| text(&r[col("Coder")])).collect();
    assert!(coders.contains(&"Ada".to_string()) && coders.contains(&"Model A".to_string()));
    let kinds: Vec<String> = t.rows.iter().map(|r| text(&r[col("Coder type")])).collect();
    assert!(kinds.contains(&"model".to_string()), "a model's codings are labeled as a model's");
    assert_eq!(text(&t.rows[0][col("Start")]), "2");
    assert_eq!(text(&t.rows[0][col("End")]), "14");

    // Filtering by coder gives only that coder's rows.
    let ada = f.p.list_coders().unwrap().into_iter().find(|c| c.name == "Ada").unwrap().id;
    let only = export_segments(&f.p, Format::Xlsx, &SegmentFilter { coder_ids: Some(vec![ada]), ..Default::default() }).unwrap();
    assert_eq!(table(&only, "s.xlsx").rows.len(), 2);
}

#[test]
fn coded_segments_in_csv_open_in_excel_and_hold_awkward_text() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    // The second line of the source holds a comma and angle brackets; code all of it.
    let source = f.p.list_sources().unwrap()[0].id;
    let mut f = f;
    let code = f.p.list_codes().unwrap()[2].id;
    let coder = f.p.list_coders().unwrap()[0].id;
    f.p.apply_coding(&NewCoding::new(source, 27, 71, code, coder)).unwrap();
    let bytes = export_segments(&f.p, Format::Csv, &SegmentFilter::default()).unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "a byte order mark so Excel reads UTF-8");
    let t = table(&bytes[3..], "segments.csv");
    let text_col = t.columns.iter().position(|c| c.name == "Text").unwrap();
    let all: Vec<String> = t.rows.iter().map(|r| r[text_col].text()).collect();
    assert!(all.iter().any(|s| s.contains("fees, though") && s.contains("<and a half>")), "{all:?}");
}

#[test]
fn coded_segments_in_word_are_grouped_by_code_with_where_each_came_from() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    let bytes = export_segments(&f.p, Format::Docx, &SegmentFilter::default()).unwrap();
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(parsed.text.contains("Belonging > Welcome (2)"), "{}", parsed.text);
    assert!(parsed.text.contains("Cost (1)"));
    assert!(parsed.text.contains("Interview 1, characters 2 to 14, coded by Ada"));
    assert!(parsed.text.contains("coded by Model A (model)"));
    assert!(!parsed.text.contains("Belonging (0)"), "codes with nothing coded are left out");
}

#[test]
fn qdc_is_for_codebooks_only_and_filters_apply_to_word_too() {
    let dir = TempDir::new().unwrap();
    let f = fixture(&dir);
    assert!(export_segments(&f.p, Format::Qdc, &SegmentFilter::default()).is_err());
    let cost = f.p.list_codes().unwrap().into_iter().find(|c| c.name == "Cost").unwrap().id;
    let bytes = export_segments(&f.p, Format::Docx, &SegmentFilter { code_ids: Some(vec![cost]), ..Default::default() }).unwrap();
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(parsed.text.contains("Cost (1)") && !parsed.text.contains("Welcome"));
}

#[test]
fn an_empty_project_still_exports_a_valid_file() {
    let dir = TempDir::new().unwrap();
    let p = Project::create(dir.path().join("empty.pqda"), &NewProject::new("Empty"), &opts()).unwrap();
    for format in [Format::Xlsx, Format::Docx, Format::Csv, Format::Qdc] {
        let bytes = export_codebook(&p, format, &CodebookOptions::default()).unwrap();
        assert!(!bytes.is_empty(), "{format:?}");
    }
    for format in [Format::Xlsx, Format::Docx, Format::Csv] {
        assert!(!export_segments(&p, format, &SegmentFilter::default()).unwrap().is_empty(), "{format:?}");
    }
}

// ---- pictures ----------------------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in data {
        a = (a + u32::from(x)) % 65_521;
        b = (b + a) % 65_521;
    }
    (b << 16) | a
}

fn png_chunk(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = (body.len() as u32).to_be_bytes().to_vec();
    let mut crc_in = kind.to_vec();
    crc_in.extend_from_slice(body);
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out.extend_from_slice(&crc32(&crc_in).to_be_bytes());
    out
}

/// A real PNG that any program can show: red top left, green top right, blue bottom left, yellow bottom right. A crop of one quarter
/// is one colour, so a person looking at an exported file can see at once whether the right part was cut out.
fn quadrant_png(w: u32, h: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        for x in 0..w {
            let pixel: [u8; 3] = match (x >= w / 2, y >= h / 2) {
                (false, false) => [220, 40, 40],
                (true, false) => [40, 180, 60],
                (false, true) => [40, 80, 220],
                (true, true) => [240, 200, 40],
            };
            raw.extend_from_slice(&pixel);
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut parts = raw.chunks(65_535).peekable();
    while let Some(part) = parts.next() {
        z.push(u8::from(parts.peek().is_none()));
        z.extend_from_slice(&(part.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(part.len() as u16)).to_le_bytes());
        z.extend_from_slice(part);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut ihdr = w.to_be_bytes().to_vec();
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(png_chunk(b"IHDR", &ihdr));
    png.extend(png_chunk(b"IDAT", &z));
    png.extend(png_chunk(b"IEND", &[]));
    png
}

/// A JPEG header with a turn marker (EXIF orientation) and no picture data: enough for the project to take it as a photograph its camera turned.
fn turned_jpeg(w: u16, h: u16, orientation: u16) -> Vec<u8> {
    let mut b = vec![0xff, 0xd8];
    let mut tiff = b"MM\0\x2a\0\0\0\x08".to_vec();
    tiff.extend_from_slice(&[0, 1]);
    tiff.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1]);
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    let len = (2 + 6 + tiff.len()) as u16;
    b.extend_from_slice(&[0xff, 0xe1]);
    b.extend_from_slice(&len.to_be_bytes());
    b.extend_from_slice(b"Exif\0\0");
    b.extend_from_slice(&tiff);
    b.extend_from_slice(&[0xff, 0xc0, 0, 17, 8]);
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    b
}

fn import_picture(p: &mut Project, name: &str, file: &str, bytes: Vec<u8>) -> i64 {
    let image = prismqda_engine::read_image(file, bytes.clone()).unwrap();
    let text = prismqda_engine::default_description(&image.file_name, image.width, image.height);
    let len = text.chars().count() as i64;
    let batch = prismqda_engine::ImportBatch {
        sources: vec![prismqda_engine::ImportedSource {
            name: name.to_string(),
            source_type: SourceType::Other,
            folder: String::new(),
            text,
            runs: vec![],
            units: vec![prismqda_engine::ImportedUnit::new(0, len, prismqda_engine::UnitKind::Paragraph)],
            default_unit_kind: prismqda_engine::UnitKind::Paragraph,
            unit_attributes: vec![],
            original_file_hash: Some(prismqda_engine::sha256_hex(&bytes)),
            settings: serde_json::Value::Null,
            warnings: vec![],
        }],
        images: vec![prismqda_engine::ImageAttachment { source: 0, image }],
        ..Default::default()
    };
    p.apply_import(&batch).unwrap().source_ids[0]
}

struct Pics {
    p: Project,
    street: i64,
    phone: i64,
}

/// One upright picture with three areas (a rectangle, an ellipse and an outline) and one photograph its camera turned, with an area.
fn picture_fixture(dir: &TempDir) -> Pics {
    use prismqda_engine::{NewRegion, Shape};
    let mut p = Project::create(dir.path().join("pics.pqda"), &NewProject::new("Pictures"), &opts()).unwrap();
    let street = import_picture(&mut p, "Street", "street.png", quadrant_png(200, 150));
    let phone = import_picture(&mut p, "Phone", "phone.jpg", turned_jpeg(1000, 800, 6));
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_region(&NewRegion::new(street, Shape::Rectangle { x: 0.5, y: 0.0, w: 0.5, h: 0.5 }, joy, ada)).unwrap();
    p.apply_region(&NewRegion::new(street, Shape::Ellipse { x: 0.1, y: 0.5, w: 0.3, h: 0.4 }, joy, ada)).unwrap();
    p.apply_region(&NewRegion::new(street, Shape::Polygon { points: vec![[0.2, 0.1], [0.4, 0.1], [0.3, 0.3]] }, joy, ada)).unwrap();
    p.apply_region(&NewRegion::new(phone, Shape::Rectangle { x: 0.1, y: 0.1, w: 0.3, h: 0.3 }, joy, ada)).unwrap();
    Pics { p, street, phone }
}

fn zip_entry(bytes: &[u8], name: &str) -> String {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut out = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(name).unwrap_or_else(|_| panic!("no {name} in the file")), &mut out).unwrap();
    out
}

fn zip_names(bytes: &[u8]) -> Vec<String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    (0..zip.len()).map(|i| zip.by_index(i).unwrap().name().unwrap().to_string()).collect()
}

#[test]
fn areas_of_a_picture_are_cropped_out_of_one_copy_in_a_word_report() {
    let dir = TempDir::new().unwrap();
    let f = picture_fixture(&dir);
    let bytes = export_segments(&f.p, Format::Docx, &SegmentFilter::default()).unwrap();
    let names = zip_names(&bytes);
    assert_eq!(names.iter().filter(|n| n.starts_with("word/media/")).count(), 1, "three areas of one picture keep one copy: {names:?}");
    let document = zip_entry(&bytes, "word/document.xml");
    assert_eq!(document.matches("<pic:pic>").count(), 3, "the turned photograph is not shown");
    // The rectangle is the top right quarter: it cuts the left half and the bottom half away.
    assert!(document.contains("<a:srcRect l=\"50000\" t=\"0\" r=\"0\" b=\"50000\"/>"), "{document}");
    // The ellipse is shown as the box around it.
    assert!(document.contains("<a:srcRect l=\"10000\" t=\"50000\" r=\"60000\" b=\"10000\"/>"));
    // The outline is shown as the box around it.
    assert!(document.contains("<a:srcRect l=\"20000\" t=\"10000\" r=\"60000\" b=\"70000\"/>"));
    assert!(document.contains("descr=\"Area of the picture Street, coded Joy: Rectangle,"), "a person who cannot see the area is told where it is");
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(parsed.text.contains("Street, an area of the picture, coded by Ada"), "{}", parsed.text);
    assert!(parsed.text.contains("Phone, an area of the picture (the picture is not shown in this file), coded by Ada"), "{}", parsed.text);
    // The picture file in the report is the one that was imported, byte for byte.
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut kept = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name("word/media/image1.png").unwrap(), &mut kept).unwrap();
    assert_eq!(kept, f.p.read_image_bytes(f.street).unwrap().1);
}

#[test]
fn areas_of_a_picture_sit_in_a_column_of_their_own_in_an_excel_workbook() {
    let dir = TempDir::new().unwrap();
    let f = picture_fixture(&dir);
    let bytes = export_segments(&f.p, Format::Xlsx, &SegmentFilter::default()).unwrap();
    let t = table(&bytes, "segments.xlsx");
    assert!(t.columns.iter().any(|c| c.name == "Picture"), "{:?}", t.columns.iter().map(|c| &c.name).collect::<Vec<_>>());
    assert_eq!(t.rows.len(), 4);
    let names = zip_names(&bytes);
    assert_eq!(names.iter().filter(|n| n.starts_with("xl/media/")).count(), 1, "{names:?}");
    let drawing = zip_entry(&bytes, "xl/drawings/drawing1.xml");
    assert_eq!(drawing.matches("<xdr:pic>").count(), 3);
    assert!(drawing.contains("<a:srcRect l=\"50000\" t=\"0\" r=\"0\" b=\"50000\"/>"));
    let sheet = zip_entry(&bytes, "xl/worksheets/sheet1.xml");
    assert!(sheet.contains("customHeight=\"1\"") && sheet.contains("<drawing r:id=\"rId1\"/>"));
    assert!(zip_entry(&bytes, "[Content_Types].xml").contains("/xl/drawings/drawing1.xml"));

    // A workbook with only the turned photograph has nothing to show, so it has no picture column at all.
    let only = export_segments(&f.p, Format::Xlsx, &SegmentFilter { source_ids: Some(vec![f.phone]), ..Default::default() }).unwrap();
    assert!(!table(&only, "p.xlsx").columns.iter().any(|c| c.name == "Picture"));
    assert!(!zip_names(&only).iter().any(|n| n.contains("drawing") || n.contains("media")));
}

#[test]
fn csv_tells_an_area_in_words_and_keeps_no_picture() {
    let dir = TempDir::new().unwrap();
    let f = picture_fixture(&dir);
    let bytes = export_segments(&f.p, Format::Csv, &SegmentFilter::default()).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    assert_eq!(text.matches("[Area of a picture] ").count(), 4, "{text}");
}

// ---- tables the data editor makes ---------------------------------------------------------------------

#[test]
fn a_table_is_saved_as_csv_or_as_a_workbook_that_keeps_numbers_as_numbers_and_identifiers_as_text() {
    let header = vec!["Site".to_string(), "Cases".to_string(), "Code".to_string()];
    let rows = vec![
        vec![Some("North, East".to_string()), Some("12".to_string()), Some("007".to_string())],
        vec![Some("South".to_string()), Some("-3.5".to_string()), None],
        vec![None, Some("1e3".to_string()), Some("+44".to_string())],
    ];
    let csv = String::from_utf8(prismqda_export::export_table("Groups", &header, &rows, Format::Csv).unwrap()).unwrap();
    assert_eq!(csv, "\u{FEFF}Site,Cases,Code\r\n\"North, East\",12,007\r\nSouth,-3.5,\r\n,1e3,+44\r\n");
    let xlsx = prismqda_export::export_table("Groups", &header, &rows, Format::Xlsx).unwrap();
    let t = table(&xlsx, "groups.xlsx");
    assert_eq!(t.columns.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Site", "Cases", "Code"]);
    assert_eq!(t.rows.len(), 3);
    assert!(matches!(t.rows[0][1], Cell::Number(n) if n == 12.0), "a plain number is a number");
    assert!(matches!(&t.rows[0][2], Cell::Text(s) if s == "007"), "a leading zero keeps it text");
    assert!(matches!(t.rows[1][1], Cell::Number(n) if n == -3.5));
    assert!(matches!(&t.rows[2][1], Cell::Text(s) if s == "1e3"), "an exponent is left as written");
    assert!(prismqda_export::export_table("x", &header, &rows, Format::Docx).is_err());
}

// ---- areas of a PDF page ------------------------------------------------------------------------------

/// The three-page PDF the importer's own tests use, imported as a PDF document that keeps its file, with two areas coded on its first page
/// and one on its third.
fn pdf_fixture(dir: &TempDir) -> (Project, i64) {
    use prismqda_engine::{NewRegion, Shape};
    let path = format!("{}/../prismqda-import/tests/fixtures/pdf/three_pages.pdf", env!("CARGO_MANIFEST_DIR"));
    let bytes = std::fs::read(&path).unwrap();
    let Opened::Batch(batch) = open_bytes("Garden.pdf", &bytes, &ImportOptions::default()).unwrap() else { panic!("a batch") };
    let mut p = Project::create(dir.path().join("pdf.pqda"), &NewProject::new("PDFs"), &opts()).unwrap();
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let joy = p.add_code(&NewCode::new("Joy")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let mut on = |page: i64, x: f64, y: f64| {
        let mut r = NewRegion::new(source, Shape::Rectangle { x, y, w: 0.3, h: 0.1 }, joy, ada);
        r.page = page;
        p.apply_region(&r).unwrap();
    };
    on(1, 0.1, 0.05);
    on(1, 0.1, 0.2);
    on(3, 0.1, 0.1);
    (p, source)
}

#[test]
fn areas_of_pdf_pages_are_cropped_out_of_the_drawn_page_in_a_word_report() {
    let dir = TempDir::new().unwrap();
    let (p, _) = pdf_fixture(&dir);
    let bytes = export_segments(&p, Format::Docx, &SegmentFilter::default()).unwrap();
    let names = zip_names(&bytes);
    assert_eq!(names.iter().filter(|n| n.starts_with("word/media/")).count(), 2, "one drawn copy of each page that has an area: {names:?}");
    let document = zip_entry(&bytes, "word/document.xml");
    assert_eq!(document.matches("<pic:pic>").count(), 3, "an area each");
    assert!(document.contains("descr=\"Area of page 1 of Garden, coded Joy: Page 1, Rectangle,"), "{document}");
    assert!(document.contains("descr=\"Area of page 3 of Garden, coded Joy: Page 3, Rectangle,"));
    // The first area is the top left of the page: the left tenth and the top twentieth are cut away, and the rest below and to the right.
    assert!(document.contains("<a:srcRect l=\"10000\" t=\"5000\" r=\"60000\" b=\"85000\"/>"), "{document}");
    let parsed = docx::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(parsed.text.contains("Garden, an area of page 1, coded by Ada"), "{}", parsed.text);
    assert!(parsed.text.contains("Garden, an area of page 3, coded by Ada"), "{}", parsed.text);
    // What is kept in the report is a PNG of the page.
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let mut kept = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_name("word/media/image1.png").unwrap(), &mut kept).unwrap();
    assert!(kept.starts_with(&[0x89, b'P', b'N', b'G']));
}

#[test]
fn areas_of_pdf_pages_sit_in_a_column_of_their_own_in_an_excel_workbook_and_csv_names_the_page() {
    let dir = TempDir::new().unwrap();
    let (p, _) = pdf_fixture(&dir);
    let bytes = export_segments(&p, Format::Xlsx, &SegmentFilter::default()).unwrap();
    let t = table(&bytes, "segments.xlsx");
    assert!(t.columns.iter().any(|c| c.name == "Picture"));
    assert_eq!(t.rows.len(), 3);
    assert_eq!(zip_names(&bytes).iter().filter(|n| n.starts_with("xl/media/")).count(), 2);
    assert_eq!(zip_entry(&bytes, "xl/drawings/drawing1.xml").matches("<xdr:pic>").count(), 3);
    let csv = String::from_utf8(export_segments(&p, Format::Csv, &SegmentFilter::default()).unwrap()).unwrap();
    assert_eq!(csv.matches("[Area of a PDF page] Page ").count(), 3, "{csv}");
}

/// A recording that stands for its document, with one coded stretch, beside a document of words with one coding.
fn clip_fixture(dir: &TempDir) -> Project {
    use prismqda_engine::{NewRegion, Shape};
    let mut p = Project::create(dir.path().join("clips.pqda"), &NewProject::new("Clips"), &opts()).unwrap();
    let source = p.add_source(&NewSource::new("Field recording", SourceType::Other, "Audio recording talk.wav. Add a description.")).unwrap();
    let wav = dir.path().join("talk.wav");
    std::fs::write(&wav, vec![0u8; 400]).unwrap();
    p.link_media_as(source, wav.to_str().unwrap(), Some(true)).unwrap();
    let words = p.add_source(&NewSource::new("Notes", SourceType::Document, "Some words here.")).unwrap();
    let humor = p.add_code(&NewCode::new("Humor")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    p.apply_region(&NewRegion::new(source, Shape::Span { start_ms: 12_000, end_ms: 45_500 }, humor, ada)).unwrap();
    p.apply_coding(&NewCoding::new(words, 0, 4, humor, ada)).unwrap();
    p
}

#[test]
fn csv_and_excel_tell_a_stretch_of_a_recording_in_words_and_in_seconds() {
    let dir = TempDir::new().unwrap();
    let p = clip_fixture(&dir);
    let csv = String::from_utf8(export_segments(&p, Format::Csv, &SegmentFilter::default()).unwrap()).unwrap();
    assert!(csv.lines().next().unwrap().ends_with("Recording from (seconds),Recording to (seconds)"), "{csv}");
    assert!(csv.contains("[Stretch of a recording] Recording, 0:12.0 to 0:45.5 (0:33.5 long)"), "{csv}");
    assert!(csv.contains(",12,45.5"), "the times are in seconds in columns of their own: {csv}");
    // Words keep their place, and a stretch has none.
    let words_row = csv.lines().find(|l| l.starts_with("Notes,")).unwrap();
    assert!(words_row.contains(",0,4,") && words_row.ends_with(",,"), "{words_row}");

    let bytes = export_segments(&p, Format::Xlsx, &SegmentFilter::default()).unwrap();
    let t = table(&bytes, "segments.xlsx");
    assert!(t.columns.iter().any(|c| c.name == "Recording from (seconds)") && t.columns.iter().any(|c| c.name == "Recording to (seconds)"));
    assert_eq!(t.rows.len(), 2);
    assert!(zip_names(&bytes).iter().all(|n| !n.starts_with("xl/media/")), "a stretch has no picture");

    // With no stretch coded, the table is what it always was.
    let only_words = export_segments(&p, Format::Csv, &SegmentFilter { source_ids: Some(vec![2]), ..SegmentFilter::default() }).unwrap();
    assert!(!String::from_utf8(only_words).unwrap().contains("Recording from"), "no columns for stretches when there are none");
}

#[test]
fn a_word_report_names_a_stretch_by_its_times_and_shows_no_picture() {
    let dir = TempDir::new().unwrap();
    let p = clip_fixture(&dir);
    let bytes = export_segments(&p, Format::Docx, &SegmentFilter::default()).unwrap();
    let doc = zip_entry(&bytes, "word/document.xml");
    assert!(doc.contains("Recording, 0:12.0 to 0:45.5 (0:33.5 long)"), "the stretch is the quote");
    assert!(doc.contains("a stretch of the recording"), "and where it came from says what it is");
    assert!(!zip_names(&bytes).iter().any(|n| n.contains("media")), "no picture is made for a stretch");
}

// ---- the report ---------------------------------------------------------------------------------------

/// A document of four paragraphs, each a unit, two codes, two people who both coded every unit, and an agreement run kept in the project.
fn study_fixture(dir: &TempDir) -> Project {
    use prismqda_engine::{AgreementRequest, ImportBatch, ImportedSource, ImportedUnit, UnitKind};
    let mut p = Project::create(dir.path().join("report.pqda"), &NewProject::new("Riders study"), &opts()).unwrap();
    let paragraphs = ["Fares are too high for me.", "The stop is far from my house.", "I love the new buses.", "The driver was rude to me."];
    let mut text = String::new();
    let mut units = Vec::new();
    for para in paragraphs {
        let start = text.chars().count() as i64;
        text.push_str(para);
        units.push(ImportedUnit::new(start, text.chars().count() as i64, UnitKind::Paragraph));
        text.push_str("\n\n");
    }
    let batch = ImportBatch {
        sources: vec![ImportedSource {
            name: "Riders".into(),
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
    };
    let source = p.apply_import(&batch).unwrap().source_ids[0];
    let mut cost = NewCode::new("Cost");
    cost.definition = "Talk about money.".into();
    let cost = p.add_code(&cost).unwrap();
    let access = p.add_code(&NewCode::new("Access")).unwrap();
    let ada = p.add_coder(&NewCoder::human("Ada")).unwrap();
    let ben = p.add_coder(&NewCoder::human("Ben")).unwrap();
    for (i, u) in p.list_units(source).unwrap().iter().enumerate() {
        let (a, b) = [(cost, cost), (access, access), (access, cost), (cost, cost)][i];
        let mut first = NewCoding::new(source, u.start, u.end, a, ada);
        first.unit_id = Some(u.id);
        p.apply_coding(&first).unwrap();
        let mut second = NewCoding::new(source, u.start, u.end, b, ben);
        second.unit_id = Some(u.id);
        p.apply_coding(&second).unwrap();
    }
    let request: AgreementRequest = serde_json::from_value(serde_json::json!({ "coder_ids": [ada, ben], "bootstrap_replications": 50 })).unwrap();
    p.agreement_run(&request).unwrap();
    p
}

fn report_text(bytes: &[u8]) -> String {
    docx::parse(bytes, &ImportOptions::default()).unwrap().text
}

#[test]
fn a_report_holds_the_sections_asked_for_and_says_only_what_the_project_holds() {
    use prismqda_engine::{MemoKind, MemoScope, NewMemo};
    use prismqda_export::{export_report, ReportOptions, ReportSection};
    let dir = TempDir::new().unwrap();
    let mut p = study_fixture(&dir);
    p.add_memo(&NewMemo { kind: MemoKind::Analytic, scope: MemoScope::Project, target_id: None, phase: None, title: "Money keeps coming up".into(), body: "Fares first.\n\nThen the stops.".into() }).unwrap();
    p.annotate("Decided to code whole paragraphs.").unwrap();

    // The usual sections.
    let usual = report_text(&export_report(&p, &ReportOptions::default()).unwrap());
    for want in [
        "Report: Riders study",
        "The study",
        "Documents",
        "Codebook",
        "Agreement between coders",
        "Run 1",
        "Use of AI",
        "No AI was used in this project.",
        "Methods appendix (draft)",
        "Ada (a person) and Ben (a person)",
        "agreement between Ada and Ben was measured on 4 units with one code for each unit",
        "Krippendorff's alpha was",
        "Talk about money.",
    ] {
        assert!(usual.contains(want), "the report says {want:?}:\n{usual}");
    }
    assert!(!usual.contains("Money keeps coming up"), "memos are not in the usual report");
    assert!(!usual.contains("Decision timeline"));

    // Other sections, a title of the person's own, and the order the report keeps whatever the order asked for.
    let chosen = ReportOptions { title: Some("  My report  ".into()), sections: Some(vec![ReportSection::Timeline, ReportSection::Memos]) };
    let other = report_text(&export_report(&p, &chosen).unwrap());
    assert!(other.starts_with("My report"), "{other}");
    assert!(other.contains("Money keeps coming up") && other.contains("Fares first.") && other.contains("Then the stops."));
    assert!(other.contains("Analytic memo about the project"));
    assert!(other.contains("Decision timeline") && other.contains("Decided to code whole paragraphs.") && other.contains("Imported Riders.") && other.contains("Measured agreement between coders on 4 units."));
    assert!(other.find("Memos").unwrap() < other.find("Decision timeline").unwrap(), "memos come before the timeline");
    assert!(!other.contains("Codebook") && !other.contains("Methods appendix"));

    // Nothing to say is said, and nothing chosen is refused.
    let empty = Project::create(dir.path().join("empty.pqda"), &NewProject::new("Empty"), &opts()).unwrap();
    let nothing = report_text(&export_report(&empty, &ReportOptions::default()).unwrap());
    assert!(nothing.contains("The project holds no documents.") && nothing.contains("The codebook has no active codes.") && nothing.contains("No agreement between coders was measured in this project."), "{nothing}");
    assert!(nothing.contains("Agreement between coders was not measured."));
    assert!(export_report(&p, &ReportOptions { title: None, sections: Some(vec![]) }).is_err());
}

#[test]
fn a_report_tells_the_decisions_that_settled_disagreements_and_says_when_there_are_none() {
    use prismqda_export::{export_report, ReportOptions, ReportSection};
    let dir = TempDir::new().unwrap();
    let mut p = study_fixture(&dir);
    let ask = |p: &Project, sections: Vec<ReportSection>| report_text(&export_report(p, &ReportOptions { title: None, sections: Some(sections) }).unwrap());
    let before = ask(&p, vec![ReportSection::Adjudication, ReportSection::Methods]);
    assert!(before.contains("No unit that coders coded differently has been settled."), "{before}");
    assert!(!before.contains("Adjudication
") || !before.contains("were settled by"), "the methods say nothing of adjudication that did not happen");

    let source = p.list_sources().unwrap()[0].id;
    let unit = p.list_units(source).unwrap()[2].id;
    let ada = p.list_coders().unwrap().into_iter().find(|c| c.name == "Ada").unwrap().id;
    let cost = p.list_codes().unwrap().into_iter().find(|c| c.name == "Cost").unwrap().id;
    p.decide(unit, &[cost], "The words are about the price of a ticket.", ada).unwrap();

    let after = ask(&p, vec![ReportSection::Adjudication, ReportSection::Methods, ReportSection::Timeline]);
    for want in [
        "Adjudicated decisions",
        "1 unit was settled. Each decision is kept apart from every coder's own coding, which was not changed.",
        "I love the new buses.",
        "The words are about the price of a ticket.",
        "1 unit that coders had coded differently was settled by Ada, each with a written reason.",
        "Settled a unit that coders had coded differently.",
    ] {
        assert!(after.contains(want), "the report says {want:?}:
{after}");
    }
}

#[test]
fn the_coded_passages_of_a_report_crop_the_areas_of_pictures() {
    use prismqda_export::{export_report, ReportOptions, ReportSection};
    let dir = TempDir::new().unwrap();
    let pics = picture_fixture(&dir);
    let bytes = export_report(&pics.p, &ReportOptions { title: None, sections: Some(vec![ReportSection::Passages]) }).unwrap();
    let document = zip_entry(&bytes, "word/document.xml");
    assert_eq!(document.matches("<pic:pic>").count(), 3, "the three areas of the upright picture are shown, the turned photograph is not");
    assert!(document.contains("<a:srcRect l=\"50000\" t=\"0\" r=\"0\" b=\"50000\"/>"));
    assert_eq!(zip_names(&bytes).iter().filter(|n| n.starts_with("word/media/")).count(), 1);
    assert!(report_text(&bytes).contains("Coded passages"));
}

/// For checking the files in real programs: with `PRISMQDA_EXPORT_SAMPLES` set to a folder, writes one sample of
/// every format there. Does nothing otherwise.
#[test]
fn writes_sample_files_for_manual_checks() {
    let Some(dir) = std::env::var_os("PRISMQDA_EXPORT_SAMPLES").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&dir).unwrap();
    let temp = TempDir::new().unwrap();
    let f = fixture(&temp);
    // A report with every section, from a small study with two coders and a run of agreement.
    let study_temp = TempDir::new().unwrap();
    let study = study_fixture(&study_temp);
    let every = prismqda_export::ReportOptions { title: None, sections: Some(vec![
        prismqda_export::ReportSection::Study, prismqda_export::ReportSection::Documents, prismqda_export::ReportSection::Codebook, prismqda_export::ReportSection::Passages,
        prismqda_export::ReportSection::Agreement, prismqda_export::ReportSection::Memos, prismqda_export::ReportSection::AiUse, prismqda_export::ReportSection::Methods, prismqda_export::ReportSection::Timeline,
    ]) };
    std::fs::write(dir.join("report.docx"), prismqda_export::export_report(&study, &every).unwrap()).unwrap();
    // Areas of a picture: in Word and Excel the top right quarter must be all green, and the box round the ellipse the lower left.
    let pics_temp = TempDir::new().unwrap();
    let pics = picture_fixture(&pics_temp);
    for format in [Format::Docx, Format::Xlsx] {
        std::fs::write(dir.join(format!("pictures.{}", format.extension())), export_segments(&pics.p, format, &SegmentFilter::default()).unwrap()).unwrap();
    }
    for format in [Format::Xlsx, Format::Docx, Format::Csv, Format::Qdc] {
        let name = format!("codebook.{}", format.extension());
        std::fs::write(dir.join(name), export_codebook(&f.p, format, &CodebookOptions::default()).unwrap()).unwrap();
    }
    for format in [Format::Xlsx, Format::Docx, Format::Csv] {
        let name = format!("segments.{}", format.extension());
        std::fs::write(dir.join(name), export_segments(&f.p, format, &SegmentFilter::default()).unwrap()).unwrap();
    }
}

#[test]
fn the_mixed_methods_section_says_the_design_the_log_and_the_matrix_and_nothing_the_project_does_not_hold() {
    use prismqda_engine::{DesignApply, DesignInput, IntegrationInput, TriangulationInput};
    use prismqda_export::{export_report, ReportOptions, ReportSection};
    let dir = TempDir::new().unwrap();
    let mut p = study_fixture(&dir);
    let ask = |p: &Project| report_text(&export_report(p, &ReportOptions { title: None, sections: Some(vec![ReportSection::MixedMethods]) }).unwrap());

    // Nothing kept, and it says so.
    let nothing = ask(&p);
    assert!(nothing.contains("Mixed methods") && nothing.contains("The project holds no mixed methods design, integration log, or triangulation matrix."), "{nothing}");
    // It is not one of the usual sections.
    let usual = report_text(&export_report(&p, &ReportOptions::default()).unwrap());
    assert!(!usual.contains("Mixed methods"), "the usual report leaves it out");

    p.design_apply(&DesignApply {
        design: DesignInput { kind: "explanatory_sequential".into(), priority: None, purpose: "To explain why some riders stopped riding.".into() },
        log: "Chose the explanatory sequential design.".into(),
    })
    .unwrap();
    p.integration_add(&IntegrationInput { on_date: Some("2026-03-02".into()), stage: "interpretation".into(), how: "connecting".into(), what: "Used the interviews to explain the scores.".into(), why: "The scores alone did not say why.".into() }).unwrap();
    let code = p.list_codes().unwrap().into_iter().find(|c| c.status == "active").expect("the fixture has a code").id;
    p.triangulation_add(&TriangulationInput {
        topic: "Cost".into(),
        code_id: Some(code),
        variable: String::new(),
        qualitative: "Riders talk about fares.".into(),
        quantitative: "Fares were the main reason in the survey.".into(),
        mark: "convergence".into(),
        note: "Both strands agree.".into(),
    })
    .unwrap();
    p.triangulation_add(&TriangulationInput { topic: "Safety".into(), code_id: None, variable: String::new(), qualitative: String::new(), quantitative: String::new(), mark: String::new(), note: String::new() }).unwrap();
    let text = ask(&p);
    for want in [
        "The study follows an explanatory sequential design: the quantitative strand comes first and the qualitative strand is used to explain its results. The quantitative strand has priority.",
        "Purpose of mixing the two kinds of data: To explain why some riders stopped riding.",
        "Where and how the two kinds of data were brought together",
        "Chose the explanatory sequential design.",
        "2026-03-02",
        "Used the interviews to explain the scores.",
        "The scores alone did not say why.",
        "Triangulation of the findings",
        "compared on 2 topics",
        "marked 1 as convergence, 0 as complementarity, and 0 as discordance; 1 not marked.",
        "Riders talk about fares.",
        "Both strands agree.",
        "not marked",
    ] {
        assert!(text.contains(want), "the section says {want:?}:\n{text}");
    }
    assert!(text.find("Where and how").unwrap() < text.find("Triangulation of the findings").unwrap(), "the log comes before the matrix");
}
