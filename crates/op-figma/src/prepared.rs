//! Two-stage binary Figma import. Preparation performs the expensive
//! decode, style resolution, and owned tree build once; callers can
//! inspect lightweight page metadata before consuming the prepared
//! file into either one page or the full document.

use crate::common::FigLayoutMode;
use crate::figma_types::{parse_fig_file, BlobOrString, FigmaDecodedFile};
use crate::image_resolver::{resolve_image_blobs_owned_with, ImageTransform};
use crate::node_mapper::{
    convert_tree_all_pages, convert_tree_page, empty_import_result, resolve_style_references,
    FigmaImportResult, FigmaPageInfo,
};
use crate::tree::{build_tree_owned, guid_to_string, is_user_page, TreeNode};
use crate::{detect_kind, FigFileKind, FigImport, FigParseError};
use std::collections::HashMap;

/// A decoded and indexed `.fig` file awaiting page conversion.
///
/// [`pages`](Self::pages) returns a borrowed metadata slice; it does
/// not rebuild or clone the document tree. Conversion consumes `self`
/// because the geometry and image blob pools are moved into the result.
#[derive(Debug)]
pub struct PreparedFig {
    file_name: String,
    layout_mode: FigLayoutMode,
    tree: Option<TreeNode>,
    pages: Vec<FigmaPageInfo>,
    blobs: Vec<BlobOrString>,
    image_files: HashMap<String, Vec<u8>>,
}

/// Decode and prepare a binary `.fig` for page discovery and deferred
/// conversion.
pub fn prepare_fig_binary(
    bytes: &[u8],
    file_name: &str,
    layout_mode: FigLayoutMode,
) -> Result<PreparedFig, FigParseError> {
    if detect_kind(bytes) != FigFileKind::Binary {
        return Err(FigParseError::UnknownFormat);
    }
    let decoded = parse_fig_file(bytes).map_err(|e| FigParseError::Binary(e.to_string()))?;
    Ok(PreparedFig::from_decoded(decoded, file_name, layout_mode))
}

