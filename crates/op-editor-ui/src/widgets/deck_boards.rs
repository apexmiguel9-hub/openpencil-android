//! What a deck's slides ARE, independent of any navigator that lists
//! them: the boards of the active page, their order, which one the camera
//! is on, and the one command that changes that order.
//!
//! This used to live inside the canvas filmstrip, back when the strip was
//! the only navigator. The strip is gone — the left rail's slides tab
//! replaced it — but none of the answers below were ever the strip's:
//! they are the deck's, and the rail asks them now. Keeping them in their
//! own module is what stops a second navigator from ever answering "which
//! slide is this" differently.

use op_editor_core::preview_slideshow::active_page_boards;
use op_editor_core::EditorState;

use crate::layout_scene::LayoutScene;
use crate::{Point2D, Rect};

/// One slide, as a navigator lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardChip {
    /// Node id of the board, used to frame it on click.
    pub id: String,
    /// The board's authored name. Empty is fine — the page number alone
    /// still identifies the slide.
    pub name: String,
}

/// The active page's top-level boards, in page order.
pub fn board_chips(state: &EditorState) -> Vec<BoardChip> {
    let children = state.active_children();
    active_page_boards(state)
        .into_iter()
        .map(|id| {
            let name = children
                .iter()
                .find(|node| op_editor_core::PenNodeExt::id_str(*node) == id)
                .and_then(|node| op_editor_core::PenNodeExt::base(node).name.clone())
                .unwrap_or_default();
            BoardChip { id, name }
        })
        .collect()
}

/// The slide the camera is looking at: the board whose centre is nearest
/// the centre of the canvas region.
///
/// Nearest-centre rather than "the one under the cursor" or "the
/// selected node" because a navigator has to answer for a camera that is
/// merely parked somewhere, which is most of the time. `None` when no
/// board resolves in the scene yet — the list then simply highlights
/// nothing rather than guessing at slide 1.
pub fn active_chip_index(
    chips: &[BoardChip],
    scene: &LayoutScene,
    state: &EditorState,
    canvas: Rect,
) -> Option<usize> {
    let viewport = &state.viewport;
    if viewport.zoom <= 0.0 {
        return None;
    }
    // Screen centre of the canvas region, back through the camera.
    let centre = Point2D::new(
        (canvas.size.x / 2.0 - viewport.pan_x) / viewport.zoom,
        (canvas.size.y / 2.0 - viewport.pan_y) / viewport.zoom,
    );
    let page = scene.active_page()?;
    chips
        .iter()
        .enumerate()
        .filter_map(|(index, chip)| {
            let bounds = page.find(&chip.id)?.aggregate_bounds();
            let dx = bounds.origin.x + bounds.size.x / 2.0 - centre.x;
            let dy = bounds.origin.y + bounds.size.y / 2.0 - centre.y;
            Some((index, dx * dx + dy * dy))
        })
        .min_by(|(_, a): &(usize, f32), (_, b)| a.total_cmp(b))
        .map(|(index, _)| index)
}

/// Where a row dragged from `from` and dropped before `slot` ends up in
/// the child array, or `None` when the drop changes nothing.
///
/// `slot` counts gaps in the CURRENT order, so dropping either side of
/// the dragged row is a no-op; every other slot converts to a
/// post-removal index, which is what `EditorCommand::MoveNode` takes.
pub fn reorder_target_index(from: usize, slot: usize) -> Option<usize> {
    if slot == from || slot == from + 1 {
        return None;
    }
    Some(if slot > from { slot - 1 } else { slot })
}

/// Move a board to child index `to`, undoably. Returns whether the
/// document changed.
///
/// A plain reparent of the board to the page root at a new index: it
/// rewrites the child ORDER and nothing else. No geometry is touched, so
/// the boards stay exactly where they are on the canvas and only the page
/// sequence — which is what presenting and exporting read — changes.
///
/// The snapshot is taken first and dropped again if the move is refused,
/// so a rejected reorder leaves no empty entry on the undo stack for the
/// user to step through.
pub fn apply_reorder(state: &mut EditorState, board_id: &str, to: usize) -> bool {
    state.commit_history();
    if state.apply(reorder_command(board_id, to)) {
        return true;
    }
    state.history.past.pop_back();
    false
}

/// The `EditorCommand` a deck reorder issues. Public so the reorder can be
/// asserted on directly.
pub fn reorder_command(board_id: &str, to: usize) -> op_editor_core::EditorCommand {
    op_editor_core::EditorCommand::MoveNode {
        node_id: op_editor_core::NodeId::new(board_id.to_string()),
        target_parent: op_editor_core::NodeId::NONE,
        page_id: None,
        index: Some(to),
    }
}

#[cfg(test)]
#[path = "deck_boards_tests.rs"]
mod deck_boards_tests;
