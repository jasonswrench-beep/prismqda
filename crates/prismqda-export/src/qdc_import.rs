//! Reading a REFI-QDA codebook (`.qdc`) into the open project (decision 160).
//!
//! The codebook of another program, or one PrismQDA wrote earlier, comes in as codes: the tree is kept, a code that cannot be applied
//! becomes a theme, the colour and the description are taken (the description's `Include when:` and `Do not include when:` lines go back
//! into the two criteria). A code that is already at that place in the codebook, by name, is left as it is and its children are looked
//! for under it, so reading the same file twice adds nothing the second time. Nothing in the file is run or fetched.
//!
//! A file is read whole or not at all: its size and depth are checked before any code is added, and if a code still cannot be added part-way
//! (a full disk, say), the codes this run added are undone.

use std::collections::HashMap;

use prismqda_engine::{NewCode, Project};
use serde::Serialize;

use crate::qdpx_import::{bad, parse_color, parse_tree, split_description, Node, MAX_DEPTH};
use crate::ExportError;

/// The most a codebook file may be. A real one is a few hundred kilobytes.
const MAX_QDC: usize = 64 << 20;
/// The most codes one file may add, so a hostile file cannot fill a project.
const MAX_CODES: usize = 100_000;

/// What a codebook file brought into the project.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct QdcImportReport {
    /// Codes added to the codebook.
    pub added: usize,
    /// Of those, the ones that group others (a code that cannot be applied).
    pub themes: usize,
    /// Codes already at that place in the codebook, left as they are.
    pub already_there: usize,
    /// What the file held that PrismQDA has no place for, one line each.
    pub left_out: Vec<String>,
}

struct Walk<'a> {
    project: &'a mut Project,
    /// An active code, by its parent and its name in lower case.
    existing: HashMap<(Option<i64>, String), i64>,
    report: QdcImportReport,
    seen: usize,
    max_codes: usize,
}

/// Checks the depth and the number of codes of a file before any is added, so that a file the walk would refuse half-way is refused whole.
fn check_shape(node: &Node, depth: usize, seen: &mut usize, max_codes: usize) -> Result<(), ExportError> {
    if depth > MAX_DEPTH {
        return Err(bad("the codebook nests too deeply, so it is damaged or hostile"));
    }
    for c in node.kids("Code") {
        *seen += 1;
        if *seen > max_codes {
            return Err(bad("the codebook holds more codes than PrismQDA reads from one file"));
        }
        check_shape(c, depth + 1, seen, max_codes)?;
    }
    Ok(())
}

impl Walk<'_> {
    fn codes(&mut self, node: &Node, parent: Option<i64>, depth: usize) -> Result<(), ExportError> {
        if depth > MAX_DEPTH {
            return Err(bad("the codebook nests too deeply, so it is damaged or hostile"));
        }
        for c in node.kids("Code") {
            self.seen += 1;
            if self.seen > self.max_codes {
                return Err(bad("the codebook holds more codes than PrismQDA reads from one file"));
            }
            let name = c.attr("name").unwrap_or("").trim();
            let name = if name.is_empty() { "Code" } else { name };
            let key = (parent, name.to_lowercase());
            let id = if let Some(&id) = self.existing.get(&key) {
                self.report.already_there += 1;
                id
            } else {
                let (definition, include, exclude) = split_description(c.kid("Description").map(|d| d.text.as_str()).unwrap_or(""));
                let theme = c.attr("isCodable").map(|v| v.trim() == "false").unwrap_or(false);
                let spec = NewCode {
                    name: name.to_string(),
                    parent_id: parent,
                    theme,
                    color: parse_color(c.attr("color")),
                    definition,
                    inclusion_criteria: include,
                    exclusion_criteria: exclude,
                    exclusive_group: String::new(),
                    rated: false,
                    rating_label: String::new(),
                    rating_low: String::new(),
                    rating_high: String::new(),
                };
                let id = self.project.add_code(&spec)?;
                self.existing.insert(key, id);
                self.report.added += 1;
                if theme {
                    self.report.themes += 1;
                }
                id
            };
            self.codes(c, Some(id), depth + 1)?;
        }
        Ok(())
    }
}

/// Reads a `.qdc` file into the open project. The project must be writable.
pub fn import_qdc(project: &mut Project, qdc: &[u8]) -> Result<QdcImportReport, ExportError> {
    import_with_limits(project, qdc, MAX_CODES, MAX_CODES)
}