impl PreparedFig {
    fn from_decoded(
        mut decoded: FigmaDecodedFile,
        file_name: &str,
        layout_mode: FigLayoutMode,
    ) -> Self {
        resolve_style_references(&mut decoded.node_changes);
        let tree = build_tree_owned(std::mem::take(&mut decoded.node_changes));
        let pages = tree
            .as_ref()
            .map(|tree| {
                tree.children
                    .iter()
                    .filter(|child| is_user_page(child))
                    .map(page_info)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            file_name: file_name.to_string(),
            layout_mode,
            tree,
            pages,
            blobs: decoded.blobs,
            image_files: decoded.image_files,
        }
    }

    /// User-visible pages in Figma page-panel order.
    pub fn pages(&self) -> &[FigmaPageInfo] {
        &self.pages
    }

    /// Convert one page without applying a host image transform.
    pub fn into_page(self, page_index: usize) -> Result<FigImport, FigParseError> {
        self.into_page_with_images(page_index, None)
    }

    /// Convert one page, transforming each referenced image at most
    /// once before embedding it as a data URL.
    pub fn into_page_with_images(
        self,
        page_index: usize,
        image_transform: Option<&ImageTransform<'_>>,
    ) -> Result<FigImport, FigParseError> {
        let page_count = self.pages.len();
        if page_index >= page_count {
            return Err(FigParseError::PageOutOfBounds {
                index: page_index,
                page_count,
            });
        }
        let Self {
            file_name,
            layout_mode,
            tree,
            blobs,
            image_files,
            ..
        } = self;
        let tree = tree.expect("prepared file with pages must have a document tree");
        let result = convert_tree_page(&tree, blobs, &file_name, page_index, layout_mode);
        // Conversion owns its output; release the decoded source tree
        // before image transforms/base64 can allocate another large
        // document-sized payload.
        drop(tree);
        Ok(resolve_images(result, image_files, image_transform))
    }

    /// Convert every user-visible page without a host image transform.
    pub fn into_all_pages(self) -> Result<FigImport, FigParseError> {
        self.into_all_pages_with_images(None)
    }

    /// Convert every user-visible page, transforming each referenced
    /// image at most once before embedding it as a data URL.
    pub fn into_all_pages_with_images(
        self,
        image_transform: Option<&ImageTransform<'_>>,
    ) -> Result<FigImport, FigParseError> {
        let Self {
            file_name,
            layout_mode,
            tree,
            blobs,
            image_files,
            ..
        } = self;
        let result = match tree {
            Some(tree) => {
                let result = convert_tree_all_pages(&tree, blobs, &file_name, layout_mode);
                drop(tree);
                result
            }
            None => empty_import_result(&file_name, "No document root found", blobs),
        };
        Ok(resolve_images(result, image_files, image_transform))
    }
}

fn page_info(page: &TreeNode) -> FigmaPageInfo {
    FigmaPageInfo {
        id: page
            .figma
            .get("guid")
            .and_then(guid_to_string)
            .unwrap_or_default(),
        name: page.figma.get_str("name").unwrap_or("Page").to_string(),
        child_count: page.children.len(),
    }
}

fn resolve_images(
    result: FigmaImportResult,
    image_files: HashMap<String, Vec<u8>>,
    image_transform: Option<&ImageTransform<'_>>,
) -> FigImport {
    let FigmaImportResult {
        mut document,
        warnings,
        image_blobs,
    } = result;
    resolve_image_blobs_owned_with(&mut document, image_blobs, image_files, image_transform);
    FigImport { document, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiwi::FigValue;

    fn object(fields: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(
            fields
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        )
    }

    fn guid(local_id: u32) -> FigValue {
        object(vec![
            ("sessionID", FigValue::Uint(0)),
            ("localID", FigValue::Uint(local_id)),
        ])
    }

    fn node(local_id: u32, node_type: &str, name: &str, parent: Option<u32>) -> FigValue {
        let mut fields = vec![
            ("guid", guid(local_id)),
            ("type", FigValue::Str(node_type.to_string())),
            ("name", FigValue::Str(name.to_string())),
        ];
        if let Some(parent) = parent {
            fields.push((
                "parentIndex",
                object(vec![
                    ("guid", guid(parent)),
                    ("position", FigValue::Str(format!("{local_id:04}"))),
                ]),
            ));
        }
        object(fields)
    }

    fn decoded() -> FigmaDecodedFile {
        FigmaDecodedFile {
            node_changes: vec![
                node(1, "DOCUMENT", "Doc", None),
                node(2, "CANVAS", "First", Some(1)),
                node(3, "RECTANGLE", "Box", Some(2)),
                node(4, "CANVAS", "Internal Only Assets", Some(1)),
                node(5, "CANVAS", "Second", Some(1)),
            ],
            blobs: Vec::new(),
            image_files: HashMap::new(),
        }
    }

    #[test]
    fn pages_are_borrowed_metadata_in_user_page_order() {
        let prepared = PreparedFig::from_decoded(decoded(), "Test", FigLayoutMode::Preserve);
        assert_eq!(prepared.pages().len(), 2);
        assert_eq!(prepared.pages()[0].name, "First");
        assert_eq!(prepared.pages()[0].child_count, 1);
        assert_eq!(prepared.pages()[1].name, "Second");
    }

    #[test]
    fn into_page_is_strict_and_converts_only_the_selected_page() {
        let prepared = PreparedFig::from_decoded(decoded(), "Test", FigLayoutMode::Preserve);
        let import = prepared.into_page(1).expect("second page converts");
        let pages = import.document.pages.expect("document pages");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].id, "figma-page-1");
        assert_eq!(pages[0].name, "Second");
    }

    #[test]
    fn into_page_reports_requested_index_and_page_count() {
        let prepared = PreparedFig::from_decoded(decoded(), "Test", FigLayoutMode::Preserve);
        let error = prepared.into_page(2).expect_err("index 2 is out of range");
        assert!(matches!(
            error,
            FigParseError::PageOutOfBounds {
                index: 2,
                page_count: 2
            }
        ));
    }
}
