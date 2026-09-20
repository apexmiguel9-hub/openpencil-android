use super::load_report::is_strict_format_version;
use super::{sidecar_path, DocIoError, DocumentLoadReport, LoadedEditorState};
use op_editor_core::EditorState;
use op_pen_loader::{apply_editor_meta, extract_editor_meta_with_report, EditorMeta};

pub fn load_editor_state(
    path: &std::path::Path,
    locale: op_editor_core::Locale,
) -> Result<EditorState, DocIoError> {
    load_editor_state_with_report(path, locale).map(|loaded| loaded.state)
}

thread_local! {
    /// Set for the duration of a load that must not publish thumbnails.
    ///
    /// A thread-local rather than a parameter because the seam is four calls
    /// deep through `op_pen_loader`, and threading a flag through a loader
    /// this pass does not own would be a much larger change for the same
    /// effect. The scope is one synchronous load on one thread.
    static SKIP_THUMBNAILS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn skip_thumbnails() -> bool {
    SKIP_THUMBNAILS.with(std::cell::Cell::get)
}

/// Load a document WITHOUT publishing its thumbnails to the process-global
/// registry.
///
/// For the multi-tenant online daemon: that registry has no tenant dimension
/// and a load replaces it wholesale, so restoring one account's document would
/// drop every other account's thumbnails. Online policy disables thumbnails
/// anyway; this keeps the restore path from reintroducing them.
pub fn load_editor_state_without_thumbnails(
    path: &std::path::Path,
    locale: op_editor_core::Locale,
) -> Result<EditorState, DocIoError> {
    SKIP_THUMBNAILS.with(|flag| flag.set(true));
    let outcome = load_editor_state_with_report(path, locale).map(|loaded| loaded.state);
    SKIP_THUMBNAILS.with(|flag| flag.set(false));
    outcome
}

pub fn load_editor_state_with_report(
    path: &std::path::Path,
    locale: op_editor_core::Locale,
) -> Result<LoadedEditorState, DocIoError> {
    let file = std::fs::File::open(path).map_err(|e| DocIoError::Io(e.to_string()))?;
    let file_len = file
        .metadata()
        .map_err(|e| DocIoError::Io(e.to_string()))?
        .len();
    if file_len == 0 {
        let (state, embedded_meta, report) = parse_editor_state_source("", locale)?;
        return Ok(finish_loaded_state_with_report(
            state,
            embedded_meta,
            report,
            path,
        ));
    }
    // SAFETY: The map is read-only and lives through this synchronous parse.
    // OpenPencil atomically replaces its own files rather than truncating the
    // mapped inode in place.
    let bytes = unsafe { memmap2::MmapOptions::new().map(&file) }
        .map_err(|e| DocIoError::Io(e.to_string()))?;
    let src = std::str::from_utf8(bytes.as_ref()).map_err(|error| {
        // `op_i18n` owns this sentence; it is end-user copy, so it is carried
        // through verbatim with its placeholder already substituted.
        DocIoError::InvalidUtf8Document(
            op_i18n::translate(locale, "dialog.loadErrorInvalidUtf8")
                .replace("{{detail}}", &error.to_string()),
        )
    })?;
    let (state, embedded_meta, report) = parse_editor_state_source(src, locale)?;
    Ok(finish_loaded_state_with_report(
        state,
        embedded_meta,
        report,
        path,
    ))
}

pub fn load_editor_state_from_source(
    src: &str,
    locale: op_editor_core::Locale,
) -> Result<EditorState, DocIoError> {
    let (state, embedded_meta, _) = parse_editor_state_source(src, locale)?;
    Ok(finish_loaded_state(state, embedded_meta))
}

fn parse_editor_state_source(
    src: &str,
    locale: op_editor_core::Locale,
) -> Result<(EditorState, Option<EditorMeta>, DocumentLoadReport), DocIoError> {
    let embedded = extract_editor_meta_with_report(src);
    let canonical = match op_pen_loader::payload::load_canonical_with_compatibility(src) {
        Ok(loaded) => loaded,
        Err(error) => {
            if looks_like_legacy_doc_payload(src) {
                return Err(DocIoError::LegacyFormat(
                    op_i18n::translate(locale, "dialog.loadErrorOldVersion").to_string(),
                ));
            }
            // `op_pen_loader` is not owned by this pass — carry its schema
            // rejection verbatim.
            return Err(DocIoError::Schema(error.to_string()));
        }
    };
    let loaded = canonical.loaded;
    if skip_thumbnails() {
        // Drop the pending thumbnail seed BEFORE `EditorState::from_document`
        // consumes the document, because that is what activates it — and
        // activation REPLACES the process-global registry wholesale. In a
        // shared process, restoring one account's tenant would therefore
        // discard every other account's thumbnails and rebind their ids to
        // these bytes. See `web_canvas_server::online_policy`.
        jian_ops_schema::image_thumbs::discard_for_document(&loaded.value);
    }
    for warning in &loaded.warnings {
        eprintln!("[open] schema warning: {warning:?}");
    }
    let malformed_format_version = loaded
        .value
        .format_version
        .as_deref()
        .is_some_and(|version| !is_strict_format_version(version));
    let rewrite_blocked_by_schema_warning = malformed_format_version
        || loaded.warnings.iter().any(|warning| {
            matches!(
                warning,
                jian_ops_schema::LoadWarning::FutureFormatVersion { .. }
            )
        });
    let report = DocumentLoadReport {
        normalized_legacy: canonical.compatibility.normalized_legacy,
        patched_legacy_version: canonical.compatibility.patched_legacy_version,
        inferred_editor_meta: embedded
            .as_ref()
            .is_some_and(|extraction| extraction.inferred_preserve_authored_geometry),
        used_legacy_sidecar: false,
        rewrite_blocked_by_schema_warning,
    };
    Ok((
        EditorState::from_document(loaded.value),
        embedded.map(|extraction| extraction.meta),
        report,
    ))
}

fn finish_loaded_state_with_report(
    state: EditorState,
    embedded_meta: Option<EditorMeta>,
    mut report: DocumentLoadReport,
    path: &std::path::Path,
) -> LoadedEditorState {
    let sidecar_meta = embedded_meta
        .is_none()
        .then(|| legacy_sidecar_meta(path))
        .flatten();
    report.used_legacy_sidecar = sidecar_meta.is_some();
    LoadedEditorState {
        state: finish_loaded_state(state, embedded_meta.or(sidecar_meta)),
        report,
    }
}

fn finish_loaded_state(mut state: EditorState, meta: Option<EditorMeta>) -> EditorState {
    if let Some(meta) = meta {
        apply_editor_meta(&mut state, meta);
    } else {
        state.ui.active_page_index = first_page_with_content(&state);
    }
    collapse_top_level_layers(&mut state);
    state
}

/// Start every top-level frame collapsed in the LayerPanel.
///
/// A fully expanded tree is unreadable the moment a document has depth: a
/// six-slide deck opens as ~90 rows and the boards themselves scroll off the
/// panel, so the one thing the list is for — seeing what the document
/// contains — is the first thing lost. Collapsed roots show the six slides
/// and let the user open the one they want.
///
/// `collapsed_layers` is view-only state: not serialized, not in the undo
/// snapshot, and toggling it pushes no history. So this changes what the
/// panel shows on open and nothing about the document.
fn collapse_top_level_layers(state: &mut EditorState) {
    let ids: Vec<op_editor_core::NodeId> = state
        .active_children()
        .iter()
        .filter(|node| {
            // Only containers collapse; a leaf has nothing to hide and would
            // just render a dead disclosure arrow.
            op_editor_core::PenNodeExt::children(*node).is_some_and(|kids| !kids.is_empty())
        })
        .map(|node| op_editor_core::NodeId::new(op_editor_core::PenNodeExt::id_str(node)))
        .collect();
    state.editor_ui.collapsed_layers.extend(ids);
}

fn first_page_with_content(state: &EditorState) -> usize {
    let Some(pages) = state.doc.pages.as_ref() else {
        return 0;
    };
    if pages.first().is_some_and(|page| !page.children.is_empty()) {
        return 0;
    }
    pages
        .iter()
        .position(|page| !page.children.is_empty())
        .unwrap_or(0)
}

fn legacy_sidecar_meta(path: &std::path::Path) -> Option<EditorMeta> {
    let meta_src = std::fs::read_to_string(sidecar_path(path)).ok()?;
    serde_json::from_str::<EditorMeta>(&meta_src).ok()
}

pub(super) fn looks_like_legacy_doc_payload(src: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(src) else {
        return false;
    };
    let Some(obj) = value.as_object() else {
        return false;
    };
    let version_is_integer = obj
        .get("version")
        .map(|version| version.is_u64() || version.is_i64())
        .unwrap_or(false);
    version_is_integer
        && obj
            .get("pages")
            .map(serde_json::Value::is_array)
            .unwrap_or(false)
}
