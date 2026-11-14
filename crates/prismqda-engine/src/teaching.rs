//! Teaching (decision 183): a class exercise. An instructor codes the documents of an exercise, makes a blind packet for every student, brings
//! the packets back when the students have coded, and reviews the class against their own coding: who found what, which codes the class
//! finds hard, and which passages the class coded differently.
//!
//! Nothing here is new machinery for the project file. A packet is a coder packet (decision 161) made once for each name; bringing the
//! packets back is Merge (a student's work stays with the student as a coder); the review is read from the codings that are there. The
//! comparison is the one Reliability makes: passages are the units of the text, and a coder has coded a unit with a code when a coding of
//! theirs with that code lies in it. Pictures, PDF areas and recordings are not part of the review yet.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::adjudication::CodeRef;
use crate::event;
use crate::merge::{quiet_with, remove_scratch, scratch_path, MergeChoices, MergeScope};
use crate::packet::CoderPacketSpec;
use crate::project::Project;
use crate::reliability::CoderInfo;
use crate::{Error, Result};

/// The most students a class holds in one run (a guard against a pasted file that is not a list of names).
pub const MAX_STUDENTS: usize = 500;
/// The longest part of a passage's words kept with a hard passage.
const MAX_TEXT_CHARS: usize = 400;

// ---- a packet for every student -----------------------------------------------------------------------------------------------

/// What goes into the packets of a class.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ClassPacketsSpec {
    /// One name for each student. A name repeated (any case) is made once.
    pub students: Vec<String>,
    /// The documents to put in; empty for all of them.
    #[serde(default)]
    pub source_ids: Vec<i64>,
    /// What every student should do and how; they see it as a memo.
    #[serde(default)]
    pub instructions: String,
}

/// What became of one student's packet.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassPacketOutcome {
    pub student: String,
    pub path: String,
    pub made: bool,
    /// Why it was not made (for example, a file of that name is already there).
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassPacketsReport {
    pub folder: String,
    pub packets: Vec<ClassPacketOutcome>,
    pub made: usize,
    /// Names left out because the same name came twice.
    pub repeated: Vec<String>,
    /// The documents each packet holds, by name.
    pub documents: Vec<String>,
    pub codes: usize,
}

/// A name as a part of a file name: characters Windows does not allow are left out.
fn file_part(name: &str) -> String {
    let cleaned: String = name.chars().filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control()).collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    cleaned.trim_matches('.').trim().chars().take(80).collect()
}

/// The names of a class, tidied: spaces squeezed, empty lines dropped, a repeated name (any case) kept once.
pub fn tidy_students(names: &[String]) -> (Vec<String>, Vec<String>) {
    let mut seen = HashSet::new();
    let (mut kept, mut repeated) = (Vec::new(), Vec::new());
    for raw in names {
        let name = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.to_lowercase()) {
            kept.push(name);
        } else {
            repeated.push(name);
        }
    }
    (kept, repeated)
}

impl Project {
    /// Makes a blind packet for each student in `folder` (made if it is not there), named `<stem> - packet for <student>.pqda`. A packet that
    /// cannot be made (a file of that name is there) is told and the others are still made. Nothing in this project changes.
    pub fn make_class_packets(&self, folder: &Path, stem: &str, spec: &ClassPacketsSpec) -> Result<ClassPacketsReport> {
        let (students, repeated) = tidy_students(&spec.students);
        if students.is_empty() {
            return Err(Error::Invalid("list at least one student".to_string()));
        }
        if students.len() > MAX_STUDENTS {
            return Err(Error::Invalid(format!("a class of more than {MAX_STUDENTS} students was listed; split it, or check that the list holds names")));
        }
        if folder.is_file() {
            return Err(Error::Invalid(format!("{} is a file; choose a folder for the packets", folder.display())));
        }
        fs::create_dir_all(folder)?;
        let mut packets = Vec::new();
        let (mut documents, mut codes) = (Vec::new(), 0);
        for student in &students {
            let who = file_part(student);
            let who = if who.is_empty() { format!("student {}", packets.len() + 1) } else { who };
            let path = folder.join(format!("{stem} - packet for {who}.pqda"));
            let made = self.make_coder_packet(&path, &CoderPacketSpec { coder_name: student.clone(), source_ids: spec.source_ids.clone(), instructions: spec.instructions.clone(), include_codings: false });
            match made {
                Ok(report) => {
                    documents = report.documents;
                    codes = report.codes;
                    packets.push(ClassPacketOutcome { student: student.clone(), path: path.display().to_string(), made: true, error: None });
                }
                Err(e) => packets.push(ClassPacketOutcome { student: student.clone(), path: path.display().to_string(), made: false, error: Some(e.to_string()) }),
            }
        }
        let made = packets.iter().filter(|p| p.made).count();
        Ok(ClassPacketsReport { folder: folder.display().to_string(), packets, made, repeated, documents, codes })
    }
}

