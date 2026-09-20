//! Worker-side Figma conversion payload.
//!
//! The browser launches a second copy of the web WASM module inside a module
//! Worker and constructs [`FigmaTempWriter`] there. Expensive Kiwi decoding and
//! Figma-to-OP conversion therefore grow the Worker's linear memory, which is
//! released when the Worker terminates instead of becoming the editor's
//! permanent WASM high-water mark.
//!
//! The writer owns the canonical document as an externalized JSON value. That
//! form keeps one shared `images` table (and `imageThumbs`) while page records
//! contain only `op-image:*` references. No public `.op` schema changes are
//! involved: [`full_document_json`](FigmaTempWriter::full_document_json) is an
//! ordinary canonical `.op` document that the existing compatibility loader
//! consumes on the main thread.

use wasm_bindgen::prelude::*;

use serde::ser::{SerializeMap, SerializeSeq};
use serde::Serialize;

/// Borrowed document view used by the internal IndexedDB page archive.
///
/// Page metadata and document-global fields are retained, while node children
/// and the image tables are stored in their own records. This is deliberately
/// an internal storage shape, not a new public `.op` schema.
struct DocumentSkeleton<'a>(&'a serde_json::Value);

impl Serialize for DocumentSkeleton<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let Some(document) = self.0.as_object() else {
            return self.0.serialize(serializer);
        };
        let mut output = serializer.serialize_map(None)?;
        for (key, value) in document {
            match key.as_str() {
                "images" | "imageThumbs" => {}
                "children" => output.serialize_entry(key, &EmptyArray)?,
                "pages" => match value.as_array() {
                    Some(pages) => output.serialize_entry(key, &PageSkeletons(pages))?,
                    None => output.serialize_entry(key, value)?,
                },
                _ => output.serialize_entry(key, value)?,
            }
        }
        output.end()
    }
}

struct EmptyArray;

impl Serialize for EmptyArray {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_seq(Some(0))?.end()
    }
}

struct PageSkeletons<'a>(&'a [serde_json::Value]);

impl Serialize for PageSkeletons<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut pages = serializer.serialize_seq(Some(self.0.len()))?;
        for page in self.0 {
            match page.as_object() {
                Some(page) => pages.serialize_element(&ObjectWithoutChildren(page))?,
                None => pages.serialize_element(page)?,
            }
        }
        pages.end()
    }
}

struct ObjectWithoutChildren<'a>(&'a serde_json::Map<String, serde_json::Value>);

impl Serialize for ObjectWithoutChildren<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut output = serializer.serialize_map(None)?;
        for (key, value) in self.0 {
            if key != "children" {
                output.serialize_entry(key, value)?;
            }
        }
        output.end()
    }
}

struct ImageTables<'a>(&'a serde_json::Value);

impl Serialize for ImageTables<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut output = serializer.serialize_map(None)?;
        if let Some(document) = self.0.as_object() {
            for key in ["images", "imageThumbs"] {
                if let Some(value) = document.get(key) {
                    output.serialize_entry(key, value)?;
                }
            }
        }
        output.end()
    }
}

/// A Worker payload that could not be built or serialized.
///
/// `Display` reproduces the ad-hoc `String` messages this enum replaced byte
/// for byte; the JS side still receives them as plain `JsValue` strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigmaTempError {
    /// The converted Figma document could not be serialized to a JSON value.
    SerializeDocument(String),
    /// The requested page index is outside the converted document.
    PageOutOfRange { index: usize, count: usize },
    /// One of the JSON views could not be serialized.
    Serialize(String),
}

