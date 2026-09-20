//! The Asset Center's Styles tab: style-guide catalogue → card view models.
//!
//! The registry stores whole markdown documents; a card needs a name, a few
//! colours, and one line of prose. Deriving that here — once, from the
//! canonical `op_ai_skills` registries rather than from a hand-copied table —
//! is what keeps the tab honest: a guide added to the corpus shows up in the
//! panel without anyone remembering to list it, and a guide whose palette
//! section changes shows the new swatches.
//!
//! Two sources feed one list. The shipped corpus is baked into the binary; the
//! user's imported `DESIGN.md` files arrive at runtime. They are merged here,
//! imports first, because a list where your own material sits below fifty
//! shipped entries is a list you scroll past your own work in. Ids keep them
//! apart — a corpus guide is pinned by its bare name, an import by its
//! `user:` id — so a file that names itself after a shipped guide cannot
//! quietly take its place.

use op_ai_skills::style_guide::{
    extract_style_guide_values, style_guide_registry, user_style_guides, ParsedStyleGuide,
    Platform, UserStyleGuide,
};
use op_util::hex_color;

use crate::Color;

/// How many swatches a card's colour band paints. The registry's palettes
/// carry more entries than this, but a band is a glance, not a legend.
pub const STYLE_SWATCH_COUNT: usize = 5;

/// One style-guide card.
pub struct StyleGuideCard {
    /// What pinning writes to the document: the guide's `name` for a corpus
    /// entry, its `user:<slug>` id for an import.
    pub id: String,
    /// The name shown on the card.
    pub name: String,
    /// Which platform the guide was written for.
    pub platform: Platform,
    /// The guide's declared tags — the search vocabulary. Kept whole even
    /// though [`Self::summary`] shows only the first few, so a guide is still
    /// findable by a tag that did not fit on the card.
    pub tags: Vec<String>,
    /// Whether this came from a `DESIGN.md` the user imported — the one
    /// difference the tab acts on: imports get a section of their own and a
    /// delete button, because they are the only ones the user can remove.
    pub is_user: bool,
    /// Palette band, left to right. Entries the guide does not declare are
    /// dropped rather than filled with a placeholder: an invented swatch
    /// would misrepresent the guide the user is about to commit to.
    pub swatches: Vec<Color>,
    /// One line under the name — the guide's own tags, in corpus order.
    pub summary: String,
}

impl StyleGuideCard {
    fn from_guide(guide: &'static ParsedStyleGuide) -> Self {
        let values = extract_style_guide_values(&guide.content);
        let colors = &values.colors;
        let swatches = [
            colors.background.as_deref(),
            colors.surface.as_deref(),
            colors.accent.as_deref(),
            colors.text_primary.as_deref(),
            colors.border.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(parse_swatch)
        .take(STYLE_SWATCH_COUNT)
        .collect();

        Self {
            id: guide.name.clone(),
            name: guide.name.clone(),
            platform: guide.platform,
            tags: guide.tags.clone(),
            is_user: false,
            swatches,
            summary: summary_for(&guide.tags, &values.typography.display_font),
        }
    }

    /// A card for an imported guide.
    ///
    /// The swatches come from the import's own hex scan rather than from
    /// [`extract_style_guide_values`]: that extractor reads the corpus's
    /// section grammar, and a community `DESIGN.md` writes its palette
    /// however it likes, so running it here would leave most imports with an
    /// empty band.
    fn from_user_guide(user: &UserStyleGuide) -> Self {
        Self {
            id: user.id.clone(),
            name: user.guide.name.clone(),
            platform: user.guide.platform,
            tags: user.guide.tags.clone(),
            is_user: true,
            swatches: user
                .swatches
                .iter()
                .filter_map(|hex| parse_swatch(hex))
                .take(STYLE_SWATCH_COUNT)
                .collect(),
            summary: user_summary(user),
        }
    }

    /// Whether this card is the pinned one.
    pub fn is_pinned(&self, pinned: Option<&str>) -> bool {
        pinned == Some(self.id.as_str())
    }

    /// Whether the search query matches this card.
    fn matches(&self, query: &str) -> bool {
        query.is_empty()
            || self.name.to_lowercase().contains(query)
            || self
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(query))
    }
}

