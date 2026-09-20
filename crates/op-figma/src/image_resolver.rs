//! Image-blob resolution — ports `figma-image-resolver.ts`. Walks the
//! converted document and replaces `__blob:N` / `__hash:HEX` image-
//! fill placeholders with `data:` URLs.

use base64::Engine;
use jian_ops_schema::document::PenDocument;
use jian_ops_schema::node::{ImageSrc, PenNode};
use jian_ops_schema::style::{PenFill, PenStroke};
use std::collections::{HashMap, HashSet};
#[cfg(test)]
use std::sync::Arc;

/// Host-supplied rewrite of raw image bytes, applied lazily to each
/// referenced blob before it is base64-encoded into the document.
/// Return `Some(bytes)` to replace the payload (e.g. the desktop
/// host's down-scale + re-encode pass, which keeps a 500MB image set
/// from entering the document at full resolution), `None` to keep the
/// original. The data-URL MIME is re-sniffed from the returned bytes,
/// so a transform may change the encoding format. op-figma stays
/// wasm32-clean by taking the transform as a callback instead of
/// depending on an image codec itself.
pub type ImageTransform<'a> = dyn Fn(&[u8]) -> Option<Vec<u8>> + 'a;

/// Encode raw image bytes as a `data:` URL, sniffing the MIME type
/// from the leading magic bytes (PNG fallback).
fn blob_to_data_url(bytes: &[u8]) -> String {
    let mime = match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x47, 0x49, ..] => "image/gif",
        [0x52, 0x49, ..] => "image/webp",
        _ => "image/png",
    };
    let prefix = format!("data:{mime};base64,");
    let encoded_len = base64::encoded_len(bytes.len(), true).unwrap_or(0);
    let mut url = String::with_capacity(prefix.len().saturating_add(encoded_len));
    url.push_str(&prefix);
    base64::engine::general_purpose::STANDARD.encode_string(bytes, &mut url);
    url
}

enum RawImage<'a> {
    Borrowed(&'a [u8]),
    Owned(Vec<u8>),
}

impl AsRef<[u8]> for RawImage<'_> {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Owned(bytes) => bytes,
        }
    }
}

enum ImagePools<'a> {
    Borrowed {
        blobs: &'a HashMap<u32, Vec<u8>>,
        files: &'a HashMap<String, Vec<u8>>,
    },
    Owned {
        blobs: HashMap<u32, Vec<u8>>,
        files: HashMap<String, Vec<u8>>,
    },
}

impl<'a> ImagePools<'a> {
    fn take_blob(&mut self, index: u32) -> Option<RawImage<'a>> {
        match self {
            Self::Borrowed { blobs, .. } => {
                let blobs: &'a HashMap<u32, Vec<u8>> = blobs;
                blobs
                    .get(&index)
                    .map(|bytes| RawImage::Borrowed(bytes.as_slice()))
            }
            Self::Owned { blobs, .. } => blobs.remove(&index).map(RawImage::Owned),
        }
    }

    fn take_file(&mut self, hash: &str) -> Option<RawImage<'a>> {
        match self {
            Self::Borrowed { files, .. } => {
                let files: &'a HashMap<String, Vec<u8>> = files;
                files
                    .get(hash)
                    .map(|bytes| RawImage::Borrowed(bytes.as_slice()))
            }
            Self::Owned { files, .. } => files.remove(hash).map(RawImage::Owned),
        }
    }
}

/// Lazy, memoized `__blob:` / `__hash:` → data-URL resolver. Encoding
/// (base64, which allocates + copies the whole payload) is deferred
/// until a fill actually references the blob, and the result is cached
/// as the schema's shared `ImageSrc` — a never-referenced blob is never
/// encoded at all, and a blob referenced by several fills is encoded exactly once,
/// with every subsequent reference sharing the same allocation via a
/// cheap refcount bump instead of a fresh `String` copy.
struct BlobCache<'a> {
    pools: ImagePools<'a>,
    transform: Option<&'a ImageTransform<'a>>,
    blob_urls: HashMap<u32, ImageSrc>,
    hash_urls: HashMap<String, ImageSrc>,
    /// Number of times a blob was actually base64-encoded (cache
    /// misses only) — exposed for tests that verify the memoization +
    /// never-referenced-blob-skipped invariants.
    encode_calls: u32,
}

