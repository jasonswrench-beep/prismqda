//! Coded segments with their text (specification 8.5 and 23.2): what exports and reports are built from.
//!
//! Every coding is returned with the words it covers, the code and coder by name, and the unit it belongs to,
//! so a file made from it can be read without opening the project. The text is cut from each source once, by
//! counting Unicode scalar values the same way the offsets do.

use std::collections::{HashMap, HashSet};

use rusqlite::params_from_iter;
use serde::{Deserialize, Serialize};

use crate::project::Project;
use crate::Result;

/// Which codings to return. Every field left out means "all".
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SegmentFilter {
    #[serde(default)]
    pub source_ids: Option<Vec<i64>>,
    /// Codes to include. Their sub-codes are included too unless `exact_codes` is set.
    #[serde(default)]
    pub code_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub exact_codes: bool,
    #[serde(default)]
    pub coder_ids: Option<Vec<i64>>,
    /// `coder` (each coder's own codings) or `final` (the adjudicated layer).
    #[serde(default)]
    pub layer: Option<String>,
    /// Only these codings (by id). Used to list the codings linked to a code as counterexamples. A negative number is a coded area
    /// of a picture (area 7 is -7).
    #[serde(default)]
    pub coding_ids: Option<Vec<i64>>,
}

/// One coding, with the text it covers.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodedSegment {
    /// `text` for words of a document, `image` for an area of a picture or a PDF page, `clip` for a stretch of a recording. A coded
    /// area or stretch is numbered apart from the codings of text, so in a list that holds both its `coding_id` is its number made
    /// negative (area 7 is -7) and the two never share a number.
    pub kind: String,
    /// The area, for a segment of kind `image`, or the stretch (`span`), for a segment of kind `clip`; `null` for text.
    pub region: Option<crate::Shape>,
    /// The page of the PDF an area is on, for a segment of kind `image` whose document is a PDF; `null` for text and for a picture.
    pub page: Option<i64>,
    pub coding_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub folder: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
    pub code_id: i64,
    pub code_name: String,
    pub coder_id: i64,
    pub coder_name: String,
    pub coder_role: String,
    pub origin: String,
    pub layer: String,
    pub rationale: String,
    /// A rating from 1 to 5, or null when the coding has not been rated.
    pub rating: Option<i64>,
    pub unit_id: Option<i64>,
    /// The unit's own identifier from the imported file, when it has one.
    pub unit_external_id: Option<String>,
    pub speaker: Option<String>,
    pub created_at: String,
}

impl Project {
    /// The codings that match the filter, in source order and then text order.
    pub fn coded_segments(&self, filter: &SegmentFilter) -> Result<Vec<CodedSegment>> {
        let codes = self.list_codes()?;
        let code_names: HashMap<i64, String> = codes.iter().map(|c| (c.id, c.name.clone())).collect();
        let wanted_codes: Option<HashSet<i64>> = filter.code_ids.as_ref().map(|ids| {
            let mut set: HashSet<i64> = ids.iter().copied().collect();
            if !filter.exact_codes {
                // Walk down the tree: a code's sub-codes count as the code.
                loop {
                    let before = set.len();
                    for c in &codes {
                        if c.parent_id.is_some_and(|p| set.contains(&p)) {
                            set.insert(c.id);
                        }
                    }
                    if set.len() == before {
                        break;
                    }
                }
            }
            set
        });
        let coders: HashMap<i64, (String, String)> = self.list_coders()?.into_iter().map(|c| (c.id, (c.name, c.role))).collect();

        // Unit identifier and speaker, for the units that have codings.
        let mut units: HashMap<i64, (Option<String>, Option<String>)> = HashMap::new();
        let mut stmt = self.conn.prepare(
            "SELECT u.id, u.speaker,
                    (SELECT av.value FROM attribute_value av JOIN attribute a ON a.id = av.attribute_id AND a.applies_to = 'unit' AND a.name = 'external_id'
                     WHERE av.target_id = u.id)
             FROM unit u WHERE u.id IN (SELECT DISTINCT unit_id FROM coding WHERE unit_id IS NOT NULL)",
        )?;
        for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (id, speaker, external) = row?;
            units.insert(id, (external, speaker));
        }

