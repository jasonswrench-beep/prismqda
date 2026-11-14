//! Coded areas of a picture or of a PDF page, and coded stretches of a recording (specification 22.1, 22.2 and 8.2). An area is a
//! rectangle, an ellipse, or a polygon drawn on an image document (on a page of a PDF it is always a rectangle), and a code is given to
//! it as a code is given to a stretch of text. A stretch of a recording is a `span`: from one moment to another, in milliseconds from the
//! start of the recording, on a document that has a recording linked. A row of `region` is one code given to one area (or stretch) by one
//! coder, so a picture coded with two codes in the same place has two rows with the same shape. A picture has one page, the first; a PDF
//! has as many as the file has, and `page` says which an area is on.
//!
//! Coordinates are fractions of the picture (or the page) as it is shown: 0 is the left or top edge and 1 is the right or bottom edge. Shapes are
//! rounded to five decimals and written one way, so two areas that are the same shape read as equal and count as one excerpt. The times
//! of a span are whole milliseconds, so they need no rounding.

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::event::{self, HistoryEntry};
use crate::ops::{exclusive_message, GroupConflict};
use crate::project::Project;
use crate::{Error, Result};

/// The fewest corners a polygon may have, and the most (a freehand outline is simplified before it gets here).
pub const MIN_POINTS: usize = 3;
pub const MAX_POINTS: usize = 500;
/// The smallest a rectangle or ellipse may be along either side, as a fraction of the picture.
pub const MIN_SIDE: f64 = 0.002;
/// The shortest stretch of a recording that can be coded, in milliseconds.
pub const MIN_SPAN_MS: i64 = 100;
/// The latest a stretch of a recording may end, in milliseconds (seven days), so a mistyped number cannot be stored.
pub const MAX_SPAN_END_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// An area of a picture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Shape {
    /// `x`, `y` is the top left corner; `w` and `h` are the width and height.
    Rectangle { x: f64, y: f64, w: f64, h: f64 },
    /// The ellipse that fits inside the box `x`, `y`, `w`, `h`.
    Ellipse { x: f64, y: f64, w: f64, h: f64 },
    /// A closed outline through these points, in order.
    Polygon { points: Vec<[f64; 2]> },
    /// A stretch of a recording: from `start_ms` to `end_ms`, in milliseconds from the start of the recording.
    Span { start_ms: i64, end_ms: i64 },
}

fn round5(v: f64) -> f64 {
    (v * 100_000.0).round() / 100_000.0
}

fn unit_coordinate(v: f64, what: &str) -> Result<f64> {
    if !v.is_finite() || !(-1e-9..=1.0 + 1e-9).contains(&v) {
        return Err(Error::Invalid(format!("{what} must be between 0 and 1 (a fraction of the picture), not {v}")));
    }
    Ok(round5(v.clamp(0.0, 1.0)))
}

/// A time of a recording written as a clock: `1:05`, `0:07.4`, or `1:02:03` once it passes an hour. Tenths of a second are shown when
/// `tenths` is set (the same count of digits is used for both ends of a stretch, so they line up).
pub fn clock(ms: i64, tenths: bool) -> String {
    let ms = ms.max(0);
    let total = ms / 1000;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    let fraction = if tenths { format!(".{}", (ms % 1000) / 100) } else { String::new() };
    if h > 0 {
        format!("{h}:{m:02}:{s:02}{fraction}")
    } else {
        format!("{m}:{s:02}{fraction}")
    }
}

/// Both ends of a stretch as clocks, with tenths when either end is not a whole second.
pub fn clock_pair(start_ms: i64, end_ms: i64) -> (String, String) {
    let tenths = start_ms % 1000 >= 100 || end_ms % 1000 >= 100;
    (clock(start_ms, tenths), clock(end_ms, tenths))
}

