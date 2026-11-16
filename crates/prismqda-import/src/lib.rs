//! Importers for PrismQDA (specification section 5).
//!
//! Every importer ends in an [`ImportBatch`] that the engine stores in one transaction. Formats that
//! are documents give a [`ParsedText`]; formats that are tables (spreadsheets, survey exports, SPSS)
//! give a [`Table`] that the import wizard's mapping then turns into sources, cases, and attributes.

pub mod builder;
pub mod codings;
pub mod docx;
pub mod email;
pub mod error;
pub mod exchange;
pub mod feed;
pub mod image;
pub mod odt;
pub mod options;
pub mod pdf;
pub mod pstat;
pub mod recording;
pub mod reference;
pub mod rtf;
pub mod sav;
pub mod segment;
pub mod social;
pub mod spreadsheet;
pub mod table;
pub mod text;
pub mod transcript;

pub use builder::{ParsedText, TextBuilder};
pub use error::{ImportError, Result};
pub use codings::{import_codings, CoderSpec, CodingImportReport, CodingMapping};
pub use options::{ImportOptions, PictureVariables};
pub use table::{import_table, AttributeColumn, Cell, Column, SurveyMapping, Table, TablePreview};

use prismqda_engine::{ImportBatch, ImportedSource, SourceType};

/// Turns what a document importer found into a batch of one source, splitting the text into units
/// unless the format supplied its own.
pub fn parsed_to_batch(default_name: &str, parsed: ParsedText, opts: &ImportOptions) -> Result<ImportBatch> {
    if parsed.text.trim().is_empty() {
        return Err(ImportError::format("the file has no text to import"));
    }
    let kind = opts.unit_kind.unwrap_or(parsed.unit_kind);
    let units = match parsed.units {
        Some(units) => units,
        None => segment::units_for(kind, &parsed.text),
    };
    Ok(ImportBatch {
        sources: vec![ImportedSource {
            name: opts.name.clone().unwrap_or_else(|| default_name.to_string()),
            source_type: opts.source_type.unwrap_or(parsed.source_type),
            folder: opts.folder.clone(),
            text: parsed.text,
            runs: parsed.runs,
            units,
            default_unit_kind: kind,
            unit_attributes: parsed.unit_attributes,
            original_file_hash: opts.file_hash.clone(),
            settings: serde_json::Value::Null,
            warnings: parsed.warnings,
        }],
        ..Default::default()
    })
}

/// The kinds of file the importers recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Txt,
    Markdown,
    Html,
    Docx,
    Odt,
    Rtf,
    Pdf,
    Csv,
    Tsv,
    Xlsx,
    Sav,
    /// A project of PrismStatistics (PSTAT-1): its data, opened as a table.
    Pstat,
    Vtt,
    Srt,
    Json,
    /// A reference library exported as RIS.
    Ris,
    /// A reference library exported as BibTeX.
    Bib,
    /// One e-mail message.
    Eml,
    /// A mailbox of e-mail messages.
    Mbox,
    /// A web page saved whole by a browser (MHTML).
    Mhtml,
    /// A news feed saved from its address (RSS or Atom).
    Feed,
    /// A picture: PNG, JPEG, GIF, WebP, or BMP.
    Image,
}

impl Format {
    /// From the file name's extension, which is how people name what a file is.
    pub fn from_name(file_name: &str) -> Option<Format> {
        let ext = file_name.rsplit_once('.')?.1.to_lowercase();
        Some(match ext.as_str() {
            "txt" | "text" => Format::Txt,
            "md" | "markdown" => Format::Markdown,
            "html" | "htm" => Format::Html,
            "docx" => Format::Docx,
            "odt" => Format::Odt,
            "rtf" => Format::Rtf,
            "pdf" => Format::Pdf,
            "csv" => Format::Csv,
            "tsv" | "tab" => Format::Tsv,
            "xlsx" | "xlsm" => Format::Xlsx,
            "sav" | "zsav" => Format::Sav,
            "pstat" => Format::Pstat,
            "vtt" => Format::Vtt,
            "srt" => Format::Srt,
            "json" => Format::Json,
            "ris" => Format::Ris,
            "bib" | "bibtex" => Format::Bib,
            "eml" => Format::Eml,
            "mbox" | "mbx" => Format::Mbox,
            "mht" | "mhtml" => Format::Mhtml,
            "rss" | "atom" | "xml" => Format::Feed,
            "png" | "jpg" | "jpeg" | "jpe" | "jfif" | "gif" | "webp" | "bmp" => Format::Image,
            _ => return None,
        })
    }

    /// Checks the file's first bytes against its claimed format and says so plainly when they disagree.
    pub fn check_signature(self, bytes: &[u8]) -> Result<()> {
        let ok = match self {
            // A `.pstat` file says in its own words what is wrong when it is not an archive (see `pstat::read_table`).
            Format::Docx | Format::Odt | Format::Xlsx => bytes.starts_with(b"PK\x03\x04"),
            Format::Pdf => bytes.windows(5).take(1024).any(|w| w == b"%PDF-"),
            Format::Rtf => bytes.starts_with(b"{\\rtf"),
            Format::Sav => bytes.starts_with(b"$FL2") || bytes.starts_with(b"$FL3"),
            _ => true,
        };
        if ok {
            Ok(())
        } else {
            Err(ImportError::format(format!(
                "the file's name says {self:?}, but its contents are not; it may be damaged or saved in a different format"
            )))
        }
    }
}

