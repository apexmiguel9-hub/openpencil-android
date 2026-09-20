//! Quote- and escape-aware tokenizing helpers shared by the cascade modules
//! (`cascade`, `cascade_parser`, `cascade_conditions`, and the deferred
//! declaration resolver). Each helper treats CSS strings as opaque so quoted
//! delimiters never confuse bracket matching or top-level splitting.

/// Returns the index of the `right` byte that closes the `left` byte opened at
/// or after `open`, scanning `input[open..end]` with quote/escape awareness.
pub(super) fn matching(input: &str, open: usize, left: u8, right: u8, end: usize) -> Option<usize> {
    let (left, right) = (left as char, right as char);
    let (mut depth, mut quote, mut escaped) = (0u32, None, false);
    for (at, ch) in input[open..end]
        .char_indices()
        .map(|(at, ch)| (at + open, ch))
    {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            ch if ch == left => depth += 1,
            ch if ch == right => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(at);
                }
            }
            _ => {}
        }
    }
    None
}

/// Returns the index of the first `delimiter` that sits outside every paren,
/// bracket, brace, and quoted string.
pub(super) fn top_level_delimiter(input: &str, delimiter: char) -> Option<usize> {
    let (mut parens, mut brackets, mut braces) = (0u32, 0u32, 0u32);
    let (mut quote, mut escaped) = (None, false);
    for (at, ch) in input.char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == active {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => quote = Some(ch),
            '(' => parens += 1,
            ')' => parens = parens.saturating_sub(1),
            '[' => brackets += 1,
            ']' => brackets = brackets.saturating_sub(1),
            '{' => braces += 1,
            '}' => braces = braces.saturating_sub(1),
            _ if ch == delimiter && parens == 0 && brackets == 0 && braces == 0 => {
                return Some(at);
            }
            _ => {}
        }
    }
    None
}

/// Splits `input` on a top-level separator: a literal `,` or an identifier
/// keyword (matched case-insensitively at identifier boundaries).
pub(super) fn split_top_level<'a>(input: &'a str, separator: &str) -> Vec<&'a str> {
    let bytes = input.as_bytes();
    let (mut start, mut depth, mut quote, mut escaped) = (0, 0u32, None, false);
    let mut parts = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let byte = bytes[at];
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == active {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' => depth += 1,
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            let hit = depth == 0
                && if separator == "," {
                    byte == b','
                } else {
                    keyword_at(input, at, separator)
                };
            if hit {
                parts.push(input[start..at].trim());
                at += separator.len();
                start = at;
                continue;
            }
        }
        at += 1;
    }
    parts.push(input[start..].trim());
    parts
}

/// Reports whether `keyword` appears at byte `at`, case-insensitively and
/// bounded by non-identifier characters on both sides.
pub(super) fn keyword_at(input: &str, at: usize, keyword: &str) -> bool {
    let Some(candidate) = input.get(at..at + keyword.len()) else {
        return false;
    };
    candidate.eq_ignore_ascii_case(keyword)
        && input[..at]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_ident(ch))
        && input[at + keyword.len()..]
            .chars()
            .next()
            .is_none_or(|ch| !is_ident(ch))
}

/// CSS identifier characters: alphanumerics, `-`, `_`, and any non-ASCII.
pub(super) fn is_ident(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '-' | '_') || !ch.is_ascii()
}
