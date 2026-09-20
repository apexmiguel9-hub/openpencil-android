//! Daemon-side device-login proxy for the web editor.
//!
//! The wasm bundle ships no auth code: it drives the flow through these
//! REST handlers, and the daemon runs the proprietary `op-auth-bridge`
//! client on its behalf. The credential store is shared with the desktop
//! GUI (`~/.openpencil/auth`), so a session started in either host is
//! visible to both — signing in on the desktop signs the web editor in
//! too, and vice versa.
//!
//! Flow (browser side): `POST /api/auth/login/begin` → poll
//! `GET /api/auth/login/status` → open `verification_uri` in a popup when
//! it appears → keep polling until `signed_in` / a terminal error.

use op_auth_bridge::AuthStatus;
use op_editor_core::{AccountState, EditorState};

use crate::web_canvas_server::{WebCanvasState, WebReply};

/// Initialize the bridge runtime and restore a persisted session into the
/// daemon's editor state. Called once from `run_web_canvas` startup;
/// stub builds leave the account UI hidden exactly like the desktop.
///
/// `remote_exposure_ok` must be false when the daemon binds a
/// non-loopback interface outside managed mode: the daemon's session is
/// its owner's session, so exposing the proxy to the LAN would sign every
/// visitor in as the owner (and let them log the owner out).
pub(crate) fn init(editor: &mut EditorState, remote_exposure_ok: bool) {
    if !remote_exposure_ok || !op_auth_bridge::available() {
        return;
    }
    let Ok(dir) = op_config_store::openpencil_dir() else {
        return;
    };
    let config = op_auth_bridge::desktop_init_config(&dir, env!("CARGO_PKG_VERSION"));
    if !op_auth_bridge::init(&config) {
        return;
    }
    editor.editor_ui.account_ui_available = true;
    if op_auth_bridge::restore() {
        adopt_session(editor);
    }
}

fn adopt_session(editor: &mut EditorState) {
    if let AuthStatus::SignedIn {
        display_name,
        username,
        ..
    } = op_auth_bridge::poll(op_auth_bridge::SESSION_HANDLE)
    {
        editor.editor_ui.account = AccountState::signed_in_profile(display_name, username);
    }
}

fn ok() -> WebReply {
    WebReply {
        status: "200 OK",
        body: r#"{"ok":true}"#.into(),
    }
}

/// `GET /api/auth/status` — whether an auth backend is linked and the
/// current session, from the daemon's session handle. Also mirrors the
/// answer into the daemon's own editor state so it can't drift from
/// what this endpoint reports (e.g. a background revalidation dropped
/// the restored session).
pub(crate) fn status(state: &mut WebCanvasState) -> WebReply {
    let available = state.editor.editor_ui.account_ui_available;
    let body = match op_auth_bridge::poll(op_auth_bridge::SESSION_HANDLE) {
        AuthStatus::SignedIn {
            display_name,
            username,
            primary_email,
            avatar_url,
            ..
        } => {
            let avatar_revision = avatar_url
                .as_deref()
                .and_then(|url| crate::profile_avatar_fetch::profile_avatar_revision(url).ok());
            state.editor.editor_ui.account =
                AccountState::signed_in_profile(display_name.clone(), username.clone());
            serde_json::json!({
                "available": available,
                "signed_in": true,
                "display_name": display_name,
                "username": username,
                "primary_email": primary_email,
                "avatar_revision": avatar_revision,
            })
        }
        _ => {
            state.editor.editor_ui.account = AccountState::Anonymous;
            serde_json::json!({
                "available": available,
                "signed_in": false,
            })
        }
    };
    WebReply {
        status: "200 OK",
        body: body.to_string(),
    }
}