impl<'a> BlobCache<'a> {
    fn new(
        image_blobs: &'a HashMap<u32, Vec<u8>>,
        image_files: &'a HashMap<String, Vec<u8>>,
        transform: Option<&'a ImageTransform<'a>>,
    ) -> Self {
        Self {
            pools: ImagePools::Borrowed {
                blobs: image_blobs,
                files: image_files,
            },
            transform,
            blob_urls: HashMap::new(),
            hash_urls: HashMap::new(),
            encode_calls: 0,
        }
    }

    fn from_owned(
        image_blobs: HashMap<u32, Vec<u8>>,
        image_files: HashMap<String, Vec<u8>>,
        transform: Option<&'a ImageTransform<'a>>,
    ) -> Self {
        Self {
            pools: ImagePools::Owned {
                blobs: image_blobs,
                files: image_files,
            },
            transform,
            blob_urls: HashMap::new(),
            hash_urls: HashMap::new(),
            encode_calls: 0,
        }
    }

    /// Apply the host transform (if any) then base64-encode. Runs on
    /// cache misses only, so a transform executes once per unique blob.
    fn encode(&mut self, bytes: RawImage<'a>) -> String {
        self.encode_calls += 1;
        match self
            .transform
            .and_then(|transform| transform(bytes.as_ref()))
        {
            Some(replaced) => {
                // With an owned pool the original allocation can leave
                // RSS before the transformed payload is base64-encoded.
                drop(bytes);
                blob_to_data_url(&replaced)
            }
            None => blob_to_data_url(bytes.as_ref()),
        }
    }

    /// Resolve a `__blob:` / `__hash:` placeholder to a shared data
    /// URL, encoding + caching on first reference only.
    fn resolve(&mut self, src: &str) -> Option<ImageSrc> {
        if let Some(rest) = src.strip_prefix("__blob:") {
            let index: u32 = rest.parse().ok()?;
            if let Some(cached) = self.blob_urls.get(&index) {
                return Some(cached.clone());
            }
            let bytes = self.pools.take_blob(index)?;
            let url = ImageSrc::from(self.encode(bytes));
            self.blob_urls.insert(index, url.clone());
            return Some(url);
        }
        if let Some(hash) = src.strip_prefix("__hash:") {
            if let Some(cached) = self.hash_urls.get(hash) {
                return Some(cached.clone());
            }
            let bytes = self.pools.take_file(hash)?;
            let url = ImageSrc::from(self.encode(bytes));
            self.hash_urls.insert(hash.to_string(), url.clone());
            return Some(url);
        }
        None
    }
}

#[derive(Default)]
struct ReferencedImages {
    blobs: HashSet<u32>,
    files: HashSet<String>,
}

fn collect_source_ref(source: &str, refs: &mut ReferencedImages) {
    if let Some(index) = source
        .strip_prefix("__blob:")
        .and_then(|value| value.parse().ok())
    {
        refs.blobs.insert(index);
    } else if let Some(hash) = source.strip_prefix("__hash:") {
        refs.files.insert(hash.to_string());
    }
}

fn collect_fill_refs(fills: Option<&[PenFill]>, refs: &mut ReferencedImages) {
    let Some(fills) = fills else { return };
    for fill in fills {
        let PenFill::Image(image) = fill else {
            continue;
        };
        collect_source_ref(&image.url, refs);
    }
}

fn collect_stroke_refs(stroke: Option<&PenStroke>, refs: &mut ReferencedImages) {
    collect_fill_refs(stroke.and_then(|stroke| stroke.fill.as_deref()), refs);
}