        // The final layer (decision 173): the decisions and the codes the coders agreed on, worked out from them. Asked for by name, or by the
        // number of one of a decision's codings (which is how the evidence of a number in Explore asks for it).
        type Raw = (i64, i64, i64, i64, i64, String, String, String, Option<i64>, String, Option<i64>);
        let wants_final = filter.layer.as_deref() == Some("final");
        let final_rows: Vec<crate::final_layer::FinalRow> = if wants_final || filter.coding_ids.as_ref().is_some_and(|ids| ids.iter().any(|i| crate::final_layer::is_final_id(*i))) { self.final_layer()?.rows } else { Vec::new() };
        // the areas and stretches of the final layer (decision 176): the ones the coders agree on and the ones a person accepted
        let final_regions: Option<HashSet<i64>> = if wants_final { Some(self.final_region_ids()?.0) } else { None };
        let from_final = |r: &crate::final_layer::FinalRow| -> Raw { (r.id, r.start, r.end, r.code_id, r.coder_id, r.origin.clone(), "final".to_string(), r.rationale.clone(), Some(r.unit_id), r.created_at.clone(), r.rating) };

        let mut out = Vec::new();
        for source in self.list_sources()? {
            if filter.source_ids.as_ref().is_some_and(|ids| !ids.contains(&source.id)) {
                continue;
            }
            let rows: Vec<Raw> = if wants_final {
                final_rows.iter().filter(|r| r.source_id == source.id).map(from_final).collect()
            } else {
                let mut sql = String::from(
                    "SELECT id, start_offset, end_offset, code_id, coder_id, origin, layer, rationale, unit_id, created_at, rating
                     FROM coding WHERE source_id = ?1",
                );
                let mut args: Vec<rusqlite::types::Value> = vec![source.id.into()];
                if let Some(layer) = &filter.layer {
                    sql.push_str(" AND layer = ?");
                    args.push(layer.clone().into());
                }
                sql.push_str(" ORDER BY start_offset, end_offset, id");
                let mut stmt = self.conn.prepare(&sql)?;
                let mut rows: Vec<Raw> = stmt
                    .query_map(params_from_iter(args.iter()), |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?))
                    })?
                    .collect::<std::result::Result<_, _>>()?;
                rows.extend(final_rows.iter().filter(|r| r.source_id == source.id && !r.real && filter.coding_ids.as_ref().is_some_and(|ids| ids.contains(&r.id))).map(from_final));
                rows.sort_by_key(|r| (r.1, r.2, r.0));
                rows
            };
            let rows: Vec<Raw> = rows
                .into_iter()
                .filter(|r| wanted_codes.as_ref().is_none_or(|w| w.contains(&r.3)))
                .filter(|r| filter.coder_ids.as_ref().is_none_or(|ids| ids.contains(&r.4)))
                .filter(|r| filter.coding_ids.as_ref().is_none_or(|ids| ids.contains(&r.0)))
                .collect();
            if rows.is_empty() {
                continue;
            }
            // Byte position of every character, so a span of characters can be cut out in one step.
            let text = self.source_text(source.id)?;
            let mut at: Vec<usize> = text.char_indices().map(|(b, _)| b).collect();
            at.push(text.len());
            for (id, start, end, code_id, coder_id, origin, layer, rationale, unit_id, created_at, rating) in rows {
                let (s, e) = (start as usize, end as usize);
                let quote = if e < at.len() && s <= e { text[at[s]..at[e]].to_string() } else { String::new() };
                let (coder_name, coder_role) = coders.get(&coder_id).cloned().unwrap_or_default();
                let (unit_external_id, speaker) = unit_id.and_then(|u| units.get(&u).cloned()).unwrap_or((None, None));
                out.push(CodedSegment {
                    kind: "text".to_string(),
                    region: None,
                    page: None,
                    coding_id: id,
                    source_id: source.id,
                    source_name: source.name.clone(),
                    folder: source.folder.clone(),
                    start,
                    end,
                    text: quote,
                    code_id,
                    code_name: code_names.get(&code_id).cloned().unwrap_or_default(),
                    coder_id,
                    coder_name,
                    coder_role,
                    origin,
                    layer,
                    rationale,
                    rating,
                    unit_id,
                    unit_external_id,
                    speaker,
                    created_at,
                });
            }
        }
        // Areas of pictures come after the text of the same document, in reading order.
        {
            let sources: HashMap<i64, (String, String)> = self.list_sources()?.into_iter().map(|s| (s.id, (s.name, s.folder))).collect();
            let pdfs: HashSet<i64> = self.list_sources()?.into_iter().filter(|s| s.pdf.is_some()).map(|s| s.id).collect();
            let mut regions: Vec<_> = self
                .all_regions()?
                .into_iter()
                .filter(|r| filter.source_ids.as_ref().is_none_or(|ids| ids.contains(&r.source_id)))
                .filter(|r| wanted_codes.as_ref().is_none_or(|w| w.contains(&r.code_id)))
                .filter(|r| filter.coder_ids.as_ref().is_none_or(|ids| ids.contains(&r.coder_id)))
                .filter(|r| match &final_regions {
                    Some(ids) => ids.contains(&r.id),
                    None => filter.layer.as_ref().is_none_or(|l| *l == r.layer),
                })
                .filter(|r| filter.coding_ids.as_ref().is_none_or(|ids| ids.contains(&-r.id)))
                .collect();
            regions.sort_by_key(|r| r.source_id);
            for r in regions {
                let (source_name, folder) = sources.get(&r.source_id).cloned().unwrap_or_default();
                let (coder_name, coder_role) = coders.get(&r.coder_id).cloned().unwrap_or_default();
                out.push(CodedSegment {
                    kind: if r.shape.is_span() { "clip" } else { "image" }.to_string(),
                    coding_id: -r.id,
                    source_id: r.source_id,
                    source_name,
                    folder,
                    start: 0,
                    end: 0,
                    text: r.text.clone(),
                    code_id: r.code_id,
                    code_name: code_names.get(&r.code_id).cloned().unwrap_or_default(),
                    coder_id: r.coder_id,
                    coder_name,
                    coder_role,
                    origin: r.origin.clone(),
                    layer: if wants_final { "final".to_string() } else { r.layer.clone() },
                    rationale: r.rationale.clone(),
                    rating: r.rating,
                    unit_id: None,
                    unit_external_id: None,
                    speaker: None,
                    created_at: r.created_at.clone(),
                    page: pdfs.contains(&r.source_id).then_some(r.page),
                    region: Some(r.shape),
                });
            }
            // Keep documents in the order the list of documents has them, whichever kind of coding each holds.
            let order: HashMap<i64, usize> = self.list_sources()?.iter().enumerate().map(|(i, s)| (s.id, i)).collect();
            out.sort_by_key(|s| order.get(&s.source_id).copied().unwrap_or(usize::MAX));
        }
        Ok(out)
    }

    /// The counterexamples marked for one code, with their text: passages coded with other codes that look as if
    /// they belong to this one and do not.
    pub fn counterexample_segments(&self, code_id: i64) -> Result<Vec<CodedSegment>> {
        let ids = self.list_counterexamples(code_id)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.coded_segments(&SegmentFilter { coding_ids: Some(ids), ..SegmentFilter::default() })
    }

    /// The anchor examples marked for one code, with their text.
    pub fn anchor_examples(&self, code_id: i64) -> Result<Vec<CodedSegment>> {
        let anchors: HashSet<i64> = self.list_anchors(code_id)?.into_iter().collect();
        if anchors.is_empty() {
            return Ok(Vec::new());
        }
        let filter = SegmentFilter { code_ids: Some(vec![code_id]), exact_codes: true, ..SegmentFilter::default() };
        Ok(self.coded_segments(&filter)?.into_iter().filter(|s| anchors.contains(&s.coding_id)).collect())
    }
}
