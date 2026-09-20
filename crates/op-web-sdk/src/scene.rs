// Paint-only LayoutScene builder for Viewer.
// Delegates to op-pen-loader, selecting either jian-core's taffy flex pass or
// the authored-geometry path embedded by Preserve-mode Figma imports.
use crate::Viewer;
use std::collections::BTreeMap;

impl Viewer {
    /// Rebuild the paint-only scene for the active page from the loaded doc.
    /// Clears the scene if no document is loaded.
    pub fn rebuild_scene(&mut self) {
        let Some(doc) = self.doc.as_ref() else {
            self.scene = None;
            return;
        };
        // v1: pass an empty active-theme map (default theme axis).
        let active_theme: BTreeMap<String, String> = BTreeMap::new();
        self.scene = Some(
            op_pen_loader::pen_document_to_layout_scene_with_geometry_mode(
                doc,
                &active_theme,
                self.active_page,
                self.preserve_authored_geometry,
            ),
        );
    }

    /// Return a reference to the cached layout scene, or `None` if no
    /// document has been loaded or `rebuild_scene` has not been called.
    pub fn scene(&self) -> Option<&op_editor_ui::layout_scene::LayoutScene> {
        self.scene.as_ref()
    }
}
