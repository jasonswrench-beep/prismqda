//! Interview and focus-group transcripts, and caption files (specification section 5).
//!
//! A transcript is split into speaker turns. Each turn becomes a unit whose speaker is recorded as
//! a unit attribute, and turns by interviewers can be left out of coding. Speaker labels are
//! recognised conservatively: a label must appear at least twice (or be named as an interviewer
//! label), so a one-off "Date:" or "Note:" line is never mistaken for a speaker.

use std::collections::HashMap;

use prismqda_engine::{AppliesTo, AttributeSpec, ImportWarning, ImportedUnit, Level, SourceType, UnitKind};

use crate::builder::{ParsedText, TextBuilder};
use crate::error::{ImportError, Result};
use crate::options::ImportOptions;
use crate::segment;
use crate::text;
use crate::Format;

/// One line of the stored text that starts a speaker's turn.
struct Hit {
    key: String,
    label: String,
    timestamp: Option<String>,
    /// Char offset in the whole text where the spoken words start on this line.
    content_start: i64,
}

/// Splits a parsed document into speaker turns. The text itself is not changed; units are spans over it.
pub fn apply_turns(mut parsed: ParsedText, opts: &ImportOptions) -> ParsedText {
    parsed.source_type = SourceType::Transcript;
    parsed.unit_kind = UnitKind::SpeakerTurn;

    // Lines with their char offsets.
    let mut lines: Vec<(&str, i64)> = Vec::new();
    let mut offset = 0i64;
    for line in parsed.text.split('\n') {
        lines.push((line, offset));
        offset += line.chars().count() as i64 + 1;
    }

    let interviewer_keys: Vec<String> = opts.interviewer_labels.iter().map(|l| l.trim().to_lowercase()).collect();
    let candidates: Vec<(usize, Hit)> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, (line, start))| label_line(line, *start).map(|h| (i, h)))
        .filter(|(_, h)| !NOT_SPEAKERS.contains(&h.key.as_str()) || interviewer_keys.contains(&h.key))
        .collect();
    let keys: Vec<String> = candidates.iter().map(|(_, h)| h.key.clone()).collect();
    let flags = accepted_flags(&keys, &interviewer_keys);
    let hits: Vec<&(usize, Hit)> = candidates.iter().zip(flags).filter(|(_, ok)| *ok).map(|(c, _)| c).collect();

    if hits.len() < 2 && !(hits.len() == 1 && !interviewer_keys.is_empty()) {
        parsed.warnings.push(ImportWarning::new(
            "no_speaker_labels",
            "No speaker labels such as \"Interviewer:\" were found, so the text was split into paragraphs instead of turns.",
        ));
        parsed.units = Some(segment::paragraphs_as(&parsed.text, UnitKind::SpeakerTurn));
        return parsed;
    }

    let mut units: Vec<ImportedUnit> = Vec::new();
    let mut any_timestamp = false;
    for (n, (line_index, hit)) in hits.iter().map(|h| (h.0, &h.1)).enumerate() {
        let next_line = hits.get(n + 1).map(|h| h.0).unwrap_or(lines.len());
        // The turn's words: the rest of the labelled line, then the lines up to the next label.
        let mut start: Option<i64> = None;
        let mut end: Option<i64> = None;
        let first_content = chars_after(lines[line_index].0, lines[line_index].1, hit.content_start);
        if !first_content.trim().is_empty() {
            start = Some(hit.content_start);
            end = Some(trimmed_end(lines[line_index].0, lines[line_index].1));
        }
        for (line, line_start) in &lines[line_index + 1..next_line] {
            if line.trim().is_empty() {
                continue;
            }
            if start.is_none() {
                start = Some(line_start + leading_chars(line));
            }
            end = Some(trimmed_end(line, *line_start));
        }
        let (Some(s), Some(e)) = (start, end) else { continue };
        if e <= s {
            continue;
        }
        let mut unit = ImportedUnit::new(s, e, UnitKind::SpeakerTurn);
        unit.speaker = Some(hit.label.clone());
        unit.excluded = interviewer_keys.contains(&hit.key);
        unit.attrs.push(("speaker".to_string(), hit.label.clone()));
        if let Some(ts) = &hit.timestamp {
            unit.attrs.push(("timestamp".to_string(), ts.clone()));
            any_timestamp = true;
        }
        units.push(unit);
    }
    parsed.unit_attributes = vec![AttributeSpec::new("speaker", AppliesTo::Unit, Level::Nominal)];
    if any_timestamp {
        parsed.unit_attributes.push(AttributeSpec::new("timestamp", AppliesTo::Unit, Level::Text));
    }
    if units.is_empty() {
        parsed.units = Some(segment::paragraphs_as(&parsed.text, UnitKind::SpeakerTurn));
        parsed.unit_attributes.clear();
    } else {
        parsed.units = Some(units);
    }
    parsed
}

