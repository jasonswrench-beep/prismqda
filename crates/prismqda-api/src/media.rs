//! Recordings through the command layer: linking one to a document, importing a recording together with its captions, and the reply
//! a player gets when it asks for part of a recording. The development bridge and the desktop window both answer a player with
//! [`Api::media_reply`], so what the browser check plays is what the app plays.

use std::path::{Path, PathBuf};

use prismqda_engine::media_kind;

use crate::{Api, ApiError};

/// What to say to a player that asked for a recording (or part of one) by its document's number.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaReply {
    /// 200 for the whole recording, 206 for a part of it, 404 when there is no recording to give, 416 for a range outside it.
    pub status: u16,
    pub content_type: String,
    /// `bytes START-END/TOTAL` (or `bytes */TOTAL` for a refused range), when the reply is about a range.
    pub content_range: Option<String>,
    pub body: Vec<u8>,
    /// True for a reply that never changes for the same address (a picture), so a window may keep it for a while instead of asking again.
    pub cacheable: bool,
}

impl MediaReply {
    pub(crate) fn plain(status: u16, words: &str) -> Self {
        Self { status, content_type: "text/plain; charset=utf-8".to_string(), content_range: None, body: words.as_bytes().to_vec(), cacheable: false }
    }
}

/// What a window asks for a page of a PDF by: `<document number>/<page>/<width in pixels>`, as in `pdf/12/3/900` once the `pdf/` is taken off.
pub fn parse_pdf_page_path(rest: &str) -> Option<(i64, u32, u32)> {
    let mut parts = rest.trim_matches('/').split('/');
    let id = parts.next()?.parse::<i64>().ok()?;
    let page = parts.next()?.parse::<u32>().ok()?;
    let width = parts.next()?.parse::<u32>().ok()?;
    parts.next().is_none().then_some((id, page, width))
}

/// The first byte and the last byte wanted by a `Range: bytes=...` header, for a recording of `total` bytes. `Ok(None)` is a header that
/// asks for nothing in particular (a form this does not read); `Err(())` is a range that is not in the recording.
fn parse_range(header: &str, total: u64) -> Result<Option<(u64, Option<u64>)>, ()> {
    let Some(spec) = header.trim().strip_prefix("bytes=") else { return Ok(None) };
    // Only the first range is answered; a player asks for one at a time.
    let first = spec.split(',').next().unwrap_or("").trim();
    let Some((from, to)) = first.split_once('-') else { return Ok(None) };
    if from.is_empty() {
        // The last N bytes.
        let n: u64 = to.trim().parse().map_err(|_| ())?;
        if n == 0 || total == 0 {
            return Err(());
        }
        return Ok(Some((total.saturating_sub(n), None)));
    }
    let start: u64 = from.trim().parse().map_err(|_| ())?;
    if start >= total {
        return Err(());
    }
    let end = if to.trim().is_empty() { None } else { Some(to.trim().parse::<u64>().map_err(|_| ())?) };
    if end.is_some_and(|e| e < start) {
        return Err(());
    }
    Ok(Some((start, end)))
}

impl Api {
    /// The answer to a request for the picture that document `source_id` is: the file as it was imported. Only a picture of the open
    /// project can be asked for.
    pub fn image_reply(&mut self, source_id: i64) -> MediaReply {
        let Ok(project) = self.project() else { return MediaReply::plain(404, "no project is open") };
        match project.read_image_bytes(source_id) {
            Ok((mime, bytes)) => MediaReply { status: 200, content_type: mime, content_range: None, body: bytes, cacheable: true },
            Err(e) => MediaReply::plain(404, &e.to_string()),
        }
    }

    /// The answer to a request for one page of the PDF that document `source_id` keeps, drawn `width` pixels wide as a PNG. Only a PDF
    /// of the open project can be asked for.
    pub fn pdf_page_reply(&mut self, source_id: i64, page: u32, width: u32) -> MediaReply {
        let Ok(project) = self.project() else { return MediaReply::plain(404, "no project is open") };
        let bytes = match project.read_pdf_bytes(source_id) {
            Ok(bytes) => bytes,
            Err(e) => return MediaReply::plain(404, &e.to_string()),
        };
        match prismqda_pdfview::render_page_png(&bytes, page, width) {
            Ok(png) => MediaReply { status: 200, content_type: "image/png".to_string(), content_range: None, body: png, cacheable: true },
            Err(e) => MediaReply::plain(404, &e.to_string()),
        }
    }

