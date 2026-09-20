//! The versioned action enum a client may post to the daemon.
//!
//! This is a separate type from [`CollabUiAction`] on purpose. The internal
//! enum carries [`CollabAdmissionRequestKey`], whose whole contract is that it
//! is an opaque routing token built only through a validating constructor;
//! deriving `Serialize`/`Deserialize` on it would let any request body
//! materialise one. Here the key crosses the wire as a plain string and is
//! re-validated on the way in, so a malformed or oversized token is rejected
//! at the boundary instead of reaching the session actor.

use serde::{Deserialize, Serialize};

use super::parts::CollabRelayRegionWire;
use crate::{CollabAdmissionRequestKey, CollabUiAction};

/// Longest LAN endpoint or discovery id the wire accepts.
///
/// Both are consumed by the runtime's address parser; the cap keeps an
/// unbounded body from reaching it.
const MAX_WIRE_ADDRESS_CHARS: usize = 255;

/// Why a posted action was refused before it reached the runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabActionWireError {
    /// The admission token is not a well-formed opaque key.
    InvalidRequestKey,
    /// A discovery id or endpoint was empty or over [`MAX_WIRE_ADDRESS_CHARS`].
    InvalidAddress,
    /// The action is a direct runtime request, not a queued panel action.
    NotAUiAction,
}

/// What a posted action asks the host to do.
///
/// Most actions are queued into the panel's pending slot and drained by the
/// runtime pump; undo is a direct call on the session. Splitting them keeps
/// the caller from having to special-case a variant the pending slot cannot
/// represent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollabWireCommand {
    /// Queue this into `CollabUiState::pending_action`.
    Ui(CollabUiAction),
    /// Call the runtime's selective-undo entry point.
    RequestUndo,
}

impl std::fmt::Display for CollabActionWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRequestKey => f.write_str("malformed collaboration admission key"),
            Self::InvalidAddress => f.write_str("malformed collaboration address"),
            Self::NotAUiAction => f.write_str("action is not a queued panel action"),
        }
    }
}

impl std::error::Error for CollabActionWireError {}

/// One collaboration action, as posted by a client.
///
/// Externally tagged on `type` so an unknown action fails deserialization
/// loudly rather than silently decoding as a neighbouring variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CollabActionWire {
    OpenCreate,
    Start,
    StartLan,
    #[serde(rename_all = "camelCase")]
    SetRelayRegion {
        region: CollabRelayRegionWire,
    },
    OpenJoin,
    BeginDiscovery,
    #[serde(rename_all = "camelCase")]
    JoinDiscovered {
        discovery_id: String,
    },
    #[serde(rename_all = "camelCase")]
    JoinAddress {
        endpoint: String,
    },
    Cancel,
    Retry,
    Leave,
    DiscardPending,
    ReapplyDiscarded,
    SaveAsFork,
    /// Ask the session for an M1 selective undo of this peer's latest edit.
    ///
    /// Additive: a daemon that does not know this variant rejects the body,
    /// which is the correct answer — the client must then fall back to a local
    /// undo rather than assume the request landed.
    RequestUndo,
    #[serde(rename_all = "camelCase")]
    ApproveAdmissionEditor {
        request_key: String,
    },
    #[serde(rename_all = "camelCase")]
    ApproveAdmissionViewer {
        request_key: String,
    },
    #[serde(rename_all = "camelCase")]
    RejectAdmission {
        request_key: String,
    },
    #[serde(rename_all = "camelCase")]
    ConfirmOwnerIdentity {
        request_key: String,
    },
    #[serde(rename_all = "camelCase")]
    RejectOwnerIdentity {
        request_key: String,
    },
}

impl CollabActionWire {
    /// Whether this action opens a connection to something the caller named.
    ///
    /// A public deployment must refuse these: `JoinAddress` resolves an
    /// arbitrary socket address, and the LAN paths expose the host's local
    /// network. The local and managed daemons allow them — that is desktop
    /// parity — so this is a classifier, not a policy. The public-mode route
    /// table is what consults it.
    pub const fn reaches_caller_named_network(&self) -> bool {
        matches!(
            self,
            Self::StartLan
                | Self::BeginDiscovery
                | Self::JoinDiscovered { .. }
                | Self::JoinAddress { .. }
        )
    }

    /// Validate and convert into the internal command.
    pub fn into_command(self) -> Result<CollabWireCommand, CollabActionWireError> {
        if matches!(self, Self::RequestUndo) {
            return Ok(CollabWireCommand::RequestUndo);
        }
        Ok(CollabWireCommand::Ui(self.into_ui_action()?))
    }

