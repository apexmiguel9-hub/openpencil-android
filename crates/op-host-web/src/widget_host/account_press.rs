//! Sign-in modal + signed-in account-dropdown press dispatchers (web).
//!
//! The hit-test + state walk lives in the shared
//! `op_editor_ui::widgets::account_press_flow`; this file keeps only the
//! web platform arms. The web host cannot call the auth client directly
//! (the flow runs in the serving daemon), so presses either fire the
//! begin request inside the click's user-activation window or queue a
//! [`PendingAuthAction`] for `web_auth_sync` to drain on its poll tick.

use super::{PendingAuthAction, WidgetHost};
use op_editor_core::LoginFlowStatus;
use op_editor_ui::widgets::account_press_flow::{
    self as account_flow, AccountMenuPress, LoginModalPress,
};

impl WidgetHost {
    /// Sign-in-modal press dispatcher.
    pub(in crate::widget_host) fn dispatch_login_modal_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) {
        match account_flow::press_login_modal(&mut self.editor_state, x, y, viewport_w, viewport_h)
        {
            LoginModalPress::Closed => self.cancel_login_flow(),
            LoginModalPress::Dismissed => {
                self.blur_text_inputs_on_blank_press();
                self.cancel_login_flow();
            }
            LoginModalPress::SignIn => {
                // A flow is already running — don't spawn another
                // loading popup or re-begin; the poll owns the modal.
                if matches!(
                    self.editor_state.editor_ui.login_modal_status,
                    Some(
                        LoginFlowStatus::WaitingBrowser
                            | LoginFlowStatus::WaitingApproval
                            | LoginFlowStatus::Exchanging
                    )
                ) {
                    self.mark_dirty();
                    return;
                }
                // Open the loading popup AND fire the begin request NOW,
                // inside the click's user-activation window — an async
                // `window.open` later would be popup-blocked, and going
                // through the poll tick would add up to two poll cycles
                // of latency before the sso page appears.
                crate::web_auth_sync::begin_login_now();
                self.editor_state.editor_ui.login_modal_status =
                    Some(LoginFlowStatus::WaitingBrowser);
                self.editor_state.editor_ui.login_modal_stub_hint_shown = false;
            }
            LoginModalPress::Inside => {
                self.blur_text_inputs_on_blank_press();
            }
        }
        self.mark_dirty();
    }

    /// Signed-in account-dropdown press dispatcher.
    pub(in crate::widget_host) fn dispatch_account_menu_press(
        &mut self,
        x: f32,
        y: f32,
        viewport_w: f32,
        _viewport_h: f32,
    ) {
        match account_flow::press_account_menu(
            &mut self.editor_state,
            x,
            y,
            viewport_w,
            self.now_ms,
        ) {
            AccountMenuPress::Vanished => return,
            AccountMenuPress::OpenMcpTokens => {
                // The hub serves this editor at its own origin, so the
                // portal's per-account MCP-token page lives at
                // `/mcp-tokens` relative to the current origin. A new tab
                // opened synchronously inside the click's user-activation
                // window is not popup-blocked (same pattern as the
                // sign-in loading popup in `web_auth_sync`).
                crate::web_mcp_tokens::open_mcp_tokens_page();
            }
            AccountMenuPress::SignOut => {
                self.pending_auth_actions.push(PendingAuthAction::SignOut);
            }
            AccountMenuPress::Dismissed => {
                self.blur_text_inputs_on_blank_press();
            }
            AccountMenuPress::Handled | AccountMenuPress::Ignored => {}
        }
        self.mark_dirty();
    }

    /// Queue a cancel and clear the local progress note.
    pub(in crate::widget_host) fn cancel_login_flow(&mut self) {
        if self.editor_state.editor_ui.login_modal_status.is_some() {
            self.pending_auth_actions
                .push(PendingAuthAction::CancelLogin);
        }
        self.editor_state.editor_ui.login_modal_status = None;
    }
}
