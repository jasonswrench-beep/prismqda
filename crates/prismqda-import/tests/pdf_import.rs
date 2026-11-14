//! PDF import against small synthetic PDFs. The fixtures in tests/fixtures/pdf were made by Ghostscript from the
//! PostScript in tests/fixtures/pdf/source; the CJK and emoji test builds its PDF by hand below.

use prismqda_engine::{NewProject, OpenOptions, Project, RunKind, SourceType, UnitKind};
use prismqda_import::{open_bytes, pdf, ImportError, ImportOptions, Opened};
use unicode_normalization::is_nfc;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/pdf/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

fn parse(name: &str) -> prismqda_import::ParsedText {
    pdf::parse(&fixture(name), &ImportOptions::default()).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The text between two offsets that count Unicode scalar values.
fn slice(text: &str, start: i64, end: i64) -> String {
    text.chars().skip(start as usize).take((end - start) as usize).collect()
}

fn page_runs(p: &prismqda_import::ParsedText) -> Vec<(u32, String)> {
    p.runs
        .iter()
        .map(|r| {
            assert_eq!(r.kind, RunKind::Page);
            (r.page.expect("a page run has its number"), slice(&p.text, r.start, r.end))
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// The fixtures
// ---------------------------------------------------------------------------------------------

const PAGE_ONE: &str = "Field Notes on a Neighbourhood Garden\n\n\
The garden opened in the spring of last year, after the council agreed to lease the empty lot beside the library. \
Volunteers cleared the rubble over two weekends and built raised beds from reclaimed timber. By June the first \
tomatoes were ripening, and neighbours who had never spoken before were swapping seedlings over the fence. The \
group also wrote a short guide with practical information for newcomers, and aims to become self-sufficient in \
compost by next year.\n\n\
Zoë brought café con leche for the señor who waters the beds, and the naïve plan to share one hose soon grew into \
a rota. Everyone agreed that the biggest surprise was how much (and how loudly) people talked while they weeded.\n\n\
\u{201C}It feels like a village inside the city,\u{201D} said one volunteer.";

const PAGE_THREE: &str = "Second Season\n\n\
In the second season the focus moved from building to sharing. A seed library now sits in a cabinet by the gate, \
and children from the primary school visit each Thursday to measure the beans.\n\n\
1. Seeds are shared on Saturdays.\n\n\
2. Compost is turned each week.\n\n\
The council has renewed the lease for five years.";

#[test]
fn a_three_page_document_keeps_paragraphs_accents_and_page_numbers() {
    let p = parse("three_pages.pdf");

    assert_eq!(p.text, format!("{PAGE_ONE}\n\n{PAGE_THREE}"));
    assert!(is_nfc(&p.text), "the text must be NFC for the offsets to hold");
    assert_eq!(p.source_type, SourceType::Pdf);
    assert_eq!(p.unit_kind, UnitKind::Sentence);
    assert!(p.units.is_none(), "units come from sentence splitting");
    assert!(p.unit_attributes.is_empty());

    // One run per page that has text, each covering exactly that page's text.
    assert_eq!(page_runs(&p), [(1, PAGE_ONE.to_string()), (3, PAGE_THREE.to_string())]);
}

#[test]
fn a_blank_page_is_reported_by_number_and_never_dropped_silently() {
    let p = parse("three_pages.pdf");
    assert_eq!(p.warnings.len(), 1, "{:?}", p.warnings);
    let w = &p.warnings[0];
    assert_eq!(w.kind, "unreadable_page");
    assert_eq!(w.location.as_deref(), Some("page 2"));
    assert!(w.message.contains("OCR"), "the message says what is needed: {}", w.message);
}

#[test]
fn hard_wrapped_lines_become_paragraphs_and_split_words_are_put_back() {
    let p = parse("three_pages.pdf");
    // "informa-" at a line end followed by "tion" becomes one word.
    assert!(p.text.contains("practical information for newcomers"), "{}", p.text);
    // A compound that is hyphenated on purpose keeps its hyphen.
    assert!(p.text.contains("self-sufficient"));
    // No line breaks inside a paragraph: every newline is part of a blank line.
    assert!(!p.text.replace("\n\n", "").contains('\n'));
    // The numbered lines are separate paragraphs.
    assert!(p.text.contains("Saturdays.\n\n2. Compost"));
}

#[test]
fn two_columns_are_read_one_column_after_the_other() {
    let p = parse("layout.pdf");
    let runs = page_runs(&p);
    let (page, text) = &runs[0];
    assert_eq!(*page, 1);
    assert_eq!(
        text,
        "Left column, first paragraph. The committee met on a Tuesday evening in the church hall and counted the chairs \
         twice before anyone arrived.\n\n\
         Left column, second paragraph. Nobody had brought a pen, so the minutes were written on the back of a tea \
         towel.\n\n\
         Right column, first paragraph. The harvest festival was held in the barn, and the youngest children were put \
         in charge of the apples.\n\n\
         Right column, second paragraph. By nine o\u{2019}clock the last pie was gone."
    );
}

#[test]
fn ligatures_superscripts_tables_and_fake_bold_come_out_cleanly() {
    let p = parse("layout.pdf");
    let (page, text) = &page_runs(&p)[1];
    assert_eq!(*page, 2);
    assert_eq!(
        text,
        // The fi and fl ligature glyphs are plain letters, so a search for "final" finds it.
        "The final floor plan was ready before the second meeting. The result was significant2 in later tests, as the \
         authors expected.\n\n\
         Overprinted heading\n\n\
         Name\tGroup\tScore\nAna\tcontrol\t41\nBen\ttreatment\t57\nCleo\ttreatment\t63\n\n\
         Table 1 shows the scores for each group."
    );
    assert!(p.warnings.is_empty());
}

#[test]
fn justified_text_with_different_word_spacing_on_every_line_reads_as_plain_sentences() {
    let p = parse("justified.pdf");
    assert_eq!(
        p.text,
        "The orchard behind the school was planted by the first head teacher, who believed that children learn more \
         from a tree than from a textbook. Every autumn the older pupils pick the apples, sort them by size, and \
         carry the best crates to the market stall that opens on Saturday mornings in the village square.\n\n\
         A second paragraph starts after a clear gap and is justified the same way, so it is short."
    );
    assert!(p.warnings.is_empty());
}

#[test]
fn a_sentence_and_a_split_word_that_run_over_a_page_break_stay_whole() {
    let p = parse("continued.pdf");
    let first = "The first page ends with a complete sentence about the weather.";
    let second = "The second page begins a new one, which stops in the middle of an important word and goes on across \
                  the page break until the committee agreed that the following steps would be taken.";
    // A page that ended its sentence is not joined to the next; the one that did not is.
    assert_eq!(p.text, format!("{first}\n\n{second}"));
    let runs = page_runs(&p);
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0], (1, first.to_string()));
    assert_eq!(runs[1].0, 2);
    assert_eq!(runs[2].0, 3);
    assert!(runs[1].1.ends_with("an impor"), "{}", runs[1].1);
    assert_eq!(runs[2].1, "tant word and goes on across the page break until the committee agreed that the following steps would be taken.");
    // The runs touch: the pieces of one word were joined without a gap.
    assert_eq!(p.runs[1].end, p.runs[2].start);
}

#[test]
fn text_after_a_picture_is_still_read_and_the_picture_only_page_is_reported() {
    let p = parse("text_and_scan.pdf");
    assert_eq!(p.text, "Text before the figure on page one.\n\nText after the figure on page one.");
    assert_eq!(p.warnings.len(), 1);
    assert_eq!(p.warnings[0].kind, "unreadable_page");
    assert_eq!(p.warnings[0].location.as_deref(), Some("page 2"));
}

#[test]
fn a_pdf_of_pictures_only_is_refused_with_a_plain_message_about_ocr() {
    let err = pdf::parse(&fixture("scanned_only.pdf"), &ImportOptions::default()).unwrap_err();
    assert!(matches!(err, ImportError::Unsupported(_)), "{err:?}");
    let message = err.to_string();
    assert!(message.contains("no text layer") && message.contains("OCR"), "{message}");
}

#[test]
fn a_password_protected_pdf_gives_a_clear_error() {
    let err = pdf::parse(&fixture("password.pdf"), &ImportOptions::default()).unwrap_err();
    assert!(matches!(err, ImportError::Format(_)), "{err:?}");
    assert!(err.to_string().contains("password"), "{err}");
}

#[test]
fn a_pdf_with_only_an_owner_password_opens_like_it_does_in_any_reader() {
    let p = parse("owner_only.pdf");
    assert!(p.text.starts_with("The first page ends with a complete sentence"));
    assert_eq!(p.runs.len(), 3);
}

// ---------------------------------------------------------------------------------------------
// Damaged files
// ---------------------------------------------------------------------------------------------

#[test]
fn a_truncated_pdf_is_an_error_not_a_panic() {
    let err = pdf::parse(&fixture("truncated.pdf"), &ImportOptions::default()).unwrap_err();
    assert!(matches!(err, ImportError::Format(_)), "{err:?}");
    assert!(err.to_string().contains("damaged"), "{err}");
}

#[test]
fn damaged_input_of_every_kind_returns_without_panicking() {
    let good = fixture("three_pages.pdf");
    // Cut at many points.
    for cut in (0..good.len()).step_by(97) {
        let _ = pdf::parse(&good[..cut], &ImportOptions::default());
    }
    // Flip a few bytes at pseudo-random places, several hundred times.
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..300 {
        let mut bad = good.clone();
        for _ in 0..(1 + next() % 4) {
            let at = (next() % bad.len() as u64) as usize;
            bad[at] = (next() & 0xFF) as u8;
        }
        let _ = pdf::parse(&bad, &ImportOptions::default());
    }
    // Things that are not PDFs at all.
    for junk in [&b""[..], b"%PDF-", b"%PDF-1.4\n", b"%PDF-1.4\n1 0 obj\n<<", &[0u8; 64], &[0xFFu8; 64]] {
        assert!(pdf::parse(junk, &ImportOptions::default()).is_err());
    }
}

// ---------------------------------------------------------------------------------------------
// A PDF built by hand: CJK, an accent, and an emoji above U+FFFF, through a ToUnicode map
// ---------------------------------------------------------------------------------------------

/// Joins numbered objects into a complete PDF file with a correct cross-reference table.
fn assemble(objects: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for o in &offsets {
        out.extend(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes(),
    );
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut v = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    v.extend(data);
    v.extend(b"\nendstream");
    v
}

/// One page of text shown with a two-byte-per-glyph font that has no font program, only a ToUnicode map.
/// `lines` are shown top to bottom; each character gets its own code.
fn cid_pdf(lines: &[&str]) -> Vec<u8> {
    let mut chars: Vec<char> = Vec::new();
    for c in lines.iter().flat_map(|l| l.chars()) {
        if !chars.contains(&c) {
            chars.push(c);
        }
    }
    let code_of = |c: char| chars.iter().position(|x| *x == c).expect("known char") as u16 + 1;

    let mut content = String::from("BT /F1 12 Tf 14.4 TL 72 700 Td\n");
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            content.push_str("T*\n");
        }
        content.push('<');
        for c in line.chars() {
            content.push_str(&format!("{:04X}", code_of(c)));
        }
        content.push_str("> Tj\n");
    }
    content.push_str("ET");

    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n\
         1 begincodespacerange <0000> <FFFF> endcodespacerange\n",
    );
    cmap.push_str(&format!("{} beginbfchar\n", chars.len()));
    for c in &chars {
        let mut units = [0u16; 2];
        let hex: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
        cmap.push_str(&format!("<{:04X}> <{hex}>\n", code_of(*c)));
    }
    cmap.push_str("endbfchar\nendcmap CMapName currentdict /CMap defineresource pop end end\n");

    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_vec(),
        stream("", content.as_bytes()),
        b"<< /Type /Font /Subtype /Type0 /BaseFont /TestFont /Encoding /Identity-H /DescendantFonts [6 0 R] /ToUnicode 7 0 R >>".to_vec(),
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /TestFont /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
          /DW 1000 /FontDescriptor 8 0 R >>"
            .to_vec(),
        stream("", cmap.as_bytes()),
        b"<< /Type /FontDescriptor /FontName /TestFont /Flags 4 /FontBBox [0 0 1000 1000] /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 >>"
            .to_vec(),
    ])
}

