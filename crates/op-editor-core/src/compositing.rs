//! Node- and fill-level compositing properties exposed by the inspector.
//!
//! The canonical schema keeps node blend/mask state on `PenNodeBase` and
//! per-fill blend state on each `PenFill` body. These helpers provide one
//! normalized edit surface while retaining compatibility with the legacy
//! `Path.mask` boolean.

use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::walkers::find_node_mut;
use jian_ops_schema::node::{MaskType, PenNode};
use jian_ops_schema::style::{BlendMode, PenFill};

fn normalized_blend_mode(mode: Option<BlendMode>) -> Option<BlendMode> {
    mode.filter(|mode| !matches!(mode, BlendMode::Normal))
}

/// Effective node blend mode. `None` and an explicitly-authored `normal`
/// both mean source-over and are therefore reported as `None`.
pub fn node_blend_mode(node: &PenNode) -> Option<BlendMode> {
    normalized_blend_mode(node.base().blend_mode.clone())
}

/// Effective mask type, including old path documents that only authored
/// `mask: true`. A canonical `maskType` always wins over the legacy marker.
pub fn node_mask_type(node: &PenNode) -> Option<MaskType> {
    node.base().mask_type.or_else(|| match node {
        PenNode::Path(path) if path.mask == Some(true) => Some(MaskType::Alpha),
        _ => None,
    })
}

/// Effective blend mode of one authored fill. Missing and explicit `normal`
/// values both return `None`.
pub fn fill_blend_mode_at(node: &PenNode, index: usize) -> Option<BlendMode> {
    let fill = crate::fills::node_fills(node)?.get(index)?;
    normalized_blend_mode(fill_blend_mode(fill).cloned())
}

fn fill_blend_mode(fill: &PenFill) -> Option<&BlendMode> {
    match fill {
        PenFill::Solid(body) => body.blend_mode.as_ref(),
        PenFill::LinearGradient(body) => body.blend_mode.as_ref(),
        PenFill::RadialGradient(body) => body.blend_mode.as_ref(),
        PenFill::MeshGradient(body) => body.blend_mode.as_ref(),
        PenFill::Shader(body) => body.blend_mode.as_ref(),
        PenFill::Image(body) => body.blend_mode.as_ref(),
    }
}

fn fill_blend_mode_mut(fill: &mut PenFill) -> &mut Option<BlendMode> {
    match fill {
        PenFill::Solid(body) => &mut body.blend_mode,
        PenFill::LinearGradient(body) => &mut body.blend_mode,
        PenFill::RadialGradient(body) => &mut body.blend_mode,
        PenFill::MeshGradient(body) => &mut body.blend_mode,
        PenFill::Shader(body) => &mut body.blend_mode,
        PenFill::Image(body) => &mut body.blend_mode,
    }
}

fn set_node_blend_mode(node: &mut PenNode, mode: Option<BlendMode>) -> bool {
    let mode = normalized_blend_mode(mode);
    if normalized_blend_mode(node.base().blend_mode.clone()) == mode {
        return false;
    }
    node.base_mut().blend_mode = mode;
    true
}

fn set_node_mask_type(node: &mut PenNode, mask_type: Option<MaskType>) -> bool {
    let legacy_mask_authored = matches!(node, PenNode::Path(path) if path.mask.is_some());
    if node.base().mask_type == mask_type && !legacy_mask_authored {
        return false;
    }
    node.base_mut().mask_type = mask_type;
    if let PenNode::Path(path) = node {
        // Once the inspector writes this property, keep one canonical source
        // of truth. This also lets choosing "None" disable an old mask:true.
        path.mask = None;
    }
    true
}

fn set_fill_blend_mode_at(node: &mut PenNode, index: usize, mode: Option<BlendMode>) -> bool {
    let Some(fill) = crate::fills::node_fills_opt_mut(node).and_then(|fills| fills.get_mut(index))
    else {
        return false;
    };
    let mode = normalized_blend_mode(mode);
    let slot = fill_blend_mode_mut(fill);
    if normalized_blend_mode(slot.clone()) == mode {
        return false;
    }
    *slot = mode;
    true
}

