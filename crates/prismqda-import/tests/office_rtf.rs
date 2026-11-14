//! RTF importer tests. The files are string literals written the way Word, WordPad and LibreOffice
//! write them; all text is invented.

mod common;

use common::*;
use prismqda_engine::{FormatRun, RunKind};
use prismqda_import::{rtf, ParsedText};

fn parse(body: &str) -> ParsedText {
    parse_whole(&rtf_doc(body))
}

/// A Unicode escape with its one-character fallback, built here so the number stays plain decimal.
fn u(code: i32) -> String {
    format!("\\u{code}?")
}

fn parse_whole(file: &str) -> ParsedText {
    let parsed = rtf::parse(file.as_bytes(), &opts()).unwrap();
    assert_sound(&parsed);
    parsed
}

#[test]
fn rtf_plain_paragraphs_are_separated_by_a_blank_line() {
    let parsed = parse(r"\pard\plain\f0\fs24 Hello world.\par\pard\plain Second one.\par");
    assert_eq!(parsed.text, "Hello world.\n\nSecond one.");
    assert!(parsed.runs.is_empty());
    assert!(parsed.warnings.is_empty());
    assert_eq!(parsed.source_type, prismqda_engine::SourceType::Document);
    assert_eq!(parsed.unit_kind, prismqda_engine::UnitKind::Sentence);
    assert!(parsed.units.is_none());
}

#[test]
fn rtf_bold_and_italic_cover_exactly_the_marked_words() {
    let parsed =
        parse(r"\pard plain {\b bold words} then {\i italic} and {\b\i both} end\par\pard {\b one\b0  two} three\par");
    assert_eq!(parsed.text, "plain bold words then italic and both end\n\none two three");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["bold words", "both", "one"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["italic", "both"]);
}

#[test]
fn rtf_formatting_ends_with_its_group_and_plain_resets_it() {
    let parsed = parse(r"\pard {\b bold {\i inner} still bold} after\par\pard \b sticky \plain reset\par");
    assert_eq!(parsed.text, "bold inner still bold after\n\nsticky reset");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["bold inner still bold", "sticky"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["inner"]);
}

#[test]
fn rtf_headings_come_from_outline_levels_and_stylesheet_names_and_parents() {
    let body = concat!(
        r"{\stylesheet{\s0\snext0 Normal;}{\s1\sbasedon0\snext0\outlinelevel0\b heading 1;}{\s2\sbasedon1\snext0 Chapter style;}",
        r"{\s3\sbasedon0\snext0 Title;}{\*\cs10 Default Paragraph Font;}}",
        r"\pard\s1 Method\par",
        r"\pard\s2 Chapter text\par",
        r"\pard\s3 The Study\par",
        r"\pard\outlinelevel2 Direct\par",
        r"\pard\s1\outlinelevel9 Body after all\par",
        r"\pard\s1 Mixed {\b bold} heading\par",
        r"\pard\s1 {\b All bold heading}\par",
        r"\pard Just text.\par",
    );
    let parsed = parse(body);
    assert_eq!(
        parsed.text,
        "Method\n\nChapter text\n\nThe Study\n\nDirect\n\nBody after all\n\nMixed bold heading\n\nAll bold heading\n\nJust text."
    );
    let headings: Vec<(String, Option<u8>)> =
        parsed.runs.iter().filter(|r| r.kind == RunKind::Heading).map(|r| (covered(&parsed, r), r.level)).collect();
    assert_eq!(
        headings,
        vec![
            ("Method".to_string(), Some(1)),
            ("Chapter text".to_string(), Some(1)),
            ("The Study".to_string(), Some(1)),
            ("Direct".to_string(), Some(3)),
            ("Mixed bold heading".to_string(), Some(1)),
            ("All bold heading".to_string(), Some(1)),
        ]
    );
    // A heading that is bold from start to end is just a heading; part of one in bold is emphasis.
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["bold"]);
}

#[test]
fn rtf_list_items_are_marked_and_their_generated_labels_are_not_text() {
    let body = concat!(
        r"\pard{\listtext\pard\plain\f0 1.\tab}\ls1\ilvl0 First item\par",
        r"\pard{\pntext\f3\'b7\tab}{\*\pn\pnlvlblt\pnf3{\pntxtb\'b7}}\fi-360\li720 Old-style bullet\par",
        r"\pard Not a list\par",
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "First item\n\nOld-style bullet\n\nNot a list");
    assert_eq!(runs_of(&parsed, RunKind::ListItem), vec!["First item", "Old-style bullet"]);
}

