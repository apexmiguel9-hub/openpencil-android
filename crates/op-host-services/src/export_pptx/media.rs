//! Embedded bitmaps: getting bytes out of a scene node, keeping one
//! copy of each, and knowing how big the source actually is.
//!
//! A `.pptx` carries its pictures as real files inside the zip, so an
//! image reaches a slide only if this module can produce BYTES for it.
//! That is the whole reason `http(s)` sources take the raster path: the
//! export must present on a machine with no network, and a `blipFill`
//! pointing at a URL is not a picture, it is a promise.
//!
//! Source dimensions matter more here than in the HTML exporter.
//! `background-size:cover` is a CSS keyword; DrawingML has no keyword —
//! covering a box means computing the crop rectangle yourself, and the
//! only honest way to compute it is from the real pixel size of the
//! bitmap. Hence [`image_size`], which reads the dimensions straight out
//! of the encoded bytes.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash as _, Hasher as _};

use super::package::{content_type_for, MediaFile};

/// The package's media table, de-duplicated by content.
///
/// A deck that repeats one logo on twenty slides carries the bytes once:
/// the loader hands the same `data:` URL to twenty nodes, and twenty
/// copies of a 400 KB PNG is an 8 MB file the presenter has to mail.
#[derive(Default)]
pub struct MediaLibrary {
    files: Vec<MediaFile>,
    /// Content hash of `files[i]`, parallel by index.
    hashes: Vec<u64>,
}

impl MediaLibrary {
    /// Add `bytes` (or find the identical bytes already present) and
    /// return its index in the package media table.
    pub fn intern(&mut self, ext: &'static str, bytes: Vec<u8>) -> usize {
        let mut hasher = DefaultHasher::new();
        ext.hash(&mut hasher);
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        if let Some(existing) = self.hashes.iter().position(|h| *h == hash) {
            // A 64-bit content hash collision between two distinct
            // images is not a correctness risk worth a byte compare
            // here; the images would have to be adversarially chosen.
            return existing;
        }
        self.files.push(MediaFile { ext, bytes });
        self.hashes.push(hash);
        self.files.len() - 1
    }

    pub fn into_files(self) -> Vec<MediaFile> {
        self.files
    }
}

/// Decode a `data:` URL into an embeddable extension + bytes.
///
/// `None` for anything that is not base64 `data:` with a media type the
/// package can declare — a `data:image/svg+xml` payload is real bytes
/// but `[Content_Types].xml` has no entry PowerPoint would accept for a
/// `blip`, so it goes down the raster path with everything else.
pub fn decode_data_url(src: &str) -> Option<(&'static str, Vec<u8>)> {
    use base64::Engine as _;

    let trimmed = src.trim();
    let rest = trimmed.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    if !meta.trim_end().ends_with(";base64") {
        // A percent-encoded data URL is legal but never produced by the
        // host's importers, and guessing at its decoding would be a
        // silent way to embed the wrong bytes.
        return None;
    }
    let mime = meta.split(';').next()?.trim().to_ascii_lowercase();
    let ext = extension_for_mime(&mime)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some((ext, bytes))
}

/// Media type → the package extension, gated on the extension being one
/// `[Content_Types].xml` can declare.
fn extension_for_mime(mime: &str) -> Option<&'static str> {
    let ext = match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpeg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        _ => return None,
    };
    content_type_for(ext).map(|_| ext)
}

/// Pixel dimensions read out of encoded image bytes, or `None` when the
/// format's header is not one this reader understands.
///
/// Only the container headers are parsed — nothing is decoded — so the
/// cost is a handful of byte reads regardless of image size.
pub fn image_size(ext: &str, bytes: &[u8]) -> Option<(f32, f32)> {
    match ext {
        "png" => png_size(bytes),
        "jpeg" => jpeg_size(bytes),
        "gif" => gif_size(bytes),
        "webp" => webp_size(bytes),
        _ => None,
    }
}

