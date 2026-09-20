//! Encoding sniffing for HTML byte streams.

use std::borrow::Cow;

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};

const META_SCAN_LIMIT: usize = 1024;

/// Decode an HTML byte stream using its BOM or an early `meta` declaration.
///
/// Modern HTML without either signal is treated as UTF-8 when it is valid.
/// Otherwise the browser-compatible Windows-1252 fallback is used. Invalid
/// byte sequences are replaced, matching browser text decoding behavior.
pub fn decode_html_bytes(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(payload) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return decode_with(UTF_8, payload);
    }
    if let Some(payload) = bytes.strip_prefix(b"\xFF\xFE") {
        return decode_with(UTF_16LE, payload);
    }
    if let Some(payload) = bytes.strip_prefix(b"\xFE\xFF") {
        return decode_with(UTF_16BE, payload);
    }

    if let Some(encoding) = sniff_meta_encoding(bytes) {
        return decode_with(encoding, bytes);
    }
    if let Ok(source) = std::str::from_utf8(bytes) {
        return Cow::Borrowed(source);
    }
    decode_with(WINDOWS_1252, bytes)
}

fn decode_with<'a>(encoding: &'static Encoding, bytes: &'a [u8]) -> Cow<'a, str> {
    encoding.decode_without_bom_handling(bytes).0
}

fn sniff_meta_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    let input = &bytes[..bytes.len().min(META_SCAN_LIMIT)];
    let mut cursor = 0;
    while cursor < input.len() {
        let Some(relative) = input[cursor..].iter().position(|byte| *byte == b'<') else {
            break;
        };
        let start = cursor + relative;
        if input
            .get(start..start.saturating_add(4))
            .is_some_and(|prefix| prefix == b"<!--")
        {
            cursor = find_ascii(input, start + 4, b"-->").unwrap_or(input.len());
            cursor = cursor.saturating_add(3).min(input.len());
            continue;
        }

        let name_start = start + 1;
        if !starts_with_ascii_case_insensitive(input, name_start, b"meta")
            || input
                .get(name_start + 4)
                .is_some_and(|byte| !is_ascii_space(*byte) && !matches!(*byte, b'/' | b'>'))
        {
            cursor = start + 1;
            continue;
        }
        let end = find_tag_end(input, name_start + 4);
        if let Some(encoding) = encoding_from_meta_attributes(&input[name_start + 4..end]) {
            return Some(encoding);
        }
        cursor = end.saturating_add(1);
    }
    None
}

fn encoding_from_meta_attributes(attributes: &[u8]) -> Option<&'static Encoding> {
    let mut charset = None;
    let mut content = None;
    let mut http_equiv = None;
    let mut cursor = 0;

    while cursor < attributes.len() {
        skip_ascii_space_and_slashes(attributes, &mut cursor);
        if cursor >= attributes.len() {
            break;
        }
        let name_start = cursor;
        while cursor < attributes.len()
            && !is_ascii_space(attributes[cursor])
            && !matches!(attributes[cursor], b'/' | b'=' | b'>')
        {
            cursor += 1;
        }
        if name_start == cursor {
            cursor += 1;
            continue;
        }
        let name = &attributes[name_start..cursor];
        while cursor < attributes.len() && is_ascii_space(attributes[cursor]) {
            cursor += 1;
        }
        let value = if attributes.get(cursor) == Some(&b'=') {
            cursor += 1;
            while cursor < attributes.len() && is_ascii_space(attributes[cursor]) {
                cursor += 1;
            }
            parse_attribute_value(attributes, &mut cursor)
        } else {
            &[][..]
        };

        if eq_ascii_case_insensitive(name, b"charset") && charset.is_none() {
            charset = Some(value);
        } else if eq_ascii_case_insensitive(name, b"content") && content.is_none() {
            content = Some(value);
        } else if eq_ascii_case_insensitive(name, b"http-equiv") && http_equiv.is_none() {
            http_equiv = Some(value);
        }
    }

    if let Some(label) = charset {
        return encoding_for_html_label(label);
    }
    let pragma_is_content_type = http_equiv
        .is_some_and(|value| eq_ascii_case_insensitive(trim_ascii(value), b"content-type"));
    if pragma_is_content_type {
        return content
            .and_then(extract_charset_from_content)
            .and_then(encoding_for_html_label);
    }
    None
}

fn parse_attribute_value<'a>(input: &'a [u8], cursor: &mut usize) -> &'a [u8] {
    let Some(first) = input.get(*cursor).copied() else {
        return &[];
    };
    if matches!(first, b'\'' | b'"') {
        *cursor += 1;
        let start = *cursor;
        while *cursor < input.len() && input[*cursor] != first {
            *cursor += 1;
        }
        let value = &input[start..*cursor];
        *cursor = cursor.saturating_add(1).min(input.len());
        value
    } else {
        let start = *cursor;
        while *cursor < input.len() && !is_ascii_space(input[*cursor]) && input[*cursor] != b'>' {
            *cursor += 1;
        }
        &input[start..*cursor]
    }
}

