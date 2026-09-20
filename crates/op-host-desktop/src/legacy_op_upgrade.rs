//! Safe one-time rewrite flow for successfully loaded legacy `.op` files.
//!
//! The source is rewritten beside itself before the loaded state is installed
//! in the interactive host.

use crate::figma_import_session::{capture_output_state, OutputEntryState};
use crate::legacy_op_upgrade_error::LegacyUpgradeError;
use op_editor_core::EditorState;
use op_host_services::doc_io::{
    commit_staged_document, copy_document_to_current_schema_path, DocumentLoadReport,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpgradeDecision {
    ReplaceOriginal,
    NumberedCopy,
    Skip,
}

static UPGRADE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Prompt and, when approved, stream the normalized state in the current
/// schema. Returns the path the editor should bind as its current document.
pub(crate) fn prompt_and_save(
    state: &mut EditorState,
    source_path: &Path,
    report: &DocumentLoadReport,
    loaded_source_state: OutputEntryState,
) -> Result<PathBuf, LegacyUpgradeError> {
    if !is_op(source_path) || !report.needs_schema_upgrade() {
        if report.rewrite_blocked_by_schema_warning {
            eprintln!(
                "[open] skipped legacy rewrite prompt for {}: malformed or future schema version",
                source_path.display()
            );
        }
        return Ok(source_path.to_path_buf());
    }

    let locale = state.editor_ui.locale;
    let name = source_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| source_path.display().to_string());
    let body =
        op_i18n::translate(locale, "dialog.upgradeOpBody").replace("{{name}}", name.as_str());
    let decision = match crate::message_dialog::ask_yes_no_cancel(
        op_i18n::translate(locale, "dialog.upgradeOpTitle"),
        &body,
        rfd::MessageLevel::Info,
    ) {
        Some(crate::message_dialog::Choice::Yes) => UpgradeDecision::ReplaceOriginal,
        Some(crate::message_dialog::Choice::No) => UpgradeDecision::NumberedCopy,
        // Cancel — or no dialog backend (`None`): open the file as-is.
        _ => UpgradeDecision::Skip,
    };
    if decision == UpgradeDecision::Skip {
        return Ok(source_path.to_path_buf());
    }
    save_decision(state, source_path, report, decision, loaded_source_state)
}

fn save_decision(
    state: &mut EditorState,
    source_path: &Path,
    report: &DocumentLoadReport,
    decision: UpgradeDecision,
    loaded_source_state: OutputEntryState,
) -> Result<PathBuf, LegacyUpgradeError> {
    if capture_output_state(source_path)? != loaded_source_state {
        return Err(LegacyUpgradeError::SourceChangedWhileOpening {
            source_path: source_path.to_path_buf(),
        });
    }
    let staging_path = upgrade_staging_path(source_path)?;
    if let Err(error) = copy_document_to_current_schema_path(
        source_path,
        &staging_path,
        op_pen_loader::EditorMeta::from_state(state),
        report.normalized_legacy,
    ) {
        remove_file_if_present(&staging_path);
        // `copy_document_to_current_schema_path` reports `doc_io::DocIoError`;
        // this variant is a flat message because the staging-write failure is
        // reported to the user as one sentence, and `DocIoError`'s `Display`
        // already produces exactly it.
        return Err(LegacyUpgradeError::WriteStaged(error.to_string()));
    }
    if report.normalized_legacy {
        crate::heap_pressure::schedule_relief("legacy OP upgrade");
    }
    let target_path =
        match publish_staged_upgrade(&staging_path, source_path, decision, loaded_source_state) {
            Ok(path) => path,
            Err(error) => {
                remove_file_if_present(&staging_path);
                return Err(error);
            }
        };
    remove_file_if_present(&staging_path);
    state.doc.format_version = Some(jian_ops_schema::version::FORMAT_VERSION_CURRENT.to_string());
    if let Some(original_version) = report.patched_legacy_version.as_ref() {
        state.doc.version.clone_from(original_version);
    }
    eprintln!(
        "[open] rewrote legacy OP in current schema: {}",
        target_path.display()
    );
    Ok(target_path)
}

fn publish_staged_upgrade(
    staging_path: &Path,
    source_path: &Path,
    decision: UpgradeDecision,
    loaded_source_state: OutputEntryState,
) -> Result<PathBuf, LegacyUpgradeError> {
    if decision == UpgradeDecision::ReplaceOriginal {
        if capture_output_state(source_path)? == loaded_source_state {
            commit_staged_document(staging_path, source_path)
                .map_err(|error| LegacyUpgradeError::CommitOverOriginal(error.to_string()))?;
            return Ok(source_path.to_path_buf());
        }
        eprintln!(
            "[open] legacy OP changed while the current-format copy was written; preserving it"
        );
    }
    publish_numbered_copy(staging_path, source_path)
}