/// Words that start many lines in a transcript's header or margins but are never a speaker.
const NOT_SPEAKERS: &[&str] = &[
    "note", "notes", "date", "time", "re", "subject", "transcript", "summary", "topic", "location", "duration", "context",
    "setting", "warning", "example", "important", "interview", "recording", "title", "name", "age", "gender", "from", "to",
    "cc", "tip", "see", "source", "page", "line", "step",
];

/// Decides which candidate labels are speakers. A label seen twice is a speaker. Once one is
/// established, a label seen only once after it is a speaker too (the second person in a short
/// exchange), but one before it is a header line such as "Interview 4" and is not. Interviewer
/// labels the person named are always speakers.
fn accepted_flags(keys: &[String], interviewer_keys: &[String]) -> Vec<bool> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for k in keys {
        *counts.entry(k.as_str()).or_default() += 1;
    }
    let first_established = keys.iter().position(|k| counts[k.as_str()] >= 2);
    keys.iter()
        .enumerate()
        .map(|(i, k)| counts[k.as_str()] >= 2 || interviewer_keys.contains(k) || first_established.is_some_and(|p| i > p))
        .collect()
}

fn leading_chars(line: &str) -> i64 {
    (line.chars().count() - line.trim_start().chars().count()) as i64
}

fn trimmed_end(line: &str, line_start: i64) -> i64 {
    line_start + line.trim_end().chars().count() as i64
}

fn chars_after(line: &str, line_start: i64, abs: i64) -> String {
    line.chars().skip((abs - line_start).max(0) as usize).collect()
}

/// Reads one line as the start of a turn: `[00:01:23] Name: words`, `Name [00:01:23]: words`,
/// `00:01:23 Name: words`, `Name: words`, or `Name:` alone.
fn label_line(line: &str, line_start: i64) -> Option<Hit> {
    let lead = line.len() - line.trim_start().len();
    let mut rest = &line[lead..];
    let mut consumed = lead;
    let mut timestamp: Option<String> = None;

    // A timestamp before the name.
    if let Some((ts, len)) = leading_timestamp(rest) {
        timestamp = Some(ts);
        let after = &rest[len..];
        let skip = after.len() - after.trim_start_matches(|c: char| c.is_whitespace() || c == '-' || c == '\u{2013}').len();
        consumed += len + skip;
        rest = &after[skip..];
    }

    let colon = colon_outside_brackets(rest)?;
    let mut label_part = rest[..colon].trim_end();
    // A timestamp after the name, in brackets.
    if timestamp.is_none() && (label_part.ends_with(']') || label_part.ends_with(')')) {
        let open = if label_part.ends_with(']') { '[' } else { '(' };
        if let Some(o) = label_part.rfind(open) {
            let inner = &label_part[o + 1..label_part.len() - 1];
            if is_timestamp(inner.trim()) {
                timestamp = Some(inner.trim().to_string());
                label_part = label_part[..o].trim_end();
            }
        }
    }
    if !plausible_label(label_part) {
        return None;
    }
    let colon_len = rest[colon..].chars().next().map(char::len_utf8).unwrap_or(1);
    let after_colon = &rest[colon + colon_len..];
    let content_trim = after_colon.len() - after_colon.trim_start().len();
    let content_byte = consumed + colon + colon_len + content_trim;

    let label = label_part.split_whitespace().collect::<Vec<_>>().join(" ");
    Some(Hit {
        key: label.to_lowercase(),
        label,
        timestamp,
        content_start: line_start + line[..content_byte].chars().count() as i64,
    })
}