/// Interstitial served at `GET /auth/loading` — the sign-in popup opens
/// here (same origin, instant) and is navigated to the verification page
/// as soon as the begin call returns it, so the user never stares at a
/// bare `about:blank`.
///
/// The daemon cannot know the browser tab's editor locale, so the page
/// embeds the `account.openingPopup` string for every locale and picks
/// one from `navigator.languages` client-side (English no-JS fallback).
pub(crate) static LOADING_PAGE_HTML: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    let mut messages = serde_json::Map::new();
    for locale in op_i18n::Locale::ALL {
        messages.insert(
            locale.code().to_ascii_lowercase(),
            serde_json::Value::String(
                op_i18n::translate(locale, "account.openingPopup").to_string(),
            ),
        );
    }
    let messages = serde_json::Value::Object(messages).to_string();
    let fallback = op_i18n::translate(op_i18n::Locale::EnUs, "account.openingPopup");
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>OpenPencil</title><style>
  html,body{{height:100%;margin:0;background:#111113;color:#e4e4e7;
    font:14px/1.6 system-ui,-apple-system,sans-serif}}
  .wrap{{height:100%;display:flex;flex-direction:column;align-items:center;
    justify-content:center;gap:14px}}
  .spin{{width:28px;height:28px;border-radius:50%;border:3px solid #3f3f46;
    border-top-color:#818cf8;animation:r .8s linear infinite}}
  @keyframes r{{to{{transform:rotate(360deg)}}}}
  p{{margin:0}}
</style></head><body><div class="wrap"><div class="spin"></div>
<p id="msg">{fallback}</p></div><script>
(function () {{
  var messages = {messages};
  var langs = navigator.languages || [navigator.language || "en"];
  for (var i = 0; i < langs.length; i++) {{
    var tag = String(langs[i]).replace(/_/g, "-").toLowerCase();
    var pick = messages[tag];
    if (!pick && tag.indexOf("zh") === 0) {{
      if (/(^|-)hans($|-)/.test(tag)) {{
        pick = messages["zh-cn"];
      }} else if (/(^|-)hant($|-)/.test(tag)) {{
        pick = messages["zh-tw"];
      }} else {{
        pick = /(^|-)(tw|hk|mo)($|-)/.test(tag)
          ? messages["zh-tw"] : messages["zh-cn"];
      }}
    }}
    if (!pick) {{ pick = messages[tag.split("-")[0]]; }}
    if (!pick && tag.indexOf("en") === 0) {{ pick = messages["en-us"]; }}
    if (pick) {{
      document.getElementById("msg").textContent = pick;
      break;
    }}
  }}
}})();
</script></body></html>"#
    )
});

/// How long `login_begin_and_wait` blocks its connection thread waiting
/// for the pairing's verification URI (the sso `start` round-trip).
const BEGIN_WAIT_STEPS: u32 = 50;
const BEGIN_WAIT_STEP_MS: u64 = 100;

/// `POST /api/auth/login/begin` (streaming tier) — start the flow, then
/// wait (off the state lock; only this connection's thread blocks) until
/// the verification URI is known and return it inline. The popup can
/// then be navigated from the begin response itself instead of waiting
/// out an extra poll cycle. On timeout/error the reply omits the URI and
/// the browser's status polling takes over.
pub(crate) fn login_begin_and_wait(state: &std::sync::Mutex<WebCanvasState>) -> WebReply {
    let handle = {
        let mut guard = state.lock().unwrap_or_else(|p| p.into_inner());
        let reply = login_begin(&mut guard);
        if reply.status != "200 OK" {
            return reply;
        }
        match guard.auth_login_handle {
            Some(handle) => handle,
            None => return reply,
        }
    };
    for _ in 0..BEGIN_WAIT_STEPS {
        match op_auth_bridge::poll(handle) {
            AuthStatus::WaitingApproval { verification_uri } if !verification_uri.is_empty() => {
                return WebReply {
                    status: "200 OK",
                    body: serde_json::json!({
                        "ok": true,
                        "verification_uri": verification_uri,
                    })
                    .to_string(),
                };
            }
            AuthStatus::Idle | AuthStatus::Starting | AuthStatus::WaitingApproval { .. } => {}
            // Terminal already (fast failure / instant approval): let the
            // status poll report it.
            _ => break,
        }
        std::thread::sleep(std::time::Duration::from_millis(BEGIN_WAIT_STEP_MS));
    }
    ok()
}

/// Start (or join) the browser pairing flow. Idempotent while a flow is
/// running.
pub(crate) fn login_begin(state: &mut WebCanvasState) -> WebReply {
    // The editor flag is set only when `init` actually ran (auth backend
    // linked AND the bind is loopback/managed), so this also refuses the
    // proxy on an exposed non-managed daemon.
    if !state.editor.editor_ui.account_ui_available {
        return WebReply {
            status: "403 Forbidden",
            body: crate::mcp_serve::rest_error_body("device-login proxy unavailable"),
        };
    }
    if state.auth_login_handle.is_none() {
        let handle = op_auth_bridge::login_begin();
        if handle == 0 {
            return WebReply {
                status: "500 Internal Server Error",
                body: crate::mcp_serve::rest_error_body("auth runtime failed to start a flow"),
            };
        }
        state.auth_login_handle = Some(handle);
    }
    ok()
}

