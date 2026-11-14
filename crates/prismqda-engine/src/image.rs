//! Images (specification 22.2, the first part). A picture comes into the project as a document: its text is the description a
//! screen reader would speak, and the picture itself is kept inside the project file exactly as the file was. Areas of the picture
//! are coded in `regions.rs`.
//!
//! PrismQDA does not decode pictures. It reads only the few bytes at the start of a file that say what kind of picture it is and how
//! large, and it reads a camera's turn marker so the size it keeps is the size the picture is shown at.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::{Error, Result};

/// The most one picture may be. A picture is kept whole inside the project, so there is a limit.
pub const MAX_IMAGE_BYTES: usize = 64 * 1024 * 1024;
/// The most pixels along either side. Beyond this a picture cannot be shown on any screen without being cut up.
pub const MAX_IMAGE_SIDE: i64 = 30_000;

/// The kinds of picture that are kept.
pub const IMAGE_MIMES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp"];

/// A picture ready to store, as an importer found it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageData {
    pub mime: String,
    pub file_name: String,
    /// Pixels, as the picture is shown.
    pub width: i64,
    pub height: i64,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

/// Which source of an import batch a picture belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub source: usize,
    pub image: ImageData,
}

/// What is known of a picture without its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageRow {
    pub source_id: i64,
    pub mime: String,
    pub file_name: String,
    pub width: i64,
    pub height: i64,
    pub size_bytes: i64,
    pub created_at: String,
}

/// The kind of picture a file name says it is, from its ending: `Ok(Some(mime))` for a kind PrismQDA keeps, `Err(reason)` for a kind
/// it knows it cannot show, `Ok(None)` for something that is not a picture.
pub fn image_kind_of_name(name: &str) -> std::result::Result<Option<&'static str>, String> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    Ok(Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tif" | "tiff" | "heic" | "heif" | "avif" | "jp2" | "psd" | "raw" | "cr2" | "nef" | "arw" | "dng" => {
            return Err(format!(
                "PrismQDA keeps PNG, JPEG, GIF, WebP, and BMP pictures, because those are the kinds Windows shows without anything added. Save {name} as a PNG or JPEG and import that."
            ))
        }
        _ => return Ok(None),
    }))
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]) as u32)
}
fn le16(b: &[u8], at: usize) -> Option<u32> {
    Some(u16::from_le_bytes([*b.get(at)?, *b.get(at + 1)?]) as u32)
}
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]))
}
fn le32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]))
}
fn le24(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, 0]))
}

/// What the first bytes of a file say it is: the kind of picture and its size in pixels as stored (before any turn marker).
/// `None` when the bytes are not one of the kinds PrismQDA keeps.
pub fn sniff(bytes: &[u8]) -> Option<(&'static str, i64, i64)> {
    if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") && &bytes[12..16] == b"IHDR" {
        return Some(("image/png", be32(bytes, 16)? as i64, be32(bytes, 20)? as i64));
    }
    if bytes.len() >= 10 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Some(("image/gif", le16(bytes, 6)? as i64, le16(bytes, 8)? as i64));
    }
    if bytes.len() >= 26 && bytes.starts_with(b"BM") {
        // The header size at 14 says which header follows: 12 bytes (16-bit sizes) or a larger one (32-bit sizes).
        let header = le32(bytes, 14)?;
        let (w, h) = if header == 12 { (le16(bytes, 18)? as i64, le16(bytes, 20)? as i64) } else { (le32(bytes, 18)? as i32 as i64, le32(bytes, 22)? as i32 as i64) };
        return Some(("image/bmp", w.abs(), h.abs()));
    }
    if bytes.len() >= 30 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return match &bytes[12..16] {
            b"VP8 " => Some(("image/webp", (le16(bytes, 26)? & 0x3fff) as i64, (le16(bytes, 28)? & 0x3fff) as i64)),
            b"VP8L" if bytes[20] == 0x2f => {
                let bits = le32(bytes, 21)?;
                Some(("image/webp", ((bits & 0x3fff) + 1) as i64, (((bits >> 14) & 0x3fff) + 1) as i64))
            }
            b"VP8X" => Some(("image/webp", (le24(bytes, 24)? + 1) as i64, (le24(bytes, 27)? + 1) as i64)),
            _ => None,
        };
    }
    if bytes.len() >= 4 && bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        // Walk the markers to the first start-of-frame, which holds the size.
        let mut at = 2;
        while at + 4 <= bytes.len() {
            if bytes[at] != 0xff {
                at += 1;
                continue;
            }
            let marker = bytes[at + 1];
            if marker == 0xff {
                at += 1;
                continue;
            }
            if marker == 0xd8 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
                at += 2;
                continue;
            }
            let len = be16(bytes, at + 2)? as usize;
            if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
                return Some(("image/jpeg", be16(bytes, at + 7)? as i64, be16(bytes, at + 5)? as i64));
            }
            if marker == 0xda || len < 2 {
                return None;
            }
            at += 2 + len;
        }
    }
    None
}

