//! Image-node panel transitions shared by the native and web widget
//! hosts (their `widget_host/image_panel_dispatch.rs` twins used to
//! carry these as copy-pasted impl blocks — Search / Generate popover
//! toggles, search submit, result apply, and the popover keyboard
//! routing).
//!
//! Everything here is pure `EditorState` state-machine work. Hosts stay
//! thin: they own the platform glue (input-selection drag bookkeeping,
//! blurring chrome inputs, the property-focus commit) and call
//! `mark_dirty()` when a transition reports a change.

use jian_ops_schema::node::PenNode;

use crate::agent_settings::AgentSettingsTab;
use crate::editor_ui_state::EditorUiState;
use crate::image_panel_state::ImageGeneratePhase;
use crate::state::EditorState;

/// What routing a keystroke into the image popovers did.
///
/// `consumed` mirrors the host method's return value (the popover owns
/// the key even when nothing changed, so it can't fall through to node
/// deletion / canvas nudge); `changed` is the host's `mark_dirty()`
/// trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImageInputEffect {
    pub consumed: bool,
    pub changed: bool,
}

impl ImageInputEffect {
    /// No popover is open — the caller keeps routing the key.
    pub const IGNORED: Self = Self {
        consumed: false,
        changed: false,
    };

    const fn consumed(changed: bool) -> Self {
        Self {
            consumed: true,
            changed,
        }
    }
}

