//! The final layer for areas of pictures and PDF pages and for stretches of recordings (decision 176).
//!
//! Adjudication of words works on units. An area or a stretch has no unit, so what is settled is whether it belongs in the final layer.
//! The coded areas of one code on one page (or the coded stretches of one code of one recording) that overlap by at least half are the same
//! area or stretch, whoever drew them. Where only one coder coded the document, every area is final. Where several did: an area that every
//! one of them marked is agreed and final; an area that some did not mark is in disagreement, and a person accepts it or leaves it out, with
//! a reason. Nothing is stored but the decisions: the groups are worked out from the coded areas each time.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::project::Project;
use crate::{Error, RegionRow, Result, Shape};

/// How much two areas (or stretches) of one code must overlap, as a share of what they cover together, to be the same one.
const MIN_OVERLAP: f64 = 0.5;

/// One coder's area in a group.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionMember {
    pub region_id: i64,
    pub coder_id: i64,
    pub coder_name: String,
    #[serde(flatten)]
    pub shape: Shape,
    pub rationale: String,
}

/// What a person decided about a group.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionDecision {
    pub accepted: bool,
    pub reason: String,
    pub adjudicator_id: i64,
    pub adjudicator_name: String,
    pub created_at: String,
}

/// An area or stretch some coders marked and others did not.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionCase {
    /// The number the decision goes by: the lowest-numbered area of the group.
    pub key: i64,
    pub source_id: i64,
    pub source_name: String,
    /// The page of the PDF, counting from 1; 1 for a picture.
    pub page: i64,
    /// `image` for an area of a picture or a PDF page, `clip` for a stretch of a recording.
    pub kind: String,
    pub code_id: i64,
    pub code_name: String,
    pub members: Vec<RegionMember>,
    /// The coders of the document who did not mark it.
    pub missing: Vec<String>,
    pub decision: Option<RegionDecision>,
}

/// How the final layer of areas and stretches stands.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RegionCounts {
    /// Areas and stretches a person decided (accepted or left out).
    pub decided: usize,
    /// Agreed by every coder of the document, or the only coder's.
    pub agreed: usize,
    /// In disagreement, with no decision yet: not in the final layer.
    pub open: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Single,
    Agreed,
    Open,
}

struct Cluster {
    key: i64,
    members: Vec<RegionRow>,
    missing: Vec<i64>,
    status: Status,
    decision: Option<(bool, String, i64, String)>,
}

