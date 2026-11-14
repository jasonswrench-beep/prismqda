//! Pictures (PNG, JPEG, GIF, WebP, BMP). A picture comes in as a document whose text is a description to be replaced by a better one,
//! with the picture's file kept whole beside it. Nothing is decoded: only the first bytes are read, to learn the kind and the size.

use prismqda_engine::{
    default_description, picture_details, read_image, AppliesTo, AttributeSpec, CaseMember, ImageAttachment, ImportBatch, ImportedCase, ImportedSource, ImportedUnit, Level, SourceType, UnitKind,
};
use serde_json::json;

use crate::error::{ImportError, Result};
use crate::options::ImportOptions;

/// A picture file as a batch of one document.
pub fn parse_image(file_name: &str, bytes: &[u8], opts: &ImportOptions) -> Result<ImportBatch> {
    let shown = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let image = read_image(shown, bytes.to_vec()).map_err(ImportError::format)?;
    let stem = shown.rsplit_once('.').map(|(s, _)| s).unwrap_or(shown);
    let text = default_description(shown, image.width, image.height);
    let len = text.chars().count() as i64;
    let settings = json!({ "format": "image", "mime": image.mime, "width": image.width, "height": image.height, "size_bytes": bytes.len(), "file_name": shown });
    let name = opts.name.clone().unwrap_or_else(|| stem.to_string());
    // The camera's facts the person asked for become the variables of a case made for this picture. A picture that does not say
    // a thing leaves that variable out, and a picture that says none of what was asked for makes no case.
    let (mut cases, mut case_attributes) = (Vec::new(), Vec::new());
    let wanted = opts.picture_variables;
    if wanted.any() {
        let details = picture_details(bytes);
        let mut attrs: Vec<(String, String)> = Vec::new();
        let mut add = |variable: &str, level: Level, value: Option<String>| {
            if let Some(value) = value {
                case_attributes.push(AttributeSpec::new(variable, AppliesTo::Case, level));
                attrs.push((variable.to_string(), value));
            }
        };
        if wanted.when {
            add("Date taken", Level::Date, details.date().map(str::to_string));
            add("Month taken", Level::Nominal, details.month().map(str::to_string));
            add("Year taken", Level::Nominal, details.year().map(str::to_string));
        }
        if wanted.camera {
            add("Camera", Level::Nominal, details.camera.clone());
        }
        if wanted.place && details.has_place() {
            add("Latitude", Level::Interval, details.latitude.map(|v| format!("{v:.6}")));
            add("Longitude", Level::Interval, details.longitude.map(|v| format!("{v:.6}")));
        }
        if !attrs.is_empty() {
            cases.push(ImportedCase { name: name.clone(), attrs, members: vec![CaseMember { source: 0, unit: None }] });
        }
    }
    Ok(ImportBatch {
        cases,
        case_attributes,
        sources: vec![ImportedSource {
            name,
            source_type: SourceType::Other,
            folder: opts.folder.clone(),
            text,
            runs: Vec::new(),
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: Vec::new(),
            original_file_hash: opts.file_hash.clone(),
            settings,
            warnings: Vec::new(),
        }],
        images: vec![ImageAttachment { source: 0, image }],
        pdfs: Vec::new(),
    })
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

    #[test]
    fn a_picture_becomes_a_document_with_a_description_and_its_file() {
        let batch = parse_image("C:\\photos\\Front Door.PNG", &png(640, 480), &ImportOptions::default()).unwrap();
        assert_eq!(batch.sources.len(), 1);
        let s = &batch.sources[0];
        assert_eq!(s.name, "Front Door");
        assert!(s.text.contains("640 by 480 pixels") && s.text.contains("Front Door.PNG"));
        assert_eq!(s.units.len(), 1);
        assert_eq!(batch.images[0].image.bytes, png(640, 480));
        assert_eq!(s.settings["format"], "image");
    }

    /// A JPEG whose camera block says only: Nikon, 2023-12-25 08:30:00.
    fn photograph() -> Vec<u8> {
        let mut t = b"MM\0\x2a".to_vec();
        t.extend_from_slice(&8u32.to_be_bytes());
        t.extend_from_slice(&2u16.to_be_bytes());
        for (tag, kind, count, offset) in [(0x010fu16, 2u16, 6u32, 38u32), (0x0132, 2, 20, 44)] {
            t.extend_from_slice(&tag.to_be_bytes());
            t.extend_from_slice(&kind.to_be_bytes());
            t.extend_from_slice(&count.to_be_bytes());
            t.extend_from_slice(&offset.to_be_bytes());
        }
        t.extend_from_slice(&0u32.to_be_bytes());
        t.extend_from_slice(b"Nikon\0");
        t.extend_from_slice(b"2023:12:25 08:30:00\0");
        let mut b = vec![0xff, 0xd8, 0xff, 0xe1];
        b.extend_from_slice(&((2 + 6 + t.len()) as u16).to_be_bytes());
        b.extend_from_slice(b"Exif\0\0");
        b.extend_from_slice(&t);
        b.extend_from_slice(&[0xff, 0xc0, 0, 17, 8, 0, 100, 0, 200, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        b
    }

    #[test]
    fn the_cameras_facts_become_variables_of_a_case_only_when_asked_for() {
        let none = parse_image("Door.jpg", &photograph(), &ImportOptions::default()).unwrap();
        assert!(none.cases.is_empty() && none.case_attributes.is_empty(), "nothing is kept unless asked for");

        let when = ImportOptions { picture_variables: crate::PictureVariables { when: true, ..Default::default() }, ..ImportOptions::default() };
        let b = parse_image("Door.jpg", &photograph(), &when).unwrap();
        assert_eq!(b.cases.len(), 1);
        assert_eq!(b.cases[0].name, "Door");
        assert_eq!(b.cases[0].attrs, vec![("Date taken".to_string(), "2023-12-25".to_string()), ("Month taken".to_string(), "2023-12".to_string()), ("Year taken".to_string(), "2023".to_string())]);
        assert_eq!((b.cases[0].members.len(), b.cases[0].members[0].source), (1, 0));
        let names: Vec<&str> = b.case_attributes.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Date taken", "Month taken", "Year taken"], "each variable is defined once");

        let all = ImportOptions { picture_variables: crate::PictureVariables { when: true, camera: true, place: true }, ..ImportOptions::default() };
        let b = parse_image("Door.jpg", &photograph(), &all).unwrap();
        assert!(b.cases[0].attrs.contains(&("Camera".to_string(), "Nikon".to_string())));
        assert!(!b.cases[0].attrs.iter().any(|(n, _)| n == "Latitude"), "this picture says nothing of where, so no place is made up");
    }

    #[test]
    fn a_picture_that_says_nothing_asked_for_makes_no_case() {
        let all = ImportOptions { picture_variables: crate::PictureVariables { when: true, camera: true, place: true }, ..ImportOptions::default() };
        let b = parse_image("Plain.png", &png(10, 10), &all).unwrap();
        assert!(b.cases.is_empty() && b.case_attributes.is_empty());
        let camera_only = ImportOptions { picture_variables: crate::PictureVariables { camera: true, ..Default::default() }, ..ImportOptions::default() };
        let b = parse_image("Door.jpg", &photograph(), &camera_only).unwrap();
        assert_eq!(b.cases[0].attrs, vec![("Camera".to_string(), "Nikon".to_string())], "only what was asked for");
    }

    #[test]
    fn a_file_that_is_not_the_picture_its_name_says_is_refused_in_words() {
        let e = parse_image("notes.png", b"just words", &ImportOptions::default()).unwrap_err().to_string();
        assert!(e.contains("not a PNG"), "{e}");
    }
}
