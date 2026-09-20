//! Aspect-ratio matching for image-backed nodes.
//!
//! The inspector's "match image ratio" action keeps the node's current
//! width and rewrites its height, so a wide screenshot dropped into a
//! tall placeholder stops leaving letterbox bands in Fit mode. The
//! intrinsic source size is resolved by the caller — an image FILL
//! authors `originalSize`, while a standalone `PenNode::Image` has no
//! such schema field and must read it back out of its raster header —
//! which leaves exactly one ratio formula and one write path here.

use crate::state::EditorState;
use crate::ui_draft::PropertyFocus;

/// Smallest dimension treated as real. Mirrors `image_crop`'s guard so
/// a zero / denormal source is rejected identically on both paths.
const SIZE_EPSILON: f32 = 1e-6;

/// Height at which a node of `width` matches `source`'s aspect ratio.
///
/// `None` when any input is non-finite or not positive: a missing or
/// degenerate `originalSize` must leave the node alone rather than
/// collapse it to zero height.
pub fn aspect_matched_height(width: f32, source: [f32; 2]) -> Option<f32> {
    let [source_width, source_height] = source;
    if !width.is_finite() || width <= SIZE_EPSILON {
        return None;
    }
    if !source_width.is_finite() || !source_height.is_finite() {
        return None;
    }
    if source_width <= SIZE_EPSILON || source_height <= SIZE_EPSILON {
        return None;
    }
    let height = width * source_height / source_width;
    (height.is_finite() && height > SIZE_EPSILON).then_some(height)
}

impl EditorState {
    /// Keep the selection's width and write the height that matches
    /// `source`'s aspect ratio, recording one undo step.
    ///
    /// `width` is the layout-resolved canvas width, so a Fill / Hug node
    /// is measured against what the user actually sees. Only the height
    /// is written — an explicit pixel height is exactly what dragging
    /// the bottom handle produces, so a flex parent treats the result
    /// as a manually sized child either way.
    pub fn match_selected_aspect_ratio(&mut self, source: [f32; 2], width: f32) -> bool {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let Some(height) = aspect_matched_height(width, source) else {
            return false;
        };
        let before = self.snapshot_for_history();
        let wrote = self.commit_property_edit(PropertyFocus::SizeH, height);
        if wrote && self.snapshot_for_history() != before {
            self.history_push_past(before);
        }
        wrote
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pen_node_ext::PenNodeExt;
    use crate::NodeId;

    /// A wide screenshot dropped into a tall placeholder — the shape
    /// the action exists for.
    fn wide_image_fill_state() -> EditorState {
        let parsed = jian_ops_schema::load_str(
            r#"{"version":"1.0.0","children":[
                {"type":"rectangle","id":"shot","name":"Screenshot",
                 "x":0,"y":0,"width":320,"height":640,
                 "fill":[{"type":"image","url":"data:image/png;base64,AA==",
                   "mode":"fit","originalSize":{"width":1600,"height":400}}]}
            ]}"#,
        )
        .expect("fixture parses")
        .value;
        let mut state = EditorState::new();
        state.doc.children = parsed.children;
        state.set_single_selection(NodeId::new("shot"));
        state
    }

    fn selected_height(state: &EditorState) -> Option<f64> {
        state.selected_node()?.height_px()
    }

    #[test]
    fn ratio_math_scales_height_from_width() {
        assert_eq!(aspect_matched_height(320.0, [1600.0, 400.0]), Some(80.0));
        assert_eq!(aspect_matched_height(100.0, [200.0, 600.0]), Some(300.0));
    }

    #[test]
    fn ratio_math_rejects_missing_or_zero_dimensions() {
        assert_eq!(aspect_matched_height(320.0, [0.0, 400.0]), None);
        assert_eq!(aspect_matched_height(320.0, [1600.0, 0.0]), None);
        assert_eq!(aspect_matched_height(320.0, [-1600.0, 400.0]), None);
        assert_eq!(aspect_matched_height(320.0, [f32::NAN, 400.0]), None);
        assert_eq!(aspect_matched_height(320.0, [1600.0, f32::INFINITY]), None);
        assert_eq!(aspect_matched_height(0.0, [1600.0, 400.0]), None);
        assert_eq!(aspect_matched_height(f32::NAN, [1600.0, 400.0]), None);
    }

    #[test]
    fn matching_a_wide_image_shrinks_a_tall_frame_to_its_ratio() {
        let mut state = wide_image_fill_state();
        assert_eq!(selected_height(&state), Some(640.0));

        assert!(state.match_selected_aspect_ratio([1600.0, 400.0], 320.0));

        assert_eq!(selected_height(&state), Some(80.0));
        // Width is deliberately untouched — the user is matching the
        // ratio to the box they already sized, not the other way round.
        assert_eq!(
            state.selected_node().and_then(|node| node.width_px()),
            Some(320.0)
        );
    }

    #[test]
    fn matching_records_exactly_one_undo_step_and_keeps_the_selection() {
        let mut state = wide_image_fill_state();
        let selected = state.selection.anchor.clone();
        let depth_before = state.history.past.len();

        assert!(state.match_selected_aspect_ratio([1600.0, 400.0], 320.0));

        assert_eq!(state.history.past.len(), depth_before + 1);
        assert_eq!(state.selection.anchor, selected);
        assert!(state.undo());
        assert_eq!(selected_height(&state), Some(640.0));
    }

    #[test]
    fn matching_advances_the_document_revision() {
        let mut state = wide_image_fill_state();
        let revision_before = state.revision;

        assert!(state.match_selected_aspect_ratio([1600.0, 400.0], 320.0));

        assert_ne!(state.revision, revision_before);
    }

    #[test]
    fn a_degenerate_source_leaves_the_node_and_the_undo_stack_alone() {
        let mut state = wide_image_fill_state();
        let depth_before = state.history.past.len();
        let revision_before = state.revision;

        assert!(!state.match_selected_aspect_ratio([0.0, 400.0], 320.0));

        assert_eq!(selected_height(&state), Some(640.0));
        assert_eq!(state.history.past.len(), depth_before);
        assert_eq!(state.revision, revision_before);
    }
}