/// The turn marker a JPEG's camera wrote (EXIF orientation, 1 to 8), when there is one. 5 to 8 mean the picture is shown on its side,
/// so its width and height swap.
pub fn exif_orientation(bytes: &[u8]) -> Option<u16> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xff {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xda || marker == 0xd9 {
            return None;
        }
        let len = be16(bytes, at + 2)? as usize;
        if marker == 0xe1 && bytes.get(at + 4..at + 10)? == b"Exif\0\0" {
            let tiff = bytes.get(at + 10..at + 2 + len)?;
            let little = match tiff.get(0..2)? {
                b"II" => true,
                b"MM" => false,
                _ => return None,
            };
            let u16_at = |o: usize| -> Option<u32> { if little { le16(tiff, o) } else { be16(tiff, o) } };
            let u32_at = |o: usize| -> Option<u32> { if little { le32(tiff, o) } else { be32(tiff, o) } };
            let ifd = u32_at(4)? as usize;
            let count = u16_at(ifd)? as usize;
            for i in 0..count.min(200) {
                let entry = ifd + 2 + i * 12;
                if u16_at(entry)? == 0x0112 {
                    let v = u16_at(entry + 8)? as u16;
                    return (1..=8).contains(&v).then_some(v);
                }
            }
            return None;
        }
        at += 2 + len;
    }
    None
}

/// What a camera wrote into a photograph besides the picture: when it was taken, with what, and where. A part is `None` when the file
/// does not say, or says something that cannot be right (a date of zeros, a place off the map).
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct PictureDetails {
    /// `YYYY-MM-DD HH:MM:SS`, as the camera's clock wrote it. A camera writes no time zone, so none is given.
    pub taken: Option<String>,
    /// The make and model, as one name.
    pub camera: Option<String>,
    /// Degrees north (negative for south), to six decimals.
    pub latitude: Option<f64>,
    /// Degrees east (negative for west), to six decimals.
    pub longitude: Option<f64>,
}

impl PictureDetails {
    /// True when the file says nothing worth keeping.
    pub fn is_empty(&self) -> bool {
        self.taken.is_none() && self.camera.is_none() && !self.has_place()
    }

    pub fn has_place(&self) -> bool {
        self.latitude.is_some() && self.longitude.is_some()
    }

    /// `YYYY-MM-DD`
    pub fn date(&self) -> Option<&str> {
        self.taken.as_deref().map(|t| &t[..10])
    }

    /// `YYYY-MM`
    pub fn month(&self) -> Option<&str> {
        self.taken.as_deref().map(|t| &t[..7])
    }

    /// `YYYY`
    pub fn year(&self) -> Option<&str> {
        self.taken.as_deref().map(|t| &t[..4])
    }
}

/// A camera's block of facts (the part of an EXIF block after its six-letter label), read in whichever byte order it was written.
struct Tiff<'a> {
    data: &'a [u8],
    little: bool,
}

impl<'a> Tiff<'a> {
    fn u16(&self, at: usize) -> Option<u32> {
        if self.little { le16(self.data, at) } else { be16(self.data, at) }
    }

    fn u32(&self, at: usize) -> Option<u32> {
        if self.little { le32(self.data, at) } else { be32(self.data, at) }
    }

