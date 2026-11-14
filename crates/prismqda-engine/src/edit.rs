//! Editing a source's text after coding has started (specification 6.1).
//!
//! The rule is simple enough to state and to test: **a span keeps the characters that survive the
//! edit.** Its new start is where the first surviving character went and its new end is just after
//! where the last one went, so text inserted inside a span becomes part of it and text inserted at
//! its edges does not. A word that was only corrected ("teh" to "the", "color" to "colour": most of
//! its letters survive) counts as surviving whole, so a coding follows a corrected word. A span with
//! no surviving character is removed, and the removal is written to the audit trail with everything
//! about it, so nothing is ever dropped silently. Units, format runs, anchor examples, and waiting
//! proposals follow the same rule.
//!
//! Lines are matched before words, and words before letters. Comparing letter by letter would pair
//! the "t" of a deleted "two" with the "t" of the "three" that follows it, and a coding would slide
//! onto the wrong word. Among alignments, the one that keeps the most matching text wins.

use std::time::{Duration, Instant};

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};
use similar::{capture_diff_slices_deadline, Algorithm, DiffOp};
use unicode_normalization::UnicodeNormalization;

use crate::deid::Edit;
use crate::event;
use crate::import::ImportedSource;
use crate::project::Project;
use crate::{Error, Result};

/// A stretch of the old text that continues into the new text. When `exact`, the two stretches are
/// identical and positions inside them map one to one. Otherwise it is a corrected word: the old
/// word became the new word, and a span that touches it takes the whole new word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Run {
    pub old: i64,
    pub old_len: i64,
    pub new: i64,
    pub new_len: i64,
    pub exact: bool,
}

impl Run {
    fn exact(old: usize, new: usize, len: usize) -> Run {
        Run { old: old as i64, old_len: len as i64, new: new as i64, new_len: len as i64, exact: true }
    }
}

/// Languages written without spaces get one token per character; everything else is split into words.
fn is_unspaced_script(c: char) -> bool {
    matches!(c as u32, 0x0E00..=0x0E7F | 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FFFF)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() && !is_unspaced_script(c)
}

/// Splits text into words and single other characters, as (start, end) character positions.
fn word_tokens(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        if is_word_char(chars[i]) {
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
        } else {
            i += 1;
        }
        out.push((start, i));
    }
    out
}

/// Splits text into lines, each keeping its line break.
fn line_tokens(chars: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, c) in chars.iter().enumerate() {
        if *c == '\n' {
            out.push((start, i + 1));
            start = i + 1;
        }
    }
    if start < chars.len() {
        out.push((start, chars.len()));
    }
    out
}

/// The share of the longer text's letters that also appear, in order, in the other.
fn similarity(a: &[char], b: &[char], deadline: Instant) -> f64 {
    const LIMIT: usize = 20_000;
    if a.is_empty() || b.is_empty() || a.len() > LIMIT || b.len() > LIMIT {
        return 0.0;
    }
    let common: usize = capture_diff_slices_deadline(Algorithm::Myers, a, b, Some(deadline))
        .into_iter()
        .map(|op| if let DiffOp::Equal { len, .. } = op { len } else { 0 })
        .sum();
    common as f64 / a.len().max(b.len()) as f64
}

/// What lining up two lists of tokens found: stretches that match, and stretches that were replaced.
enum Align {
    Equal { a: usize, b: usize, len: usize },
    Replace { a: usize, a_len: usize, b: usize, b_len: usize },
}

/// Lines two token lists up. Tokens at the beginning and end that match are taken off first. In the
/// middle the alignment keeps the most matching text (a long matching line beats a lone blank line),
/// and when several alignments keep the same amount, the earliest match wins.
fn align(a: &[&[char]], b: &[&[char]], deadline: Instant) -> Vec<Align> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let max_suffix = a.len().min(b.len()) - prefix;
    let suffix = a.iter().rev().zip(b.iter().rev()).take(max_suffix).take_while(|(x, y)| x == y).count();
    let mut ops = Vec::new();
    if prefix > 0 {
        ops.push(Align::Equal { a: 0, b: 0, len: prefix });
    }
    let (ma, mb) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    if !ma.is_empty() && !mb.is_empty() {
        let mut inner = if ma.len() * mb.len() <= 4_000_000 { align_heaviest(ma, mb) } else { align_patience(ma, mb, deadline) };
        for op in inner.drain(..) {
            ops.push(match op {
                Align::Equal { a, b, len } => Align::Equal { a: a + prefix, b: b + prefix, len },
                Align::Replace { a, a_len, b, b_len } => Align::Replace { a: a + prefix, a_len, b: b + prefix, b_len },
            });
        }
    }
    if suffix > 0 {
        ops.push(Align::Equal { a: a.len() - suffix, b: b.len() - suffix, len: suffix });
    }
    ops
}

