//! Deck slideshow state for Preview (Play) mode.
//!
//! A document tagged [`TemplateScene::Slides`] is a deck, and previewing a
//! deck means presenting it: one board at a time, filling the viewport,
//! advanced with the arrow keys — not the interactive app preview an app
//! design gets. This module owns the whole decision and the whole
//! transition set, so the hosts only paint and forward keys.
//!
//! Everything here is derived from FACTS: the document's own scenario tag
//! (recorded when it was opened from a template or generated as a deck) and
//! the authored child order of the page. Nothing is inferred from artboard
//! size — a 1920x1080 document is not automatically a deck.

use crate::scene_template_catalog::TemplateScene;
use crate::{EditorState, PenNodeExt};

/// A control on the presenting toolbar.
///
/// Lives here rather than in the widget layer for the same reason
/// `PreviewDeviceKind` does: the host records which control is pressed /
/// hovered in editor state, and the widget only reads it back to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlideshowToolbarButton {
    Previous,
    Next,
    Exit,
}

/// Which board of a deck the slideshow is currently presenting.
///
/// The board list is captured when the slideshow starts rather than
/// re-derived per frame: the document cannot change during a preview (the
/// runtime never writes back), and a fixed list means the index can never
/// point at a board that has moved out from under it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlideshowState {
    boards: Vec<String>,
    index: usize,
}

impl SlideshowState {
    /// A slideshow over `boards`, starting at the first one. `None` for an
    /// empty deck — there is nothing to present, and every method below
    /// would have to invent an answer.
    pub fn new(boards: Vec<String>) -> Option<Self> {
        (!boards.is_empty()).then_some(Self { boards, index: 0 })
    }

    /// Node ids of the boards, in the order they are presented.
    pub fn boards(&self) -> &[String] {
        &self.boards
    }

    /// Zero-based index of the board on screen.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Number of boards in the deck. Never zero for a live slideshow.
    pub fn len(&self) -> usize {
        self.boards.len()
    }

    /// Always false for a slideshow built through [`Self::new`]; present so
    /// `len` does not stand alone.
    pub fn is_empty(&self) -> bool {
        self.boards.is_empty()
    }

    /// Node id of the board on screen.
    pub fn current_board(&self) -> Option<&str> {
        self.boards.get(self.index).map(String::as_str)
    }

    /// Advance one board. Clamps at the last one — a presentation that
    /// wrapped around to the title slide on the final click would read as
    /// having lost the presenter's place. Returns whether the board changed.
    pub fn next_board(&mut self) -> bool {
        self.step(1)
    }

    /// Go back one board, clamping at the first. Returns whether the board
    /// changed.
    pub fn previous_board(&mut self) -> bool {
        self.step(-1)
    }

    /// Shared clamped move, so forward and back can never drift apart.
    pub fn step(&mut self, delta: i32) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let last = self.boards.len() - 1;
        let target = (self.index as i64 + delta as i64).clamp(0, last as i64) as usize;
        let moved = target != self.index;
        self.index = target;
        moved
    }

    /// Jump straight to a board, clamped into the deck. Returns whether the
    /// board on screen changed. Backs Home / End, which a presenter uses to
    /// get to the title or the closing slide without walking the deck.
    pub fn go_to(&mut self, index: usize) -> bool {
        if self.boards.is_empty() {
            return false;
        }
        let target = index.min(self.boards.len() - 1);
        let moved = target != self.index;
        self.index = target;
        moved
    }

    /// Whether a board exists before / after the one on screen. Drives the
    /// toolbar's disabled ends, so what the buttons look like and what they
    /// do come from one answer.
    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.boards.len()
    }

    /// The presenter's position, e.g. `"3 / 6"`.
    ///
    /// Bare numerals with a slash on purpose: it reads the same in every
    /// language, so the overlay needs no translation and cannot show an
    /// untranslated key.
    pub fn counter_label(&self) -> String {
        format!("{} / {}", self.index + 1, self.boards.len())
    }
}

