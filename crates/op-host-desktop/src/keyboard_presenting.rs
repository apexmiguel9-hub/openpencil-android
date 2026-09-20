//! Presenter keys, claimed before the editor's shortcut table.
//!
//! While Preview presents a deck (`op_editor_core::preview_slideshow`) the
//! keyboard belongs to the presentation: the deck is being SHOWN, not
//! edited, so Backspace steps back through slides rather than deleting a
//! node, Enter advances instead of sending a chat turn, and Space does not
//! start a canvas pan.
//!
//! This runs ahead of `handle_key_pressed`'s match rather than as arms
//! inside it, for two reasons. Every one of these keys already has an
//! earlier arm in that table, so as arms they would have to sit at the very
//! top and stay there — a silent ordering dependency ninety arms long. And
//! keeping the whole presenter mapping in one place makes it readable as
//! what it is: the Keynote conventions, in one list.
//!
//! Arrow keys are NOT here. Preview already routes them through
//! `WidgetHostNative::preview_dispatch_key`, which claims them for the deck
//! at the same point it would otherwise hand them to the widget runtime.

use winit::keyboard::{Key, NamedKey};

use crate::DesktopApp;

impl DesktopApp {
    /// Handle `logical_key` as a presenter key. Returns whether the key was
    /// consumed — `false` for every key outside a presentation, so the
    /// editor's own table sees exactly what it saw before.
    pub(crate) fn handle_presenting_key(&mut self, logical_key: &Key) -> bool {
        if !self.host.preview_slideshow_active() {
            return false;
        }
        let Key::Named(named) = logical_key else {
            return false;
        };
        match named {
            // Forward: the keys a remote clicker sends, plus the ones a hand
            // on the keyboard reaches for.
            NamedKey::Enter | NamedKey::Space | NamedKey::PageDown => {
                self.host.preview_slideshow_step(1);
            }
            NamedKey::Backspace | NamedKey::PageUp => {
                self.host.preview_slideshow_step(-1);
            }
            // Jump to the title / closing slide without walking the deck.
            NamedKey::Home => {
                self.host.preview_slideshow_to_end(false);
            }
            NamedKey::End => {
                self.host.preview_slideshow_to_end(true);
            }
            // Escape is deliberately absent: it exits through the editor's
            // own escape ladder, which already puts Preview at the top.
            _ => return false,
        }
        true
    }
}
