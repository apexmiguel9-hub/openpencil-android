//! Arrow-key routing for the Scene Template Center, split out of
//! `keyboard_input.rs` at the repo's 800-line cap.
//!
//! The panel floats over the canvas, so an arrow it has no use for still has
//! to stop at it: one that reached the canvas would nudge the selected node
//! while the user is typing a search query or a deck topic, editing the
//! document behind an open panel.

use winit::keyboard::NamedKey;

use crate::DesktopApp;

impl DesktopApp {
    /// Whether an arrow belongs to the open panel rather than the canvas.
    ///
    /// Cmd/Ctrl-held arrows are left alone — those are canvas chords, and the
    /// panel has no use for them.
    pub(crate) fn scene_template_owns_arrow(&self, key: &NamedKey) -> bool {
        if self.zoom_modifier {
            return false;
        }
        matches!(
            key,
            NamedKey::ArrowLeft | NamedKey::ArrowRight | NamedKey::ArrowUp | NamedKey::ArrowDown
        ) && self
            .host
            .editor_state()
            .editor_ui
            .scene_template_center
            .open
    }

    /// Route one arrow to the panel's focused field, and report it consumed
    /// either way. Left / Right move the caret; Up / Down mean nothing in a
    /// one-line field but are still swallowed.
    pub(crate) fn apply_scene_template_arrow(&mut self, key: &NamedKey) -> bool {
        match key {
            NamedKey::ArrowLeft => {
                self.host
                    .apply_scene_template_caret(false, self.shift_modifier);
            }
            NamedKey::ArrowRight => {
                self.host
                    .apply_scene_template_caret(true, self.shift_modifier);
            }
            _ => {}
        }
        true
    }
}