// ---- bringing the packets back --------------------------------------------------------------------------------------------------

/// What became of one file in the folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassCollectOutcome {
    pub file: String,
    /// The student the packet is for, when the file is a packet.
    pub student: Option<String>,
    /// `merged`, `partly` (some of its codings could not be placed, with the reason in the message), `nothing_new` (the packet's work was already here), `not_a_packet`, `other_study` (it holds documents this project does not have), or `failed`.
    pub status: String,
    /// One sentence a person can read.
    pub message: String,
    pub codings_added: usize,
    pub codings_already_here: usize,
    pub codings_skipped: usize,
    /// Documents of the packet whose words are not the words of the document of that name here (it was changed after the packets were made), so
    /// none of the codings on them was brought in.
    pub documents_in_conflict: Vec<String>,
    /// Codes the student added to the codebook in the packet, which are in this project's codebook now.
    pub codes_added: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassCollectReport {
    pub folder: String,
    pub files: Vec<ClassCollectOutcome>,
    /// Packets whose work was brought in (or was already here).
    pub packets: usize,
    pub codings_added: usize,
    /// Where the backup made before anything was brought in was saved; none when there was nothing to bring.
    pub backup: Option<String>,
}

impl Project {
    /// Brings in the work of every coder packet of this project in `folder`: one backup first, then each packet's coders and codings (not its
    /// instructions memo, and no documents). A file that is not a packet, that holds documents this project does not have, or that cannot be
    /// read is told and left; the others are still brought in. A packet brought in twice changes nothing the second time.
    pub fn collect_class_packets(&mut self, folder: &Path, passphrase: Option<&str>) -> Result<ClassCollectReport> {
        self.ensure_writable()?;
        if !folder.is_dir() {
            return Err(Error::Invalid(format!("{} is not a folder", folder.display())));
        }
        let own = fs::canonicalize(&self.path).ok();
        let mut files: Vec<PathBuf> = fs::read_dir(folder)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_file() && p.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pqda")))
            .filter(|p| fs::canonicalize(p).ok() != own)
            .collect();
        files.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        let our_names: HashSet<String> = self.list_sources()?.into_iter().map(|s| s.name).collect();
        let pass = passphrase.map(str::to_string).or_else(|| self.key.as_ref().map(|k| k.as_str().to_string()));

        // Look at each file first, on a scratch copy of it, so a file that is not to be brought in costs nothing.
        let mut outcomes: Vec<ClassCollectOutcome> = Vec::new();
        let mut to_bring: Vec<(usize, PathBuf)> = Vec::new();
        for file in files {
            let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let mut out = ClassCollectOutcome { file: name, student: None, status: "failed".to_string(), message: String::new(), codings_added: 0, codings_already_here: 0, codings_skipped: 0, documents_in_conflict: Vec::new(), codes_added: Vec::new() };
            match inspect_packet(&file, pass.as_deref(), &our_names) {
                Ok(Inspected::Packet { student }) => {
                    out.student = Some(student);
                    to_bring.push((outcomes.len(), file));
                    out.status = "pending".to_string();
                }
                Ok(Inspected::NotAPacket) => {
                    out.status = "not_a_packet".to_string();
                    out.message = "This file is not a coder packet, so it was left.".to_string();
                }
                Ok(Inspected::OtherStudy { student, missing }) => {
                    out.student = Some(student);
                    out.status = "other_study".to_string();
                    out.message = format!("It holds {} that this project does not have ({}), so it may belong to another study. It was left.", if missing.len() == 1 { "a document" } else { "documents" }, missing.join(", "));
                }
                Err(e) => out.message = format!("It could not be read: {e}"),
            }
            outcomes.push(out);
        }

        let mut backup = None;
        if !to_bring.is_empty() {
            let (path, _) = self.backup_now_noted()?;
            backup = Some(path.display().to_string());
        }
        let scope = || MergeScope { sources: None, codings: true, memos: false, recordings: false, cases: false, choices: MergeChoices::default() };
        let (mut brought, mut added_total) = (0, 0);
        for (at, file) in to_bring {
            let scratch = scratch_path("class");
            let merged = (|| -> Result<crate::merge::MergeReport> {
                fs::copy(&file, &scratch)?;
                let theirs = Project::open(&scratch, &quiet_with(pass.as_deref()))?;
                self.merge_from(&theirs, &scope())
            })();
            remove_scratch(&scratch);
            let out = &mut outcomes[at];
            match merged {
                Ok(r) => {
                    out.codings_added = r.codings_added;
                    out.codings_already_here = r.codings_already_here;
                    out.codings_skipped = r.codings_skipped.iter().map(|s| s.count).sum();
                    brought += 1;
                    added_total += r.codings_added;
                    out.documents_in_conflict = r.documents_in_conflict.clone();
                    out.codes_added = r.codes_added.clone();
                    if r.codings_added == 0 && r.codings_already_here > 0 && out.documents_in_conflict.is_empty() {
                        out.status = "nothing_new".to_string();
                        out.message = "Its work was already here.".to_string();
                    } else {
                        out.status = if out.documents_in_conflict.is_empty() && out.codings_skipped == 0 { "merged" } else { "partly" }.to_string();
                        out.message = format!("{} coding{} brought in.", r.codings_added, if r.codings_added == 1 { "" } else { "s" });
                        if out.codings_skipped > 0 {
                            out.message.push_str(&format!(" {} could not be placed and {} left out.", out.codings_skipped, if out.codings_skipped == 1 { "was" } else { "were" }));
                        }
                        if !out.documents_in_conflict.is_empty() {
                            out.message.push_str(&format!(
                                " The words of {} here are not the words in the packet (was {} changed after the packets were made?), so no coding on {} was brought in.",
                                out.documents_in_conflict.join(", "),
                                if out.documents_in_conflict.len() == 1 { "it" } else { "one" },
                                if out.documents_in_conflict.len() == 1 { "it" } else { "them" },
                            ));
                        }
                        if !out.codes_added.is_empty() {
                            out.message.push_str(&format!(" The student added {} to the codebook: {}.", if out.codes_added.len() == 1 { "a code" } else { "codes" }, out.codes_added.join(", ")));
                        }
                    }
                }
                Err(e) => {
                    out.status = "failed".to_string();
                    out.message = format!("It could not be brought in: {e}");
                }
            }
        }
        if brought > 0 {
            let tx = self.conn.transaction()?;
            event::insert(&tx, &self.actor, &self.session, "class.collect", None, None, Some(&json!({ "packets": brought, "codings_added": added_total })))?;
            tx.commit()?;
        }
        Ok(ClassCollectReport { folder: folder.display().to_string(), files: outcomes, packets: brought, codings_added: added_total, backup })
    }
}

