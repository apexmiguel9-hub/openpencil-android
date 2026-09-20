use op_editor_core::prompt_center_catalog::PromptCategory;
use op_editor_core::EditorState;

#[test]
fn app_preferences_preserve_prompt_center_store_state() {
    let mut previous = EditorState::new();
    previous
        .editor_ui
        .prompt_center
        .install_custom_prompts(Vec::new(), true);
    previous.editor_ui.prompt_center.add_custom_prompt(
        "Reusable".into(),
        "Reusable body".into(),
        PromptCategory::Modify,
        42,
    );
    let mut next = EditorState::new();

    crate::file_actions::preserve_app_preferences(&previous, &mut next);

    assert_eq!(next.editor_ui.prompt_center.custom_prompts.len(), 1);
    assert!(next.editor_ui.prompt_center.custom_store_writable);
    assert!(next.editor_ui.prompt_center.custom_store_dirty);
}