    /// The entry for `tag` in the list that starts at `ifd`: its kind, how many values it holds, and where the first one starts.
    fn entry(&self, ifd: usize, tag: u32) -> Option<(u32, usize, usize)> {
        let count = self.u16(ifd)? as usize;
        for i in 0..count.min(500) {
            let at = ifd + 2 + i * 12;
            if self.u16(at)? != tag {
                continue;
            }
            let kind = self.u16(at + 2)?;
            let n = self.u32(at + 4)? as usize;
            let size = n.checked_mul(match kind {
                1 | 2 | 7 => 1,
                3 => 2,
                4 | 9 => 4,
                5 | 10 => 8,
                _ => return None,
            })?;
            let start = if size <= 4 { at + 8 } else { self.u32(at + 8)? as usize };
            return (start.checked_add(size)? <= self.data.len()).then_some((kind, n, start));
        }
        None
    }

    fn text(&self, ifd: usize, tag: u32) -> Option<String> {
        let (kind, n, start) = self.entry(ifd, tag)?;
        if kind != 2 {
            return None;
        }
        let bytes = self.data.get(start..start + n)?;
        let text = String::from_utf8_lossy(bytes);
        let text = text.split('\0').next().unwrap_or("").trim().to_string();
        (!text.is_empty()).then_some(text)
    }

    /// Where another list starts, when `tag` points to one.
    fn pointer(&self, ifd: usize, tag: u32) -> Option<usize> {
        let (kind, n, start) = self.entry(ifd, tag)?;
        (kind == 4 && n == 1).then(|| self.u32(start)).flatten().map(|v| v as usize)
    }

    /// A place's degrees, minutes and seconds as degrees.
    fn degrees(&self, ifd: usize, tag: u32) -> Option<f64> {
        let (kind, n, start) = self.entry(ifd, tag)?;
        if kind != 5 || n != 3 {
            return None;
        }
        let part = |i: usize| -> Option<f64> {
            let (num, den) = (self.u32(start + i * 8)?, self.u32(start + i * 8 + 4)?);
            (den != 0).then(|| f64::from(num) / f64::from(den))
        };
        Some(part(0)? + part(1)? / 60.0 + part(2)? / 3600.0)
    }
}

/// `2024:05:01 13:45:09` as `2024-05-01 13:45:09`, when it is a date and time that could have happened.
fn exif_date(text: &str) -> Option<String> {
    let b = text.as_bytes();
    if b.len() < 19 || !(0..19).all(|i| if matches!(i, 4 | 7 | 10 | 13 | 16) { b[i].is_ascii() } else { b[i].is_ascii_digit() }) {
        return None;
    }
    let number = |from: usize, to: usize| -> u32 { text[from..to].parse().unwrap_or(0) };
    let (year, month, day, hour, minute, second) = (number(0, 4), number(5, 7), number(8, 10), number(11, 13), number(14, 16), number(17, 19));
    if year < 1900 || !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}"))
}

/// The block of camera facts in a JPEG, when it has one.
fn exif_tiff(bytes: &[u8]) -> Option<&[u8]> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut at = 2;
    while at + 4 <= bytes.len() {
        if bytes[at] != 0xff {
            return None;
        }
        let marker = bytes[at + 1];
        if marker == 0xda || marker == 0xd9 {
            return None;
        }
        let len = be16(bytes, at + 2)? as usize;
        if marker == 0xe1 && bytes.get(at + 4..at + 10)? == b"Exif\0\0" {
            return bytes.get(at + 10..at + 2 + len);
        }
        at += 2 + len;
    }
    None
}