impl Shape {
    /// `rectangle`, `ellipse`, `polygon`, or `span`.
    pub fn name(&self) -> &'static str {
        match self {
            Shape::Rectangle { .. } => "rectangle",
            Shape::Ellipse { .. } => "ellipse",
            Shape::Polygon { .. } => "polygon",
            Shape::Span { .. } => "span",
        }
    }

    /// True for a stretch of a recording.
    pub fn is_span(&self) -> bool {
        matches!(self, Shape::Span { .. })
    }

    /// The same shape with every number checked and rounded, or the reason it cannot be an area.
    pub fn checked(&self) -> Result<Shape> {
        match self {
            Shape::Rectangle { x, y, w, h } | Shape::Ellipse { x, y, w, h } => {
                let (x, y) = (unit_coordinate(*x, "the left edge")?, unit_coordinate(*y, "the top edge")?);
                let (w, h) = (round5(*w), round5(*h));
                if !w.is_finite() || !h.is_finite() || w < MIN_SIDE || h < MIN_SIDE {
                    return Err(Error::Invalid("the area is too small to code: each side must be at least 0.2% of the picture".to_string()));
                }
                if x + w > 1.0 + 1e-9 || y + h > 1.0 + 1e-9 {
                    return Err(Error::Invalid("the area reaches past the edge of the picture".to_string()));
                }
                let (w, h) = (w.min(1.0 - x), h.min(1.0 - y));
                Ok(if matches!(self, Shape::Rectangle { .. }) { Shape::Rectangle { x, y, w, h } } else { Shape::Ellipse { x, y, w, h } })
            }
            Shape::Polygon { points } => {
                if points.len() < MIN_POINTS {
                    return Err(Error::Invalid(format!("an outline needs at least {MIN_POINTS} points")));
                }
                if points.len() > MAX_POINTS {
                    return Err(Error::Invalid(format!("an outline has at most {MAX_POINTS} points")));
                }
                let mut out = Vec::with_capacity(points.len());
                for (i, p) in points.iter().enumerate() {
                    out.push([unit_coordinate(p[0], &format!("point {} across", i + 1))?, unit_coordinate(p[1], &format!("point {} down", i + 1))?]);
                }
                let shape = Shape::Polygon { points: out };
                let (x0, y0, x1, y1) = shape.bbox();
                if x1 - x0 < MIN_SIDE || y1 - y0 < MIN_SIDE || shape.area() < 1e-6 {
                    return Err(Error::Invalid("the outline is too thin to code: it must enclose some area".to_string()));
                }
                Ok(shape)
            }
            Shape::Span { start_ms, end_ms } => {
                if *start_ms < 0 {
                    return Err(Error::Invalid("a stretch of a recording cannot start before the recording does".to_string()));
                }
                if *end_ms <= *start_ms {
                    return Err(Error::Invalid("a stretch of a recording must end after it starts".to_string()));
                }
                if end_ms - start_ms < MIN_SPAN_MS {
                    return Err(Error::Invalid("the stretch is too short to code: it must be at least a tenth of a second".to_string()));
                }
                if *end_ms > MAX_SPAN_END_MS {
                    return Err(Error::Invalid("the stretch ends more than seven days into the recording, which is not a time on any recording".to_string()));
                }
                Ok(self.clone())
            }
        }
    }

    /// The box around the shape: left, top, right, bottom.
    pub fn bbox(&self) -> (f64, f64, f64, f64) {
        match self {
            Shape::Rectangle { x, y, w, h } | Shape::Ellipse { x, y, w, h } => (*x, *y, x + w, y + h),
            Shape::Polygon { points } => {
                let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for p in points {
                    b = (b.0.min(p[0]), b.1.min(p[1]), b.2.max(p[0]), b.3.max(p[1]));
                }
                b
            }
            // A stretch has no height: its "box" runs from its start to its end in milliseconds, and from 0 to 1 the other way.
            Shape::Span { start_ms, end_ms } => (*start_ms as f64, 0.0, *end_ms as f64, 1.0),
        }
    }

    /// The area enclosed, as a fraction of the picture; for a stretch of a recording, its length in seconds.
    pub fn area(&self) -> f64 {
        match self {
            Shape::Span { start_ms, end_ms } => (end_ms - start_ms) as f64 / 1000.0,
            Shape::Rectangle { w, h, .. } => w * h,
            Shape::Ellipse { w, h, .. } => std::f64::consts::PI * w * h / 4.0,
            Shape::Polygon { points } => {
                let mut sum = 0.0;
                for i in 0..points.len() {
                    let (a, b) = (points[i], points[(i + 1) % points.len()]);
                    sum += a[0] * b[1] - b[0] * a[1];
                }
                (sum / 2.0).abs()
            }
        }
    }

    /// True when the point is inside the shape (a polygon is filled by the even-odd rule, so a outline that crosses itself leaves a hole).
    pub fn contains(&self, px: f64, py: f64) -> bool {
        match self {
            // For a stretch `px` is a time in milliseconds and `py` is ignored.
            Shape::Span { start_ms, end_ms } => px >= *start_ms as f64 && px <= *end_ms as f64,
            Shape::Rectangle { x, y, w, h } => px >= *x && px <= x + w && py >= *y && py <= y + h,
            Shape::Ellipse { x, y, w, h } => {
                let (rx, ry) = (w / 2.0, h / 2.0);
                let (dx, dy) = ((px - (x + rx)) / rx, (py - (y + ry)) / ry);
                dx * dx + dy * dy <= 1.0
            }
            Shape::Polygon { points } => {
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let (a, b) = (points[i], points[j]);
                    if (a[1] > py) != (b[1] > py) && px < (b[0] - a[0]) * (py - a[1]) / (b[1] - a[1]) + a[0] {
                        inside = !inside;
                    }
                    j = i;
                }
                inside
            }
        }
    }

    /// The shape written one way (rounded, fixed order of keys), so equal shapes are equal text.
    pub fn canonical_json(&self) -> String {
        serde_json::to_string(self).expect("a shape is always JSON")
    }

    /// The shape in words, for lists, tables, and exports that cannot show a picture: where it is and how much of the picture it takes.
    pub fn describe(&self) -> String {
        self.describe_on(None)
    }

    /// The same in words for an area of a PDF page when `page` is given ("Page 3, Rectangle, ... of the page").
    pub fn describe_on(&self, page: Option<i64>) -> String {
        if let Shape::Span { start_ms, end_ms } = self {
            let (from, to) = clock_pair(*start_ms, *end_ms);
            let length = clock(end_ms - start_ms, (end_ms - start_ms) % 1000 >= 100);
            return format!("Recording, {from} to {to} ({length} long)");
        }
        let (x0, y0, x1, y1) = self.bbox();
        let pct = |v: f64| (v * 100.0).round() as i64;
        let kind = match self {
            Shape::Rectangle { .. } => "Rectangle".to_string(),
            Shape::Ellipse { .. } => "Ellipse".to_string(),
            Shape::Polygon { points } => format!("Outline of {} points", points.len()),
            Shape::Span { .. } => unreachable!("a stretch is told in words above"),
        };
        let share = (self.area() * 100.0).round().max(if self.area() > 0.0 { 1.0 } else { 0.0 }) as i64;
        match page {
            Some(p) => format!("Page {p}, {kind}, {}% to {}% across and {}% to {}% down, {share}% of the page", pct(x0), pct(x1), pct(y0), pct(y1)),
            None => format!("{kind}, {}% to {}% across and {}% to {}% down, {share}% of the picture", pct(x0), pct(x1), pct(y0), pct(y1)),
        }
    }
}

