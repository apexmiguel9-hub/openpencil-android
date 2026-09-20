//! Lenient `DESIGN.md` reader for user-supplied style guides.
//!
//! `DESIGN.md` is a community convention rather than a schema: a YAML-ish
//! header (sometimes fenced, sometimes front-matter, sometimes absent), a
//! palette written as a table or a list, and prose. Files in the wild disagree
//! about all of it, so this parser is built to *never reject a file for being
//! shaped differently* — it looks for a name, a handful of colours, and keeps
//! the whole document verbatim for the prompt. Anything it cannot find it
//! simply does without.
//!
//! The three rejections it does make are about the input not being a style
//! guide at all: empty, binary, or too large to inject. Those are reported so
//! the user sees a failure instead of a card with nothing in it — a
//! half-swallowed import is worse than none, because it looks like it worked.
//!
//! The parser is also a trust boundary: the text comes from a file the user
//! picked or pasted, so every routine here walks characters rather than
//! slicing byte ranges, and the fuzz-shaped cases (NUL bytes, one 200k line,
//! an unterminated fence, no ASCII at all) are covered in the tests below.

use crate::frontmatter::{parse_array, split_frontmatter, unquote};
use crate::style_guide::types::Platform;

/// Largest document accepted, in bytes.
///
/// A style guide is injected into a generation prompt in full, and the
/// budget trimmer measures in tokens; half a megabyte is already far past
/// anything that survives trimming, so accepting more would only produce a
/// card whose content silently never reaches the model.
pub const MAX_DESIGN_MD_BYTES: usize = 512 * 1024;

/// Shortest document accepted, in non-whitespace characters.
const MIN_MEANINGFUL_CHARS: usize = 8;

/// How many swatches an imported guide keeps for its card band.
pub const IMPORT_SWATCH_COUNT: usize = 5;

/// Ceiling on tags lifted out of a header, so a file with a hundred of them
/// cannot turn the card summary into a paragraph.
const MAX_TAGS: usize = 8;

/// Why a `DESIGN.md` could not be read as a style guide.
///
/// Deliberately only three: every other shape difference is absorbed rather
/// than reported, because the format has no authority to be non-conformant to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesignMdImportError {
    /// Nothing, whitespace only, or too short to be a guide.
    Empty,
    /// Binary or otherwise not readable as markdown text.
    NotText,
    /// Past [`MAX_DESIGN_MD_BYTES`].
    TooLarge,
}

impl DesignMdImportError {
    /// i18n key for the message shown to the user.
    pub fn message_key(self) -> &'static str {
        match self {
            DesignMdImportError::Empty => "assetCenter.style.importEmpty",
            DesignMdImportError::NotText => "assetCenter.style.importNotText",
            DesignMdImportError::TooLarge => "assetCenter.style.importTooLarge",
        }
    }
}

impl std::fmt::Display for DesignMdImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            DesignMdImportError::Empty => "the file is empty or too short to be a style guide",
            DesignMdImportError::NotText => "the file is not readable as markdown text",
            DesignMdImportError::TooLarge => "the file is larger than 512 KB",
        };
        f.write_str(text)
    }
}

impl std::error::Error for DesignMdImportError {}

/// What a `DESIGN.md` yielded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedDesignMd {
    /// Display name — header `name` / `title`, else the first H1, else the
    /// caller's fallback.
    pub name: String,
    /// Header tags, lowercased. Empty when the file declares none.
    pub tags: Vec<String>,
    /// Header platform; `webapp` when unstated, matching the corpus default.
    pub platform: Platform,
    /// Up to [`IMPORT_SWATCH_COUNT`] hex colours in document order, for the
    /// card's colour band. Empty is legal: a typography-only guide has none.
    pub swatches: Vec<String>,
    /// The document, byte for byte. This is what generation injects, so it is
    /// never normalized, reflowed, or re-escaped here.
    pub content: String,
}

/// Read a `DESIGN.md`.
///
/// `fallback_name` is used only when the document names itself nowhere — the
/// hosts pass the file stem, which is what the user will recognize.
pub fn parse_design_md(
    raw: &str,
    fallback_name: &str,
) -> Result<ImportedDesignMd, DesignMdImportError> {
    if raw.len() > MAX_DESIGN_MD_BYTES {
        return Err(DesignMdImportError::TooLarge);
    }
    if !is_readable_text(raw) {
        return Err(DesignMdImportError::NotText);
    }
    if raw.chars().filter(|c| !c.is_whitespace()).count() < MIN_MEANINGFUL_CHARS {
        return Err(DesignMdImportError::Empty);
    }

    let header = header_block(raw);
    let name = header
        .as_deref()
        .and_then(|block| scalar_field(block, "name"))
        .or_else(|| {
            header
                .as_deref()
                .and_then(|block| scalar_field(block, "title"))
        })
        .or_else(|| first_heading(raw))
        .unwrap_or_else(|| fallback_name.trim().to_string());
    let name = if name.trim().is_empty() {
        "imported style".to_string()
    } else {
        name
    };

    let tags = header.as_deref().map(header_tags).unwrap_or_default();
    let platform = header
        .as_deref()
        .and_then(|block| scalar_field(block, "platform"))
        .map(|value| Platform::from_str(&value))
        .unwrap_or(Platform::Webapp);

    Ok(ImportedDesignMd {
        name: collapse_whitespace(&name),
        tags,
        platform,
        swatches: scan_hex_colors(raw, IMPORT_SWATCH_COUNT),
        content: raw.to_string(),
    })
}

