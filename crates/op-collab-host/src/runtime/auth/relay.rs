use std::sync::{Arc, RwLock};

use op_collab_relay_client::{
    ChallengeBoundRelayAuthenticator, PinnedRelayX25519Keys, RelayAuthError, RelayAuthenticator,
    RelayClientX25519Agreement, RelayCredential, RelayCredentialProvider,
};
use op_collab_relay_protocol::{CallerDeviceDhPublic, RelayAuthExtensionV1};
use op_collab_transport::DeviceStaticKey;
use zeroize::Zeroizing;

use super::{LocalAdmission, RelayBearerCredentialSource};
use crate::runtime::types::{CollabRuntimeError, CollabRuntimeFailure};

struct SharedLocalRelayCredentials {
    admission: Arc<RwLock<LocalAdmission>>,
}

impl RelayCredentialProvider for SharedLocalRelayCredentials {
    fn credential(&self) -> Result<RelayCredential, RelayAuthError> {
        let admission = self
            .admission
            .read()
            .map_err(|_| RelayAuthError::Unavailable)?;
        relay_credential(admission.relay_bearer_bytes())
    }
}

struct DeviceRelayKeyAgreement {
    key: Arc<DeviceStaticKey>,
}

impl RelayClientX25519Agreement for DeviceRelayKeyAgreement {
    fn caller_public_key(&self) -> Result<CallerDeviceDhPublic, RelayAuthError> {
        CallerDeviceDhPublic::new(*self.key.public_key()).map_err(|_| RelayAuthError::KeyAgreement)
    }

    fn agree(&self, relay_public_key: &[u8; 32]) -> Result<Zeroizing<[u8; 32]>, RelayAuthError> {
        self.key
            .agree_x25519(relay_public_key)
            .map_err(|_| RelayAuthError::KeyAgreement)
    }
}

impl LocalAdmission {
    pub(in crate::runtime) const fn relay_ticket(&self) -> &op_auth_bridge::OpaqueCollabTicket {
        &self.ticket
    }

    /// The bytes the relay is handed as its `Authorization: Bearer`.
    ///
    /// Normally the claim-minimized relay token. It falls back to the full
    /// ticket only when the linked auth ABI predates the relay-token
    /// capability — a build-time property decided once at session start, never
    /// a retry in response to a relay rejection.
    pub(in crate::runtime) fn relay_bearer_bytes(&self) -> &[u8] {
        match &self.relay_bearer {
            RelayBearerCredentialSource::MinimizedRelayToken(token) => token.expose(),
            RelayBearerCredentialSource::LegacyFullTicket => self.ticket.expose(),
        }
    }

    /// Whether this session's relay bearer carries no identity claims.
    ///
    /// Test-only: nothing in the runtime may branch on this. The bearer choice
    /// is made once when the admission is built and must never be re-decided
    /// per connection attempt, which is what a runtime caller would invite.
    #[cfg(test)]
    pub(in crate::runtime) const fn uses_minimized_relay_bearer(&self) -> bool {
        matches!(
            self.relay_bearer,
            RelayBearerCredentialSource::MinimizedRelayToken(_)
        )
    }

    pub(in crate::runtime) fn install_shared_relay_renewal(
        admission: &RwLock<Self>,
        renewed: Self,
    ) -> Result<(), CollabRuntimeError> {
        let mut current = admission.write().map_err(|_| {
            CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
        })?;
        *current = renewed;
        Ok(())
    }

    /// Each relay attempt reads the latest locally verified ticket. Owner lane
    /// replacement and guest relay reauthentication therefore never reuse the
    /// bearer captured at session start.
    pub(in crate::runtime) fn shared_relay_credentials(
        admission: Arc<RwLock<Self>>,
    ) -> Result<Arc<dyn RelayCredentialProvider>, CollabRuntimeError> {
        {
            let current = admission.read().map_err(|_| {
                CollabRuntimeError::new(CollabRuntimeFailure::AuthenticationUnavailable)
            })?;
            relay_credential(current.relay_bearer_bytes()).map_err(|_| relay_ticket_rejected())?;
        }
        Ok(Arc::new(SharedLocalRelayCredentials { admission }))
    }

    pub(in crate::runtime) fn challenge_bound_relay_authenticator(
        admission: Arc<RwLock<Self>>,
        key: Arc<DeviceStaticKey>,
        relay_keys: Arc<PinnedRelayX25519Keys>,
    ) -> Result<Arc<dyn RelayAuthenticator>, CollabRuntimeError> {
        let credentials = Self::shared_relay_credentials(admission)?;
        let key_agreement: Arc<dyn RelayClientX25519Agreement> =
            Arc::new(DeviceRelayKeyAgreement { key });
        Ok(Arc::new(ChallengeBoundRelayAuthenticator::new(
            credentials,
            key_agreement,
            relay_keys,
        )))
    }

