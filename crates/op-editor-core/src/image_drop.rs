//! Dropping an image FILE onto an existing node.
//!
//! The desktop host resolves a drop point to a scene hit path, then asks
//! here which node in that path should receive the bitmap and applies it.
//! Both steps are platform-free so they can be unit-tested without a window.
//!
//! The rule is structural, never name-based: a node accepts an image drop
//! when it paints a filled AREA the bitmap can cover. Text / icons / form
//! widgets carry a `fill` too, but there it colours glyphs or chrome, so a
//! drop over one walks UP to the nearest area ancestor instead — which is
//! what makes "drop onto a placeholder box that contains a hint label and an
//! upload icon" land on the box.

use crate::walkers::find_node;
use crate::{EditorState, ImageFillMode, NodeId};
use jian_ops_schema::node::PenNode;

/// Whether a dropped image can become this node's content.
///
/// `Image` is included because replacing its `src` is the same gesture from
/// the user's side. `Group` is excluded: it is a structural wrapper with no
/// painted body of its own, so filling it would show nothing.
pub fn node_accepts_image_drop(node: &PenNode) -> bool {
    matches!(
        node,
        PenNode::Frame(_)
            | PenNode::Rectangle(_)
            | PenNode::Ellipse(_)
            | PenNode::Polygon(_)
            | PenNode::Path(_)
            | PenNode::Image(_)
    )
}

impl EditorState {
    /// Pick the node an image dropped over `hit_path` should fill.
    ///
    /// `hit_path` is the scene's root-to-deepest hit chain
    /// (`LayoutScene::node_path_at_doc_point`). The search runs deepest-first
    /// so the innermost box under the cursor wins, and skips locked /
    /// unresolvable entries so a drop can still reach an editable ancestor.
    pub fn resolve_image_drop_target(&self, hit_path: &[String]) -> Option<NodeId> {
        for raw in hit_path.iter().rev() {
            let id = NodeId::new(raw.as_str());
            let Some(node) = find_node(self.active_children(), &id) else {
                continue;
            };
            if node_accepts_image_drop(node) && self.is_editable(&id) {
                return Some(id);
            }
        }
        None
    }

    /// Apply a dropped image to `target` as ONE undoable step.
    ///
    /// The fill is written in `Fill` mode — cover: the bitmap scales to the
    /// short side and is centre-cropped to the node's box — which is what a
    /// drop onto a placeholder frame is asking for. Returns `false` (leaving
    /// history untouched) when the target vanished or refuses image fills.
    pub fn apply_image_drop(
        &mut self,
        target: &NodeId,
        src: &str,
        original_size: Option<[f32; 2]>,
    ) -> bool {
        let accepts =
            find_node(self.active_children(), target).is_some_and(node_accepts_image_drop);
        if !accepts || !self.is_editable(target) {
            return false;
        }
        self.commit_history();
        let applied =
            self.set_node_fill_image_url(target, src, original_size, Some(ImageFillMode::Fill));
        debug_assert!(
            applied,
            "a node that accepts image drops must take the fill"
        );
        if applied {
            self.set_single_selection(target.clone());
        }
        applied
    }
}

#[cfg(test)]
#[path = "image_drop_tests.rs"]
mod tests;
