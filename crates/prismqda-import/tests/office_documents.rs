//! Tests for the DOCX, ODT and RTF importers. The packages are built here, in test code, with a zip
//! writer, so every test shows exactly what the importer was given. All text is invented.

mod common;

use common::*;
use prismqda_engine::{FormatRun, RunKind};
use prismqda_import::docx;

const STYLES_BASIC: &str = r#"
<w:docDefaults><w:rPrDefault><w:rPr><w:b w:val="0"/></w:rPr></w:rPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:i/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Kopf3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/></w:style>
<w:style w:type="paragraph" w:styleId="Chapter"><w:name w:val="Chapter"/><w:basedOn w:val="Heading1"/></w:style>
<w:style w:type="paragraph" w:styleId="ListBullet"><w:name w:val="List Bullet"/><w:basedOn w:val="Normal"/><w:pPr><w:numPr><w:numId w:val="3"/></w:numPr></w:pPr></w:style>
<w:style w:type="character" w:styleId="Strong"><w:name w:val="Strong"/><w:rPr><w:b/></w:rPr></w:style>
<w:style w:type="character" w:styleId="Quiet"><w:name w:val="Quiet"/><w:basedOn w:val="Strong"/><w:rPr><w:i/></w:rPr></w:style>
"#;

#[test]
fn docx_plain_paragraphs_are_separated_by_a_blank_line() {
    let parsed = docx::parse(&docx(&format!("{}{}", p("First paragraph."), p("Second paragraph."))), &opts()).unwrap();
    assert_eq!(parsed.text, "First paragraph.\n\nSecond paragraph.");
    assert!(parsed.runs.is_empty());
    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.source_type, prismqda_engine::SourceType::Document);
    assert_eq!(parsed.unit_kind, prismqda_engine::UnitKind::Sentence);
    assert!(parsed.units.is_none());
}

#[test]
fn docx_headings_come_from_style_ids_names_outline_levels_and_parent_styles() {
    let body = concat!(
        r#"<w:p><w:pPr><w:pStyle w:val="Title"/></w:pPr><w:r><w:t>The Study</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Method</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Sample</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Kopf3"/></w:pPr><w:r><w:t>Recruitment</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Chapter"/></w:pPr><w:r><w:t>Findings</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:outlineLvl w:val="3"/></w:pPr><w:r><w:t>Direct outline</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:outlineLvl w:val="9"/></w:pPr><w:r><w:t>Body after all</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>Just text.</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx_with_styles(body, STYLES_BASIC), &opts()).unwrap();
    assert_eq!(
        parsed.text,
        "The Study\n\nMethod\n\nSample\n\nRecruitment\n\nFindings\n\nDirect outline\n\nBody after all\n\nJust text."
    );
    let headings: Vec<(String, Option<u8>)> =
        parsed.runs.iter().filter(|r| r.kind == RunKind::Heading).map(|r| (covered(&parsed, r), r.level)).collect();
    assert_eq!(
        headings,
        vec![
            ("The Study".to_string(), Some(1)),
            ("Method".to_string(), Some(1)),
            ("Sample".to_string(), Some(2)),
            ("Recruitment".to_string(), Some(3)),
            ("Findings".to_string(), Some(1)),
            ("Direct outline".to_string(), Some(4)),
        ]
    );
    // Heading styles are bold (and italic) as a look; that is not emphasis inside the text. The one
    // paragraph that has a heading style but was explicitly set to body text is just bold text.
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["Body after all"]);
    assert!(runs_of(&parsed, RunKind::Italic).is_empty());
}

#[test]
fn docx_headings_work_without_a_styles_part() {
    let body = r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Plain</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Heading9"/></w:pPr><w:r><w:t>Deep</w:t></w:r></w:p>"#;
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    let levels: Vec<Option<u8>> = parsed.runs.iter().map(|r| r.level).collect();
    assert_eq!(levels, vec![Some(2), Some(6)], "level 9 is capped at 6");
}

#[test]
fn docx_bold_and_italic_runs_cover_exactly_the_marked_words() {
    let body = concat!(
        r#"<w:p>"#,
        r#"<w:r><w:t xml:space="preserve">plain </w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">bold words </w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>both</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve"> more bold</w:t></w:r>"#,
        r#"<w:r><w:t xml:space="preserve"> then </w:t></w:r>"#,
        r#"<w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b w:val="0"/><w:i w:val="false"/></w:rPr><w:t xml:space="preserve"> off</w:t></w:r>"#,
        r#"</w:p>"#
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "plain bold words both more bold then italic off");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["bold words both more bold"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["both", "italic"]);
}

