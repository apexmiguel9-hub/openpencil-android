use crate::layout_scene::{stable_image_source_id, SceneNode};

pub(super) fn intrinsic_dimensions(n: &SceneNode, src: &str) -> Option<(f32, f32)> {
    let id = if n.image_src_id == 0 {
        stable_image_source_id(src)
    } else {
        n.image_src_id
    };
    let bytes = crate::widgets::canvas_viewport_image::image_source_bytes(src, id)?;
    crate::image_runtime::encoded_image_dimensions(&bytes)
        .map(|(width, height)| (width as f32, height as f32))
}
