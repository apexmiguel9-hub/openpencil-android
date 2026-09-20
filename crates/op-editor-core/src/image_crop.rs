//! Image-fill crop editing.
//!
//! Figma image transforms map a node-local normalized point `(x, y)` to
//! normalized source-image UV coordinates. Moving the visible bitmap by a
//! local delta therefore subtracts the transform's linear projection of that
//! delta from its translation.

use crate::fills::{node_fills, node_fills_mut};
use crate::walkers::find_node_mut;
use crate::EditorState;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::style::{
    ImageFillBody, ImageFillMode as SchemaImageFillMode, ImageOriginalSize, ImageTransform, PenFill,
};

const SIZE_EPSILON: f32 = 1e-6;

/// Whether a primary image-fill body has crop semantics.
///
/// Older `.op` files may still contain Figma's raw `Stretch + transform`
/// representation. Treat that combination as Crop without changing the wire
/// value so those documents remain editable.
pub fn image_fill_body_is_crop(body: &ImageFillBody) -> bool {
    matches!(body.mode, Some(SchemaImageFillMode::Crop))
        || (matches!(body.mode, Some(SchemaImageFillMode::Stretch))
            && body
                .transform
                .as_ref()
                .is_some_and(|transform| !image_transform_is_identity(transform)))
}

fn image_transform_is_identity(transform: &ImageTransform) -> bool {
    const EPSILON: f32 = 1e-6;
    (transform.m00 - 1.0).abs() <= EPSILON
        && transform.m01.abs() <= EPSILON
        && transform.m02.abs() <= EPSILON
        && transform.m10.abs() <= EPSILON
        && (transform.m11 - 1.0).abs() <= EPSILON
        && transform.m12.abs() <= EPSILON
}

/// Whether `node` exposes an editable primary image-fill crop.
///
/// Standalone `ImageNode`s deliberately return false because their current
/// schema has no affine crop transform field.
pub fn primary_image_fill_is_crop_editable(node: &PenNode) -> bool {
    matches!(
        node_fills(node).and_then(|fills| fills.first()),
        Some(PenFill::Image(body))
            if image_fill_body_is_crop(body)
                && (body.transform.is_some()
                    || body.original_size.as_ref().is_some_and(valid_original_size))
    )
}

fn valid_original_size(size: &ImageOriginalSize) -> bool {
    size.width.is_finite()
        && size.height.is_finite()
        && size.width > SIZE_EPSILON
        && size.height > SIZE_EPSILON
}

/// Current primary image-fill transform in renderer matrix order.
pub fn primary_image_fill_transform(node: &PenNode) -> Option<[f32; 6]> {
    let PenFill::Image(body) = node_fills(node)?.first()? else {
        return None;
    };
    let transform = body.transform.as_ref()?;
    Some(transform_array(transform))
}

/// Build the explicit affine transform equivalent of centered `cover`.
///
/// The transform's unit-square range is the visible source UV window. It is
/// narrower on exactly one axis unless the node and image aspect ratios match.
pub(crate) fn centered_crop_transform(
    node_width: f32,
    node_height: f32,
    original_size: Option<&ImageOriginalSize>,
) -> Option<ImageTransform> {
    let original = original_size?;
    if !node_width.is_finite()
        || !node_height.is_finite()
        || !original.width.is_finite()
        || !original.height.is_finite()
        || node_width <= SIZE_EPSILON
        || node_height <= SIZE_EPSILON
        || original.width <= SIZE_EPSILON
        || original.height <= SIZE_EPSILON
    {
        return None;
    }

    let node_aspect = node_width / node_height;
    let image_aspect = original.width / original.height;
    let (m00, m02, m11, m12) = if image_aspect > node_aspect {
        let visible_width = (node_aspect / image_aspect).clamp(0.0, 1.0);
        (visible_width, (1.0 - visible_width) * 0.5, 1.0, 0.0)
    } else {
        let visible_height = (image_aspect / node_aspect).clamp(0.0, 1.0);
        (1.0, 0.0, visible_height, (1.0 - visible_height) * 0.5)
    };
    Some(ImageTransform {
        m00,
        m01: 0.0,
        m02,
        m10: 0.0,
        m11,
        m12,
    })
}

