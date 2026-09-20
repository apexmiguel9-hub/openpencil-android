//! Guest-side projection of the owner identity awaiting human confirmation.
//!
//! This is the one place in shared UI state that deliberately carries a peer's
//! account subject and device id. The guest has to see *who* it is about to
//! join before anything from the session is applied, and an opaque routing key
//! cannot answer that question. Everything published here comes from the
//! verified admission ticket; nothing peer-asserted outside it may enter.
//!
//! Account subject and device id are issuer-controlled and are therefore the
//! authoritative part of the projection. Display name and avatar URL are chosen
//! by the peer's own account, so they are attacker-influenced: they are kept in
//! separate, separately-labelled fields, stripped of the invisible formatting
//! characters that let one string masquerade as another, and are never allowed
//! to stand in for the authoritative identity.

use std::fmt;

use crate::collab_admission_ui::CollabAdmissionRequestKey;
use crate::{CollabConnectionPhase, CollabUiState};

/// Display bound for an issuer-controlled identifier.
pub const MAX_COLLAB_OWNER_IDENTIFIER_CHARS: usize = 72;
/// Display bound for the peer-chosen profile name.
pub const MAX_COLLAB_OWNER_DISPLAY_NAME_CHARS: usize = 40;

/// The verified owner identity a guest is being asked to accept.
#[derive(Clone, PartialEq, Eq)]
pub struct CollabOwnerIdentityUi {
    subject: String,
    device_id: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

impl CollabOwnerIdentityUi {
    /// Build a bounded projection from already-verified ticket claims.
    ///
    /// Returns `None` when the authoritative half is missing: an identity the
    /// guest cannot name is not a decision a human can make, so the caller must
    /// refuse the connection instead of prompting about a blank peer.
    pub fn from_verified(
        subject: &str,
        device_id: &str,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Option<Self> {
        let subject = bounded_identifier(subject)?;
        let device_id = bounded_identifier(device_id)?;
        Some(Self {
            subject,
            device_id,
            display_name: display_name.and_then(bounded_display_name),
            avatar_url: avatar_url.and_then(bounded_avatar_url),
        })
    }

    /// Issuer-controlled account identifier. Authoritative.
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Issuer-controlled device identifier. Authoritative.
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// Profile name chosen by the peer's own account. Never authoritative:
    /// any account may set any name, so this must always be presented as a
    /// claim next to the subject rather than as the peer's identity.
    pub fn claimed_display_name(&self) -> Option<&str> {
        self.display_name.as_deref()
    }

    /// Profile avatar chosen by the peer's own account. Same trust level as
    /// the claimed display name.
    pub fn claimed_avatar_url(&self) -> Option<&str> {
        self.avatar_url.as_deref()
    }
}

impl fmt::Debug for CollabOwnerIdentityUi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Visible on screen by design, redacted in crash reports and UI model
        // snapshots like every other identity projection.
        formatter
            .debug_struct("CollabOwnerIdentityUi")
            .field("subject", &"[REDACTED]")
            .field("device_id", &"[REDACTED]")
            .field(
                "display_name",
                &self.display_name.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "avatar_url",
                &self.avatar_url.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

/// One owner identity awaiting an explicit guest decision.
#[derive(Clone, PartialEq, Eq)]
pub struct PendingOwnerConfirmationUi {
    request_key: CollabAdmissionRequestKey,
    identity: CollabOwnerIdentityUi,
}

impl PendingOwnerConfirmationUi {
    pub fn new(request_key: CollabAdmissionRequestKey, identity: CollabOwnerIdentityUi) -> Self {
        Self {
            request_key,
            identity,
        }
    }

    pub fn request_key(&self) -> &CollabAdmissionRequestKey {
        &self.request_key
    }

    pub fn identity(&self) -> &CollabOwnerIdentityUi {
        &self.identity
    }
}

impl fmt::Debug for PendingOwnerConfirmationUi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingOwnerConfirmationUi")
            .field("request_key", &self.request_key)
            .field("identity", &self.identity)
            .finish()
    }
}

impl CollabUiState {
    /// The owner identity this guest must confirm before the session starts.
    ///
    /// Fails closed outside the pre-session authentication phase: once a
    /// session is live the decision has already been made, and a stale
    /// projection must not resurface as a second prompt.
    pub fn pending_owner_confirmation(&self) -> Option<&PendingOwnerConfirmationUi> {
        (self.phase == CollabConnectionPhase::Authenticating)
            .then_some(self.pending_owner_confirmation.as_ref())
            .flatten()
    }

    /// Publish the verified owner identity awaiting confirmation.
    ///
    /// Returns `false` unless the connection is authenticated but not yet
    /// admitted, which is the only window in which this decision exists.
    pub fn publish_owner_confirmation(
        &mut self,
        request_key: CollabAdmissionRequestKey,
        identity: CollabOwnerIdentityUi,
    ) -> bool {
        if self.phase != CollabConnectionPhase::Authenticating || self.authenticated.is_some() {
            return false;
        }
        let pending = PendingOwnerConfirmationUi::new(request_key, identity);
        if self.pending_owner_confirmation.as_ref() != Some(&pending) {
            self.panel.hover = None;
        }
        self.pending_owner_confirmation = Some(pending);
        true
    }

    pub fn clear_owner_confirmation(&mut self) {
        if self.pending_owner_confirmation.take().is_some() {
            self.panel.hover = None;
        }
    }
}

fn bounded_identifier(value: &str) -> Option<String> {
    let bounded: String = value
        .chars()
        .filter(|character| !is_invisible(*character))
        .take(MAX_COLLAB_OWNER_IDENTIFIER_CHARS)
        .collect();
    let bounded = bounded.trim().to_owned();
    (!bounded.is_empty()).then_some(bounded)
}

fn bounded_display_name(value: &str) -> Option<String> {
    let bounded: String = value
        .chars()
        .filter(|character| !character.is_control() && !is_invisible(*character))
        .take(MAX_COLLAB_OWNER_DISPLAY_NAME_CHARS)
        .collect();
    let bounded = bounded.trim().to_owned();
    (!bounded.is_empty()).then_some(bounded)
}

fn bounded_avatar_url(value: &str) -> Option<String> {
    // Only an https origin the profile pipeline would already accept. A guest
    // must never be shown a scheme it cannot reason about while deciding.
    (value.starts_with("https://")
        && value.is_ascii()
        && !value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace()))
    .then(|| value.to_owned())
}

/// Zero-width, bidirectional-override, and other invisible formatting
/// characters. They render as nothing while changing what a string appears to
/// say, which is exactly how a chosen display name would try to impersonate an
/// account identifier.
fn is_invisible(character: char) -> bool {
    matches!(character,
        '\u{00ad}'
        | '\u{061c}'
        | '\u{200b}'..='\u{200f}'
        | '\u{2028}'..='\u{202e}'
        | '\u{2060}'..='\u{2064}'
        | '\u{2066}'..='\u{206f}'
        | '\u{feff}'
        | '\u{fff9}'..='\u{fffb}')
        || matches!(character as u32, 0xe0000..=0xe007f)
}

#[cfg(test)]
#[path = "collab_owner_confirm_ui_tests.rs"]
mod tests;