enum Inspected {
    Packet { student: String },
    NotAPacket,
    OtherStudy { student: String, missing: Vec<String> },
}

/// Opens a scratch copy of a file and says whether it is a packet whose documents this project has.
fn inspect_packet(file: &Path, pass: Option<&str>, our_names: &HashSet<String>) -> Result<Inspected> {
    let scratch = scratch_path("class-look");
    let result = (|| {
        fs::copy(file, &scratch)?;
        let theirs = Project::open(&scratch, &quiet_with(pass))?;
        let Some(info) = theirs.coder_packet_info()? else { return Ok(Inspected::NotAPacket) };
        let missing: Vec<String> = theirs.list_sources()?.into_iter().map(|s| s.name).filter(|n| !our_names.contains(n)).collect();
        if missing.is_empty() {
            Ok(Inspected::Packet { student: info.coder_name })
        } else {
            Ok(Inspected::OtherStudy { student: info.coder_name, missing })
        }
    })();
    remove_scratch(&scratch);
    result
}

// ---- the class review -----------------------------------------------------------------------------------------------------------

fn ten() -> usize {
    10
}

/// What to review.
#[derive(Debug, Clone, Deserialize)]
pub struct ClassReviewSpec {
    /// The coder whose coding is the answer the class is compared with.
    pub instructor_id: i64,
    /// The students; empty for every other human coder of the project.
    #[serde(default)]
    pub student_ids: Vec<i64>,
    /// The documents to compare on; empty for all of them.
    #[serde(default)]
    pub source_ids: Vec<i64>,
    /// Call the students "Student 1", "Student 2" and so on, in an order that does not follow their names, so the review can be shown to the class.
    #[serde(default)]
    pub anonymous: bool,
    /// How many of the hardest passages to list (the passages the fewest students coded as the instructor did).
    #[serde(default = "ten")]
    pub hardest: usize,
}