/// The result of opening a file: either ready to store, or a table that needs the wizard's mapping first.
#[derive(Debug)]
pub enum Opened {
    Batch(ImportBatch),
    Table(Table),
}

/// Opens a file's bytes using the format its name implies.
pub fn open_bytes(file_name: &str, bytes: &[u8], opts: &ImportOptions) -> Result<Opened> {
    let format = match Format::from_name(file_name) {
        Some(f) => f,
        // A picture of a kind PrismQDA cannot show says what to do about it.
        None => match prismqda_engine::image_kind_of_name(file_name) {
            Err(why) => return Err(ImportError::unsupported(why)),
            Ok(_) => return Err(ImportError::unsupported(format!("'{file_name}' is not a kind of file PrismQDA can import yet"))),
        },
    };
    format.check_signature(bytes)?;
    let stem = file_name.rsplit_once('.').map(|(s, _)| s).unwrap_or(file_name);
    let stem = stem.rsplit(['/', '\\']).next().unwrap_or(stem);

    // Any document can be a transcript: the person says so, and the text is split into speaker turns.
    let doc = |parsed: ParsedText| {
        let parsed = if opts.source_type == Some(SourceType::Transcript) { transcript::apply_turns(parsed, opts) } else { parsed };
        parsed_to_batch(stem, parsed, opts).map(Opened::Batch)
    };
    match format {
        Format::Txt => doc(text::parse_txt(bytes, opts)?),
        Format::Markdown => doc(text::parse_markdown(bytes, opts)?),
        Format::Html => doc(text::parse_html(bytes, opts)?),
        Format::Docx => doc(docx::parse(bytes, opts)?),
        Format::Odt => doc(odt::parse(bytes, opts)?),
        Format::Rtf => doc(rtf::parse(bytes, opts)?),
        Format::Pdf => {
            let opened = doc(pdf::parse(bytes, opts)?)?;
            Ok(pdf::keep_file(opened, file_name, bytes))
        }
        Format::Vtt | Format::Srt => doc(transcript::parse_captions(bytes, format, opts)?),
        Format::Ris => Ok(Opened::Batch(reference::parse_ris(bytes, opts)?)),
        Format::Bib => Ok(Opened::Batch(reference::parse_bibtex(bytes, opts)?)),
        Format::Eml => Ok(Opened::Batch(email::parse_eml(bytes, opts)?)),
        Format::Mbox => Ok(Opened::Batch(email::parse_mbox(bytes, opts)?)),
        Format::Mhtml => Ok(Opened::Batch(email::parse_mhtml(bytes, opts)?)),
        Format::Feed => Ok(Opened::Batch(feed::parse_feed(bytes, opts)?)),
        Format::Image => Ok(Opened::Batch(image::parse_image(file_name, bytes, opts)?)),
        Format::Csv => Ok(Opened::Table(spreadsheet::read_csv(bytes, stem, None)?)),
        Format::Tsv => Ok(Opened::Table(spreadsheet::read_csv(bytes, stem, Some(b'\t'))?)),
        Format::Xlsx => Ok(Opened::Table(spreadsheet::read_xlsx(bytes, stem)?)),
        Format::Pstat => Ok(Opened::Table(pstat::read_table(bytes, stem)?)),
        Format::Sav => {
            let mut table = sav::read_table(bytes)?;
            if table.name.is_empty() {
                table.name = stem.to_string();
            }
            Ok(Opened::Table(table))
        }
        Format::Json => {
            let json_bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(bytes);
            let value: serde_json::Value = serde_json::from_slice(json_bytes)
                .map_err(|e| ImportError::format(format!("the file is not valid JSON: {e}")))?;
            // A Reddit export is a list of listings; a YouTube export has a list of comments (the
            // yt-dlp layout) or of items (the YouTube Data API layout).
            let batch = if exchange::looks_like_exchange(&value) {
                // The exchange file of Prism Reference Manager: sources and the excerpts taken from them.
                exchange::parse_exchange(&value, opts)?
            } else if reference::looks_like_csl_json(&value) {
                // A reference library exported as CSL JSON (what reference managers write).
                reference::parse_csl_json(&value, opts)?
            } else if value.is_array() {
                social::parse_reddit_json(bytes, opts)?
            } else if value.get("comments").is_some() || value.get("items").is_some() {
                social::parse_youtube_json(bytes, opts)?
            } else {
                return Err(ImportError::unsupported(
                    "this JSON file is not a layout PrismQDA knows (YouTube comments or a Reddit thread)",
                ));
            };
            Ok(Opened::Batch(batch))
        }
    }
}

/// What kind of source a format usually is.
pub fn default_source_type(format: Format) -> SourceType {
    match format {
        Format::Pdf => SourceType::Pdf,
        Format::Csv | Format::Tsv | Format::Xlsx | Format::Sav | Format::Pstat => SourceType::Survey,
        Format::Vtt | Format::Srt => SourceType::Transcript,
        Format::Json => SourceType::Social,
        Format::Image => SourceType::Other,
        _ => SourceType::Document,
    }
}