/// Pan the bitmap inside a primary image-fill crop.
///
/// `local_dx/local_dy` are node-local document-pixel deltas. Translation is
/// clamped so all transformed unit-square corners stay inside source UV space,
/// preventing transparent gaps at the crop edges.
pub fn translate_primary_image_crop(
    node: &mut PenNode,
    node_width: f32,
    node_height: f32,
    local_dx: f32,
    local_dy: f32,
) -> bool {
    if !node_width.is_finite()
        || !node_height.is_finite()
        || node_width <= SIZE_EPSILON
        || node_height <= SIZE_EPSILON
        || !local_dx.is_finite()
        || !local_dy.is_finite()
        || (local_dx.abs() <= f32::EPSILON && local_dy.abs() <= f32::EPSILON)
    {
        return false;
    }

    let Some(fills) = node_fills_mut(node) else {
        return false;
    };
    let Some(PenFill::Image(body)) = fills.first_mut() else {
        return false;
    };
    if !image_fill_body_is_crop(body) {
        return false;
    }

    let Some(current) = body
        .transform
        .clone()
        .or_else(|| centered_crop_transform(node_width, node_height, body.original_size.as_ref()))
    else {
        return false;
    };
    let dx = local_dx / node_width;
    let dy = local_dy / node_height;
    let mut next = current.clone();
    next.m02 -= current.m00 * dx + current.m01 * dy;
    next.m12 -= current.m10 * dx + current.m11 * dy;

    let (min_x, max_x, min_y, max_y) = linear_corner_extents(&current);
    next.m02 = clamp_translation(next.m02, -min_x, 1.0 - max_x);
    next.m12 = clamp_translation(next.m12, -min_y, 1.0 - max_y);
    if next == current {
        return false;
    }
    body.transform = Some(next);
    true
}

