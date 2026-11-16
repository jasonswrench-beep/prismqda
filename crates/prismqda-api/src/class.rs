//! A class exercise through the command layer (decision 183): a blind packet for every student, bringing the packets back, and the review of
//! the class against the instructor's own coding, which can also be saved as a table.

use std::path::PathBuf;

use prismqda_engine::{ClassPacketsSpec, ClassReview, ClassReviewSpec, ClassSideBySide, ClassSideBySideSpec};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{export_target, parse, to_json, write_file, Api, ApiError, ApiResult};

#[derive(Deserialize)]
struct PacketsArgs {
    /// The folder the packets go in; it is made when it is not there.
    folder: String,
    #[serde(flatten)]
    spec: ClassPacketsSpec,
}

#[derive(Deserialize)]
struct CollectArgs {
    folder: String,
    /// The passphrase of the packets when the project is encrypted and they are too (a team often shares one).
    #[serde(default)]
    passphrase: Option<String>,
}

#[derive(Deserialize)]
struct ReviewArgs {
    /// The coder whose coding is the answer. When absent, the person using the program.
    #[serde(default)]
    instructor_id: Option<i64>,
    #[serde(default)]
    student_ids: Vec<i64>,
    #[serde(default)]
    source_ids: Vec<i64>,
    #[serde(default)]
    anonymous: bool,
    #[serde(default)]
    hardest: Option<usize>,
}

#[derive(Deserialize)]
struct SaveArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(flatten)]
    review: ReviewArgs,
}

#[derive(Deserialize)]
struct SideArgs {
    /// The passages to show, as the review lists them; empty for the hardest ones.
    #[serde(default)]
    unit_ids: Vec<i64>,
    #[serde(flatten)]
    review: ReviewArgs,
}

#[derive(Deserialize)]
struct SideSaveArgs {
    path: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(default)]
    unit_ids: Vec<i64>,
    #[serde(flatten)]
    review: ReviewArgs,
}

#[derive(Deserialize)]
struct FeedbackArgs {
    /// The folder the files go in; it is made when it is not there.
    folder: String,
    #[serde(default)]
    overwrite: bool,
    #[serde(flatten)]
    review: ReviewArgs,
}

/// The columns of the table of students, in order.
const STUDENT_HEADER: [&str; 13] = [
    "Student",
    "Passages coded",
    "Same codes as the instructor",
    "Some of the same codes",
    "Other codes only",
    "Left uncoded",
    "Coded where the instructor did not",
    "Codes found",
    "Codes missed",
    "Codes added",
    "Share of the instructor's passages coded the same (%)",
    "Share of the instructor's codes found (%)",
    "Share of the student's codes the instructor did not give (%)",
];

fn percent(share: Option<f64>) -> String {
    share.map(|s| format!("{:.1}", s * 100.0)).unwrap_or_default()
}

/// The table of students as a CSV file's bytes.
pub(crate) fn students_csv(review: &ClassReview) -> Vec<u8> {
    let header: Vec<String> = STUDENT_HEADER.iter().map(|h| h.to_string()).collect();
    let rows: Vec<Vec<String>> = review
        .students
        .iter()
        .map(|s| {
            vec![
                s.label.clone(),
                s.passages_coded.to_string(),
                s.same.to_string(),
                s.partly.to_string(),
                s.different.to_string(),
                s.left_uncoded.to_string(),
                s.extra.to_string(),
                s.found.to_string(),
                s.missed.to_string(),
                s.added.to_string(),
                percent(s.share_same),
                percent(s.share_found),
                percent(s.share_added),
            ]
        })
        .collect();
    // Written without the replacement of names (no scope is open), because the names are what this table is about.
    prismqda_export::write_csv(&header, &rows)
}