    /// Reduced-assurance/debug handshakes still need the caller public key.
    /// Strict production replaces this extension with its challenge-bound v2
    /// proof inside the relay client.
    pub(in crate::runtime) fn relay_auth_extension(
        local_static: [u8; 32],
    ) -> Result<RelayAuthExtensionV1, CollabRuntimeError> {
        let caller = CallerDeviceDhPublic::new(local_static).map_err(|_| relay_protocol_error())?;
        Ok(RelayAuthExtensionV1::without_possession_proof(caller))
    }
}

fn relay_credential(ticket: &[u8]) -> Result<RelayCredential, RelayAuthError> {
    let ticket = std::str::from_utf8(ticket).map_err(|_| RelayAuthError::InvalidCredential)?;
    let mut copy = String::with_capacity(ticket.len());
    copy.push_str(ticket);
    RelayCredential::bearer(copy)
}

fn relay_protocol_error() -> CollabRuntimeError {
    CollabRuntimeError::new(CollabRuntimeFailure::RelayUnavailable)
}

fn relay_ticket_rejected() -> CollabRuntimeError {
    CollabRuntimeError::new(CollabRuntimeFailure::TicketRejected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_auth_bridge::{OpaqueCollabRelayToken, OpaqueCollabTicket};
    use op_collab::VerifiedAuthMetadata;

    fn metadata() -> VerifiedAuthMetadata {
        VerifiedAuthMetadata {
            issuer: "https://issuer.example".to_owned(),
            subject: "subject".to_owned(),
            device_id: "device".to_owned(),
            proof_binding: "binding".to_owned(),
            expires_at_unix_ms: 10_000,
            display_name: None,
            avatar_url: None,
        }
    }

    fn admission(relay_token: &str) -> LocalAdmission {
        LocalAdmission {
            ticket: OpaqueCollabTicket::new(b"ticket.payload.signature".to_vec()).unwrap(),
            relay_bearer: RelayBearerCredentialSource::MinimizedRelayToken(
                OpaqueCollabRelayToken::new(relay_token.as_bytes().to_vec()).unwrap(),
            ),
            auth: metadata(),
        }
    }

    fn legacy_admission(ticket: &str) -> LocalAdmission {
        LocalAdmission {
            ticket: OpaqueCollabTicket::new(ticket.as_bytes().to_vec()).unwrap(),
            relay_bearer: RelayBearerCredentialSource::LegacyFullTicket,
            auth: metadata(),
        }
    }

    #[test]
    fn relay_credential_provider_reads_the_latest_shared_relay_token() {
        let shared = Arc::new(RwLock::new(admission("header.payload.signature")));
        let provider = LocalAdmission::shared_relay_credentials(Arc::clone(&shared)).unwrap();
        let first = provider.credential().unwrap();
        assert!(!format!("{first:?}").contains("header"));
        LocalAdmission::install_shared_relay_renewal(
            shared.as_ref(),
            admission("invalid,credential"),
        )
        .unwrap();
        assert_eq!(
            provider.credential().unwrap_err(),
            RelayAuthError::InvalidCredential
        );
    }

    #[test]
    fn relay_bearer_is_the_minimized_token_and_never_the_identity_ticket() {
        let shared = Arc::new(RwLock::new(admission("relay.token.signature")));
        {
            let admission = shared.read().unwrap();
            assert!(admission.uses_minimized_relay_bearer());
            assert_eq!(admission.relay_bearer_bytes(), b"relay.token.signature");
            // The full ticket stays behind for Noise-channel peer admission.
            assert_eq!(
                admission.relay_ticket().expose(),
                b"ticket.payload.signature"
            );
            assert_ne!(
                admission.relay_bearer_bytes(),
                admission.relay_ticket().expose()
            );
        }
    }

    #[test]
    fn pre_capability_builds_fall_back_to_the_dual_accepted_full_ticket() {
        let admission = legacy_admission("header.payload.signature");
        assert!(!admission.uses_minimized_relay_bearer());
        assert_eq!(admission.relay_bearer_bytes(), b"header.payload.signature");
    }

    #[test]
    fn relay_extension_carries_the_ticket_bound_device_public_key() {
        let public = [7_u8; 32];
        let extension = LocalAdmission::relay_auth_extension(public).unwrap();
        assert_eq!(extension.caller_device_dh_pub_x25519().as_bytes(), &public);
        assert!(extension.possession_proof().is_none());
    }

    #[test]
    fn device_agreement_uses_the_static_key_without_exporting_it() {
        let device = Arc::new(DeviceStaticKey::from_private([3_u8; 32]).unwrap());
        let relay = DeviceStaticKey::from_private([5_u8; 32]).unwrap();
        let agreement = DeviceRelayKeyAgreement {
            key: Arc::clone(&device),
        };
        assert_eq!(
            agreement.agree(relay.public_key()).unwrap().as_ref(),
            relay.agree_x25519(device.public_key()).unwrap().as_ref()
        );
    }
}
