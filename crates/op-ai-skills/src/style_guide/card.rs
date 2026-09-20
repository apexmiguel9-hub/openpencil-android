//! The pinned guide, expanded: what a person needs to decide whether the
//! style in force is the one they meant.
//!
//! [`super::summary`] answers "is a style pinned, and did its values parse" in
//! a chip. That is the right amount for a row above the input, and too little
//! for the question a user actually has when they stop and look at it: *which*
//! of my styles is this, where did it come from, and what does it do. A name
//! plus four unlabelled swatches cannot tell an imported `DESIGN.md` from a
//! shipped corpus guide of a similar palette — and mistaking one for the other
//! is precisely how a generation comes back in a style the user thought they
//! had replaced.
//!
//! So this is the summary's larger sibling: same lookup, same memoization
//! discipline, more of the file read out. It is built on a hover, not on every
//! frame, but it is still memoized — a hover holds still for whole seconds, and
//! re-running the value extractor over tens of kilobytes of markdown sixty
//! times a second for a card that has not changed is the same mistake the
//! summary exists to avoid.
//!
//! Its rule matches the rest of the style-guide stack: state what the file
//! states, and where the file says nothing, carry `None` rather than a guess.
//! An absent field is rendered as an absent row, never as a plausible default.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use super::parser::{extract_style_guide_values, StyleGuideValues};
use super::token_table::sample_palette;
use super::user_registry::find_style_guide;

/// Most swatches a card carries.
///
/// Eight is what the seven named colour roles plus one sampled extra come to,
/// and about as many as read as a palette rather than a gradient strip.
pub const STYLE_CARD_SWATCH_CAP: usize = 8;

/// Longest description kept, in characters. Past this the card would be
/// reading the guide aloud rather than introducing it.
const DESCRIPTION_CAP: usize = 180;

/// A pinned guide expanded for the hover card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleGuideCard {
    /// The id it resolved from — `user:<slug>` or a corpus name.
    pub id: String,
    /// The name a person reads.
    pub name: String,
    /// Whether this came from an import rather than the shipped corpus.
    ///
    /// The one fact the chip cannot carry and the user most needs: a pin is
    /// only trustworthy if you know which catalogue it points into.
    pub is_user: bool,
    /// Up to [`STYLE_CARD_SWATCH_CAP`] `#RRGGBB` values — the named roles the
    /// guide states, in role order, then whatever else it mentions.
    ///
    /// Empty carries the same meaning it does on the chip: this file stated no
    /// colours anything could read.
    pub swatches: Vec<String>,
    /// Heading / display family, when the guide names one.
    pub display_font: Option<String>,
    /// Body family, when the guide names one.
    pub body_font: Option<String>,
    /// The guide's own one-sentence self-description, verbatim.
    pub description: Option<String>,
}

type Memo = HashMap<String, Option<Arc<StyleGuideCard>>>;

fn memo() -> &'static RwLock<Memo> {
    static MEMO: LazyLock<RwLock<Memo>> = LazyLock::new(|| RwLock::new(HashMap::new()));
    &MEMO
}

/// Drop every memoized card. Called by the imported-guide mutators, in step
/// with [`super::summary::invalidate_summaries`] — the two memos are keyed off
/// the same catalogue and must never disagree about what is in it.
pub(super) fn invalidate_cards() {
    if let Ok(mut memo) = memo().write() {
        memo.clear();
    }
}

/// Expand the guide `id` names, or `None` when it names none.
///
/// `None` is the answer for a stale pin, exactly as it is for a summary: by
/// then generation has already fallen back to choosing its own style, and a
/// card describing the dead guide would be a confident lie.
pub fn style_guide_card(id: &str) -> Option<Arc<StyleGuideCard>> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    if let Ok(memo) = memo().read() {
        if let Some(hit) = memo.get(id) {
            return hit.clone();
        }
    }
    let card = find_style_guide(id).map(|guide| {
        let values = extract_style_guide_values(&guide.content);
        Arc::new(StyleGuideCard {
            id: guide.id().to_string(),
            is_user: guide.is_user(),
            name: guide.name.clone(),
            swatches: swatches(&values, &guide.content),
            display_font: values.typography.display_font.clone(),
            body_font: values.typography.body_font.clone(),
            description: description(&guide.content),
        })
    });
    if let Ok(mut memo) = memo().write() {
        memo.insert(id.to_string(), card.clone());
    }
    card
}

/// The card's colour band: every named role the guide states, in the order a
/// palette is read (ground, then surface, then the accents and text), topped up
/// from the document's own mentions when the named roles came up short.
///
/// The top-up is deliberately unclassified — `sample_palette` passes roles
/// through without inferring them, and that is the right behaviour here too:
/// reaching this branch means the structured pass already failed, and guessing
/// which of the remaining colours is "the background" is how a card ends up
/// showing an accent as the page ground.
fn swatches(values: &StyleGuideValues, content: &str) -> Vec<String> {
    let colors = &values.colors;
    let mut out: Vec<String> = Vec::new();
    for named in [
        colors.background.as_deref(),
        colors.surface.as_deref(),
        colors.accent.as_deref(),
        colors.text_primary.as_deref(),
        colors.text_secondary.as_deref(),
        colors.text_muted.as_deref(),
        colors.border.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        push_unique(&mut out, named);
    }
    if out.len() < STYLE_CARD_SWATCH_CAP {
        for sample in sample_palette(content, STYLE_CARD_SWATCH_CAP) {
            push_unique(&mut out, &sample.color);
            if out.len() >= STYLE_CARD_SWATCH_CAP {
                break;
            }
        }
    }
    out.truncate(STYLE_CARD_SWATCH_CAP);
    out
}