/// The first colon that is not inside brackets, so the colons of a timestamp like "[4:15]" are skipped.
fn colon_outside_brackets(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = (depth - 1).max(0),
            ':' | '\u{ff1a}' if depth == 0 => return Some(i),
            _ => {}
        }
    }
    None
}

fn plausible_label(label: &str) -> bool {
    let chars = label.chars().count();
    if chars == 0 || chars > 40 {
        return false;
    }
    let words: Vec<&str> = label.split_whitespace().collect();
    if words.len() > 4 {
        return false;
    }
    let first = label.chars().next().unwrap_or(' ');
    if !first.is_alphanumeric() {
        return false;
    }
    if label.chars().all(|c| c.is_ascii_digit()) || label.contains("http") || label.contains('/') {
        return false;
    }
    // A '.' is allowed only in a title such as "Dr. Smith".
    if label.contains('.') {
        let titled = ["Dr.", "Mr.", "Ms.", "Mrs.", "Prof."].iter().any(|t| label.starts_with(t));
        if !titled || label.matches('.').count() > 1 {
            return false;
        }
    }
    if words.len() >= 2 {
        // "Participant 3" and "Dr. Smith" are names; "and then he said" is not.
        return words.iter().all(|w| w.chars().next().is_some_and(|c| c.is_uppercase() || c.is_ascii_digit() || c == '('));
    }
    true
}

/// A timestamp at the start of the text, in brackets or bare. Returns the timestamp and how many bytes it used.
fn leading_timestamp(s: &str) -> Option<(String, usize)> {
    if let Some(open) = s.chars().next().filter(|c| *c == '[' || *c == '(') {
        let close = if open == '[' { ']' } else { ')' };
        let end = s.find(close)?;
        let inner = s[1..end].trim();
        return is_timestamp(inner).then(|| (inner.to_string(), end + 1));
    }
    let token_end = s.find(|c: char| c.is_whitespace()).unwrap_or(s.len());
    let token = &s[..token_end];
    let token = token.trim_end_matches([',', ';']);
    is_timestamp(token).then(|| (token.to_string(), token_end))
}

/// `m:ss`, `mm:ss`, `h:mm:ss`, or `hh:mm:ss`, with optional fractional seconds after a dot or comma.
fn is_timestamp(s: &str) -> bool {
    let (main, frac) = match s.split_once(['.', ',']) {
        Some((m, f)) => (m, Some(f)),
        None => (s, None),
    };
    if let Some(f) = frac {
        if f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let parts: Vec<&str> = main.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return false;
    }
    if !parts.iter().all(|p| !p.is_empty() && p.len() <= 2 && p.bytes().all(|b| b.is_ascii_digit())) {
        return false;
    }
    let n = |p: &str| p.parse::<u32>().unwrap_or(99);
    let last_two_ok = n(parts[parts.len() - 1]) < 60 && (parts.len() == 2 || n(parts[1]) < 60);
    last_two_ok && (parts.len() == 3 || parts[0].len() <= 2)
}

// ---- caption files ----

