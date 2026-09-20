//! Collaboration HTTP route paths shared by the web shell (client) and the
//! serve-web daemon (server).
//!
//! The wasm bundle carries the collaboration *UI* but no transport: sessions
//! run inside the daemon, which is a native process and can hold the device
//! credentials a relay session needs. The browser drives it over these routes.
//! Keeping the paths in one wasm-clean crate both sides already depend on
//! means the client and server can never drift apart — the same reasoning as
//! [`auth_routes`](crate::auth_routes).

/// Prefix shared by every JSON collaboration API route below.
///
/// The daemon's sensitive-POST and CORS gates key off this prefix, so a route
/// added outside it would silently escape them.
pub const API_PREFIX: &str = "/api/collab/";

/// `GET` — the whole collaboration UI projection plus the two sequence
/// numbers a client polls on (`collabSeq`, `documentRevision`).
pub const STATE: &str = "/api/collab/state";

/// `POST` — enqueue one collaboration UI action.
///
/// Accepted actions are a versioned wire enum, not the internal
/// `CollabUiAction`; see [`collab_wire`](crate::collab_wire).
pub const ACTION: &str = "/api/collab/action";

/// `POST` — publish the local cursor / viewport for peer presence.
pub const PRESENCE: &str = "/api/collab/presence";

/// `POST` — proxy one roster participant's profile image.
///
/// The browser sends only the epoch-local `participantKey` it already has from
/// [`STATE`] and receives bounded bytes plus an opaque revision. Verified
/// roster avatar URLs stay inside the daemon, which owns the public-only HTTPS
/// client — the same split as
/// [`auth_routes::AVATAR`](crate::auth_routes::AVATAR).
pub const AVATAR: &str = "/api/collab/avatar";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_routes_share_the_gating_prefix() {
        for route in [STATE, ACTION, PRESENCE, AVATAR] {
            assert!(
                route.starts_with(API_PREFIX),
                "{route} outside {API_PREFIX}"
            );
        }
    }

    #[test]
    fn collaboration_routes_do_not_collide_with_the_auth_family() {
        assert_ne!(API_PREFIX, crate::auth_routes::API_PREFIX);
        for route in [STATE, ACTION, PRESENCE, AVATAR] {
            assert!(!route.starts_with(crate::auth_routes::API_PREFIX));
        }
    }
}
