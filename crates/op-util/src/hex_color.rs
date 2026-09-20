//! Canonical hex-color parsing.
//!
//! One ASCII-safe, case-insensitive parser replaces the nine divergent
//! copies that used to live in op-pen-loader, op-editor-core, op-editor-ui,
//! op-design-lint, op-orchestrator, and op-ai-skills. The full union of
//! formats is `#RGB`, `#RGBA`, `#RRGGBB`, `#RRGGBBAA` with an optional
//! leading `#`; call sites that intentionally accept a narrower set express
//! that through [`HexOptions`] instead of keeping a private fork.
//!
//! Input is trimmed. Non-ASCII input can never panic here (unlike some of
//! the retired copies, which byte-sliced without a char-boundary guard).

/// Which hex forms a call site accepts. The canonical parser supports the
/// union; sites narrow it to preserve their historical contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexOptions {
    /// Require the leading `#` (reject bare digit strings).
    pub require_hash: bool,
    /// Accept the 3-digit `#RGB` shorthand (each nibble duplicated).
    pub allow_rgb_shorthand: bool,
    /// Accept the 4-digit `#RGBA` shorthand (each nibble duplicated).
    pub allow_rgba_shorthand: bool,
    /// Accept the 8-digit `#RRGGBBAA` form (alpha byte parsed).
    pub allow_alpha: bool,
}

impl HexOptions {
    /// The full union: optional `#`, 3/4/6/8 hex digits.
    pub const LENIENT: Self = Self {
        require_hash: false,
        allow_rgb_shorthand: true,
        allow_rgba_shorthand: true,
        allow_alpha: true,
    };
}

impl Default for HexOptions {
    fn default() -> Self {
        Self::LENIENT
    }
}

/// Parse a hex color into RGBA bytes (`[r, g, b, a]`, alpha 255 when the
/// form carries none). Returns `None` for malformed or non-ASCII input and
/// for forms the given [`HexOptions`] excludes.
pub fn parse_hex_rgba8(input: &str, opts: HexOptions) -> Option<[u8; 4]> {
    let s = input.trim();
    let digits = match s.strip_prefix('#') {
        Some(rest) => rest,
        None if opts.require_hash => return None,
        None => s,
    };
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let b = digits.as_bytes();
    // Safe: every byte was validated as an ASCII hex digit above.
    let nib = |i: usize| (b[i] as char).to_digit(16).unwrap_or(0) as u8;
    let pair = |i: usize| nib(i) * 16 + nib(i + 1);
    let dup = |i: usize| nib(i) * 17; // shorthand nibble duplication: 0xF -> 0xFF
    match digits.len() {
        3 if opts.allow_rgb_shorthand => Some([dup(0), dup(1), dup(2), 255]),
        4 if opts.allow_rgba_shorthand => Some([dup(0), dup(1), dup(2), dup(3)]),
        6 => Some([pair(0), pair(2), pair(4), 255]),
        8 if opts.allow_alpha => Some([pair(0), pair(2), pair(4), pair(6)]),
        _ => None,
    }
}

/// RGBA bytes to normalized `f32` components (0.0–1.0).
pub fn rgba8_to_f32(c: [u8; 4]) -> [f32; 4] {
    c.map(|v| v as f32 / 255.0)
}

/// [`parse_hex_rgba8`] straight to normalized `f32` RGBA.
pub fn parse_hex_rgba_f32(input: &str, opts: HexOptions) -> Option<[f32; 4]> {
    parse_hex_rgba8(input, opts).map(rgba8_to_f32)
}

