//! Links between two coded passages with a named relation (decision 162): "this passage *contradicts* that one", "this *explains* that".
//!
//! The project file has a `link` table (a labeled connection between any two things); a passage link is a row of it from one coding to
//! another, where a coding is words of a document or (a negative number, as everywhere a list holds both) an area of a picture or PDF page
//! or a stretch of a recording. The relation is the link's label: a word or two, anything the researcher wants, with a few common ones
//! offered first. A link is one way (from the first passage to the second); the same pair can be linked again with another relation.
//!
//! A link whose passage has been removed is kept (so Undo can bring both back together) but is not listed while either end is missing.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension, Transaction};
use serde::Serialize;
use serde_json::{json, Value};

use crate::segments::{CodedSegment, SegmentFilter};
use crate::{event, Error, Project, Result};

/// The longest a relation may be, in characters.
pub const MAX_RELATION_CHARS: usize = 80;

/// The relations offered first, before the ones the project already uses.
pub const COMMON_RELATIONS: &[&str] = &["supports", "contradicts", "explains", "leads to", "is an example of", "is similar to", "contrasts with"];

/// One link between two passages, with the passages.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PassageLink {
    pub id: i64,
    pub relation: String,
    pub from: CodedSegment,
    pub to: CodedSegment,
}

/// A relation the project uses, and how many links carry it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Relation {
    pub name: String,
    pub uses: i64,
}

fn clean_relation(relation: &str) -> Result<String> {
    let r = relation.split_whitespace().collect::<Vec<_>>().join(" ");
    if r.is_empty() {
        return Err(Error::Invalid("say how the two passages are related, in a word or two".to_string()));
    }
    if r.chars().count() > MAX_RELATION_CHARS {
        return Err(Error::Invalid(format!("a relation is at most {MAX_RELATION_CHARS} characters")));
    }
    Ok(r)
}

/// Takes the links of a passage (a coding's id, or an area's id made negative) out of the project and returns them, to be kept in the event that
/// removes the passage. A passage's number can be given to a new passage later (the largest number is used again), so a link left behind would
/// attach itself to someone else's words. Undo puts the passage back with `restore_links`.
pub(crate) fn take_links(tx: &Transaction, passage: i64) -> Result<Vec<Value>> {
    let mut stmt = tx.prepare(
        "SELECT id, from_type, from_id, to_type, to_id, label FROM link
         WHERE (from_type = 'coding' AND from_id = ?1) OR (to_type = 'coding' AND to_id = ?1)",
    )?;
    let rows: Vec<Value> = stmt
        .query_map([passage], |r| {
            Ok(json!({ "id": r.get::<_, i64>(0)?, "from_type": r.get::<_, String>(1)?, "from_id": r.get::<_, i64>(2)?, "to_type": r.get::<_, String>(3)?, "to_id": r.get::<_, i64>(4)?, "label": r.get::<_, String>(5)? }))
        })?
        .collect::<std::result::Result<_, _>>()?;
    drop(stmt);
    tx.execute("DELETE FROM link WHERE (from_type = 'coding' AND from_id = ?1) OR (to_type = 'coding' AND to_id = ?1)", [passage])?;
    Ok(rows)
}

/// Puts back links that `take_links` kept.
pub(crate) fn restore_links(tx: &Transaction, links: &Value) -> Result<()> {
    for l in links.as_array().into_iter().flatten() {
        tx.execute(
            "INSERT OR IGNORE INTO link (id, from_type, from_id, to_type, to_id, label) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![l["id"].as_i64(), l["from_type"].as_str().unwrap_or("coding"), l["from_id"].as_i64(), l["to_type"].as_str().unwrap_or("coding"), l["to_id"].as_i64(), l["label"].as_str().unwrap_or("")],
        )?;
    }
    Ok(())
}

impl Project {
    /// Links passage `from` to passage `to` (coding ids; an area is its number made negative) with a relation. Returns the link's id.
    pub fn add_passage_link(&mut self, from: i64, to: i64, relation: &str) -> Result<i64> {
        self.ensure_writable()?;
        let relation = clean_relation(relation)?;
        if from == to {
            return Err(Error::Invalid("a passage cannot be linked to itself".to_string()));
        }
        let twin: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM link WHERE from_type = 'coding' AND to_type = 'coding' AND from_id = ?1 AND to_id = ?2 AND lower(label) = lower(?3)",
                params![from, to, relation],
                |r| r.get(0),
            )
            .optional()?;
        if twin.is_some() {
            return Err(Error::Invalid("these two passages are already linked in that direction with that relation".to_string()));
        }
        self.add_link("coding", from, "coding", to, &relation)
    }

    /// Changes the relation of a link.
    pub fn update_passage_link(&mut self, id: i64, relation: &str) -> Result<()> {
        self.ensure_writable()?;
        let relation = clean_relation(relation)?;
        let tx = self.conn.transaction()?;
        let (old, from, to): (String, i64, i64) = tx
            .query_row("SELECT label, from_id, to_id FROM link WHERE id = ?1 AND from_type = 'coding' AND to_type = 'coding'", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "link", id })?;
        if old == relation {
            return Ok(());
        }
        let twin: Option<i64> = tx
            .query_row(
                "SELECT id FROM link WHERE from_type = 'coding' AND to_type = 'coding' AND from_id = ?1 AND to_id = ?2 AND lower(label) = lower(?3) AND id <> ?4",
                params![from, to, relation, id],
                |r| r.get(0),
            )
            .optional()?;
        if twin.is_some() {
            return Err(Error::Invalid("these two passages are already linked in that direction with that relation".to_string()));
        }
        tx.execute("UPDATE link SET label = ?1 WHERE id = ?2", params![relation, id])?;
        event::insert(&tx, &self.actor, &self.session, "link.update", Some(("link", id)), Some(&json!({ "label": old })), Some(&json!({ "label": relation })))?;
        tx.commit()?;
        Ok(())
    }

    /// Every link between passages (or, with `coding_id`, those that start or end at that passage), oldest first, each with both passages.
    /// A link is left out while either of its passages is missing.
    pub fn list_passage_links(&self, coding_id: Option<i64>) -> Result<Vec<PassageLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, from_id, to_id, label FROM link
             WHERE from_type = 'coding' AND to_type = 'coding' AND (?1 IS NULL OR from_id = ?1 OR to_id = ?1) ORDER BY id",
        )?;
        let rows: Vec<(i64, i64, i64, String)> = stmt.query_map([coding_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<std::result::Result<_, _>>()?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let mut ids: Vec<i64> = rows.iter().flat_map(|(_, a, b, _)| [*a, *b]).collect();
        ids.sort_unstable();
        ids.dedup();
        let segments: HashMap<i64, CodedSegment> = self
            .coded_segments(&SegmentFilter { coding_ids: Some(ids), ..Default::default() })?
            .into_iter()
            .map(|s| (s.coding_id, s))
            .collect();
        Ok(rows
            .into_iter()
            .filter_map(|(id, a, b, relation)| Some(PassageLink { id, relation, from: segments.get(&a)?.clone(), to: segments.get(&b)?.clone() }))
            .collect())
    }

    /// The relations in use, most used first.
    pub fn passage_relations(&self) -> Result<Vec<Relation>> {
        let mut stmt = self.conn.prepare(
            "SELECT min(label), COUNT(*) FROM link WHERE from_type = 'coding' AND to_type = 'coding'
             GROUP BY lower(label) ORDER BY COUNT(*) DESC, lower(label)",
        )?;
        let rows = stmt.query_map([], |r| Ok(Relation { name: r.get(0)?, uses: r.get(1)? }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }
}
