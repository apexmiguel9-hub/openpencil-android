use super::*;
use serde_json::json;

#[test]
fn png_mime_is_default() {
    assert!(blob_to_data_url(&[0x89, 0x50, 0x4e, 0x47]).starts_with("data:image/png;base64,"));
}

#[test]
fn jpeg_mime_detected() {
    assert!(blob_to_data_url(&[0xFF, 0xD8, 0xFF, 0xE0]).starts_with("data:image/jpeg;base64,"));
}

fn png_bytes() -> Vec<u8> {
    vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0xAA]
}

fn document_with_urls(urls: &[&str]) -> PenDocument {
    let children = urls
        .iter()
        .enumerate()
        .map(|(index, url)| {
            json!({
                "type": "rectangle",
                "id": format!("image-{index}"),
                "fill": [{"type": "image", "url": url}],
            })
        })
        .collect::<Vec<_>>();
    serde_json::from_value(json!({"version": "1", "children": children}))
        .expect("test document is valid")
}

#[test]
fn resolve_blob_ref() {
    let mut blobs = HashMap::new();
    blobs.insert(3u32, png_bytes());
    let no_files = HashMap::new();
    let mut cache = BlobCache::new(&blobs, &no_files, None);
    let resolved = cache.resolve("__blob:3").expect("blob 3 resolves");
    assert!(resolved.starts_with("data:image/png;base64,"));
    assert!(cache.resolve("__blob:9").is_none());
}

#[test]
fn resolve_hash_ref() {
    let mut files = HashMap::new();
    files.insert("abcd".to_string(), png_bytes());
    let no_blobs = HashMap::new();
    let mut cache = BlobCache::new(&no_blobs, &files, None);
    let resolved = cache.resolve("__hash:abcd").expect("hash abcd resolves");
    assert!(resolved.starts_with("data:image/png;base64,"));
}

/// A blob that no fill ever references must never be run through
/// `blob_to_data_url` — the whole point of laziness is to skip the
/// (potentially large) encode for image-heavy files where most
/// blobs aren't actually used by any visible fill.
#[test]
fn unreferenced_blob_is_never_encoded() {
    let mut blobs = HashMap::new();
    blobs.insert(0u32, png_bytes());
    blobs.insert(1u32, png_bytes());
    let no_files = HashMap::new();
    let mut cache = BlobCache::new(&blobs, &no_files, None);

    cache.resolve("__blob:0").expect("blob 0 resolves");

    assert_eq!(cache.encode_calls, 1, "only the referenced blob is encoded");
    assert!(cache.blob_urls.contains_key(&0));
    assert!(
        !cache.blob_urls.contains_key(&1),
        "blob 1 was never referenced and must not be cached/encoded"
    );
}

/// A host-supplied transform (e.g. desktop down-scaling) rewrites
/// the payload before base64 encoding — the data URL must carry
/// the transformed bytes (and their sniffed MIME), and memoization
/// must keep the transform to one call per unique blob.
#[test]
fn transform_rewrites_referenced_blob_before_encoding() {
    use std::cell::Cell;
    let mut blobs = HashMap::new();
    blobs.insert(0u32, png_bytes());
    let no_files = HashMap::new();
    let calls = Cell::new(0u32);
    let transform = |_bytes: &[u8]| {
        calls.set(calls.get() + 1);
        // Pretend the host re-encoded the image as JPEG.
        Some(vec![0xFF, 0xD8, 0xFF, 0xE0])
    };
    let mut cache = BlobCache::new(&blobs, &no_files, Some(&transform));

    let first = cache.resolve("__blob:0").expect("blob 0 resolves");
    let second = cache.resolve("__blob:0").expect("blob 0 resolves again");

    assert!(
        first.starts_with("data:image/jpeg;base64,"),
        "data URL carries the transformed bytes' MIME, got {first}"
    );
    assert!(
        Arc::ptr_eq(&first.as_arc(), &second.as_arc()),
        "memoized after transform"
    );
    assert_eq!(calls.get(), 1, "transform runs once per unique blob");
}

/// A transform returning `None` keeps the original payload — the
/// pass-through contract mirrors `maybe_downscale`'s "already
/// small enough" case.
#[test]
fn transform_none_keeps_original_bytes() {
    let mut files = HashMap::new();
    files.insert("abcd".to_string(), png_bytes());
    let no_blobs = HashMap::new();
    let transform = |_bytes: &[u8]| None;
    let mut cache = BlobCache::new(&no_blobs, &files, Some(&transform));
    let resolved = cache.resolve("__hash:abcd").expect("hash abcd resolves");
    assert!(
        resolved.starts_with("data:image/png;base64,"),
        "original PNG payload survives a None transform"
    );
}