/// Reads when, with what, and where a photograph was taken from the facts its camera wrote into it. Only a JPEG carries them here;
/// anything else gives nothing. Nothing is decoded and nothing is guessed: what the file does not say stays empty.
pub fn picture_details(bytes: &[u8]) -> PictureDetails {
    let mut details = PictureDetails::default();
    let Some(data) = exif_tiff(bytes) else { return details };
    let little = match data.get(0..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => return details,
    };
    let tiff = Tiff { data, little };
    let Some(ifd0) = tiff.u32(4).map(|v| v as usize) else { return details };

    // When: the moment the shutter was pressed, else when the camera made the file, else when the file was last written.
    let exif = tiff.pointer(ifd0, 0x8769);
    details.taken = exif
        .and_then(|e| tiff.text(e, 0x9003).or_else(|| tiff.text(e, 0x9004)))
        .or_else(|| tiff.text(ifd0, 0x0132))
        .and_then(|t| exif_date(&t));

    // With what: the make and model, with the make left out of the model when the model starts with it.
    let make = tiff.text(ifd0, 0x010f);
    let model = tiff.text(ifd0, 0x0110);
    let camera = match (make, model) {
        (Some(make), Some(model)) => Some(if model.to_lowercase().starts_with(&make.to_lowercase()) { model } else { format!("{make} {model}") }),
        (None, Some(one)) | (Some(one), None) => Some(one),
        (None, None) => None,
    };
    details.camera = camera.map(|c| c.chars().take(80).collect());

    // Where: degrees north or south and east or west.
    if let Some(gps) = tiff.pointer(ifd0, 0x8825) {
        let side = |tag: u32, negative: &str| -> Option<f64> {
            let sign = tiff.text(gps, tag)?;
            Some(if sign.eq_ignore_ascii_case(negative) { -1.0 } else { 1.0 })
        };
        let (lat, lon) = (
            tiff.degrees(gps, 0x0002).zip(side(0x0001, "S")).map(|(d, s)| d * s),
            tiff.degrees(gps, 0x0004).zip(side(0x0003, "W")).map(|(d, s)| d * s),
        );
        if let (Some(lat), Some(lon)) = (lat, lon) {
            // A camera with no fix often writes zeros; that is not a place.
            if lat.abs() <= 90.0 && lon.abs() <= 180.0 && !(lat == 0.0 && lon == 0.0) {
                let six = |v: f64| (v * 1_000_000.0).round() / 1_000_000.0;
                details.latitude = Some(six(lat));
                details.longitude = Some(six(lon));
            }
        }
    }
    details
}

/// Reads a picture file: its kind and size as shown. Refuses what is not a picture of a kept kind, a picture with no size, one too
/// large to keep, and one whose ending says one kind while its bytes are another.
pub fn read_image(file_name: &str, bytes: Vec<u8>) -> std::result::Result<ImageData, String> {
    if bytes.is_empty() {
        return Err(format!("{file_name} is empty"));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("{file_name} is {} MB, and PrismQDA keeps pictures up to {} MB inside the project. Save it smaller and import that.", bytes.len() / (1024 * 1024), MAX_IMAGE_BYTES / (1024 * 1024)));
    }
    let Some((mime, mut w, mut h)) = sniff(&bytes) else {
        return Err(format!("{file_name} is not a PNG, JPEG, GIF, WebP, or BMP picture, whatever its ending says"));
    };
    if let Ok(Some(named)) = image_kind_of_name(file_name) {
        if named != mime {
            return Err(format!("{file_name} is named as {} but its bytes are {}. Rename it to match, then import it.", named.trim_start_matches("image/"), mime.trim_start_matches("image/")));
        }
    }
    if mime == "image/jpeg" && exif_orientation(&bytes).is_some_and(|o| o >= 5) {
        std::mem::swap(&mut w, &mut h);
    }
    if w <= 0 || h <= 0 {
        return Err(format!("{file_name} says it has no size"));
    }
    if w > MAX_IMAGE_SIDE || h > MAX_IMAGE_SIDE {
        return Err(format!("{file_name} is {w} by {h} pixels, and PrismQDA keeps pictures up to {MAX_IMAGE_SIDE} pixels on a side"));
    }
    Ok(ImageData { mime: mime.to_string(), file_name: file_name.to_string(), width: w, height: h, bytes })
}

/// The description a picture starts with, until its person writes a better one.
pub fn default_description(file_name: &str, width: i64, height: i64) -> String {
    format!("Picture {file_name}, {width} by {height} pixels. Add a description of what it shows.")
}

