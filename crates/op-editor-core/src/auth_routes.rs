//! Device-login HTTP route paths shared by the web shell (client) and
//! the serve-web daemon (server).
//!
//! The wasm bundle ships no auth code — it drives the daemon's
//! device-login proxy over these routes. Keeping the paths in one
//! wasm-clean crate both sides already depend on means the client and
//! server can never drift apart.

/// Sign-in popup interstitial page (auth-exempt static HTML that shows
/// a spinner until the popup is navigated to the verification URI).
pub const LOADING_PAGE: &str = "/auth/loading";

/// Prefix shared by every JSON auth API route below (used by the
/// server's sensitive-POST / CORS gating).
pub const API_PREFIX: &str = "/api/auth/";

/// `GET` — session/status snapshot (seeds `account_ui_available`).
pub const STATUS: &str = "/api/auth/status";

/// `POST` — bounded current-account avatar bytes from the same-origin daemon.
///
/// JSON POST is intentional: unlike a GET, a cross-site image element cannot
/// trigger the daemon's upstream fetch without passing the existing
/// same-origin and JSON content-type gates.
pub const AVATAR: &str = "/api/auth/avatar";

/// `POST` — begin a device-login pairing. The daemon holds the request
/// until the pairing's verification URI is known.
pub const LOGIN_BEGIN: &str = "/api/auth/login/begin";

/// `GET` — poll the in-flight login pairing for approval progress.
pub const LOGIN_STATUS: &str = "/api/auth/login/status";

/// `POST` — cancel the in-flight login pairing.
pub const LOGIN_CANCEL: &str = "/api/auth/login/cancel";

/// `POST` — sign out of the current session.
pub const LOGOUT: &str = "/api/auth/logout";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_routes_share_the_gating_prefix() {
        for route in [
            STATUS,
            AVATAR,
            LOGIN_BEGIN,
            LOGIN_STATUS,
            LOGIN_CANCEL,
            LOGOUT,
        ] {
            assert!(
                route.starts_with(API_PREFIX),
                "{route} outside {API_PREFIX}"
            );
        }
        // The interstitial is deliberately outside the API prefix — it is
        // an auth-exempt static page, not a JSON route.
        assert!(!LOADING_PAGE.starts_with(API_PREFIX));
    }
}