fn collect_node_refs(node: &PenNode, refs: &mut ReferencedImages) {
    let children = match node {
        PenNode::Frame(frame) => {
            collect_fill_refs(frame.container.fill.as_deref(), refs);
            collect_stroke_refs(frame.container.stroke.as_ref(), refs);
            frame.children.as_ref()
        }
        PenNode::Group(group) => {
            collect_fill_refs(group.container.fill.as_deref(), refs);
            collect_stroke_refs(group.container.stroke.as_ref(), refs);
            group.children.as_ref()
        }
        PenNode::Rectangle(rectangle) => {
            collect_fill_refs(rectangle.container.fill.as_deref(), refs);
            collect_stroke_refs(rectangle.container.stroke.as_ref(), refs);
            rectangle.children.as_ref()
        }
        PenNode::Ellipse(ellipse) => {
            collect_fill_refs(ellipse.fill.as_deref(), refs);
            collect_stroke_refs(ellipse.stroke.as_ref(), refs);
            None
        }
        PenNode::Line(line) => {
            collect_stroke_refs(line.stroke.as_ref(), refs);
            None
        }
        PenNode::Polygon(polygon) => {
            collect_fill_refs(polygon.fill.as_deref(), refs);
            collect_stroke_refs(polygon.stroke.as_ref(), refs);
            None
        }
        PenNode::Path(path) => {
            collect_fill_refs(path.fill.as_deref(), refs);
            collect_stroke_refs(path.stroke.as_ref(), refs);
            None
        }
        PenNode::Text(text) => {
            collect_fill_refs(text.fill.as_deref(), refs);
            None
        }
        PenNode::TextInput(input) => {
            collect_fill_refs(input.fill.as_deref(), refs);
            collect_stroke_refs(input.stroke.as_ref(), refs);
            None
        }
        PenNode::Image(image) => {
            collect_source_ref(&image.src, refs);
            None
        }
        PenNode::IconFont(icon) => {
            collect_fill_refs(icon.fill.as_deref(), refs);
            collect_stroke_refs(icon.stroke.as_ref(), refs);
            None
        }
        PenNode::TextArea(area) => {
            collect_fill_refs(area.fill.as_deref(), refs);
            collect_stroke_refs(area.stroke.as_ref(), refs);
            None
        }
        PenNode::Select(select) => {
            collect_fill_refs(select.fill.as_deref(), refs);
            collect_stroke_refs(select.stroke.as_ref(), refs);
            None
        }
        PenNode::Switch(switch) => {
            collect_fill_refs(switch.fill.as_deref(), refs);
            collect_stroke_refs(switch.stroke.as_ref(), refs);
            None
        }
        PenNode::Checkbox(checkbox) => {
            collect_fill_refs(checkbox.fill.as_deref(), refs);
            collect_stroke_refs(checkbox.stroke.as_ref(), refs);
            None
        }
        PenNode::Slider(slider) => {
            collect_fill_refs(slider.fill.as_deref(), refs);
            collect_stroke_refs(slider.stroke.as_ref(), refs);
            None
        }
        PenNode::RadioGroup(group) => {
            collect_fill_refs(group.fill.as_deref(), refs);
            collect_stroke_refs(group.stroke.as_ref(), refs);
            None
        }
        PenNode::NumberInput(input) => {
            collect_fill_refs(input.fill.as_deref(), refs);
            collect_stroke_refs(input.stroke.as_ref(), refs);
            None
        }
        PenNode::Progress(progress) => {
            collect_fill_refs(progress.fill.as_deref(), refs);
            collect_stroke_refs(progress.stroke.as_ref(), refs);
            None
        }
        PenNode::Tabs(tabs) => {
            collect_fill_refs(tabs.fill.as_deref(), refs);
            collect_stroke_refs(tabs.stroke.as_ref(), refs);
            tabs.children.as_ref()
        }
        PenNode::Ref(reference) => reference.children.as_ref(),
    };
    if let Some(children) = children {
        for child in children {
            collect_node_refs(child, refs);
        }
    }
}

