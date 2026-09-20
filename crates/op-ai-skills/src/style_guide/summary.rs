//! What a pinned style guide looks like, cheaply and repeatedly.
//!
//! The editor's chat panel paints a receipt of the pinned guide — its name and
//! a few of its colours — on every frame it is visible. Deriving that from the
//! guide's markdown each time means running the whole value extractor over a
//! document that can be tens of kilobytes, sixty times a second, on the chrome
//! paint path the renderer is already tight on.
//!
//! So it is memoized by id. The two halves of the catalogue make that safe in
//! different ways: corpus guides are baked into the binary and cannot change at
//! all, and imported ones only change through the mutators in
//! [`super::user_registry`], each of which drops the memo. There is no third
//! way for a guide's content to move.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, RwLock};

use super::parser::extract_style_guide_values;
use super::user_registry::find_style_guide;

/// A pinned guide reduced to what a receipt shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleGuideSummary {
    /// The id it resolved from — `user:<slug>` or a corpus name.
    pub id: String,
    /// The name to show a person. An id like `user:dimension-style-reference`
    /// is how the pipeline refers to a guide, not how its author named it.
    pub name: String,
    /// Up to four hex colours — background, surface, accent, primary text —
    /// in that order, skipping the ones the guide does not state.
    ///
    /// Empty is meaningful rather than a failure: it says the file's values
    /// could not be read, which is precisely the condition that used to be
    /// invisible until a whole generation came back in the wrong palette.
    pub swatches: Vec<String>,
}

type Memo = HashMap<String, Option<Arc<StyleGuideSummary>>>;

fn memo() -> &'static RwLock<Memo> {
    static MEMO: LazyLock<RwLock<Memo>> = LazyLock::new(|| RwLock::new(HashMap::new()));
    &MEMO
}

/// Drop every memoized summary. Called by the imported-guide mutators.
///
/// The hover card's memo ([`super::card`]) is keyed off the same catalogue and
/// is dropped in the same breath: the two describe one guide at two levels of
/// detail, and a card surviving the summary that displaced it would show the
/// deleted file's palette under the live file's name.
pub(super) fn invalidate_summaries() {
    if let Ok(mut memo) = memo().write() {
        memo.clear();
    }
    super::card::invalidate_cards();
}

/// Summarize the guide `id` names, or `None` when it names none.
///
/// `None` is the answer for a stale pin — an import deleted since it was
/// pinned — and callers are expected to show nothing rather than a name,
/// because by that point generation has already fallen back to choosing its
/// own style.
pub fn style_guide_summary(id: &str) -> Option<Arc<StyleGuideSummary>> {
    let id = id.trim();
    if id.is_empty() {
        return None;
    }
    if let Ok(memo) = memo().read() {
        if let Some(hit) = memo.get(id) {
            return hit.clone();
        }
    }
    let summary = find_style_guide(id).map(|guide| {
        let values = extract_style_guide_values(&guide.content);
        let colors = &values.colors;
        Arc::new(StyleGuideSummary {
            id: guide.id().to_string(),
            name: guide.name.clone(),
            swatches: [
                colors.background.as_deref(),
                colors.surface.as_deref(),
                colors.accent.as_deref(),
                colors.text_primary.as_deref(),
            ]
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect(),
        })
    });
    if let Ok(mut memo) = memo().write() {
        memo.insert(id.to_string(), summary.clone());
    }
    summary
}

#[cfg(test)]
#[path = "summary_tests.rs"]
mod summary_tests;