/// The alignment that keeps the most matching characters, found by dynamic programming.
fn align_heaviest(a: &[&[char]], b: &[&[char]]) -> Vec<Align> {
    let (n, m) = (a.len(), b.len());
    let w = |i: usize| a[i].len() as u32;
    let width = m + 1;
    let mut dp = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let mut best = dp[(i + 1) * width + j].max(dp[i * width + j + 1]);
            if a[i] == b[j] {
                best = best.max(w(i) + dp[(i + 1) * width + j + 1]);
            }
            dp[i * width + j] = best;
        }
    }
    let mut matches: Vec<(usize, usize)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[i] == b[j] && dp[i * width + j] == w(i) + dp[(i + 1) * width + j + 1] {
            matches.push((i, j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * width + j] >= dp[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    matches_to_ops(&matches, n, m)
}

/// A cheaper alignment for very large middles, from the diff library.
fn align_patience(a: &[&[char]], b: &[&[char]], deadline: Instant) -> Vec<Align> {
    let mut ops = Vec::new();
    for op in capture_diff_slices_deadline(Algorithm::Patience, a, b, Some(deadline)) {
        match op {
            DiffOp::Equal { old_index, new_index, len } => ops.push(Align::Equal { a: old_index, b: new_index, len }),
            DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                ops.push(Align::Replace { a: old_index, a_len: old_len, b: new_index, b_len: new_len })
            }
            DiffOp::Delete { .. } | DiffOp::Insert { .. } => {}
        }
    }
    ops
}

/// Turns matched pairs into runs of equal tokens and the replaced stretches between them.
fn matches_to_ops(matches: &[(usize, usize)], n: usize, m: usize) -> Vec<Align> {
    let mut ops = Vec::new();
    let (mut ai, mut bi) = (0usize, 0usize);
    let mut k = 0;
    while k < matches.len() {
        let (ma, mb) = matches[k];
        if ma > ai && mb > bi {
            ops.push(Align::Replace { a: ai, a_len: ma - ai, b: bi, b_len: mb - bi });
        }
        let mut len = 1;
        while k + len < matches.len() && matches[k + len] == (ma + len, mb + len) {
            len += 1;
        }
        ops.push(Align::Equal { a: ma, b: mb, len });
        ai = ma + len;
        bi = mb + len;
        k += len;
    }
    if n > ai && m > bi {
        ops.push(Align::Replace { a: ai, a_len: n - ai, b: bi, b_len: m - bi });
    }
    ops
}

/// Lines first, then words inside lines that changed, then (for a single changed word) whether it
/// was only corrected. Each level lines up what is the same and recurses into what is not.
fn diff_region(oc: &[char], nc: &[char], ob: usize, nb: usize, by_lines: bool, until: Instant, out: &mut Vec<Run>) {
    let ot = if by_lines { line_tokens(oc) } else { word_tokens(oc) };
    let nt = if by_lines { line_tokens(nc) } else { word_tokens(nc) };
    let ow: Vec<&[char]> = ot.iter().map(|&(s, e)| &oc[s..e]).collect();
    let nw: Vec<&[char]> = nt.iter().map(|&(s, e)| &nc[s..e]).collect();
    for op in align(&ow, &nw, until) {
        match op {
            Align::Equal { a, b, len } => {
                let (os, oe) = (ot[a].0, ot[a + len - 1].1);
                out.push(Run::exact(ob + os, nb + nt[b].0, oe - os));
            }
            Align::Replace { a, a_len, b, b_len } => {
                let (os, oe) = (ot[a].0, ot[a + a_len - 1].1);
                let (ns, ne) = (nt[b].0, nt[b + b_len - 1].1);
                if by_lines {
                    diff_region(&oc[os..oe], &nc[ns..ne], ob + os, nb + ns, false, until, out);
                } else if similarity(&oc[os..oe], &nc[ns..ne], until) >= 0.5 {
                    out.push(Run {
                        old: (ob + os) as i64,
                        old_len: (oe - os) as i64,
                        new: (nb + ns) as i64,
                        new_len: (ne - ns) as i64,
                        exact: false,
                    });
                }
            }
        }
    }
}

/// Finds what continues from the old text into the new.
pub(crate) fn surviving_runs(old: &str, new: &str, deadline: Duration) -> Vec<Run> {
    let until = Instant::now() + deadline;
    let oc: Vec<char> = old.chars().collect();
    let nc: Vec<char> = new.chars().collect();
    let mut runs = Vec::new();
    diff_region(&oc, &nc, 0, 0, true, until, &mut runs);
    runs
}

/// Where the span `[start, end)` of the old text is in the new text, or `None` if none of it survived.
pub(crate) fn remap_span(runs: &[Run], start: i64, end: i64) -> Option<(i64, i64)> {
    // The first run that ends after the span starts, and the last run that starts before it ends.
    let first = runs.partition_point(|r| r.old + r.old_len <= start);
    let last = runs.partition_point(|r| r.old < end).checked_sub(1)?;
    if first >= runs.len() || first > last {
        return None;
    }
    let (a, b) = (&runs[first], &runs[last]);
    let new_start = if a.exact { a.new + (start.max(a.old) - a.old) } else { a.new };
    let new_end = if b.exact { b.new + (end.min(b.old + b.old_len) - b.old) } else { b.new + b.new_len };
    (new_end > new_start).then_some((new_start, new_end))
}

/// The runs for a text in which known stretches were replaced and nothing else changed (pseudonymizing): the text between the
/// replacements continues exactly, and each replaced stretch continues as the stretch that took its place, so a coding that touches a
/// name follows the name's replacement instead of being lost.
pub(crate) fn runs_from_edits(edits: &[Edit], old_chars: usize) -> Vec<Run> {
    let mut runs = Vec::new();
    let (mut old, mut new) = (0usize, 0usize);
    for e in edits {
        if e.old_start > old {
            runs.push(Run::exact(old, new, e.old_start - old));
        }
        runs.push(Run { old: e.old_start as i64, old_len: (e.old_end - e.old_start) as i64, new: e.new_start as i64, new_len: (e.new_end - e.new_start) as i64, exact: false });
        old = e.old_end;
        new = e.new_end;
    }
    if old < old_chars {
        runs.push(Run::exact(old, new, old_chars - old));
    }
    runs
}

/// What an edit did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EditOutcome {
    pub text_revision: i64,
    pub characters: i64,
    pub codings_moved: usize,
    pub codings_removed: usize,
    pub units_moved: usize,
    pub units_removed: usize,
    pub proposals_rejected: usize,
    /// Units of a revised file that the document had no unit for, added beside the ones it kept (always 0 for a plain edit).
    pub units_added: usize,
}