/// True when two areas share some part of the picture. A rectangle against a rectangle is exact; anything else is found by testing
/// a grid of 48 by 48 points inside the box the two share, which finds any overlap wider than about two percent of that box.
pub fn overlaps(a: &Shape, b: &Shape) -> bool {
    // Two stretches of a recording overlap when they share some time (stretches that only touch do not); a stretch never overlaps an area.
    match (a, b) {
        (Shape::Span { start_ms: s0, end_ms: e0 }, Shape::Span { start_ms: s1, end_ms: e1 }) => return s0 < e1 && s1 < e0,
        (Shape::Span { .. }, _) | (_, Shape::Span { .. }) => return false,
        _ => {}
    }
    let (a0, a1, a2, a3) = a.bbox();
    let (b0, b1, b2, b3) = b.bbox();
    let (x0, y0, x1, y1) = (a0.max(b0), a1.max(b1), a2.min(b2), a3.min(b3));
    if x1 <= x0 || y1 <= y0 {
        return false;
    }
    if matches!((a, b), (Shape::Rectangle { .. }, Shape::Rectangle { .. })) {
        return true;
    }
    const N: usize = 48;
    for i in 0..N {
        let px = x0 + (i as f64 + 0.5) / N as f64 * (x1 - x0);
        for j in 0..N {
            let py = y0 + (j as f64 + 0.5) / N as f64 * (y1 - y0);
            if a.contains(px, py) && b.contains(px, py) {
                return true;
            }
        }
    }
    false
}