#[test]
fn rtf_tables_use_tabs_between_cells_and_newlines_between_rows() {
    let body = concat!(
        r"\pard Before.\par",
        r"\trowd\trgaph108\cellx3000\cellx6000",
        r"\pard\intbl Name\cell",
        r"\pard\intbl {\b Role}\cell\row",
        r"\trowd\trgaph108\cellx3000\cellx6000",
        r"\pard\intbl Ana\cell",
        r"\pard\intbl Nurse\par and teacher\cell\row",
        r"\trowd\trgaph108\cellx3000\cellx6000",
        r"\pard\intbl\cell",
        r"\pard\intbl Only second\cell\row",
        r"\pard After.\par",
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Before.\n\nName\tRole\nAna\tNurse\nand teacher\n\tOnly second\n\nAfter.");
    assert_eq!(runs_of(&parsed, RunKind::TableCell), vec!["Name", "Role", "Ana", "Nurse\nand teacher", "Only second"]);
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["Role"]);
}

#[test]
fn rtf_nested_tables_follow_the_same_conventions() {
    let body = concat!(
        r"\trowd\cellx5000\cellx9000",
        r"\pard\intbl\itap1 outer\par",
        r"\pard\intbl\itap2 i1\nestcell",
        r"\pard\intbl\itap2 i2\nestcell{\*\nesttableprops\trowd\cellx2000\cellx4000\nestrow}",
        r"\pard\intbl\itap2 i3\nestcell",
        r"\pard\intbl\itap2 i4\nestcell{\*\nesttableprops\trowd\cellx2000\cellx4000\nestrow}",
        r"\pard\intbl\itap1 tail\cell",
        r"\pard\intbl\itap1 side\cell\row",
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "outer\ni1\ti2\ni3\ti4\ntail\tside");
    assert_eq!(
        runs_of(&parsed, RunKind::TableCell),
        vec!["outer\ni1\ti2\ni3\ti4\ntail", "i1", "i2", "i3", "i4", "side"]
    );
}

#[test]
fn rtf_a_tab_inside_a_table_cell_cannot_be_mistaken_for_a_cell_separator() {
    let parsed = parse(r"\trowd\cellx1000\cellx2000\pard\intbl a\tab b\cell\pard\intbl c\cell\row\pard x\tab y\par");
    assert_eq!(parsed.text, "a b\tc\n\nx\ty");
}

#[test]
fn rtf_unicode_escapes_skip_their_fallback_characters_and_join_surrogate_pairs() {
    // Two UTF-16 units make the emoji U+1F600; each one is followed by one fallback character.
    let body = format!(r"\pard caf\'e9 {}{}{} {{\b {}{}}} end\par", u(26085), u(26412), u(35486), u(-10179), u(-8704));
    let parsed = parse(&body);
    assert_eq!(parsed.text, "café 日本語 😀 end");
    let bold: Vec<&FormatRun> = parsed.runs.iter().filter(|r| r.kind == RunKind::Bold).collect();
    assert_eq!(bold.len(), 1);
    assert_eq!(
        (bold[0].start, bold[0].end),
        (9, 10),
        "the emoji is one scalar value, not two UTF-16 units or four bytes"
    );
    assert_eq!(covered(&parsed, bold[0]), "😀");
    assert!(parsed.text.len() > parsed.text.chars().count());

    // \uc2: the fallback is two bytes, written as two hex escapes.
    let parsed = parse(&format!(r"\pard \uc2\u{}\'93\'fa\uc1  then\par", 26085));
    assert_eq!(parsed.text, "日 then");

    // A lone high surrogate becomes the replacement character and does not damage what follows.
    let parsed = parse(&format!(r"\pard a{}b\par", u(-10179)));
    assert_eq!(parsed.text, "a\u{FFFD}b");

    // \uc0: no fallback follows, so nothing is skipped (the space after the number belongs to it).
    let parsed = parse(&format!(r"\pard \uc0\u{} 24z\par", 233));
    assert_eq!(parsed.text, "é24z");
}

#[test]
fn rtf_hex_escapes_use_the_document_or_font_code_page() {
    let japanese =
        r"{\rtf1\ansi\ansicpg932\deff0{\fonttbl{\f0\fnil\fcharset128 MS Mincho;}}\pard \'93\'fa\'96\'7b\'8c\'ea\par}";
    assert_eq!(parse_whole(japanese).text, "日本語");

    // The font table says this font is GB2312, whatever the document code page is.
    let mixed = r"{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fnil\fcharset0 Arial;}{\f1\fnil\fcharset134 SimSun;}}\pard Mixed \f1\'d6\'d0\'ce\'c4\f0  text\par}";
    assert_eq!(parse_whole(mixed).text, "Mixed 中文 text");

    let hebrew = r"{\rtf1\ansi\ansicpg1255\deff0{\fonttbl{\f0\fnil\fcharset177 Arial;}}\pard \'f9\'ec\'e5\'ed\par}";
    assert_eq!(parse_whole(hebrew).text, "שלום");

    let russian =
        r"{\rtf1\ansi\ansicpg1251\deff0{\fonttbl{\f0\fnil\fcharset204 Arial;}}\pard \'cf\'f0\'e8\'e2\'e5\'f2\par}";
    assert_eq!(parse_whole(russian).text, "Привет");

    // Raw high bytes (not written as escapes) are decoded the same way.
    let mut raw = b"{\\rtf1\\ansi\\ansicpg1252\\deff0 \\pard caf".to_vec();
    raw.push(0xE9);
    raw.extend_from_slice(b" ok\\par}");
    assert_eq!(rtf::parse(&raw, &opts()).unwrap().text, "café ok");

    // A code page this reader has no table for is reported, not hidden.
    let dos = r"{\rtf1\ansi\ansicpg437 \pard caf\'82\par}";
    let parsed = parse_whole(dos);
    let w = warning(&parsed, "unsupported_codepage").expect("a warning about the code page");
    assert!(w.message.contains("437"), "{}", w.message);
}