/// `GET /api/auth/login/status` — poll the in-flight flow. Terminal
/// states clear the stored handle; `signed_in` also updates the daemon's
/// editor account state.
pub(crate) fn login_status(state: &mut WebCanvasState) -> WebReply {
    let Some(handle) = state.auth_login_handle else {
        return WebReply {
            status: "200 OK",
            body: serde_json::json!({ "state": "idle" }).to_string(),
        };
    };
    let body = match op_auth_bridge::poll(handle) {
        AuthStatus::Idle | AuthStatus::Starting => serde_json::json!({ "state": "starting" }),
        AuthStatus::WaitingApproval { verification_uri } => serde_json::json!({
            "state": "waiting_approval",
            "verification_uri": verification_uri,
        }),
        AuthStatus::Exchanging => serde_json::json!({ "state": "exchanging" }),
        AuthStatus::SignedIn {
            display_name,
            username,
            primary_email,
            avatar_url,
            ..
        } => {
            let avatar_revision = avatar_url
                .as_deref()
                .and_then(|url| crate::profile_avatar_fetch::profile_avatar_revision(url).ok());
            state.auth_login_handle = None;
            state.editor.editor_ui.account =
                AccountState::signed_in_profile(display_name.clone(), username.clone());
            serde_json::json!({
                "state": "signed_in",
                "display_name": display_name,
                "username": username,
                "primary_email": primary_email,
                "avatar_revision": avatar_revision,
            })
        }
        AuthStatus::Error { code } => {
            state.auth_login_handle = None;
            serde_json::json!({ "state": "error", "code": code })
        }
        AuthStatus::Canceled => {
            state.auth_login_handle = None;
            serde_json::json!({ "state": "canceled" })
        }
    };
    WebReply {
        status: "200 OK",
        body: body.to_string(),
    }
}

/// `POST /api/auth/login/cancel` — abort an in-flight flow (modal closed
/// browser-side). No-op when nothing is running.
pub(crate) fn login_cancel(state: &mut WebCanvasState) -> WebReply {
    if let Some(handle) = state.auth_login_handle.take() {
        op_auth_bridge::cancel(handle);
    }
    ok()
}

/// `POST /api/auth/avatar` — proxy the current profile image through the
/// daemon's public-only HTTPS client. The browser receives only an opaque
/// revision and bounded bytes, never the signed CDN URL.
pub(crate) fn avatar() -> WebReply {
    if !op_auth_bridge::available() {
        return WebReply {
            status: "404 Not Found",
            body: crate::mcp_serve::rest_error_body("account avatar unavailable"),
        };
    }
    let AuthStatus::SignedIn {
        avatar_url: Some(url),
        ..
    } = op_auth_bridge::poll(op_auth_bridge::SESSION_HANDLE)
    else {
        return WebReply {
            status: "404 Not Found",
            body: crate::mcp_serve::rest_error_body("account avatar unavailable"),
        };
    };
    let Ok(revision) = crate::profile_avatar_fetch::profile_avatar_revision(&url) else {
        return WebReply {
            status: "404 Not Found",
            body: crate::mcp_serve::rest_error_body("account avatar unavailable"),
        };
    };
    let Ok(bytes) = crate::profile_avatar_fetch::fetch_account_avatar_blocking(&url) else {
        return WebReply {
            status: "502 Bad Gateway",
            body: crate::mcp_serve::rest_error_body("account avatar fetch failed"),
        };
    };
    use base64::Engine as _;
    WebReply {
        status: "200 OK",
        body: serde_json::json!({
            "revision": revision,
            "encoded": base64::engine::general_purpose::STANDARD.encode(bytes),
        })
        .to_string(),
    }
}

/// `POST /api/auth/logout` — drop the shared session (revokes the device
/// token server-side on a background thread inside the library).
pub(crate) fn logout(state: &mut WebCanvasState) -> WebReply {
    op_auth_bridge::sign_out();
    state.editor.editor_ui.account = AccountState::Anonymous;
    ok()
}

#[cfg(test)]
mod loading_page_i18n_tests {
    use super::LOADING_PAGE_HTML;

    #[test]
    fn loading_interstitial_embeds_every_locale_and_an_english_fallback() {
        let html = LOADING_PAGE_HTML.as_str();
        let fallback = op_i18n::translate(op_i18n::Locale::EnUs, "account.openingPopup");
        assert!(html.contains(&format!(r#"<p id="msg">{fallback}</p>"#)));
        assert!(!html.contains("<p>正在打开登录页…</p>"));

        for locale in op_i18n::Locale::ALL {
            let key = locale.code().to_ascii_lowercase();
            let value = serde_json::to_string(op_i18n::translate(locale, "account.openingPopup"))
                .expect("translation serializes");
            assert!(
                html.contains(&format!(r#""{key}":{value}"#)),
                "loading page omits locale {}",
                locale.code()
            );
        }
    }

    #[test]
    fn loading_interstitial_prefers_explicit_chinese_script_tags() {
        let html = LOADING_PAGE_HTML.as_str();
        let hans = html
            .find(r#"/(^|-)hans($|-)/"#)
            .expect("Hans selector is embedded");
        let hant = html
            .find(r#"/(^|-)hant($|-)/"#)
            .expect("Hant selector is embedded");
        let region = html
            .find(r#"/(^|-)(tw|hk|mo)($|-)/"#)
            .expect("Chinese region fallback is embedded");
        assert!(hans < region && hant < region);
    }
}
