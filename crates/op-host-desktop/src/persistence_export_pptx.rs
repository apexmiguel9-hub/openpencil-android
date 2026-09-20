//! Desktop flow for File ▸ "Export PowerPoint".
//!
//! One save picker, then one `.pptx` holding the whole deck as editable
//! slides. The package and the DrawingML both live in
//! `op_host_services::export_pptx`; this file owns only the dialogs.

use op_host_native::WidgetHostNative;
use op_host_services::export_pptx::export_deck_pptx;
use op_i18n::Locale;
use std::path::Path;

/// Default file name offered by the save picker.
const DEFAULT_FILE_NAME: &str = "openpencil-deck.pptx";

/// Run the whole flow: save picker → export → report.
pub fn handle_export_pptx(host: &mut WidgetHostNative) {
    let locale = host.editor_state().editor_ui.locale;
    let Some(path) = rfd::FileDialog::new()
        .set_title(op_i18n::translate(locale, "dialog.pptxTitle"))
        .add_filter("PowerPoint", &["pptx"])
        .set_file_name(DEFAULT_FILE_NAME)
        .save_file()
    else {
        return;
    };

    match export_deck_pptx(host.editor_state(), &path) {
        // The dialog reports slides only. The structured / rastered node
        // split is diagnostic detail; the user wants to know the deck
        // landed and where.
        Ok(export) => info_dialog(
            op_i18n::translate(locale, "dialog.pptxTitle"),
            summary_body(locale, &path, export.slides),
            rfd::MessageLevel::Info,
        ),
        Err(e) => {
            eprintln!("[export-pptx] {e}");
            info_dialog(
                op_i18n::translate(locale, "dialog.pptxTitle"),
                failure_body(locale, &e),
                rfd::MessageLevel::Warning,
            );
        }
    }
}

/// Body of the success dialog: how many slides landed and where.
fn summary_body(locale: Locale, path: &Path, slides: usize) -> String {
    let mut body =
        op_i18n::translate(locale, "dialog.pptxSummary").replace("{{count}}", &slides.to_string());
    body.push_str("\n\n");
    body.push_str(&path.display().to_string());
    body
}

/// Body of the failure dialog. An empty deck gets the sentence written
/// for it; every other failure reports the exporter's own message, which
/// names the node that could not be rendered.
fn failure_body(locale: Locale, error: &op_host_services::export::ExportError) -> String {
    if matches!(
        error,
        op_host_services::export::ExportError::NothingToExport
    ) {
        return op_i18n::translate(locale, "dialog.pptxEmpty").to_string();
    }
    let mut body = op_i18n::translate(locale, "dialog.exportErrorLead").to_string();
    body.push_str("\n\n");
    body.push_str(&error.to_string());
    body
}

/// Pop the same style of native dialog `persistence::show_error_dialog`
/// uses, at a caller-chosen level (the happy path is not an error).
fn info_dialog(title: &str, body: String, level: rfd::MessageLevel) {
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(&body)
        .set_level(level)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_host_services::export::ExportError;

    #[test]
    fn a_clean_run_reports_the_count_and_the_file() {
        let body = summary_body(Locale::EnUs, Path::new("/tmp/deck.pptx"), 6);
        assert!(body.starts_with("Exported 6 slides to:"), "{body}");
        assert!(body.contains("/tmp/deck.pptx"), "{body}");
    }

    #[test]
    fn the_summary_is_localised() {
        let body = summary_body(Locale::ZhCn, Path::new("/tmp/deck.pptx"), 3);
        assert!(body.contains("已导出 3 张幻灯片到："), "{body}");
    }

    #[test]
    fn an_empty_deck_gets_its_own_sentence_rather_than_the_raw_error() {
        let body = failure_body(Locale::EnUs, &ExportError::NothingToExport);
        assert_eq!(body, "This deck has no visible slides to export.");
        assert!(!body.contains("nothing to export"), "{body}");
    }

    #[test]
    fn other_failures_name_the_node_that_could_not_render() {
        let body = failure_body(
            Locale::EnUs,
            &ExportError::NodePaintsNothing {
                node_id: "slide-4".to_string(),
            },
        );
        assert!(body.contains("slide-4 paints nothing"), "{body}");
    }
}