#[test]
fn cjk_accents_and_an_emoji_above_the_basic_plane_keep_exact_offsets() {
    let first = "Caf\u{00E9} \u{4F60}\u{597D}\u{4E16}\u{754C}";
    let second = "Fin \u{1F600} ok.";
    let bytes = cid_pdf(&[first, second]);
    let p = pdf::parse(&bytes, &ImportOptions::default()).unwrap();

    // Both lines are one paragraph, joined with a space (a hard-wrapped paragraph).
    assert_eq!(p.text, format!("{first} {second}"));
    assert!(is_nfc(&p.text));

    // The offsets count scalar values: the emoji is one position, not two.
    assert_eq!(p.text.chars().count(), 9 + 1 + 9);
    assert_eq!(p.runs.len(), 1);
    assert_eq!((p.runs[0].start, p.runs[0].end, p.runs[0].page), (0, 19, Some(1)));
    assert_eq!(slice(&p.text, 14, 15), "\u{1F600}");

    // Through the whole importer: sentence units with exact offsets.
    let batch = match open_bytes("hand.pdf", &bytes, &ImportOptions::default()).unwrap() {
        Opened::Batch(b) => b,
        other => panic!("expected a batch, got {other:?}"),
    };
    let source = &batch.sources[0];
    assert_eq!(source.name, "hand");
    assert_eq!(source.source_type, SourceType::Pdf);
    assert_eq!(source.default_unit_kind, UnitKind::Sentence);
    assert_eq!(source.runs.len(), 1);
    assert!(source.units.iter().all(|u| u.kind == UnitKind::Sentence));
    let unit_text: Vec<String> = source.units.iter().map(|u| slice(&source.text, u.start, u.end)).collect();
    assert_eq!(unit_text.concat().replace(' ', ""), p.text.replace(' ', ""));
}