fn publish_numbered_copy(
    staging_path: &Path,
    source_path: &Path,
) -> Result<PathBuf, LegacyUpgradeError> {
    let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = source_path
        .file_stem()
        .map(|stem| stem.to_string_lossy())
        .unwrap_or_else(|| op_editor_ui::PRODUCT_NAME.into());
    for suffix in 1..=10_000 {
        let candidate = parent.join(format!("{stem} ({suffix}).op"));
        match std::fs::hard_link(staging_path, &candidate) {
            Ok(()) => {
                remove_sidecar_if_present(&candidate);
                return Ok(candidate);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(LegacyUpgradeError::PublishNumbered {
                    path: candidate,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(LegacyUpgradeError::OutputNamesExhausted {
        source_path: source_path.to_path_buf(),
    })
}

fn upgrade_staging_path(source_path: &Path) -> Result<PathBuf, LegacyUpgradeError> {
    let parent = source_path.parent().unwrap_or_else(|| Path::new(""));
    let name = source_path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "OpenPencil.op".into());
    for _ in 0..100 {
        let sequence = UPGRADE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{name}.op-upgrade-{}-{sequence}",
            std::process::id()
        ));
        if !candidate
            .try_exists()
            .map_err(|error| LegacyUpgradeError::StagingProbe {
                path: candidate.clone(),
                message: error.to_string(),
            })?
        {
            return Ok(candidate);
        }
    }
    Err(LegacyUpgradeError::StagingNamesExhausted {
        source_path: source_path.to_path_buf(),
    })
}

fn remove_file_if_present(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => eprintln!(
            "[open] could not remove OP upgrade staging file {}: {error}",
            path.display()
        ),
    }
}

fn remove_sidecar_if_present(path: &Path) {
    remove_file_if_present(&op_host_services::doc_io::sidecar_path(path));
}

fn is_op(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("op"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "openpencil-legacy-upgrade-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    #[test]
    fn numbered_copy_uses_first_unused_suffix_without_overwrite() {
        let dir = temp_dir("numbered");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Design.op");
        let staging = dir.join(".converted");
        std::fs::write(&source, b"original").unwrap();
        std::fs::write(&staging, b"converted").unwrap();
        std::fs::write(dir.join("Design (1).op"), b"copy one").unwrap();

        let published = publish_numbered_copy(&staging, &source).unwrap();
        assert_eq!(published, dir.join("Design (2).op"));
        assert_eq!(std::fs::read(&published).unwrap(), b"converted");
        assert_eq!(std::fs::read(&source).unwrap(), b"original");
        assert_eq!(
            std::fs::read(dir.join("Design (1).op")).unwrap(),
            b"copy one"
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn changed_approved_original_falls_back_to_numbered_copy() {
        let dir = temp_dir("changed");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Legacy.op");
        let staging = dir.join(".converted");
        std::fs::write(&source, b"approved bytes").unwrap();
        std::fs::write(&staging, b"converted bytes").unwrap();
        let approved = capture_output_state(&source).unwrap();
        std::fs::write(&source, b"changed after approval").unwrap();

        let target = publish_staged_upgrade(
            &staging,
            &source,
            UpgradeDecision::ReplaceOriginal,
            approved,
        )
        .unwrap();
        assert_eq!(target, dir.join("Legacy (1).op"));
        assert_eq!(std::fs::read(&source).unwrap(), b"changed after approval");
        assert_eq!(std::fs::read(&target).unwrap(), b"converted bytes");

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn current_or_unsafe_reports_do_not_request_upgrade() {
        assert!(!DocumentLoadReport::default().needs_schema_upgrade());
        let report = DocumentLoadReport {
            normalized_legacy: true,
            rewrite_blocked_by_schema_warning: true,
            ..Default::default()
        };
        assert!(!report.needs_schema_upgrade());
    }

    #[test]
    fn numbered_upgrade_writes_current_schema_and_embedded_editor_meta() {
        let dir = temp_dir("save-current");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Legacy.op");
        std::fs::write(
            &source,
            r#"{"version":"1.0","futureTop":{"mustSurvive":true},"children":[{"type":"path","id":"p","geometry":"M0 0L1 1","futureNode":{"mustSurvive":true}}]}"#,
        )
        .unwrap();
        let loaded = op_host_services::doc_io::load_editor_state_with_report(
            &source,
            op_editor_core::Locale::EnUs,
        )
        .unwrap();
        assert!(loaded.report.needs_schema_upgrade());
        let mut state = loaded.state;
        state.editor_ui.preserve_authored_geometry = true;
        let source_state = capture_output_state(&source).unwrap();
        let target = save_decision(
            &mut state,
            &source,
            &loaded.report,
            UpgradeDecision::NumberedCopy,
            source_state,
        )
        .unwrap();

        assert!(std::fs::read_to_string(&source)
            .unwrap()
            .contains("\"geometry\""));
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
        assert_eq!(
            saved["formatVersion"],
            jian_ops_schema::version::FORMAT_VERSION_CURRENT
        );
        assert_eq!(
            saved["editorMeta"]["preserveAuthoredGeometry"],
            serde_json::Value::Bool(true)
        );
        assert_eq!(saved["futureTop"]["mustSurvive"], true);
        assert_eq!(saved["children"][0]["futureNode"]["mustSurvive"], true);
        assert!(saved["children"][0].get("geometry").is_none());
        let reopened = op_host_services::doc_io::load_editor_state_with_report(
            &target,
            op_editor_core::Locale::EnUs,
        )
        .unwrap();
        assert!(!reopened.report.needs_schema_upgrade());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn changed_source_is_rejected_before_a_migration_copy_is_written() {
        let dir = temp_dir("save-failure");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Legacy.op");
        std::fs::write(
            &source,
            r#"{"version":"1.0","children":[{"type":"path","id":"p","geometry":"M0 0"}]}"#,
        )
        .unwrap();
        let loaded = op_host_services::doc_io::load_editor_state_with_report(
            &source,
            op_editor_core::Locale::EnUs,
        )
        .unwrap();
        let source_state = capture_output_state(&source).unwrap();
        std::fs::write(&source, r#"{"version":"1.0","children":[]}"#).unwrap();
        let mut state = loaded.state;

        let error = save_decision(
            &mut state,
            &source,
            &loaded.report,
            UpgradeDecision::NumberedCopy,
            source_state,
        )
        .expect_err("external source change must abort migration")
        .to_string();
        assert!(error.contains("changed while it was opening"));
        assert!(!dir.join("Legacy (1).op").exists());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn current_preserve_document_does_not_signal_an_upgrade() {
        let dir = temp_dir("current-preserve");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Ant Design Open Source (Community).op");
        std::fs::write(
            &source,
            r#"{"version":"1","editorMeta":{"activePageIndex":0,"preserveAuthoredGeometry":true},"pages":[{"id":"figma-page-0","name":"P","children":[]}],"children":[]}"#,
        )
        .unwrap();

        let loaded = op_host_services::doc_io::load_editor_state_with_report(
            &source,
            op_editor_core::Locale::EnUs,
        )
        .unwrap();
        assert!(!loaded.report.needs_schema_upgrade());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replacing_original_embeds_and_removes_adopted_sidecar() {
        let dir = temp_dir("replace-sidecar");
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("Legacy.op");
        let sidecar = op_host_services::doc_io::sidecar_path(&source);
        std::fs::write(&source, r#"{"version":"1.0","children":[]}"#).unwrap();
        std::fs::write(
            &sidecar,
            r#"{"active_page_index":0,"preserve_authored_geometry":true}"#,
        )
        .unwrap();
        let loaded = op_host_services::doc_io::load_editor_state_with_report(
            &source,
            op_editor_core::Locale::EnUs,
        )
        .unwrap();
        assert!(loaded.report.used_legacy_sidecar);
        let mut state = loaded.state;
        let source_state = capture_output_state(&source).unwrap();

        let target = save_decision(
            &mut state,
            &source,
            &loaded.report,
            UpgradeDecision::ReplaceOriginal,
            source_state,
        )
        .unwrap();

        assert_eq!(target, source);
        assert!(!sidecar.exists());
        let saved: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&source).unwrap()).unwrap();
        assert_eq!(
            saved["editorMeta"]["preserveAuthoredGeometry"],
            serde_json::Value::Bool(true)
        );

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn upgrade_prompt_resolves_localized_copy() {
        // The upgrade dialog keys are translated (they used to fall back
        // to English; the catalogs have since gained real translations).
        for key in ["dialog.upgradeOpTitle", "dialog.upgradeOpBody"] {
            let en = op_i18n::translate(op_editor_core::Locale::EnUs, key);
            let ja = op_i18n::translate(op_editor_core::Locale::Ja, key);
            assert_ne!(en, key, "English copy exists for {key}");
            assert_ne!(ja, en, "Japanese copy is a real translation for {key}");
        }
    }
}
