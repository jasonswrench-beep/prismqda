//! The codebook in the REFI-QDA codebook exchange format (`.qdc`), so it can move to other qualitative
//! analysis tools. One `Code` element per code, nested like the tree, each with a stable identifier.

use prismqda_engine::sha256_hex;

use crate::xml::escape;

/// One code, ready to write.
#[derive(Debug, Clone)]
pub struct QdcCode {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub color: String,
    pub description: String,
    /// False for a theme, which groups other codes and is not applied to text itself.
    pub codable: bool,
}

/// A repeatable identifier in the form `8-4-4-4-12`, made from the project's identifier and the code's number, so
/// exporting the same codebook twice gives the same identifiers and another tool can tell the codes apart.
pub fn guid(project_uid: &str, code_id: i64) -> String {
    let hex = sha256_hex(format!("{project_uid}:code:{code_id}").as_bytes());
    let h = &hex[..32];
    // Version 5 and the RFC 4122 variant bits, so the value is a well-formed UUID.
    let variant = ['8', '9', 'a', 'b'][usize::from_str_radix(&h[16..17], 16).unwrap_or(0) % 4];
    format!("{}-{}-5{}-{variant}{}-{}", &h[0..8], &h[8..12], &h[13..16], &h[17..20], &h[20..32])
}

fn valid_color(c: &str) -> Option<String> {
    let c = c.trim();
    (c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())).then(|| c.to_lowercase())
}

fn write_code(out: &mut String, code: &QdcCode, all: &[QdcCode], project_uid: &str, depth: usize) {
    let pad = "  ".repeat(depth + 2);
    out.push_str(&format!("{pad}<Code guid=\"{}\" name=\"{}\" isCodable=\"{}\"", guid(project_uid, code.id), escape(&crate::scrub::text(&code.name)), code.codable));
    if let Some(c) = valid_color(&code.color) {
        out.push_str(&format!(" color=\"{c}\""));
    }
    let children: Vec<&QdcCode> = all.iter().filter(|c| c.parent_id == Some(code.id)).collect();
    if code.description.trim().is_empty() && children.is_empty() {
        out.push_str("/>\n");
        return;
    }
    out.push_str(">\n");
    if !code.description.trim().is_empty() {
        out.push_str(&format!("{pad}  <Description>{}</Description>\n", escape(&crate::scrub::text(&code.description))));
    }
    for child in children {
        write_code(out, child, all, project_uid, depth + 1);
    }
    out.push_str(&format!("{pad}</Code>\n"));
}

/// The codebook as QDC text. `codes` must be in the order they should appear among their siblings.
pub fn write_qdc(codes: &[QdcCode], project_uid: &str, origin: &str) -> String {
    let known: std::collections::HashSet<i64> = codes.iter().map(|c| c.id).collect();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    out.push_str(&format!("<CodeBook xmlns=\"urn:QDA-XML:codebook:1.0\" origin=\"{}\">\n  <Codes>\n", escape(origin)));
    // A code whose parent is not in the list (retired, say) goes at the top.
    for code in codes.iter().filter(|c| c.parent_id.is_none_or(|p| !known.contains(&p))) {
        write_code(&mut out, code, codes, project_uid, 0);
    }
    out.push_str("  </Codes>\n</CodeBook>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_stable_well_formed_uuids_that_differ_by_code_and_project() {
        let a = guid("project-1", 1);
        assert_eq!(a, guid("project-1", 1));
        assert_ne!(a, guid("project-1", 2));
        assert_ne!(a, guid("project-2", 1));
        let parts: Vec<&str> = a.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), [8, 4, 4, 4, 12]);
        assert!(parts[2].starts_with('5'), "version 5: {a}");
        assert!("89ab".contains(parts[3].chars().next().unwrap()), "RFC 4122 variant: {a}");
        assert!(a.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
    }

    #[test]
    fn a_code_with_an_unlisted_parent_is_written_at_the_top() {
        let codes = vec![QdcCode { id: 5, name: "Orphan".into(), parent_id: Some(99), color: "#abcdef".into(), description: String::new(), codable: true }];
        let xml = write_qdc(&codes, "p", "test");
        assert!(xml.contains("name=\"Orphan\"") && xml.contains("color=\"#abcdef\""));
    }

    #[test]
    fn bad_colors_are_left_out_not_written() {
        assert_eq!(valid_color("#ABCDEF"), Some("#abcdef".to_string()));
        assert_eq!(valid_color("red"), None);
        assert_eq!(valid_color("#12345"), None);
    }
}