/// The picture's row, as the engine stores it, checked.
pub(crate) fn insert_image(tx: &Transaction, source_id: i64, image: &ImageData) -> Result<()> {
    if !IMAGE_MIMES.contains(&image.mime.as_str()) {
        return Err(Error::Invalid(format!("'{}' is not a kind of picture PrismQDA keeps", image.mime)));
    }
    if image.width <= 0 || image.height <= 0 || image.bytes.is_empty() {
        return Err(Error::Invalid(format!("the picture {} has no size or no bytes", image.file_name)));
    }
    tx.execute(
        "INSERT INTO image (source_id, mime, file_name, width, height, size_bytes, data, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![source_id, image.mime, image.file_name, image.width, image.height, image.bytes.len() as i64, image.bytes],
    )?;
    Ok(())
}

fn image_from_row(r: &rusqlite::Row) -> rusqlite::Result<ImageRow> {
    Ok(ImageRow { source_id: r.get(0)?, mime: r.get(1)?, file_name: r.get(2)?, width: r.get(3)?, height: r.get(4)?, size_bytes: r.get(5)?, created_at: r.get(6)? })
}

impl Project {
    /// What is known of the picture a document is, without its bytes; `None` for a document that is not a picture.
    pub fn get_image(&self, source_id: i64) -> Result<Option<ImageRow>> {
        Ok(self
            .conn
            .query_row("SELECT source_id, mime, file_name, width, height, size_bytes, created_at FROM image WHERE source_id = ?1", [source_id], image_from_row)
            .optional()?)
    }

    /// Every picture in the project, without its bytes.
    pub fn list_images(&self) -> Result<Vec<ImageRow>> {
        let mut stmt = self.conn.prepare("SELECT source_id, mime, file_name, width, height, size_bytes, created_at FROM image ORDER BY source_id")?;
        let rows = stmt.query_map([], image_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// The picture's file as it was imported, and what kind it is.
    pub fn read_image_bytes(&self, source_id: i64) -> Result<(String, Vec<u8>)> {
        self.conn
            .query_row("SELECT mime, data FROM image WHERE source_id = ?1", [source_id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or_else(|| Error::Invalid("this document is not a picture".to_string()))
    }

    /// Writes a picture's file to a path (for opening it in another program or keeping a copy).
    pub fn save_image_copy(&self, source_id: i64, path: &std::path::Path) -> Result<()> {
        let (_, bytes) = self.read_image_bytes(source_id)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0]);
        b
    }

    fn jpeg(w: u16, h: u16, orientation: Option<u16>) -> Vec<u8> {
        let mut b = vec![0xff, 0xd8];
        if let Some(o) = orientation {
            let mut tiff = b"MM\0\x2a\0\0\0\x08".to_vec();
            tiff.extend_from_slice(&[0, 1]); // one entry
            tiff.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1]);
            tiff.extend_from_slice(&o.to_be_bytes());
            tiff.extend_from_slice(&[0, 0]);
            tiff.extend_from_slice(&[0, 0, 0, 0]);
            let len = (2 + 6 + tiff.len()) as u16;
            b.extend_from_slice(&[0xff, 0xe1]);
            b.extend_from_slice(&len.to_be_bytes());
            b.extend_from_slice(b"Exif\0\0");
            b.extend_from_slice(&tiff);
        }
        b.extend_from_slice(&[0xff, 0xc0, 0, 17, 8]);
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        b
    }