/// A filesystem-safe, id-safe form of `name`.
///
/// Alphanumerics survive in any script — a guide called `温暖厨房` keeps a
/// readable file name rather than becoming `style-3` — and everything else
/// collapses to a single `-`. Never returns an empty string.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in name.chars() {
        if ch.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            for lower in ch.to_lowercase() {
                slug.push(lower);
            }
        } else {
            pending_dash = true;
        }
        // Char count, not byte length: a CJK name would otherwise be cut
        // mid-word by a byte budget that reads as three characters per glyph.
        if slug.chars().count() >= 48 {
            break;
        }
    }
    if slug.is_empty() {
        "style".to_string()
    } else {
        slug
    }
}

/// Whether `raw` reads as text rather than as a binary file that happened to
/// decode.
///
/// NUL is decisive on its own — no markdown file carries one — and a scatter
/// of other control codes is the signature of a decoded binary. Tabs, newlines
/// and carriage returns are text, so they never count against the file.
fn is_readable_text(raw: &str) -> bool {
    let mut total = 0_usize;
    let mut suspicious = 0_usize;
    for ch in raw.chars() {
        total += 1;
        if ch == '\0' {
            return false;
        }
        if ch.is_control() && !matches!(ch, '\n' | '\r' | '\t') {
            suspicious += 1;
        }
    }
    if total == 0 {
        return true;
    }
    suspicious * 100 <= total
}

/// The document's YAML-ish header, if it has one.
///
/// Two shapes are common and both are accepted: real `---` front matter, and
/// a leading ```yaml fence. Anything else returns `None` and the caller falls
/// back to the markdown body.
fn header_block(raw: &str) -> Option<String> {
    if let Some((front_matter, _body)) = split_frontmatter(raw) {
        return Some(front_matter);
    }
    leading_yaml_fence(raw)
}

/// The first fenced block tagged `yaml` / `yml`, when it is the document's
/// opening content (only blank lines may precede it).
///
/// The "opening" restriction matters: a `yaml` example further down a guide
/// is documentation, not the guide's own header, and lifting a name out of it
/// would rename the style after one of its own examples.
fn leading_yaml_fence(raw: &str) -> Option<String> {
    let mut inside = false;
    let mut block = String::new();
    for line in raw.lines().take(200) {
        let trimmed = line.trim();
        if inside {
            if trimmed.starts_with("```") {
                return Some(block);
            }
            block.push_str(line);
            block.push('\n');
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        let tag = trimmed.strip_prefix("```")?.trim().to_ascii_lowercase();
        if tag != "yaml" && tag != "yml" {
            return None;
        }
        inside = true;
    }
    // Unterminated fence: whatever was collected is still the header the
    // author was writing, and refusing it would fail the whole import over a
    // missing three characters.
    if inside && !block.trim().is_empty() {
        Some(block)
    } else {
        None
    }
}

/// A top-level `key: value` scalar from a header block.
fn scalar_field(block: &str, key: &str) -> Option<String> {
    for line in block.lines() {
        // Top level only: an indented `name:` belongs to whatever nested it.
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((found, value)) = line.split_once(':') else {
            continue;
        };
        if !found.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        let value = unquote(value.trim());
        if value.is_empty() {
            continue;
        }
        return Some(value);
    }
    None
}

/// Header tags, from either `tags: [a, b]` or a `tags:` block list.
fn header_tags(block: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut in_list = false;
    for line in block.lines() {
        let trimmed = line.trim();
        if in_list {
            match trimmed.strip_prefix('-') {
                Some(item) => push_tag(&mut tags, item),
                None => in_list = false,
            }
            if in_list {
                continue;
            }
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("tags") {
            continue;
        }
        let value = value.trim();
        if value.starts_with('[') {
            for tag in parse_array(value) {
                push_tag(&mut tags, &tag);
            }
        } else if value.is_empty() {
            in_list = true;
        } else {
            for tag in value.split(',') {
                push_tag(&mut tags, tag);
            }
        }
    }
    tags.truncate(MAX_TAGS);
    tags
}

fn push_tag(tags: &mut Vec<String>, raw: &str) {
    let tag = unquote(raw.trim()).trim().to_lowercase();
    if tag.is_empty() || tags.len() >= MAX_TAGS || tags.contains(&tag) {
        return;
    }
    tags.push(tag);
}

/// The first ATX heading's text, at any level.
fn first_heading(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix('#') else {
            continue;
        };
        let text = rest.trim_start_matches('#').trim();
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }
    None
}

/// Hex colours in document order, deduplicated case-insensitively.
///
/// Deliberately a scan of the whole document rather than of a palette
/// section: every dialect writes the palette somewhere different, and the
/// first few colours a guide mentions are in practice its primary ones.
fn scan_hex_colors(raw: &str, limit: usize) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let chars: Vec<char> = raw.chars().collect();
    let mut index = 0_usize;
    while index < chars.len() {
        if chars[index] != '#' {
            index += 1;
            continue;
        }
        let mut digits = 0_usize;
        while index + 1 + digits < chars.len() && chars[index + 1 + digits].is_ascii_hexdigit() {
            digits += 1;
        }
        // A run longer than 8 is an id or a hash, not a colour; a following
        // alphanumeric means the token continues past the colour it looked
        // like (`#deadbeefcafe`).
        let bounded = !matches!(chars.get(index + 1 + digits), Some(c) if c.is_alphanumeric());
        if bounded && matches!(digits, 3 | 4 | 6 | 8) {
            let hex: String = std::iter::once('#')
                .chain(chars[index + 1..index + 1 + digits].iter().copied())
                .collect();
            if !found.iter().any(|seen| seen.eq_ignore_ascii_case(&hex)) {
                found.push(hex);
                if found.len() >= limit {
                    return found;
                }
            }
        }
        index += 1 + digits.max(1);
    }
    found
}

fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
#[path = "design_md_import_tests.rs"]
mod design_md_import_tests;
