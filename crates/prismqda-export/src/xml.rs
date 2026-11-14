//! Small helpers for writing XML by hand.

/// Escapes text for an XML element or attribute, and drops the control characters XML 1.0 cannot hold
/// (everything below space except tab, newline, and carriage return, plus the two non-characters).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' | '\n' | '\r' => out.push(c),
            c if (c as u32) < 0x20 || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_characters_are_escaped_and_illegal_ones_dropped() {
        assert_eq!(escape("a < b & c > \"d\" 'e'"), "a &lt; b &amp; c &gt; &quot;d&quot; &apos;e&apos;");
        assert_eq!(escape("tab\there\nnew\rline"), "tab\there\nnew\rline");
        assert_eq!(escape("bell\u{7} nul\u{0} ok\u{FFFE}"), "bell nul ok");
        assert_eq!(escape("caf\u{e9} \u{1F600}"), "caf\u{e9} \u{1F600}");
    }
}