/// A one-page PDF whose content stream is given, with a standard font that is not embedded.
fn simple_pdf(content: &str) -> Vec<u8> {
    assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_vec(),
        stream("", content.as_bytes()),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    ])
}

#[test]
fn kerning_inside_a_text_array_does_not_split_words_but_wide_gaps_are_spaces() {
    // LaTeX-style output: words are separate strings with a gap between them and small kerning inside.
    let p = pdf::parse(
        &simple_pdf("BT /F1 12 Tf 72 700 Td [(Wor) 40 (ld) -250 (of) -250 (T) 80 (eX) -250 (and) -250 (the) -250 (rest.)] TJ ET"),
        &ImportOptions::default(),
    )
    .unwrap();
    assert_eq!(p.text, "World of TeX and the rest.");
}

#[test]
fn lines_shown_with_the_quote_operators_are_not_lost() {
    // The `'` and `"` operators both mean "next line, then show this"; many generators use them for every line.
    let p = pdf::parse(
        &simple_pdf("BT /F1 12 Tf 14.4 TL 72 700 Td (First line of the paragraph that is) Tj (wrapped over three lines but) ' 0 0 (stays one paragraph.) \" ET"),
        &ImportOptions::default(),
    )
    .unwrap();
    assert_eq!(p.text, "First line of the paragraph that is wrapped over three lines but stays one paragraph.");
}

