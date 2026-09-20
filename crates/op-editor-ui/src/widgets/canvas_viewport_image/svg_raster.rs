//! SVG → PNG rasterization at the image byte-cache seam.
//!
//! Skia (native) and CanvasKit (web) decode PNG / JPEG / GIF / WebP — not
//! SVG — so an SVG that reaches the painter as bytes (a captured page's
//! `data:image/svg+xml` fallback, a remote `.svg` the host fetched) used to
//! paint as the dashed placeholder forever. Every byte payload entering the
//! shared cache passes through [`ensure_raster_bytes`]: SVG sources are
//! rasterized once with resvg and cached as PNG, everything else passes
//! through untouched, and both hosts then paint an ordinary bitmap.
//!
//! Deliberately minimal resvg: no `text` support (it needs fontdb and system
//! font access the wasm host cannot have), so an `<svg>` with `<text>`
//! renders its shapes only. The capture side already vectorizes flat-colour
//! icon art into native path nodes; this is the fallback for everything that
//! path cannot express (strokes, gradients, masks, mixed art).

use std::sync::Arc;

/// Longest raster edge produced for an SVG, in pixels. Matches the capture
/// side's own image ceiling; anything larger is downscaled to fit.
#[cfg(not(target_arch = "wasm32"))]
const MAX_SVG_RASTER_EDGE: f32 = 2048.0;
/// Oversampling for small art so a zoomed-in icon stays crisp instead of
/// blurring at the first 2× zoom.
#[cfg(not(target_arch = "wasm32"))]
const SVG_RASTER_SCALE: f32 = 2.0;

/// Rasterize `bytes` when they are an SVG document; return them unchanged
/// otherwise (including on any parse or render failure — the caller then
/// paints the usual undecodable-image placeholder, exactly as before).
///
/// On wasm32 this is a pass-through: resvg measured +0.9 MiB gzip against
/// the web bundle's 6 MiB ceiling, and the browser host does not need it —
/// its CanvasKit bridge (`op_ck_image_cache.js`) falls back to the
/// browser's own SVG decoder when `MakeImageFromEncoded` rejects the bytes.
pub(crate) fn ensure_raster_bytes(bytes: Arc<[u8]>) -> Arc<[u8]> {
    #[cfg(target_arch = "wasm32")]
    {
        bytes
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if !sniffs_as_svg(&bytes) {
            return bytes;
        }
        match rasterize(&bytes) {
            Some(png) => Arc::from(png.into_boxed_slice()),
            None => bytes,
        }
    }
}

/// Loose sniff: the payload reads as XML-ish text whose first 4 KiB contain
/// an `<svg` tag. A false positive only costs a failed parse, which passes
/// the bytes through unchanged.
#[cfg(not(target_arch = "wasm32"))]
fn sniffs_as_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(4096)];
    let Ok(text) = std::str::from_utf8(head) else {
        return false;
    };
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with('<') && trimmed.contains("<svg")
}

#[cfg(not(target_arch = "wasm32"))]
fn rasterize(bytes: &[u8]) -> Option<Vec<u8>> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &options).ok()?;
    // `usvg::Size` guarantees positive finite dimensions by construction;
    // the guards keep that assumption local instead of trusting it forever.
    let size = tree.size();
    if size.width() <= 0.0 || size.height() <= 0.0 {
        return None;
    }
    let longest = size.width().max(size.height());
    let scale = SVG_RASTER_SCALE.min(MAX_SVG_RASTER_EDGE / longest);
    if scale <= 0.0 || !scale.is_finite() {
        return None;
    }
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_RECT_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#ff0000"/></svg>"##;

    fn arc(bytes: &[u8]) -> Arc<[u8]> {
        Arc::from(bytes.to_vec().into_boxed_slice())
    }

    #[test]
    fn an_svg_document_becomes_png_bytes() {
        let out = ensure_raster_bytes(arc(RED_RECT_SVG.as_bytes()));
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"), "rasterized to PNG");
    }

    #[test]
    fn a_stroked_svg_also_rasterizes() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M4 4L20 20" stroke="#123456" stroke-width="2" fill="none"/></svg>"##;
        let out = ensure_raster_bytes(arc(svg.as_bytes()));
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn png_bytes_pass_through_untouched() {
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        let out = ensure_raster_bytes(arc(&png));
        assert_eq!(&out[..], &png[..]);
    }

    #[test]
    fn malformed_svg_text_passes_through() {
        let bytes = b"<svg definitely not well formed".to_vec();
        let out = ensure_raster_bytes(arc(&bytes));
        assert_eq!(&out[..], &bytes[..]);
    }

    #[test]
    fn a_leading_xml_prolog_and_bom_still_sniff() {
        let svg = format!("\u{feff}<?xml version=\"1.0\"?>{RED_RECT_SVG}");
        let out = ensure_raster_bytes(arc(svg.as_bytes()));
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    /// An enormous authored size is clamped to the raster ceiling instead of
    /// allocating a multi-gigabyte pixmap.
    #[test]
    fn an_oversized_svg_is_clamped_to_the_edge_ceiling() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100000" height="50"><rect width="100000" height="50" fill="#00ff00"/></svg>"##;
        let out = ensure_raster_bytes(arc(svg.as_bytes()));
        assert!(out.starts_with(b"\x89PNG\r\n\x1a\n"));
        // PNG width lives in the IHDR chunk at offset 16..20.
        let width = u32::from_be_bytes([out[16], out[17], out[18], out[19]]);
        assert!(width <= 2048, "width {width} must be clamped");
    }
}
