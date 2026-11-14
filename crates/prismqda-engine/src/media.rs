//! Recordings (specification 22.1, the first part). A document, usually a transcript read from captions, can have an audio or video
//! recording linked to it. PrismQDA keeps where the recording is on this computer, a quick fingerprint to tell whether the file has
//! changed, and what kind it is; it does not copy the recording into the project. A unit of a transcript made from captions knows
//! where in the recording it is spoken, so the text and the recording can be followed together. A recording can also be a document of
//! its own, imported without captions (`stands_alone`): its text is a description of it, and stretches of it are coded by time (see
//! `regions.rs`).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::ops::sha256_hex;
use crate::project::Project;
use crate::{Error, Result};

const AUDIO: &[&str] = &["mp3", "wav", "m4a", "aac", "flac", "ogg", "oga", "opus", "wma"];
const VIDEO: &[&str] = &["mp4", "mov", "avi", "mkv", "webm", "m4v"];
/// The most one request for a part of a recording returns, so a player that asks for the whole file is given it in pieces.
pub const MAX_CHUNK: u64 = 4 * 1024 * 1024;
/// How much of each end of a recording goes into its fingerprint.
const FINGERPRINT_EDGE: u64 = 1024 * 1024;

/// `audio` or `video` for a file name PrismQDA plays, from its ending; `None` for any other kind of file.
pub fn media_kind(path: &str) -> Option<&'static str> {
    let ext = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    if AUDIO.contains(&ext.as_str()) {
        Some("audio")
    } else if VIDEO.contains(&ext.as_str()) {
        Some("video")
    } else {
        None
    }
}

/// What a browser or player is told the recording is.
pub fn mime_for(path: &str) -> &'static str {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "m4a" | "aac" => "audio/mp4",
        "flac" => "audio/flac",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wma" => "audio/x-ms-wma",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MediaRow {
    pub source_id: i64,
    pub kind: String,
    pub path: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub fingerprint: String,
    /// The last time any unit of the document names, in milliseconds; `None` when the units have no times.
    pub duration_ms: Option<i64>,
    pub created_at: String,
    /// True for a recording imported without captions: its document's text is a description of the recording, and the recording itself
    /// is what is coded. False for a recording linked to a transcript.
    pub stands_alone: bool,
    /// False when the recording is not at its path now (it was moved or the drive is not there).
    pub found: bool,
    /// False when the file at the path is not the size it was when it was linked, so it was probably replaced.
    pub unchanged: bool,
}

/// A piece of a recording, for a player that asks for a range of bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaChunk {
    pub bytes: Vec<u8>,
    pub start: u64,
    /// The last byte this piece holds.
    pub end: u64,
    pub total: u64,
    pub mime: &'static str,
}

/// A quick fingerprint: the size and the first and last megabyte, hashed. Enough to tell that a recording was replaced, without reading all of it.
fn fingerprint_of(file: &mut File, size: u64) -> Result<String> {
    let mut data = size.to_le_bytes().to_vec();
    let head = size.min(FINGERPRINT_EDGE) as usize;
    let mut buf = vec![0u8; head];
    file.seek(SeekFrom::Start(0))?;
    file.read_exact(&mut buf)?;
    data.extend_from_slice(&buf);
    if size > FINGERPRINT_EDGE {
        let tail = (size - FINGERPRINT_EDGE).min(FINGERPRINT_EDGE) as usize;
        let mut buf = vec![0u8; tail];
        file.seek(SeekFrom::End(-(tail as i64)))?;
        file.read_exact(&mut buf)?;
        data.extend_from_slice(&buf);
    }
    Ok(sha256_hex(&data))
}

/// A recording is only opened from a drive with a letter (see [`crate::paths`]): a path that came from inside a project or an exchange file
/// may name a network address, and merely looking at that connects to it. The words say what to do instead.
fn refuse_network_path(path: &str) -> Result<()> {
    if crate::paths::is_local_drive_path(path) {
        return Ok(());
    }
    Err(Error::Invalid("PrismQDA plays a recording only from a drive with a letter, such as C: or a network folder that has been given a letter such as Z:, and not from a network address that starts with two slashes; give the folder a drive letter in Windows and link the recording again".to_string()))
}