#[test]
fn a_page_the_pdf_library_cannot_read_is_reported_and_the_other_pages_are_still_imported() {
    // Page 2 selects a font its resources do not have, which the library cannot cope with.
    let bytes = assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 5 0 R 7 0 R] /Count 3 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 8 0 R >> >> >>".to_vec(),
        stream("", b"BT /F1 12 Tf 72 700 Td (Page one is fine.) Tj ET"),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 6 0 R /Resources << /Font << /F1 8 0 R >> >> >>".to_vec(),
        stream("", b"BT /F9 12 Tf 72 700 Td (Page two names a font that is not there.) Tj ET"),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 9 0 R /Resources << /Font << /F1 8 0 R >> >> >>".to_vec(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        stream("", b"BT /F1 12 Tf 72 700 Td (Page three is fine too.) Tj ET"),
    ]);
    let p = pdf::parse(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(p.text, "Page one is fine.\n\nPage three is fine too.");
    assert_eq!(page_runs(&p).iter().map(|(n, _)| *n).collect::<Vec<_>>(), [1, 3]);
    assert_eq!(p.warnings.len(), 1);
    assert_eq!(p.warnings[0].kind, "unreadable_page");
    assert_eq!(p.warnings[0].location.as_deref(), Some("page 2"));
    assert!(p.warnings[0].message.contains("could not be read"), "{}", p.warnings[0].message);
}

