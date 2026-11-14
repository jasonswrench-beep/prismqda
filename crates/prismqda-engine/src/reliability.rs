//! Reliability (specification 18.3): drawing a sample, measuring agreement between coders on it, and
//! reporting the result with every coder named and labeled honestly.
//!
//! Agreement is measured on predefined units. For each unit in the sample, each coder either coded it
//! (they have at least one coding on it) or did not. A coder who coded a unit without applying a
//! particular code has "not applied" it; a coder who never coded the unit has no rating there at all.
//!
//! Two views are offered:
//! * **One code per unit** (`single_label`): each coder gave each unit exactly one code, as in content
//!   analysis. The coefficients are computed across the codes, with a confusion table for two coders.
//! * **Each code on its own** (`per_code`): for every code, did the coder apply it? Several codes may
//!   apply to one unit. The coefficients are computed per code, with prevalence and bias indices.

use std::collections::{BTreeMap, HashMap, HashSet};

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use prismqda_stats::{
    agreement, bootstrap_intervals, code_agreement, confusion_table, Agreement, CodeAgreement, Coefficient, Interval, Ratings, SplitMix64,
};

use crate::event;
use crate::project::Project;
use crate::regions::Shape;
use crate::{Error, Result};

/// A picture is compared by laying a grid of this many cells along each side over it. A cell is a unit of its own, and a coder has coded
/// it with a code when the middle of the cell lies inside an area that coder drew with that code.
pub const AREA_GRID: usize = 16;

/// The number a cell of a picture (or of one page of a PDF) goes by among the units. Real units have positive numbers, so a cell's is
/// negative and never meets one.
fn cell_unit(source_id: i64, page: i64, cell: usize) -> i64 {
    -((source_id * 20_001 + page) * 100_000 + cell as i64 + 1)
}

/// A recording is compared by cutting it into cells this long. A cell is a unit of its own, and a coder has coded it with a code when the
/// middle of the cell lies inside a stretch that coder gave the code. A stretch shorter than a cell may cover none.
pub const SPAN_CELL_MS: i64 = 1000;
/// The most cells of one recording that are compared (a recording longer than this many cells is compared up to there).
const MAX_SPAN_CELLS: i64 = 99_999;

/// The cells of the grid (or of the recording) whose middle lies inside the shape. An area smaller than one cell may cover none.
fn covered_cells(shape: &Shape) -> Vec<usize> {
    if let Shape::Span { start_ms, end_ms } = shape {
        // The cell `n` runs from n*1000 to (n+1)*1000, and its middle is at n*1000 + 500.
        let first = (start_ms - SPAN_CELL_MS / 2 + SPAN_CELL_MS - 1).div_euclid(SPAN_CELL_MS).max(0);
        let last = (end_ms - SPAN_CELL_MS / 2).div_euclid(SPAN_CELL_MS).min(MAX_SPAN_CELLS - 1);
        return (first..=last).filter(|n| *n >= 0).map(|n| n as usize).collect();
    }
    let (x0, y0, x1, y1) = shape.bbox();
    let mut cells = Vec::new();
    for row in 0..AREA_GRID {
        let y = (row as f64 + 0.5) / AREA_GRID as f64;
        if y < y0 || y > y1 {
            continue;
        }
        for col in 0..AREA_GRID {
            let x = (col as f64 + 0.5) / AREA_GRID as f64;
            if x >= x0 && x <= x1 && shape.contains(x, y) {
                cells.push(row * AREA_GRID + col);
            }
        }
    }
    cells
}

/// How to divide the units before sampling, so each group is represented in proportion.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "by", content = "attribute")]
pub enum Stratify {
    #[default]
    None,
    /// By source.
    Source,
    /// By the value of a unit attribute, such as `speaker`.
    Attribute(String),
}

