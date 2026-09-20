//! Native persistence for user-saved Prompt Center entries.
//!
//! The editor core owns the in-memory state and dirty bit. This module is
//! the desktop-only bridge to `~/.openpencil`, keeping filesystem access out
//! of the wasm-clean editor crates.

use op_config_store::ConfigStore;
use op_editor_core::{CustomPrompt, PromptCenterState};
use op_host_native::WidgetHostNative;

const PROMPT_CENTER_CUSTOM_FILE: &str = "prompt_center_custom.json";

/// Load desktop custom prompts and advertise writable native storage.
pub(crate) fn install_user_prompts(host: &mut WidgetHostNative) {
    crate::test_config_root::guard_user_config();
    let store = match ConfigStore::user() {
        Ok(store) => store,
        Err(error) => {
            eprintln!("openpencil-desktop: prompt center store unavailable: {error}");
            host.editor_state_mut()
                .editor_ui
                .prompt_center
                .install_custom_prompts(Vec::new(), false);
            return;
        }
    };
    let prompts = load_prompts(&store);
    host.editor_state_mut()
        .editor_ui
        .prompt_center
        .install_custom_prompts(prompts, true);
}

/// Persist a pending save/delete mutation, leaving the dirty bit set on error.
pub(crate) fn flush_user_prompts_if_dirty(host: &mut WidgetHostNative) -> bool {
    if !host
        .editor_state()
        .editor_ui
        .prompt_center
        .custom_store_dirty
    {
        return false;
    }
    crate::test_config_root::guard_user_config();
    let store = match ConfigStore::user() {
        Ok(store) => store,
        Err(error) => {
            eprintln!("openpencil-desktop: prompt center store unavailable: {error}");
            return false;
        }
    };
    match flush_to_store(&store, &mut host.editor_state_mut().editor_ui.prompt_center) {
        Ok(wrote) => wrote,
        Err(error) => {
            eprintln!("openpencil-desktop: prompt center save failed: {error}");
            false
        }
    }
}

fn load_prompts(store: &ConfigStore) -> Vec<CustomPrompt> {
    match store.read_json(PROMPT_CENTER_CUSTOM_FILE) {
        Ok(Some(prompts)) => prompts,
        Ok(None) => Vec::new(),
        Err(error) => {
            eprintln!("openpencil-desktop: prompt center load failed: {error}");
            Vec::new()
        }
    }
}

fn flush_to_store(store: &ConfigStore, state: &mut PromptCenterState) -> std::io::Result<bool> {
    if !state.custom_store_dirty {
        return Ok(false);
    }
    store.write_json(PROMPT_CENTER_CUSTOM_FILE, &state.custom_prompts)?;
    state.custom_store_dirty = false;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_core::prompt_center_catalog::PromptCategory;
    use op_editor_core::PromptFilter;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

    struct ScratchStore {
        root: PathBuf,
        store: ConfigStore,
    }

    impl ScratchStore {
        fn new(label: &str) -> Self {
            let serial = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "openpencil-prompt-center-{label}-{}-{serial}",
                std::process::id()
            ));
            Self {
                store: ConfigStore::at(root.clone()),
                root,
            }
        }
    }

    impl Drop for ScratchStore {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn writable_state() -> PromptCenterState {
        let mut state = PromptCenterState::default();
        state.install_custom_prompts(Vec::new(), true);
        state
    }

    #[test]
    fn save_load_delete_and_restart_round_trip() {
        let scratch = ScratchStore::new("round-trip");
        let mut first_run = writable_state();
        let first_id = first_run
            .add_custom_prompt(
                "Trip planner".into(),
                "Build a travel planner".into(),
                PromptCategory::MobileApp,
                10,
            )
            .expect("writable state should add a prompt");
        first_run
            .add_custom_prompt(
                "Dense metrics".into(),
                "Build a metrics dashboard".into(),
                PromptCategory::Dashboard,
                11,
            )
            .expect("writable state should add a second prompt");
        assert!(flush_to_store(&scratch.store, &mut first_run).unwrap());
        assert!(!first_run.custom_store_dirty);

        let mut second_run = PromptCenterState::default();
        second_run.install_custom_prompts(load_prompts(&scratch.store), true);
        assert_eq!(second_run.custom_prompts.len(), 2);
        assert_eq!(second_run.custom_prompts[0].title, "Trip planner");
        assert!(second_run.delete_custom_prompt(&first_id));
        assert!(second_run.custom_store_dirty);
        assert!(flush_to_store(&scratch.store, &mut second_run).unwrap());

        let mut third_run = PromptCenterState::default();
        third_run.install_custom_prompts(load_prompts(&scratch.store), true);
        assert_eq!(third_run.custom_prompts.len(), 1);
        assert_eq!(third_run.custom_prompts[0].title, "Dense metrics");
        assert_eq!(third_run.filter, PromptFilter::All);
        assert!(third_run.custom_store_writable);
        assert!(!third_run.custom_store_dirty);
    }

    #[test]
    fn malformed_file_loads_as_empty_writable_state() {
        let scratch = ScratchStore::new("malformed");
        std::fs::create_dir_all(&scratch.root).unwrap();
        std::fs::write(
            scratch.store.path(PROMPT_CENTER_CUSTOM_FILE).unwrap(),
            b"{broken",
        )
        .unwrap();

        let mut restarted = PromptCenterState::default();
        restarted.install_custom_prompts(load_prompts(&scratch.store), true);
        assert!(restarted.custom_prompts.is_empty());
        assert!(restarted.custom_store_writable);
        assert!(!restarted.custom_store_dirty);
    }

    #[test]
    fn failed_write_keeps_dirty_for_retry() {
        let scratch = ScratchStore::new("write-error");
        std::fs::write(&scratch.root, b"not a directory").unwrap();
        let mut state = writable_state();
        state
            .add_custom_prompt(
                "Retry me".into(),
                "Keep this body after an I/O failure".into(),
                PromptCategory::Starter,
                12,
            )
            .unwrap();

        assert!(flush_to_store(&scratch.store, &mut state).is_err());
        assert!(state.custom_store_dirty);
    }
}