#[test]
fn docx_character_and_paragraph_styles_give_bold_and_italic() {
    let styles = format!(
        "{STYLES_BASIC}{}",
        r#"<w:style w:type="paragraph" w:styleId="Shout"><w:name w:val="Shout"/><w:basedOn w:val="Normal"/><w:rPr><w:b/></w:rPr></w:style>"#
    );
    let body = concat!(
        r#"<w:p><w:r><w:t xml:space="preserve">see </w:t></w:r><w:r><w:rPr><w:rStyle w:val="Strong"/></w:rPr><w:t>strong</w:t></w:r><w:r><w:t xml:space="preserve"> and </w:t></w:r><w:r><w:rPr><w:rStyle w:val="Quiet"/></w:rPr><w:t>quiet</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Shout"/></w:pPr><w:r><w:t>All bold paragraph</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Shout"/></w:pPr><w:r><w:rPr><w:b w:val="0"/></w:rPr><w:t>Switched off</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx_with_styles(body, &styles), &opts()).unwrap();
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["strong", "quiet", "All bold paragraph"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["quiet"]);
}

#[test]
fn docx_lists_mark_the_paragraph_without_inventing_bullets() {
    let body = concat!(
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>First point</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Nested point</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="ListBullet"/></w:pPr><w:r><w:t>From the style</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="ListBullet"/><w:numPr><w:numId w:val="0"/></w:numPr></w:pPr><w:r><w:t>Numbering removed</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>Not a list</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx_with_styles(body, STYLES_BASIC), &opts()).unwrap();
    assert_eq!(parsed.text, "First point\n\nNested point\n\nFrom the style\n\nNumbering removed\n\nNot a list");
    assert_eq!(runs_of(&parsed, RunKind::ListItem), vec!["First point", "Nested point", "From the style"]);
}

#[test]
fn docx_numbered_headings_stay_headings_not_list_items() {
    let styles = r#"<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:pPr><w:numPr><w:numId w:val="2"/></w:numPr><w:outlineLvl w:val="0"/></w:pPr></w:style>"#;
    let body = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Numbered heading</w:t></w:r></w:p>"#;
    let parsed = docx::parse(&docx_with_styles(body, styles), &opts()).unwrap();
    assert_eq!(runs_of(&parsed, RunKind::Heading), vec!["Numbered heading"]);
    assert!(runs_of(&parsed, RunKind::ListItem).is_empty());
}

#[test]
fn docx_tables_use_tabs_between_cells_and_newlines_between_rows() {
    let body = format!(
        "{}<w:tbl><w:tblPr><w:tblW w:w=\"0\"/></w:tblPr><w:tblGrid><w:gridCol/><w:gridCol/></w:tblGrid>{}{}{}</w:tbl>{}",
        p("Before the table."),
        r#"<w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc><w:tcPr><w:tcW w:w="100"/></w:tcPr><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Role</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>Ana</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Nurse</w:t></w:r></w:p><w:p><w:r><w:t>and teacher</w:t></w:r></w:p></w:tc></w:tr>"#,
        r#"<w:tr><w:tc><w:p/></w:tc><w:tc><w:p><w:r><w:t>Only second</w:t></w:r></w:p></w:tc></w:tr>"#,
        p("After the table."),
    );
    let parsed = docx::parse(&docx(&body), &opts()).unwrap();
    assert_eq!(
        parsed.text,
        "Before the table.\n\nName\tRole\nAna\tNurse\nand teacher\n\tOnly second\n\nAfter the table."
    );
    assert_eq!(runs_of(&parsed, RunKind::TableCell), vec!["Name", "Role", "Ana", "Nurse\nand teacher", "Only second"]);
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["Role"]);
}

#[test]
fn docx_nested_tables_are_flattened_with_the_same_conventions() {
    let inner = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>i1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>i2</w:t></w:r></w:p></w:tc></w:tr><w:tr><w:tc><w:p><w:r><w:t>i3</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>i4</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#;
    let body = format!(
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>outer</w:t></w:r></w:p>{inner}<w:p><w:r><w:t>tail</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>side</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#
    );
    let parsed = docx::parse(&docx(&body), &opts()).unwrap();
    assert_eq!(parsed.text, "outer\ni1\ti2\ni3\ti4\ntail\tside");
    assert_eq!(
        runs_of(&parsed, RunKind::TableCell),
        vec!["outer\ni1\ti2\ni3\ti4\ntail", "i1", "i2", "i3", "i4", "side"]
    );
}