#[test]
fn rtf_bold_and_italic_for_complex_scripts_follow_the_associated_settings() {
    let body = format!(
        r"\pard {{\rtlch\fcs1 \ab\ai\af0 \ltrch\fcs0 \b0\i0 {}{}{}{}}} hello {{\b {}{}{}}}\par",
        u(1513),
        u(1500),
        u(1493),
        u(1501),
        u(1502),
        u(1493),
        u(1502)
    );
    let parsed = parse(&body);
    assert_eq!(parsed.text, "שלום hello מומ");
    // The first Hebrew word uses the associated settings (bold and italic); the second has only \b,
    // which this reader accepts for Hebrew text too.
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["שלום", "מומ"]);
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["שלום"]);
}

#[test]
fn rtf_special_characters_and_symbols() {
    let body = r"\pard a\~b \{curly\} back\\slash \emdash\endash\lquote x\rquote\ldblquote y\rdblquote\bullet\tab t\line l\_h\-z\par";
    let parsed = parse(body);
    assert_eq!(parsed.text, "a\u{a0}b {curly} back\\slash \u{2014}\u{2013}\u{2018}x\u{2019}\u{201C}y\u{201D}\u{2022}\tt\nl-hz");
}

#[test]
fn rtf_destinations_that_are_not_text_are_skipped() {
    let body = concat!(
        r"{\info{\title Secret title}{\author Someone}}{\*\generator Msftedit 5.41;}",
        r"{\header\pard Header text\par}{\footer\pard Footer text\par}",
        r"{\*\listtable{\list\listtemplateid1{\listlevel\levelnfc0}}}",
        r"\pard Body {\*\bkmkstart mark}text{\*\bkmkend mark} with ",
        r#"{\field{\*\fldinst HYPERLINK "http://example.invalid/"}{\fldrslt {\ul link text}}} and a note{\super\chftn}"#,
        r"{\footnote\pard\plain {\super\chftn} Note text\par}.\par",
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "Body text with link text and a note.");
    let w = warning(&parsed, "skipped_footnote").unwrap();
    assert!(w.message.contains("1 footnote or endnote was left out"), "{}", w.message);
}

#[test]
fn rtf_pictures_become_placeholders_with_one_warning() {
    let data = rtf_doc(r"\pard Before {\pict\wmetafile8\picw100\pich100 0100090000039e00} after\par");
    assert_eq!(rtf::parse(data.as_bytes(), &opts()).unwrap().text, "Before [image] after");

    // Binary picture data may hold braces and backslashes; \bin says how many bytes to skip.
    let binary = b"{\\rtf1\\ansi\\pard X {\\pict\\pngblip\\bin6 }{\\ab}} Y {\\pict\\wmetafile8 0100} Z\\par}";
    let parsed = rtf::parse(binary, &opts()).unwrap();
    assert_eq!(parsed.text, "X [image] Y [image] Z");
    let w = warning(&parsed, "skipped_image").unwrap();
    assert!(w.message.starts_with("2 embedded images were replaced by the placeholder [image]"), "{}", w.message);
    assert_eq!(parsed.warnings.iter().filter(|w| w.kind == "skipped_image").count(), 1);

    // Word writes a picture twice: once for new readers (shppict) and once for old ones (nonshppict).
    let twice = rtf_doc(r"\pard A {\*\shppict{\pict\pngblip 89504e47}}{\nonshppict{\pict\wmetafile8 0100}} B\par");
    let parsed = rtf::parse(twice.as_bytes(), &opts()).unwrap();
    assert_eq!(parsed.text, "A [image] B");
    assert!(warning(&parsed, "skipped_image").unwrap().message.starts_with("1 embedded image was replaced"));
}

