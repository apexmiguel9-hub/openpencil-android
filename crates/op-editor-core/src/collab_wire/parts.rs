//! Leaf wire types for the collaboration REST projection.
//!
//! Every enum here is a *versioned copy* of an internal collaboration UI enum,
//! not a serde derive on the internal type. The duplication is the point: the
//! internal enums are free to grow variants or reorder without silently
//! changing the wire, and an unknown wire value decodes to an explicit
//! fallback instead of failing the whole payload.

use serde::{Deserialize, Serialize};

use crate::{
    CollabAvailability, CollabConnectErrorUi, CollabConnectionPathUi, CollabConnectionPhase,
    CollabDiscardedEditUi, CollabNotice, CollabNoticeKind, CollabPanelView, CollabParticipantUi,
    CollabPendingEditUi, CollabRejectUiCode, CollabRelayRegion, CollabUiRole, RemotePresenceUi,
};

macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $name:ident <=> $internal:path {
            $($wire:ident <=> $variant:ident),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub enum $name {
            $($wire),+
        }

        impl From<$internal> for $name {
            fn from(value: $internal) -> Self {
                use $internal as Internal;
                match value {
                    $(Internal::$variant => Self::$wire),+
                }
            }
        }

        impl From<$name> for $internal {
            fn from(value: $name) -> Self {
                use $internal as Internal;
                match value {
                    $($name::$wire => Internal::$variant),+
                }
            }
        }
    };
}

wire_enum! {
    /// Whether collaboration can be started at all on this deployment.
    CollabAvailabilityWire <=> CollabAvailability {
        Unavailable <=> Unavailable,
        SignInRequired <=> SignInRequired,
        Ready <=> Ready,
    }
}

wire_enum! {
    /// Where the local peer sits in the session lifecycle.
    CollabPhaseWire <=> CollabConnectionPhase {
        Idle <=> Idle,
        Starting <=> Starting,
        Discovering <=> Discovering,
        Joining <=> Joining,
        Authenticating <=> Authenticating,
        Active <=> Active,
        Reconnecting <=> Reconnecting,
        ReadOnly <=> ReadOnly,
        Ended <=> Ended,
    }
}

wire_enum! {
    /// Authority the local peer holds in the session.
    CollabRoleWire <=> CollabUiRole {
        Owner <=> Owner,
        Editor <=> Editor,
        Viewer <=> Viewer,
    }
}

wire_enum! {
    /// Which collaboration panel screen the client should show.
    CollabPanelViewWire <=> CollabPanelView {
        Home <=> Home,
        Create <=> Create,
        Join <=> Join,
        Session <=> Session,
    }
}

wire_enum! {
    /// Public-relay service region.
    CollabRelayRegionWire <=> CollabRelayRegion {
        China <=> China,
        Global <=> Global,
    }
}

wire_enum! {
    /// State of a local edit that has not yet been accepted by the owner.
    CollabPendingEditWire <=> CollabPendingEditUi {
        None <=> None,
        Submitting <=> Submitting,
        Replaying <=> Replaying,
        Conflict <=> Conflict,
    }
}

wire_enum! {
    /// Why the owner refused a local edit.
    CollabRejectCodeWire <=> CollabRejectUiCode {
        StaleBase <=> StaleBase,
        ReadOnly <=> ReadOnly,
        Unsupported <=> Unsupported,
        Conflict <=> Conflict,
        ResourceLimit <=> ResourceLimit,
        Authentication <=> Authentication,
        Unknown <=> Unknown,
    }
}

wire_enum! {
    /// Why a connection attempt failed.
    CollabConnectErrorWire <=> CollabConnectErrorUi {
        InviteUnavailable <=> InviteUnavailable,
        InviteInvalid <=> InviteInvalid,
        InviteExpired <=> InviteExpired,
        RelayUnavailable <=> RelayUnavailable,
        RelayNotConfigured <=> RelayNotConfigured,
        RegionUnavailable <=> RegionUnavailable,
        RateLimited <=> RateLimited,
        Incompatible <=> Incompatible,
        SecureKeyUnavailable <=> SecureKeyUnavailable,
        OwnerNotConfirmed <=> OwnerNotConfirmed,
    }
}

/// How the session reaches its peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CollabConnectionPathWire {
    Lan,
    #[serde(rename_all = "camelCase")]
    Relay {
        home_region: CollabRelayRegionWire,
    },
}

impl From<CollabConnectionPathUi> for CollabConnectionPathWire {
    fn from(value: CollabConnectionPathUi) -> Self {
        match value {
            CollabConnectionPathUi::Lan => Self::Lan,
            CollabConnectionPathUi::Relay { home_region } => Self::Relay {
                home_region: home_region.into(),
            },
        }
    }
}

impl From<CollabConnectionPathWire> for CollabConnectionPathUi {
    fn from(value: CollabConnectionPathWire) -> Self {
        match value {
            CollabConnectionPathWire::Lan => Self::Lan,
            CollabConnectionPathWire::Relay { home_region } => Self::Relay {
                home_region: home_region.into(),
            },
        }
    }
}

/// A transient banner the panel shows.
///
/// `UnsupportedEdit` deliberately collapses its 17-variant feature payload to
/// the i18n key: the client renders that key and nothing else, so shipping the
/// whole enum would only create a second place to keep in sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CollabNoticeKindWire {
    Connect {
        error: CollabConnectErrorWire,
    },
    DisconnectedReadOnly,
    TicketExpired,
    OwnerLeft,
    EpochChanged,
    Reject {
        code: CollabRejectCodeWire,
    },
    EditConflictDiscarded,
    LocalEditPreserved,
    UndoConflict,
    #[serde(rename_all = "camelCase")]
    UnsupportedEdit {
        i18n_key: String,
    },
}