fn be_u32(b: &[u8], at: usize) -> Option<u32> {
    let slice = b.get(at..at + 4)?;
    Some(u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn be_u16(b: &[u8], at: usize) -> Option<u16> {
    let slice = b.get(at..at + 2)?;
    Some(u16::from_be_bytes([slice[0], slice[1]]))
}

fn size_of(w: u32, h: u32) -> Option<(f32, f32)> {
    if w > 0 && h > 0 {
        Some((w as f32, h as f32))
    } else {
        None
    }
}

/// PNG: the IHDR chunk is mandated to be first, so width and height sit
/// at fixed offsets 16 and 20.
fn png_size(b: &[u8]) -> Option<(f32, f32)> {
    if !b.starts_with(&[0x89, b'P', b'N', b'G']) {
        return None;
    }
    size_of(be_u32(b, 16)?, be_u32(b, 20)?)
}

/// JPEG: walk the marker segments to the start-of-frame, which is the
/// only one carrying the frame size. SOF0..SOF15 all qualify except the
/// three that are not frame headers (DHT `C4`, JPG `C8`, DAC `CC`).
fn jpeg_size(b: &[u8]) -> Option<(f32, f32)> {
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2usize;
    while i + 3 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = b[i + 1];
        // Padding fill bytes and the standalone markers carry no length.
        if marker == 0xFF || (0xD0..=0xD9).contains(&marker) || marker == 0x01 {
            i += 1;
            continue;
        }
        let length = be_u16(b, i + 2)? as usize;
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let height = be_u16(b, i + 5)? as u32;
            let width = be_u16(b, i + 7)? as u32;
            return size_of(width, height);
        }
        i += 2 + length.max(2);
    }
    None
}

/// GIF: the logical screen descriptor is fixed at offset 6, little
/// endian.
fn gif_size(b: &[u8]) -> Option<(f32, f32)> {
    if !b.starts_with(b"GIF8") {
        return None;
    }
    let w = u16::from_le_bytes([*b.get(6)?, *b.get(7)?]) as u32;
    let h = u16::from_le_bytes([*b.get(8)?, *b.get(9)?]) as u32;
    size_of(w, h)
}

/// WEBP: the extended (`VP8X`) and simple-lossy (`VP8 `) headers.
/// Lossless (`VP8L`) packs its size into a bit stream and is left to the
/// raster path rather than bit-twiddled here.
fn webp_size(b: &[u8]) -> Option<(f32, f32)> {
    if !b.starts_with(b"RIFF") || b.get(8..12)? != b"WEBP" {
        return None;
    }
    match b.get(12..16)? {
        b"VP8X" => {
            let w = 1
                + u32::from(*b.get(24)?)
                + (u32::from(*b.get(25)?) << 8)
                + (u32::from(*b.get(26)?) << 16);
            let h = 1
                + u32::from(*b.get(27)?)
                + (u32::from(*b.get(28)?) << 8)
                + (u32::from(*b.get(29)?) << 16);
            size_of(w, h)
        }
        b"VP8 " => {
            // The keyframe header starts after the 3-byte frame tag and
            // the 3-byte start code.
            if b.get(23..26)? != [0x9D, 0x01, 0x2A] {
                return None;
            }
            let w = u16::from_le_bytes([*b.get(26)?, *b.get(27)?]) as u32 & 0x3FFF;
            let h = u16::from_le_bytes([*b.get(28)?, *b.get(29)?]) as u32 & 0x3FFF;
            size_of(w, h)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest legal PNG header the size reader needs — signature
    /// plus an IHDR declaring 800×600.
    fn png_header(w: u32, h: u32) -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b
    }

    #[test]
    fn png_dimensions_come_from_the_ihdr() {
        assert_eq!(
            image_size("png", &png_header(800, 600)),
            Some((800.0, 600.0))
        );
    }

    #[test]
    fn a_data_url_decodes_to_bytes_and_a_declarable_extension() {
        use base64::Engine as _;
        let bytes = png_header(4, 2);
        let url = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        );
        let (ext, decoded) = decode_data_url(&url).expect("decodes");
        assert_eq!(ext, "png");
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn an_undeclarable_media_type_is_refused_rather_than_embedded() {
        // SVG bytes are real, but no `[Content_Types]` default this
        // package writes would let PowerPoint read them as a blip.
        assert!(decode_data_url("data:image/svg+xml;base64,PHN2Zy8+").is_none());
        assert!(decode_data_url("https://example.com/a.png").is_none());
        assert!(decode_data_url("data:image/png,%89PNG").is_none());
    }

    #[test]
    fn identical_bytes_are_stored_once() {
        let mut library = MediaLibrary::default();
        let first = library.intern("png", vec![1, 2, 3]);
        let again = library.intern("png", vec![1, 2, 3]);
        let other = library.intern("png", vec![9, 9, 9]);
        assert_eq!(first, again);
        assert_ne!(first, other);
        assert_eq!(library.into_files().len(), 2);
    }

    #[test]
    fn jpeg_dimensions_come_from_the_start_of_frame() {
        // SOI, a skipped APP0 segment, then an SOF0 declaring 120×60.
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
        b.extend_from_slice(&60u16.to_be_bytes());
        b.extend_from_slice(&120u16.to_be_bytes());
        assert_eq!(image_size("jpeg", &b), Some((120.0, 60.0)));
    }
}