    /// A JPEG header whose camera block says: Canon EOS R5, taken 2024-05-01 13:45:09, at 40 deg 26 min 46.8 sec north and 79 deg 58 min 56 sec west.
    fn photograph(little: bool, with_place: bool) -> Vec<u8> {
        fn w16(out: &mut Vec<u8>, little: bool, v: u16) {
            out.extend_from_slice(&if little { v.to_le_bytes() } else { v.to_be_bytes() });
        }
        fn w32(out: &mut Vec<u8>, little: bool, v: u32) {
            out.extend_from_slice(&if little { v.to_le_bytes() } else { v.to_be_bytes() });
        }
        fn entry(out: &mut Vec<u8>, little: bool, tag: u16, kind: u16, count: u32, value: u32) {
            w16(out, little, tag);
            w16(out, little, kind);
            w32(out, little, count);
            w32(out, little, value);
        }
        let mut t = if little { b"II\x2a\0".to_vec() } else { b"MM\0\x2a".to_vec() };
        w32(&mut t, little, 8);
        // The first list: make, model, a pointer to the second list, and a pointer to the third.
        w16(&mut t, little, if with_place { 4 } else { 3 });
        entry(&mut t, little, 0x010f, 2, 6, 62);
        entry(&mut t, little, 0x0110, 2, 13, 68);
        entry(&mut t, little, 0x8769, 4, 1, 82);
        if with_place {
            entry(&mut t, little, 0x8825, 4, 1, 120);
        }
        w32(&mut t, little, 0);
        // Not every list has the same length, so the entry that is not there leaves its twelve bytes empty.
        t.resize(62, 0);
        t.extend_from_slice(b"Canon\0");
        t.extend_from_slice(b"Canon EOS R5\0");
        t.resize(82, 0);
        w16(&mut t, little, 1);
        entry(&mut t, little, 0x9003, 2, 20, 100);
        w32(&mut t, little, 0);
        assert_eq!(t.len(), 100);
        t.extend_from_slice(b"2024:05:01 13:45:09\0");
        assert_eq!(t.len(), 120);
        w16(&mut t, little, 4);
        let side = |c: u8| u32::from_le_bytes([c, 0, 0, 0]);
        entry(&mut t, little, 0x0001, 2, 2, if little { side(b'N') } else { u32::from_be_bytes([b'N', 0, 0, 0]) });
        entry(&mut t, little, 0x0002, 5, 3, 174);
        entry(&mut t, little, 0x0003, 2, 2, if little { side(b'W') } else { u32::from_be_bytes([b'W', 0, 0, 0]) });
        entry(&mut t, little, 0x0004, 5, 3, 198);
        w32(&mut t, little, 0);
        assert_eq!(t.len(), 174);
        for (n, d) in [(40u32, 1u32), (26, 1), (468, 10), (79, 1), (58, 1), (56, 1)] {
            w32(&mut t, little, n);
            w32(&mut t, little, d);
        }
        let mut b = vec![0xff, 0xd8, 0xff, 0xe1];
        b.extend_from_slice(&((2 + 6 + t.len()) as u16).to_be_bytes());
        b.extend_from_slice(b"Exif\0\0");
        b.extend_from_slice(&t);
        b.extend_from_slice(&[0xff, 0xc0, 0, 17, 8, 0, 100, 0, 200, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        b
    }

    #[test]
    fn a_photographs_date_camera_and_place_are_read_in_either_byte_order() {
        for little in [false, true] {
            let d = picture_details(&photograph(little, true));
            assert_eq!(d.taken.as_deref(), Some("2024-05-01 13:45:09"), "little {little}");
            assert_eq!((d.date(), d.month(), d.year()), (Some("2024-05-01"), Some("2024-05"), Some("2024")));
            assert_eq!(d.camera.as_deref(), Some("Canon EOS R5"), "the make is not said twice");
            assert!((d.latitude.unwrap() - 40.446333).abs() < 1e-6, "{:?}", d.latitude);
            assert!((d.longitude.unwrap() + 79.982222).abs() < 1e-6, "west is negative: {:?}", d.longitude);
            assert!(d.has_place() && !d.is_empty());
        }
        let no_place = picture_details(&photograph(false, false));
        assert!(!no_place.has_place() && no_place.taken.is_some() && no_place.camera.is_some());
    }

    #[test]
    fn a_picture_that_says_nothing_gives_nothing_and_a_damaged_block_gives_no_error() {
        assert!(picture_details(&png(10, 10)).is_empty(), "only a JPEG carries these facts");
        assert!(picture_details(&jpeg(10, 10, None)).is_empty());
        assert!(picture_details(&jpeg(10, 10, Some(6))).is_empty(), "a turn marker alone is not a date");
        assert!(picture_details(&[]).is_empty());
        // A block cut short in the middle of a list reads as far as it goes and never panics.
        let whole = photograph(false, true);
        for cut in (0..whole.len()).step_by(7) {
            let _ = picture_details(&whole[..cut]);
        }
        assert_eq!(exif_date("0000:00:00 00:00:00"), None);
        assert_eq!(exif_date("2024:13:01 10:00:00"), None);
        assert_eq!(exif_date("2024:05:01 25:00:00"), None);
        assert_eq!(exif_date("not a date at all!"), None);
        assert_eq!(exif_date("2024:02:29 23:59:59").as_deref(), Some("2024-02-29 23:59:59"));
    }

    #[test]
    fn each_kept_kind_is_read_by_its_first_bytes() {
        assert_eq!(sniff(&png(640, 480)), Some(("image/png", 640, 480)));
        assert_eq!(sniff(&jpeg(1024, 768, None)), Some(("image/jpeg", 1024, 768)));
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&[0x20, 0x01, 0x10, 0x00, 0, 0, 0]);
        assert_eq!(sniff(&gif), Some(("image/gif", 288, 16)));
        let mut bmp = vec![0u8; 30];
        bmp[0] = b'B';
        bmp[1] = b'M';
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&200i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-100i32).to_le_bytes());
        assert_eq!(sniff(&bmp), Some(("image/bmp", 200, 100)), "a bitmap stored top-down has a negative height");
        let mut lossy = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        lossy.resize(26, 0);
        lossy.extend_from_slice(&[0x40, 0x01, 0xf0, 0x00]);
        assert_eq!(sniff(&lossy), Some(("image/webp", 320, 240)));
        let mut ext = b"RIFF\0\0\0\0WEBPVP8X".to_vec();
        ext.resize(24, 0);
        ext.extend_from_slice(&[0xff, 0x03, 0x00, 0xff, 0x01, 0x00]);
        assert_eq!(sniff(&ext), Some(("image/webp", 1024, 512)));
        let mut lossless = b"RIFF\0\0\0\0WEBPVP8L".to_vec();
        lossless.resize(20, 0);
        lossless.push(0x2f);
        lossless.extend_from_slice(&(99u32 | (49u32 << 14)).to_le_bytes());
        lossless.resize(30, 0);
        assert_eq!(sniff(&lossless), Some(("image/webp", 100, 50)));
    }

    #[test]
    fn something_that_is_not_a_picture_is_not_read_as_one() {
        assert_eq!(sniff(b"hello, this is text"), None);
        assert_eq!(sniff(&[]), None);
        assert_eq!(sniff(&[0xff, 0xd8, 0xff]), None, "a JPEG that stops before its frame has no size");
        assert!(read_image("notes.png", b"words".to_vec()).unwrap_err().contains("not a PNG"));
    }

    #[test]
    fn a_cameras_turn_marker_swaps_the_size_to_what_is_shown() {
        let upright = read_image("a.jpg", jpeg(4000, 3000, Some(1))).unwrap();
        assert_eq!((upright.width, upright.height), (4000, 3000));
        let on_its_side = read_image("b.jpg", jpeg(4000, 3000, Some(6))).unwrap();
        assert_eq!((on_its_side.width, on_its_side.height), (3000, 4000), "orientation 6 is a quarter turn");
        assert_eq!(exif_orientation(&jpeg(10, 10, Some(8))), Some(8));
        assert_eq!(exif_orientation(&jpeg(10, 10, None)), None);
    }

    #[test]
    fn a_name_that_disagrees_with_the_bytes_is_refused_and_the_unreadable_kinds_say_what_to_do() {
        assert!(read_image("photo.jpg", png(10, 10)).unwrap_err().contains("named as jpeg but its bytes are png"));
        assert!(image_kind_of_name("scan.tiff").unwrap_err().contains("Save scan.tiff as a PNG or JPEG"));
        assert_eq!(image_kind_of_name("a.PNG"), Ok(Some("image/png")));
        assert_eq!(image_kind_of_name("a.txt"), Ok(None));
    }

    #[test]
    fn limits_are_kept() {
        assert!(read_image("big.png", png(40_000, 10)).unwrap_err().contains("30000 pixels"));
        assert!(read_image("empty.png", Vec::new()).unwrap_err().contains("empty"));
        assert!(read_image("zero.png", png(0, 10)).unwrap_err().contains("no size"));
    }
}