#[test]
fn docx_a_tab_inside_a_table_cell_cannot_be_mistaken_for_a_cell_separator() {
    let body = r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p><w:r><w:t>x</w:t><w:tab/><w:t>y</w:t></w:r></w:p>"#;
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "a b\tc\n\nx\ty");
}

#[test]
fn docx_line_breaks_tabs_hyphens_and_fields() {
    let body = concat!(
        r#"<w:p><w:r><w:t>one</w:t><w:br/><w:t>two</w:t><w:tab/><w:t>three</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>well</w:t><w:noBreakHyphen/><w:t>known</w:t><w:softHyphen/></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>page</w:t></w:r><w:r><w:br w:type="page"/></w:r><w:r><w:t>next</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> HYPERLINK "http://example.invalid/" </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>link text</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
        r#"<w:p><w:hyperlink r:id="rId5"><w:r><w:t>second link</w:t></w:r></w:hyperlink><w:r><w:t xml:space="preserve"> after</w:t></w:r></w:p>"#,
        r#"<w:p><w:fldSimple w:instr=" DATE "><w:r><w:t>today</w:t></w:r></w:fldSimple></w:p>"#,
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "one\ntwo\tthree\n\nwell-known\n\npage\nnext\n\nlink text\n\nsecond link after\n\ntoday");
}

#[test]
fn docx_content_controls_and_smart_tags_pass_their_text_through() {
    let body = concat!(
        r#"<w:sdt><w:sdtPr><w:alias w:val="Title"/><w:rPr><w:b/></w:rPr><w:text/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Inside a block control</w:t></w:r></w:p></w:sdtContent></w:sdt>"#,
        r#"<w:p><w:r><w:t xml:space="preserve">Before </w:t></w:r><w:sdt><w:sdtPr><w:id w:val="5"/></w:sdtPr><w:sdtContent><w:r><w:t>inline control</w:t></w:r></w:sdtContent></w:sdt><w:smartTag w:uri="x" w:element="place"><w:r><w:t xml:space="preserve"> and tag</w:t></w:r></w:smartTag></w:p>"#,
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "Inside a block control\n\nBefore inline control and tag");
    assert!(parsed.runs.is_empty(), "the properties of a content control are not run formatting");
}