/// Seed for the search query / generate prompt: the node's authored
/// `imageSearchQuery` / `imagePrompt`, else its name (TS
/// `node.imageSearchQuery ?? node.name ?? ''`).
pub fn selected_image_seed(state: &EditorState, prompt: bool) -> String {
    match state.selected_node() {
        Some(PenNode::Image(image)) => {
            let authored = if prompt {
                image.image_prompt.as_deref()
            } else {
                image.image_search_query.as_deref()
            };
            authored
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| image.base.name.clone())
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

/// Apply the Search popover toggle. `opening` + `seed` are resolved by
/// the caller BEFORE it runs its platform glue (drag reset / input
/// blur), so the seed reflects the pre-blur selection.
pub fn apply_image_search_toggle(
    state: &mut EditorState,
    opening: bool,
    seed: String,
    now_ms: u64,
) {
    let panel = &mut state.editor_ui.image_panel;
    // Opening either popover closes the other (TS popovers are
    // mutually exclusive portals).
    panel.close_popovers();
    if opening {
        panel.search_open = true;
        panel.search_query.set_text(seed);
        panel.search_query.touch(now_ms);
    }
}

/// Apply the Generate popover toggle (TS `handleOpenChange`: reset
/// prompt + phase + preview + error on open).
pub fn apply_image_generate_toggle(
    state: &mut EditorState,
    opening: bool,
    seed: String,
    now_ms: u64,
) {
    let panel = &mut state.editor_ui.image_panel;
    panel.close_popovers();
    if opening {
        panel.generate_open = true;
        panel.generate_prompt.set_text(seed);
        panel.generate_prompt.touch(now_ms);
        panel.generate_phase = ImageGeneratePhase::Idle;
        panel.generate_preview = None;
        panel.generate_error.clear();
    }
}

/// Close the property pickers that would overlap the image popovers.
pub fn close_other_property_popovers_for_image(ui: &mut EditorUiState) {
    ui.close_fill_type_picker();
    ui.image_fill_popover_open = false;
    ui.font_weight_picker_open = false;
    ui.export_scale_picker_open = false;
    ui.export_format_picker_open = false;
    ui.close_color_variable_picker();
    ui.close_font_picker();
}

/// Submit the search box (Enter / the icon button). No-op while a
/// search is in flight or the query is blank (TS disables the button on
/// both).
pub fn run_image_search(state: &mut EditorState) {
    let panel = &mut state.editor_ui.image_panel;
    if !panel.search_open || panel.search_loading || panel.search_query.text().trim().is_empty() {
        return;
    }
    panel.search_loading = true;
    panel.search_has_searched = true;
    panel.search_epoch = panel.search_epoch.wrapping_add(1);
}

/// The clicked result's thumbnail URL (TS `onSelect(result.thumbUrl)`).
pub fn image_search_result_url(state: &EditorState, index: usize) -> Option<String> {
    state
        .editor_ui
        .image_panel
        .search_results
        .get(index)
        .map(|hit| hit.thumb_data_url.as_ref().clone())
}

/// The generated preview's URL, if the popover is showing one.
pub fn generated_preview_url(state: &EditorState) -> Option<String> {
    state
        .editor_ui
        .image_panel
        .generate_preview
        .as_ref()
        .map(|url| url.as_ref().clone())
}

/// Kick off generation (drained by the host pump). The not-configured
/// gate lives in the popover view; this also guards so a stale press
/// can't start a job with no profile.
pub fn run_image_generate(state: &mut EditorState) {
    let configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if !panel.generate_open
        || !configured
        || panel.generate_prompt.text().trim().is_empty()
        || panel.generate_phase == ImageGeneratePhase::Loading
    {
        return;
    }
    panel.generate_phase = ImageGeneratePhase::Loading;
    panel.generate_error.clear();
    panel.generate_preview = None;
    panel.generate_epoch = panel.generate_epoch.wrapping_add(1);
}

/// Back to the generate popover's idle view.
pub fn retry_image_generate(state: &mut EditorState) {
    let panel = &mut state.editor_ui.image_panel;
    panel.generate_phase = ImageGeneratePhase::Idle;
    panel.generate_preview = None;
    panel.generate_error.clear();
}

/// Open the settings modal on the Images tab (TS `setDialogOpen(true)`
/// from the not-configured view).
pub fn open_image_gen_settings(state: &mut EditorState) {
    state.editor_ui.image_panel.close_popovers();
    state.editor_ui.agent_settings_open = true;
    state.editor_ui.agent_settings.tab = AgentSettingsTab::Images;
}

/// Commit `src` onto the selected image node (with history). Returns
/// `true` when the write ran, i.e. the host should mark dirty.
pub fn write_selected_image_src(state: &mut EditorState, src: &str) -> bool {
    let id = state.selection.anchor.clone();
    if !id.is_real() || src.is_empty() {
        return false;
    }
    state.commit_history();
    if let Some(PenNode::Image(image)) =
        crate::walkers::find_node_mut(state.active_children_mut(), &id)
    {
        image.src = src.into();
    }
    true
}

/// Route a printable char into whichever popover input is open.
pub fn image_panel_text(state: &mut EditorState, c: char, now_ms: u64) -> ImageInputEffect {
    if c.is_control() {
        return ImageInputEffect::IGNORED;
    }
    let generate_configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if panel.search_open {
        let mut text = [0u8; 4];
        panel
            .search_query
            .insert_str(c.encode_utf8(&mut text), now_ms);
        return ImageInputEffect::consumed(true);
    }
    if panel.generate_open {
        // Loading / preview do not paint an editor. Swallow printable
        // input so it cannot mutate either the hidden prompt or canvas.
        let mut changed = false;
        if let Some(input) = panel.active_input_mut(generate_configured) {
            let mut text = [0u8; 4];
            input.insert_str(c.encode_utf8(&mut text), now_ms);
            changed = true;
        }
        return ImageInputEffect::consumed(changed);
    }
    ImageInputEffect::IGNORED
}

/// Backspace in the open popover's input. Consumes the key even on an
/// empty draft so it can't fall through to node deletion.
pub fn image_panel_backspace(state: &mut EditorState, now_ms: u64) -> ImageInputEffect {
    let generate_configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if panel.search_open {
        let before = panel.search_query.text().to_owned();
        panel.search_query.backspace(now_ms);
        return ImageInputEffect::consumed(panel.search_query.text() != before);
    }
    if panel.generate_open {
        let mut changed = false;
        if let Some(input) = panel.active_input_mut(generate_configured) {
            let before = input.text().to_owned();
            input.backspace(now_ms);
            changed = input.text() != before;
        }
        return ImageInputEffect::consumed(changed);
    }
    ImageInputEffect::IGNORED
}

/// Forward Delete in the visible image-popover input. The popover still
/// consumes Delete when no glyph changes so the selected image node
/// behind it can never be removed accidentally.
pub fn image_panel_delete(state: &mut EditorState, now_ms: u64) -> ImageInputEffect {
    let generate_configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if !panel.search_open && !panel.generate_open {
        return ImageInputEffect::IGNORED;
    }
    let mut changed = false;
    if let Some(input) = panel.active_input_mut(generate_configured) {
        let before = input.text().to_owned();
        input.delete_forward(now_ms);
        changed = input.text() != before;
    }
    ImageInputEffect::consumed(changed)
}

/// Move the persistent image-popover caret. Consumes the arrow at text
/// boundaries so it never falls through to canvas nudge.
pub fn image_panel_caret(
    state: &mut EditorState,
    forward: bool,
    extend: bool,
    now_ms: u64,
) -> ImageInputEffect {
    let generate_configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if !panel.search_open && !panel.generate_open {
        return ImageInputEffect::IGNORED;
    }
    let mut changed = false;
    if let Some(input) = panel.active_input_mut(generate_configured) {
        if forward {
            input.move_right(extend, now_ms);
        } else {
            input.move_left(extend, now_ms);
        }
        changed = true;
    }
    ImageInputEffect::consumed(changed)
}

/// Move the visible image-popover input to its start or end. The key is
/// still consumed when the generate view has no editable field.
pub fn image_panel_edge(
    state: &mut EditorState,
    end: bool,
    extend: bool,
    now_ms: u64,
) -> ImageInputEffect {
    let configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if !panel.search_open && !panel.generate_open {
        return ImageInputEffect::IGNORED;
    }
    let mut changed = false;
    if let Some(input) = panel.active_input_mut(configured) {
        if end {
            input.move_end(extend, now_ms);
        } else {
            input.move_home(extend, now_ms);
        }
        changed = true;
    }
    ImageInputEffect::consumed(changed)
}

/// Select-all inside the visible image-popover input.
pub fn image_panel_select_all(state: &mut EditorState, now_ms: u64) -> ImageInputEffect {
    let configured = state.editor_ui.agent_settings.image_generation_configured();
    let panel = &mut state.editor_ui.image_panel;
    if !panel.search_open && !panel.generate_open {
        return ImageInputEffect::IGNORED;
    }
    let mut changed = false;
    if let Some(input) = panel.active_input_mut(configured) {
        input.select_all();
        input.touch(now_ms);
        changed = true;
    }
    ImageInputEffect::consumed(changed)
}

/// Close the property-owned floating popovers before a higher-z overlay
/// (or a selection-changing secondary click) takes focus, so keyboard /
/// pointer ownership cannot remain hidden underneath it. Returns `true`
/// when something closed.
///
/// The caller commits the image-fill tile-scale draft first — that
/// commit routes through host-owned variable/effect commits, and the
/// two touch disjoint state.
pub fn close_image_popovers_for_higher_overlay(state: &mut EditorState) -> bool {
    let mut changed = false;
    {
        let panel = &mut state.editor_ui.image_panel;
        if panel.search_open || panel.generate_open {
            panel.close_popovers();
            changed = true;
        }
    }
    if state.editor_ui.image_fill_popover_open {
        state.editor_ui.image_fill_popover_open = false;
        changed = true;
    }
    if state.editor_ui.compositing_picker.open {
        state.editor_ui.close_compositing_picker();
        changed = true;
    }
    changed
}