/// The boards of the active page: its top-level frames, in document child
/// order.
///
/// That order IS the authored order — the scaffold inserts boards in order
/// and the deck generator builds them in order — so it is taken as given
/// rather than re-derived from geometry. Sorting by position would reorder a
/// deck whose author moved a slide on the canvas without meaning to renumber
/// it, and would have no answer at all for boards stacked at the same point.
///
/// Only frames count. A loose rectangle or a stray text node beside the deck
/// is annotation, not a slide, and presenting it would be presenting
/// something the author never authored as a board.
pub fn active_page_boards(state: &EditorState) -> Vec<String> {
    state
        .active_children()
        .iter()
        .filter(|node| matches!(node, jian_ops_schema::node::PenNode::Frame(_)))
        .map(|node| node.id_str().to_string())
        .collect()
}

/// The boards of the active page that the current selection covers, in the
/// same document child order [`active_page_boards`] returns.
///
/// **The single rule for "which slides are selected."** The slides rail
/// counts these to label its `Export selected slides (N)` row, and the deck
/// PDF exporter takes exactly these when that row is picked, so the number
/// the user was shown and the pages that come out of the file are answers to
/// one question rather than two.
///
/// Derived from [`active_page_boards`] rather than from the selection set
/// directly: a selection may hold a text node inside a slide, or a node on
/// another page, and neither of those is a slide. Filtering the board list
/// means anything that is not a board simply does not appear, with no second
/// definition of what a board is.
pub fn selected_page_boards(state: &EditorState) -> Vec<String> {
    active_page_boards(state)
        .into_iter()
        .filter(|id| {
            state
                .selection
                .contains(&crate::node_id::NodeId::new(id.clone()))
        })
        .collect()
}

/// The slideshow this document's preview should run, or `None` for the
/// ordinary interactive preview.
///
/// Gated on the recorded scenario, never on what the content looks like: the
/// tag is the only thing that actually knows the document is a deck.
pub fn slideshow_for_document(state: &EditorState) -> Option<SlideshowState> {
    if state.editor_ui.scenario != Some(TemplateScene::Slides) {
        return None;
    }
    SlideshowState::new(active_page_boards(state))
}

impl EditorState {
    /// Start the deck slideshow for this document, if it is a deck with
    /// boards. Returns whether a slideshow is now running.
    ///
    /// Called by the host right after the preview session is built. A
    /// document that is not a deck — or a deck with nothing on the page —
    /// leaves preview exactly as it was.
    pub fn begin_preview_slideshow(&mut self) -> bool {
        let slideshow = slideshow_for_document(self);
        let started = slideshow.is_some();
        self.editor_ui.preview.slideshow = slideshow;
        started
    }

    /// The running slideshow, if preview is presenting a deck.
    pub fn preview_slideshow(&self) -> Option<&SlideshowState> {
        self.editor_ui.preview.slideshow.as_ref()
    }

    /// Move the running slideshow one board forward or back. Returns whether
    /// the board on screen changed; `false` outside a slideshow and at the
    /// clamped ends.
    pub fn step_preview_slideshow(&mut self, delta: i32) -> bool {
        self.editor_ui
            .preview
            .slideshow
            .as_mut()
            .is_some_and(|slideshow| slideshow.step(delta))
    }