/// The class heat map as a CSV file's bytes: a row for each student, a column for each code, the share of the instructor's passages with the code
/// in which the student gave it too, in percent.
pub(crate) fn heat_map_csv(review: &ClassReview) -> Vec<u8> {
    let mut header = vec!["Student".to_string()];
    header.extend(review.heat_map.codes.iter().map(|c| format!("{} (% found)", c.name)));
    let rows: Vec<Vec<String>> = review
        .heat_map
        .rows
        .iter()
        .map(|r| {
            let mut row = vec![r.label.clone()];
            row.extend(r.found.iter().map(|f| percent(*f)));
            row
        })
        .collect();
    prismqda_export::write_csv(&header, &rows)
}

/// The side-by-side view as a CSV file's bytes: a row for each passage, the instructor's codes and then each student's.
pub(crate) fn side_by_side_csv(view: &ClassSideBySide) -> Vec<u8> {
    let mut header = vec!["Document".to_string(), "Passage".to_string(), "Instructor".to_string()];
    header.extend(view.students.iter().cloned());
    let codes = |v: &[prismqda_engine::CodeRef]| v.iter().map(|c| c.name.clone()).collect::<Vec<_>>().join("; ");
    let rows: Vec<Vec<String>> = view
        .passages
        .iter()
        .map(|p| {
            let mut row = vec![p.source_name.clone(), p.text.clone(), codes(&p.instructor_codes)];
            row.extend(p.students.iter().map(|s| codes(&s.codes)));
            row
        })
        .collect();
    prismqda_export::write_csv(&header, &rows)
}

impl Api {
    fn review_spec(&mut self, a: &ReviewArgs) -> Result<ClassReviewSpec, ApiError> {
        let instructor_id = match a.instructor_id {
            Some(id) => id,
            None => self.me()?,
        };
        Ok(ClassReviewSpec { instructor_id, student_ids: a.student_ids.clone(), source_ids: a.source_ids.clone(), anonymous: a.anonymous, hardest: a.hardest.unwrap_or(10) })
    }

    fn class_review_for(&mut self, a: &ReviewArgs) -> Result<ClassReview, ApiError> {
        let spec = self.review_spec(a)?;
        Ok(self.project()?.class_review(&spec)?)
    }

    fn class_side_for(&mut self, review: &ReviewArgs, unit_ids: &[i64]) -> Result<ClassSideBySide, ApiError> {
        let spec = ClassSideBySideSpec { review: self.review_spec(review)?, unit_ids: unit_ids.to_vec() };
        Ok(self.project()?.class_side_by_side(&spec)?)
    }