fn iou(a: &Shape, b: &Shape) -> f64 {
    let (a, b) = (a.bbox(), b.bbox());
    let ix = (a.2.min(b.2) - a.0.max(b.0)).max(0.0);
    let iy = (a.3.min(b.3) - a.1.max(b.1)).max(0.0);
    let inter = ix * iy;
    let union = (a.2 - a.0) * (a.3 - a.1) + (b.2 - b.0) * (b.3 - b.1) - inter;
    if union > 0.0 {
        inter / union
    } else {
        0.0
    }
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

impl Project {
    fn region_clusters(&self) -> Result<Vec<Cluster>> {
        let regions: Vec<RegionRow> = self.all_regions()?.into_iter().filter(|r| r.layer == "coder").collect();
        let mut decisions: HashMap<i64, (bool, String, i64, String)> = HashMap::new();
        {
            let mut stmt = self.conn.prepare("SELECT region_id, accepted, reason, adjudicator_id, created_at FROM region_decision")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)? != 0, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, String>(4)?))))?;
            for row in rows {
                let (id, d) = row?;
                decisions.insert(id, d);
            }
        }
        let mut coders_of: BTreeMap<i64, BTreeSet<i64>> = BTreeMap::new();
        for r in &regions {
            coders_of.entry(r.source_id).or_default().insert(r.coder_id);
        }
        // the areas of one code on one page of one document (stretches apart from areas): the groups that can be the same area
        let mut groups: BTreeMap<(i64, i64, bool, i64), Vec<usize>> = BTreeMap::new();
        for (i, r) in regions.iter().enumerate() {
            groups.entry((r.source_id, r.page, r.shape.is_span(), r.code_id)).or_default().push(i);
        }
        let mut parent: Vec<usize> = (0..regions.len()).collect();
        for members in groups.values() {
            for (a, &i) in members.iter().enumerate() {
                for &j in &members[a + 1..] {
                    if regions[i].coder_id != regions[j].coder_id && iou(&regions[i].shape, &regions[j].shape) >= MIN_OVERLAP {
                        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                        if ri != rj {
                            parent[ri] = rj;
                        }
                    }
                }
            }
        }
        let mut components: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..regions.len() {
            let root = find(&mut parent, i);
            components.entry(root).or_default().push(i);
        }
        let mut out = Vec::new();
        for idx in components.values() {
            let mut members: Vec<RegionRow> = idx.iter().map(|&i| regions[i].clone()).collect();
            members.sort_by_key(|r| (r.coder_id, r.id));
            let key = members.iter().map(|r| r.id).min().unwrap_or(0);
            let source = members[0].source_id;
            let everyone = coders_of.get(&source).cloned().unwrap_or_default();
            let here: BTreeSet<i64> = members.iter().map(|r| r.coder_id).collect();
            let status = if everyone.len() <= 1 {
                Status::Single
            } else if here == everyone {
                Status::Agreed
            } else {
                Status::Open
            };
            let missing: Vec<i64> = everyone.difference(&here).copied().collect();
            // the decision on the group: on its key, or (when groups have grown together) the latest on any of its areas
            let decision = decisions.get(&key).cloned().or_else(|| members.iter().filter_map(|m| decisions.get(&m.id)).max_by(|a, b| a.3.cmp(&b.3)).cloned());
            out.push(Cluster { key, members, missing, status, decision });
        }
        out.sort_by_key(|c| (c.members[0].source_id, c.members[0].page, c.key));
        Ok(out)
    }

    /// The areas and stretches that are in the final layer, and how it stands.
    pub(crate) fn final_region_ids(&self) -> Result<(HashSet<i64>, RegionCounts)> {
        let mut ids = HashSet::new();
        let mut counts = RegionCounts::default();
        for c in self.region_clusters()? {
            let representative = c.members[0].id;
            match (c.status, &c.decision) {
                (Status::Single | Status::Agreed, _) => {
                    counts.agreed += 1;
                    ids.insert(representative);
                }
                (Status::Open, Some((accepted, ..))) => {
                    counts.decided += 1;
                    if *accepted {
                        ids.insert(representative);
                    }
                }
                (Status::Open, None) => counts.open += 1,
            }
        }
        Ok((ids, counts))
    }

    /// The areas and stretches that some of the coders of a document marked and others did not, each with who marked it, and the decision if a
    /// person made one. In reading order.
    pub fn region_disagreements(&self) -> Result<Vec<RegionCase>> {
        let coders: HashMap<i64, String> = self.list_coders()?.into_iter().map(|c| (c.id, c.name)).collect();
        let sources: HashMap<i64, String> = self.list_sources()?.into_iter().map(|s| (s.id, s.name)).collect();
        let codes: HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();
        let name = |id: i64| coders.get(&id).cloned().unwrap_or_default();
        let mut out = Vec::new();
        for c in self.region_clusters()?.into_iter().filter(|c| c.status == Status::Open) {
            let first = &c.members[0];
            out.push(RegionCase {
                key: c.key,
                source_id: first.source_id,
                source_name: sources.get(&first.source_id).cloned().unwrap_or_default(),
                page: first.page,
                kind: if first.shape.is_span() { "clip" } else { "image" }.to_string(),
                code_id: first.code_id,
                code_name: codes.get(&first.code_id).cloned().unwrap_or_default(),
                members: c.members.iter().map(|m| RegionMember { region_id: m.id, coder_id: m.coder_id, coder_name: name(m.coder_id), shape: m.shape.clone(), rationale: m.rationale.clone() }).collect(),
                missing: c.missing.iter().map(|id| name(*id)).collect(),
                decision: c.decision.map(|(accepted, reason, adjudicator_id, created_at)| RegionDecision { accepted, reason, adjudicator_id, adjudicator_name: name(adjudicator_id), created_at }),
            });
        }
        Ok(out)
    }

    /// Accepts an area or stretch that is in disagreement into the final layer, or leaves it out, with the reason. `key` is the number the case
    /// goes by (`RegionCase::key`). Deciding again replaces the decision.
    pub fn decide_region(&mut self, key: i64, accept: bool, reason: &str, adjudicator_id: i64) -> Result<RegionCase> {
        self.ensure_writable()?;
        let reason = reason.trim().to_string();
        if reason.is_empty() {
            return Err(Error::Invalid("a decision needs its reason: say why this area belongs in the final layer, or why it does not".to_string()));
        }
        if !self.region_clusters()?.iter().any(|c| c.key == key && c.status == Status::Open) {
            return Err(Error::Invalid("that area or stretch is not in disagreement: every coder of its document marked it, or only one coder worked on the document".to_string()));
        }
        let before: Option<Value> = self
            .conn
            .query_row("SELECT adjudicator_id, accepted, reason, created_at FROM region_decision WHERE region_id = ?1", [key], |r| {
                Ok(json!({ "adjudicator_id": r.get::<_, i64>(0)?, "accepted": r.get::<_, i64>(1)? != 0, "reason": r.get::<_, String>(2)?, "created_at": r.get::<_, String>(3)? }))
            })
            .optional()?;
        let tx = self.conn.transaction()?;
        let coder: Option<i64> = tx.query_row("SELECT id FROM coder WHERE id = ?1", [adjudicator_id], |r| r.get(0)).optional()?;
        if coder.is_none() {
            return Err(Error::NotFound { what: "coder", id: adjudicator_id });
        }
        tx.execute("DELETE FROM region_decision WHERE region_id = ?1", [key])?;
        tx.execute(
            "INSERT INTO region_decision (region_id, adjudicator_id, accepted, reason, created_at) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![key, adjudicator_id, accept as i64, reason],
        )?;
        let after = json!({ "adjudicator_id": adjudicator_id, "accepted": accept, "reason": reason });
        event::insert(&tx, &self.actor, &self.session, "adjudication.region.set", Some(("region", key)), before.as_ref(), Some(&after))?;
        tx.commit()?;
        self.region_disagreements()?.into_iter().find(|c| c.key == key).ok_or(Error::NotFound { what: "area in disagreement", id: key })
    }
}
