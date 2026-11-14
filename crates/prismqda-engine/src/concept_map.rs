//! Concept maps (Explore): codes, cases, memos, and free notes placed on a page and joined by named links, to think with and to
//! show. A node that stands for a code, case, or memo shows that thing's own name, so renaming it renames it on every map. Every
//! change is logged and can be undone.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};

use crate::event;
use crate::explore::{RelationMode, RelationsSpec, Scope};
use crate::project::Project;
use crate::{Error, Result};

const MAX_NAME: usize = 80;
const MAX_LABEL: usize = 200;
/// Where a layout puts nodes, in the map's own units: the nodes are about 160 wide and 40 tall.
const GAP_X: f64 = 220.0;
const GAP_Y: f64 = 110.0;

/// A place on the map to one decimal. Places are written into the log and read back when a change is undone, and a number with
/// only one decimal comes back exactly, where a long fraction can come back a digit different.
fn tidy(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapRow {
    pub id: i64,
    pub name: String,
    pub notes: String,
    pub nodes: usize,
    pub links: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapNode {
    pub id: i64,
    pub kind: String,
    pub ref_id: Option<i64>,
    /// The words on the node: the name of the code, case, or memo it stands for, or a note's own text.
    pub label: String,
    pub x: f64,
    pub y: f64,
    /// True when the code, case, or memo it stood for no longer exists.
    pub missing: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MapLink {
    pub id: i64,
    pub from_node: i64,
    pub to_node: i64,
    pub label: String,
    pub directed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConceptMap {
    pub id: i64,
    pub name: String,
    pub notes: String,
    pub nodes: Vec<MapNode>,
    pub links: Vec<MapLink>,
    pub created_at: String,
    pub updated_at: String,
}

/// What adding codes from the codebook did.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AddedCodes {
    pub nodes_added: usize,
    pub links_added: usize,
    /// Codes that were already on the map.
    pub already_there: usize,
}

fn clean_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(Error::Invalid("give the map a name".to_string()));
    }
    if n.chars().count() > MAX_NAME {
        return Err(Error::Invalid(format!("a map's name can be at most {MAX_NAME} characters")));
    }
    Ok(n.to_string())
}

fn clean_label(label: &str) -> Result<String> {
    let l = label.trim();
    if l.chars().count() > MAX_LABEL {
        return Err(Error::Invalid(format!("the words of a note or a link can be at most {MAX_LABEL} characters")));
    }
    Ok(l.to_string())
}

/// The name a node stands for, or `None` when the thing is gone.
fn name_of(conn: &Connection, kind: &str, id: i64) -> Result<Option<String>> {
    let sql = match kind {
        "code" => "SELECT name FROM code WHERE id = ?1",
        "case" => "SELECT name FROM research_case WHERE id = ?1",
        "memo" => "SELECT CASE WHEN trim(title) <> '' THEN title ELSE substr(body, 1, 60) END FROM memo WHERE id = ?1",
        _ => return Ok(None),
    };
    Ok(conn.query_row(sql, [id], |r| r.get(0)).optional()?)
}

fn node_from(conn: &Connection, r: &rusqlite::Row) -> Result<MapNode> {
    let (id, kind, ref_id, label, x, y): (i64, String, Option<i64>, String, f64, f64) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?);
    let (label, missing) = match (kind.as_str(), ref_id) {
        ("note", _) | (_, None) => (label, false),
        (k, Some(rid)) => match name_of(conn, k, rid)? {
            Some(n) => (n, false),
            None => (format!("(removed {k})"), true),
        },
    };
    Ok(MapNode { id, kind, ref_id, label, x, y, missing })
}

impl Project {
    fn map_nodes(&self, map_id: i64) -> Result<Vec<MapNode>> {
        let mut stmt = self.conn.prepare("SELECT id, kind, ref_id, label, x, y FROM map_node WHERE map_id = ?1 ORDER BY id")?;
        let mut rows = stmt.query([map_id])?;
        let mut out = Vec::new();
        while let Some(r) = rows.next()? {
            out.push(node_from(&self.conn, r)?);
        }
        Ok(out)
    }

