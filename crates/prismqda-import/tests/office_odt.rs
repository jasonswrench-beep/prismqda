//! ODT importer tests. The packages are built in test code; all text is invented.

mod common;

use common::*;
use prismqda_engine::{FormatRun, RunKind};
use prismqda_import::odt;

const AUTO_STYLES: &str = r#"
<style:style style:name="Bold" style:family="text"><style:text-properties fo:font-weight="bold" style:font-weight-asian="bold" style:font-weight-complex="bold"/></style:style>
<style:style style:name="Ital" style:family="text"><style:text-properties fo:font-style="italic" style:font-style-asian="italic" style:font-style-complex="italic"/></style:style>
<style:style style:name="BoldItal" style:family="text" style:parent-style-name="Bold"><style:text-properties fo:font-style="italic"/></style:style>
<style:style style:name="Strongish" style:family="text" style:parent-style-name="Strong_20_Emphasis"/>
<style:style style:name="AsianBold" style:family="text"><style:text-properties style:font-weight-asian="bold"/></style:style>
<style:style style:name="ComplexBold" style:family="text"><style:text-properties style:font-weight-complex="bold"/></style:style>
<style:style style:name="NotBold" style:family="text"><style:text-properties fo:font-weight="normal"/></style:style>
<style:style style:name="Hidden" style:family="text"><style:text-properties text:display="none"/></style:style>
<style:style style:name="P1" style:family="paragraph" style:parent-style-name="Standard"><style:text-properties fo:font-weight="bold"/></style:style>
"#;

const NAMED_STYLES: &str = r#"
<style:style style:name="Standard" style:family="paragraph" style:class="text"/>
<style:style style:name="Strong_20_Emphasis" style:display-name="Strong Emphasis" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>
<style:style style:name="Heading" style:family="paragraph" style:parent-style-name="Standard"><style:text-properties fo:font-size="14pt" fo:font-weight="bold"/></style:style>
<style:style style:name="Heading_20_1" style:display-name="Heading 1" style:family="paragraph" style:parent-style-name="Heading" style:default-outline-level="1"><style:text-properties fo:font-weight="bold"/></style:style>
<style:style style:name="Heading_20_2" style:display-name="Heading 2" style:family="paragraph" style:parent-style-name="Heading" style:default-outline-level="2"/>
<style:style style:name="Chapter" style:family="paragraph" style:parent-style-name="Heading_20_2"/>
<style:style style:name="Title" style:family="paragraph" style:parent-style-name="Heading"/>
"#;

fn parse(body: &str) -> prismqda_import::ParsedText {
    let parsed = odt::parse(&odt_with_styles(AUTO_STYLES, NAMED_STYLES, body), &opts()).unwrap();
    assert_sound(&parsed);
    parsed
}

#[test]
fn odt_plain_paragraphs_are_separated_by_a_blank_line() {
    let parsed =
        odt::parse(&odt("", "<text:p>First paragraph.</text:p><text:p>Second paragraph.</text:p>"), &opts()).unwrap();
    assert_eq!(parsed.text, "First paragraph.\n\nSecond paragraph.");
    assert!(parsed.runs.is_empty());
    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.source_type, prismqda_engine::SourceType::Document);
    assert_eq!(parsed.unit_kind, prismqda_engine::UnitKind::Sentence);
    assert!(parsed.units.is_none());
}

#[test]
fn odt_headings_come_from_outline_levels_style_levels_names_and_parents() {
    let body = concat!(
        r#"<text:p text:style-name="Title">The Study</text:p>"#,
        r#"<text:h text:style-name="Heading_20_1" text:outline-level="1">Method</text:h>"#,
        r#"<text:h text:style-name="Heading_20_2" text:outline-level="2">Sample</text:h>"#,
        r#"<text:p text:style-name="Chapter">Chapter by parent style</text:p>"#,
        r#"<text:h text:outline-level="9">Level capped</text:h>"#,
        r#"<text:p text:style-name="Heading_20_4">By name only</text:p>"#,
        r#"<text:p text:style-name="Standard">Just text.</text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(
        parsed.text,
        "The Study\n\nMethod\n\nSample\n\nChapter by parent style\n\nLevel capped\n\nBy name only\n\nJust text."
    );
    let headings: Vec<(String, Option<u8>)> =
        parsed.runs.iter().filter(|r| r.kind == RunKind::Heading).map(|r| (covered(&parsed, r), r.level)).collect();
    assert_eq!(
        headings,
        vec![
            ("The Study".to_string(), Some(1)),
            ("Method".to_string(), Some(1)),
            ("Sample".to_string(), Some(2)),
            ("Chapter by parent style".to_string(), Some(2)),
            ("Level capped".to_string(), Some(6)),
            ("By name only".to_string(), Some(4)),
        ]
    );
    // The heading styles are bold as a look; that is not emphasis in the text.
    assert!(runs_of(&parsed, RunKind::Bold).is_empty());
}