impl From<CollabNoticeKind> for CollabNoticeKindWire {
    fn from(value: CollabNoticeKind) -> Self {
        match value {
            CollabNoticeKind::Connect(error) => Self::Connect {
                error: error.into(),
            },
            CollabNoticeKind::DisconnectedReadOnly => Self::DisconnectedReadOnly,
            CollabNoticeKind::TicketExpired => Self::TicketExpired,
            CollabNoticeKind::OwnerLeft => Self::OwnerLeft,
            CollabNoticeKind::EpochChanged => Self::EpochChanged,
            CollabNoticeKind::Reject(code) => Self::Reject { code: code.into() },
            CollabNoticeKind::EditConflictDiscarded => Self::EditConflictDiscarded,
            CollabNoticeKind::LocalEditPreserved => Self::LocalEditPreserved,
            CollabNoticeKind::UndoConflict => Self::UndoConflict,
            CollabNoticeKind::UnsupportedEdit(feature) => Self::UnsupportedEdit {
                i18n_key: feature.i18n_key().to_owned(),
            },
        }
    }
}

/// One banner plus the clock reading that dates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabNoticeWire {
    #[serde(flatten)]
    pub kind: CollabNoticeKindWire,
    pub created_at_ms: u64,
}

impl From<CollabNotice> for CollabNoticeWire {
    fn from(value: CollabNotice) -> Self {
        Self {
            kind: value.kind.into(),
            created_at_ms: value.created_at_ms,
        }
    }
}

/// A peer in the session roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabParticipantWire {
    pub participant_key: String,
    pub display_name: String,
    pub color_rgba: u32,
    pub role: CollabRoleWire,
    pub is_self: bool,
}

impl From<&CollabParticipantUi> for CollabParticipantWire {
    fn from(value: &CollabParticipantUi) -> Self {
        Self {
            participant_key: value.participant_key.clone(),
            display_name: value.display_name.clone(),
            color_rgba: value.color_rgba,
            role: value.role.into(),
            is_self: value.is_self,
        }
    }
}

impl CollabParticipantWire {
    /// Rebuild the internal type. Initials are deliberately not carried on the
    /// wire — the constructor rederives them, so a client cannot inject a
    /// display string that disagrees with the name.
    pub fn to_ui(&self) -> CollabParticipantUi {
        CollabParticipantUi::new(
            self.participant_key.clone(),
            self.display_name.clone(),
            self.color_rgba,
            self.role.into(),
            self.is_self,
        )
    }
}

/// A canvas-space point.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CollabPointWire {
    pub x: f64,
    pub y: f64,
}

/// One peer's cursor / selection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabPresenceWire {
    pub participant_key: String,
    pub cursor: Option<CollabPointWire>,
    pub selection: Vec<String>,
    pub editing_node: Option<String>,
    pub updated_at_ms: u64,
}

impl From<&RemotePresenceUi> for CollabPresenceWire {
    fn from(value: &RemotePresenceUi) -> Self {
        Self {
            participant_key: value.participant_key.clone(),
            cursor: value.cursor.map(|point| CollabPointWire {
                x: point.x,
                y: point.y,
            }),
            selection: value.selection.as_ref().clone(),
            editing_node: value.editing_node.clone(),
            updated_at_ms: value.updated_at_ms,
        }
    }
}

impl CollabPresenceWire {
    /// Rebuild the internal type through the bounding constructor, which caps
    /// selection length and trims oversized ids.
    pub fn to_ui(&self) -> RemotePresenceUi {
        RemotePresenceUi::bounded(
            self.participant_key.clone(),
            self.cursor.map(|point| crate::CollabCanvasPoint {
                x: point.x,
                y: point.y,
            }),
            self.selection.iter().cloned(),
            self.editing_node.clone(),
            self.updated_at_ms,
        )
    }
}

/// The local peer's own cursor / selection, pushed by the browser.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabLocalPresenceWire {
    pub cursor: Option<CollabPointWire>,
    /// Reserved for the multi-account server: identifies which browser
    /// connection produced this presence. Ignored by the single-tenant daemon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

/// A guest waiting for the owner's admission decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabAdmissionWire {
    /// Opaque routing token. It is a random per-request identifier, never a
    /// peer identity, and the client may only echo it back verbatim.
    pub request_key: String,
    pub resume_role: Option<CollabRoleWire>,
}

/// The verified owner identity a guest must confirm before anything is applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabOwnerConfirmationWire {
    pub request_key: String,
    pub subject: String,
    pub device_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

/// The local edit a conflict discarded, offered for replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabDiscardedEditWire {
    pub node_label: String,
    pub fields: Vec<String>,
}

impl From<&CollabDiscardedEditUi> for CollabDiscardedEditWire {
    fn from(value: &CollabDiscardedEditUi) -> Self {
        Self {
            node_label: value.node_label.clone(),
            fields: value.fields.clone(),
        }
    }
}

impl CollabDiscardedEditWire {
    pub fn to_ui(&self) -> CollabDiscardedEditUi {
        CollabDiscardedEditUi::bounded(self.node_label.clone(), self.fields.iter().cloned())
    }
}

/// One endpoint found by LAN discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollabDiscoveredWire {
    pub discovery_id: String,
    pub endpoint: String,
    pub compatible: bool,
}