/// How one student's coding compares with the instructor's.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StudentReview {
    /// Absent when the review is anonymous.
    pub coder_id: Option<i64>,
    /// The student's name, or "Student 1" when the review is anonymous.
    pub label: String,
    /// The passages the student coded at all.
    pub passages_coded: usize,
    /// Of the passages the instructor coded: the student gave the same codes, gave some of the same and some others, gave other codes only, or did not code it.
    pub same: usize,
    pub partly: usize,
    pub different: usize,
    pub left_uncoded: usize,
    /// Passages the student coded that the instructor did not.
    pub extra: usize,
    /// Each (passage, code) the instructor applied: the student applied it too (found) or not (missed); and the ones the student applied that the instructor did not (added).
    pub found: usize,
    pub missed: usize,
    pub added: usize,
    /// `same` over the passages the instructor coded; `found` over the instructor's applications; `added` over the student's applications. Absent when there is nothing to divide.
    pub share_same: Option<f64>,
    pub share_found: Option<f64>,
    pub share_added: Option<f64>,
}

/// How the class did on one code.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CodeReview {
    pub code_id: i64,
    pub name: String,
    /// The passages the instructor gave this code.
    pub instructor_passages: usize,
    /// The students' average share of those passages that they gave the code too (absent when the instructor did not use it).
    pub mean_found: Option<f64>,
    /// How many times students gave the code to a passage where the instructor did not, all students together.
    pub added: usize,
    /// The average of the students' Cohen's kappa with the instructor for this code, over the students it is defined for.
    pub mean_kappa: Option<f64>,
    pub kappa_students: usize,
}

/// A passage the class coded differently from the instructor.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HardPassage {
    pub unit_id: i64,
    pub source_id: i64,
    pub source_name: String,
    pub start: i64,
    pub end: i64,
    pub text: String,
    pub instructor_codes: Vec<CodeRef>,
    pub students_same: usize,
    pub students_total: usize,
    /// Codes students gave that the instructor did not, with how many students gave each.
    pub added_codes: Vec<(CodeRef, usize)>,
    /// Codes the instructor gave that students did not, with how many students left each out.
    pub missed_codes: Vec<(CodeRef, usize)>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ClassReview {
    pub instructor: CoderInfo,
    pub anonymous: bool,
    /// The passages the instructor coded (in the documents compared).
    pub instructor_passages: usize,
    /// Every passage the instructor or a student coded.
    pub passages_compared: usize,
    pub students: Vec<StudentReview>,
    /// The codes, hardest for the class first.
    pub by_code: Vec<CodeReview>,
    pub hardest: Vec<HardPassage>,
    pub notes: Vec<String>,
}

fn cut(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_TEXT_CHARS {
        return text.to_string();
    }
    let mut out: String = text.chars().take(MAX_TEXT_CHARS - 1).collect();
    out.push('\u{2026}');
    out
}