impl EditorState {
    /// Effective compositing mode of the anchor-selected node.
    pub fn selected_node_blend_mode(&self) -> Option<BlendMode> {
        if let Some(display) = crate::instance_override::resolve_instance_display_node_for_anchor(
            &self.doc,
            &self.selection.anchor,
        ) {
            return node_blend_mode(&display);
        }
        node_blend_mode(self.selected_node()?)
    }

    /// Effective mask mode of the anchor-selected node.
    pub fn selected_node_mask_type(&self) -> Option<MaskType> {
        if let Some(display) = crate::instance_override::resolve_instance_display_node_for_anchor(
            &self.doc,
            &self.selection.anchor,
        ) {
            return node_mask_type(&display);
        }
        node_mask_type(self.selected_node()?)
    }

    /// Set node compositing without materializing the source-over default.
    pub fn set_selected_node_blend_mode(&mut self, mode: Option<BlendMode>) -> bool {
        let selected = self.selection.anchor.clone();
        if !selected.is_real() || !self.is_editable(&selected) {
            return false;
        }
        find_node_mut(self.active_children_mut(), &selected)
            .is_some_and(|node| set_node_blend_mode(node, mode))
    }

    /// Write canonical mask semantics and remove any legacy `Path.mask`
    /// marker so future reads cannot observe conflicting fields.
    pub fn set_selected_node_mask_type(&mut self, mask_type: Option<MaskType>) -> bool {
        let selected = self.selection.anchor.clone();
        if !selected.is_real() || !self.is_editable(&selected) {
            return false;
        }
        find_node_mut(self.active_children_mut(), &selected)
            .is_some_and(|node| set_node_mask_type(node, mask_type))
    }

