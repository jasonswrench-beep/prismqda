//! A recording with no captions (MP3, WAV, MP4, and the other kinds PrismQDA plays). It comes in as a document of its own whose text is a
//! description to be replaced by a better one, as a picture's is; the recording itself stays where it is and is linked to the document
//! afterwards (see `Project::link_media_as`). Nothing is decoded and nothing is turned into text: the recording is coded by stretches of
//! time.

use prismqda_engine::{default_recording_description, ImportBatch, ImportedSource, ImportedUnit, SourceType, UnitKind};
use serde_json::json;

use crate::options::ImportOptions;

/// A recording as a batch of one document. `fingerprint` is the quick fingerprint of the file (see `recording_fingerprint`), kept as the
/// document's hash so the same recording is not imported twice by mistake.
pub fn parse_recording(file_name: &str, kind: &str, size_bytes: u64, fingerprint: &str, opts: &ImportOptions) -> ImportBatch {
    let shown = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = shown.rsplit_once('.').map(|(s, _)| s).unwrap_or(shown);
    let text = default_recording_description(shown, kind, size_bytes);
    let len = text.chars().count() as i64;
    let settings = json!({ "format": "recording", "kind": kind, "file_name": shown, "size_bytes": size_bytes });
    ImportBatch {
        sources: vec![ImportedSource {
            name: opts.name.clone().unwrap_or_else(|| stem.to_string()),
            source_type: SourceType::Other,
            folder: opts.folder.clone(),
            text,
            runs: Vec::new(),
            units: vec![ImportedUnit::new(0, len, UnitKind::Paragraph)],
            default_unit_kind: UnitKind::Paragraph,
            unit_attributes: Vec::new(),
            original_file_hash: Some(format!("recording:{fingerprint}")),
            settings,
            warnings: Vec::new(),
        }],
        ..ImportBatch::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_recording_comes_in_as_a_described_document_named_for_its_file() {
        let batch = parse_recording("C:\\Recordings\\interview-4.mp3", "audio", 3 * 1024 * 1024, "abc123", &ImportOptions::default());
        let s = &batch.sources[0];
        assert_eq!(s.name, "interview-4");
        assert!(s.text.starts_with("Audio recording interview-4.mp3, 3.0 MB."), "{}", s.text);
        assert_eq!(s.settings["format"], "recording");
        assert_eq!(s.original_file_hash.as_deref(), Some("recording:abc123"));
        assert_eq!(s.units.len(), 1);
        assert!(batch.images.is_empty() && batch.pdfs.is_empty());
    }

    #[test]
    fn a_name_the_person_chose_is_used_and_a_video_is_called_a_video() {
        let opts = ImportOptions { name: Some("Focus group 2".to_string()), folder: "Audio".to_string(), ..ImportOptions::default() };
        let batch = parse_recording("fg2.mp4", "video", 300 * 1024, "f", &opts);
        let s = &batch.sources[0];
        assert_eq!((s.name.as_str(), s.folder.as_str()), ("Focus group 2", "Audio"));
        assert!(s.text.starts_with("Video fg2.mp4, 300 KB."), "{}", s.text);
    }
}
