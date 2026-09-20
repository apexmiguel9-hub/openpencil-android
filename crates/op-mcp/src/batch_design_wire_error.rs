//! Typed failures for the hand-rolled `nodes_json` wire parser
//! (`batch_design_wire.rs`).
//!
//! Style follows `ProgramError`: a plain enum plus a hand-written `Display`,
//! no `thiserror` and no new dependency. Each variant's `Display` reproduces
//! the exact sentence the stringly-typed parser produced, because those
//! sentences ship verbatim to the model as the `batch_design`
//! `InvalidArgument` payload.
//!
//! What the enum buys over `String` is the CLASSIFICATION: grammar faults
//! (`Expected*` / `Unterminated*`), descriptor-level faults (`MissingField`
//! / `UnknownKey` / `UnsupportedKind`), value faults (`InvalidFillHex` /
//! `NotAnI32` / `NegativeSize`), and encoding faults (`InvalidUtf8`) are now
//! separable without matching prose.

use std::fmt;

use super::write_tools::ALLOWED_KINDS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WireParseError {
    /// The payload does not open with `[`.
    ArrayMustStartWithBracket,
    /// Input ran out while an array was still open.
    UnterminatedArray,
    /// Non-whitespace bytes follow the closing `]`.
    TrailingGarbage,
    /// A descriptor is followed by something other than `,` or `]`.
    ExpectedCommaOrBracket(char),
    /// A descriptor does not open with `{`.
    ExpectedDescriptorBrace,
    /// Input ran out while a descriptor was still open.
    UnterminatedDescriptor,
    /// A descriptor key is not followed by `:`.
    ExpectedColonAfterKey(String),
    /// A descriptor carries a key the flat grammar has no slot for.
    UnknownKey(String),
    /// A required descriptor field is absent. The payload is the wire key.
    MissingField(&'static str),
    /// `kind` is not one of `ALLOWED_KINDS`.
    UnsupportedKind(String),
    /// `width` / `height` parsed but are negative.
    NegativeSize,
    /// `fill_hex` is not a supported hex spelling.
    InvalidFillHex(String),
    /// A `fill` array slice did not deserialize into a `PenFill` stack.
    InvalidFillArray(String),
    /// A `fill` object slice did not deserialize into a `PenFill`.
    InvalidFillObject(String),
    /// A raw-JSON value was expected but the input ended.
    ExpectedJsonValue,
    /// A raw-JSON value never closed its brackets.
    UnterminatedJsonValue,
    /// A byte slice was not valid UTF-8. The payload names the region, as
    /// it appears in the message (`JSON value` / `nodes_json` / `string` /
    /// `integer`).
    InvalidUtf8(&'static str),
    /// A quoted string was expected.
    ExpectedString,
    /// Input ran out immediately after a `\` escape marker.
    UnterminatedEscape,
    /// An escape sequence outside the supported `"` `\` `n` `t` `r` `/` set.
    UnsupportedEscape(char),
    /// A quoted string never closed.
    UnterminatedString,
    /// An integer was expected but no digits were present.
    ExpectedInteger,
    /// Digits parsed but do not fit an `i32`.
    NotAnI32(String),
}

impl fmt::Display for WireParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireParseError::ArrayMustStartWithBracket => {
                f.write_str("nodes_json must start with `[`")
            }
            WireParseError::UnterminatedArray => f.write_str("unterminated array"),
            WireParseError::TrailingGarbage => f.write_str("trailing garbage after array"),
            WireParseError::ExpectedCommaOrBracket(got) => {
                write!(f, "expected `,` or `]` after item, got {got:?}")
            }
            WireParseError::ExpectedDescriptorBrace => {
                f.write_str("expected `{` to start a descriptor")
            }
            WireParseError::UnterminatedDescriptor => f.write_str("unterminated descriptor"),
            WireParseError::ExpectedColonAfterKey(key) => {
                write!(f, "expected `:` after key {key:?}")
            }
            WireParseError::UnknownKey(key) => write!(f, "unknown key {key:?} in descriptor"),
            WireParseError::MissingField(field) => write!(f, "descriptor missing `{field}`"),
            WireParseError::UnsupportedKind(kind) => write!(
                f,
                "kind {kind:?} not supported; allowed: {}",
                ALLOWED_KINDS.join(", ")
            ),
            WireParseError::NegativeSize => f.write_str("width / height must be non-negative"),
            WireParseError::InvalidFillHex(hex) => {
                write!(f, "fill_hex must be #rgb/#rrggbb/#rrggbbaa, got {hex:?}")
            }
            WireParseError::InvalidFillArray(detail) => {
                write!(f, "invalid `fill` array: {detail}")
            }
            WireParseError::InvalidFillObject(detail) => {
                write!(f, "invalid `fill` object: {detail}")
            }
            WireParseError::ExpectedJsonValue => f.write_str("expected a JSON value"),
            WireParseError::UnterminatedJsonValue => f.write_str("unterminated JSON value"),
            WireParseError::InvalidUtf8(region) => write!(f, "invalid UTF-8 in {region}"),
            WireParseError::ExpectedString => f.write_str("expected string"),
            WireParseError::UnterminatedEscape => f.write_str("unterminated escape"),
            WireParseError::UnsupportedEscape(escape) => {
                write!(f, "unsupported escape \\{escape}")
            }
            WireParseError::UnterminatedString => f.write_str("unterminated string"),
            WireParseError::ExpectedInteger => f.write_str("expected integer"),
            WireParseError::NotAnI32(raw) => write!(f, "expected i32, got {raw:?}"),
        }
    }
}

impl std::error::Error for WireParseError {}
