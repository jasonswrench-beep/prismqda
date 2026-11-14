//! A class exercise through the command layer (decision 183): a blind packet for every student, bringing the packets back, and the review of
//! the class against the instructor's own coding, which can also be saved as a table.

use std::path::PathBuf;

use prismqda_engine::{ClassPacketsSpec, ClassReview, ClassReviewSpec};
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

impl Api {
    fn class_review_for(&mut self, a: &ReviewArgs) -> Result<ClassReview, ApiError> {
        let instructor_id = match a.instructor_id {
            Some(id) => id,
            None => self.me()?,
        };
        let spec = ClassReviewSpec { instructor_id, student_ids: a.student_ids.clone(), source_ids: a.source_ids.clone(), anonymous: a.anonymous, hardest: a.hardest.unwrap_or(10) };
        Ok(self.project()?.class_review(&spec)?)
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
            _ => return None,
        })
    }
}