    /// The answer to a player's request for the recording of document `source_id`, with its `Range` header if it sent one.
    pub fn media_reply(&mut self, source_id: i64, range: Option<&str>) -> MediaReply {
        let Ok(project) = self.project() else { return MediaReply::plain(404, "no project is open") };
        let row = match project.get_media(source_id) {
            Ok(Some(row)) => row,
            Ok(None) => return MediaReply::plain(404, "this document has no recording linked"),
            Err(e) => return MediaReply::plain(404, &e.to_string()),
        };
        let total = row.size_bytes.max(0) as u64;
        let wanted = match range.map(|h| parse_range(h, total)) {
            Some(Err(())) => {
                let mut r = MediaReply::plain(416, "that part of the recording does not exist");
                r.content_range = Some(format!("bytes */{total}"));
                return r;
            }
            Some(Ok(Some(w))) => Some(w),
            _ => None,
        };
        let (start, end) = wanted.unwrap_or((0, None));
        match project.read_media(source_id, start, end) {
            Ok(chunk) => {
                // A whole small recording is a plain 200; anything else says which part it is.
                let whole = wanted.is_none() && chunk.start == 0 && chunk.end + 1 == chunk.total;
                MediaReply {
                    status: if whole { 200 } else { 206 },
                    content_type: chunk.mime.to_string(),
                    content_range: if whole { None } else { Some(format!("bytes {}-{}/{}", chunk.start, chunk.end, chunk.total)) },
                    body: chunk.bytes,
                    cacheable: false,
                }
            }
            Err(e) => MediaReply::plain(if matches!(e, prismqda_engine::Error::Invalid(_)) { 404 } else { 500 }, &e.to_string()),
        }
    }
}

/// The caption file that goes with a recording: the same name with a WebVTT or SubRip ending, in the same folder.
pub(crate) fn companion_captions(media: &Path) -> Option<PathBuf> {
    let stem = media.file_stem()?.to_os_string();
    let dir = media.parent()?;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        let same_stem = p.file_stem().is_some_and(|s| s.eq_ignore_ascii_case(&stem));
        let caption = p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("vtt") || e.eq_ignore_ascii_case("srt"));
        if same_stem && caption && p.is_file() {
            return Some(p);
        }
    }
    None
}

/// For a recording the person chose to import: the kind it is, and the captions beside it if there are any (a recording with none comes
/// in as a document of its own, coded by time), or why it cannot be imported. `None` for a file that is not a recording.
pub(crate) fn recording_to_import(path: &str) -> Result<Option<(&'static str, Option<PathBuf>)>, ApiError> {
    let Some(kind) = media_kind(path) else { return Ok(None) };
    let media = PathBuf::from(path);
    if !media.is_absolute() {
        return Err(ApiError::new("invalid", "type the full path of the recording, for example C:\\Recordings\\interview-1.mp3"));
    }
    if !prismqda_engine::paths::is_local_drive_path(path) {
        return Err(ApiError::new("invalid", "PrismQDA plays a recording only from a drive with a letter, such as C: or a network folder that has been given a letter such as Z:, and not from a network address that starts with two slashes; give the folder a drive letter in Windows and choose the recording again"));
    }
    if !media.metadata().is_ok_and(|m| m.is_file() && m.len() > 0) {
        return Err(ApiError::new("io", format!("the recording {path} cannot be read: it is missing, empty, or a folder")));
    }
    Ok(Some((kind, companion_captions(&media))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_a_pdf_is_asked_for_by_document_page_and_width() {
        assert_eq!(parse_pdf_page_path("12/3/900"), Some((12, 3, 900)));
        assert_eq!(parse_pdf_page_path("/12/3/900/"), Some((12, 3, 900)));
        for wrong in ["", "12", "12/3", "12/3/x", "x/3/900", "12/3/900/1", "12/-3/900", "12/3/-900"] {
            assert_eq!(parse_pdf_page_path(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_range_header_is_read_in_the_shapes_players_send() {
        assert_eq!(parse_range("bytes=0-", 1000), Ok(Some((0, None))));
        assert_eq!(parse_range("bytes=100-199", 1000), Ok(Some((100, Some(199)))));
        assert_eq!(parse_range("bytes=-200", 1000), Ok(Some((800, None))), "the last 200 bytes");
        assert_eq!(parse_range("bytes=-5000", 1000), Ok(Some((0, None))), "more than there is is all of it");
        assert_eq!(parse_range("bytes=0-9, 20-29", 1000), Ok(Some((0, Some(9)))), "only the first range is answered");
        assert_eq!(parse_range("items=0-9", 1000), Ok(None), "another kind of range is not read");
        for outside in ["bytes=1000-", "bytes=2000-3000", "bytes=50-10", "bytes=x-", "bytes=-0"] {
            assert_eq!(parse_range(outside, 1000), Err(()), "{outside}");
        }
    }
}