#[test]
fn odt_spans_give_bold_and_italic_with_parent_styles() {
    let body = concat!(
        r#"<text:p>plain <text:span text:style-name="Bold">bold</text:span> and <text:span text:style-name="Ital">italic</text:span> and "#,
        r#"<text:span text:style-name="BoldItal">both</text:span> and <text:span text:style-name="Strongish">parent</text:span>.</text:p>"#,
        r#"<text:p text:style-name="P1">Whole bold paragraph</text:p>"#,
        r#"<text:p text:style-name="P1">Switched <text:span text:style-name="NotBold">off</text:span> inside</text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(
        parsed.text,
        "plain bold and italic and both and parent.\n\nWhole bold paragraph\n\nSwitched off inside"
    );
    assert_eq!(
        runs_of(&parsed, RunKind::Bold),
        vec!["bold", "both", "parent", "Whole bold paragraph", "Switched", "inside"]
    );
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["italic", "both"]);
}

#[test]
fn odt_xml_white_space_collapses_and_explicit_spaces_tabs_and_breaks_are_kept() {
    let body = "<text:p>  Leading   spaces\n     collapse<text:s text:c=\"3\"/>after<text:tab/>tab<text:line-break/>next line</text:p>\
                <text:p>one <text:span text:style-name=\"Bold\"> two</text:span> <text:s/>three</text:p>";
    let parsed = parse(body);
    assert_eq!(parsed.text, "Leading spaces collapse   after\ttab\nnext line\n\none two  three");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["two"]);
}

