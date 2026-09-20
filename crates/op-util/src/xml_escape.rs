//! Canonical HTML/XML text escaping.
//!
//! Two variants cover the call sites that used to keep private copies:
//! [`escape_html`] is the 4-entity form (`& < > "`) — attribute-safe for
//! double-quoted attributes and element text; [`escape_xml`] additionally
//! escapes `'` for single-quoted XML attribute contexts (SVG export).

/// Escape `& < > "` — safe for HTML element text and double-quoted
/// attribute values.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// Escape `& < > " '` — safe for any XML text or attribute context.
pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_four_entities() {
        assert_eq!(
            escape_html(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;'"
        );
    }

    #[test]
    fn xml_also_escapes_apostrophe() {
        assert_eq!(escape_xml("it's <b>"), "it&apos;s &lt;b&gt;");
    }
}