    /// Validate and convert into the internal UI action.
    ///
    /// `RequestUndo` has no `CollabUiAction` form — the runtime exposes undo as
    /// a direct call, not a queued panel action — so it is the one variant this
    /// cannot produce. Use [`Self::into_command`] to handle both.
    pub fn into_ui_action(self) -> Result<CollabUiAction, CollabActionWireError> {
        Ok(match self {
            Self::RequestUndo => return Err(CollabActionWireError::NotAUiAction),
            Self::OpenCreate => CollabUiAction::OpenCreate,
            Self::Start => CollabUiAction::Start,
            Self::StartLan => CollabUiAction::StartLan,
            Self::SetRelayRegion { region } => CollabUiAction::SetRelayRegion {
                region: region.into(),
            },
            Self::OpenJoin => CollabUiAction::OpenJoin,
            Self::BeginDiscovery => CollabUiAction::BeginDiscovery,
            Self::JoinDiscovered { discovery_id } => CollabUiAction::JoinDiscovered {
                discovery_id: checked_address(discovery_id)?,
            },
            Self::JoinAddress { endpoint } => CollabUiAction::JoinAddress {
                endpoint: checked_address(endpoint)?,
            },
            Self::Cancel => CollabUiAction::Cancel,
            Self::Retry => CollabUiAction::Retry,
            Self::Leave => CollabUiAction::Leave,
            Self::DiscardPending => CollabUiAction::DiscardPending,
            Self::ReapplyDiscarded => CollabUiAction::ReapplyDiscarded,
            Self::SaveAsFork => CollabUiAction::SaveAsFork,
            Self::ApproveAdmissionEditor { request_key } => {
                CollabUiAction::ApproveAdmissionEditor {
                    request_key: checked_request_key(&request_key)?,
                }
            }
            Self::ApproveAdmissionViewer { request_key } => {
                CollabUiAction::ApproveAdmissionViewer {
                    request_key: checked_request_key(&request_key)?,
                }
            }
            Self::RejectAdmission { request_key } => CollabUiAction::RejectAdmission {
                request_key: checked_request_key(&request_key)?,
            },
            Self::ConfirmOwnerIdentity { request_key } => CollabUiAction::ConfirmOwnerIdentity {
                request_key: checked_request_key(&request_key)?,
            },
            Self::RejectOwnerIdentity { request_key } => CollabUiAction::RejectOwnerIdentity {
                request_key: checked_request_key(&request_key)?,
            },
        })
    }
}

fn checked_request_key(value: &str) -> Result<CollabAdmissionRequestKey, CollabActionWireError> {
    CollabAdmissionRequestKey::new(value).ok_or(CollabActionWireError::InvalidRequestKey)
}

fn checked_address(value: String) -> Result<String, CollabActionWireError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_WIRE_ADDRESS_CHARS {
        return Err(CollabActionWireError::InvalidAddress);
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_internal_action_has_a_wire_variant() {
        // Exhaustive by construction: adding a `CollabUiAction` variant makes
        // this match fail to compile, which is the reminder to extend the wire.
        let sample = CollabUiAction::OpenCreate;
        let covered = match sample {
            CollabUiAction::OpenCreate
            | CollabUiAction::Start
            | CollabUiAction::StartLan
            | CollabUiAction::SetRelayRegion { .. }
            | CollabUiAction::OpenJoin
            | CollabUiAction::BeginDiscovery
            | CollabUiAction::JoinDiscovered { .. }
            | CollabUiAction::JoinAddress { .. }
            | CollabUiAction::Cancel
            | CollabUiAction::Retry
            | CollabUiAction::Leave
            | CollabUiAction::DiscardPending
            | CollabUiAction::ReapplyDiscarded
            | CollabUiAction::SaveAsFork
            | CollabUiAction::ApproveAdmissionEditor { .. }
            | CollabUiAction::ApproveAdmissionViewer { .. }
            | CollabUiAction::RejectAdmission { .. }
            | CollabUiAction::ConfirmOwnerIdentity { .. }
            | CollabUiAction::RejectOwnerIdentity { .. } => 19,
        };
        assert_eq!(covered, 19);
    }

    #[test]
    fn parameterless_actions_round_trip_through_json() {
        let json = r#"{"type":"start"}"#;
        let wire: CollabActionWire = serde_json::from_str(json).expect("decodes");
        assert_eq!(wire, CollabActionWire::Start);
        assert_eq!(serde_json::to_string(&wire).expect("encodes"), json);
        assert_eq!(wire.into_ui_action().expect("valid"), CollabUiAction::Start);
    }

    #[test]
    fn admission_keys_are_revalidated_not_trusted() {
        let good = CollabActionWire::RejectAdmission {
            request_key: "abc-123_XYZ".into(),
        };
        assert!(good.into_ui_action().is_ok());

        for bad in ["", "has space", &"x".repeat(97), "semi;colon"] {
            let wire = CollabActionWire::ApproveAdmissionEditor {
                request_key: bad.to_string(),
            };
            assert_eq!(
                wire.into_ui_action(),
                Err(CollabActionWireError::InvalidRequestKey),
                "{bad:?} must not become a request key"
            );
        }
    }

    #[test]
    fn addresses_are_trimmed_and_bounded() {
        let wire = CollabActionWire::JoinAddress {
            endpoint: "  192.168.1.10:4321  ".into(),
        };
        assert_eq!(
            wire.into_ui_action().expect("valid"),
            CollabUiAction::JoinAddress {
                endpoint: "192.168.1.10:4321".into()
            }
        );

        for bad in ["", "   ", &"a".repeat(MAX_WIRE_ADDRESS_CHARS + 1)] {
            let wire = CollabActionWire::JoinAddress {
                endpoint: bad.to_string(),
            };
            assert_eq!(
                wire.into_ui_action(),
                Err(CollabActionWireError::InvalidAddress)
            );
        }
    }

    #[test]
    fn unknown_action_types_are_rejected_outright() {
        let err = serde_json::from_str::<CollabActionWire>(r#"{"type":"selfDestruct"}"#);
        assert!(err.is_err());
    }

    #[test]
    fn caller_named_network_actions_are_classified() {
        for action in [
            CollabActionWire::StartLan,
            CollabActionWire::BeginDiscovery,
            CollabActionWire::JoinDiscovered {
                discovery_id: "d".into(),
            },
            CollabActionWire::JoinAddress {
                endpoint: "1.2.3.4:5".into(),
            },
        ] {
            assert!(action.reaches_caller_named_network(), "{action:?}");
        }
        for action in [
            CollabActionWire::Start,
            CollabActionWire::Leave,
            CollabActionWire::SaveAsFork,
        ] {
            assert!(!action.reaches_caller_named_network(), "{action:?}");
        }
    }
}