/// WebVTT and SubRip caption files. Consecutive cues by the same named speaker are joined into one
/// turn; cues with no speaker stay one unit each, because guessing where a thought ends would be
/// worse than leaving the cuts as the captioner made them.
pub fn parse_captions(bytes: &[u8], format: Format, opts: &ImportOptions) -> Result<ParsedText> {
    let (decoded, warning) = text::decode(bytes);
    let decoded = text::clean(&decoded);
    let mut cues = parse_cues(&decoded);
    if cues.is_empty() {
        return Err(ImportError::format(match format {
            Format::Vtt => "no captions were found; a WebVTT file has lines like \"00:00:01.000 --> 00:00:04.000\" followed by the caption",
            _ => "no captions were found; a SubRip file has lines like \"00:00:01,000 --> 00:00:04,000\" followed by the caption",
        }));
    }

    let interviewer_keys: Vec<String> = opts.interviewer_labels.iter().map(|l| l.trim().to_lowercase()).collect();
    // Plain "Name: words" prefixes count as speakers by the same rule as in a transcript.
    let mut candidates: Vec<(usize, Hit)> = Vec::new();
    for (i, cue) in cues.iter().enumerate() {
        if cue.speaker.is_none() {
            if let Some(h) = label_line(&cue.text, 0) {
                if h.timestamp.is_none() && (!NOT_SPEAKERS.contains(&h.key.as_str()) || interviewer_keys.contains(&h.key)) {
                    candidates.push((i, h));
                }
            }
        }
    }
    let keys: Vec<String> = candidates.iter().map(|(_, h)| h.key.clone()).collect();
    for ((i, h), ok) in candidates.into_iter().zip(accepted_flags(&keys, &interviewer_keys)) {
        if ok {
            cues[i].text = cues[i].text.chars().skip(h.content_start as usize).collect();
            cues[i].speaker = Some(h.label);
        }
    }

    let mut b = TextBuilder::new();
    if let Some(w) = warning {
        b.warn(w);
    }
    let mut units: Vec<ImportedUnit> = Vec::new();
    let mut i = 0;
    while i < cues.len() {
        let mut words = vec![cues[i].text.clone()];
        let mut j = i + 1;
        if cues[i].speaker.is_some() {
            while j < cues.len() && cues[j].speaker == cues[i].speaker {
                words.push(cues[j].text.clone());
                j += 1;
            }
        }
        b.paragraph_break();
        let (start, end) = b.push(&words.join(" "));
        let mut unit = ImportedUnit::new(start, end, UnitKind::SpeakerTurn);
        if let Some(speaker) = &cues[i].speaker {
            unit.speaker = Some(speaker.clone());
            unit.excluded = interviewer_keys.contains(&speaker.to_lowercase());
            unit.attrs.push(("speaker".to_string(), speaker.clone()));
        }
        unit.attrs.push(("timestamp".to_string(), cues[i].start.clone()));
        // Where in the recording the turn is spoken: from its first cue's start to its last cue's end.
        if let (Some(from), Some(to)) = (cues[i].start_ms, cues[j - 1].end_ms) {
            unit.time_ms = Some((from, to.max(from)));
        }
        units.push(unit);
        i = j;
    }
    let mut parsed = b.finish(SourceType::Transcript, UnitKind::SpeakerTurn);
    units.retain(|u| u.end > u.start);
    let any_speaker = units.iter().any(|u| u.speaker.is_some());
    parsed.unit_attributes = vec![AttributeSpec::new("timestamp", AppliesTo::Unit, Level::Text)];
    if any_speaker {
        parsed.unit_attributes.push(AttributeSpec::new("speaker", AppliesTo::Unit, Level::Nominal));
    } else {
        for u in units.iter_mut() {
            u.attrs.retain(|(n, _)| n != "speaker");
        }
    }
    parsed.units = Some(units);
    Ok(parsed)
}

struct Cue {
    start: String,
    text: String,
    speaker: Option<String>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
}

/// A caption time such as `00:01:02.500`, `01:02.500`, or `1:02,5`, in milliseconds. Fractions after a dot or comma are read as parts of a second.
pub(crate) fn timestamp_ms(s: &str) -> Option<i64> {
    let s = s.trim();
    let (main, frac) = match s.split_once(['.', ',']) {
        Some((m, f)) => (m, f),
        None => (s, ""),
    };
    if frac.len() > 3 || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ms: i64 = if frac.is_empty() { 0 } else { format!("{frac:0<3}").parse().ok()? };
    let parts: Vec<i64> = main.split(':').map(|p| p.parse::<i64>().ok()).collect::<Option<_>>()?;
    let seconds = match parts.as_slice() {
        [m, s] if *s < 60 => m * 60 + s,
        [h, m, s] if *m < 60 && *s < 60 => h * 3600 + m * 60 + s,
        _ => return None,
    };
    Some(seconds * 1000 + ms)
}

fn parse_cues(text: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let lines: Vec<&str> = block.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let Some(timing) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let (from, to) = lines[timing].split_once("-->").unwrap_or(("", ""));
        let start = from.trim().replace(',', ".");
        // The end time is the first word after the arrow (a WebVTT cue may go on with settings such as "align:start").
        let end_ms = to.split_whitespace().next().and_then(timestamp_ms);
        if !is_timestamp(start.as_str()) && !start.split(':').all(|p| !p.is_empty()) {
            continue;
        }
        let mut speaker: Option<String> = None;
        let mut pieces: Vec<String> = Vec::new();
        for line in &lines[timing + 1..] {
            let (clean, voice) = strip_tags(line);
            if speaker.is_none() {
                speaker = voice;
            }
            if !clean.trim().is_empty() {
                pieces.push(clean.trim().to_string());
            }
        }
        if pieces.is_empty() {
            continue;
        }
        let start_ms = timestamp_ms(&start);
        cues.push(Cue { start, text: pieces.join(" "), speaker, start_ms, end_ms });
    }
    cues
}

