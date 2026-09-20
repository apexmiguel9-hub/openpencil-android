//! Hand-rolled XML lexing: flat element scan, tag/attribute parsing
//! and the numeric scanners shared by the shape and path readers.

use super::*;

/// Flat scan of every `<tag …>` / `<tag … />` element in `svg`.
/// Comments, the XML prolog, closing tags and DOCTYPE are skipped;
/// nesting is ignored, so a shape inside a `<g>` is still found.
#[allow(dead_code)] // Kept for tests / fallback callers; the TS-parity tree walker is `parse_svg_tree`.
pub(super) fn parse_svg_elements(svg: &str) -> Vec<SvgElement> {
    let bytes = svg.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        // Skip comments `<!-- … -->`.
        if svg[i..].starts_with("<!--") {
            match svg[i..].find("-->") {
                Some(rel) => i += rel + 3,
                None => break,
            }
            continue;
        }
        // Closing tag / prolog / DOCTYPE — skip to the matching `>`.
        if matches!(bytes.get(i + 1), Some(b'/') | Some(b'?') | Some(b'!')) {
            match svg[i..].find('>') {
                Some(rel) => i += rel + 1,
                None => break,
            }
            continue;
        }
        // Open / self-closing element: read until the matching `>`,
        // honouring quoted attribute values.
        let Some(end) = find_tag_end(bytes, i + 1) else {
            break;
        };
        let inner = &svg[i + 1..end];
        if let Some(el) = parse_element(inner) {
            out.push(el);
        }
        i = end + 1;
    }
    out
}

/// Index of the `>` that closes a tag started at `from`, skipping any
/// `>` that sits inside a quoted attribute value.
pub(super) fn find_tag_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let c = bytes[i];
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None => match c {
                b'"' | b'\'' => quote = Some(c),
                b'>' => return Some(i),
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// Parse the inside of a tag (`rect x="1" y="2" /`) into tag name +
/// attribute pairs.
pub(super) fn parse_element(inner: &str) -> Option<SvgElement> {
    let trimmed = inner.trim().trim_end_matches('/').trim();
    // Tag name runs up to the first whitespace.
    let name_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    let tag = trimmed[..name_end].to_ascii_lowercase();
    if tag.is_empty() {
        return None;
    }
    Some(SvgElement {
        tag,
        attrs: parse_attrs(&trimmed[name_end..]),
    })
}

/// Parse `key="value"` / `key='value'` pairs from an attribute run.
pub(super) fn parse_attrs(s: &str) -> Vec<(String, String)> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        // Skip to a key start.
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && bytes[i] != b'=' && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if key_start == i {
            break;
        }
        let key = s[key_start..i].to_ascii_lowercase();
        // Skip whitespace + the `=`.
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue; // valueless attribute — ignore
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let quote = bytes[i];
        if quote != b'"' && quote != b'\'' {
            continue;
        }
        i += 1;
        let val_start = i;
        while i < bytes.len() && bytes[i] != quote {
            i += 1;
        }
        let value = s[val_start..i.min(s.len())].to_string();
        out.push((key, value));
        i += 1;
    }
    out
}

/// Parse an SVG `points` list (`"1,2 3,4"` / `"1 2 3 4"`).
pub(super) fn parse_point_list(s: &str) -> Vec<(f64, f64)> {
    let nums = scan_numbers(s);
    nums.chunks_exact(2).map(|c| (c[0], c[1])).collect()
}

/// Scan every number out of `s`, treating commas + whitespace as
/// separators. Tolerates the SVG quirks: a leading `.`, a `-` that
/// starts a new number, and scientific `e` notation.
fn scan_numbers(s: &str) -> Vec<f64> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            let start = i;
            i += 1;
            let mut seen_dot = c == b'.';
            let mut seen_exp = false;
            while i < bytes.len() {
                let d = bytes[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == b'.' && !seen_dot && !seen_exp {
                    seen_dot = true;
                    i += 1;
                } else if (d == b'e' || d == b'E') && !seen_exp {
                    seen_exp = true;
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(n) = s[start..i].parse::<f64>() {
                out.push(n);
            }
        } else {
            i += 1;
        }
    }
    out
}
