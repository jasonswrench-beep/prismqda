//! CSV that Excel opens correctly: UTF-8 with a byte order mark, comma separated, CRLF line ends, and every
//! field quoted when it holds a comma, a quote, or a line break.

/// One table as CSV bytes.
pub fn write_csv(header: &[String], rows: &[Vec<String>]) -> Vec<u8> {
    let mut out = String::from("\u{FEFF}");
    let mut line = |fields: &[String]| {
        let cells: Vec<String> = fields.iter().map(|f| quote(&crate::scrub::text(f))).collect();
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    };
    line(header);
    for row in rows {
        line(row);
    }
    out.into_bytes()
}

fn quote(field: &str) -> String {
    // Text that starts like a formula would be run by a spreadsheet; a leading apostrophe stops that. A plain
    // number such as -5 or +3.5 is left alone.
    let risky = field.starts_with(['=', '@', '\t', '\r']) || (field.starts_with(['+', '-']) && field.trim().parse::<f64>().is_err());
    let guarded = if risky { format!("'{field}") } else { field.to_string() };
    if guarded.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn fields_with_commas_quotes_and_line_breaks_are_quoted() {
        let bytes = write_csv(&s(&["a", "b"]), &[s(&["x,y", "say \"hi\""]), s(&["two\nlines", "plain"])]);
        let text = String::from_utf8(bytes).unwrap();
        assert_eq!(text, "\u{FEFF}a,b\r\n\"x,y\",\"say \"\"hi\"\"\"\r\n\"two\nlines\",plain\r\n");
    }

    #[test]
    fn text_that_a_spreadsheet_would_run_as_a_formula_is_defused_but_numbers_are_left_alone() {
        let text = String::from_utf8(write_csv(&s(&["v"]), &[s(&["=1+1"]), s(&["-5"]), s(&["@cmd"]), s(&["+3.5"]), s(&["- dash"])])).unwrap();
        assert_eq!(text, "\u{FEFF}v\r\n'=1+1\r\n-5\r\n'@cmd\r\n+3.5\r\n'- dash\r\n");
    }
}