/// One code given to one area of a picture.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RegionRow {
    pub id: i64,
    pub source_id: i64,
    /// The page of the PDF the area is on, counting from 1; always 1 for a picture.
    pub page: i64,
    #[serde(flatten)]
    pub shape: Shape,
    pub code_id: i64,
    pub coder_id: i64,
    pub origin: String,
    pub layer: String,
    pub rationale: String,
    /// A rating from 1 to 5, or null when the coding has not been rated.
    pub rating: Option<i64>,
    pub created_at: String,
    /// The area in words, for places that list codings as text.
    pub text: String,
}

/// A code to give to an area.
#[derive(Debug, Clone)]
pub struct NewRegion {
    pub source_id: i64,
    /// The page of a PDF the area is on; 1 for a picture.
    pub page: i64,
    pub shape: Shape,
    pub code_id: i64,
    pub coder_id: i64,
    pub rationale: String,
}

impl NewRegion {
    pub fn new(source_id: i64, shape: Shape, code_id: i64, coder_id: i64) -> Self {
        Self { source_id, page: 1, shape, code_id, coder_id, rationale: String::new() }
    }
}

const REGION_COLUMNS: &str = "id, source_id, geometry_json, code_id, coder_id, origin, layer, rationale, rating, created_at, page,
     EXISTS (SELECT 1 FROM pdf_file pf WHERE pf.source_id = region.source_id)";

fn region_from_row(r: &rusqlite::Row) -> rusqlite::Result<RegionRow> {
    let geometry: String = r.get(2)?;
    let shape: Shape = serde_json::from_str(&geometry).map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e)))?;
    let page: i64 = r.get(10)?;
    let on_pdf: bool = r.get(11)?;
    let text = shape.describe_on(on_pdf.then_some(page));
    Ok(RegionRow {
        id: r.get(0)?,
        source_id: r.get(1)?,
        page,
        shape,
        code_id: r.get(3)?,
        coder_id: r.get(4)?,
        origin: r.get(5)?,
        layer: r.get(6)?,
        rationale: r.get(7)?,
        rating: r.get(8)?,
        created_at: r.get(9)?,
        text,
    })
}

fn region_json(r: &RegionRow) -> Value {
    json!({
        "source_id": r.source_id, "page": r.page, "geometry": serde_json::to_value(&r.shape).unwrap_or(Value::Null),
        "code_id": r.code_id, "coder_id": r.coder_id, "origin": r.origin, "layer": r.layer, "rationale": r.rationale, "rating": r.rating,
    })
}

fn get_region_in(tx: &Transaction, id: i64) -> Result<RegionRow> {
    tx.query_row(&format!("SELECT {REGION_COLUMNS} FROM region WHERE id = ?1"), [id], region_from_row)
        .optional()?
        .ok_or(Error::NotFound { what: "region", id })
}

/// Puts a row in. `id` is given when an undo puts a removed one back under its old number.
#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_region_row(
    tx: &Transaction,
    id: Option<i64>,
    source_id: i64,
    page: i64,
    shape: &Shape,
    code_id: i64,
    coder_id: i64,
    origin: &str,
    layer: &str,
    rationale: &str,
    rating: Option<i64>,
) -> Result<i64> {
    let (x0, y0, x1, y1) = shape.bbox();
    tx.execute(
        "INSERT INTO region (id, source_id, page, shape, geometry_json, x0, y0, x1, y1, code_id, coder_id, origin, layer, rationale, rating, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![id, source_id, page, shape.name(), shape.canonical_json(), x0, y0, x1, y1, code_id, coder_id, origin, layer, rationale, rating],
    )?;
    Ok(id.unwrap_or_else(|| tx.last_insert_rowid()))
}

