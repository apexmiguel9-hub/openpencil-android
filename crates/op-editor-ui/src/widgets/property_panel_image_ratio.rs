//! Intrinsic image size behind the image popover's "match ratio" row.
//!
//! The row keeps the node's width and rewrites its height to the
//! image's own proportions, so the only thing this module has to answer
//! is *how big is the source image really*. The write itself lives in
//! `op_editor_core::image_aspect`.

use crate::widgets::canvas_viewport_image::image_source_bytes;
use jian_ops_schema::node::PenNode;
use op_editor_core::ImageFillSummary;

/// Intrinsic pixel size of the image `summary` describes.
///
/// An image FILL authors `originalSize` when the picture is uploaded,
/// dropped or imported, so that is the primary source. The fallback
/// reads the size out of the encoded raster header for the two cases
/// that have no authored value: a standalone `PenNode::Image`, whose
/// schema carries no `originalSize` field at all, and `.op` files
/// written before fills recorded one. It reads through the same shared
/// byte cache the popover's preview thumbnail already paints from, so
/// it costs a cache lookup rather than a decode, and it covers
/// host-fetched remote images for free.
pub fn image_source_size(summary: &ImageFillSummary) -> Option<[f32; 2]> {
    if let Some(size) = summary.original_size {
        return Some(size);
    }
    let src = summary.image_url.as_deref()?;
    let bytes = image_source_bytes(src, jian_ops_schema::node::image_src::paint_image_id(src))?;
    let (width, height) = crate::image_runtime::encoded_image_dimensions(&bytes)?;
    Some([width as f32, height as f32])
}

/// Same resolution, straight off a node — the hosts' entry point, since
/// they hold the document node rather than the panel snapshot.
pub fn node_image_source_size(node: &PenNode) -> Option<[f32; 2]> {
    let summary = op_editor_core::first_image_fill_summary(node)
        .or_else(|| op_editor_core::image_node_summary(node))?;
    image_source_size(&summary)
}