impl Project {
    /// Replaces a source's text and carries every coding, unit, and format run across the change.
    pub fn replace_source_text(&mut self, source_id: i64, new_text: &str) -> Result<EditOutcome> {
        self.replace_text(source_id, new_text, None)
    }

    /// Takes a revised file as the new version of a document. The document stays the same document, with its codings, memos,
    /// case, and attributes; its text becomes the file's text, and the codings follow the words that survive, exactly as in a
    /// text edit. The file's own formatting replaces the old, and units the revised text has that the document had none for
    /// (a new speaker turn) are added. A coding whose words are not in the revised text is removed, and the audit trail keeps it.
    pub fn revise_source(&mut self, source_id: i64, file: &ImportedSource) -> Result<EditOutcome> {
        self.replace_text(source_id, &file.text, Some(file))
    }

    fn replace_text(&mut self, source_id: i64, new_text: &str, file: Option<&ImportedSource>) -> Result<EditOutcome> {
        self.ensure_writable()?;
        if let Some(run) = self.reliability_lock(source_id)? {
            return Err(Error::Invalid(format!(
                "this source has units in reliability run {run}, which has not been archived, so its text cannot change; archive the run first"
            )));
        }
        if new_text.contains('\0') {
            return Err(Error::Invalid("the text contains a NUL character, which cannot be stored".to_string()));
        }
        let new_text: String = new_text.nfc().collect();
        let new_len = new_text.chars().count() as i64;
        if new_len == 0 {
            return Err(Error::Invalid("the source would have no text; remove the source instead".to_string()));
        }

        let tx = self.conn.transaction()?;
        let outcome = replace_text_in(&tx, &self.actor, &self.session, source_id, &new_text, file, None, true)?;
        tx.commit()?;
        Ok(outcome)
    }
}