#[test]
fn docx_tracked_insertions_are_kept_and_deletions_are_left_out() {
    let body = concat!(
        r#"<w:p><w:r><w:t xml:space="preserve">We kept </w:t></w:r>"#,
        r#"<w:ins w:id="1" w:author="A" w:date="2020-01-01T00:00:00Z"><w:r><w:t xml:space="preserve">the new </w:t></w:r></w:ins>"#,
        r#"<w:del w:id="2" w:author="A" w:date="2020-01-01T00:00:00Z"><w:r><w:delText xml:space="preserve">old </w:delText></w:r></w:del>"#,
        r#"<w:r><w:t>words</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:rPr><w:ins w:id="3" w:author="A"/></w:rPr></w:pPr><w:r><w:t>Second</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "We kept the new words\n\nSecond");
    let w = warning(&parsed, "tracked_changes").expect("a warning about tracked changes");
    assert!(w.message.contains("1 insertion was kept"), "{}", w.message);
    assert!(w.message.contains("1 deletion was left out"), "{}", w.message);
}

#[test]
fn docx_images_become_one_placeholder_each_and_one_warning() {
    let drawing = r#"<w:drawing><wp:inline><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:blipFill><a:blip r:embed="rId9"/></pic:blipFill></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"#;
    let old_style = r#"<w:pict><v:shape><v:imagedata r:id="rId10"/></v:shape></w:pict>"#;
    // Word writes the same picture twice: once for new readers (Choice) and once for old (Fallback).
    let alternate = format!(
        r#"<w:r><mc:AlternateContent><mc:Choice Requires="wps">{drawing}</mc:Choice><mc:Fallback>{old_style}</mc:Fallback></mc:AlternateContent></w:r>"#
    );
    let body = format!(
        r#"<w:p><w:r><w:t xml:space="preserve">Before </w:t></w:r><w:r>{drawing}</w:r><w:r><w:t xml:space="preserve"> middle </w:t></w:r><w:r>{old_style}</w:r></w:p><w:p>{alternate}</w:p>"#
    );
    let parsed = docx::parse(&docx(&body), &opts()).unwrap();
    assert_eq!(parsed.text, "Before [image] middle [image]\n\n[image]");
    let w = warning(&parsed, "skipped_image").unwrap();
    assert!(w.message.starts_with("3 embedded images were replaced by the placeholder [image]"), "{}", w.message);
    assert_eq!(parsed.warnings.iter().filter(|w| w.kind == "skipped_image").count(), 1, "one aggregated warning");
}

#[test]
fn docx_things_that_cannot_be_carried_over_are_counted_in_warnings() {
    let body = concat!(
        // A text box (modern drawing form): its paragraphs must not leak into the text.
        r#"<w:p><w:r><w:t>Body text</w:t></w:r><w:r><w:drawing><wp:anchor><a:graphic><a:graphicData><wps:wsp><wps:txbx><w:txbxContent><w:p><w:r><w:t>Hidden in a box</w:t></w:r></w:p></w:txbxContent></wps:txbx></wps:wsp></a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"#,
        // A footnote reference, a comment reference and an equation.
        r#"<w:p><w:r><w:t>Claim</w:t></w:r><w:r><w:footnoteReference w:id="2"/></w:r><w:commentRangeStart w:id="0"/><w:r><w:t xml:space="preserve"> text</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r><m:oMath><m:r><m:t>x=1</m:t></m:r></m:oMath></w:p>"#,
        // Hidden text, ruby text and an embedded document.
        r#"<w:p><w:r><w:t xml:space="preserve">Shown </w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:t>secret</w:t></w:r><w:ruby><w:rt><w:r><w:t>kana</w:t></w:r></w:rt><w:rubyBase><w:r><w:t>base</w:t></w:r></w:rubyBase></w:ruby><w:altChunk r:id="rId7"/></w:p>"#,
        // A shape without a picture or a text box.
        r#"<w:p><w:r><w:drawing><wp:inline><a:graphic><a:graphicData><wps:wsp/></a:graphicData></a:graphic></wp:inline></w:drawing></w:r><w:r><w:t>end</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "Body text\n\nClaim text\n\nShown base\n\nend");
    for (kind, needle) in [
        ("skipped_text_box", "1 text box could not be imported"),
        ("skipped_footnote", "1 footnote or endnote was left out"),
        ("skipped_comment", "1 comment was left out"),
        ("skipped_equation", "1 equation could not be imported"),
        ("skipped_hidden_text", "1 piece of hidden text was left out"),
        ("skipped_phonetic_guide", "1 phonetic guide"),
        ("skipped_embedded_document", "1 embedded document could not be imported"),
        ("skipped_shape", "1 drawing object"),
    ] {
        let w = warning(&parsed, kind).unwrap_or_else(|| panic!("missing warning {kind}: {:?}", parsed.warnings));
        assert!(w.message.contains(needle), "{kind}: {}", w.message);
    }
    assert!(warning(&parsed, "skipped_image").is_none());
}

#[test]
fn docx_complex_script_formatting_uses_the_complex_script_settings() {
    // Hebrew text is bold when bCs is set; a plain w:b does not make Arabic bold (that is what Word shows).
    let body = r#"<w:p><w:r><w:rPr><w:bCs/><w:iCs/></w:rPr><w:t>שלום עולם</w:t></w:r><w:r><w:t xml:space="preserve"> hello </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>مرحبا</w:t></w:r></w:p>"#;
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    assert_eq!(parsed.text, "שלום עולם hello مرحبا");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["שלום עולם"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["שלום עולם"]);
}

#[test]
fn docx_offsets_count_scalar_values_not_bytes() {
    // Accented letters, CJK, a letter and a separate combining accent, and an emoji above U+FFFF.
    let body = concat!(
        r#"<w:p><w:r><w:t xml:space="preserve">café 日本語 </w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>😀 emoji</w:t></w:r><w:r><w:t xml:space="preserve"> e</w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>&#x301;nd</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Σοφία</w:t></w:r></w:p>"#,
    );
    let parsed = docx::parse(&docx(body), &opts()).unwrap();
    // "e" + combining acute composes to one character, so the text is in NFC.
    assert_eq!(parsed.text, "café 日本語 😀 emoji \u{e9}nd\n\nΣοφία");
    assert_sound(&parsed);
    let bold: Vec<&FormatRun> = parsed.runs.iter().filter(|r| r.kind == RunKind::Bold).collect();
    assert_eq!(bold.len(), 2);
    assert_eq!((bold[0].start, bold[0].end), (9, 16), "'😀 emoji' is 7 scalar values and starts after 9");
    assert_eq!(covered(&parsed, bold[0]), "😀 emoji");
    assert_eq!(covered(&parsed, bold[1]), "Σοφία");
    // The accent belongs to the letter before it, so the whole composed character is in the earlier run's style.
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["nd"]);
    assert!(parsed.text.len() > parsed.text.chars().count(), "the text has multi-byte characters");
}