/// The full catalogue as cards — imports first, then the corpus in registry
/// order.
pub fn style_guide_cards() -> Vec<StyleGuideCard> {
    filtered_style_guide_cards("")
}

/// Cards surviving a search query, matched against name and tags.
pub fn filtered_style_guide_cards(query: &str) -> Vec<StyleGuideCard> {
    let query = query.trim().to_lowercase();
    let mut cards: Vec<StyleGuideCard> = user_style_guides()
        .iter()
        .map(|user| StyleGuideCard::from_user_guide(user))
        .collect();
    cards.extend(
        style_guide_registry()
            .iter()
            .map(StyleGuideCard::from_guide),
    );
    cards.retain(|card| card.matches(&query));
    cards
}

/// How many of `cards` are imports.
///
/// The list is ordered imports-first, so this is also the index the corpus
/// section starts at — which is what the grid needs to know where to put its
/// section headings.
pub fn user_card_count(cards: &[StyleGuideCard]) -> usize {
    cards.iter().take_while(|card| card.is_user).count()
}

fn parse_swatch(raw: &str) -> Option<Color> {
    let [r, g, b, a] = hex_color::parse_hex_rgba8(raw, hex_color::HexOptions::LENIENT)?;
    Some(Color::rgba_u8(r, g, b, a as f32 / 255.0))
}

/// "swiss · minimal · light-mode — Inter" — the tags the guide declares,
/// with its display font when it names one. Tags are the vocabulary the
/// prompt ranker already scores against, so a user picking by tag is
/// picking the same way the automatic path does.
fn summary_for(tags: &[String], display_font: &Option<String>) -> String {
    let mut summary = tags
        .iter()
        .take(4)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" · ");
    if let Some(font) = display_font {
        if !summary.is_empty() {
            summary.push_str(" — ");
        }
        summary.push_str(font);
    }
    summary
}

/// An import's summary: its own tags when it declared any, else the first
/// line of prose it wrote.
///
/// The fallback matters more than it looks. Most community `DESIGN.md` files
/// carry no tags at all, and a card showing only a name is indistinguishable
/// from the next one — the opening line is what the author chose to say first
/// about the aesthetic, so it is the best one-line description available
/// without inventing one.
fn user_summary(user: &UserStyleGuide) -> String {
    let tagged = summary_for(&user.guide.tags, &None);
    if !tagged.is_empty() {
        return tagged;
    }
    user.guide
        .content
        .lines()
        .map(str::trim)
        .find(|line| {
            !line.is_empty()
                && !line.starts_with('#')
                && !line.starts_with("---")
                && !line.starts_with("```")
                && !line.contains(':')
        })
        .map(|line| line.chars().take(120).collect())
        .unwrap_or_else(|| user.guide.platform.as_str().to_string())
}

/// Serialized access to the process-global imported-style registry.
///
/// Every Asset Center test reads that registry, and a few write it. Cargo runs
/// them on parallel threads of one process, so without a lock a test that
/// imports two guides changes what an unrelated test's grid contains. Any test
/// touching the Styles tab takes this — including the read-only ones, which is
/// the point: they are the ones that would otherwise fail mysteriously.
#[cfg(test)]
pub(crate) mod style_test_support {
    use std::sync::{Mutex, MutexGuard};

    pub(crate) fn exclusive_user_styles() -> MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        op_ai_skills::style_guide::set_user_style_guides(Vec::new());
        guard
    }

    /// Import a minimal `DESIGN.md` named `name`, returning its id.
    pub(crate) fn import_style(name: &str) -> String {
        let raw = format!("---\nname: {name}\n---\n\n# {name}\n\nAccent #2563EB everywhere.\n");
        op_ai_skills::style_guide::import_design_md(&raw, name)
            .expect("the fixture is a valid DESIGN.md")
            .id
            .clone()
    }
}

#[cfg(test)]
#[path = "asset_center_style_cards_tests.rs"]
mod asset_center_style_cards_tests;