fn collect_document_refs(doc: &PenDocument) -> ReferencedImages {
    let mut refs = ReferencedImages::default();
    if let Some(pages) = &doc.pages {
        for page in pages {
            for child in &page.children {
                collect_node_refs(child, &mut refs);
            }
        }
    }
    for child in &doc.children {
        collect_node_refs(child, &mut refs);
    }
    refs
}

/// Discard raw in-container image blobs which the converted document
/// cannot reach. Single-page imports otherwise retain images used only
/// by the other pages until the base64 phase completes.
pub(crate) fn retain_referenced_image_blobs(
    doc: &PenDocument,
    image_blobs: &mut HashMap<u32, Vec<u8>>,
) {
    let refs = collect_document_refs(doc);
    image_blobs.retain(|index, _| refs.blobs.contains(index));
}

fn patch_fills(fills: &mut Option<Vec<PenFill>>, cache: &mut BlobCache) -> usize {
    let mut count = 0;
    if let Some(fills) = fills {
        for fill in fills {
            if let PenFill::Image(img) = fill {
                if img.url.starts_with("__blob:") || img.url.starts_with("__hash:") {
                    if let Some(url) = cache.resolve(&img.url) {
                        img.url = url;
                        count += 1;
                    }
                }
            }
        }
    }
    count
}

fn patch_stroke(stroke: &mut Option<PenStroke>, cache: &mut BlobCache) -> usize {
    stroke
        .as_mut()
        .map_or(0, |stroke| patch_fills(&mut stroke.fill, cache))
}

fn patch_source(source: &mut ImageSrc, cache: &mut BlobCache) -> usize {
    if !source.starts_with("__blob:") && !source.starts_with("__hash:") {
        return 0;
    }
    let Some(resolved) = cache.resolve(source) else {
        return 0;
    };
    *source = resolved;
    1
}

/// Patch one node's image fills, then recurse into its children.
fn patch_node(node: &mut PenNode, cache: &mut BlobCache) -> usize {
    let mut count = 0;
    let children: Option<&mut Vec<PenNode>> = match node {
        PenNode::Frame(f) => {
            count += patch_fills(&mut f.container.fill, cache);
            count += patch_stroke(&mut f.container.stroke, cache);
            f.children.as_mut()
        }
        PenNode::Group(g) => {
            count += patch_fills(&mut g.container.fill, cache);
            count += patch_stroke(&mut g.container.stroke, cache);
            g.children.as_mut()
        }
        PenNode::Rectangle(r) => {
            count += patch_fills(&mut r.container.fill, cache);
            count += patch_stroke(&mut r.container.stroke, cache);
            r.children.as_mut()
        }
        PenNode::Ellipse(e) => {
            count += patch_fills(&mut e.fill, cache);
            count += patch_stroke(&mut e.stroke, cache);
            None
        }
        PenNode::Line(l) => {
            count += patch_stroke(&mut l.stroke, cache);
            None
        }
        PenNode::Polygon(p) => {
            count += patch_fills(&mut p.fill, cache);
            count += patch_stroke(&mut p.stroke, cache);
            None
        }
        PenNode::Path(p) => {
            count += patch_fills(&mut p.fill, cache);
            count += patch_stroke(&mut p.stroke, cache);
            None
        }
        PenNode::Text(t) => {
            count += patch_fills(&mut t.fill, cache);
            None
        }
        PenNode::TextInput(input) => {
            count += patch_fills(&mut input.fill, cache);
            count += patch_stroke(&mut input.stroke, cache);
            None
        }
        PenNode::Image(image) => {
            count += patch_source(&mut image.src, cache);
            None
        }
        PenNode::IconFont(icon) => {
            count += patch_fills(&mut icon.fill, cache);
            count += patch_stroke(&mut icon.stroke, cache);
            None
        }
        PenNode::TextArea(area) => {
            count += patch_fills(&mut area.fill, cache);
            count += patch_stroke(&mut area.stroke, cache);
            None
        }
        PenNode::Select(select) => {
            count += patch_fills(&mut select.fill, cache);
            count += patch_stroke(&mut select.stroke, cache);
            None
        }
        PenNode::Switch(switch) => {
            count += patch_fills(&mut switch.fill, cache);
            count += patch_stroke(&mut switch.stroke, cache);
            None
        }
        PenNode::Checkbox(checkbox) => {
            count += patch_fills(&mut checkbox.fill, cache);
            count += patch_stroke(&mut checkbox.stroke, cache);
            None
        }
        PenNode::Slider(slider) => {
            count += patch_fills(&mut slider.fill, cache);
            count += patch_stroke(&mut slider.stroke, cache);
            None
        }
        PenNode::RadioGroup(group) => {
            count += patch_fills(&mut group.fill, cache);
            count += patch_stroke(&mut group.stroke, cache);
            None
        }
        PenNode::NumberInput(input) => {
            count += patch_fills(&mut input.fill, cache);
            count += patch_stroke(&mut input.stroke, cache);
            None
        }
        PenNode::Progress(progress) => {
            count += patch_fills(&mut progress.fill, cache);
            count += patch_stroke(&mut progress.stroke, cache);
            None
        }
        PenNode::Tabs(tabs) => {
            count += patch_fills(&mut tabs.fill, cache);
            count += patch_stroke(&mut tabs.stroke, cache);
            tabs.children.as_mut()
        }
        PenNode::Ref(r) => r.children.as_mut(),
    };
    if let Some(children) = children {
        for child in children {
            count += patch_node(child, cache);
        }
    }
    count
}