impl std::fmt::Display for FigmaTempError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigmaTempError::SerializeDocument(error) => write!(f, "{error}"),
            FigmaTempError::PageOutOfRange { index, count } => {
                write!(
                    f,
                    "Figma page index {index} is out of range (count {count})"
                )
            }
            FigmaTempError::Serialize(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for FigmaTempError {}

impl From<FigmaTempError> for String {
    fn from(error: FigmaTempError) -> String {
        error.to_string()
    }
}

/// Canonical JSON retained only inside the short-lived import Worker.
struct FigmaTempPayload {
    document: serde_json::Value,
    warnings: Vec<String>,
}

impl FigmaTempPayload {
    fn from_import(import: op_figma::FigImport) -> Result<Self, FigmaTempError> {
        let op_figma::FigImport { document, warnings } = import;
        let mut document = serde_json::to_value(document)
            .map_err(|error| FigmaTempError::SerializeDocument(error.to_string()))?;
        // This also snapshots the Worker's thumbnail registry into
        // `imageThumbs`, so loading the returned canonical JSON on the main
        // thread restores blur-up thumbnails through the normal loader path.
        jian_ops_schema::image_table::externalize_images(&mut document);
        Ok(Self { document, warnings })
    }

    fn pages(&self) -> Option<&Vec<serde_json::Value>> {
        self.document.get("pages")?.as_array()
    }

    fn page_count(&self) -> usize {
        self.pages().map_or(0, Vec::len)
    }

    fn page_json(&self, index: usize) -> Result<String, FigmaTempError> {
        let count = self.page_count();
        let page = self
            .pages()
            .and_then(|pages| pages.get(index))
            .ok_or(FigmaTempError::PageOutOfRange { index, count })?;
        serde_json::to_string(page).map_err(|error| FigmaTempError::Serialize(error.to_string()))
    }

    fn page_string_field(&self, index: usize, field: &str) -> Option<String> {
        self.pages()?
            .get(index)?
            .get(field)?
            .as_str()
            .map(ToOwned::to_owned)
    }

    fn full_document_json(&self) -> Result<String, FigmaTempError> {
        serde_json::to_string(&self.document)
            .map_err(|error| FigmaTempError::Serialize(error.to_string()))
    }

    fn skeleton_json(&self) -> Result<String, FigmaTempError> {
        serde_json::to_string(&DocumentSkeleton(&self.document))
            .map_err(|error| FigmaTempError::Serialize(error.to_string()))
    }

    fn image_tables_json(&self) -> Result<String, FigmaTempError> {
        serde_json::to_string(&ImageTables(&self.document))
            .map_err(|error| FigmaTempError::Serialize(error.to_string()))
    }

    fn warnings_json(&self) -> Result<String, FigmaTempError> {
        serde_json::to_string(&self.warnings)
            .map_err(|error| FigmaTempError::Serialize(error.to_string()))
    }
}

/// Incremental read surface used by `figma_temp_worker.js`.
///
/// Keeping the value behind a class avoids returning one giant JS object from
/// Rust. The Worker serializes and commits one page at a time, then serializes
/// the complete canonical document once for the current eager editor install.
#[wasm_bindgen]
pub struct FigmaTempWriter {
    payload: FigmaTempPayload,
}

#[wasm_bindgen]
impl FigmaTempWriter {
    /// Parse and convert a `.fig` byte stream in Preserve geometry mode.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8], file_name: &str) -> Result<FigmaTempWriter, JsValue> {
        let import =
            op_figma::parse_fig_binary(bytes, file_name, op_figma::FigLayoutMode::Preserve)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let payload = FigmaTempPayload::from_import(import)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self { payload })
    }

    pub fn page_count(&self) -> usize {
        self.payload.page_count()
    }

    pub fn page_id(&self, index: usize) -> Option<String> {
        self.payload.page_string_field(index, "id")
    }

    pub fn page_name(&self, index: usize) -> Option<String> {
        self.payload.page_string_field(index, "name")
    }

    /// A single canonical `PenPage` JSON record. Large images are references
    /// into the complete document's shared `images` table, not repeated data
    /// URLs.
    pub fn page_json(&self, index: usize) -> Result<String, JsValue> {
        self.payload
            .page_json(index)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Complete canonical `.op` JSON used by the existing eager install path.
    pub fn full_document_json(&self) -> Result<String, JsValue> {
        self.payload
            .full_document_json()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Document-global metadata plus page metadata, without page node trees or
    /// image payloads. This is an internal IndexedDB record used by the future
    /// page-residency layer; it is not exposed as a `.op` file.
    pub fn skeleton_json(&self) -> Result<String, JsValue> {
        self.payload
            .skeleton_json()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Shared `images` and `imageThumbs` tables referenced by page shards.
    pub fn image_tables_json(&self) -> Result<String, JsValue> {
        self.payload
            .image_tables_json()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn warnings_json(&self) -> Result<String, JsValue> {
        self.payload
            .warnings_json()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jian_ops_schema::PenDocument;

    fn two_page_image_document() -> (PenDocument, String) {
        let source = format!("data:image/png;base64,{}", "A".repeat(5_000));
        let json = serde_json::json!({
            "version": "1.0",
            "name": "Images",
            "pages": [
                {
                    "id": "p1",
                    "name": "One",
                    "children": [{"type": "image", "id": "i1", "src": source.clone()}]
                },
                {
                    "id": "p2",
                    "name": "Two",
                    "children": [{"type": "image", "id": "i2", "src": source.clone()}]
                }
            ],
            "children": []
        });
        (
            serde_json::from_value(json).expect("valid image document"),
            source,
        )
    }

    #[test]
    fn page_records_share_the_full_documents_image_table() {
        let (document, source) = two_page_image_document();
        let payload = FigmaTempPayload::from_import(op_figma::FigImport {
            document,
            warnings: vec!["best effort".into()],
        })
        .expect("payload");

        let full = payload.full_document_json().expect("full JSON");
        assert_eq!(full.matches(&source).count(), 1, "one shared image payload");
        assert!(full.contains("\"images\""));
        for index in 0..2 {
            let page = payload.page_json(index).expect("page JSON");
            assert!(page.contains("op-image:"));
            assert!(!page.contains(&source));
        }
    }

    #[test]
    fn complete_json_remains_a_legacy_compatible_canonical_document() {
        let (document, source) = two_page_image_document();
        let payload = FigmaTempPayload::from_import(op_figma::FigImport {
            document,
            warnings: Vec::new(),
        })
        .expect("payload");

        let loaded = op_pen_loader::load_canonical(
            &payload.full_document_json().expect("full document JSON"),
        )
        .expect("canonical loader accepts worker output");
        let pages = loaded.value.pages.expect("pages");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].children.len(), 1);
        assert_eq!(pages[1].children.len(), 1);
        let jian_ops_schema::node::PenNode::Image(first) = &pages[0].children[0] else {
            panic!("first page image expected");
        };
        let jian_ops_schema::node::PenNode::Image(second) = &pages[1].children[0] else {
            panic!("second page image expected");
        };
        assert_eq!(first.src.as_str(), source.as_str());
        assert_eq!(second.src.as_str(), source.as_str());
    }

    #[test]
    fn page_archive_separates_skeleton_and_image_tables() {
        let (document, source) = two_page_image_document();
        let payload = FigmaTempPayload::from_import(op_figma::FigImport {
            document,
            warnings: Vec::new(),
        })
        .expect("payload");

        let skeleton: serde_json::Value =
            serde_json::from_str(&payload.skeleton_json().expect("skeleton JSON"))
                .expect("valid skeleton");
        let pages = skeleton["pages"].as_array().expect("skeleton pages");
        assert_eq!(pages.len(), 2);
        assert!(pages.iter().all(|page| page.get("children").is_none()));
        assert!(skeleton.get("images").is_none());

        let images: serde_json::Value =
            serde_json::from_str(&payload.image_tables_json().expect("image tables JSON"))
                .expect("valid image tables");
        assert_eq!(
            images["images"].as_object().map(|table| table.len()),
            Some(1)
        );
        assert_eq!(images.to_string().matches(&source).count(), 1);
    }

    #[test]
    fn page_metadata_and_warnings_are_incrementally_readable() {
        let (document, _) = two_page_image_document();
        let payload = FigmaTempPayload::from_import(op_figma::FigImport {
            document,
            warnings: vec!["w1".into(), "w2".into()],
        })
        .expect("payload");
        assert_eq!(payload.page_count(), 2);
        assert_eq!(payload.page_string_field(1, "id").as_deref(), Some("p2"));
        assert_eq!(payload.page_string_field(1, "name").as_deref(), Some("Two"));
        assert_eq!(payload.warnings_json().unwrap(), r#"["w1","w2"]"#);
    }
}