#[test]
fn rtf_shapes_text_boxes_comments_equations_and_hidden_text_are_counted() {
    let body = concat!(
        r"\pard C {\shp{\*\shpinst\shpleft0{\sp{\sn fFilled}{\sv 1}}{\shptxt\pard Text in box\par}}}D\par",
        r"\pard E {\shp{\*\shpinst{\sp{\sn pib}{\sv {\pict\pngblip 89504e47}}}}}F\par",
        r"\pard G {\shp{\*\shpinst{\sp{\sn shapeType}{\sv 1}}}}H\par",
        r"\pard x{\*\atrfstart 1}y{\*\atrfend 1}{\*\atnid AB}{\*\atnauthor A}\chatn{\*\annotation{\*\atnref 1}{\*\atndate 0}\pard\plain Comment text\par}\par",
        r"\pard Equation {\mmath{\*\mmathPr\mmathFont34}{\mr x=1}} here\par",
        r"\pard Shown{\v  secret} end\par",
    );
    let parsed = parse(body);
    assert_eq!(parsed.text, "C D\n\nE [image]F\n\nG H\n\nxy\n\nEquation  here\n\nShown end");
    for (kind, needle) in [
        ("skipped_text_box", "1 text box could not be imported"),
        ("skipped_image", "1 embedded image was replaced"),
        ("skipped_shape", "1 drawing object"),
        ("skipped_comment", "1 comment was left out"),
        ("skipped_equation", "1 equation could not be imported"),
        ("skipped_hidden_text", "1 piece of hidden text was left out"),
    ] {
        let w = warning(&parsed, kind).unwrap_or_else(|| panic!("missing warning {kind}: {:?}", parsed.warnings));
        assert!(w.message.contains(needle), "{kind}: {}", w.message);
    }
}

#[test]
fn rtf_libreoffice_style_file() {
    let file = concat!(
        r"{\rtf1\ansi\deff3\adeflang1025",
        r"{\fonttbl{\f0\froman\fprq2\fcharset0 Times New Roman;}{\f3\froman\fprq2\fcharset0 Liberation Serif;}{\f5\fnil\fprq0\fcharset0 Noto Sans CJK SC;}}",
        r"{\colortbl;\red0\green0\blue0;\red255\green255\blue255;}",
        r"{\stylesheet{\s0\snext0\rtlch\af7\afs24\alang1081 \ltrch\lang1033\langfe2052\hich\af3\loch\ql\hyphpar0\ltrpar\cf0\f3\fs24\lang1033 Normal;}",
        r"{\s1\sbasedon0\snext1\rtlch\af8\afs36\ab\ai0\ltrch\lang1033\hich\af3\loch\sb240\sa120\keepn\ql\outlinelevel0\b\fs36 Heading 1;}}",
        r"{\*\listtable}{\*\listoverridetable}",
        r"{\info{\author Test}{\creatim\yr2026\mo1\dy1\hr0\min0}}\deftab709",
        "\r\n",
        r"\pard\plain \s1\ql\outlinelevel0 {\loch\f3\fs36\b\hich\af3\dbch\af8 Interview notes}\par",
        "\r\n",
        r"\pard\plain \s0\ql {\loch\f3\fs24 First paragraph with }{\loch\f3\fs24\i emphasis}{\loch\f3\fs24 .}\par",
        "\r\n}",
    );
    let parsed = parse_whole(file);
    assert_eq!(parsed.text, "Interview notes\n\nFirst paragraph with emphasis.");
    assert_eq!(runs_of(&parsed, RunKind::Heading), vec!["Interview notes"]);
    assert!(runs_of(&parsed, RunKind::Bold).is_empty(), "a heading that is bold from start to end is just a heading");
    assert_eq!(runs_of(&parsed, RunKind::Italic), vec!["emphasis"]);
}

#[test]
fn rtf_wordpad_style_file_with_line_ends_and_plain_formatting() {
    // WordPad writes Windows line ends between groups and may wrap a long line after a space. Line
    // ends are not text, and anything after the closing brace (here a NUL) is not part of the file.
    let file = concat!(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0\\nouicompat\\deflang1033{\\fonttbl{\\f0\\fnil\\fcharset0 Calibri;}}\r\n",
        "{\\*\\generator Riched20 10.0.19041}\\viewkind4\\uc1 \r\n",
        "\\pard\\sa200\\sl276\\slmult1\\f0\\fs22\\lang9 This is a long line that \r\n",
        "the editor wrapped in the file.\\par\r\n",
        "\\b Bold start\\b0  and the rest.\\par\r\n}\r\n\0",
    );
    let parsed = parse_whole(file);
    assert_eq!(parsed.text, "This is a long line that the editor wrapped in the file.\n\nBold start and the rest.");
    assert_eq!(runs_of(&parsed, RunKind::Bold), vec!["Bold start"]);
}

#[test]
fn rtf_an_empty_document_has_no_text() {
    let parsed = parse_whole(r"{\rtf1\ansi}");
    assert_eq!(parsed.text, "");
    assert!(parsed.runs.is_empty());
}
