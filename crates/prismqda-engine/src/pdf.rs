//! PDF pages (specification 8.2, PDF regions). A PDF comes into the project as a document whose text is the words read from the file,
//! as before. What this adds is the file itself, kept inside the project exactly as it was, and the size of each page, so a window can
//! draw a page when it is looked at and a person can draw a rectangle on it. Areas of a page are coded in `regions.rs`, in fractions of
//! the page as it is drawn.
//!
//! The engine does not draw pages. `prismqda-pdfview` does, from the bytes this module keeps; the engine only stores what it is
//! given and checks that it is consistent.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::{event, Error, Result};

/// The most one PDF may be. A PDF is kept whole inside the project, so there is a limit.
pub const MAX_PDF_BYTES: usize = 256 * 1024 * 1024;
/// The most pages of one PDF whose sizes are kept.
pub const MAX_PDF_PAGES: usize = 20_000;

/// A PDF ready to store, as an importer found it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfData {
    pub file_name: String,
    /// The size of each page, in points as the page is drawn: the first is page 1.
    pub pages: Vec<(f64, f64)>,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// Which source of an import batch a PDF belongs to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdfAttachment {
    pub source: usize,
    pub pdf: PdfData,
}

/// What is known of a kept PDF without its bytes.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PdfRow {
    pub source_id: i64,
    pub file_name: String,
    pub page_count: i64,
    pub size_bytes: i64,
    pub created_at: String,
    /// The size of each page in points, page 1 first.
    pub pages: Vec<PageSizeRow>,
}

/// The size of one page, as it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PageSizeRow {
    pub page: i64,
    pub width: f64,
    pub height: f64,
}

/// What a list of documents needs to know about a document that has its PDF kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourcePdf {
    pub file_name: String,
    pub page_count: i64,
    pub size_bytes: i64,
}

/// Checks a PDF and puts its rows in.
pub(crate) fn insert_pdf(tx: &Transaction, source_id: i64, pdf: &PdfData) -> Result<()> {
    if pdf.bytes.is_empty() {
        return Err(Error::Invalid(format!("the PDF {} has no bytes", pdf.file_name)));
    }
    if pdf.bytes.len() > MAX_PDF_BYTES {
        return Err(Error::Invalid(format!(
            "{} is {} MB, and PrismQDA keeps PDFs up to {} MB inside the project",
            pdf.file_name,
            pdf.bytes.len() / (1024 * 1024),
            MAX_PDF_BYTES / (1024 * 1024)
        )));
    }
    if pdf.pages.is_empty() || pdf.pages.len() > MAX_PDF_PAGES {
        return Err(Error::Invalid(format!("the PDF {} has {} pages; PrismQDA keeps from 1 to {MAX_PDF_PAGES}", pdf.file_name, pdf.pages.len())));
    }
    if pdf.pages.iter().any(|(w, h)| !(w.is_finite() && h.is_finite() && *w > 0.0 && *h > 0.0)) {
        return Err(Error::Invalid(format!("the PDF {} has a page with no usable size", pdf.file_name)));
    }
    tx.execute(
        "INSERT INTO pdf_file (source_id, file_name, page_count, size_bytes, data, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![source_id, pdf.file_name, pdf.pages.len() as i64, pdf.bytes.len() as i64, pdf.bytes],
    )?;
    let mut stmt = tx.prepare("INSERT INTO pdf_page (source_id, page, width, height) VALUES (?1, ?2, ?3, ?4)")?;
    for (i, (w, h)) in pdf.pages.iter().enumerate() {
        stmt.execute(params![source_id, i as i64 + 1, w, h])?;
    }
    Ok(())
}

impl Project {
    /// What is known of the PDF a document keeps, with the size of every page; `None` for a document with no PDF kept.
    pub fn get_pdf(&self, source_id: i64) -> Result<Option<PdfRow>> {
        let head = self
            .conn
            .query_row("SELECT source_id, file_name, page_count, size_bytes, created_at FROM pdf_file WHERE source_id = ?1", [source_id], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?))
            })
            .optional()?;
        let Some((source_id, file_name, page_count, size_bytes, created_at)) = head else { return Ok(None) };
        let mut stmt = self.conn.prepare("SELECT page, width, height FROM pdf_page WHERE source_id = ?1 ORDER BY page")?;
        let pages = stmt
            .query_map([source_id], |r| Ok(PageSizeRow { page: r.get(0)?, width: r.get(1)?, height: r.get(2)? }))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Some(PdfRow { source_id, file_name, page_count, size_bytes, created_at, pages }))
    }

    /// The file of the PDF a document keeps, as it was imported.
    pub fn read_pdf_bytes(&self, source_id: i64) -> Result<Vec<u8>> {
        self.conn
            .query_row("SELECT data FROM pdf_file WHERE source_id = ?1", [source_id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| Error::Invalid("this document has no PDF kept with it".to_string()))
    }

    /// Writes the PDF a document keeps to a path (for opening it in another program or keeping a copy).
    pub fn save_pdf_copy(&self, source_id: i64, path: &std::path::Path) -> Result<()> {
        std::fs::write(path, self.read_pdf_bytes(source_id)?)?;
        Ok(())
    }

    /// The pages of a PDF a document keeps are `1..=n`; this says `n`, or `None` when no PDF is kept.
    pub fn pdf_page_count(&self, source_id: i64) -> Result<Option<i64>> {
        Ok(self.conn.query_row("SELECT page_count FROM pdf_file WHERE source_id = ?1", [source_id], |r| r.get(0)).optional()?)
    }

    /// Keeps a PDF with a document that was imported without it (a PDF imported before the file was kept, or one whose pages could not be
    /// drawn then). The document must be a PDF document that keeps none, and the file must have as many pages as the document's words
    /// came from, so the words and the pages belong together.
    pub fn attach_pdf(&mut self, source_id: i64, pdf: &PdfData) -> Result<PdfRow> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let kind: Option<(String, String)> = tx
            .query_row("SELECT source_type, formatting_runs_json FROM source WHERE id = ?1", [source_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let (kind, runs) = kind.ok_or(Error::NotFound { what: "source", id: source_id })?;
        if kind != "pdf" {
            return Err(Error::Invalid("only a document that was imported from a PDF can have a PDF attached".to_string()));
        }
        let kept: Option<i64> = tx.query_row("SELECT source_id FROM pdf_file WHERE source_id = ?1", [source_id], |r| r.get(0)).optional()?;
        if kept.is_some() {
            return Err(Error::Invalid("this document already keeps its PDF".to_string()));
        }
        // The words came from pages; the highest page that has words must exist in this file.
        let highest_page = serde_json::from_str::<Vec<serde_json::Value>>(&runs)
            .unwrap_or_default()
            .iter()
            .filter(|r| r["kind"] == "page")
            .filter_map(|r| r["page"].as_i64())
            .max()
            .unwrap_or(0);
        if (pdf.pages.len() as i64) < highest_page {
            return Err(Error::Invalid(format!(
                "this PDF has {} pages, but the document's words came from page {highest_page}; it is not the same file",
                pdf.pages.len()
            )));
        }
        insert_pdf(&tx, source_id, pdf)?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "pdf.attach",
            Some(("source", source_id)),
            None,
            Some(&serde_json::json!({ "file_name": pdf.file_name, "pages": pdf.pages.len(), "size_bytes": pdf.bytes.len() })),
        )?;
        tx.commit()?;
        self.get_pdf(source_id)?.ok_or(Error::NotFound { what: "pdf", id: source_id })
    }
}