/// Append `hex` unless the band already holds that colour. Case-insensitive:
/// the corpus writes `#FAFAFA` and the token-table dialect writes `#fafafa`,
/// and the same colour twice reads as a two-colour palette.
fn push_unique(out: &mut Vec<String>, hex: &str) {
    let hex = hex.trim();
    if hex.is_empty() || out.len() >= STYLE_CARD_SWATCH_CAP {
        return;
    }
    if out.iter().any(|held| held.eq_ignore_ascii_case(hex)) {
        return;
    }
    out.push(hex.to_string());
}

/// The guide's own opening sentence.
///
/// Preference matters more than it looks: every corpus guide opens with an
/// identical "Style Scope" boilerplate paragraph, so taking the first prose
/// paragraph would show the same forty words for all of them. A section titled
/// *Summary* / *Overview* is where a guide says what it is, so that is what is
/// looked for first; the plain first paragraph is only the fallback for files
/// that carry no such section.
fn description(content: &str) -> Option<String> {
    let body = strip_frontmatter(content);
    first_sentence(&summary_section(body).or_else(|| first_paragraph(body))?)
}

/// Everything after a leading `---` frontmatter block, or the whole input
/// when there is none.
fn strip_frontmatter(content: &str) -> &str {
    let Some(rest) = content.strip_prefix("---") else {
        return content;
    };
    let rest = rest.trim_start_matches('\n');
    match rest.find("\n---") {
        Some(end) => rest[end + 4..].trim_start_matches(['\n', '\r']),
        None => content,
    }
}

/// The first paragraph under a `##`-level heading that names itself a summary.
fn summary_section(body: &str) -> Option<String> {
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        let Some(title) = line.trim_start().strip_prefix("##") else {
            continue;
        };
        let title = title.trim_start_matches('#').trim().to_ascii_lowercase();
        if !(title.contains("summary") || title.contains("overview")) {
            continue;
        }
        if let Some(paragraph) = paragraph_from(&mut lines) {
            return Some(paragraph);
        }
    }
    None
}

/// The first prose paragraph anywhere in `body`.
fn first_paragraph(body: &str) -> Option<String> {
    paragraph_from(&mut body.lines())
}

/// Consume `lines` up to and through the next prose paragraph.
///
/// Headings, tables, lists, and fenced code are skipped rather than read: a
/// table row rendered as a sentence is noise, and a bullet is half a thought.
fn paragraph_from<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Option<String> {
    let mut held = String::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !held.is_empty() {
                break;
            }
            continue;
        }
        if is_structural(trimmed) {
            if !held.is_empty() {
                break;
            }
            continue;
        }
        if !held.is_empty() {
            held.push(' ');
        }
        held.push_str(trimmed);
    }
    (!held.is_empty()).then_some(held)
}

/// Whether the line is markdown structure rather than prose.
fn is_structural(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with('|')
        || line.starts_with("- ")
        || line.starts_with("* ")
        || line.starts_with("```")
        || line.starts_with('>')
}

/// The first sentence of `paragraph`, stripped of markdown decoration and
/// capped at [`DESCRIPTION_CAP`] characters.
fn first_sentence(paragraph: &str) -> Option<String> {
    let plain = paragraph.replace("**", "").replace('`', "");
    let plain = plain.trim();
    let mut end = plain.len();
    let bytes = plain.as_bytes();
    for (index, ch) in plain.char_indices() {
        let terminates = match ch {
            // A full stop only ends a sentence when prose follows it. `0.9`
            // and `v1.2` are not two sentences, and a guide that writes a
            // version number in its opening line is not unusual.
            '.' => {
                !bytes
                    .get(index.wrapping_sub(1))
                    .is_some_and(u8::is_ascii_digit)
                    && plain[index + ch.len_utf8()..]
                        .chars()
                        .next()
                        .is_none_or(char::is_whitespace)
            }
            '。' | '！' | '？' => true,
            _ => false,
        };
        if terminates {
            end = index + ch.len_utf8();
            break;
        }
    }
    let sentence = plain[..end].trim();
    if sentence.is_empty() {
        return None;
    }
    Some(truncate_chars(sentence, DESCRIPTION_CAP))
}

/// `text` cut to at most `cap` characters, with an ellipsis when cut.
fn truncate_chars(text: &str, cap: usize) -> String {
    if text.chars().count() <= cap {
        return text.to_string();
    }
    let mut out: String = text.chars().take(cap).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[path = "card_tests.rs"]
mod card_tests;