/// A number that depends on the coder only, so a blind order does not follow the names.
fn blind_rank(coder_id: i64) -> u64 {
    let b = (coder_id as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let x = b.rotate_left(17) ^ 0x9E37_79B9_7F4A_7C15;
    x ^ (x >> 29)
}

fn share(part: usize, whole: usize) -> Option<f64> {
    (whole > 0).then(|| part as f64 / whole as f64)
}

/// Cohen's kappa of two coders' yes or no on each of `n` passages: `a` both said yes, `b` only the first, `c` only the second, `d` neither.
/// Absent when chance agreement is complete (everyone said the same thing every time, so there is nothing to correct).
pub fn kappa_2x2(a: usize, b: usize, c: usize, d: usize) -> Option<f64> {
    let n = (a + b + c + d) as f64;
    if n == 0.0 {
        return None;
    }
    let (a, b, c, d) = (a as f64, b as f64, c as f64, d as f64);
    let observed = (a + d) / n;
    let expected = ((a + b) * (a + c) + (c + d) * (b + d)) / (n * n);
    if (1.0 - expected).abs() < 1e-12 {
        return None;
    }
    Some((observed - expected) / (1.0 - expected))
}

impl Project {
    /// Compares each student's coding with the instructor's on the passages of the documents chosen.
    pub fn class_review(&self, spec: &ClassReviewSpec) -> Result<ClassReview> {
        let coders = self.list_coders()?;
        let instructor = coders.iter().find(|c| c.id == spec.instructor_id).ok_or(Error::NotFound { what: "coder", id: spec.instructor_id })?;
        let mut student_ids: Vec<i64> = if spec.student_ids.is_empty() {
            coders.iter().filter(|c| c.role == "human" && c.id != spec.instructor_id).map(|c| c.id).collect()
        } else {
            spec.student_ids.clone()
        };
        let mut seen = HashSet::new();
        student_ids.retain(|id| seen.insert(*id));
        for id in &student_ids {
            if *id == spec.instructor_id {
                return Err(Error::Invalid("the instructor cannot be one of the students".to_string()));
            }
            if !coders.iter().any(|c| c.id == *id) {
                return Err(Error::NotFound { what: "coder", id: *id });
            }
        }
        if student_ids.is_empty() {
            return Err(Error::Invalid("there is no student to compare: a student is a human coder other than the instructor, so bring the students' packets in first".to_string()));
        }
        if student_ids.len() > MAX_STUDENTS {
            return Err(Error::Invalid(format!("more than {MAX_STUDENTS} students were chosen")));
        }
        let all_sources = self.list_sources()?;
        for id in &spec.source_ids {
            if !all_sources.iter().any(|s| s.id == *id) {
                return Err(Error::NotFound { what: "document", id: *id });
            }
        }

        // The passages of the documents compared, and what each coder gave each. Every coder is asked about every one: a coder who did not
        // code a passage gave it no code, which, in an exercise, is an answer.
        let in_scope: Option<HashSet<i64>> = (!spec.source_ids.is_empty()).then(|| spec.source_ids.iter().copied().collect());
        let mut unit_stmt = self.conn.prepare("SELECT id, source_id, start_offset, end_offset FROM unit WHERE excluded = 0")?;
        let mut units: HashMap<i64, (i64, i64, i64)> = HashMap::new();
        for row in unit_stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)))? {
            let (id, source, start, end) = row?;
            if in_scope.as_ref().is_none_or(|s| s.contains(&source)) {
                units.insert(id, (source, start, end));
            }
        }
        let mut ids = vec![spec.instructor_id];
        ids.extend(&student_ids);
        let given = self.codes_by_unit(&ids, false)?;
        let empty: HashSet<i64> = HashSet::new();
        let by = |who: usize, unit: i64| -> &HashSet<i64> { given[who].get(&unit).unwrap_or(&empty) };

        let mut compared: Vec<i64> = units.keys().copied().filter(|u| given.iter().any(|g| g.get(u).is_some_and(|s| !s.is_empty()))).collect();
        compared.sort_unstable();
        let instructor_units: Vec<i64> = compared.iter().copied().filter(|u| !by(0, *u).is_empty()).collect();

        let names: HashMap<i64, String> = self.list_codes()?.into_iter().map(|c| (c.id, c.name)).collect();
        let code_ref = |id: i64| CodeRef { id, name: names.get(&id).cloned().unwrap_or_else(|| id.to_string()) };

        // The labels: the names, or "Student n" in an order that does not follow the names.
        let mut order: Vec<usize> = (0..student_ids.len()).collect();
        if spec.anonymous {
            order.sort_by_key(|i| blind_rank(student_ids[*i]));
        }
        let mut label_of = vec![String::new(); student_ids.len()];
        for (n, i) in order.iter().enumerate() {
            label_of[*i] = if spec.anonymous { format!("Student {}", n + 1) } else { coders.iter().find(|c| c.id == student_ids[*i]).map(|c| c.name.clone()).unwrap_or_default() };
        }

        if instructor_units.is_empty() {
            return Err(Error::Invalid("the instructor has not coded any passage of these documents, so there is nothing to compare the students with".to_string()));
        }

        // One student at a time.
        let mut per_student: Vec<StudentReview> = Vec::new();
        for (i, student) in student_ids.iter().enumerate() {
            let who = i + 1;
            let mut r = StudentReview {
                coder_id: (!spec.anonymous).then_some(*student),
                label: label_of[i].clone(),
                passages_coded: compared.iter().filter(|u| !by(who, **u).is_empty()).count(),
                same: 0,
                partly: 0,
                different: 0,
                left_uncoded: 0,
                extra: 0,
                found: 0,
                missed: 0,
                added: 0,
                share_same: None,
                share_found: None,
                share_added: None,
            };
            for u in &compared {
                let (mine, theirs) = (by(0, *u), by(who, *u));
                if mine.is_empty() {
                    if !theirs.is_empty() {
                        r.extra += 1;
                        r.added += theirs.len();
                    }
                    continue;
                }
                let both = mine.intersection(theirs).count();
                r.found += both;
                r.missed += mine.len() - both;
                r.added += theirs.len() - both;
                if theirs.is_empty() {
                    r.left_uncoded += 1;
                } else if mine == theirs {
                    r.same += 1;
                } else if both > 0 {
                    r.partly += 1;
                } else {
                    r.different += 1;
                }
            }
            r.share_same = share(r.same, instructor_units.len());
            r.share_found = share(r.found, r.found + r.missed);
            r.share_added = share(r.added, r.found + r.added);
            per_student.push(r);
        }

        // The codes: how the class did on each.
        let mut used: Vec<i64> = {
            let mut set: HashSet<i64> = HashSet::new();
            for g in &given {
                for u in &compared {
                    if let Some(codes) = g.get(u) {
                        set.extend(codes);
                    }
                }
            }
            set.into_iter().collect()
        };
        used.sort_unstable();
        let mut by_code: Vec<CodeReview> = Vec::new();
        for code in used {
            let instructor_passages = compared.iter().filter(|u| by(0, **u).contains(&code)).count();
            let (mut found_shares, mut kappas, mut added) = (Vec::new(), Vec::new(), 0);
            for who in 1..=student_ids.len() {
                let (mut a, mut b, mut c, mut d) = (0, 0, 0, 0);
                for u in &compared {
                    match (by(0, *u).contains(&code), by(who, *u).contains(&code)) {
                        (true, true) => a += 1,
                        (true, false) => b += 1,
                        (false, true) => c += 1,
                        (false, false) => d += 1,
                    }
                }
                added += c;
                if let Some(s) = share(a, a + b) {
                    found_shares.push(s);
                }
                if let Some(k) = kappa_2x2(a, b, c, d) {
                    kappas.push(k);
                }
            }
            let mean = |v: &[f64]| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
            by_code.push(CodeReview {
                code_id: code,
                name: names.get(&code).cloned().unwrap_or_else(|| code.to_string()),
                instructor_passages,
                mean_found: mean(&found_shares),
                added,
                mean_kappa: mean(&kappas),
                kappa_students: kappas.len(),
            });
        }
        // The codes the class found hardest first; a code the instructor did not use comes after, the one students added most first.
        by_code.sort_by(|x, y| {
            let key = |c: &CodeReview| (c.mean_found.is_none(), c.mean_found.unwrap_or(0.0), std::cmp::Reverse(c.added));
            let (kx, ky) = (key(x), key(y));
            kx.0.cmp(&ky.0).then(kx.1.partial_cmp(&ky.1).unwrap_or(std::cmp::Ordering::Equal)).then(kx.2.cmp(&ky.2)).then(x.name.to_lowercase().cmp(&y.name.to_lowercase()))
        });

        // The passages the class coded least like the instructor.
        let source_name: HashMap<i64, String> = all_sources.iter().map(|s| (s.id, s.name.clone())).collect();
        let total = student_ids.len();
        let mut hard: Vec<(usize, i64)> = Vec::new();
        for u in &instructor_units {
            let same = (1..=total).filter(|who| by(*who, *u) == by(0, *u)).count();
            if same < total {
                hard.push((same, *u));
            }
        }
        hard.sort_by_key(|(same, u)| (*same, units[u].0, units[u].1, *u));
        let mut hardest = Vec::new();
        for (same, u) in hard.into_iter().take(spec.hardest.min(200)) {
            let (source_id, start, end) = units[&u];
            let mut mine: Vec<i64> = by(0, u).iter().copied().collect();
            mine.sort_unstable();
            let mut added: HashMap<i64, usize> = HashMap::new();
            let mut missed: HashMap<i64, usize> = HashMap::new();
            for who in 1..=total {
                for c in by(who, u) {
                    if !by(0, u).contains(c) {
                        *added.entry(*c).or_default() += 1;
                    }
                }
                for c in by(0, u) {
                    if !by(who, u).contains(c) {
                        *missed.entry(*c).or_default() += 1;
                    }
                }
            }
            let listed = |m: HashMap<i64, usize>| -> Vec<(CodeRef, usize)> {
                let mut v: Vec<(i64, usize)> = m.into_iter().collect();
                v.sort_by_key(|(c, n)| (std::cmp::Reverse(*n), *c));
                v.into_iter().map(|(c, n)| (code_ref(c), n)).collect()
            };
            hardest.push(HardPassage {
                unit_id: u,
                source_id,
                source_name: source_name.get(&source_id).cloned().unwrap_or_default(),
                start,
                end,
                text: cut(&self.text_of_span(source_id, start, end)?),
                instructor_codes: mine.into_iter().map(code_ref).collect(),
                students_same: same,
                students_total: total,
                added_codes: listed(added),
                missed_codes: listed(missed),
            });
        }

        // The list is in the order of the labels.
        let mut students = per_student;
        if spec.anonymous {
            students = order.iter().map(|i| students[*i].clone()).collect();
        } else {
            students.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.coder_id.cmp(&b.coder_id)));
        }

        let mut notes = vec!["A passage is a unit of the text as it was imported (a paragraph, a turn, or a sentence). A coder has coded it with a code when a coding of theirs with that code lies in it. A passage a student did not code counts as left uncoded, which is an answer in an exercise.".to_string()];
        let blank = students.iter().filter(|s| s.passages_coded == 0).count();
        if blank > 0 {
            notes.push(format!("{blank} student{} coded nothing in these documents.", if blank == 1 { "" } else { "s" }));
        }
        let areas = self.all_regions()?.iter().filter(|r| r.layer == "coder" && ids.contains(&r.coder_id) && in_scope.as_ref().is_none_or(|s| s.contains(&r.source_id))).count();
        if areas > 0 {
            notes.push(format!("{areas} coded area{} or stretch{} of pictures, PDF pages and recordings {} not compared: the class review reads text only.", if areas == 1 { "" } else { "s" }, if areas == 1 { "" } else { "es" }, if areas == 1 { "was" } else { "were" }));
        }
        Ok(ClassReview {
            instructor: CoderInfo { id: instructor.id, name: instructor.name.clone(), role: instructor.role.clone(), model_name: instructor.model_name.clone(), model_version: instructor.model_version.clone() },
            anonymous: spec.anonymous,
            instructor_passages: instructor_units.len(),
            passages_compared: compared.len(),
            students,
            by_code,
            hardest,
            notes,
        })
    }
}
