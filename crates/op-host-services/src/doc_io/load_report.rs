//! Compatibility report returned alongside a successfully loaded document.

/// Exact legacy repairs used while opening a document.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DocumentLoadReport {
    pub normalized_legacy: bool,
    pub patched_legacy_version: Option<String>,
    pub inferred_editor_meta: bool,
    pub used_legacy_sidecar: bool,
    /// The declared schema is malformed or newer than this build can write.
    pub rewrite_blocked_by_schema_warning: bool,
}

impl DocumentLoadReport {
    /// Whether a current-shape rewrite is both useful and known to be safe.
    pub fn needs_schema_upgrade(&self) -> bool {
        !self.rewrite_blocked_by_schema_warning
            && (self.normalized_legacy
                || self.patched_legacy_version.is_some()
                || self.inferred_editor_meta
                || self.used_legacy_sidecar)
    }
}

pub(super) fn is_strict_format_version(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let patch = parts.next();
    parts.next().is_none()
        && [Some(major), Some(minor), patch]
            .into_iter()
            .flatten()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

/// Editor state plus compatibility provenance from the successful load.
pub struct LoadedEditorState {
    pub state: op_editor_core::EditorState,
    pub report: DocumentLoadReport,
}