/// Replace every image-fill placeholder in `doc` with a data URL,
/// applying an optional host [`ImageTransform`] to each referenced
/// blob's bytes before encoding. Returns the number of replacements
/// made.
pub fn resolve_image_blobs_with(
    doc: &mut PenDocument,
    image_blobs: &HashMap<u32, Vec<u8>>,
    image_files: &HashMap<String, Vec<u8>>,
    transform: Option<&ImageTransform<'_>>,
) -> usize {
    if image_blobs.is_empty() && image_files.is_empty() {
        return 0;
    }
    let mut cache = BlobCache::new(image_blobs, image_files, transform);

    let mut count = 0;
    if let Some(pages) = &mut doc.pages {
        for page in pages {
            for child in &mut page.children {
                count += patch_node(child, &mut cache);
            }
        }
    }
    for child in &mut doc.children {
        count += patch_node(child, &mut cache);
    }
    count
}

/// Owned variant used by file import. Raw payloads are removed from
/// their maps on first use, so previously encoded data URLs do not
/// coexist with the complete original image pool. Repeated fills still
/// share one encoded [`ImageSrc`] through the resolver cache.
pub(crate) fn resolve_image_blobs_owned_with(
    doc: &mut PenDocument,
    mut image_blobs: HashMap<u32, Vec<u8>>,
    mut image_files: HashMap<String, Vec<u8>>,
    transform: Option<&ImageTransform<'_>>,
) -> usize {
    let refs = collect_document_refs(doc);
    image_blobs.retain(|index, _| refs.blobs.contains(index));
    image_files.retain(|hash, _| refs.files.contains(hash));
    if image_blobs.is_empty() && image_files.is_empty() {
        return 0;
    }

    let mut cache = BlobCache::from_owned(image_blobs, image_files, transform);
    let mut count = 0;
    if let Some(pages) = &mut doc.pages {
        for page in pages {
            for child in &mut page.children {
                count += patch_node(child, &mut cache);
            }
        }
    }
    for child in &mut doc.children {
        count += patch_node(child, &mut cache);
    }
    count
}

#[cfg(test)]
mod tests;