    /// Set one fill's blend mode. `Normal` is stored as absence for old-wire
    /// compatibility; all other fields and fills remain untouched.
    pub fn set_selected_fill_blend_mode(&mut self, index: usize, mode: Option<BlendMode>) -> bool {
        let selected = self.selection.anchor.clone();
        if !selected.is_real() || !self.is_editable(&selected) {
            return false;
        }
        find_node_mut(self.active_children_mut(), &selected)
            .is_some_and(|node| set_fill_blend_mode_at(node, index, mode))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_id::NodeId;
    use crate::test_support::{rect, state_with};

    #[test]
    fn legacy_path_mask_reads_as_alpha_then_canonicalizes() {
        let path: PenNode =
            serde_json::from_str(r#"{"type":"path","id":"p","mask":true}"#).expect("legacy path");
        let mut state = state_with(vec![path]);
        state.set_single_selection(NodeId::new("p"));

        assert_eq!(state.selected_node_mask_type(), Some(MaskType::Alpha));
        assert!(state.set_selected_node_mask_type(Some(MaskType::Vector)));
        let PenNode::Path(path) = state.selected_node().unwrap() else {
            panic!("expected path")
        };
        assert_eq!(path.base.mask_type, Some(MaskType::Vector));
        assert_eq!(path.mask, None);
    }

    #[test]
    fn clearing_legacy_path_mask_is_a_real_write() {
        let path: PenNode =
            serde_json::from_str(r#"{"type":"path","id":"p","mask":true}"#).expect("legacy path");
        let mut state = state_with(vec![path]);
        state.set_single_selection(NodeId::new("p"));

        assert!(state.set_selected_node_mask_type(None));
        assert_eq!(state.selected_node_mask_type(), None);
        assert!(!state.set_selected_node_mask_type(None));
    }

    #[test]
    fn normal_blend_is_normalized_to_absence_and_noops_when_absent() {
        let mut node = rect("r", "Rect", 0.0, 0.0, 10.0, 10.0);
        node.base_mut().blend_mode = Some(BlendMode::Multiply);
        let mut state = state_with(vec![node]);
        state.set_single_selection(NodeId::new("r"));

        assert_eq!(state.selected_node_blend_mode(), Some(BlendMode::Multiply));
        assert!(state.set_selected_node_blend_mode(Some(BlendMode::Normal)));
        assert_eq!(state.selected_node().unwrap().base().blend_mode, None);
        assert!(!state.set_selected_node_blend_mode(None));
    }

    #[test]
    fn explicit_legacy_normal_node_blend_is_a_byte_shape_noop() {
        let mut node = rect("r", "Rect", 0.0, 0.0, 10.0, 10.0);
        node.base_mut().blend_mode = Some(BlendMode::Normal);
        let mut state = state_with(vec![node]);
        state.set_single_selection(NodeId::new("r"));
        let before = serde_json::to_string(&state.doc).unwrap();

        assert_eq!(state.selected_node_blend_mode(), None);
        assert!(!state.set_selected_node_blend_mode(Some(BlendMode::Normal)));
        assert_eq!(serde_json::to_string(&state.doc).unwrap(), before);
        assert_eq!(
            state.selected_node().unwrap().base().blend_mode,
            Some(BlendMode::Normal)
        );

        assert!(state.set_selected_node_blend_mode(Some(BlendMode::Screen)));
        assert_eq!(
            state.selected_node().unwrap().base().blend_mode,
            Some(BlendMode::Screen)
        );
        assert!(state.set_selected_node_blend_mode(Some(BlendMode::Normal)));
        assert_eq!(state.selected_node().unwrap().base().blend_mode, None);
    }

    #[test]
    fn instance_reads_effective_master_blend_and_routes_direct_writes() {
        let document: jian_ops_schema::PenDocument = serde_json::from_str(
            r#"{"version":"1","children":[
                {"type":"frame","id":"master","reusable":true,"blendMode":"multiply"},
                {"type":"ref","id":"instance","ref":"master"}
            ]}"#,
        )
        .expect("instance document");
        let mut state = EditorState::from_document(document);
        let instance_id = NodeId::new("instance");
        state.set_single_selection(instance_id.clone());

        assert_eq!(state.selected_node_blend_mode(), Some(BlendMode::Multiply));
        assert_eq!(
            crate::apply_instance_override(&mut state, &instance_id, |state| {
                state.set_selected_node_blend_mode(Some(BlendMode::Screen))
            }),
            Some(true)
        );
        assert_eq!(
            crate::apply_instance_override(&mut state, &instance_id, |state| {
                state.set_selected_node_mask_type(Some(MaskType::Luminance))
            }),
            Some(true)
        );
        let PenNode::Ref(reference) = state.selected_node().unwrap() else {
            panic!("expected ref")
        };
        assert_eq!(reference.base.blend_mode, Some(BlendMode::Screen));
        assert_eq!(reference.base.mask_type, Some(MaskType::Luminance));
        assert!(reference.descendants.is_none());
    }

    #[test]
    fn indexed_fill_blend_preserves_the_fill_and_normalizes_normal() {
        let mut node = rect("r", "Rect", 0.0, 0.0, 10.0, 10.0);
        assert!(crate::fills::set_primary_fill_type(
            &mut node,
            crate::FillType::Image
        ));
        let mut state = state_with(vec![node]);
        state.set_single_selection(NodeId::new("r"));

        assert!(state.set_selected_fill_blend_mode(0, Some(BlendMode::Screen)));
        assert_eq!(
            fill_blend_mode_at(state.selected_node().unwrap(), 0),
            Some(BlendMode::Screen)
        );
        assert!(matches!(
            crate::fills::node_fills(state.selected_node().unwrap()).unwrap()[0],
            PenFill::Image(_)
        ));
        assert!(state.set_selected_fill_blend_mode(0, Some(BlendMode::Normal)));
        assert_eq!(fill_blend_mode_at(state.selected_node().unwrap(), 0), None);
        assert!(!state.set_selected_fill_blend_mode(99, Some(BlendMode::Multiply)));
    }

    #[test]
    fn explicit_legacy_normal_fill_blend_is_a_byte_shape_noop() {
        let mut node = rect("r", "Rect", 0.0, 0.0, 10.0, 10.0);
        assert!(crate::fills::set_primary_fill_type(
            &mut node,
            crate::FillType::Image
        ));
        let PenFill::Image(body) = &mut crate::fills::node_fills_mut(&mut node).expect("fills")[0]
        else {
            panic!("expected image fill")
        };
        body.blend_mode = Some(BlendMode::Normal);

        let mut state = state_with(vec![node]);
        state.set_single_selection(NodeId::new("r"));
        let before = serde_json::to_string(&state.doc).unwrap();

        assert_eq!(fill_blend_mode_at(state.selected_node().unwrap(), 0), None);
        assert!(!state.set_selected_fill_blend_mode(0, Some(BlendMode::Normal)));
        assert_eq!(serde_json::to_string(&state.doc).unwrap(), before);
        let PenFill::Image(body) =
            &crate::fills::node_fills(state.selected_node().unwrap()).expect("fills")[0]
        else {
            panic!("expected image fill")
        };
        assert_eq!(body.blend_mode, Some(BlendMode::Normal));

        assert!(state.set_selected_fill_blend_mode(0, Some(BlendMode::Screen)));
        assert!(state.set_selected_fill_blend_mode(0, Some(BlendMode::Normal)));
        let PenFill::Image(body) =
            &crate::fills::node_fills(state.selected_node().unwrap()).expect("fills")[0]
        else {
            panic!("expected image fill")
        };
        assert_eq!(body.blend_mode, None);
    }

    #[test]
    fn missing_fill_blend_target_is_a_byte_shape_noop() {
        let node = rect("r", "Rect", 0.0, 0.0, 10.0, 10.0);
        let mut state = state_with(vec![node]);
        state.set_single_selection(NodeId::new("r"));
        let before = serde_json::to_string(&state.doc).unwrap();

        assert!(!state.set_selected_fill_blend_mode(0, Some(BlendMode::Multiply)));
        assert_eq!(serde_json::to_string(&state.doc).unwrap(), before);
    }

    #[test]
    fn image_fill_tile_scale_defaults_clamps_and_rejects_image_nodes() {
        let mut fill_node = rect("fill", "Fill", 0.0, 0.0, 10.0, 10.0);
        assert!(crate::fills::set_primary_fill_type(
            &mut fill_node,
            crate::FillType::Image
        ));
        let mut state = state_with(vec![fill_node]);
        state.set_single_selection(NodeId::new("fill"));

        let summary = crate::first_image_fill_summary(state.selected_node().unwrap()).unwrap();
        assert_eq!(summary.tile_scale, Some(1.0));
        assert!(!state.set_selected_image_tile_scale(1.0));
        assert!(state.set_selected_image_tile_scale(0.38618907));
        assert_eq!(
            crate::first_image_fill_summary(state.selected_node().unwrap())
                .unwrap()
                .tile_scale,
            Some(0.38618907)
        );
        assert!(state.set_selected_image_tile_scale(500.0));
        assert_eq!(
            crate::first_image_fill_summary(state.selected_node().unwrap())
                .unwrap()
                .tile_scale,
            Some(crate::fills::MAX_IMAGE_TILE_SCALE)
        );
        assert!(state.set_selected_image_tile_scale(0.001));
        assert_eq!(
            crate::first_image_fill_summary(state.selected_node().unwrap())
                .unwrap()
                .tile_scale,
            Some(crate::fills::MIN_IMAGE_TILE_SCALE)
        );
        assert!(!state.set_selected_image_tile_scale(0.0));
        assert!(!state.set_selected_image_tile_scale(f32::NAN));
        assert_eq!(
            crate::first_image_fill_summary(state.selected_node().unwrap())
                .unwrap()
                .tile_scale,
            Some(crate::fills::MIN_IMAGE_TILE_SCALE)
        );

        let PenFill::Image(body) = &mut crate::fills::node_fills_mut(
            state
                .active_children_mut()
                .first_mut()
                .expect("selected fill node"),
        )
        .unwrap()[0] else {
            panic!("expected image fill")
        };
        body.tile_scale = Some(1.0);
        assert!(!state.set_selected_image_tile_scale(1.0));
        let PenFill::Image(body) =
            &crate::fills::node_fills(state.selected_node().unwrap()).unwrap()[0]
        else {
            panic!("expected image fill")
        };
        assert_eq!(body.tile_scale, Some(1.0));

        let image: PenNode =
            serde_json::from_str(r#"{"type":"image","id":"image","src":"asset.png"}"#)
                .expect("image node");
        state.doc.children = vec![image];
        state.set_single_selection(NodeId::new("image"));
        assert_eq!(
            crate::image_node_summary(state.selected_node().unwrap())
                .unwrap()
                .tile_scale,
            None
        );
        assert!(!state.set_selected_image_tile_scale(2.0));
    }
}
