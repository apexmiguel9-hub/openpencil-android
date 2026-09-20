//! The editor's one transient notice slot.
//!
//! Some things the editor does to a user's document need saying out loud even
//! though there is no panel listening. The online conflict auto-accept is the
//! motivating case: the remote document replaces what the tab had, the local
//! copy is recoverable through undo, and the surface that would normally carry
//! that sentence — the collaboration panel's notice strip — is structurally
//! unreachable outside a session.
//!
//! Deliberately **one slot, not a queue**. A queue turns a notice into a
//! backlog the user has to read through, and the second message is nearly
//! always the one that matters: a newer notice supersedes an older one rather
//! than waiting behind it.
//!
//! Time-driven, not event-driven: the toast expires at a wall instant, so the
//! hosts' animation scheduler is told when to wake (see
//! `op_editor_ui::widgets::editor_toast_flow::next_deadline_ms`). Without that
//! it would linger until the user's next mouse move.

use crate::editor_ui_state::EditorUiState;

/// How long a toast stays up before it expires on its own.
///
/// Long enough to read a sentence that names a recovery action, short enough
/// that it is gone before it becomes furniture.
pub const EDITOR_TOAST_LIFETIME_MS: u64 = 8_000;

/// How loudly a toast reads.
///
/// Only two, on purpose: a toast is never an error dialog. Anything that needs
/// a decision from the user needs a surface that takes focus, which this one
/// deliberately does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorToastLevel {
    /// Something happened and it is fine. Neutral chrome.
    #[default]
    Info,
    /// Something happened that the user may want to act on — the accented
    /// variant, e.g. a document replaced under them with a way back.
    Warn,
}

/// The live toast.
///
/// Carries a locale key plus its arguments rather than a finished sentence:
/// `EditorUiState` outlives any one locale, and the browser host can switch
/// language while a toast is up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorToastState {
    /// Locale-table key for the message.
    pub i18n_key: String,
    /// Structured fields the localized template interpolates.
    pub args: Vec<(String, String)>,
    pub level: EditorToastLevel,
    /// Host clock reading when the toast was raised.
    pub shown_at_ms: u64,
}

impl EditorToastState {
    /// The instant this toast stops being shown.
    pub const fn expires_at_ms(&self) -> u64 {
        self.shown_at_ms.saturating_add(EDITOR_TOAST_LIFETIME_MS)
    }

    /// Whether it is still due to paint at `now_ms`.
    ///
    /// A clock that runs backwards (a host re-basing `now_ms`) keeps the toast
    /// up rather than hiding it early — the expiry is a courtesy, not a
    /// correctness boundary.
    pub const fn is_visible(&self, now_ms: u64) -> bool {
        now_ms < self.expires_at_ms()
    }

    /// Borrowed arg pairs in the shape `op_i18n::interpolate` wants.
    pub fn arg_pairs(&self) -> Vec<(&str, &str)> {
        self.args
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }
}

impl EditorUiState {
    /// Raise a toast, replacing whatever was in the slot.
    ///
    /// Replacement is the whole policy: the newest notice is the one that
    /// describes what just happened to the document, and stacking or queueing
    /// would show the user a stale sentence first.
    pub fn show_toast(
        &mut self,
        i18n_key: impl Into<String>,
        args: Vec<(String, String)>,
        level: EditorToastLevel,
        now_ms: u64,
    ) {
        self.editor_toast = Some(EditorToastState {
            i18n_key: i18n_key.into(),
            args,
            level,
            shown_at_ms: now_ms,
        });
    }

    /// Clear the slot — the dismiss button, and any transition that makes the
    /// message no longer true (an account switch, say: one user's notice must
    /// never survive into the next user's tab).
    pub fn dismiss_toast(&mut self) {
        self.editor_toast = None;
    }

    /// The toast that should paint at `now_ms`, if any.
    ///
    /// Expiry is read here rather than swept by a timer so the state stays a
    /// pure function of the clock: no host has to remember to tick it, and a
    /// host that never repaints simply never shows a stale toast.
    pub fn visible_toast(&self, now_ms: u64) -> Option<&EditorToastState> {
        self.editor_toast
            .as_ref()
            .filter(|toast| toast.is_visible(now_ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ui() -> EditorUiState {
        EditorUiState::default()
    }

    #[test]
    fn a_fresh_editor_has_no_toast() {
        assert!(ui().visible_toast(0).is_none());
    }

    #[test]
    fn a_raised_toast_is_visible_until_its_lifetime_runs_out() {
        let mut ui = ui();
        ui.show_toast(
            "collab.status.localEditPreserved",
            Vec::new(),
            EditorToastLevel::Warn,
            1_000,
        );

        let toast = ui.visible_toast(1_000).expect("visible when raised");
        assert_eq!(toast.i18n_key, "collab.status.localEditPreserved");
        assert_eq!(toast.level, EditorToastLevel::Warn);
        assert_eq!(toast.expires_at_ms(), 1_000 + EDITOR_TOAST_LIFETIME_MS);

        // The last millisecond still paints; the expiry instant itself does not.
        assert!(ui
            .visible_toast(1_000 + EDITOR_TOAST_LIFETIME_MS - 1)
            .is_some());
        assert!(ui.visible_toast(1_000 + EDITOR_TOAST_LIFETIME_MS).is_none());
    }

    #[test]
    fn an_expired_toast_is_hidden_without_being_swept() {
        // Expiry is a read-time filter, so the slot may still hold the value.
        // What matters is that nothing paints it and a later `show_toast`
        // replaces it cleanly.
        let mut ui = ui();
        ui.show_toast("a", Vec::new(), EditorToastLevel::Info, 0);
        assert!(ui.visible_toast(EDITOR_TOAST_LIFETIME_MS).is_none());

        ui.show_toast(
            "b",
            Vec::new(),
            EditorToastLevel::Info,
            EDITOR_TOAST_LIFETIME_MS,
        );
        assert_eq!(
            ui.visible_toast(EDITOR_TOAST_LIFETIME_MS)
                .map(|t| t.i18n_key.as_str()),
            Some("b")
        );
    }

    #[test]
    fn a_newer_toast_supersedes_the_one_on_screen() {
        // Single slot: no queue, no stacking. The second message is the one
        // that describes the current state of the document.
        let mut ui = ui();
        ui.show_toast("first", Vec::new(), EditorToastLevel::Info, 0);
        ui.show_toast(
            "second",
            vec![("name".into(), "Ada".into())],
            EditorToastLevel::Warn,
            10,
        );

        let toast = ui.visible_toast(10).expect("the newer toast is up");
        assert_eq!(toast.i18n_key, "second");
        assert_eq!(toast.level, EditorToastLevel::Warn);
        assert_eq!(
            toast.shown_at_ms, 10,
            "the clock restarts with the new message"
        );
        assert_eq!(toast.arg_pairs(), vec![("name", "Ada")]);
    }

    #[test]
    fn dismiss_empties_the_slot() {
        let mut ui = ui();
        ui.show_toast("x", Vec::new(), EditorToastLevel::Info, 0);
        ui.dismiss_toast();
        assert!(
            ui.editor_toast.is_none(),
            "dismiss must not leave a hidden value behind"
        );
        assert!(ui.visible_toast(0).is_none());
    }
}
