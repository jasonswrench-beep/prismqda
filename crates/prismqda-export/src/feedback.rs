//! A student's feedback on a class exercise, as a Word file (decision 197, teaching mode).
//!
//! It is made from the comparison the class review makes ([`prismqda_engine::ClassFeedback`]): the figures are that review's, and the passages are the
//! ones the student coded differently from the instructor. A student reads their own file, so it carries their name and nobody else's: no other
//! student, no class figure, and no name of an instructor's coder other than the instructor's.

use prismqda_engine::{ClassFeedback, FeedbackPassage, StudentFeedback};

use crate::docx::{write_docx, Block, Run};
use crate::ExportError;

fn names(codes: &[prismqda_engine::CodeRef]) -> String {
    if codes.is_empty() {
        "none".to_string()
    } else {
        codes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")
    }
}

fn percent(share: Option<f64>) -> String {
    share.map_or_else(String::new, |s| format!("{:.0} percent", s * 100.0))
}

/// The sections of the passages, in the order a student should read them: what was left out, then what was coded otherwise, then what only
/// partly agreed, then what the instructor did not code.
const SECTIONS: [(&str, &str, &str); 4] = [
    ("left_uncoded", "Passages you left uncoded", "The instructor gave these a code and you did not."),
    ("different", "Passages you coded with other codes", "You gave these codes that are not the instructor's, and none of the instructor's."),
    ("partly", "Passages where some of your codes match", "You gave these some of the instructor's codes and not all, or others besides."),
    ("extra", "Passages the instructor did not code", "You coded these and the instructor did not. That may be a good find or a passage to leave alone; look at the words."),
];

fn passage_blocks(p: &FeedbackPassage) -> Vec<Block> {
    vec![
        Block::Quote { text: p.text.clone(), source: Some(p.source_name.clone()) },
        Block::Paragraph(vec![Run::bold("The instructor's codes: "), Run::plain(names(&p.instructor_codes))]),
        Block::Paragraph(vec![Run::bold("Your codes: "), Run::plain(names(&p.student_codes))]),
    ]
}

/// The Word file for one student.
pub fn student_feedback_docx(class: &ClassFeedback, student: &StudentFeedback) -> Result<Vec<u8>, ExportError> {
    let s = &student.summary;
    let teacher = if class.instructor.name.trim().is_empty() { "the instructor".to_string() } else { class.instructor.name.clone() };
    let mut blocks = vec![
        Block::Heading(1, "Feedback on the coding exercise".to_string()),
        Block::Paragraph(vec![Run::bold("For "), Run::plain(student.name.clone())]),
        Block::paragraph(format!(
            "Your coding is compared with the coding of {teacher}. The comparison is made passage by passage. A passage is a unit of the text as it was imported (a paragraph, a turn, or a sentence), and you have given a passage a code when a coding of yours with that code lies in it. {teacher} coded {} passages; {} passages were coded by someone in all.",
            class.instructor_passages, class.passages_compared
        )),
        Block::Heading(2, "How your coding compares".to_string()),
        Block::Table {
            header: vec!["".to_string(), "Passages".to_string()],
            rows: vec![
                vec!["Passages you coded".to_string(), s.passages_coded.to_string()],
                vec![format!("Coded with the same codes as {teacher}"), format!("{} ({} of the {} passages)", s.same, percent(s.share_same), class.instructor_passages)],
                vec!["Coded with some of the same codes".to_string(), s.partly.to_string()],
                vec!["Coded with other codes only".to_string(), s.different.to_string()],
                vec![format!("Left uncoded ({teacher} coded them)"), s.left_uncoded.to_string()],
                vec![format!("Coded where {teacher} did not"), s.extra.to_string()],
            ],
        },
        Block::paragraph(format!(
            "Counting codes rather than passages: you gave {} of the {} codes {teacher} gave ({}), missed {}, and added {} that {teacher} did not give.",
            s.found,
            s.found + s.missed,
            percent(s.share_found),
            s.missed,
            s.added
        )),
    ];

    if student.passages.is_empty() {
        blocks.push(Block::Heading(2, "Passages to look at again".to_string()));
        blocks.push(Block::paragraph(format!("Every passage you coded matches the coding of {teacher}, and you left none of {teacher}'s passages uncoded.")));
    } else {
        blocks.push(Block::Heading(2, "Passages to look at again".to_string()));
        blocks.push(Block::paragraph(format!(
            "These are the {} passages where your codes were not the same as {teacher}'s, with the words of each. Read the words first, then the two sets of codes. A difference is a place to think about what the code means, not always a mistake.",
            student.passages.len()
        )));
        for (status, title, help) in SECTIONS {
            let mine: Vec<&FeedbackPassage> = student.passages.iter().filter(|p| p.status == status).collect();
            if mine.is_empty() {
                continue;
            }
            blocks.push(Block::Heading(3, format!("{title} ({})", mine.len())));
            blocks.push(Block::Paragraph(vec![Run { text: help.to_string(), bold: false, italic: true }]));
            for p in mine {
                blocks.extend(passage_blocks(p));
            }
        }
    }

    let notes: Vec<&String> = class.notes.iter().filter(|n| !n.starts_with("A passage is a unit")).collect();
    if !notes.is_empty() {
        blocks.push(Block::Heading(2, "About this comparison".to_string()));
        for n in notes {
            blocks.push(Block::paragraph(n.clone()));
        }
    }
    write_docx(&format!("Feedback for {}", student.name), &blocks)
}

/// A file name for a student: the name, with what Windows does not allow in a file name made harmless, and `feedback` after it.
pub fn feedback_file_name(student: &str) -> String {
    let cleaned: String = student.chars().map(|c| if c.is_control() || matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '-' } else { c }).collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    let cleaned: String = cleaned.chars().take(80).collect();
    let cleaned = if cleaned.trim().is_empty() { "Student".to_string() } else { cleaned.trim().to_string() };
    format!("{cleaned} - feedback.docx")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_name_is_made_from_the_name_without_what_windows_refuses() {
        assert_eq!(feedback_file_name("Ada Lovelace"), "Ada Lovelace - feedback.docx");
        assert_eq!(feedback_file_name("A/B: C?"), "A-B- C- - feedback.docx");
        assert_eq!(feedback_file_name("  .. "), "Student - feedback.docx");
        assert_eq!(feedback_file_name(&"x".repeat(200)).chars().count(), 80 + " - feedback.docx".len());
    }
}
