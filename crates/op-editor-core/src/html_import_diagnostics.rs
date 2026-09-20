//! Post-import diagnostics for HTML pages and web projects.
//!
//! The importer (`op-html`) reports every degradation as a typed
//! `ImportWarning`. This module is the platform-free carrier for those
//! reports: it holds the stable machine code, the locale key, the structured
//! arguments the localized template interpolates, and the importer's own
//! English sentence as the last-resort fallback.
//!
//! It deliberately does NOT depend on `op-html` — the browser host builds the
//! same rows out of a worker's JSON payload, and the editor core stays free of
//! the importer's dependency tree.

/// One reported degradation from an HTML import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlImportDiagnostic {
    /// Stable machine identifier, e.g. `layout.flex_wrap_node_limit`.
    pub code: String,
    /// Locale-table key holding the panel copy.
    pub i18n_key: String,
    /// Structured fields for placeholder substitution in the localized copy.
    pub args: Vec<(String, String)>,
    /// The importer's English sentence. Shown when the locale table has no
    /// entry for `i18n_key`, so a brand-new warning code is still readable.
    pub message: String,
}

impl HtmlImportDiagnostic {
    /// Build a row from the four parts every host already has to hand.
    pub fn new(
        code: impl Into<String>,
        i18n_key: impl Into<String>,
        args: Vec<(String, String)>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            i18n_key: i18n_key.into(),
            args,
            message: message.into(),
        }
    }

    /// Borrowed view of `args` in the shape `op_i18n::translate_with` wants.
    pub fn arg_pairs(&self) -> Vec<(&str, &str)> {
        self.args
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect()
    }
}

/// Build overlay rows from an importer's carrier-shaped parts, each a
/// `(code, i18n_key, args, message)` tuple.
///
/// This is the receiving half of `op_html::diagnostic_parts`. The split exists
/// because this module must not depend on `op-html` (see the module docs), so
/// the importer owns "which fields a warning contributes" and this owns "what
/// a row is made of". Both hosts call
/// `rows_from_parts(op_html::diagnostic_parts(&warnings))` instead of each
/// carrying its own copy of the conversion.
pub fn rows_from_parts<I>(parts: I) -> Vec<HtmlImportDiagnostic>
where
    I: IntoIterator<Item = (String, String, Vec<(String, String)>, String)>,
{
    parts
        .into_iter()
        .map(|(code, i18n_key, args, message)| HtmlImportDiagnostic {
            code,
            i18n_key,
            args,
            message,
        })
        .collect()
}

/// Hoverable controls of the diagnostics overlay — cursor-move writes it,
/// paint tints from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HtmlImportDiagnosticsHover {
    /// The show-details / hide-details toggle.
    Toggle,
    /// The dismiss button.
    Dismiss,
}

/// Upper bound on stored rows.
///
/// The importer de-duplicates by message, so a pathological page tops out in
/// the low hundreds; the cap keeps a runaway document from parking an
/// unbounded list on `EditorUiState` for the rest of the session. The overlay
/// still reports the true total through [`HtmlImportDiagnosticsSummary`].
pub const MAX_DIAGNOSTIC_ROWS: usize = 200;

/// What the overlay header reports: how many degradations happened, and how
/// many of them survived the [`MAX_DIAGNOSTIC_ROWS`] cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HtmlImportDiagnosticsSummary {
    /// Rows actually retained.
    pub shown: usize,
    /// Degradations the importer reported, including any dropped by the cap.
    pub total: usize,
}

impl HtmlImportDiagnosticsSummary {
    /// Rows the cap discarded.
    pub fn hidden(&self) -> usize {
        self.total.saturating_sub(self.shown)
    }
}

/// Trim `rows` to [`MAX_DIAGNOSTIC_ROWS`] and report what was kept.
pub fn cap_rows(rows: &mut Vec<HtmlImportDiagnostic>) -> HtmlImportDiagnosticsSummary {
    let total = rows.len();
    rows.truncate(MAX_DIAGNOSTIC_ROWS);
    HtmlImportDiagnosticsSummary {
        shown: rows.len(),
        total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize) -> HtmlImportDiagnostic {
        HtmlImportDiagnostic::new(
            format!("layout.row_{index}"),
            format!("htmlImport.warn.layout.row_{index}"),
            vec![("value".to_string(), index.to_string())],
            format!("row {index}"),
        )
    }

    #[test]
    fn arg_pairs_borrows_without_reordering() {
        let diagnostic = HtmlImportDiagnostic::new(
            "layout.float_ignored",
            "htmlImport.warn.layout.float_ignored",
            vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ],
            "CSS float ignored during structured HTML import",
        );
        assert_eq!(diagnostic.arg_pairs(), vec![("a", "1"), ("b", "2")]);
    }

    #[test]
    fn capping_reports_the_true_total() {
        let mut rows: Vec<_> = (0..MAX_DIAGNOSTIC_ROWS + 5).map(row).collect();
        let summary = cap_rows(&mut rows);
        assert_eq!(rows.len(), MAX_DIAGNOSTIC_ROWS);
        assert_eq!(summary.shown, MAX_DIAGNOSTIC_ROWS);
        assert_eq!(summary.total, MAX_DIAGNOSTIC_ROWS + 5);
        assert_eq!(summary.hidden(), 5);
    }

    #[test]
    fn rows_are_built_from_importer_parts_without_reordering() {
        let rows = rows_from_parts(vec![
            (
                "layout.float_ignored".to_string(),
                "htmlImport.warn.layout.float_ignored".to_string(),
                Vec::new(),
                "CSS float ignored".to_string(),
            ),
            (
                "list.style_type_unsupported".to_string(),
                "htmlImport.warn.list.style_type_unsupported".to_string(),
                vec![("value".to_string(), "hebrew".to_string())],
                "unsupported list-style-type".to_string(),
            ),
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].code, "layout.float_ignored");
        assert_eq!(rows[1].arg_pairs(), vec![("value", "hebrew")]);
        assert_eq!(rows[1].message, "unsupported list-style-type");
    }

    #[test]
    fn a_short_list_is_kept_whole() {
        let mut rows: Vec<_> = (0..3).map(row).collect();
        let summary = cap_rows(&mut rows);
        assert_eq!(rows.len(), 3);
        assert_eq!(summary.hidden(), 0);
    }
}
