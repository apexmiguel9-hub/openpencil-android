//! Encoding sniffing for external CSS byte streams.

use std::borrow::Cow;

use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8, WINDOWS_1252};

/// Decodes a CSS byte stream using its BOM or an initial `@charset` rule.
///
/// Undeclared CSS is UTF-8 when valid. Invalid UTF-8 falls back explicitly to
/// Windows-1252 so legacy stylesheets behave like they do in browsers instead
/// of producing UTF-8 replacement characters.
pub fn decode_css_bytes(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(payload) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return decode_with(UTF_8, payload);
    }
    if let Some(payload) = bytes.strip_prefix(b"\xFF\xFE") {
        return decode_with(UTF_16LE, payload);
    }
    if let Some(payload) = bytes.strip_prefix(b"\xFE\xFF") {
        return decode_with(UTF_16BE, payload);
    }

    if let Some(encoding) = sniff_charset_encoding(bytes) {
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

fn sniff_charset_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    const KEYWORD: &[u8] = b"@charset";
    if !bytes
        .get(..KEYWORD.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(KEYWORD))
    {
        return None;
    }

    let mut cursor = KEYWORD.len();
    if !bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        return None;
    }
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }

    let quote = bytes
        .get(cursor)
        .copied()
        .filter(|byte| matches!(*byte, b'\'' | b'"'))?;
    cursor += 1;
    let label_start = cursor;
    while let Some(byte) = bytes.get(cursor) {
        if *byte == quote {
            break;
        }
        if !byte.is_ascii() || matches!(*byte, b'\\' | b'\r' | b'\n' | b'\x0c') {
            return None;
        }
        cursor += 1;
    }
    let label = bytes.get(label_start..cursor)?;
    cursor += 1;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b';') {
        return None;
    }

    let encoding = Encoding::for_label(label)?;
    // An ASCII `@charset` prefix cannot itself be UTF-16. CSS Syntax specifies
    // that these labels are interpreted as UTF-8 unless a UTF-16 BOM won above.
    if encoding == UTF_16LE || encoding == UTF_16BE {
        Some(UTF_8)
    } else {
        Some(encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_wins_and_decodes_utf8_and_utf16() {
        let utf8 = b"\xEF\xBB\xBF@charset \"gbk\";.hot{content:\"\xE4\xBD\xA0\xE5\xA5\xBD\"}";
        assert!(decode_css_bytes(utf8).contains("你好"));

        let mut little_endian = b"\xFF\xFE".to_vec();
        little_endian.extend(".热{color:red}".encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(decode_css_bytes(&little_endian), ".热{color:red}");

        let mut big_endian = b"\xFE\xFF".to_vec();
        big_endian.extend(".热{color:red}".encode_utf16().flat_map(u16::to_be_bytes));
        assert_eq!(decode_css_bytes(&big_endian), ".热{color:red}");
    }

    #[test]
    fn initial_charset_is_case_insensitive_and_decodes_gbk() {
        let (encoded, _, had_errors) = encoding_rs::GBK.encode("@ChArSeT 'gbk';.热{color:red}");
        assert!(!had_errors);
        assert_eq!(decode_css_bytes(&encoded), "@ChArSeT 'gbk';.热{color:red}");
    }

    #[test]
    fn ignores_invalid_or_non_initial_charset_declarations() {
        assert_eq!(
            decode_css_bytes(b"@charset \"not-an-encoding\";.a{}"),
            "@charset \"not-an-encoding\";.a{}"
        );

        let bytes = b" /* prefix */ @charset \"gbk\";.caf\xE9{color:red}";
        assert!(decode_css_bytes(bytes).contains(".café{"));
    }

    #[test]
    fn invalid_undeclared_utf8_uses_windows_1252_fallback() {
        let bytes = b".note{content:\x93legacy\x94;color:\x80}";
        assert_eq!(
            decode_css_bytes(bytes),
            ".note{content:\u{201c}legacy\u{201d};color:\u{20ac}}"
        );
    }
}