/// A recording's size and quick fingerprint, without reading all of it. A recording imported without captions is told from one imported
/// before by this, as a document read from a file is told by the hash of the whole file.
pub fn recording_fingerprint(path: &str) -> Result<(String, u64)> {
    refuse_network_path(path)?;
    let mut file = File::open(path).map_err(|e| Error::Invalid(format!("the recording cannot be opened: {e}")))?;
    let size = file.metadata()?.len();
    Ok((fingerprint_of(&mut file, size)?, size))
}

fn row_json(r: &MediaRow) -> Value {
    json!({
        "kind": r.kind, "path": r.path, "file_name": r.file_name, "size_bytes": r.size_bytes,
        "fingerprint": r.fingerprint, "duration_ms": r.duration_ms, "created_at": r.created_at, "stands_alone": r.stands_alone,
    })
}

/// What a list of documents needs to know about a document that has a recording linked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRecording {
    pub kind: String,
    pub file_name: String,
    pub size_bytes: i64,
    pub stands_alone: bool,
}

/// The description a recording starts with when it is imported without captions, until its person writes a better one.
pub fn default_recording_description(file_name: &str, kind: &str, size_bytes: u64) -> String {
    let what = if kind == "video" { "Video" } else { "Audio recording" };
    let mb = size_bytes as f64 / (1024.0 * 1024.0);
    let size = if mb >= 1.0 { format!("{mb:.1} MB") } else { format!("{} KB", ((size_bytes as f64 / 1024.0).round() as u64).max(1)) };
    format!("{what} {file_name}, {size}. Add a description of what it holds: who speaks, where it was made, and what it is about.")
}

impl Project {
    fn media_from_row(r: &rusqlite::Row) -> rusqlite::Result<MediaRow> {
        let path: String = r.get(2)?;
        let size: i64 = r.get(4)?;
        let fingerprint: String = r.get(5)?;
        // Cheap on purpose (a player asks for a recording again and again): the file is there, and is the size it was when linked.
        // A path from inside a file may name a network address; looking at one connects to it, so it is never looked at (see `paths`).
        let (found, unchanged) = match if crate::paths::is_local_drive_path(&path) { std::fs::metadata(&path).ok() } else { None } {
            Some(meta) if meta.is_file() => (true, meta.len() as i64 == size),
            _ => (false, false),
        };
        Ok(MediaRow {
            source_id: r.get(0)?,
            kind: r.get(1)?,
            path,
            file_name: r.get(3)?,
            size_bytes: size,
            fingerprint,
            duration_ms: r.get(6)?,
            created_at: r.get(7)?,
            stands_alone: r.get::<_, i64>(8)? != 0,
            found,
            unchanged,
        })
    }

    /// The recording linked to a document, if any, and whether it is still where it was.
    pub fn get_media(&self, source_id: i64) -> Result<Option<MediaRow>> {
        Ok(self
            .conn
            .query_row(
                "SELECT source_id, kind, path, file_name, size_bytes, fingerprint, duration_ms, created_at, stands_alone FROM media WHERE source_id = ?1",
                [source_id],
                Self::media_from_row,
            )
            .optional()?)
    }