/// A sample of units, drawn so it can be drawn again exactly.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
    pub seed: u64,
    pub size: usize,
    pub pool: usize,
    pub unit_ids: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgreementRequest {
    /// The coders to compare; at least two.
    pub coder_ids: Vec<i64>,
    /// The units to measure on. When absent, every unit at least two of these coders coded.
    #[serde(default)]
    pub unit_ids: Option<Vec<i64>>,
    /// `single_label` (one code per unit) or `per_code` (each code on its own). Default `single_label`.
    #[serde(default = "default_mode")]
    pub mode: String,
    /// The word the report uses: `intercoder_reliability` or `concordance`. When absent, a run that includes a
    /// model coder is called concordance, because agreement with a model is not reliability between people.
    #[serde(default)]
    pub reporting_term: Option<String>,
    #[serde(default = "default_reps")]
    pub bootstrap_replications: usize,
    #[serde(default = "default_seed")]
    pub seed: u64,
    /// Keep the run in the project (default true).
    #[serde(default = "yes")]
    pub save: bool,
    /// Also compare the areas coders drew on pictures, cell by cell on a grid (default true). Used only when `unit_ids` is absent.
    #[serde(default = "yes")]
    pub areas: bool,
}

fn default_mode() -> String {
    "single_label".to_string()
}
fn default_reps() -> usize {
    1000
}
fn default_seed() -> u64 {
    20_261_105
}
fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CoderInfo {
    pub id: i64,
    pub name: String,
    pub role: String,
    pub model_name: Option<String>,
    pub model_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeReport {
    pub code_id: i64,
    pub name: String,
    pub detail: CodeAgreement,
    pub below_target: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgreementReport {
    pub run_id: Option<i64>,
    pub mode: String,
    pub reporting_term: String,
    pub coders: Vec<CoderInfo>,
    pub units_in_sample: usize,
    /// Across all codes (single-label runs), or `None` for a per-code run.
    pub overall: Option<Agreement>,
    pub intervals: BTreeMap<String, Interval>,
    pub target: f64,
    /// Codes under the target, worst first (per-code runs).
    pub by_code: Vec<CodeReport>,
    /// For two coders in a single-label run: rows are the first coder's codes, columns the second's.
    pub confusion: Option<Vec<Vec<usize>>>,
    pub confusion_codes: Vec<String>,
    /// How many units the coders disagreed on (single-label, two coders).
    pub disagreements: usize,
    pub notes: Vec<String>,
    pub report_text: String,
}

fn coefficient_name(c: Coefficient) -> &'static str {
    match c {
        Coefficient::PercentAgreement => "percent_agreement",
        Coefficient::CohenKappa => "cohen_kappa",
        Coefficient::ScottPi => "scott_pi",
        Coefficient::FleissKappa => "fleiss_kappa",
        Coefficient::BrennanPrediger => "brennan_prediger",
        Coefficient::GwetAc1 => "gwet_ac1",
        Coefficient::KrippendorffAlpha => "krippendorff_alpha",
    }
}

fn pct(x: Option<f64>) -> String {
    x.map(|v| format!("{v:.3}")).unwrap_or_else(|| "undefined".to_string())
}

impl Project {
    /// Draws a random sample of units, the same way every time for the same seed. Units left out of
    /// coding are never drawn. Stratified draws take from each group in proportion to its size.
    pub fn draw_sample(&self, size: usize, seed: u64, stratify: &Stratify, source_ids: Option<&[i64]>) -> Result<Sample> {
        if size == 0 {
            return Err(Error::Invalid("a sample needs at least one unit".to_string()));
        }
        // Each unit with the group it belongs to.
        let mut stmt = self.conn.prepare("SELECT id, source_id FROM unit WHERE excluded = 0 ORDER BY id")?;
        let units: Vec<(i64, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        let units: Vec<(i64, i64)> = units.into_iter().filter(|(_, s)| source_ids.is_none_or(|ids| ids.contains(s))).collect();
        let group_of: HashMap<i64, String> = match stratify {
            Stratify::None => HashMap::new(),
            Stratify::Source => units.iter().map(|(u, s)| (*u, s.to_string())).collect(),
            Stratify::Attribute(name) => {
                let mut a = self.conn.prepare(
                    "SELECT av.target_id, COALESCE(av.value, '') FROM attribute_value av
                     JOIN attribute a ON a.id = av.attribute_id AND a.applies_to = 'unit' AND a.name = ?1",
                )?;
                let rows = a.query_map([name], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
                rows.collect::<std::result::Result<_, _>>()?
            }
        };
        let pool = units.len();
        if size > pool {
            return Err(Error::Invalid(format!("the sample asked for {size} units, but only {pool} are available")));
        }
        let mut rng = SplitMix64::new(seed);
        let mut chosen: Vec<i64> = Vec::with_capacity(size);
        if matches!(stratify, Stratify::None) {
            chosen = pick(&units.iter().map(|(u, _)| *u).collect::<Vec<_>>(), size, &mut rng);
        } else {
            let mut groups: BTreeMap<String, Vec<i64>> = BTreeMap::new();
            for (u, _) in &units {
                groups.entry(group_of.get(u).cloned().unwrap_or_default()).or_default().push(*u);
            }
            // Largest-remainder allocation: each group gets its share, and the leftovers go to the biggest remainders.
            let mut alloc: Vec<(String, usize, f64)> = groups
                .iter()
                .map(|(g, ids)| {
                    let exact = size as f64 * ids.len() as f64 / pool as f64;
                    (g.clone(), exact.floor() as usize, exact - exact.floor())
                })
                .collect();
            let mut left = size - alloc.iter().map(|a| a.1).sum::<usize>();
            let mut order: Vec<usize> = (0..alloc.len()).collect();
            order.sort_by(|a, b| alloc[*b].2.partial_cmp(&alloc[*a].2).expect("finite").then(a.cmp(b)));
            for i in order {
                if left == 0 {
                    break;
                }
                if alloc[i].1 < groups[&alloc[i].0].len() {
                    alloc[i].1 += 1;
                    left -= 1;
                }
            }
            for (g, n, _) in alloc {
                chosen.extend(pick(&groups[&g], n, &mut rng));
            }
        }
        chosen.sort_unstable();
        Ok(Sample { seed, size, pool, unit_ids: chosen })
    }

    /// Measures agreement between coders on a set of units, stores the run, and writes the report.
    pub fn agreement_run(&mut self, req: &AgreementRequest) -> Result<AgreementReport> {
        if req.coder_ids.len() < 2 {
            return Err(Error::Invalid("choose at least two coders to compare".to_string()));
        }
        let distinct: HashSet<_> = req.coder_ids.iter().collect();
        if distinct.len() != req.coder_ids.len() {
            return Err(Error::Invalid("the same coder was chosen twice".to_string()));
        }
        if !matches!(req.mode.as_str(), "single_label" | "per_code") {
            return Err(Error::Invalid("the mode should be single_label or per_code".to_string()));
        }
        if let Some(t) = &req.reporting_term {
            if !matches!(t.as_str(), "intercoder_reliability" | "concordance") {
                return Err(Error::Invalid("the reporting term should be intercoder_reliability or concordance".to_string()));
            }
        }
        let coders = self.coder_infos(&req.coder_ids)?;
        let target: f64 = self.conn.query_row("SELECT agreement_target FROM project WHERE id = 1", [], |r| r.get(0))?;
        let any_model = coders.iter().any(|c| c.role == "model");
        let term = req.reporting_term.clone().unwrap_or_else(|| if any_model { "concordance" } else { "intercoder_reliability" }.to_string());

        // codes[coder_index][unit_id] = the set of code ids that coder applied to that unit.
        let codes = self.codes_by_unit(&req.coder_ids, req.areas && req.unit_ids.is_none())?;
        let sample: Vec<i64> = match &req.unit_ids {
            Some(ids) => ids.clone(),
            None => {
                let mut all: HashSet<i64> = HashSet::new();
                for c in &codes {
                    all.extend(c.keys());
                }
                let mut v: Vec<i64> = all.into_iter().filter(|u| codes.iter().filter(|c| c.contains_key(u)).count() >= 2).collect();
                v.sort_unstable();
                v
            }
        };
        if sample.is_empty() {
            return Err(Error::Invalid("no unit was coded by at least two of these coders, so there is nothing to compare".to_string()));
        }
        let names: HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();

        let mut report = AgreementReport {
            run_id: None,
            mode: req.mode.clone(),
            reporting_term: term.clone(),
            coders: coders.clone(),
            units_in_sample: sample.len(),
            overall: None,
            intervals: BTreeMap::new(),
            target,
            by_code: Vec::new(),
            confusion: None,
            confusion_codes: Vec::new(),
            disagreements: 0,
            notes: Vec::new(),
            report_text: String::new(),
        };
        let used_codes: Vec<i64>;
        let coefficients_json: Value;

        if req.mode == "single_label" {
            // One category per coder per unit.
            let mut all: Vec<i64> = codes.iter().flat_map(|m| sample.iter().filter_map(move |u| m.get(u))).flatten().copied().collect();
            all.sort_unstable();
            all.dedup();
            let index: HashMap<i64, usize> = all.iter().enumerate().map(|(i, c)| (*c, i)).collect();
            if all.len() < 2 {
                return Err(Error::Invalid("the coders used fewer than two different codes, so there is nothing to measure".to_string()));
            }
            let mut rows: Vec<Vec<Option<usize>>> = Vec::with_capacity(sample.len());
            for u in &sample {
                let mut row = Vec::with_capacity(codes.len());
                for (ci, per_coder) in codes.iter().enumerate() {
                    match per_coder.get(u) {
                        None => row.push(None),
                        Some(set) if set.len() == 1 => row.push(Some(index[set.iter().next().expect("one")])),
                        Some(set) => {
                            return Err(Error::Invalid(format!(
                                "{} applied {} codes to unit {u}, so the codes cannot be compared one per unit; use the per-code view",
                                coders[ci].name,
                                set.len()
                            )))
                        }
                    }
                }
                rows.push(row);
            }
            let ratings = Ratings::new(all.len(), rows).map_err(Error::Invalid)?;
            let overall = agreement(&ratings);
            let asked = [Coefficient::KrippendorffAlpha, Coefficient::CohenKappa, Coefficient::PercentAgreement];
            for (which, interval) in asked.iter().zip(bootstrap_intervals(&ratings, &asked, req.bootstrap_replications, req.seed, 0.95)) {
                if let Some(iv) = interval {
                    report.intervals.insert(coefficient_name(*which).to_string(), iv);
                }
            }
            if ratings.coders() == 2 {
                report.confusion = Some(confusion_table(&ratings, 0, 1));
                report.confusion_codes = all.iter().map(|c| names.get(c).cloned().unwrap_or_else(|| c.to_string())).collect();
                report.disagreements = ratings.units().iter().filter(|r| matches!((r[0], r[1]), (Some(a), Some(b)) if a != b)).count();
            }
            used_codes = all.clone();
            coefficients_json = serde_json::to_value(&overall)?;
            report.overall = Some(overall);
        } else {
            // Each code on its own.
            let mut all: Vec<i64> = codes.iter().flat_map(|m| sample.iter().filter_map(move |u| m.get(u))).flatten().copied().collect();
            all.sort_unstable();
            all.dedup();
            for code in &all {
                let applied: Vec<Vec<Option<bool>>> =
                    sample.iter().map(|u| codes.iter().map(|m| m.get(u).map(|set| set.contains(code))).collect()).collect();
                let detail = code_agreement(&applied).map_err(Error::Invalid)?;
                let measure = detail.agreement.krippendorff_alpha.or(detail.agreement.percent_agreement);
                report.by_code.push(CodeReport {
                    code_id: *code,
                    name: names.get(code).cloned().unwrap_or_else(|| code.to_string()),
                    below_target: measure.is_none_or(|m| m < target),
                    detail,
                });
            }
            report.by_code.sort_by(|a, b| {
                // Alpha is undefined when everything is in one category; fall back to plain agreement so a code
                // everyone applied the same way sorts last, not first.
                let key = |c: &CodeReport| c.detail.agreement.krippendorff_alpha.or(c.detail.agreement.percent_agreement).unwrap_or(f64::NEG_INFINITY);
                key(a).partial_cmp(&key(b)).expect("not NaN")
            });
            used_codes = all;
            coefficients_json = serde_json::to_value(&report.by_code)?;
        }

        report.notes = notes_for(&report, &coders, any_model);
        let cells = sample.iter().filter(|u| **u < 0).count();
        if cells > 0 {
            report.notes.push(format!(
                "{cells} of the {} units are cells of a {AREA_GRID} by {AREA_GRID} grid laid over a picture. A cell counts for a coder when the middle of the cell lies inside an area that coder drew. Neighbouring cells are not independent of each other, so the intervals are narrower than the coding supports, and an area smaller than one cell is not counted. Treat the figures for pictures as a guide.",
                sample.len()
            ));
        }
        report.report_text = write_report(&report);

        if req.save {
            let tx = self.conn.transaction()?;
            tx.execute(
                "INSERT INTO reliability_run (coders_json, codes_json, units_json, coefficients_json, results_json, reporting_term, sample_seed, report_text, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![
                    serde_json::to_string(&coders)?,
                    serde_json::to_string(&used_codes)?,
                    serde_json::to_string(&sample)?,
                    coefficients_json.to_string(),
                    serde_json::to_string(&report)?,
                    term,
                    req.seed as i64,
                    report.report_text
                ],
            )?;
            let id = tx.last_insert_rowid();
            report.run_id = Some(id);
            event::insert(
                &tx,
                &self.actor,
                &self.session,
                "reliability.run",
                Some(("reliability_run", id)),
                None,
                Some(&json!({ "mode": req.mode, "coders": req.coder_ids, "units": sample.len(), "reporting_term": report.reporting_term })),
            )?;
            tx.commit()?;
        }
        Ok(report)
    }

    /// Past runs, newest first.
    pub fn list_reliability_runs(&self) -> Result<Vec<RunSummary>> {
        let mut stmt = self.conn.prepare("SELECT id, created_at, reporting_term, units_json, archived FROM reliability_run ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| {
            let units: String = r.get(3)?;
            let n = serde_json::from_str::<Vec<Value>>(&units).map(|v| v.len()).unwrap_or(0);
            Ok(RunSummary { id: r.get(0)?, created_at: r.get(1)?, reporting_term: r.get(2)?, units: n, archived: r.get::<_, i64>(4)? != 0 })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// The saved report of a run.
    pub fn get_reliability_run(&self, id: i64) -> Result<AgreementReportRecord> {
        let row: Option<(String, String)> = self
            .conn
            .query_row("SELECT results_json, report_text FROM reliability_run WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let (json, text) = row.ok_or(Error::NotFound { what: "reliability run", id })?;
        Ok(AgreementReportRecord { id, report: serde_json::from_str(&json)?, text })
    }

    /// Archives a run. Its units are free to change again, and the run stays in the project as a record.
    pub fn archive_reliability_run(&mut self, id: i64) -> Result<()> {
        self.ensure_writable()?;
        let tx = self.conn.transaction()?;
        let n = tx.execute("UPDATE reliability_run SET archived = 1 WHERE id = ?1 AND archived = 0", [id])?;
        if n == 0 {
            return Err(Error::Invalid("that run does not exist or is already archived".to_string()));
        }
        event::insert(&tx, &self.actor, &self.session, "reliability.archive", Some(("reliability_run", id)), None, None)?;
        tx.commit()?;
        Ok(())
    }

    /// Sources that have units in a run that is not archived. Their text cannot be edited, so the
    /// reported agreement always matches the text it was computed on (specification 6.1).
    pub fn locked_sources(&self) -> Result<Vec<(i64, i64)>> {
        let mut runs = self.conn.prepare("SELECT id, units_json FROM reliability_run WHERE archived = 0")?;
        let runs: Vec<(i64, String)> = runs.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        let mut out = Vec::new();
        for (run, units_json) in runs {
            let ids: Vec<i64> = serde_json::from_str(&units_json)?;
            let mut seen = HashSet::new();
            for chunk in ids.chunks(500) {
                let marks = vec!["?"; chunk.len()].join(",");
                let mut stmt = self.conn.prepare(&format!("SELECT DISTINCT source_id FROM unit WHERE id IN ({marks})"))?;
                let sources = stmt.query_map(rusqlite::params_from_iter(chunk.iter()), |r| r.get::<_, i64>(0))?;
                for s in sources {
                    seen.insert(s?);
                }
            }
            out.extend(seen.into_iter().map(|s| (s, run)));
        }
        out.sort_unstable();
        Ok(out)
    }

    /// The first run that is not archived and uses units of this source, if any.
    pub(crate) fn reliability_lock(&self, source_id: i64) -> Result<Option<i64>> {
        let mut runs = self.conn.prepare("SELECT id, units_json FROM reliability_run WHERE archived = 0 ORDER BY id")?;
        let runs: Vec<(i64, String)> = runs.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<std::result::Result<_, _>>()?;
        if runs.is_empty() {
            return Ok(None);
        }
        let mut stmt = self.conn.prepare("SELECT id FROM unit WHERE source_id = ?1")?;
        let mine: HashSet<i64> = stmt.query_map([source_id], |r| r.get(0))?.collect::<std::result::Result<_, _>>()?;
        for (run, units_json) in runs {
            let ids: Vec<i64> = serde_json::from_str(&units_json)?;
            if ids.iter().any(|u| mine.contains(u)) {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    pub(crate) fn coder_infos(&self, ids: &[i64]) -> Result<Vec<CoderInfo>> {
        ids.iter()
            .map(|id| {
                self.conn
                    .query_row("SELECT name, role, model_name, model_version FROM coder WHERE id = ?1", [id], |r| {
                        Ok(CoderInfo { id: *id, name: r.get(0)?, role: r.get(1)?, model_name: r.get(2)?, model_version: r.get(3)? })
                    })
                    .optional()?
                    .ok_or(Error::NotFound { what: "coder", id: *id })
            })
            .collect()
    }

    /// For each coder (in the order given), the codes they applied to each unit, using their own codings
    /// only (never the adjudicated final layer).
    pub(crate) fn codes_by_unit(&self, coder_ids: &[i64], with_areas: bool) -> Result<Vec<HashMap<i64, HashSet<i64>>>> {
        let mut out = Vec::new();
        for coder in coder_ids {
            let mut map: HashMap<i64, HashSet<i64>> = HashMap::new();
            // A coding belongs to a unit by its unit id, or, when it has none, by lying inside the unit.
            let mut stmt = self.conn.prepare(
                "SELECT COALESCE(c.unit_id, (SELECT u.id FROM unit u WHERE u.source_id = c.source_id AND u.start_offset <= c.start_offset AND u.end_offset >= c.end_offset LIMIT 1)),
                        c.code_id
                 FROM coding c WHERE c.coder_id = ?1 AND c.layer = 'coder'",
            )?;
            for row in stmt.query_map([coder], |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?)))? {
                if let (Some(unit), code) = row? {
                    map.entry(unit).or_default().insert(code);
                }
            }
            out.push(map);
        }
        // The areas coders drew on pictures, as the cells of each picture they cover.
        if with_areas {
            let regions = self.all_regions()?;
            for (map, coder) in out.iter_mut().zip(coder_ids) {
                for r in regions.iter().filter(|r| r.coder_id == *coder && r.layer == "coder") {
                    for cell in covered_cells(&r.shape) {
                        map.entry(cell_unit(r.source_id, r.page, cell)).or_default().insert(r.code_id);
                    }
                }
            }
        }
        Ok(out)
    }
}

/// One line of the list of past runs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSummary {
    pub id: i64,
    pub created_at: String,
    pub reporting_term: String,
    pub units: usize,
    pub archived: bool,
}

/// A stored run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgreementReportRecord {
    pub id: i64,
    pub report: Value,
    pub text: String,
}

/// Picks `n` items at random without replacement (a partial Fisher-Yates shuffle).
fn pick(items: &[i64], n: usize, rng: &mut SplitMix64) -> Vec<i64> {
    let mut pool = items.to_vec();
    let n = n.min(pool.len());
    for i in 0..n {
        let j = i + rng.below(pool.len() - i);
        pool.swap(i, j);
    }
    pool.truncate(n);
    pool
}

fn notes_for(report: &AgreementReport, coders: &[CoderInfo], any_model: bool) -> Vec<String> {
    let mut notes = Vec::new();
    let models: Vec<&CoderInfo> = coders.iter().filter(|c| c.role == "model").collect();
    if models.len() >= 2 {
        notes.push(format!(
            "Limitation: {} of the coders are models. Models may share training data and habits, so their agreement with each other may overstate how independent the coding is.",
            models.len()
        ));
    }
    if any_model && report.reporting_term == "intercoder_reliability" {
        notes.push("This run includes a model but is called intercoder reliability because you chose that term. Agreement with a model is usually reported as concordance, since a model is not an independent human judge.".to_string());
    }
    if let Some(o) = &report.overall {
        if let (Some(p), Some(k)) = (o.percent_agreement, o.cohen_kappa.or(o.krippendorff_alpha)) {
            if p >= 0.8 && k < 0.6 {
                notes.push(format!(
                    "Percent agreement is high ({p:.3}) but the chance-corrected coefficient is low ({k:.3}). This usually means a few codes take up most of the units, which makes chance agreement high. Gwet's AC1 ({}) and the Brennan-Prediger coefficient ({}) are less affected by that.",
                    pct(o.gwet_ac1),
                    pct(o.brennan_prediger)
                ));
            }
        }
    }
    for c in &report.by_code {
        if let (Some(p), Some(k)) = (c.detail.agreement.percent_agreement, c.detail.agreement.krippendorff_alpha) {
            if p >= 0.9 && k < 0.6 {
                notes.push(format!(
                    "Code '{}': agreement is {p:.3} but alpha is {k:.3}. The code is rare or very common, so most units are easy; see its prevalence index ({}).",
                    c.name,
                    pct(c.detail.prevalence_index)
                ));
            }
        }
    }
    notes
}

fn term_label(term: &str) -> &'static str {
    if term == "concordance" {
        "concordance"
    } else {
        "intercoder reliability"
    }
}

/// The report as plain text, which names every coder and says exactly what was and was not measured.
fn write_report(r: &AgreementReport) -> String {
    let mut t = String::new();
    t.push_str(&format!("Agreement report ({})\n\n", term_label(&r.reporting_term)));
    t.push_str("Coders\n");
    for c in &r.coders {
        let model = match (&c.model_name, &c.model_version) {
            (Some(m), Some(v)) => format!(", {m} version {v}"),
            (Some(m), None) => format!(", {m}"),
            _ => String::new(),
        };
        t.push_str(&format!("  {} ({}{model})\n", c.name, c.role));
    }
    t.push_str(&format!("\nUnits compared: {}. Target: {:.2}.\n", r.units_in_sample, r.target));
    if let Some(o) = &r.overall {
        t.push_str(&format!("\nOverall, across all codes (one code per unit)\n  percent agreement      {}\n", pct(o.percent_agreement)));
        if r.coders.len() == 2 {
            t.push_str(&format!("  Cohen's kappa          {}\n", pct(o.cohen_kappa)));
        }
        t.push_str(&format!(
            "  Scott's pi             {}\n  Fleiss' kappa          {} ({} units)\n  Brennan-Prediger       {}\n  Gwet's AC1             {}\n  Krippendorff's alpha   {}\n",
            pct(o.scott_pi),
            pct(o.fleiss_kappa),
            o.fleiss_units,
            pct(o.brennan_prediger),
            pct(o.gwet_ac1),
            pct(o.krippendorff_alpha)
        ));
        for (name, iv) in &r.intervals {
            t.push_str(&format!("  {name}: 95% interval {:.3} to {:.3} ({} bootstrap resamples of units)\n", iv.low, iv.high, iv.replications));
        }
        if r.coders.len() == 2 {
            t.push_str(&format!("  Units the coders disagreed on: {}\n", r.disagreements));
        }
        if let Some(alpha) = o.krippendorff_alpha {
            let verdict = if alpha >= r.target { "meets" } else { "is below" };
            t.push_str(&format!("\nKrippendorff's alpha {alpha:.3} {verdict} the project target of {:.2}.\n", r.target));
        }
    }
    if !r.by_code.is_empty() {
        t.push_str("\nBy code (each code applied or not), lowest alpha first\n");
        for c in &r.by_code {
            let a = &c.detail.agreement;
            t.push_str(&format!(
                "  {:<28} agree {}  alpha {}  kappa {}  both applied {} / either applied {}{}\n",
                c.name,
                pct(a.percent_agreement),
                pct(a.krippendorff_alpha),
                pct(a.cohen_kappa),
                c.detail.both,
                c.detail.both + c.detail.first_only + c.detail.second_only,
                if c.below_target { "  (under target)" } else { "" }
            ));
        }
    }
    if !r.notes.is_empty() {
        t.push_str("\nNotes\n");
        for n in &r.notes {
            t.push_str(&format!("  - {n}\n"));
        }
    }
    t.push_str("\nEvery number above was computed from the codings stored in this project. No coder, coding, or statistic was added.\n");
    t
}

#[cfg(test)]
mod span_cells {
    use super::*;

    #[test]
    fn a_stretch_covers_the_seconds_whose_middle_it_contains() {
        // Second n runs from n*1000 to (n+1)*1000 and its middle is at n*1000 + 500.
        assert_eq!(covered_cells(&Shape::Span { start_ms: 1000, end_ms: 4000 }), [1, 2, 3]);
        assert_eq!(covered_cells(&Shape::Span { start_ms: 1400, end_ms: 3600 }), [1, 2, 3]);
        assert_eq!(covered_cells(&Shape::Span { start_ms: 1600, end_ms: 3400 }), [2], "the middles of the first and last seconds are outside");
        assert!(covered_cells(&Shape::Span { start_ms: 0, end_ms: 300 }).is_empty(), "it ends before the middle of the first second");
        assert_eq!(covered_cells(&Shape::Span { start_ms: 1500, end_ms: 2500 }), [1, 2], "a middle exactly on an end is inside");
        assert_eq!(covered_cells(&Shape::Span { start_ms: 0, end_ms: 500 }), [0]);
        assert!(covered_cells(&Shape::Span { start_ms: 600, end_ms: 900 }).is_empty(), "shorter than a cell, and the middle is outside");
        // Two coders who mark the same stretch share every cell; one second apart they share all but the ends.
        let a = covered_cells(&Shape::Span { start_ms: 0, end_ms: 10_000 });
        let b = covered_cells(&Shape::Span { start_ms: 1_000, end_ms: 10_000 });
        assert_eq!((a.len(), b.len()), (10, 9));
        assert!(cell_unit(3, 1, 4) < 0);
        assert_ne!(cell_unit(3, 1, 4), cell_unit(3, 1, 5));
        assert_eq!(covered_cells(&Shape::Span { start_ms: 0, end_ms: 200_000_000 }).len(), 99_999, "a very long recording is compared up to the limit");
    }
}