/// The regions by this coder on this layer, of the same picture and page, that share part of it with `shape` and whose active code
/// is in the same exclusive group as `code_id` (a different code of it).
#[allow(clippy::too_many_arguments)]
fn region_conflicts(tx: &Transaction, source_id: i64, page: i64, shape: &Shape, code_id: i64, coder_id: i64, layer: &str, except: Option<i64>) -> Result<Vec<GroupConflict>> {
    let mut stmt = tx.prepare(
        "SELECT g.id, g.code_id, c2.name, c1.exclusive_group, g.geometry_json
         FROM code c1, region g JOIN code c2 ON c2.id = g.code_id
         WHERE c1.id = ?1 AND c1.exclusive_group <> ''
           AND g.source_id = ?2 AND g.coder_id = ?3 AND g.layer = ?4 AND g.page = ?6
           AND g.code_id <> ?1 AND c2.status = 'active' AND lower(c2.exclusive_group) = lower(c1.exclusive_group)
           AND (?5 IS NULL OR g.id <> ?5)
         ORDER BY g.id",
    )?;
    let rows = stmt.query_map(params![code_id, source_id, coder_id, layer, except, page], |r| {
        Ok((GroupConflict { coding_id: r.get(0)?, code_id: r.get(1)?, code_name: r.get(2)?, group: r.get(3)? }, r.get::<_, String>(4)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (conflict, geometry) = row?;
        if let Ok(other) = serde_json::from_str::<Shape>(&geometry) {
            if overlaps(shape, &other) {
                out.push(conflict);
            }
        }
    }
    Ok(out)
}

impl Project {
    /// The areas coded on one picture or PDF, in reading order (by page, then top to bottom, then left to right).
    pub fn list_regions(&self, source_id: i64) -> Result<Vec<RegionRow>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {REGION_COLUMNS} FROM region WHERE source_id = ?1 ORDER BY page, y0, x0, id"))?;
        let rows = stmt.query_map([source_id], region_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Every coded area in the project, by picture and then in reading order.
    pub fn all_regions(&self) -> Result<Vec<RegionRow>> {
        let mut stmt = self.conn.prepare(&format!("SELECT {REGION_COLUMNS} FROM region ORDER BY source_id, page, y0, x0, id"))?;
        let rows = stmt.query_map([], region_from_row)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn get_region(&self, id: i64) -> Result<RegionRow> {
        self.conn
            .query_row(&format!("SELECT {REGION_COLUMNS} FROM region WHERE id = ?1"), [id], region_from_row)
            .optional()?
            .ok_or(Error::NotFound { what: "region", id })
    }

    /// Gives a code to an area of a picture, of a page of a PDF, or to a stretch of a recording. Refuses an area outside the picture or
    /// too small, a document that is neither a picture nor a PDF that keeps its pages, a page the PDF does not have, any shape but a
    /// rectangle on a PDF page, a stretch of a document that has no recording linked (and an area of one that has no picture), a code that
    /// is not active, and (when the code is in a mutually exclusive group) an area that shares ground with another code of the group that
    /// the same coder has given.
    pub fn apply_region(&mut self, new: &NewRegion) -> Result<i64> {
        self.ensure_writable()?;
        let shape = new.shape.checked()?;
        let tx = self.conn.transaction()?;
        if shape.is_span() {
            let has_recording: Option<i64> = tx.query_row("SELECT source_id FROM media WHERE source_id = ?1", [new.source_id], |r| r.get(0)).optional()?;
            if has_recording.is_none() {
                let exists: Option<i64> = tx.query_row("SELECT id FROM source WHERE id = ?1", [new.source_id], |r| r.get(0)).optional()?;
                exists.ok_or(Error::NotFound { what: "source", id: new.source_id })?;
                return Err(Error::Invalid("this document has no recording linked, so a stretch of it cannot be coded; link the recording to it first, or select words instead".to_string()));
            }
            if new.page != 1 {
                return Err(Error::Invalid("a recording has no pages".to_string()));
            }
        } else {
            let is_image: Option<i64> = tx.query_row("SELECT source_id FROM image WHERE source_id = ?1", [new.source_id], |r| r.get(0)).optional()?;
            let pages: Option<i64> = tx.query_row("SELECT page_count FROM pdf_file WHERE source_id = ?1", [new.source_id], |r| r.get(0)).optional()?;
            if is_image.is_none() && pages.is_none() {
                let exists: Option<i64> = tx.query_row("SELECT id FROM source WHERE id = ?1", [new.source_id], |r| r.get(0)).optional()?;
                exists.ok_or(Error::NotFound { what: "source", id: new.source_id })?;
                return Err(Error::Invalid("this document is not a picture or a PDF whose pages are kept, so an area cannot be coded; select words instead".to_string()));
            }
            match pages {
                Some(count) => {
                    if new.page < 1 || new.page > count {
                        return Err(Error::Invalid(format!("this PDF has {count} pages, so there is no page {}", new.page)));
                    }
                    if !matches!(shape, Shape::Rectangle { .. }) {
                        return Err(Error::Invalid("an area of a PDF page is a rectangle".to_string()));
                    }
                }
                None => {
                    if new.page != 1 {
                        return Err(Error::Invalid("a picture has one page".to_string()));
                    }
                }
            }
        }
        let status: Option<String> = tx.query_row("SELECT status FROM code WHERE id = ?1", [new.code_id], |r| r.get(0)).optional()?;
        match status.as_deref() {
            None => return Err(Error::NotFound { what: "code", id: new.code_id }),
            Some("active") => {}
            Some(other) => return Err(Error::Invalid(format!("code {} is {other}, so it cannot be applied", new.code_id))),
        }
        let coder: Option<i64> = tx.query_row("SELECT id FROM coder WHERE id = ?1", [new.coder_id], |r| r.get(0)).optional()?;
        coder.ok_or(Error::NotFound { what: "coder", id: new.coder_id })?;
        let conflicts = region_conflicts(&tx, new.source_id, new.page, &shape, new.code_id, new.coder_id, "coder", None)?;
        if !conflicts.is_empty() {
            let name: String = tx.query_row("SELECT name FROM code WHERE id = ?1", [new.code_id], |r| r.get(0))?;
            return Err(Error::ExclusiveGroup(exclusive_message(&name, &conflicts).replace("these words already have", "this area already has")));
        }
        let id = insert_region_row(&tx, None, new.source_id, new.page, &shape, new.code_id, new.coder_id, "human", "coder", &new.rationale, None)?;
        let row = get_region_in(&tx, id)?;
        event::insert(&tx, &self.actor, &self.session, "region.add", Some(("region", id)), None, Some(&region_json(&row)))?;
        tx.commit()?;
        Ok(id)
    }

    /// Removes a coded area. The removal is logged with everything the area was, never silently.
    pub fn remove_region(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let row = get_region_in(&tx, id)?;
        // a decision about the area goes with it, kept here so that Undo can put it back
        let decision: Option<serde_json::Value> = tx
            .query_row("SELECT adjudicator_id, accepted, reason, created_at FROM region_decision WHERE region_id = ?1", [id], |r| {
                Ok(serde_json::json!({ "adjudicator_id": r.get::<_, i64>(0)?, "accepted": r.get::<_, i64>(1)? != 0, "reason": r.get::<_, String>(2)?, "created_at": r.get::<_, String>(3)? }))
            })
            .optional()?;
        tx.execute("DELETE FROM region_decision WHERE region_id = ?1", [id])?;
        tx.execute("DELETE FROM region WHERE id = ?1", [id])?;
        // The area's links go with it (an area is its number made negative among passages), kept here so that Undo can put them back.
        let mut before = region_json(&row);
        if let Some(d) = decision {
            before["decision"] = d;
        }
        let links = crate::passage_links::take_links(&tx, -id)?;
        if !links.is_empty() {
            before["links"] = serde_json::Value::Array(links);
        }
        event::insert(&tx, &self.actor, &self.session, "region.remove", Some(("region", id)), Some(&before), None)?;
        tx.commit()?;
        Ok(())
    }

    /// Changes which code an area has.
    pub fn change_region_code(&mut self, id: i64, new_code_id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let row = get_region_in(&tx, id)?;
        let target: Option<(String, String, i64)> = tx
            .query_row("SELECT status, name, rated FROM code WHERE id = ?1", [new_code_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let (status, name, rated) = target.ok_or(Error::NotFound { what: "code", id: new_code_id })?;
        if status != "active" {
            return Err(Error::Invalid(format!("code {new_code_id} is {status}, so it cannot be applied")));
        }
        let conflicts = region_conflicts(&tx, row.source_id, row.page, &row.shape, new_code_id, row.coder_id, &row.layer, Some(id))?;
        if !conflicts.is_empty() {
            return Err(Error::ExclusiveGroup(exclusive_message(&name, &conflicts).replace("these words already have", "this area already has")));
        }
        let kept = if rated != 0 { row.rating } else { None };
        tx.execute("UPDATE region SET code_id = ?1, rating = ?2 WHERE id = ?3", params![new_code_id, kept, id])?;
        let (before, after) = if kept == row.rating {
            (json!({ "code_id": row.code_id }), json!({ "code_id": new_code_id }))
        } else {
            (json!({ "code_id": row.code_id, "rating": row.rating }), json!({ "code_id": new_code_id, "rating": kept }))
        };
        event::insert(&tx, &self.actor, &self.session, "region.change_code", Some(("region", id)), Some(&before), Some(&after))?;
        tx.commit()?;
        Ok(())
    }

    /// Moves or resizes an area (its shape changes; its code and coder stay).
    pub fn reshape_region(&mut self, id: i64, shape: &Shape) -> Result<RegionRow> {
        self.ensure_writable()?;
        let shape = shape.checked()?;
        let tx = self.conn.transaction()?;
        let row = get_region_in(&tx, id)?;
        if row.shape == shape {
            return Ok(row);
        }
        if shape.name() != row.shape.name() {
            return Err(Error::Invalid(format!("a {} cannot become a {}; draw a new area instead", row.shape.name(), shape.name())));
        }
        let conflicts = region_conflicts(&tx, row.source_id, row.page, &shape, row.code_id, row.coder_id, &row.layer, Some(id))?;
        if !conflicts.is_empty() {
            let name: String = tx.query_row("SELECT name FROM code WHERE id = ?1", [row.code_id], |r| r.get(0))?;
            return Err(Error::ExclusiveGroup(exclusive_message(&name, &conflicts).replace("these words already have", "this area already has")));
        }
        let (x0, y0, x1, y1) = shape.bbox();
        tx.execute(
            "UPDATE region SET geometry_json = ?1, x0 = ?2, y0 = ?3, x1 = ?4, y1 = ?5 WHERE id = ?6",
            params![shape.canonical_json(), x0, y0, x1, y1, id],
        )?;
        let after = get_region_in(&tx, id)?;
        event::insert(
            &tx,
            &self.actor,
            &self.session,
            "region.reshape",
            Some(("region", id)),
            Some(&json!({ "geometry": serde_json::to_value(&row.shape)? })),
            Some(&json!({ "geometry": serde_json::to_value(&after.shape)? })),
        )?;
        tx.commit()?;
        Ok(after)
    }

    /// Sets the note that explains why an area was coded.
    pub fn set_region_rationale(&mut self, id: i64, rationale: &str) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let old = get_region_in(&tx, id)?.rationale;
        if old == rationale {
            return Ok(());
        }
        tx.execute("UPDATE region SET rationale = ?1 WHERE id = ?2", params![rationale, id])?;
        event::insert(&tx, &self.actor, &self.session, "region.rationale", Some(("region", id)), Some(&json!({ "rationale": old })), Some(&json!({ "rationale": rationale })))?;
        tx.commit()?;
        Ok(())
    }

    /// Gives an area a rating from 1 to 5, or clears it with `None`.
    pub fn set_region_rating(&mut self, id: i64, rating: Option<i64>) -> Result<()> {
        self.ensure_writable()?;
        if let Some(r) = rating {
            if !(1..=5).contains(&r) {
                return Err(Error::Invalid(format!("a rating is a whole number from 1 to 5 (or none), not {r}")));
            }
        }
        let tx = self.conn.transaction()?;
        let row: Option<(Option<i64>, i64, String)> = tx
            .query_row("SELECT g.rating, c.rated, c.name FROM region g JOIN code c ON c.id = g.code_id WHERE g.id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()?;
        let (old, rated, name) = row.ok_or(Error::NotFound { what: "region", id })?;
        if old == rating {
            return Ok(());
        }
        if rating.is_some() && rated == 0 {
            return Err(Error::Invalid(format!("'{name}' is not a rated code, so its codings cannot be rated. Turn on Weighted / Rated for the code in the Codebook Editor first.")));
        }
        tx.execute("UPDATE region SET rating = ?1 WHERE id = ?2", params![rating, id])?;
        event::insert(&tx, &self.actor, &self.session, "region.rating", Some(("region", id)), Some(&json!({ "rating": old })), Some(&json!({ "rating": rating })))?;
        tx.commit()?;
        Ok(())
    }

    /// Everything that happened to one coded area, oldest first.
    pub fn region_history(&self, id: i64) -> Result<Vec<HistoryEntry>> {
        event::for_region(&self.conn, id)
    }
}
