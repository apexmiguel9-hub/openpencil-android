//! Desktop flow for File ▸ "Export all frames".
//!
//! One directory picker, then one PNG per top-level frame of the active
//! page (or per selected frame when the selection covers two or more).
//! The render itself is the untouched single-frame exporter — this file
//! only owns the dialogs and the summary report; see
//! `op_host_services::export_batch` for the loop and
//! `op_editor_core::export_batch` for the frame plan.

use op_editor_core::export_batch::plan_frame_exports;
use op_host_native::WidgetHostNative;
use op_host_services::export_batch::{export_frames_to_dir, BatchExportReport, BATCH_EXTENSION};
use op_i18n::Locale;
use std::path::Path;

/// Cap on failure lines echoed into the summary dialog — a document
/// where everything failed would otherwise produce a dialog taller than
/// the screen. The elided count is still reported.
const MAX_FAILURE_LINES: usize = 8;

/// Run the whole flow: plan → directory picker → export → report.
pub fn handle_export_all_frames(host: &mut WidgetHostNative) {
    let locale = host.editor_state().editor_ui.locale;
    let targets = plan_frame_exports(host.editor_state(), BATCH_EXTENSION);
    if targets.is_empty() {
        info_dialog(
            op_i18n::translate(locale, "dialog.batchExportTitle"),
            op_i18n::translate(locale, "dialog.batchExportEmpty").to_string(),
            rfd::MessageLevel::Warning,
        );
        return;
    }
    let Some(directory) = rfd::FileDialog::new()
        .set_title(op_i18n::translate(locale, "dialog.pickerExportFolderTitle"))
        .pick_folder()
    else {
        return;
    };

    let report = export_frames_to_dir(host.editor_state(), &directory, &targets);

    for (name, detail) in &report.failed {
        eprintln!("[export-all-frames] {name}: {detail}");
    }
    info_dialog(
        op_i18n::translate(locale, "dialog.batchExportTitle"),
        summary_body(locale, &directory, &report),
        if report.all_written() {
            rfd::MessageLevel::Info
        } else {
            rfd::MessageLevel::Warning
        },
    );
}

/// Body of the summary dialog: how many frames landed, where, and —
/// when some failed — which ones and why.
fn summary_body(locale: Locale, directory: &Path, report: &BatchExportReport) -> String {
    let mut body = op_i18n::translate(locale, "dialog.batchExportSummary")
        .replace("{{ok}}", &report.written.len().to_string())
        .replace("{{total}}", &report.attempted().to_string());
    body.push_str("\n\n");
    body.push_str(&directory.display().to_string());
    if report.failed.is_empty() {
        return body;
    }
    body.push_str("\n\n");
    body.push_str(op_i18n::translate(locale, "dialog.batchExportFailedLead"));
    for (name, detail) in report.failed.iter().take(MAX_FAILURE_LINES) {
        body.push_str(&format!("\n{name} — {detail}"));
    }
    if report.failed.len() > MAX_FAILURE_LINES {
        body.push_str(&format!("\n… +{}", report.failed.len() - MAX_FAILURE_LINES));
    }
    body
}

/// Pop the same style of native dialog `persistence::show_error_dialog`
/// uses, at a caller-chosen level (the happy path is not an error).
fn info_dialog(title: &str, body: String, level: rfd::MessageLevel) {
    crate::message_dialog::alert(title, &body, level);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(written: &[&str], failed: &[(&str, &str)]) -> BatchExportReport {
        BatchExportReport {
            written: written.iter().map(|s| s.to_string()).collect(),
            failed: failed
                .iter()
                .map(|(n, d)| (n.to_string(), d.to_string()))
                .collect(),
        }
    }

    #[test]
    fn a_clean_run_reports_the_count_and_directory_only() {
        let body = summary_body(
            Locale::EnUs,
            Path::new("/tmp/deck"),
            &report(&["01-a.png", "02-b.png"], &[]),
        );
        assert!(body.starts_with("Exported 2 of 2 frames to:"), "{body}");
        assert!(body.contains("/tmp/deck"));
        assert!(!body.contains("could not be exported"));
    }

    #[test]
    fn failures_are_listed_after_the_summary() {
        let body = summary_body(
            Locale::EnUs,
            Path::new("/tmp/deck"),
            &report(&["01-a.png"], &[("02-b.png", "node n2 paints nothing")]),
        );
        assert!(body.contains("Exported 1 of 2 frames to:"), "{body}");
        assert!(body.contains("02-b.png — node n2 paints nothing"), "{body}");
    }

    #[test]
    fn a_long_failure_list_is_elided_but_still_counted() {
        let failed: Vec<(String, String)> = (0..12)
            .map(|i| (format!("{i:02}-x.png"), "boom".to_string()))
            .collect();
        let body = summary_body(
            Locale::EnUs,
            Path::new("/tmp/deck"),
            &BatchExportReport {
                written: vec![],
                failed,
            },
        );
        assert_eq!(body.matches("boom").count(), MAX_FAILURE_LINES);
        assert!(body.contains("… +4"), "{body}");
    }

    #[test]
    fn the_summary_is_localised() {
        let body = summary_body(
            Locale::ZhCn,
            Path::new("/tmp/deck"),
            &report(&["01-a.png"], &[]),
        );
        assert!(body.contains("已导出 1/1 个画框到："), "{body}");
    }
}
