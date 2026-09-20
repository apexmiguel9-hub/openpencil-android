//! Public-relay collaboration values safe to project into shared UI state.
//!
//! Invite codes are bearer routing capabilities. They may be rendered or
//! copied only in an explicit owner/join flow, and are always redacted from
//! debug output.

/// Maximum invite size accepted by editor state before protocol parsing.
pub const MAX_COLLAB_INVITE_CODE_CHARS: usize = 1_024;

/// A bounded, redacted OpenPencil collaboration invite.
#[derive(Clone, PartialEq, Eq)]
pub struct CollabInviteCode(String);

/// Crockford base32 alphabet of the 10-char short pairing code. Kept in sync
/// with `op_collab_relay_protocol::PAIRING_CODE_ALPHABET`; duplicated so the
/// wasm-clean editor core does not grow a protocol-crate dependency.
const PAIRING_CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const PAIRING_CODE_CHARS: usize = 10;

impl CollabInviteCode {
    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        let long_invite = value.starts_with("opc1_")
            && value.chars().count() <= MAX_COLLAB_INVITE_CODE_CHARS
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
        let pairing_code = value.len() == PAIRING_CODE_CHARS
            && value
                .bytes()
                .all(|byte| PAIRING_CODE_ALPHABET.contains(&byte));
        (long_invite || pairing_code).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for CollabInviteCode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("CollabInviteCode([REDACTED])")
    }
}

/// The region that owns the relay route for the lifetime of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollabRelayRegion {
    #[default]
    China,
    Global,
}

impl CollabRelayRegion {
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::China => "collab.region.china",
            Self::Global => "collab.region.global",
        }
    }
}

/// Authenticated display-only connection path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabConnectionPathUi {
    Lan,
    Relay { home_region: CollabRelayRegion },
}

/// Owner-only invite plus the authenticated path used by this peer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollabPublicSessionUi {
    invite: Option<CollabInviteCode>,
    connection: Option<CollabConnectionPathUi>,
}

impl CollabPublicSessionUi {
    pub fn invite(&self) -> Option<&CollabInviteCode> {
        self.invite.as_ref()
    }

    pub const fn connection(&self) -> Option<CollabConnectionPathUi> {
        self.connection
    }
}

impl CollabConnectionPathUi {
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::Lan => "collab.connection.lan",
            Self::Relay { .. } => "collab.connection.relay",
        }
    }

    pub const fn home_region(self) -> Option<CollabRelayRegion> {
        match self {
            Self::Lan => None,
            Self::Relay { home_region } => Some(home_region),
        }
    }
}

/// Public connection setup failures. These are deliberately separate from
/// authenticated-session disconnect/reject notices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollabConnectErrorUi {
    InviteUnavailable,
    /// The invite string itself is malformed — it never named a route.
    InviteInvalid,
    /// The invite was authentic but its pairing window is over.
    InviteExpired,
    RelayUnavailable,
    /// This device has no relay bootstrap configured; connecting cannot work
    /// until the app is set up, so the copy must not suggest waiting.
    RelayNotConfigured,
    RegionUnavailable,
    RateLimited,
    Incompatible,
    /// The platform secure key store refused or lost this device's static
    /// key; no connection was attempted, so the copy must not read as a
    /// transient network drop.
    SecureKeyUnavailable,
    /// The guest was shown the verified owner identity and declined it, or the
    /// prompt went unanswered. No session data was ever applied.
    OwnerNotConfirmed,
}

impl CollabConnectErrorUi {
    pub const fn i18n_key(self) -> &'static str {
        match self {
            Self::InviteUnavailable => "collab.error.inviteUnavailable",
            Self::InviteInvalid => "collab.error.inviteInvalid",
            Self::InviteExpired => "collab.error.inviteExpired",
            Self::RelayUnavailable => "collab.error.relayUnavailable",
            Self::RelayNotConfigured => "collab.error.relayNotConfigured",
            Self::RegionUnavailable => "collab.error.regionUnavailable",
            Self::RateLimited => "collab.error.rateLimited",
            Self::Incompatible => "collab.join.incompatible",
            Self::SecureKeyUnavailable => "collab.error.secureKeyUnavailable",
            Self::OwnerNotConfirmed => "collab.error.ownerNotConfirmed",
        }
    }
}

impl crate::CollabUiState {
    pub fn public_session(&self) -> Option<&CollabPublicSessionUi> {
        self.phase
            .is_authenticated()
            .then_some(&self.public_session)
    }

    /// Publish relay display data only after peer admission. Guests may see
    /// their connection path, but never retain an owner bearer invite.
    pub fn set_public_session(
        &mut self,
        invite: Option<CollabInviteCode>,
        connection: CollabConnectionPathUi,
    ) {
        let owner = self
            .authenticated_session()
            .is_some_and(|session| session.role == crate::CollabUiRole::Owner);
        let public_session = CollabPublicSessionUi {
            invite: owner.then_some(invite).flatten(),
            connection: Some(connection),
        };
        if self.public_session != public_session {
            self.panel.hover = None;
            self.public_session = public_session;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_is_bounded_and_debug_redacted() {
        let raw = "opc1_super-secret-routing-capability";
        let invite = CollabInviteCode::new(raw).expect("valid invite");
        assert_eq!(invite.as_str(), raw);
        let debug = format!("{invite:?}");
        assert!(!debug.contains("super-secret"));
        assert!(debug.contains("[REDACTED]"));
        assert!(CollabInviteCode::new("not-an-openpencil-invite").is_none());
        // 10-char short pairing codes are displayable invites too.
        assert!(CollabInviteCode::new("A2C4E6G8J0").is_some());
        assert!(
            CollabInviteCode::new("A2C4E6G8J").is_none(),
            "length is exact"
        );
        assert!(
            CollabInviteCode::new("A2C4E6G8JU").is_none(),
            "U is outside the Crockford alphabet"
        );
        assert!(
            CollabInviteCode::new("a2c4e6g8j0").is_none(),
            "display form is canonical uppercase"
        );
        assert!(CollabInviteCode::new(format!(
            "opc1_{}",
            "a".repeat(MAX_COLLAB_INVITE_CODE_CHARS)
        ))
        .is_none());
    }
}