    /// Jump the running slideshow to the first (`false`) or last (`true`)
    /// board — Home / End. Returns whether the board on screen changed.
    pub fn preview_slideshow_to_end(&mut self, last: bool) -> bool {
        self.editor_ui
            .preview
            .slideshow
            .as_mut()
            .is_some_and(|slideshow| {
                let target = if last {
                    slideshow.len().saturating_sub(1)
                } else {
                    0
                };
                slideshow.go_to(target)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck_state(source: &str) -> EditorState {
        let document = jian_ops_schema::load_str(source)
            .expect("fixture parses")
            .value;
        let mut state = EditorState::from_document(document);
        state.editor_ui.scenario = Some(TemplateScene::Slides);
        state
    }

    const THREE_BOARDS: &str = r#"{"version":"1.0.0","children":[
        {"type":"frame","id":"slide-1","x":0,"y":0,"width":1920,"height":1080},
        {"type":"frame","id":"slide-2","x":2100,"y":0,"width":1920,"height":1080},
        {"type":"frame","id":"slide-3","x":4200,"y":0,"width":1920,"height":1080}
    ]}"#;

    #[test]
    fn stepping_clamps_at_both_ends_instead_of_wrapping() {
        let mut slideshow = SlideshowState::new(vec!["a".into(), "b".into(), "c".into()])
            .expect("three boards is a slideshow");
        assert_eq!(slideshow.current_board(), Some("a"));
        assert!(!slideshow.previous_board(), "already at the first board");
        assert_eq!(slideshow.index(), 0);

        assert!(slideshow.next_board());
        assert!(slideshow.next_board());
        assert_eq!(slideshow.current_board(), Some("c"));
        assert!(
            !slideshow.next_board(),
            "the last board is the end of the deck"
        );
        assert_eq!(slideshow.index(), 2);

        assert!(slideshow.previous_board());
        assert_eq!(slideshow.current_board(), Some("b"));
    }

    #[test]
    fn the_counter_reads_one_based_out_of_the_deck_length() {
        let mut slideshow =
            SlideshowState::new(vec!["a".into(), "b".into(), "c".into()]).expect("boards");
        assert_eq!(slideshow.counter_label(), "1 / 3");
        slideshow.next_board();
        assert_eq!(slideshow.counter_label(), "2 / 3");
    }

    #[test]
    fn an_empty_deck_has_no_slideshow() {
        assert_eq!(SlideshowState::new(Vec::new()), None);

        let mut state = deck_state(r#"{"version":"1.0.0","children":[]}"#);
        assert!(!state.begin_preview_slideshow());
        assert!(state.preview_slideshow().is_none());
        // Stepping an absent slideshow must be a no-op, not a panic.
        assert!(!state.step_preview_slideshow(1));
    }

    #[test]
    fn boards_follow_document_child_order_not_canvas_position() {
        // Board 2 sits to the LEFT of board 1 on the canvas; presentation
        // order still follows the authored child order.
        let state = deck_state(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"first","x":9000,"y":0,"width":1920,"height":1080},
                {"type":"frame","id":"second","x":0,"y":0,"width":1920,"height":1080}
            ]}"#,
        );

        assert_eq!(active_page_boards(&state), vec!["first", "second"]);
    }

    #[test]
    fn only_frames_are_boards() {
        let state = deck_state(
            r#"{"version":"1.0.0","children":[
                {"type":"frame","id":"slide-1","x":0,"y":0,"width":1920,"height":1080},
                {"type":"text","id":"a-note","x":0,"y":1200,"content":"todo: rewrite"},
                {"type":"rectangle","id":"a-swatch","x":400,"y":1200,"width":40,"height":40}
            ]}"#,
        );

        assert_eq!(active_page_boards(&state), vec!["slide-1"]);
    }

    #[test]
    fn only_a_deck_document_presents() {
        let mut ordinary = deck_state(THREE_BOARDS);
        ordinary.editor_ui.scenario = None;
        assert!(!ordinary.begin_preview_slideshow());

        let mut carousel = deck_state(THREE_BOARDS);
        carousel.editor_ui.scenario = Some(TemplateScene::Carousel);
        assert!(!carousel.begin_preview_slideshow());

        let mut deck = deck_state(THREE_BOARDS);
        assert!(deck.begin_preview_slideshow());
        assert_eq!(
            deck.preview_slideshow().map(SlideshowState::current_board),
            Some(Some("slide-1"))
        );
    }

    #[test]
    fn leaving_preview_ends_the_slideshow() {
        let mut deck = deck_state(THREE_BOARDS);
        deck.editor_ui.enter_preview();
        assert!(deck.begin_preview_slideshow());

        deck.editor_ui.exit_preview();

        assert!(deck.preview_slideshow().is_none());
    }
}
