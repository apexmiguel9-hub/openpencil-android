// Document parsing and page snapshot accessors for Viewer.
use crate::error::ViewerLoadError;
use crate::Viewer;
use wasm_bindgen::prelude::wasm_bindgen;

impl Viewer {
    /// Parse a canonical `.op` JSON string (best-effort, legacy-tolerant).
    /// Immediately rebuilds the cached `LayoutScene` from the loaded document.
    /// Internal Rust API; the JS-facing wrapper is `Viewer::load_str` (maps the
    /// error to a `JsValue` so it surfaces as a thrown exception).
    pub fn load(&mut self, src: &str) -> Result<(), ViewerLoadError> {
        let editor_meta = op_pen_loader::extract_editor_meta(src);
        let loaded =
            op_pen_loader::load_canonical(src).map_err(|e| ViewerLoadError(format!("{e:?}")))?;
        let doc = loaded.value;
        let page_count = doc
            .pages
            .as_ref()
            .map(|pages| pages.len())
            .unwrap_or(1)
            .max(1);
        self.active_page = editor_meta
            .as_ref()
            .map(|meta| meta.active_page_index.min(page_count - 1))
            .unwrap_or_else(|| {
                doc.pages
                    .as_ref()
                    .and_then(|pages| pages.iter().position(|page| !page.children.is_empty()))
                    .unwrap_or(0)
            });
        self.preserve_authored_geometry = editor_meta
            .as_ref()
            .map(|meta| meta.preserve_authored_geometry)
            .unwrap_or(false);
        self.doc = Some(doc);
        self.rebuild_scene();
        Ok(())
    }
}

/// Read-only page accessors. Exported to JS so consumers (and the smoke page)
/// can query the loaded document without a full snapshot round-trip.
#[wasm_bindgen]
impl Viewer {
    /// Number of pages in the loaded document.
    /// Returns 0 when no document is loaded; at least 1 otherwise.
    pub fn page_count(&self) -> usize {
        self.doc
            .as_ref()
            .map(|d| d.pages.as_ref().map(|p| p.len()).unwrap_or(1).max(1))
            .unwrap_or(0)
    }

    /// Index of the currently active (visible) page (0-based).
    pub fn active_page_index(&self) -> usize {
        self.active_page
    }
}