fn extract_charset_from_content(content: &[u8]) -> Option<&[u8]> {
    let mut cursor = 0;
    while cursor + b"charset".len() <= content.len() {
        if !starts_with_ascii_case_insensitive(content, cursor, b"charset") {
            cursor += 1;
            continue;
        }
        let before_is_boundary = cursor == 0 || !is_ascii_name_byte(content[cursor - 1]);
        let after = cursor + b"charset".len();
        let after_is_boundary = content
            .get(after)
            .is_none_or(|byte| !is_ascii_name_byte(*byte));
        if !before_is_boundary || !after_is_boundary {
            cursor += 1;
            continue;
        }
        let mut value_start = after;
        while content
            .get(value_start)
            .is_some_and(|byte| is_ascii_space(*byte))
        {
            value_start += 1;
        }
        if content.get(value_start) != Some(&b'=') {
            cursor = after;
            continue;
        }
        value_start += 1;
        while content
            .get(value_start)
            .is_some_and(|byte| is_ascii_space(*byte))
        {
            value_start += 1;
        }
        let quote = content
            .get(value_start)
            .copied()
            .filter(|byte| matches!(*byte, b'\'' | b'"'));
        if quote.is_some() {
            value_start += 1;
        }
        let mut value_end = value_start;
        while let Some(byte) = content.get(value_end) {
            if quote.map_or_else(
                || is_ascii_space(*byte) || *byte == b';',
                |quote| *byte == quote,
            ) {
                break;
            }
            value_end += 1;
        }
        return (value_start < value_end).then_some(&content[value_start..value_end]);
    }
    None
}

fn encoding_for_html_label(label: &[u8]) -> Option<&'static Encoding> {
    let label = trim_ascii(label);
    if eq_ascii_case_insensitive(label, b"x-user-defined") {
        return Some(WINDOWS_1252);
    }
    let encoding = Encoding::for_label(label)?;
    if encoding == UTF_16LE || encoding == UTF_16BE {
        Some(UTF_8)
    } else {
        Some(encoding)
    }
}

fn find_tag_end(input: &[u8], mut cursor: usize) -> usize {
    let mut quote = None;
    while cursor < input.len() {
        let byte = input[cursor];
        match quote {
            Some(active) if byte == active => quote = None,
            Some(_) => {}
            None if matches!(byte, b'\'' | b'"') => quote = Some(byte),
            None if byte == b'>' => return cursor,
            None => {}
        }
        cursor += 1;
    }
    input.len()
}

fn find_ascii(input: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    input
        .get(start..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| start + position)
}

fn starts_with_ascii_case_insensitive(input: &[u8], start: usize, expected: &[u8]) -> bool {
    input
        .get(start..start.saturating_add(expected.len()))
        .is_some_and(|actual| eq_ascii_case_insensitive(actual, expected))
}

fn eq_ascii_case_insensitive(left: &[u8], right: &[u8]) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn trim_ascii(mut input: &[u8]) -> &[u8] {
    while input.first().is_some_and(|byte| is_ascii_space(*byte)) {
        input = &input[1..];
    }
    while input.last().is_some_and(|byte| is_ascii_space(*byte)) {
        input = &input[..input.len() - 1];
    }
    input
}

fn skip_ascii_space_and_slashes(input: &[u8], cursor: &mut usize) {
    while input
        .get(*cursor)
        .is_some_and(|byte| is_ascii_space(*byte) || *byte == b'/')
    {
        *cursor += 1;
    }
}

fn is_ascii_space(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0c | b'\r' | b' ')
}

fn is_ascii_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_takes_precedence_over_conflicting_meta() {
        let source = b"\xEF\xBB\xBF<meta charset=windows-1252><p>\xC3\xA9</p>";
        assert_eq!(
            decode_html_bytes(source),
            "<meta charset=windows-1252><p>é</p>"
        );
    }

    #[test]
    fn detects_meta_attributes_case_insensitively_and_in_any_order() {
        let source = b"<META content='text/html; CHARSET = windows-1252' HTTP-EQUIV='Content-Type'><p>\x93ok\x94</p>";
        assert!(decode_html_bytes(source).contains("“ok”"));
    }

    #[test]
    fn ignores_meta_inside_comments_and_after_the_prescan_window() {
        let commented = b"<!-- <meta charset=windows-1252> --><p>\xC3\xA9</p>";
        assert!(decode_html_bytes(commented).contains('é'));

        let mut late = vec![b' '; META_SCAN_LIMIT + 1];
        late.extend_from_slice(b"<meta charset=gbk><p>\xC4\xE3</p>");
        assert!(decode_html_bytes(&late).contains("Äã"));
    }

    #[test]
    fn decodes_utf16_boms() {
        let mut little_endian = b"\xFF\xFE".to_vec();
        little_endian.extend("<p>你好</p>".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_html_bytes(&little_endian), "<p>你好</p>");

        let mut big_endian = b"\xFE\xFF".to_vec();
        big_endian.extend("<p>你好</p>".encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(decode_html_bytes(&big_endian), "<p>你好</p>");
    }

    #[test]
    fn parses_unquoted_http_equiv_content_values() {
        let source =
            b"<meta http-equiv=content-type content=text/html;charset=windows-1252><p>\x80</p>";
        assert!(decode_html_bytes(source).contains('€'));
    }
}
