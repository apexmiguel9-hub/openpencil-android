//! JavaScript-compatible text primitives.
//!
//! The logic in this crate replaces JS that ran against JS semantics, and the
//! extension's observable behaviour must not shift. Two places where Rust's
//! defaults differ from JavaScript's therefore get explicit helpers:
//!
//! * **Whitespace.** JS `\s` (and `String.prototype.trim`) is
//!   `WhiteSpace | LineTerminator`, which includes `U+FEFF` — a character
//!   Unicode does *not* give the `White_Space` property, so
//!   [`char::is_whitespace`] returns `false` for it. Conversely
//!   `char::is_whitespace` accepts `U+0085` (NEL), which JS `\s` rejects.
//!   [`is_js_space`] corrects both directions, so it and [`js_trim`] are
//!   exactly their JS counterparts rather than approximately.
//! * **String length.** JS strings are counted in UTF-16 code units, so
//!   `slice(0, n)` and `.length` are UTF-16 measurements. Rust's `len()` is
//!   UTF-8 bytes and `chars()` counts scalar values; neither matches.

/// True for exactly the characters JS `\s` matches.
pub fn is_js_space(c: char) -> bool {
    // `char::is_whitespace` covers `White_Space`, which is JS `\s` minus
    // `U+FEFF` plus `U+0085`. Both corrections are applied here rather than
    // left to the callers: filename sanitisation happens to strip `Cc` (and
    // so `U+0085`) before this runs, but endpoint parsing and `js_number` do
    // not, and for them a NEL that JS keeps must stay a non-space — JS
    // answers `Number("\u{85}12")` with `NaN`, not `12`.
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// Trim JS-`\s` from both ends, matching `String.prototype.trim`.
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// Truncate `s` to at most `limit` UTF-16 code units.
///
/// JS `slice(0, limit)` cuts at a code-unit boundary, which can leave a lone
/// high surrogate when the limit lands in the middle of an astral character.
/// A lone surrogate is not representable in a Rust `str`, so the whole
/// character is dropped instead. The difference is observable only for a
/// string whose `limit`-th code unit splits a surrogate pair, and only ever
/// removes a half-character that JS would have emitted as an unpaired
/// surrogate — never valid text.
pub fn truncate_utf16(s: &str, limit: usize) -> &str {
    let mut units = 0usize;
    for (offset, c) in s.char_indices() {
        let next = units + c.len_utf16();
        if next > limit {
            return &s[..offset];
        }
        units = next;
    }
    s
}