/// `check_max` is the most codes the check before the walk allows and `walk_max` the most the walk allows; they are the same in use, and a
/// test sets them apart to make the walk fail part-way.
fn import_with_limits(project: &mut Project, qdc: &[u8], check_max: usize, walk_max: usize) -> Result<QdcImportReport, ExportError> {
    if qdc.len() > MAX_QDC {
        return Err(bad("this codebook file is over 64 MB, far more than a codebook holds"));
    }
    if qdc.starts_with(&[0xFF, 0xFE]) || qdc.starts_with(&[0xFE, 0xFF]) {
        return Err(bad("this codebook is saved as UTF-16; save it again as UTF-8 and bring it in once more"));
    }
    let bytes = qdc.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(qdc);
    let xml = std::str::from_utf8(bytes).map_err(|_| bad("this file is not UTF-8 text, so it is not a codebook"))?;
    let root = parse_tree(xml, "this file")?;
    match root.name.as_str() {
        "CodeBook" => {}
        "Project" => return Err(bad("this is a whole project (.qde), not a codebook; use Import QDPX for a project")),
        other => return Err(bad(format!("this is not a REFI-QDA codebook: its first element is '{other}', not 'CodeBook'"))),
    }
    let Some(codes) = root.kid("Codes") else {
        return Err(bad("this codebook has no Codes element, so there is nothing to bring in"));
    };
    check_shape(codes, 0, &mut 0, check_max)?;
    let mut existing = HashMap::new();
    for c in project.list_codes()? {
        if c.status == "active" {
            existing.insert((c.parent_id, c.name.to_lowercase()), c.id);
        }
    }
    let mut walk = Walk { project, existing, report: QdcImportReport::default(), seen: 0, max_codes: walk_max };
    if let Err(e) = walk.codes(codes, None, 0) {
        // Put the codebook back as it was: undo each code this run added, the newest first.
        for _ in 0..walk.report.added {
            if walk.project.undo().is_err() {
                break;
            }
        }
        return Err(e);
    }
    let mut report = walk.report;
    // The standard's codebook also carries sets; they have no place in a QDC file's code tree here.
    let sets = root.kid("Sets").map(|s| s.kids("Set").count()).unwrap_or(0);
    if sets > 0 {
        report.left_out.push(format!("{sets} set{} of codes (a set is taken from a whole project, not from a codebook file)", if sets == 1 { "" } else { "s" }));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prismqda_engine::{NewProject, OpenOptions};

    fn project(dir: &tempfile::TempDir) -> Project {
        Project::create(dir.path().join("p.pqda"), &NewProject::new("Study"), &OpenOptions::new("test")).unwrap()
    }

    fn names(p: &Project) -> Vec<String> {
        let mut v: Vec<String> = p.list_codes().unwrap().into_iter().filter(|c| c.status == "active").map(|c| c.name).collect();
        v.sort();
        v
    }

    const FILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<CodeBook xmlns="urn:QDA-XML:codebook:1.0"><Codes>
  <Code name="One" isCodable="true"><Code name="Two" isCodable="true"/></Code>
  <Code name="Three" isCodable="true"/>
  <Code name="Four" isCodable="true"/>
</Codes></CodeBook>"#;

    #[test]
    fn a_file_with_more_codes_than_allowed_is_refused_before_any_code_is_added() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = project(&dir);
        let before = names(&p);
        let e = import_with_limits(&mut p, FILE.as_bytes(), 3, 3).unwrap_err();
        assert!(e.to_string().contains("more codes"), "{e}");
        assert_eq!(names(&p), before, "nothing was added, not even the codes before the limit");
    }

    #[test]
    fn a_walk_that_fails_part_way_undoes_the_codes_it_added() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = project(&dir);
        let before = names(&p);
        // the check lets the file through; the walk stops after the third code, as a full disk would stop it
        let e = import_with_limits(&mut p, FILE.as_bytes(), 100, 3).unwrap_err();
        assert!(e.to_string().contains("more codes"), "{e}");
        assert_eq!(names(&p), before, "the three codes added before the failure were taken back");
        // and the same file, with room, still comes in whole
        let r = import_with_limits(&mut p, FILE.as_bytes(), 100, 100).unwrap();
        assert_eq!(r.added, 4);
        assert_eq!(names(&p), ["Four", "One", "Three", "Two"]);
    }
}
