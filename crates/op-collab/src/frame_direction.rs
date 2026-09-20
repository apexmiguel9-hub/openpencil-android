//! The direction a collaboration message may travel and the trusted local
//! direction used to select its pre-decode size ceiling.
//!
//! Both session cores refuse a frame that arrived from the wrong side, and the
//! decoder receives its expected direction from the authenticated local
//! transport. An attacker-controlled wire `kind` must never select the larger
//! owner-to-guest Snapshot budget.

use crate::{
    codec::usize_limit, CollabMessage, ProtocolError, WireLimits, MAX_IDENTIFIER_BYTES,
    MAX_TXN_BYTES,
};

/// Fixed allowance for the envelope's JSON keys and structural bytes.
const GUEST_FRAME_STRUCTURE_BYTES: u32 = 1_024;
/// Identifier slots a guest frame can carry: the session id plus the peer ids
/// of an undo request's two client-op ids.
const GUEST_FRAME_IDENTIFIER_SLOTS: u32 = 4;

/// Default-limits value of [`guest_to_owner_envelope_limit`].
pub const MAX_GUEST_TO_OWNER_ENVELOPE_BYTES: u32 = MAX_TXN_BYTES
    + MAX_IDENTIFIER_BYTES * GUEST_FRAME_IDENTIFIER_SLOTS
    + GUEST_FRAME_STRUCTURE_BYTES;

/// The directions a collaboration message kind may legitimately travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameDirection {
    /// Only a guest sends this kind, to the owner.
    GuestToOwner,
    /// Only the owner sends this kind, to a guest.
    OwnerToGuest,
    /// Either side may send this kind.
    Bidirectional,
}

/// Authenticated direction expected by the local inbound decoder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboundFrameDirection {
    /// The local endpoint is the owner and the remote endpoint is a guest.
    GuestToOwner,
    /// The local endpoint is a guest and the remote endpoint is the owner.
    OwnerToGuest,
}

impl InboundFrameDirection {
    /// Derives the inbound direction from the authenticated remote role.
    pub const fn from_remote_role(role: crate::Role) -> Self {
        match role {
            crate::Role::Owner => Self::OwnerToGuest,
            crate::Role::Editor | crate::Role::Viewer => Self::GuestToOwner,
        }
    }
}

/// Directions the wire discriminant `kind` may legitimately travel.
///
/// An unrecognised discriminant is reported as guest input so the conservative
/// inbound ceiling applies; the generic decode rejects it immediately after.
pub(crate) fn direction_for_kind(kind: &str) -> FrameDirection {
    match kind {
        "welcome" | "commit" | "reject" | "snapshot" | "undo_result" | "presence_changed"
        | "participant_joined" | "participant_left" => FrameDirection::OwnerToGuest,
        "renew_ticket" | "bye" => FrameDirection::Bidirectional,
        _ => FrameDirection::GuestToOwner,
    }
}

/// The directions this message may legitimately travel.
pub fn message_direction(message: &CollabMessage) -> FrameDirection {
    direction_for_kind(message.kind())
}

/// Whether a guest may send this message to the owner.
pub(crate) fn message_travels_guest_to_owner(message: &CollabMessage) -> bool {
    matches!(
        message_direction(message),
        FrameDirection::GuestToOwner | FrameDirection::Bidirectional
    )
}

/// Whether the owner may send this message to a guest.
pub(crate) fn message_travels_owner_to_guest(message: &CollabMessage) -> bool {
    matches!(
        message_direction(message),
        FrameDirection::OwnerToGuest | FrameDirection::Bidirectional
    )
}

/// Inbound byte ceiling for a frame a guest may legitimately have sent.
///
/// `max_envelope_bytes` is 64 MiB because an owner→guest `Snapshot` carries a
/// whole document; nothing a guest sends comes close. Sharing that ceiling in
/// both directions would let one admitted guest force a 64 MiB
/// `serde_json::Value` per frame, long before the per-message caps could
/// reject it. This ceiling is therefore derived from those caps: the largest
/// legitimate guest payload is a `Submit` carrying a `max_txn_bytes`
/// transaction (or a `max_presence_bytes` presence update), plus a bounded
/// allowance for the envelope keys and the identifiers it carries. It never
/// exceeds `max_envelope_bytes`.
pub fn guest_to_owner_envelope_limit(limits: WireLimits) -> usize {
    let payload = limits.max_txn_bytes.max(limits.max_presence_bytes);
    let allowance = limits
        .max_identifier_bytes
        .saturating_mul(GUEST_FRAME_IDENTIFIER_SLOTS)
        .saturating_add(GUEST_FRAME_STRUCTURE_BYTES);
    usize_limit(payload.saturating_add(allowance)).min(usize_limit(limits.max_envelope_bytes))
}

/// Applies the trusted local-direction ceiling before any JSON value or typed
/// payload is decoded.
pub(crate) fn enforce_inbound_envelope_limit(
    direction: InboundFrameDirection,
    actual: usize,
    limits: WireLimits,
) -> Result<(), ProtocolError> {
    if direction == InboundFrameDirection::OwnerToGuest {
        // Authenticated owner traffic keeps the full `max_envelope_bytes`
        // ceiling, which the caller has already enforced; Snapshot needs it.
        return Ok(());
    }
    let limit = guest_to_owner_envelope_limit(limits);
    if actual > limit {
        return Err(ProtocolError::GuestEnvelopeTooLarge { actual, limit });
    }
    Ok(())
}
