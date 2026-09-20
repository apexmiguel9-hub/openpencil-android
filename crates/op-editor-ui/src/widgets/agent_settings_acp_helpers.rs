//! Card-form geometry helpers for the ACP agent-settings section, split out of
//! `agent_settings_acp.rs` to keep that file under the 800-line cap. The text
//! paint helpers formerly here (`draw_text` / `ellipsize`) moved to the shared
//! `settings_form` module.

use crate::{Point2D, Rect};
use op_editor_core::agent_settings::{AcpAgentField, AcpConnectionType};

// Card-form geometry constants (split out with the geometry helpers below to
// keep `agent_settings_acp.rs` under the 800-line cap).
pub(super) const EXPANDED_CARD_H: f32 = 332.0;
pub(super) const DRAFT_CARD_H: f32 = 370.0;
const FIELD_H: f32 = 28.0;
const ENV_FIELD_H: f32 = 64.0;

/// Read-only transport badge. New custom-Agent drafts are Local-only in
/// M1; legacy Remote rows keep their persisted label but cannot switch here.
pub(super) fn connection_type_rect(card: Rect) -> Rect {
    Rect {
        origin: Point2D::new(card.origin.x + 12.0, card.origin.y + 100.0),
        size: Point2D::new(card.size.x - 24.0, 28.0),
    }
}

pub(super) fn field_input_rect(card: Rect, field: AcpAgentField) -> Rect {
    let y = match field {
        AcpAgentField::DisplayName => card.origin.y + 34.0,
        AcpAgentField::Command | AcpAgentField::Url => card.origin.y + 154.0,
        AcpAgentField::Args => card.origin.y + 208.0,
        AcpAgentField::Env => card.origin.y + 262.0,
    };
    let h = if field == AcpAgentField::Env {
        ENV_FIELD_H
    } else {
        FIELD_H
    };
    Rect {
        origin: Point2D::new(card.origin.x + 12.0, y),
        size: Point2D::new(card.size.x - 24.0, h),
    }
}

/// Y (relative to the card top) of the Save/Cancel row — just below the last
/// field, which depends on the connection type's field set. Remote (Display
/// name + URL) ends far above Local (Display name + Command + Args + Env).
pub(super) fn form_actions_y(kind: AcpConnectionType) -> f32 {
    match kind {
        AcpConnectionType::Remote => 154.0 + FIELD_H + 6.0,
        AcpConnectionType::Local => EXPANDED_CARD_H,
    }
}

/// Full DRAFT card height — fields PLUS the Save/Cancel action row.
pub(super) fn form_card_h(kind: AcpConnectionType) -> f32 {
    form_actions_y(kind) + (DRAFT_CARD_H - EXPANDED_CARD_H)
}
