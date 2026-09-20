//! Contract tests for typing into the Asset Center on the web host.
//!
//! Web was worse off than native: besides missing from `input_active` — which
//! is what gives the hidden capture input DOM focus, and therefore the only
//! reason composition events ever arrive — `apply_text` had no branch for the
//! panel at all. A character typed into the gallery fell through to the
//! canvas shortcuts, where a bare letter switches the active tool behind the
//! open overlay.

use super::WidgetHost;
use op_editor_core::SceneTemplateFocus;

fn gallery_host() -> WidgetHost {
    let mut host = WidgetHost::new();
    host.last_viewport_w = 1440.0;
    host.last_viewport_h = 900.0;
    host.editor_state.editor_ui.open_scene_template_center(0);
    host
}

fn search(host: &WidgetHost) -> String {
    host.editor_state
        .editor_ui
        .scene_template_center
        .search
        .text()
        .to_string()
}

/// The gate the browser's hidden IME input hangs off.
#[test]
fn an_open_gallery_owns_the_keyboard_for_ime_purposes() {
    assert!(!WidgetHost::new().text_input_focus_active());
    assert!(gallery_host().text_input_focus_active());
}

/// `apply_text` had no branch for the panel at all, so every character fell
/// through to the canvas paths behind the overlay.
#[test]
fn plain_characters_reach_the_panel() {
    let mut host = gallery_host();

    for c in "rect".chars() {
        assert!(host.apply_text(c));
    }

    assert_eq!(search(&host), "rect");
}

/// The gate the tool shortcuts read, so a letter typed into the gallery is
/// never also a tool switch.
#[test]
fn an_open_gallery_suppresses_the_editor_shortcuts() {
    assert!(gallery_host().input_active());
}

/// A composed candidate arrives through the same door a paste does.
#[test]
fn a_committed_candidate_lands_in_the_focused_field() {
    let mut host = gallery_host();
    host.editor_state.editor_ui.scene_template_center.focus = SceneTemplateFocus::Generate;

    assert!(host.apply_paste_text("季度复盘"));

    assert_eq!(
        host.editor_state
            .editor_ui
            .scene_template_center
            .generate
            .text(),
        "季度复盘"
    );
    assert!(search(&host).is_empty());
}

/// The candidate window anchors at the field, not at the last pointer
/// position — the fallback this branch exists to replace.
#[test]
fn the_candidate_window_anchors_at_the_focused_field() {
    let mut host = gallery_host();
    host.last_cursor_x = 20.0;
    host.last_cursor_y = 20.0;

    let rect = host.ime_anchor_rect().expect("an open gallery anchors");
    let panel_rect = host
        .scene_template_panel_rect(1440.0, 900.0)
        .expect("the panel has a rect while open");

    assert!(
        panel_rect.contains(rect.origin),
        "the anchor fell back to the pointer: {rect:?}"
    );
}

/// Copy takes the gallery's own selection rather than whatever the canvas
/// had selected behind the overlay.
#[test]
fn copy_reads_the_gallery_field() {
    let mut host = gallery_host();
    {
        let input = &mut host.editor_state.editor_ui.scene_template_center.search;
        input.set_text("演示文稿");
        input.select_all();
    }

    assert_eq!(
        host.focused_input_selected_text().as_deref(),
        Some("演示文稿")
    );
}