    /// Every linked recording.
    pub fn list_media(&self) -> Result<Vec<MediaRow>> {
        let mut stmt = self.conn.prepare("SELECT source_id, kind, path, file_name, size_bytes, fingerprint, duration_ms, created_at, stands_alone FROM media ORDER BY source_id")?;
        let rows = stmt.query_map([], Self::media_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Links a recording to a document (replacing one already linked). The recording stays where it is. A recording linked again to a
    /// document that stands for it keeps standing for it.
    pub fn link_media(&mut self, source_id: i64, path: &str) -> Result<MediaRow> {
        self.link_media_as(source_id, path, None)
    }

    /// The same, saying whether the document stands for the recording (`Some`), or leaving that as it was (`None`).
    pub fn link_media_as(&mut self, source_id: i64, path: &str, stands_alone: Option<bool>) -> Result<MediaRow> {
        self.ensure_writable()?;
        let p = Path::new(path.trim());
        if !p.is_absolute() {
            return Err(Error::Invalid("type the full path of the recording, for example C:\\Recordings\\interview-1.mp3".to_string()));
        }
        refuse_network_path(path)?;
        let shown = p.to_string_lossy().to_string();
        let kind = media_kind(&shown).ok_or_else(|| Error::Invalid("PrismQDA plays MP3, WAV, M4A, AAC, FLAC, OGG, and WMA audio and MP4, MOV, AVI, MKV, WebM, and M4V video".to_string()))?;
        let mut file = File::open(p).map_err(|e| Error::Invalid(format!("the recording cannot be opened: {e}")))?;
        let meta = file.metadata()?;
        if !meta.is_file() || meta.len() == 0 {
            return Err(Error::Invalid("that is not a recording: it is a folder or an empty file".to_string()));
        }
        let fingerprint = fingerprint_of(&mut file, meta.len())?;
        let file_name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| shown.clone());
        let duration: Option<i64> = self.conn.query_row("SELECT MAX(time_end_ms) FROM unit WHERE source_id = ?1", [source_id], |r| r.get(0))?;
        let before = self.get_media(source_id)?;
        let stands_alone = stands_alone.unwrap_or_else(|| before.as_ref().is_some_and(|b| b.stands_alone));
        let tx = self.conn.transaction()?;
        let exists: Option<i64> = tx.query_row("SELECT id FROM source WHERE id = ?1", [source_id], |r| r.get(0)).optional()?;
        exists.ok_or(Error::NotFound { what: "source", id: source_id })?;
        tx.execute("DELETE FROM media WHERE source_id = ?1", [source_id])?;
        tx.execute(
            "INSERT INTO media (source_id, kind, path, file_name, size_bytes, fingerprint, duration_ms, created_at, stands_alone)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?8)",
            params![source_id, kind, shown, file_name, meta.len() as i64, fingerprint, duration, stands_alone as i64],
        )?;
        let created: String = tx.query_row("SELECT created_at FROM media WHERE source_id = ?1", [source_id], |r| r.get(0))?;
        let row = MediaRow { source_id, kind: kind.to_string(), path: shown, file_name, size_bytes: meta.len() as i64, fingerprint, duration_ms: duration, created_at: created, stands_alone, found: true, unchanged: true };
        event::insert(&tx, &self.actor, &self.session, "media.link", Some(("source", source_id)), before.as_ref().map(row_json).as_ref(), Some(&row_json(&row)))?;
        tx.commit()?;
        Ok(row)
    }

    /// Takes the link to a recording away. The recording itself is not touched.
    pub fn unlink_media(&mut self, source_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let Some(before) = self.get_media(source_id)? else {
            return Err(Error::Invalid("this document has no recording linked".to_string()));
        };
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM media WHERE source_id = ?1", [source_id])?;
        event::insert(&tx, &self.actor, &self.session, "media.unlink", Some(("source", source_id)), Some(&row_json(&before)), None)?;
        tx.commit()?;
        Ok(())
    }

    /// A part of the recording linked to a document, from byte `start` to byte `end` (the last one wanted), at most [`MAX_CHUNK`] long.
    pub fn read_media(&self, source_id: i64, start: u64, end: Option<u64>) -> Result<MediaChunk> {
        let row = self.get_media(source_id)?.ok_or_else(|| Error::Invalid("this document has no recording linked".to_string()))?;
        if !crate::paths::is_local_drive_path(&row.path) {
            // A path that came from inside a file is not opened, and is answered as a recording that is not there (see `paths`).
            return Err(Error::Invalid(format!("the recording {} is not at {} any more; link it again from its new place", row.file_name, row.path)));
        }
        let mut file = File::open(&row.path).map_err(|_| Error::Invalid(format!("the recording {} is not at {} any more; link it again from its new place", row.file_name, row.path)))?;
        let total = file.metadata()?.len();
        if start >= total {
            return Err(Error::Invalid(format!("the recording has {total} bytes, so it has nothing from byte {start}")));
        }
        let last = end.unwrap_or(total - 1).min(total - 1).min(start + MAX_CHUNK - 1);
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![0u8; (last - start + 1) as usize];
        file.read_exact(&mut bytes)?;
        Ok(MediaChunk { bytes, start, end: last, total, mime: mime_for(&row.path) })
    }
}