/// Laziness extends to the transform: a blob no fill references
/// must never be transformed (the whole point for image-heavy
/// files where some blobs are unused).
#[test]
fn unreferenced_blob_is_never_transformed() {
    use std::cell::Cell;
    let mut blobs = HashMap::new();
    blobs.insert(0u32, png_bytes());
    blobs.insert(1u32, png_bytes());
    let no_files = HashMap::new();
    let calls = Cell::new(0u32);
    let transform = |_bytes: &[u8]| {
        calls.set(calls.get() + 1);
        None
    };
    let mut cache = BlobCache::new(&blobs, &no_files, Some(&transform));
    cache.resolve("__blob:0").expect("blob 0 resolves");
    assert_eq!(calls.get(), 1, "only the referenced blob is transformed");
}

/// Two fills pointing at the same blob must share the encoding
/// work: the base64 pass runs once, and both callers get the same
/// `Arc` allocation back (a refcount bump, not a fresh string copy).
#[test]
fn shared_blob_is_encoded_exactly_once() {
    let mut blobs = HashMap::new();
    blobs.insert(0u32, png_bytes());
    let no_files = HashMap::new();
    let mut cache = BlobCache::new(&blobs, &no_files, None);

    let first = cache.resolve("__blob:0").expect("first fill resolves");
    let second = cache.resolve("__blob:0").expect("second fill resolves");

    assert_eq!(
        cache.encode_calls, 1,
        "encode runs once despite two references"
    );
    assert!(
        Arc::ptr_eq(&first.as_arc(), &second.as_arc()),
        "both references share the same Arc allocation"
    );
}

#[test]
fn owned_pool_removes_raw_bytes_on_first_use() {
    let mut blobs = HashMap::new();
    blobs.insert(0u32, png_bytes());
    let mut cache = BlobCache::from_owned(blobs, HashMap::new(), None);

    let first = cache.resolve("__blob:0").expect("first fill resolves");
    let ImagePools::Owned { blobs, .. } = &cache.pools else {
        panic!("expected owned image pool")
    };
    assert!(
        !blobs.contains_key(&0),
        "raw allocation leaves the pool as soon as its URL is encoded"
    );
    let second = cache.resolve("__blob:0").expect("cached fill resolves");
    assert_eq!(cache.encode_calls, 1);
    assert!(Arc::ptr_eq(&first.as_arc(), &second.as_arc()));
}

#[test]
fn owned_resolver_keeps_only_referenced_blob_and_file_payloads() {
    use std::cell::RefCell;

    let mut doc = document_with_urls(&["__blob:0", "__blob:0", "__hash:used"]);
    let mut blobs = HashMap::new();
    blobs.insert(0, png_bytes());
    let mut unused_blob = vec![0; 1024 * 1024];
    unused_blob[..4].copy_from_slice(&[0x89, 0x50, 0x4e, 0x47]);
    blobs.insert(1, unused_blob);
    let mut files = HashMap::new();
    files.insert("used".to_string(), png_bytes());
    files.insert("unused".to_string(), vec![0xFF; 1024 * 1024]);

    let transformed_sizes = RefCell::new(Vec::new());
    let transform = |bytes: &[u8]| {
        transformed_sizes.borrow_mut().push(bytes.len());
        None
    };
    let resolved = resolve_image_blobs_owned_with(&mut doc, blobs, files, Some(&transform));

    assert_eq!(resolved, 3, "both shared fills and the hash fill resolve");
    assert_eq!(
        transformed_sizes.into_inner(),
        vec![png_bytes().len(), png_bytes().len()],
        "the shared payload is transformed once and unused 1 MiB payloads are skipped"
    );
    let PenNode::Rectangle(first) = &doc.children[0] else {
        panic!("expected rectangle")
    };
    let PenNode::Rectangle(second) = &doc.children[1] else {
        panic!("expected rectangle")
    };
    let PenFill::Image(first) = &first.container.fill.as_ref().unwrap()[0] else {
        panic!("expected image fill")
    };
    let PenFill::Image(second) = &second.container.fill.as_ref().unwrap()[0] else {
        panic!("expected image fill")
    };
    assert!(Arc::ptr_eq(&first.url.as_arc(), &second.url.as_arc()));
}

#[test]
fn owned_resolver_retains_and_patches_image_stroke_payloads() {
    let mut doc: PenDocument = serde_json::from_value(json!({
        "version": "1",
        "children": [{
            "type": "line",
            "id": "image-stroke",
            "stroke": {
                "thickness": 1,
                "fill": [{"type": "image", "url": "__blob:7"}]
            }
        }]
    }))
    .expect("image stroke document is valid");
    let blobs = HashMap::from([(7, png_bytes()), (8, vec![0xFF; 1024])]);

    assert_eq!(
        resolve_image_blobs_owned_with(&mut doc, blobs, HashMap::new(), None),
        1
    );
    let PenNode::Line(line) = &doc.children[0] else {
        panic!("expected line")
    };
    let PenFill::Image(image) = &line.stroke.as_ref().unwrap().fill.as_ref().unwrap()[0] else {
        panic!("expected image stroke")
    };
    assert!(image.url.starts_with("data:image/png;base64,"));
}