    /// The commands of a class exercise; `None` for a command that is not one of them.
    pub(crate) fn call_class(&mut self, cmd: &str, args: Value) -> Option<ApiResult> {
        Some(match cmd {
            "class.packets" => (|| {
                let a: PacketsArgs = parse(cmd, args)?;
                // The documents go into a packet whole, with any names in them, so the switch that replaces names in exports stops it (as it stops one packet).
                if self.project()?.deid_state()?.exports {
                    return Err(ApiError::new(
                        "invalid",
                        "the list of names to replace is applied to exports, and a packet holds the documents whole; turn that switch off, or replace the names in the project itself first",
                    ));
                }
                let folder = PathBuf::from(a.folder.trim());
                if folder.as_os_str().is_empty() {
                    return Err(ApiError::new("invalid", "say which folder the packets should go in"));
                }
                // The packets are named for the project file, so a student can tell whose study it is.
                let stem = self.project()?.path().file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Class".to_string());
                to_json(&self.project()?.make_class_packets(&folder, &stem, &a.spec)?)
            })(),
            "class.collect" => (|| {
                let a: CollectArgs = parse(cmd, args)?;
                let folder = PathBuf::from(a.folder.trim());
                if folder.as_os_str().is_empty() {
                    return Err(ApiError::new("invalid", "say which folder the students' packets are in"));
                }
                to_json(&self.project()?.collect_class_packets(&folder, a.passphrase.as_deref())?)
            })(),
            "class.review" => (|| {
                let a: ReviewArgs = parse(cmd, args)?;
                to_json(&self.class_review_for(&a)?)
            })(),
            "export.class_review" => (|| {
                let a: SaveArgs = parse(cmd, args)?;
                let review = self.class_review_for(&a.review)?;
                let target = export_target(&a.path, "csv", a.overwrite)?;
                let mut saved = write_file(&target, &students_csv(&review))?;
                saved["rows"] = json!(review.students.len());
                Ok(saved)
            })(),
            // The class heat map and the side-by-side view (decision 198).
            "class.side_by_side" => (|| {
                let a: SideArgs = parse(cmd, args)?;
                to_json(&self.class_side_for(&a.review, &a.unit_ids)?)
            })(),
            "export.class_heat_map" => (|| {
                let a: SaveArgs = parse(cmd, args)?;
                let review = self.class_review_for(&a.review)?;
                let target = export_target(&a.path, "csv", a.overwrite)?;
                let mut saved = write_file(&target, &heat_map_csv(&review))?;
                saved["rows"] = json!(review.heat_map.rows.len());
                saved["columns"] = json!(review.heat_map.codes.len());
                Ok(saved)
            })(),
            "export.class_side_by_side" => (|| {
                let a: SideSaveArgs = parse(cmd, args)?;
                let view = self.class_side_for(&a.review, &a.unit_ids)?;
                let target = export_target(&a.path, "csv", a.overwrite)?;
                let mut saved = write_file(&target, &side_by_side_csv(&view))?;
                saved["rows"] = json!(view.passages.len());
                Ok(saved)
            })(),
            // A Word file of feedback for each student (decision 197): the figures of the review and the passages coded differently, one file each.
            "export.class_feedback" => (|| {
                let a: FeedbackArgs = parse(cmd, args)?;
                let instructor_id = match a.review.instructor_id {
                    Some(id) => id,
                    None => self.me()?,
                };
                // A student reads their own feedback, by name, so the review's own choice to call students by number does not apply here.
                let spec = ClassReviewSpec { instructor_id, student_ids: a.review.student_ids.clone(), source_ids: a.review.source_ids.clone(), anonymous: false, hardest: 0 };
                let feedback = self.project()?.class_feedback(&spec)?;
                let folder = PathBuf::from(a.folder.trim());
                if folder.as_os_str().is_empty() {
                    return Err(ApiError::new("invalid", "say which folder the feedback files should go in"));
                }
                // One file for each student, each named for them; two students of one name get a number.
                let mut used: Vec<String> = Vec::new();
                let mut plan: Vec<(usize, PathBuf)> = Vec::new();
                for (i, student) in feedback.students.iter().enumerate() {
                    let mut name = prismqda_export::feedback_file_name(&student.name);
                    let mut k = 2;
                    while used.contains(&name.to_lowercase()) {
                        name = prismqda_export::feedback_file_name(&format!("{} ({k})", student.name));
                        k += 1;
                    }
                    used.push(name.to_lowercase());
                    plan.push((i, folder.join(name)));
                }
                if !a.overwrite {
                    if let Some((_, existing)) = plan.iter().find(|(_, p)| p.exists()) {
                        return Err(ApiError::new("already_exists", format!("{} already exists, so nothing was saved; choose another folder, or say that files may be replaced", existing.display())));
                    }
                }
                std::fs::create_dir_all(&folder).map_err(|e| ApiError::new("io", format!("cannot make the folder {}: {e}", folder.display())))?;
                let mut files = Vec::new();
                for (i, path) in &plan {
                    let student = &feedback.students[*i];
                    let bytes = prismqda_export::student_feedback_docx(&feedback, student)?;
                    write_file(path, &bytes)?;
                    files.push(json!({ "student": student.name, "path": path.display().to_string(), "bytes": bytes.len(), "passages": student.passages.len() }));
                }
                Ok(json!({ "folder": folder.display().to_string(), "written": files.len(), "files": files, "notes": feedback.notes }))
            })(),
            _ => return None,
        })
    }
}