#[test]
fn odt_lists_mark_items_and_headings_in_lists_stay_headings() {
    let body = concat!(
        r#"<text:list text:style-name="L1">"#,
        r#"<text:list-header><text:p>Header item</text:p></text:list-header>"#,
        r#"<text:list-item><text:p>First</text:p><text:list><text:list-item><text:p>Nested</text:p></text:list-item></text:list></text:list-item>"#,
        r#"<text:list-item><text:h text:outline-level="2">Heading in list</text:h></text:list-item>"#,
        r#"</text:list>"#,
        r#"<text:p>After list</text:p>"#,
        r#"<text:numbered-paragraph text:level="1"><text:p>Numbered</text:p></text:numbered-paragraph>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Header item\n\nFirst\n\nNested\n\nHeading in list\n\nAfter list\n\nNumbered");
    assert_eq!(runs_of(&parsed, RunKind::ListItem), vec!["Header item", "First", "Nested", "Numbered"]);
    assert_eq!(runs_of(&parsed, RunKind::Heading), vec!["Heading in list"]);
}

#[test]
fn odt_tables_use_tabs_between_cells_and_newlines_between_rows() {
    let body = concat!(
        r#"<text:p>Before the table.</text:p>"#,
        r#"<table:table table:name="T1"><table:table-column table:number-columns-repeated="2"/>"#,
        r#"<table:table-header-rows><table:table-row>"#,
        r#"<table:table-cell office:value-type="string"><text:p>Name</text:p></table:table-cell>"#,
        r#"<table:table-cell><text:p><text:span text:style-name="Bold">Role</text:span></text:p></table:table-cell>"#,
        r#"</table:table-row></table:table-header-rows>"#,
        r#"<table:table-row><table:table-cell><text:p>Ana</text:p></table:table-cell>"#,
        r#"<table:table-cell><text:p>Nurse</text:p><text:p>and teacher</text:p></table:table-cell></table:table-row>"#,
        r#"<table:table-row><table:table-cell table:number-columns-spanned="2"><text:p>Spans</text:p></table:table-cell><table:covered-table-cell/></table:table-row>"#,
        r#"<table:table-row table:number-rows-repeated="900000000"><table:table-cell table:number-columns-repeated="900000000"/></table:table-row>"#,
        r#"</table:table>"#,
        r#"<text:p>After the table.</text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Before the table.\n\nName\tRole\nAna\tNurse\nand teacher\nSpans\t\n\nAfter the table.");
    assert_eq!(runs_of(&parsed, RunKind::TableCell), vec!["Name", "Role", "Ana", "Nurse\nand teacher", "Spans"]);
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["Role"]);
}

#[test]
fn odt_nested_tables_and_tabs_inside_cells() {
    let inner = r#"<table:table><table:table-row><table:table-cell><text:p>i1</text:p></table:table-cell><table:table-cell><text:p>i2</text:p></table:table-cell></table:table-row><table:table-row><table:table-cell><text:p>i3</text:p></table:table-cell><table:table-cell><text:p>i4</text:p></table:table-cell></table:table-row></table:table>"#;
    let body = format!(
        r#"<table:table><table:table-row><table:table-cell><text:p>out<text:tab/>er</text:p>{inner}<text:p>tail</text:p></table:table-cell><table:table-cell><text:p>side</text:p></table:table-cell></table:table-row></table:table>"#
    );
    let parsed = parse(&body);
    assert_eq!(parsed.text, "out er\ni1\ti2\ni3\ti4\ntail\tside");
    assert_eq!(
        runs_of(&parsed, RunKind::TableCell),
        vec!["out er\ni1\ti2\ni3\ti4\ntail", "i1", "i2", "i3", "i4", "side"]
    );
}

#[test]
fn odt_list_items_do_not_leak_into_table_cells_inside_them() {
    let body = r#"<text:list><text:list-item><text:p>Item</text:p><table:table><table:table-row><table:table-cell><text:p>cell</text:p></table:table-cell></table:table-row></table:table><text:p>Item again</text:p></text:list-item></text:list>"#;
    let parsed = parse(body);
    assert_eq!(runs_of(&parsed, RunKind::ListItem), vec!["Item", "Item again"]);
    assert_eq!(runs_of(&parsed, RunKind::TableCell), vec!["cell"]);
}

#[test]
fn odt_images_become_placeholders_with_one_warning() {
    let body = concat!(
        r#"<text:p>Look <draw:frame draw:name="f1" text:anchor-type="as-char"><draw:image xlink:href="Pictures/a.png"/></draw:frame> here</text:p>"#,
        r#"<text:p><draw:frame><draw:image xlink:href="Pictures/b.png"><text:p>alt text</text:p></draw:image></draw:frame></text:p>"#,
        r#"<draw:frame text:anchor-type="page"><draw:image xlink:href="Pictures/c.png"/></draw:frame>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Look [image] here\n\n[image]\n\n[image]");
    let w = warning(&parsed, "skipped_image").unwrap();
    assert!(w.message.starts_with("3 embedded images were replaced by the placeholder [image]"), "{}", w.message);
    assert_eq!(parsed.warnings.iter().filter(|w| w.kind == "skipped_image").count(), 1);
}

#[test]
fn odt_things_that_cannot_be_carried_over_are_counted_in_warnings() {
    let body = concat!(
        r#"<text:p>Box<draw:frame><draw:text-box><text:p>Hidden in a box</text:p></draw:text-box></draw:frame> text</text:p>"#,
        r#"<text:p>Equation <draw:frame><draw:object><math:math><math:mi>x</math:mi></math:math></draw:object><draw:image xlink:href="ObjectReplacements/Object 1"/></draw:frame>.</text:p>"#,
        r#"<text:p>Shape<draw:custom-shape><text:p>shape text</text:p></draw:custom-shape> here</text:p>"#,
        r#"<text:p>Claim<text:note text:id="ftn1" text:note-class="footnote"><text:note-citation>1</text:note-citation><text:note-body><text:p>The note</text:p></text:note-body></text:note> text</text:p>"#,
        r#"<text:p>Commented<office:annotation><dc:creator>A</dc:creator><text:p>comment body</text:p></office:annotation> text</text:p>"#,
        r#"<text:p><text:ruby><text:ruby-base>base</text:ruby-base><text:ruby-text>kana</text:ruby-text></text:ruby></text:p>"#,
        r#"<text:p>Shown <text:span text:style-name="Hidden">secret</text:span>end</text:p>"#,
        r#"<text:table-of-content><text:table-of-content-source text:outline-level="3"><text:index-title-template>Contents template</text:index-title-template></text:table-of-content-source><text:index-body><text:p>Index line</text:p></text:index-body></text:table-of-content>"#,
    );
    let parsed = parse(body);
    assert_eq!(
        parsed.text,
        "Box text\n\nEquation .\n\nShape here\n\nClaim text\n\nCommented text\n\nbase\n\nShown end\n\nIndex line"
    );
    for (kind, needle) in [
        ("skipped_text_box", "1 text box could not be imported"),
        ("skipped_equation", "1 equation could not be imported"),
        ("skipped_shape", "1 drawing object"),
        ("skipped_footnote", "1 footnote or endnote was left out"),
        ("skipped_comment", "1 comment was left out"),
        ("skipped_phonetic_guide", "1 phonetic guide"),
        ("skipped_hidden_text", "1 piece of hidden text was left out"),
    ] {
        let w = warning(&parsed, kind).unwrap_or_else(|| panic!("missing warning {kind}: {:?}", parsed.warnings));
        assert!(w.message.contains(needle), "{kind}: {}", w.message);
    }
    assert!(warning(&parsed, "skipped_image").is_none(), "an object's preview picture is not a separate image");
}

#[test]
fn odt_tracked_insertions_are_kept_and_deletions_are_left_out() {
    let body = concat!(
        r#"<text:tracked-changes>"#,
        r#"<text:changed-region text:id="ct1"><text:deletion><office:change-info><dc:creator>A</dc:creator></office:change-info><text:p>deleted words</text:p></text:deletion></text:changed-region>"#,
        r#"<text:changed-region text:id="ct2"><text:insertion><office:change-info><dc:creator>A</dc:creator></office:change-info></text:insertion></text:changed-region>"#,
        r#"</text:tracked-changes>"#,
        r#"<text:p>Kept <text:change-start text:change-id="ct2"/>inserted<text:change-end text:change-id="ct2"/> text<text:change text:change-id="ct1"/>.</text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Kept inserted text.");
    let w = warning(&parsed, "tracked_changes").expect("a warning about tracked changes");
    assert!(w.message.contains("1 insertion was kept"), "{}", w.message);
    assert!(w.message.contains("1 deletion was left out"), "{}", w.message);
}

#[test]
fn odt_bold_follows_the_script_of_the_text() {
    // LibreOffice writes the bold setting for the script of the selected text only.
    let body = concat!(
        r#"<text:p>日本語 <text:span text:style-name="AsianBold">太字</text:span> ok <text:span text:style-name="ComplexBold">مرحبا</text:span> 😀 "#,
        r#"<text:span text:style-name="AsianBold">not bold latin</text:span> <text:span text:style-name="ComplexBold">שלום</text:span></text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "日本語 太字 ok مرحبا 😀 not bold latin שלום");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["太字", "مرحبا", "שלום"]);
}

#[test]
fn odt_offsets_count_scalar_values_not_bytes() {
    let body = concat!(
        r#"<text:p>café 日本語 <text:span text:style-name="Bold">😀 emoji</text:span> e<text:span text:style-name="Ital">&#x301;nd</text:span></text:p>"#,
        r#"<text:p><text:span text:style-name="Bold">Σοφία</text:span></text:p>"#,
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "café 日本語 😀 emoji \u{e9}nd\n\nΣοφία");
    let bold: Vec<&FormatRun> = parsed.runs.iter().filter(|r| r.kind == RunKind::Bold).collect();
    assert_eq!(bold.len(), 2);
    assert_eq!((bold[0].start, bold[0].end), (9, 16), "'😀 emoji' is 7 scalar values and starts after 9");
    assert_eq!(covered(&parsed, bold[0]), "😀 emoji");
    assert_eq!(covered(&parsed, bold[1]), "Σοφία");
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["nd"]);
}

#[test]
fn odt_works_without_a_styles_part_and_reports_a_damaged_one() {
    let body = "<text:p text:style-name=\"Heading_20_2\">Named heading</text:p><text:p>Text</text:p>";
    let parsed = odt::parse(&odt(AUTO_STYLES, body), &opts()).unwrap();
    assert_eq!(runs_of(&parsed, RunKind::Heading), vec!["Named heading"]);

    let damaged = zip_bytes(&[
        ("mimetype", TEXT_MIMETYPE),
        ("content.xml", &content_xml(AUTO_STYLES, "<text:p>Still readable</text:p>")),
        ("styles.xml", "<office:document-styles><office:styles><style:style"),
    ]);
    let parsed = odt::parse(&damaged, &opts()).unwrap();
    assert_eq!(parsed.text, "Still readable");
    assert!(warning(&parsed, "unreadable_styles").is_some());
}

#[test]
fn odt_other_opendocument_kinds_are_refused_with_a_clear_message() {
    let sheet = zip_bytes(&[
        ("mimetype", "application/vnd.oasis.opendocument.spreadsheet"),
        ("content.xml", &content_xml("", "<text:p>x</text:p>")),
    ]);
    let message = format_error(odt::parse(&sheet, &opts()));
    assert!(message.contains("spreadsheet"), "{message}");
    assert!(message.contains("not an OpenDocument text file"), "{message}");
}

#[test]
fn odt_utf16_content_is_read() {
    let xml = content_xml("", "<text:p>Wide characters: 日本語</text:p>");
    let mut bytes = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    let package = zip_raw(&[("mimetype", TEXT_MIMETYPE.as_bytes()), ("content.xml", &bytes)]);
    let parsed = odt::parse(&package, &opts()).unwrap();
    assert_eq!(parsed.text, "Wide characters: 日本語");
}