#[test]
fn text_in_a_form_xobject_is_read() {
    let bytes = assemble(&[
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /XObject << /Fm1 6 0 R >> >> >>".to_vec(),
        stream("", b"q /Fm1 Do Q"),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >>",
            b"BT /F1 12 Tf 72 700 Td 14.4 TL (Inside a form.) Tj (Next one.) ' ET",
        ),
    ]);
    let p = pdf::parse(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(p.text, "Inside a form. Next one.");
}

#[test]
fn many_pages_read_in_reasonable_time_and_every_page_is_cited() {
    // 400 pages of ordinary text in a standard font that is not embedded.
    let mut objects: Vec<Vec<u8>> = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        Vec::new(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
    ];
    let pages = 400usize;
    let mut kids = String::new();
    for n in 0..pages {
        let page_id = 4 + 2 * n;
        kids.push_str(&format!("{page_id} 0 R "));
        objects.push(
            format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", page_id + 1)
                .into_bytes(),
        );
        let mut content = String::from("BT /F1 12 Tf 14.4 TL 72 720 Td\n");
        for line in 0..30 {
            content.push_str(&format!(
                "(Page {} line {} of the long document, with some more words to fill it.) Tj T*\n",
                n + 1,
                line + 1
            ));
        }
        content.push_str("ET");
        objects.push(stream("", content.as_bytes()));
    }
    objects[1] = format!("<< /Type /Pages /Kids [{kids}] /Count {pages} >>").into_bytes();
    let bytes = assemble(&objects);

    let started = std::time::Instant::now();
    let p = pdf::parse(&bytes, &ImportOptions::default()).unwrap();
    assert!(started.elapsed().as_secs() < 60, "took {:?}", started.elapsed());

    assert_eq!(p.runs.len(), pages);
    assert!(p.warnings.is_empty());
    for (i, run) in p.runs.iter().enumerate() {
        assert_eq!(run.page, Some(i as u32 + 1));
        let text = slice(&p.text, run.start, run.end);
        assert!(text.starts_with(&format!("Page {} line 1 of", i + 1)), "{}", &text[..40]);
        assert!(text.ends_with("line 30 of the long document, with some more words to fill it."));
    }
    // Page runs never overlap and stay in order.
    assert!(p.runs.windows(2).all(|w| w[0].end <= w[1].start));
}

// ---------------------------------------------------------------------------------------------
// Through the engine
// ---------------------------------------------------------------------------------------------

