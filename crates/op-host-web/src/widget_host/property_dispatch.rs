//! Web PropertyPanel action dispatch, mirroring the native host.
//!
//! The action match itself is shared with the native host in
//! `op_editor_ui::widgets::property_panel_dispatch`; this file keeps
//! the web platform glue — layout-scene sizing resolution, viewport
//! refit, image-crop entry, and the browser codegen clipboard /
//! download / bundle IO.

#[path = "property_compositing_dispatch.rs"]
mod property_compositing_dispatch;
#[path = "property_input_dispatch.rs"]
mod property_input_dispatch;
// Draft seeding is shared with the native host; re-exported here so the
// web press / focus call sites keep their historical path.
pub(in crate::widget_host) use op_editor_ui::widgets::property_panel_commit::property_focus_initial;

use super::WidgetHost;
use op_editor_ui::widgets::property_panel_dispatch as dispatch;
use op_editor_ui::widgets::PropertyPanelAction;

impl WidgetHost {
    /// Swap a synced document into the live editor state via the shared, tested
    /// `EditorState::replace_document`, then `mark_dirty()` so the next paint
    /// re-derives the layout scene from the NEW document. Without the
    /// `mark_dirty()` the web host's `refresh_layout_scene()` is a no-op (the
    /// dirty flag isn't set), so the repaint would present the STALE scene yet
    /// succeed — and `WebSyncClient::sync` would then commit the version against
    /// a stale paint. Used by the opt-in `live-sync` glue. Lives here (not
    /// `widget_host.rs`) to keep that spine under the 800-line cap.
    /// `undoable` = the sync client was already initialized, i.e. this
    /// is NOT the mount-time first pull (starter → daemon doc), so the
    /// external write (AI turn / MCP client) lands as one undo step.
    #[cfg(feature = "canvaskit")]
    pub(crate) fn replace_document_from_sync(
        &mut self,
        doc: op_editor_core::PenDocument,
        undoable: bool,
    ) {
        if undoable {
            self.editor_state.replace_document_with_undo(doc);
        } else {
            self.editor_state.replace_document(doc);
        }
        // A whole-document replacement can restart the revision at 0 / page 0,
        // aliasing the previous document's LayerPanel row-model-cache key — rotate
        // the owner so the next owned paint resolve rebuilds the rows.
        self.force_rotate_layer_panel_owner();
        self.mark_dirty();
    }

    pub(in crate::widget_host) fn apply_property_action(&mut self, action: PropertyPanelAction) {
        use dispatch::{
            InstanceLifecycleOutcome, PropertyActionFollowUp as F, PropertyActionOutcome as O,
        };
        use PropertyPanelAction as A;
        // ImageTileScale lives in the floating image-fill editor. Any button
        // action may close that editor (or switch away from Tile), so commit
        // its draft before the instance-write scope and before the input can
        // disappear. Regular PropertyPanel presses already blur inputs; this
        // also covers popup-owned actions and direct dispatch in tests/hosts.
        self.commit_image_tile_scale_focus_if_any();
        // Compositing and page-background edits share the same exact
        // undo contract as native: snapshot before the instance-write
        // redirect and push only after the routed document really changed.
        let document_before =
            dispatch::updates_document(&action).then(|| self.editor_state.snapshot_for_history());
        // Instance / component lifecycle actions act on the REAL Ref node,
        // so they dispatch BEFORE the instance-write redirect scope below.
        if let InstanceLifecycleOutcome::Handled {
            page_switched,
            select,
        } = dispatch::apply_instance_lifecycle_action(&mut self.editor_state, &action)
        {
            if page_switched {
                self.fit_active_page_after_switch(self.last_viewport_w, self.last_viewport_h);
            }
            if let Some(master) = select {
                self.editor_state.set_single_selection(master);
            }
            self.mark_dirty();
            return;
        }
        // Resolve Fill/Hug pixels against the real scene before an instance
        // write scope temporarily swaps a Ref anchor for its display node.
        let resolved_sizing_fallback = match action {
            A::ToggleSizeFillWidth | A::ToggleSizeHugWidth => self.selected_resolved_size(true),
            A::ToggleSizeFillHeight | A::ToggleSizeHugHeight => self.selected_resolved_size(false),
            _ => None,
        };
        let instance_scope = self.editor_state.begin_instance_write_for_anchor();
        let outcome = dispatch::apply_property_action(
            &mut self.editor_state,
            &action,
            dispatch::PropertyActionContext {
                now_ms: self.now_ms,
                resolved_sizing_fallback,
                image_adjustment_drag: &mut self.image_adjustment_drag,
                effect_radius_drag: &mut self.effect_radius_drag,
            },
        );
        match outcome {
            O::Handled => {}
            O::FollowUp(F::EnterImageCropEdit) => {
                let _ = self.enter_selected_image_crop_edit();
            }
            O::FollowUp(F::ExitImageCropEdit) => {
                self.exit_image_crop_edit();
            }
            O::FollowUp(F::EnsureSystemFontsLoaded) => {
                // The web entry drains Local Font Access after this
                // press; until the browser resolves / rejects that
                // permission flow the picker paints the bundled group
                // plus the TS fallback system list.
            }
            O::HostOwned => self.apply_host_property_action(action),
        }
        if let Some(scope) = instance_scope {
            self.editor_state.finish_instance_write(scope);
        }
        if let Some(before) = document_before {
            if self.editor_state.snapshot_for_history() != before {
                self.editor_state.history_push_past(before);
            }
        }
        self.mark_dirty();
    }