/// The property-panel "forgiving" mode (ported verbatim from
/// op-editor-ui): 1–8 hex digits parse. The CSS 3-char shorthand expands
/// each nibble (`#F00` → `#FF0000`); lengths 1–2 / 4–5 and 7 are
/// zero-padded into the next supported width (6 or 8) so a mid-edit commit
/// like `#0000` / `#0000000` doesn't visibly reset the colour. Note this
/// deliberately conflicts with [`HexOptions::allow_rgba_shorthand`]: here a
/// 4-digit string is zero-padded RGB, not RGBA.
pub fn parse_hex_rgba8_padded(input: &str) -> Option<[u8; 4]> {
    let digits = input.trim().trim_start_matches('#');
    if digits.is_empty() || digits.len() > 8 {
        return None;
    }
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // Canonicalise to either 6 (`RRGGBB`) or 8 (`RRGGBBAA`) digits.
    let canonical = match digits.len() {
        3 => {
            let mut out = String::with_capacity(6);
            for c in digits.chars() {
                out.push(c);
                out.push(c);
            }
            out
        }
        8 => digits.to_string(),
        7 => format!("{:0>8}", digits),
        _ => format!("{:0>6}", digits),
    };
    parse_hex_rgba8(&canonical, HexOptions::LENIENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STRICT_HASH: HexOptions = HexOptions {
        require_hash: true,
        allow_rgb_shorthand: true,
        allow_rgba_shorthand: false,
        allow_alpha: true,
    };

    #[test]
    fn parses_all_union_forms() {
        assert_eq!(
            parse_hex_rgba8("#F00", HexOptions::LENIENT),
            Some([255, 0, 0, 255])
        );
        assert_eq!(
            parse_hex_rgba8("#F008", HexOptions::LENIENT),
            Some([255, 0, 0, 136])
        );
        assert_eq!(
            parse_hex_rgba8("#8040C0", HexOptions::LENIENT),
            Some([128, 64, 192, 255])
        );
        assert_eq!(
            parse_hex_rgba8("#8040c080", HexOptions::LENIENT),
            Some([128, 64, 192, 128])
        );
        // Bare digits + surrounding whitespace are fine in lenient mode.
        assert_eq!(
            parse_hex_rgba8("  ff8000 ", HexOptions::LENIENT),
            Some([255, 128, 0, 255])
        );
    }

    #[test]
    fn options_narrow_the_accepted_set() {
        assert_eq!(parse_hex_rgba8("f00", STRICT_HASH), None);
        assert_eq!(parse_hex_rgba8("#f008", STRICT_HASH), None);
        assert_eq!(parse_hex_rgba8("#f00", STRICT_HASH), Some([255, 0, 0, 255]));
        let six_only = HexOptions {
            require_hash: false,
            allow_rgb_shorthand: false,
            allow_rgba_shorthand: false,
            allow_alpha: false,
        };
        assert_eq!(parse_hex_rgba8("#fff", six_only), None);
        assert_eq!(parse_hex_rgba8("#ffffff00", six_only), None);
        assert_eq!(
            parse_hex_rgba8("ffffff", six_only),
            Some([255, 255, 255, 255])
        );
    }

    #[test]
    fn non_ascii_and_garbage_return_none_without_panicking() {
        // The retired op-orchestrator copy panicked on this (mid-codepoint
        // byte slice); the canonical parser must return None.
        assert_eq!(parse_hex_rgba8("#é1", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("#颜色颜色颜色", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("#", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("#12345", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("#ffffffzz", HexOptions::LENIENT), None);
        assert_eq!(parse_hex_rgba8("#+f00d0", HexOptions::LENIENT), None);
    }

    #[test]
    fn padded_mode_keeps_property_panel_semantics() {
        assert_eq!(parse_hex_rgba8_padded("#F00"), Some([255, 0, 0, 255]));
        // 4 digits are zero-padded RGB here — NOT RGBA shorthand.
        assert_eq!(parse_hex_rgba8_padded("#abcd"), Some([0, 171, 205, 255]));
        assert_eq!(parse_hex_rgba8_padded("#0000000"), Some([0, 0, 0, 0]));
        assert_eq!(parse_hex_rgba8_padded("#1"), Some([0, 0, 1, 255]));
        assert_eq!(parse_hex_rgba8_padded(""), None);
        assert_eq!(parse_hex_rgba8_padded("#123456789"), None);
        assert_eq!(parse_hex_rgba8_padded("#é1"), None);
    }

    #[test]
    fn f32_conversion() {
        assert_eq!(
            parse_hex_rgba_f32("#000000", HexOptions::LENIENT),
            Some([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(
            parse_hex_rgba_f32("#ffffff", HexOptions::LENIENT),
            Some([1.0, 1.0, 1.0, 1.0])
        );
    }
}