    fn map_links(&self, map_id: i64) -> Result<Vec<MapLink>> {
        let mut stmt = self.conn.prepare("SELECT id, from_node, to_node, label, directed FROM map_link WHERE map_id = ?1 ORDER BY id")?;
        let rows = stmt.query_map([map_id], |r| Ok(MapLink { id: r.get(0)?, from_node: r.get(1)?, to_node: r.get(2)?, label: r.get(3)?, directed: r.get::<_, i64>(4)? != 0 }))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    fn require_map(&self, id: i64) -> Result<()> {
        let found: Option<i64> = self.conn.query_row("SELECT id FROM concept_map WHERE id = ?1", [id], |r| r.get(0)).optional()?;
        found.map(|_| ()).ok_or(Error::NotFound { what: "concept map", id })
    }

    fn map_name_free(&self, name: &str, except: Option<i64>) -> Result<bool> {
        let taken: Option<i64> = self.conn.query_row("SELECT id FROM concept_map WHERE lower(name) = lower(?1) AND id IS NOT ?2", params![name, except], |r| r.get(0)).optional()?;
        Ok(taken.is_none())
    }

    fn node_row(&self, node_id: i64) -> Result<(i64, String, Option<i64>, String, f64, f64)> {
        self.conn
            .query_row("SELECT map_id, kind, ref_id, label, x, y FROM map_node WHERE id = ?1", [node_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "map node", id: node_id })
    }

    pub fn list_concept_maps(&self) -> Result<Vec<MapRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.name, m.notes, (SELECT COUNT(*) FROM map_node WHERE map_id = m.id), (SELECT COUNT(*) FROM map_link WHERE map_id = m.id), m.created_at, m.updated_at
             FROM concept_map m ORDER BY lower(m.name)",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(MapRow { id: r.get(0)?, name: r.get(1)?, notes: r.get(2)?, nodes: r.get::<_, i64>(3)? as usize, links: r.get::<_, i64>(4)? as usize, created_at: r.get(5)?, updated_at: r.get(6)? })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_concept_map(&self, id: i64) -> Result<ConceptMap> {
        let (name, notes, created_at, updated_at): (String, String, String, String) = self
            .conn
            .query_row("SELECT name, notes, created_at, updated_at FROM concept_map WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "concept map", id })?;
        Ok(ConceptMap { id, name, notes, nodes: self.map_nodes(id)?, links: self.map_links(id)?, created_at, updated_at })
    }

    pub fn create_concept_map(&mut self, name: &str) -> Result<i64> {
        self.ensure_writable()?;
        let name = clean_name(name)?;
        if !self.map_name_free(&name, None)? {
            return Err(Error::Invalid(format!("there is already a map named '{name}'")));
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO concept_map (name, created_at, updated_at) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            [&name],
        )?;
        let id = tx.last_insert_rowid();
        event::insert(&tx, &self.actor, &self.session, "map.create", Some(("concept_map", id)), None, Some(&json!({ "name": name })))?;
        tx.commit()?;
        Ok(id)
    }

    pub fn update_concept_map(&mut self, id: i64, name: Option<&str>, notes: Option<&str>) -> Result<ConceptMap> {
        self.ensure_writable()?;
        let (old_name, old_notes): (String, String) = self
            .conn
            .query_row("SELECT name, notes FROM concept_map WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "concept map", id })?;
        let new_name = match name {
            Some(n) => clean_name(n)?,
            None => old_name.clone(),
        };
        if new_name != old_name && !self.map_name_free(&new_name, Some(id))? {
            return Err(Error::Invalid(format!("there is already a map named '{new_name}'")));
        }
        let new_notes = notes.map_or_else(|| old_notes.clone(), |n| n.trim().to_string());
        if (&new_name, &new_notes) != (&old_name, &old_notes) {
            let tx = self.conn.transaction()?;
            tx.execute("UPDATE concept_map SET name = ?1, notes = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?3", params![new_name, new_notes, id])?;
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "map.update",
                Some(("concept_map", id)),
                Some(&json!({ "name": old_name, "notes": old_notes })),
                Some(&json!({ "name": new_name, "notes": new_notes })),
            )?;
            tx.commit()?;
        }
        self.get_concept_map(id)
    }

    /// Deletes a map with its nodes and links. The whole map is kept in the log, so an undo puts it all back.
    pub fn delete_concept_map(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let (name, notes, created_at, updated_at): (String, String, String, String) = self
            .conn
            .query_row("SELECT name, notes, created_at, updated_at FROM concept_map WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "concept map", id })?;
        let nodes: Vec<Value> = {
            let mut stmt = self.conn.prepare("SELECT id, kind, ref_id, label, x, y FROM map_node WHERE map_id = ?1 ORDER BY id")?;
            let rows = stmt.query_map([id], |r| Ok(json!({ "id": r.get::<_, i64>(0)?, "kind": r.get::<_, String>(1)?, "ref_id": r.get::<_, Option<i64>>(2)?, "label": r.get::<_, String>(3)?, "x": r.get::<_, f64>(4)?, "y": r.get::<_, f64>(5)? })))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let links: Vec<Value> = {
            let mut stmt = self.conn.prepare("SELECT id, from_node, to_node, label, directed FROM map_link WHERE map_id = ?1 ORDER BY id")?;
            let rows = stmt.query_map([id], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "from": r.get::<_, i64>(1)?, "to": r.get::<_, i64>(2)?, "label": r.get::<_, String>(3)?, "directed": r.get::<_, i64>(4)? }))
            })?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM map_link WHERE map_id = ?1", [id])?;
        tx.execute("DELETE FROM map_node WHERE map_id = ?1", [id])?;
        tx.execute("DELETE FROM concept_map WHERE id = ?1", [id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "map.delete",
            Some(("concept_map", id)),
            Some(&json!({ "name": name, "notes": notes, "created_at": created_at, "updated_at": updated_at, "nodes": nodes, "links": links })),
            None,
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Puts a code, case, memo, or free note on a map. A code, case, or memo is on a map once.
    pub fn add_map_node(&mut self, map_id: i64, kind: &str, ref_id: Option<i64>, label: &str, x: f64, y: f64) -> Result<MapNode> {
        self.ensure_writable()?;
        self.require_map(map_id)?;
        let (label, ref_id) = match kind {
            "note" => {
                let l = clean_label(label)?;
                if l.is_empty() {
                    return Err(Error::Invalid("write the words of the note".to_string()));
                }
                (l, None)
            }
            "code" | "case" | "memo" => {
                let id = ref_id.ok_or_else(|| Error::Invalid(format!("say which {kind} to put on the map")))?;
                name_of(&self.conn, kind, id)?.ok_or(Error::NotFound { what: "thing to put on the map", id })?;
                let there: Option<i64> = self.conn.query_row("SELECT id FROM map_node WHERE map_id = ?1 AND kind = ?2 AND ref_id = ?3", params![map_id, kind, id], |r| r.get(0)).optional()?;
                if there.is_some() {
                    return Err(Error::Invalid(format!("that {kind} is already on this map")));
                }
                (String::new(), Some(id))
            }
            other => return Err(Error::Invalid(format!("a node is a code, a case, a memo, or a note, not '{other}'"))),
        };
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::Invalid("the place on the map is not a number".to_string()));
        }
        let (x, y) = (tidy(x), tidy(y));
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO map_node (map_id, kind, ref_id, label, x, y) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![map_id, kind, ref_id, label, x, y])?;
        let id = tx.last_insert_rowid();
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(&tx, &self.actor, &self.session, "map.node.add", Some(("map_node", id)), None, Some(&json!({ "map_id": map_id, "kind": kind, "ref_id": ref_id, "label": label, "x": x, "y": y })))?;
        tx.commit()?;
        let mut stmt = self.conn.prepare("SELECT id, kind, ref_id, label, x, y FROM map_node WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;
        let row = rows.next()?.ok_or(Error::NotFound { what: "map node", id })?;
        node_from(&self.conn, row)
    }

    /// Moves a node to a new place.
    pub fn move_map_node(&mut self, node_id: i64, x: f64, y: f64) -> Result<()> {
        self.ensure_writable()?;
        let (map_id, _, _, _, ox, oy) = self.node_row(node_id)?;
        if !x.is_finite() || !y.is_finite() {
            return Err(Error::Invalid("the place on the map is not a number".to_string()));
        }
        let (x, y) = (tidy(x), tidy(y));
        if (ox, oy) == (x, y) {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE map_node SET x = ?1, y = ?2 WHERE id = ?3", params![x, y, node_id])?;
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(&tx, &self.actor, &self.session, "map.node.move", Some(("map_node", node_id)), Some(&json!({ "x": ox, "y": oy })), Some(&json!({ "x": x, "y": y })))?;
        tx.commit()?;
        Ok(())
    }

    /// Changes the words of a note. A node that stands for a code, case, or memo takes its words from that thing.
    pub fn update_map_note(&mut self, node_id: i64, label: &str) -> Result<()> {
        self.ensure_writable()?;
        let (map_id, kind, _, old, _, _) = self.node_row(node_id)?;
        if kind != "note" {
            return Err(Error::Invalid("only a note has words of its own; a code, case, or memo shows its own name".to_string()));
        }
        let new = clean_label(label)?;
        if new.is_empty() {
            return Err(Error::Invalid("write the words of the note".to_string()));
        }
        if new == old {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute("UPDATE map_node SET label = ?1 WHERE id = ?2", params![new, node_id])?;
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(&tx, &self.actor, &self.session, "map.node.update", Some(("map_node", node_id)), Some(&json!({ "label": old })), Some(&json!({ "label": new })))?;
        tx.commit()?;
        Ok(())
    }

    /// Takes a node off the map, with the links that join it. Both are kept in the log, so an undo puts them back.
    pub fn delete_map_node(&mut self, node_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let (map_id, kind, ref_id, label, x, y) = self.node_row(node_id)?;
        let links: Vec<Value> = {
            let mut stmt = self.conn.prepare("SELECT id, from_node, to_node, label, directed FROM map_link WHERE from_node = ?1 OR to_node = ?1 ORDER BY id")?;
            let rows = stmt.query_map([node_id], |r| {
                Ok(json!({ "id": r.get::<_, i64>(0)?, "from": r.get::<_, i64>(1)?, "to": r.get::<_, i64>(2)?, "label": r.get::<_, String>(3)?, "directed": r.get::<_, i64>(4)? }))
            })?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM map_link WHERE from_node = ?1 OR to_node = ?1", [node_id])?;
        tx.execute("DELETE FROM map_node WHERE id = ?1", [node_id])?;
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "map.node.delete",
            Some(("map_node", node_id)),
            Some(&json!({ "map_id": map_id, "kind": kind, "ref_id": ref_id, "label": label, "x": x, "y": y, "links": links })),
            None,
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Joins two nodes of one map with words naming the relation.
    pub fn add_map_link(&mut self, map_id: i64, from_node: i64, to_node: i64, label: &str, directed: bool) -> Result<MapLink> {
        self.ensure_writable()?;
        self.require_map(map_id)?;
        if from_node == to_node {
            return Err(Error::Invalid("a link joins two different nodes".to_string()));
        }
        for node in [from_node, to_node] {
            let (m, ..) = self.node_row(node)?;
            if m != map_id {
                return Err(Error::Invalid("both ends of a link must be on the same map".to_string()));
            }
        }
        let label = clean_label(label)?;
        let same: Option<i64> = self
            .conn
            .query_row(
                "SELECT id FROM map_link WHERE map_id = ?1 AND label = ?4 AND ((from_node = ?2 AND to_node = ?3) OR (?5 = 0 AND from_node = ?3 AND to_node = ?2))",
                params![map_id, from_node, to_node, label, directed as i64],
                |r| r.get(0),
            )
            .optional()?;
        if same.is_some() {
            return Err(Error::Invalid("these two nodes are already joined by a link with those words".to_string()));
        }
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO map_link (map_id, from_node, to_node, label, directed) VALUES (?1, ?2, ?3, ?4, ?5)", params![map_id, from_node, to_node, label, directed as i64])?;
        let id = tx.last_insert_rowid();
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "map.link.add",
            Some(("map_link", id)),
            None,
            Some(&json!({ "map_id": map_id, "from": from_node, "to": to_node, "label": label, "directed": directed })),
        )?;
        tx.commit()?;
        Ok(MapLink { id, from_node, to_node, label, directed })
    }

    pub fn update_map_link(&mut self, link_id: i64, label: Option<&str>, directed: Option<bool>) -> Result<MapLink> {
        self.ensure_writable()?;
        let (map_id, from, to, old_label, old_directed): (i64, i64, i64, String, i64) = self
            .conn
            .query_row("SELECT map_id, from_node, to_node, label, directed FROM map_link WHERE id = ?1", [link_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "map link", id: link_id })?;
        let new_label = match label {
            Some(l) => clean_label(l)?,
            None => old_label.clone(),
        };
        let new_directed = directed.map_or(old_directed, |d| d as i64);
        if (&new_label, new_directed) != (&old_label, old_directed) {
            let tx = self.conn.transaction()?;
            tx.execute("UPDATE map_link SET label = ?1, directed = ?2 WHERE id = ?3", params![new_label, new_directed, link_id])?;
            tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "map.link.update",
                Some(("map_link", link_id)),
                Some(&json!({ "label": old_label, "directed": old_directed })),
                Some(&json!({ "label": new_label, "directed": new_directed })),
            )?;
            tx.commit()?;
        }
        Ok(MapLink { id: link_id, from_node: from, to_node: to, label: new_label, directed: new_directed != 0 })
    }

    pub fn delete_map_link(&mut self, link_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let (map_id, from, to, label, directed): (i64, i64, i64, String, i64) = self
            .conn
            .query_row("SELECT map_id, from_node, to_node, label, directed FROM map_link WHERE id = ?1", [link_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .optional()?
            .ok_or(Error::NotFound { what: "map link", id: link_id })?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM map_link WHERE id = ?1", [link_id])?;
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "map.link.delete",
            Some(("map_link", link_id)),
            Some(&json!({ "map_id": map_id, "from": from, "to": to, "label": label, "directed": directed })),
            None,
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Puts codes from the codebook on a map, laid out in a circle round the middle of what is already there, optionally joined
    /// by the codebook's hierarchy (a parent "contains" each child, when both are on the map) and by how often two codes
    /// occur together (overlapping words by the same coder, with the count in the link's words, for pairs at or above `min_together`).
    pub fn add_codes_to_map(&mut self, map_id: i64, code_ids: &[i64], hierarchy_links: bool, min_together: Option<usize>) -> Result<AddedCodes> {
        self.ensure_writable()?;
        self.require_map(map_id)?;
        if code_ids.is_empty() {
            return Err(Error::Invalid("choose at least one code to put on the map".to_string()));
        }
        for id in code_ids {
            name_of(&self.conn, "code", *id)?.ok_or(Error::NotFound { what: "code", id: *id })?;
        }
        let existing: HashMap<i64, i64> = {
            let mut stmt = self.conn.prepare("SELECT ref_id, id FROM map_node WHERE map_id = ?1 AND kind = 'code'")?;
            let rows = stmt.query_map([map_id], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let (cx, cy, count_before): (f64, f64, i64) = self.conn.query_row("SELECT COALESCE(AVG(x), 0), COALESCE(AVG(y), 0), COUNT(*) FROM map_node WHERE map_id = ?1", [map_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        let fresh: Vec<i64> = {
            let mut seen = HashSet::new();
            code_ids.iter().copied().filter(|id| !existing.contains_key(id) && seen.insert(*id)).collect()
        };
        let radius = (fresh.len() as f64 * 60.0).max(160.0);
        let mut node_of: HashMap<i64, i64> = existing.clone();
        let mut created_nodes: Vec<i64> = Vec::new();
        let mut created_links: Vec<i64> = Vec::new();
        let together: HashMap<(i64, i64), i64> = match min_together {
            Some(_) => {
                let r = self.code_relations(&RelationsSpec { code_ids: code_ids.to_vec(), include_descendants: false, mode: RelationMode::Overlap, same_coder: true, scope: Scope::default() })?;
                let mut m = HashMap::new();
                for (i, a) in r.codes.iter().enumerate() {
                    for (j, b) in r.codes.iter().enumerate().skip(i + 1) {
                        m.insert((a.code_id, b.code_id), r.cells[i][j]);
                    }
                }
                m
            }
            None => HashMap::new(),
        };
        let parents: HashMap<i64, Option<i64>> = {
            let mut stmt = self.conn.prepare("SELECT id, parent_id FROM code")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<i64>>(1)?)))?;
            rows.collect::<std::result::Result<_, _>>()?
        };

        let tx = self.conn.transaction()?;
        for (i, code) in fresh.iter().enumerate() {
            let angle = (i as f64) * std::f64::consts::TAU / fresh.len() as f64;
            let (x, y) = if count_before == 0 && fresh.len() == 1 { (tidy(cx), tidy(cy)) } else { (tidy(cx + radius * angle.cos()), tidy(cy + radius * angle.sin())) };
            tx.execute("INSERT INTO map_node (map_id, kind, ref_id, label, x, y) VALUES (?1, 'code', ?2, '', ?3, ?4)", params![map_id, code, x, y])?;
            let id = tx.last_insert_rowid();
            created_nodes.push(id);
            node_of.insert(*code, id);
        }
        let mut add_link = |tx: &rusqlite::Transaction, from: i64, to: i64, label: &str, directed: bool| -> Result<bool> {
            let there: Option<i64> =
                tx.query_row("SELECT id FROM map_link WHERE map_id = ?1 AND ((from_node = ?2 AND to_node = ?3) OR (from_node = ?3 AND to_node = ?2)) AND label = ?4", params![map_id, from, to, label], |r| r.get(0)).optional()?;
            if there.is_some() {
                return Ok(false);
            }
            tx.execute("INSERT INTO map_link (map_id, from_node, to_node, label, directed) VALUES (?1, ?2, ?3, ?4, ?5)", params![map_id, from, to, label, directed as i64])?;
            created_links.push(tx.last_insert_rowid());
            Ok(true)
        };
        if hierarchy_links {
            for (code, node) in &node_of {
                if let Some(Some(parent)) = parents.get(code) {
                    if let Some(parent_node) = node_of.get(parent) {
                        if fresh.contains(code) || fresh.contains(parent) {
                            add_link(&tx, *parent_node, *node, "contains", true)?;
                        }
                    }
                }
            }
        }
        if let Some(min) = min_together {
            for ((a, b), n) in &together {
                if *n as usize >= min.max(1) && (fresh.contains(a) || fresh.contains(b)) {
                    if let (Some(na), Some(nb)) = (node_of.get(a), node_of.get(b)) {
                        add_link(&tx, *na, *nb, &format!("occur together {n} times"), false)?;
                    }
                }
            }
        }
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "map.add_codes",
            Some(("concept_map", map_id)),
            None,
            Some(&json!({ "map_id": map_id, "node_ids": created_nodes, "link_ids": created_links })),
        )?;
        tx.commit()?;
        Ok(AddedCodes { nodes_added: created_nodes.len(), links_added: created_links.len(), already_there: code_ids.iter().collect::<HashSet<_>>().len() - fresh.len() })
    }

    /// Arranges every node of a map: `circle` (round a circle), `grid` (rows of five), or `layers` (arrows pointing down the page: the
    /// nodes nothing points to at the top, each below the longest chain that reaches it). One undo puts every node back.
    pub fn layout_concept_map(&mut self, map_id: i64, how: &str) -> Result<()> {
        self.ensure_writable()?;
        self.require_map(map_id)?;
        if !matches!(how, "circle" | "grid" | "layers") {
            return Err(Error::Invalid(format!("a layout is circle, grid, or layers, not '{how}'")));
        }
        let nodes = self.map_nodes(map_id)?;
        if nodes.is_empty() {
            return Ok(());
        }
        let links = self.map_links(map_id)?;
        let places: Vec<(f64, f64)> = place_nodes(&nodes, &links, how).into_iter().map(|(x, y)| (tidy(x), tidy(y))).collect();
        let before: Vec<Value> = nodes.iter().map(|n| json!({ "id": n.id, "x": n.x, "y": n.y })).collect();
        let after: Vec<Value> = nodes.iter().zip(&places).map(|(n, (x, y))| json!({ "id": n.id, "x": x, "y": y })).collect();
        let tx = self.conn.transaction()?;
        for (n, (x, y)) in nodes.iter().zip(&places) {
            tx.execute("UPDATE map_node SET x = ?1, y = ?2 WHERE id = ?3", params![x, y, n.id])?;
        }
        tx.execute("UPDATE concept_map SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1", [map_id])?;
        event::insert(&tx, &self.actor, &self.session, "map.layout", Some(("concept_map", map_id)), Some(&json!({ "nodes": before })), Some(&json!({ "layout": how, "nodes": after })))?;
        tx.commit()?;
        Ok(())
    }
}

/// Where each node goes for a layout, in the order of `nodes`.
fn place_nodes(nodes: &[MapNode], links: &[MapLink], how: &str) -> Vec<(f64, f64)> {
    let n = nodes.len();
    match how {
        "circle" => {
            let radius = (n as f64 * 55.0).max(160.0);
            (0..n)
                .map(|i| {
                    if n == 1 {
                        return (0.0, 0.0);
                    }
                    let a = i as f64 * std::f64::consts::TAU / n as f64 - std::f64::consts::FRAC_PI_2;
                    (radius * a.cos(), radius * a.sin())
                })
                .collect()
        }
        "grid" => (0..n).map(|i| ((i % 5) as f64 * GAP_X, (i / 5) as f64 * GAP_Y)).collect(),
        _ => {
            // Layers: a node's layer is the longest chain of links that reaches it, found by walking the links `n` times, so a
            // cycle (A to B to A) cannot run on for ever.
            let index: HashMap<i64, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
            let edges: Vec<(usize, usize)> = links
                .iter()
                .filter(|l| l.directed)
                .filter_map(|l| Some((*index.get(&l.from_node)?, *index.get(&l.to_node)?)))
                .collect();
            let mut layer = vec![0usize; n];
            for _ in 0..n {
                let mut changed = false;
                for (a, b) in &edges {
                    if layer[*b] < layer[*a] + 1 && layer[*a] + 1 < n {
                        layer[*b] = layer[*a] + 1;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let depth = layer.iter().copied().max().unwrap_or(0);
            let mut in_layer = vec![0usize; depth + 1];
            let mut width = vec![0usize; depth + 1];
            for l in &layer {
                width[*l] += 1;
            }
            (0..n)
                .map(|i| {
                    let l = layer[i];
                    let slot = in_layer[l];
                    in_layer[l] += 1;
                    let x = (slot as f64 - (width[l] as f64 - 1.0) / 2.0) * GAP_X;
                    (x, l as f64 * GAP_Y)
                })
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: i64) -> MapNode {
        MapNode { id, kind: "note".into(), ref_id: None, label: format!("n{id}"), x: 0.0, y: 0.0, missing: false }
    }

    fn link(id: i64, from: i64, to: i64, directed: bool) -> MapLink {
        MapLink { id, from_node: from, to_node: to, label: String::new(), directed }
    }

    #[test]
    fn layers_put_what_nothing_points_to_at_the_top_and_stay_finite_with_a_cycle() {
        let nodes: Vec<MapNode> = (1..=4).map(node).collect();
        // 1 -> 2 -> 3 and 1 -> 3, and 4 is alone.
        let links = vec![link(1, 1, 2, true), link(2, 2, 3, true), link(3, 1, 3, true)];
        let p = place_nodes(&nodes, &links, "layers");
        assert_eq!((p[0].1, p[1].1, p[2].1, p[3].1), (0.0, GAP_Y, 2.0 * GAP_Y, 0.0), "3 is below 2, which is below 1; the loner is at the top");
        assert!(p[0].0 != p[3].0, "two nodes in one layer do not share a place");
        let cycle = vec![link(1, 1, 2, true), link(2, 2, 1, true)];
        let q = place_nodes(&nodes[..2], &cycle, "layers");
        assert!(q.iter().all(|(x, y)| x.is_finite() && y.is_finite()));
        // An undirected link does not make layers.
        let flat = place_nodes(&nodes[..2], &[link(1, 1, 2, false)], "layers");
        assert_eq!((flat[0].1, flat[1].1), (0.0, 0.0));
    }

    #[test]
    fn circle_and_grid_give_every_node_its_own_place() {
        let nodes: Vec<MapNode> = (1..=7).map(node).collect();
        for how in ["circle", "grid"] {
            let p = place_nodes(&nodes, &[], how);
            let distinct: HashSet<(i64, i64)> = p.iter().map(|(x, y)| (x.round() as i64, y.round() as i64)).collect();
            assert_eq!(distinct.len(), 7, "{how}");
        }
        assert_eq!(place_nodes(&nodes[..1], &[], "circle"), vec![(0.0, 0.0)]);
        let grid = place_nodes(&nodes, &[], "grid");
        assert_eq!((grid[5].0, grid[5].1), (0.0, GAP_Y), "rows of five");
    }
}