    /// The arms the shared dispatcher hands back: image Search /
    /// Generate popovers (host-owned input-selection drag + blur glue),
    /// the effect-param focus seed (needs the host-owned draft commits
    /// first), and the Code panel's browser IO.
    fn apply_host_property_action(&mut self, action: PropertyPanelAction) {
        use PropertyPanelAction as A;
        match action {
            A::MatchImageAspectRatio => self.match_selected_image_aspect_ratio(),
            A::ToggleImageSearchPopover => self.toggle_image_search_popover(),
            A::ToggleImageGeneratePopover => self.toggle_image_generate_popover(),
            A::RunImageSearch => self.run_image_search(),
            A::SelectImageSearchResult(index) => self.select_image_search_result(index),
            A::RunImageGenerate => self.run_image_generate(),
            A::ApplyGeneratedImage => self.apply_generated_image(),
            A::RetryImageGenerate => self.retry_image_generate(),
            A::OpenImageGenSettings => self.open_image_gen_settings(),
            A::FocusEffectParam {
                effect,
                field,
                value,
            } => {
                // Commit whatever draft owned the input before seeding
                // this param's, then re-read the live value.
                self.commit_property_focus_if_any();
                dispatch::focus_effect_param(
                    &mut self.editor_state,
                    effect,
                    field,
                    value,
                    self.now_ms,
                );
            }
            A::Codegen(codegen_action) => self.apply_codegen_action(codegen_action),
            // Everything else was applied by the shared dispatcher.
            _ => {}
        }
    }

    /// Dispatch a Code-panel action. `SelectFramework` is pure
    /// `editor_state.codegen` state (works without the `codegen`
    /// feature); `Generate` / `Regenerate` / `Cancel` raise the pending
    /// flags the `lib.rs` mousedown drain turns into
    /// `codegen_web::drain_codegen_flags` work (the dispatch has no
    /// `Inner` / daemon base in scope — mirror of the desktop
    /// pending-flag + `launch_codegen_if_pending` /
    /// `drain_codegen_cancel_request` pattern); `Copy` / `Download` are
    /// browser IO via `web_clipboard` (Download produces a
    /// `component.zip` when the generation returned image assets —
    /// desktop `codegen_export` layout).
    fn apply_codegen_action(
        &mut self,
        action: op_editor_ui::widgets::property_panel_action::CodegenAction,
    ) {
        use dispatch::CodegenFollowUp;
        match dispatch::apply_codegen_action(&mut self.editor_state, &action, self.now_ms) {
            CodegenFollowUp::None => {}
            CodegenFollowUp::FrameworkChanged => {
                self.code_selection_drag = None;
            }
            CodegenFollowUp::Copy(_code) => {
                #[cfg(feature = "canvaskit")]
                self.host_copy_text(&_code);
            }
            CodegenFollowUp::Download => {
                #[cfg(feature = "canvaskit")]
                crate::codegen_web::download_generated(&self.editor_state, self.document_epoch());
            }
            CodegenFollowUp::ExportBundle => {
                // Live structure bundle (TS code-panel.tsx
                // `handleDownloadStructureBundle` → `buildAIStructureBundle`):
                // built FRESH from the selection (or active page) at click
                // time — no completed generation required. Nothing to bundle
                // returns silently, like the TS handler.
                #[cfg(feature = "canvaskit")]
                {
                    if let Some(bytes) =
                        crate::codegen_bundle::build_live_bundle_zip(&self.editor_state)
                    {
                        let _ = crate::web_clipboard::download_bytes(
                            "bundle.zip",
                            "application/zip",
                            &bytes,
                        );
                    }
                }
            }
        }
    }
}

/// Public alias for the shared colour-target mapping — used by the
/// press dispatch in `press.rs` so it can anchor the colour picker at
/// the clicked y instead of always passing `0.0`.
pub(in crate::widget_host) fn color_target_public(
    t: op_editor_core::ColorTarget,
) -> op_editor_core::ui_draft::ColorTarget {
    dispatch::color_target(t)
}