fn linear_corner_extents(transform: &ImageTransform) -> (f32, f32, f32, f32) {
    let corners = [(0.0_f32, 0.0_f32), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for (x, y) in corners {
        let u = transform.m00 * x + transform.m01 * y;
        let v = transform.m10 * x + transform.m11 * y;
        min_x = min_x.min(u);
        max_x = max_x.max(u);
        min_y = min_y.min(v);
        max_y = max_y.max(v);
    }
    (min_x, max_x, min_y, max_y)
}

fn clamp_translation(value: f32, lower: f32, upper: f32) -> f32 {
    if lower <= upper {
        value.clamp(lower, upper)
    } else {
        // A malformed/skewed transform can span more than the full source.
        // No translation can eliminate gaps, so keep the excess centered.
        (lower + upper) * 0.5
    }
}

fn transform_array(transform: &ImageTransform) -> [f32; 6] {
    [
        transform.m00,
        transform.m01,
        transform.m02,
        transform.m10,
        transform.m11,
        transform.m12,
    ]
}

impl EditorState {
    pub fn can_edit_selected_image_crop(&self) -> bool {
        let id = &self.selection.anchor;
        id.is_real()
            && self.is_editable(id)
            && self
                .selected_node()
                .is_some_and(primary_image_fill_is_crop_editable)
    }

    /// Apply one live crop-pan delta and advance the document revision.
    pub fn translate_selected_image_crop(
        &mut self,
        node_width: f32,
        node_height: f32,
        local_dx: f32,
        local_dy: f32,
    ) -> bool {
        let id = self.selection.anchor.clone();
        if !id.is_real() || !self.is_editable(&id) {
            return false;
        }
        let Some(node) = find_node_mut(self.active_children_mut(), &id) else {
            return false;
        };
        let changed =
            translate_primary_image_crop(node, node_width, node_height, local_dx, local_dy);
        if changed {
            self.mark_document_changed();
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fills::{node_fills_mut, set_primary_fill_type, set_primary_image_fill_mode};
    use crate::{FillType, NodeId};

    fn crop_node(image_width: f32, image_height: f32) -> PenNode {
        let parsed = jian_ops_schema::load_str(
            r#"{"version":"1.0.0","children":[
                {"type":"rectangle","id":"photo","name":"Photo","x":0,"y":0,
                 "width":100,"height":100}
            ]}"#,
        )
        .expect("fixture parses")
        .value;
        let mut node = parsed.children.into_iter().next().expect("one node");
        set_primary_fill_type(&mut node, FillType::Image);
        let Some(PenFill::Image(body)) =
            node_fills_mut(&mut node).and_then(|fills| fills.first_mut())
        else {
            panic!("image fill");
        };
        body.mode = Some(SchemaImageFillMode::Crop);
        body.original_size = Some(ImageOriginalSize {
            width: image_width,
            height: image_height,
        });
        node
    }

    #[test]
    fn centered_cover_transform_uses_overflow_axis() {
        let transform = centered_crop_transform(
            100.0,
            100.0,
            Some(&ImageOriginalSize {
                width: 200.0,
                height: 100.0,
            }),
        )
        .expect("valid transform");
        assert_eq!(transform_array(&transform), [0.5, 0.0, 0.25, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn crop_pan_moves_translation_and_clamps_source_edges() {
        let mut node = crop_node(200.0, 100.0);
        assert!(translate_primary_image_crop(
            &mut node, 100.0, 100.0, 25.0, 0.0
        ));
        assert_eq!(
            primary_image_fill_transform(&node),
            Some([0.5, 0.0, 0.125, 0.0, 1.0, 0.0])
        );

        assert!(translate_primary_image_crop(
            &mut node, 100.0, 100.0, 1000.0, 0.0
        ));
        assert_eq!(
            primary_image_fill_transform(&node),
            Some([0.5, 0.0, 0.0, 0.0, 1.0, 0.0])
        );
        assert!(
            !translate_primary_image_crop(&mut node, 100.0, 100.0, 0.0, 50.0),
            "non-overflow axis stays clamped"
        );
    }

    #[test]
    fn image_mode_transition_initializes_and_clears_crop_transform() {
        let mut node = crop_node(200.0, 100.0);
        let Some(PenFill::Image(body)) =
            node_fills_mut(&mut node).and_then(|fills| fills.first_mut())
        else {
            panic!("image fill");
        };
        body.transform = None;

        assert!(set_primary_image_fill_mode(
            &mut node,
            crate::ImageFillMode::Crop
        ));
        assert_eq!(
            primary_image_fill_transform(&node),
            Some([0.5, 0.0, 0.25, 0.0, 1.0, 0.0])
        );
        assert!(set_primary_image_fill_mode(
            &mut node,
            crate::ImageFillMode::Fit
        ));
        assert_eq!(primary_image_fill_transform(&node), None);
    }

    #[test]
    fn editor_state_crop_pan_marks_document_changed() {
        let node = crop_node(200.0, 100.0);
        let mut state = EditorState::new();
        state.doc.children = vec![node];
        state.set_single_selection(NodeId::new("photo"));
        let revision = state.revision;
        assert!(state.can_edit_selected_image_crop());
        assert!(state.translate_selected_image_crop(100.0, 100.0, 10.0, 0.0));
        assert!(state.revision > revision);
    }

    #[test]
    fn transformless_crop_without_source_dimensions_is_not_editable() {
        let mut node = crop_node(200.0, 100.0);
        let Some(PenFill::Image(body)) =
            node_fills_mut(&mut node).and_then(|fills| fills.first_mut())
        else {
            panic!("image fill");
        };
        body.original_size = None;
        body.transform = None;
        assert!(!primary_image_fill_is_crop_editable(&node));
        assert!(!translate_primary_image_crop(
            &mut node, 100.0, 100.0, 10.0, 0.0
        ));
    }
}
