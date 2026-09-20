//! Canonical JSON string-literal escaping.
//!
//! Single-sources the escaping previously copy-pasted in op-mcp
//! (`json_serializer`), op-cli (`mcp_http_cli`), and op-host-services
//! (`mcp_serve::doc_sync` — whose copy lossily replaced control characters
//! with spaces; that one was a bug). Control characters below U+0020 get
//! the mandatory `\uXXXX` escapes so the output is always valid JSON.

/// Escape `s` for inclusion inside a JSON string literal (no surrounding
/// quotes added).
pub fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    push_escaped(&mut out, s);
    out
}

/// Escape `s` as a complete JSON string literal, surrounding quotes
/// included.
pub fn escape_json_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    push_escaped(&mut out, s);
    out.push('"');
    out
}

fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_specials_and_controls() {
        assert_eq!(escape_json("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape_json("line1\nline2\r\t"), "line1\\nline2\\r\\t");
        assert_eq!(escape_json("nul\u{0}bell\u{7}"), "nul\\u0000bell\\u0007");
        assert_eq!(escape_json("中文 ok"), "中文 ok");
    }

    #[test]
    fn quoted_variant_wraps() {
        assert_eq!(escape_json_quoted("hi"), "\"hi\"");
        assert_eq!(escape_json_quoted("a\"b"), "\"a\\\"b\"");
    }
}