/// Replaces a source's text inside a transaction that someone else opened and will commit. `known_runs` says what continues from the old text
/// into the new when the caller already knows (a list of replaced names); otherwise the two texts are compared. With `log` false no
/// audit event is written, for a caller that writes one event for many documents.
#[allow(clippy::too_many_arguments)]
pub(crate) fn replace_text_in(
    tx: &Transaction,
    actor: &str,
    session: &str,
    source_id: i64,
    new_text: &str,
    file: Option<&ImportedSource>,
    known_runs: Option<Vec<Run>>,
    log: bool,
) -> Result<EditOutcome> {
    {
        let new_text: String = new_text.nfc().collect();
        let new_text = new_text.as_str();
        let new_len = new_text.chars().count() as i64;
        let (old_text, revision, runs_json): (String, i64, String) = tx
            .query_row("SELECT text, text_revision, formatting_runs_json FROM source WHERE id = ?1", [source_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()?
            .ok_or(Error::NotFound { what: "source", id: source_id })?;
        if old_text == new_text && file.is_none() {
            return Ok(EditOutcome {
                text_revision: revision,
                characters: new_len,
                codings_moved: 0,
                codings_removed: 0,
                units_moved: 0,
                units_removed: 0,
                proposals_rejected: 0,
                units_added: 0,
            });
        }
        let runs = match known_runs {
            Some(known) => known,
            None => surviving_runs(old_text.as_str(), new_text, Duration::from_secs(5)),
        };

        // Codings.
        struct Row {
            id: i64,
            start: i64,
            end: i64,
            code_id: i64,
            coder_id: i64,
            rationale: String,
            rating: Option<i64>,
        }
        let rows: Vec<Row> = {
            let mut stmt = tx.prepare("SELECT id, start_offset, end_offset, code_id, coder_id, rationale, rating FROM coding WHERE source_id = ?1")?;
            let mapped = stmt.query_map([source_id], |r| {
                Ok(Row { id: r.get(0)?, start: r.get(1)?, end: r.get(2)?, code_id: r.get(3)?, coder_id: r.get(4)?, rationale: r.get(5)?, rating: r.get(6)? })
            })?;
            mapped.collect::<std::result::Result<_, _>>()?
        };
        let (mut moved, mut removed) = (0usize, Vec::new());
        {
            let mut update = tx.prepare_cached("UPDATE coding SET start_offset = ?1, end_offset = ?2 WHERE id = ?3")?;
            for row in &rows {
                match remap_span(&runs, row.start, row.end) {
                    Some((s, e)) if (s, e) == (row.start, row.end) => {}
                    Some((s, e)) => {
                        update.execute(params![s, e, row.id])?;
                        moved += 1;
                    }
                    None => removed.push(json!({
                        "id": row.id, "start": row.start, "end": row.end, "code_id": row.code_id,
                        "coder_id": row.coder_id, "rationale": row.rationale, "rating": row.rating,
                        "text": old_text.chars().skip(row.start as usize).take((row.end - row.start) as usize).collect::<String>(),
                    })),
                }
            }
        }
        for r in &removed {
            let id = r["id"].as_i64().expect("id");
            tx.execute("DELETE FROM code_anchor WHERE coding_id = ?1", [id])?;
            tx.execute("DELETE FROM coding WHERE id = ?1", [id])?;
            // Its links go with it (a revision is not undone one coding at a time).
            crate::passage_links::take_links(tx, id)?;
        }

        // Units.
        let unit_rows: Vec<(i64, i64, i64)> = {
            let mut stmt = tx.prepare("SELECT id, start_offset, end_offset FROM unit WHERE source_id = ?1")?;
            let mapped = stmt.query_map([source_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            mapped.collect::<std::result::Result<_, _>>()?
        };
        let (mut units_moved, mut gone_units) = (0usize, Vec::new());
        for (id, s, e) in &unit_rows {
            match remap_span(&runs, *s, *e) {
                Some((ns, ne)) if (ns, ne) == (*s, *e) => {}
                Some((ns, ne)) => {
                    tx.execute("UPDATE unit SET start_offset = ?1, end_offset = ?2 WHERE id = ?3", params![ns, ne, id])?;
                    units_moved += 1;
                }
                None => gone_units.push((*id, *s, *e)),
            }
        }
        for (id, ..) in &gone_units {
            // Replies to a removed unit become top-level; its codings are already gone because none of its text survived.
            tx.execute("UPDATE unit SET parent_unit_id = NULL WHERE parent_unit_id = ?1", [id])?;
            tx.execute("DELETE FROM case_member WHERE unit_id = ?1", [id])?;
            tx.execute("DELETE FROM attribute_value WHERE target_id = ?1 AND attribute_id IN (SELECT id FROM attribute WHERE applies_to = 'unit')", [id])?;
            tx.execute("UPDATE coding SET unit_id = NULL WHERE unit_id = ?1", [id])?;
            tx.execute("DELETE FROM unit WHERE id = ?1", [id])?;
        }

        // Format runs.
        let mut format_runs: Vec<Value> = serde_json::from_str(&runs_json)?;
        format_runs.retain_mut(|r| {
            let (Some(s), Some(e)) = (r["start"].as_i64(), r["end"].as_i64()) else { return false };
            match remap_span(&runs, s, e) {
                Some((ns, ne)) => {
                    r["start"] = json!(ns);
                    r["end"] = json!(ne);
                    true
                }
                None => false,
            }
        });

        // A revised file brings its own formatting (new headings, for example), which replaces the old.
        if let Some(f) = file {
            format_runs = serde_json::to_value(&f.runs)?.as_array().cloned().unwrap_or_default();
        }

        // Suggestions waiting for review point into the old text, so they are turned down, not guessed at.
        let proposals_rejected = tx.execute(
            "UPDATE proposal SET status = 'rejected', decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), decided_by = 'text edit'
             WHERE status = 'pending' AND json_extract(payload_json, '$.source_id') = ?1",
            [source_id],
        )?;

        let new_revision = revision + 1;
        tx.execute(
            "UPDATE source SET text = ?1, char_count = ?2, text_revision = ?3, formatting_runs_json = ?4 WHERE id = ?5",
            params![new_text, new_len, new_revision, Value::Array(format_runs).to_string(), source_id],
        )?;

        // Units the revised text has and the document has none for: added beside the units that were kept (a unit that
        // already overlaps one of them is never added a second time, so no coding is ever split between two).
        let mut units_added = 0usize;
        if let Some(f) = file {
            let mut covered: Vec<(i64, i64)> = {
                let mut stmt = tx.prepare("SELECT start_offset, end_offset FROM unit WHERE source_id = ?1")?;
                let mapped = stmt.query_map([source_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
                mapped.collect::<std::result::Result<_, _>>()?
            };
            let mut wanted: Vec<&crate::import::ImportedUnit> = f.units.iter().filter(|u| u.start >= 0 && u.end > u.start && u.end <= new_len).collect();
            wanted.sort_by_key(|u| (u.start, u.end));
            for u in wanted {
                if covered.iter().any(|(s, e)| *s < u.end && u.start < *e) {
                    continue;
                }
                tx.execute(
                    "INSERT INTO unit (source_id, kind, start_offset, end_offset, speaker, parent_unit_id, excluded, time_start_ms, time_end_ms) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8)",
                    params![source_id, u.kind.as_str(), u.start, u.end, u.speaker, u.excluded as i64, u.time_ms.map(|t| t.0), u.time_ms.map(|t| t.1)],
                )?;
                covered.push((u.start, u.end));
                units_added += 1;
            }
            let settings = if f.settings.is_null() { "{}".to_string() } else { f.settings.to_string() };
            tx.execute(
                "UPDATE source SET original_file_hash = ?1, import_settings_json = ?2, import_report_json = ?3 WHERE id = ?4",
                params![f.original_file_hash, settings, serde_json::to_string(&f.warnings)?, source_id],
            )?;
        }

        let mut after = json!({
            "text_revision": new_revision,
            "characters": new_len,
            "codings_moved": moved,
            "codings_removed": removed.len(),
            "units_moved": units_moved,
            "units_removed": gone_units.len(),
            "proposals_rejected": proposals_rejected,
        });
        if let Some(f) = file {
            after["units_added"] = json!(units_added);
            after["from_file"] = json!(f.name);
            after["original_file_hash"] = json!(f.original_file_hash);
        }
        if log {
            event::insert(
                tx,
                    actor,
                    session,
                if file.is_some() { "source.revise" } else { "source.edit" },
                Some(("source", source_id)),
                Some(&json!({
                    "text_revision": revision,
                    "characters": old_text.chars().count(),
                    "removed_codings": removed,
                    "removed_units": gone_units.iter().map(|(id, s, e)| json!({ "id": id, "start": s, "end": e })).collect::<Vec<_>>(),
                })),
                Some(&after),
            )?;
        }
        Ok(EditOutcome {
            text_revision: new_revision,
            characters: new_len,
            codings_moved: moved,
            codings_removed: removed.len(),
            units_moved,
            units_removed: gone_units.len(),
            proposals_rejected,
            units_added,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runs_for(old: &str, new: &str) -> Vec<Run> {
        surviving_runs(old, new, Duration::from_secs(2))
    }

    fn remap(old: &str, new: &str, start: i64, end: i64) -> Option<(i64, i64)> {
        remap_span(&runs_for(old, new), start, end)
    }

    #[test]
    fn an_insertion_inside_a_span_joins_it_but_one_at_an_edge_does_not() {
        let (old, new) = ("hello world", "hello brave world");
        assert_eq!(remap(old, new, 0, 5), Some((0, 5)), "before the edit: unchanged");
        assert_eq!(remap(old, new, 6, 11), Some((12, 17)), "after the edit: shifted");
        assert_eq!(remap(old, new, 2, 9), Some((2, 15)), "inside the span: it grows");
        assert_eq!(remap(old, new, 0, 6), Some((0, 6)), "at the end of the span: not included");
        assert_eq!(remap(old, new, 6, 8), Some((12, 14)), "at the start of the span: not included");
    }

    #[test]
    fn deleted_text_shrinks_or_removes_a_span() {
        let (old, new) = ("one two three", "one three");
        assert_eq!(remap(old, new, 4, 7), None, "the word two is gone, and the t of three must not stand in for it");
        assert_eq!(remap(old, new, 0, 7), Some((0, 4)), "a span that lost its tail keeps what survived");
        assert_eq!(remap(old, new, 4, 13), Some((4, 9)), "or its head");
        assert_eq!(remap(old, new, 0, 13), Some((0, 9)));
    }

    #[test]
    fn a_corrected_word_stays_coded_whole() {
        assert_eq!(remap("fix teh typo", "fix the typo", 4, 7), Some((4, 7)), "a transposed word");
        assert_eq!(remap("the color red", "the colour red", 4, 9), Some((4, 10)), "an added letter");
        assert_eq!(remap("the color red", "the colour red", 6, 8), Some((4, 10)), "even a span inside the word takes the whole new word");
        assert_eq!(remap("fix teh typo", "fix the typo", 0, 7), Some((0, 7)), "a span ending in the corrected word");
    }

    #[test]
    fn a_replaced_word_is_not_a_corrected_word() {
        let (old, new) = ("the cat sat", "the dog sat");
        assert_eq!(remap(old, new, 4, 7), None, "the coded word was replaced by an unrelated one");
        assert_eq!(remap(old, new, 0, 7), Some((0, 4)), "only what survived stays");
        assert_eq!(remap(old, new, 0, 11), Some((0, 11)), "a span around the whole change keeps all of it");
    }

    #[test]
    fn languages_written_without_spaces_work_character_by_character() {
        let (old, new) = ("\u{4f60}\u{597d}\u{4e16}\u{754c}", "\u{4f60}\u{597d}\u{5927}\u{4e16}\u{754c}");
        assert_eq!(remap(old, new, 2, 4), Some((3, 5)), "the last two characters moved over");
        assert_eq!(remap(old, new, 0, 4), Some((0, 5)), "a span across the insertion grows");
    }

    #[test]
    fn scalar_values_not_bytes_are_counted() {
        assert_eq!(remap("a\u{1F600}b", "a\u{1F600}\u{1F600}b", 2, 3), Some((3, 4)), "b moved by one emoji");
    }

    #[test]
    fn identical_or_wholly_different_texts_behave() {
        assert_eq!(remap("same", "same", 1, 3), Some((1, 3)));
        assert_eq!(remap("abc", "xyz", 0, 3), None);
        assert_eq!(remap("abc", "", 0, 3), None);
        assert_eq!(remap("", "abc", 0, 0), None);
    }

    #[test]
    fn odd_run_lists_do_not_panic() {
        assert_eq!(remap_span(&[], 0, 5), None);
        let runs = vec![Run::exact(5, 0, 3)];
        assert_eq!(remap_span(&runs, 0, 5), None);
        assert_eq!(remap_span(&runs, 8, 20), None);
        assert_eq!(remap_span(&runs, 6, 7), Some((1, 2)));
    }
}