#[test]
fn the_batch_is_stored_with_page_runs_and_the_import_report() {
    let bytes = fixture("three_pages.pdf");
    let opts = ImportOptions {
        name: Some("Garden notes".to_string()),
        file_hash: Some("ab".repeat(32)),
        ..Default::default()
    };
    let batch = match open_bytes("three_pages.pdf", &bytes, &opts).unwrap() {
        Opened::Batch(b) => b,
        other => panic!("expected a batch, got {other:?}"),
    };
    assert_eq!(batch.sources.len(), 1);
    assert_eq!(batch.sources[0].warnings.len(), 1);

    let dir = tempfile::TempDir::new().unwrap();
    let mut o = OpenOptions::new("tester");
    o.backup_on_open = false;
    let mut project = Project::create(dir.path().join("p.pqda"), &NewProject::new("P"), &o).unwrap();
    let outcome = project.apply_import(&batch).unwrap();
    assert_eq!(outcome.source_ids.len(), 1);
    assert!(outcome.units_created > 10);

    let conn = rusqlite::Connection::open(project.path()).unwrap();
    let (runs, report, kind, source_type): (String, String, String, String) = conn
        .query_row(
            "SELECT formatting_runs_json, import_report_json, default_unit_kind, source_type FROM source WHERE id = ?1",
            [outcome.source_ids[0]],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!((kind.as_str(), source_type.as_str()), ("sentence", "pdf"));
    let runs: serde_json::Value = serde_json::from_str(&runs).unwrap();
    let pages: Vec<i64> = runs.as_array().unwrap().iter().map(|r| r["page"].as_i64().unwrap()).collect();
    assert_eq!(pages, [1, 3]);
    assert!(report.contains("unreadable_page") && report.contains("page 2"));
}

// ---------------------------------------------------------------------------------------------
// Keeping the file, so its pages can be drawn and areas of them coded
// ---------------------------------------------------------------------------------------------

fn opened_batch(name: &str, opts: &ImportOptions) -> prismqda_engine::ImportBatch {
    match open_bytes(name, &fixture(name), opts).unwrap_or_else(|e| panic!("{name}: {e}")) {
        Opened::Batch(b) => b,
        Opened::Table(_) => panic!("{name} is not a table"),
    }
}

#[test]
fn a_pdf_comes_in_with_its_file_kept_and_the_size_of_every_page() {
    let batch = opened_batch("three_pages.pdf", &ImportOptions::default());
    assert_eq!(batch.pdfs.len(), 1);
    let kept = &batch.pdfs[0];
    assert_eq!((kept.source, kept.pdf.file_name.as_str(), kept.pdf.pages.len()), (0, "three_pages.pdf", 3));
    assert_eq!(kept.pdf.bytes, fixture("three_pages.pdf"), "byte for byte");
    assert!(kept.pdf.pages.iter().all(|(w, h)| *w > 100.0 && *h > 100.0));
    assert!(!batch.sources[0].warnings.iter().any(|w| w.kind.starts_with("pdf_")), "{:?}", batch.sources[0].warnings);
}

#[test]
fn a_pdf_read_as_a_transcript_is_words_only() {
    let opts = ImportOptions { source_type: Some(SourceType::Transcript), ..ImportOptions::default() };
    let batch = opened_batch("three_pages.pdf", &opts);
    assert!(batch.pdfs.is_empty());
}

#[test]
fn a_pdf_locked_only_to_restrict_printing_is_kept_as_it_was_and_its_pages_are_drawn_like_any_reader_does() {
    let batch = opened_batch("owner_only.pdf", &ImportOptions::default());
    assert_eq!(batch.pdfs.len(), 1, "{:?}", batch.sources[0].warnings);
    assert_eq!(batch.pdfs[0].pdf.bytes, fixture("owner_only.pdf"), "byte for byte, lock and all");
    assert!(prismqda_pdfview::render_page_png(&batch.pdfs[0].pdf.bytes, 1, 400).is_ok());
}