/// Removes caption markup (`<i>`, `<c.yellow>`, `<00:00:01.000>`) and returns the voice name from `<v Name>`, if any.
fn strip_tags(line: &str) -> (String, Option<String>) {
    let mut out = String::new();
    let mut voice: Option<String> = None;
    let mut rest = line;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('>') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let tag = &rest[open + 1..open + close];
        if let Some(v) = tag.strip_prefix("v ").or_else(|| tag.strip_prefix("v.").and_then(|t| t.split_once(' ').map(|x| x.1))) {
            if voice.is_none() {
                voice = Some(v.trim().to_string());
            }
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    // Entities used in caption files.
    let out = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ");
    (out, voice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(text: &str) -> ParsedText {
        let mut b = TextBuilder::new();
        for (i, block) in text.split("\n\n").enumerate() {
            if i > 0 {
                b.paragraph_break();
            }
            b.push(block);
        }
        b.finish(SourceType::Document, UnitKind::Sentence)
    }

    fn pieces(p: &ParsedText) -> Vec<(String, String)> {
        p.units
            .as_ref()
            .unwrap()
            .iter()
            .map(|u| {
                let words: String = p.text.chars().skip(u.start as usize).take((u.end - u.start) as usize).collect();
                (u.speaker.clone().unwrap_or_default(), words)
            })
            .collect()
    }

    const INTERVIEW: &str = "Interview 4\nDate: 2026-10-30\n\nInterviewer: Thanks for coming. Tell me about the first day.\n\nParticipant: I felt welcome\nfrom the first day.\nSomeone met me at the door.\n\nInterviewer: And the cost?\nParticipant: The cost was the hard part.";

    #[test]
    fn turns_follow_the_speaker_labels_and_continue_over_lines() {
        let opts = ImportOptions { interviewer_labels: vec!["interviewer".into()], ..Default::default() };
        let p = apply_turns(parsed(INTERVIEW), &opts);
        assert_eq!(p.source_type, SourceType::Transcript);
        assert_eq!(
            pieces(&p),
            [
                ("Interviewer".to_string(), "Thanks for coming. Tell me about the first day.".to_string()),
                ("Participant".to_string(), "I felt welcome\nfrom the first day.\nSomeone met me at the door.".to_string()),
                ("Interviewer".to_string(), "And the cost?".to_string()),
                ("Participant".to_string(), "The cost was the hard part.".to_string()),
            ]
        );
        let units = p.units.as_ref().unwrap();
        assert_eq!(units.iter().map(|u| u.excluded).collect::<Vec<_>>(), [true, false, true, false], "interviewer turns are excluded from coding");
        assert!(units.iter().all(|u| u.kind == UnitKind::SpeakerTurn));
        assert!(units[1].attrs.contains(&("speaker".to_string(), "Participant".to_string())));
        assert!(p.unit_attributes.iter().any(|a| a.name == "speaker"));
    }

    #[test]
    fn a_one_off_header_line_is_not_a_speaker() {
        let p = apply_turns(parsed(INTERVIEW), &ImportOptions::default());
        assert!(pieces(&p).iter().all(|(s, _)| s == "Interviewer" || s == "Participant"), "{:?}", pieces(&p));
        assert!(!p.units.as_ref().unwrap().iter().any(|u| u.excluded), "nothing is excluded unless asked");
    }

    #[test]
    fn timestamps_are_recognised_in_common_places() {
        let text = "[00:01:23] Ann: first words here\n\n(0:02:05) Bo: second words\n\n00:03:10 Ann: third words\n\nBo [4:15]: fourth words\n\nAnn (00:05:01.250): fifth words";
        let p = apply_turns(parsed(text), &ImportOptions::default());
        let got = pieces(&p);
        assert_eq!(got.len(), 5, "{got:?}");
        assert_eq!(got[0].1, "first words here");
        assert_eq!(got[3].1, "fourth words");
        let stamps: Vec<String> = p.units.as_ref().unwrap().iter().map(|u| u.attrs.iter().find(|(n, _)| n == "timestamp").unwrap().1.clone()).collect();
        assert_eq!(stamps, ["00:01:23", "0:02:05", "00:03:10", "4:15", "00:05:01.250"]);
        assert!(p.unit_attributes.iter().any(|a| a.name == "timestamp"));
    }

    #[test]
    fn a_label_alone_on_its_line_takes_the_lines_after_it() {
        let text = "Ann:\nHello there,\nhow are you?\n\nBo:\nFine, thanks.\n\nAnn:\nGood.";
        let got = pieces(&apply_turns(parsed(text), &ImportOptions::default()));
        assert_eq!(
            got,
            [
                ("Ann".to_string(), "Hello there,\nhow are you?".to_string()),
                ("Bo".to_string(), "Fine, thanks.".to_string()),
                ("Ann".to_string(), "Good.".to_string())
            ]
        );
    }

    #[test]
    fn question_and_answer_labels_group_ignoring_case() {
        let text = "Q: First?\nA: Yes.\nq: Second?\na: No.";
        let got = pieces(&apply_turns(parsed(text), &ImportOptions::default()));
        assert_eq!(got.len(), 4);
        assert_eq!(got[0].0, "Q");
        assert_eq!(got[2].0, "q", "the label is kept as typed, grouping ignores case");
    }

    #[test]
    fn ordinary_sentences_with_colons_are_not_labels() {
        let text = "Here is the thing: it rained all day.\n\nThe reason: nobody came.\n\nAs I said: it was cold.";
        let p = apply_turns(parsed(text), &ImportOptions::default());
        assert!(p.warnings.iter().any(|w| w.kind == "no_speaker_labels"));
        assert_eq!(p.units.as_ref().unwrap().len(), 3, "falls back to paragraphs");
        assert!(p.units.as_ref().unwrap().iter().all(|u| u.kind == UnitKind::SpeakerTurn));
    }

    #[test]
    fn titles_and_numbered_participants_are_valid_names() {
        let text = "Dr. Lee: Welcome.\nParticipant 3: Hello.\nDr. Lee: Please begin.\nParticipant 3: Okay.";
        let got = pieces(&apply_turns(parsed(text), &ImportOptions::default()));
        assert_eq!(got.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), ["Dr. Lee", "Participant 3", "Dr. Lee", "Participant 3"]);
    }

    #[test]
    fn offsets_count_scalar_values_in_other_scripts() {
        let text = "Ann: \u{4f60}\u{597d} \u{1F600} caf\u{e9}\nBo: \u{05e9}\u{05dc}\u{05d5}\u{05dd}\nAnn: ok";
        let got = pieces(&apply_turns(parsed(text), &ImportOptions::default()));
        assert_eq!(got[0].1, "\u{4f60}\u{597d} \u{1F600} caf\u{e9}");
        assert_eq!(got[1].1, "\u{05e9}\u{05dc}\u{05d5}\u{05dd}");
    }

    #[test]
    fn timestamp_shapes() {
        for ok in ["0:05", "12:34", "1:02:03", "00:00:01.000", "01:02:03,5", "9:59:59"] {
            assert!(is_timestamp(ok), "{ok}");
        }
        for bad in ["12", "1:2:3:4", "12:60", "1:60:00", "ab:cd", "10:30pm", "1:02:", ":30", "123:45", "00:00:01.0000", "1.5"] {
            assert!(!is_timestamp(bad), "{bad}");
        }
    }

    // ---- captions ----

    #[test]
    fn webvtt_voices_are_speakers_and_same_speaker_cues_join() {
        let vtt = "WEBVTT\n\nNOTE a comment\n\n1\n00:00:01.000 --> 00:00:03.000\n<v Ann>Hello,\n\n2\n00:00:03.000 --> 00:00:05.000\n<v Ann>how are <i>you</i>?\n\n3\n00:00:05.500 --> 00:00:07.000\n<v Bo>Fine &amp; well.\n";
        let p = parse_captions(vtt.as_bytes(), Format::Vtt, &ImportOptions::default()).unwrap();
        assert_eq!(p.source_type, SourceType::Transcript);
        assert_eq!(pieces(&p), [("Ann".to_string(), "Hello, how are you?".to_string()), ("Bo".to_string(), "Fine & well.".to_string())]);
        let units = p.units.as_ref().unwrap();
        assert!(units[0].attrs.contains(&("timestamp".to_string(), "00:00:01.000".to_string())));
        assert!(units[1].attrs.contains(&("timestamp".to_string(), "00:00:05.500".to_string())));
    }

    #[test]
    fn caption_times_are_read_in_milliseconds_in_the_shapes_captions_use() {
        assert_eq!(timestamp_ms("00:00:01.000"), Some(1000));
        assert_eq!(timestamp_ms("01:02,500"), Some(62_500));
        assert_eq!(timestamp_ms("1:02:03.5"), Some(3_723_500));
        assert_eq!(timestamp_ms("00:00:00.050"), Some(50));
        for bad in ["", "abc", "1", "00:61", "1:2:3:4", "00:00:01.5000", "00:00:01.x"] {
            assert_eq!(timestamp_ms(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_turn_made_from_cues_knows_where_in_the_recording_it_is_spoken() {
        let vtt = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:03.000 align:start\n<v Ann>Hello,\n\n2\n00:00:03.000 --> 00:00:05.250\n<v Ann>how are you?\n\n3\n00:00:05.500 --> 00:00:07.000\n<v Bo>Fine.\n";
        let p = parse_captions(vtt.as_bytes(), Format::Vtt, &ImportOptions::default()).unwrap();
        let units = p.units.as_ref().unwrap();
        assert_eq!(units.len(), 2, "Ann's two cues are one turn");
        assert_eq!(units[0].time_ms, Some((1000, 5250)), "from the first cue's start to the last cue's end");
        assert_eq!(units[1].time_ms, Some((5500, 7000)));
    }

    #[test]
    fn subrip_without_speakers_keeps_one_unit_per_cue() {
        let srt = "1\n00:00:01,000 --> 00:00:02,500\nFirst line\nsecond line\n\n2\n00:00:03,000 --> 00:00:04,000\nNext cue\n";
        let p = parse_captions(srt.as_bytes(), Format::Srt, &ImportOptions::default()).unwrap();
        assert_eq!(pieces(&p), [(String::new(), "First line second line".to_string()), (String::new(), "Next cue".to_string())]);
        assert!(!p.unit_attributes.iter().any(|a| a.name == "speaker"));
        assert!(p.units.as_ref().unwrap()[0].attrs.contains(&("timestamp".to_string(), "00:00:01.000".to_string())));
    }

    #[test]
    fn captions_with_name_prefixes_find_speakers_and_exclusions() {
        let srt = "1\n00:00:01,000 --> 00:00:02,000\nHOST: Welcome.\n\n2\n00:00:02,000 --> 00:00:03,000\nGUEST: Thanks.\n\n3\n00:00:03,000 --> 00:00:04,000\nHOST: First question.\n";
        let opts = ImportOptions { interviewer_labels: vec!["host".into()], ..Default::default() };
        let p = parse_captions(srt.as_bytes(), Format::Srt, &opts).unwrap();
        assert_eq!(pieces(&p).iter().map(|x| x.1.as_str()).collect::<Vec<_>>(), ["Welcome.", "Thanks.", "First question."]);
        assert_eq!(p.units.as_ref().unwrap().iter().map(|u| u.excluded).collect::<Vec<_>>(), [true, false, true]);
    }

    #[test]
    fn a_file_with_no_captions_is_explained() {
        let err = parse_captions(b"WEBVTT\n\nnothing here", Format::Vtt, &ImportOptions::default()).unwrap_err();
        assert!(err.to_string().contains("no captions"));
        assert!(parse_captions(b"", Format::Srt, &ImportOptions::default()).is_err());
    }

    #[test]
    fn caption_markup_is_stripped() {
        assert_eq!(strip_tags("<c.yellow>hi</c> <00:00:01.000>there <v Ann>x").0, "hi there x");
        assert_eq!(strip_tags("a < b").0, "a < b", "a lone angle bracket survives");
        assert_eq!(strip_tags("<v.loud Bo>words").1.as_deref(), Some("Bo"));
    }
}
