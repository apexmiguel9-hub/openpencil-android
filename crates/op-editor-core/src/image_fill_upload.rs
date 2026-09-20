//! Host-facing image-fill upload mutation.

use crate::fills::node_fills_mut;
use crate::walkers::find_node_mut;
use crate::{EditorState, ImageFillMode, NodeId};
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{ImageFillBody, ImageOriginalSize, PenFill};

impl EditorState {
    /// Replace the selected node's primary fill with an image rooted at `src`.
    ///
    /// Callers that know the encoded bitmap dimensions should use
    /// [`Self::set_selected_fill_image_url_with_original_size`] so a later
    /// switch to Crop can initialize an editable cover transform.
    pub fn set_selected_fill_image_url(&mut self, src: &str) -> bool {
        self.set_selected_fill_image_url_with_original_size(src, None)
    }

    /// Sized variant of [`Self::set_selected_fill_image_url`].
    ///
    /// Replacing the image resets its prior mode/transform and exits any crop
    /// edit session for the selected node, preventing a stale "editing" UI
    /// state from surviving after the crop payload has gone away.
    pub fn set_selected_fill_image_url_with_original_size(
        &mut self,
        src: &str,
        original_size: Option<[f32; 2]>,
    ) -> bool {
        let sel = self.selection.anchor.clone();
        self.set_node_fill_image_url(&sel, src, original_size, None)
    }

    /// Write `src` into `id`'s primary fill as an image.
    ///
    /// `mode` is written verbatim when given; `None` leaves the field absent,
    /// which every renderer resolves to `Fill` (cover) anyway. An `Image` node
    /// takes the url on its `src` instead — it has no fill list.
    ///
    /// Does NOT snapshot history: the two call sites differ (the fill picker
    /// bumps the revision, the drag-and-drop path wants one undo step around
    /// its whole gesture), so the caller owns that decision.
    pub fn set_node_fill_image_url(
        &mut self,
        id: &NodeId,
        src: &str,
        original_size: Option<[f32; 2]>,
        mode: Option<ImageFillMode>,
    ) -> bool {
        let sel = id.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return false;
        }
        let original_size = original_size.and_then(|[width, height]| {
            (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
                .then_some(ImageOriginalSize { width, height })
        });
        let updated = {
            let Some(node) = find_node_mut(self.active_children_mut(), &sel) else {
                return false;
            };
            if let PenNode::Image(image) = node {
                image.src = src.into();
                true
            } else {
                let Some(fills) = node_fills_mut(node) else {
                    return false;
                };
                let body = PenFill::Image(ImageFillBody {
                    url: src.into(),
                    mode: mode.map(ImageFillMode::to_schema),
                    original_size,
                    transform: None,
                    tile_scale: None,
                    explain: None,
                    opacity: None,
                    blend_mode: None,
                    exposure: None,
                    contrast: None,
                    saturation: None,
                    temperature: None,
                    tint: None,
                    highlights: None,
                    shadows: None,
                });
                if fills.is_empty() {
                    fills.push(body);
                } else {
                    fills[0] = body;
                }
                true
            }
        };
        if updated && self.editor_ui.image_crop_editing.as_ref() == Some(&sel) {
            self.editor_ui.image_crop_editing = None;
        }
        updated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sized_fill_upload_persists_dimensions_and_exits_stale_crop_edit() {
        let mut state = EditorState::sample();
        let selected = state.selection.anchor.clone();
        assert!(state.set_selected_fill_image_url_with_original_size(
            "data:image/png;base64,old",
            Some([400.0, 200.0]),
        ));
        assert!(state.set_selected_image_fill_mode(ImageFillMode::Crop));
        state.editor_ui.image_crop_editing = Some(selected.clone());

        assert!(state.set_selected_fill_image_url_with_original_size(
            "data:image/png;base64,new",
            Some([640.0, 480.0]),
        ));

        let summary = crate::fills::first_image_fill_summary(
            state.selected_node().expect("selected fill node"),
        )
        .expect("image fill summary");
        assert_eq!(
            summary.image_url.as_deref(),
            Some("data:image/png;base64,new")
        );
        assert_eq!(summary.original_size, Some([640.0, 480.0]));
        assert_eq!(summary.mode, ImageFillMode::Fill);
        assert_eq!(summary.transform, None);
        assert_eq!(state.editor_ui.image_crop_editing, None);
        assert_eq!(state.selection.anchor, selected);
    }

    #[test]
    fn rejected_fill_upload_keeps_unrelated_crop_edit_state() {
        let mut state = EditorState::sample();
        state.editor_ui.image_crop_editing = Some(NodeId::new("photo"));
        state.clear_selection();
        state.editor_ui.image_crop_editing = Some(NodeId::new("photo"));

        assert!(!state.set_selected_fill_image_url_with_original_size(
            "data:image/png;base64,new",
            Some([640.0, 480.0]),
        ));
        assert_eq!(
            state.editor_ui.image_crop_editing,
            Some(NodeId::new("photo"))
        );
    }
}
