//! Redacted debug projections for collaboration UI state.

use std::fmt;

use crate::{CollabPanelState, CollabUiAction};

impl fmt::Debug for CollabPanelState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollabPanelState")
            .field("open", &self.open)
            .field("view", &self.view)
            .field("join_input", &"[REDACTED]")
            .field("join_address_focused", &self.join_address_focused)
            .field("hover", &self.hover)
            .field("discovered", &self.discovered)
            .field("relay_region", &self.relay_region)
            .finish()
    }
}

impl fmt::Debug for CollabUiAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenCreate => formatter.write_str("OpenCreate"),
            Self::Start => formatter.write_str("Start"),
            Self::StartLan => formatter.write_str("StartLan"),
            Self::SetRelayRegion { region } => formatter
                .debug_struct("SetRelayRegion")
                .field("region", region)
                .finish(),
            Self::OpenJoin => formatter.write_str("OpenJoin"),
            Self::BeginDiscovery => formatter.write_str("BeginDiscovery"),
            Self::JoinDiscovered { .. } => formatter.write_str("JoinDiscovered([REDACTED])"),
            Self::JoinAddress { .. } => formatter.write_str("JoinAddress([REDACTED])"),
            Self::Cancel => formatter.write_str("Cancel"),
            Self::Retry => formatter.write_str("Retry"),
            Self::Leave => formatter.write_str("Leave"),
            Self::DiscardPending => formatter.write_str("DiscardPending"),
            Self::ReapplyDiscarded => formatter.write_str("ReapplyDiscarded"),
            Self::SaveAsFork => formatter.write_str("SaveAsFork"),
            Self::ApproveAdmissionEditor { .. } => {
                formatter.write_str("ApproveAdmissionEditor([REDACTED])")
            }
            Self::ApproveAdmissionViewer { .. } => {
                formatter.write_str("ApproveAdmissionViewer([REDACTED])")
            }
            Self::RejectAdmission { .. } => formatter.write_str("RejectAdmission([REDACTED])"),
            Self::ConfirmOwnerIdentity { .. } => {
                formatter.write_str("ConfirmOwnerIdentity([REDACTED])")
            }
            Self::RejectOwnerIdentity { .. } => {
                formatter.write_str("RejectOwnerIdentity([REDACTED])")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_address_is_redacted_from_debug() {
        let mut panel = CollabPanelState::default();
        panel.join_input.set_text("opc1_secret-route-capability");
        let debug = format!("{panel:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret-route-capability"));
    }

    #[test]
    fn pending_join_action_is_redacted_from_debug() {
        let action = CollabUiAction::JoinAddress {
            endpoint: "opc1_secret-route-capability".to_owned(),
        };
        let debug = format!("{action:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("secret-route-capability"));
    }
}
