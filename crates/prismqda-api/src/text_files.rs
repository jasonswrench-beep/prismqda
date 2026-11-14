//! `text.read` and `text.write`: a small text file in and out, for the files people hand to PrismQDA and keep
//! outside a project (a translation of the window's words, for one). Neither needs a project to be open.
//!
//! Reading takes what spreadsheets and editors really write: UTF-8, UTF-8 with a byte-order mark, UTF-16 (what
//! Excel's "Unicode Text" is), and the old Windows-1252 that Excel's plain "CSV" uses; the encoding found is told
//! back, so the window can say when accented letters may have gone wrong.

use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::ApiError;

/// The largest file read. A translation of every word of the window is about a megabyte.
const MAX_READ: u64 = 16 * 1024 * 1024;
/// The largest text written.
const MAX_WRITE: usize = 32 * 1024 * 1024;

fn needs_full_path(path: &str) -> Result<&Path, ApiError> {
    let p = Path::new(path.trim());
    if path.trim().is_empty() || !p.is_absolute() {
        return Err(ApiError::new("invalid", "type the full path of the file, for example C:\\Translations\\fr.csv"));
    }
    Ok(p)
}

/// Turns the bytes of a file into text and says which encoding was used.
pub(crate) fn decode(bytes: &[u8]) -> (String, &'static str) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(rest).into_owned(), "utf-8");
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (s, _, _) = encoding_rs::UTF_16LE.decode(bytes);
        return (s.into_owned(), "utf-16");
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (s, _, _) = encoding_rs::UTF_16BE.decode(bytes);
        return (s.into_owned(), "utf-16");
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_string(), "utf-8"),
        Err(_) => {
            let (s, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            (s.into_owned(), "windows-1252")
        }
    }
}

/// `{ name, text, bytes, encoding }` for a file.
pub(crate) fn read(path: &str) -> Result<Value, ApiError> {
    let p = needs_full_path(path)?;
    let meta = fs::metadata(p).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", p.display())))?;
    if !meta.is_file() {
        return Err(ApiError::new("invalid", format!("{} is not a file", p.display())));
    }
    if meta.len() > MAX_READ {
        return Err(ApiError::new("too_large", format!("{} is larger than 16 MB, which is more than a translation file needs", p.display())));
    }
    let bytes = fs::read(p).map_err(|e| ApiError::new("io", format!("cannot read {}: {e}", p.display())))?;
    let (text, encoding) = decode(&bytes);
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Ok(json!({ "name": name, "text": text, "bytes": bytes.len(), "encoding": encoding }))
}

/// Writes text as UTF-8. A file that is already there is not replaced unless `replace` says so (the native save dialog
/// has asked the person already).
pub(crate) fn write(path: &str, text: &str, replace: bool) -> Result<Value, ApiError> {
    let p = needs_full_path(path)?;
    if text.len() > MAX_WRITE {
        return Err(ApiError::new("too_large", "the text is larger than 32 MB, which is more than a translation file needs"));
    }
    if let Some(dir) = p.parent() {
        if !dir.as_os_str().is_empty() && !dir.is_dir() {
            return Err(ApiError::new("invalid", format!("the folder {} does not exist", dir.display())));
        }
    }
    if p.exists() && !replace {
        return Err(ApiError::new("invalid", format!("{} already exists, so nothing was saved; choose another name", p.display())));
    }
    fs::write(p, text.as_bytes()).map_err(|e| ApiError::new("io", format!("cannot save {}: {e}", p.display())))?;
    Ok(json!({ "path": p.display().to_string(), "bytes": text.len() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_encodings_spreadsheets_write_are_read() {
        assert_eq!(decode("Caf\u{e9}".as_bytes()), ("Caf\u{e9}".to_string(), "utf-8"));
        assert_eq!(decode(b"\xEF\xBB\xBFname").0, "name");
        // UTF-16 little endian with a mark, as Excel's "Unicode Text"
        assert_eq!(decode(&[0xFF, 0xFE, b'a', 0, 0xE9, 0]), ("a\u{e9}".to_string(), "utf-16"));
        // Excel's plain CSV on Windows: 0xE9 alone is not UTF-8
        assert_eq!(decode(b"Caf\xE9"), ("Caf\u{e9}".to_string(), "windows-1252"));
    }
}
