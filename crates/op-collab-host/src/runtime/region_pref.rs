//! Persisted public-relay service-region preference.
//!
//! The value is a plain reachability choice between the built-in hubs; it
//! carries no credential or endpoint material, so a corrupt or missing file
//! simply falls back to the default region.

use op_editor_core::CollabRelayRegion;
use serde::{Deserialize, Serialize};

use super::relay::protocol_region;
use super::CollabRuntime;
use crate::host::CollabHost;

const REGION_PREF_FILE: &str = "collaboration-region-v1.json";

impl CollabRuntime {
    /// Project the runtime-owned service-region preference into the panel
    /// state. Runs every frame: document open/replace swaps the whole
    /// `EditorState`, so the panel field would otherwise silently reset to
    /// the default region while the runtime kept using the real preference.
    pub(super) fn sync_relay_region(&mut self, host: &mut impl CollabHost) {
        let region = *self.relay_region_pref.get_or_insert_with(load);
        let collab = &mut host.editor_state_mut().editor_ui.collab;
        if collab.panel.relay_region != region {
            collab.panel.relay_region = region;
            host.mark_editor_state_dirty();
        }
    }

    /// The user's current service-region preference, in protocol terms.
    pub(super) fn preferred_region(&mut self) -> op_collab_relay_protocol::RelayRegion {
        protocol_region(*self.relay_region_pref.get_or_insert_with(load))
    }

    pub(super) fn set_relay_region(
        &mut self,
        host: &mut impl CollabHost,
        region: CollabRelayRegion,
    ) {
        self.relay_region_pref = Some(region);
        // Persist unconditionally: `store` is best-effort, so re-selecting
        // the same region must retry a write that previously failed rather
        // than being skipped by an equality gate.
        store(region);
        let collab = &mut host.editor_state_mut().editor_ui.collab;
        if collab.panel.relay_region != region {
            collab.panel.relay_region = region;
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RegionPrefFile {
    region: String,
}

fn load() -> CollabRelayRegion {
    match op_config_store::read_json::<RegionPrefFile>(REGION_PREF_FILE) {
        Ok(Some(file)) => match file.region.as_str() {
            "cn" => CollabRelayRegion::China,
            "global" => CollabRelayRegion::Global,
            _ => CollabRelayRegion::default(),
        },
        _ => CollabRelayRegion::default(),
    }
}

/// Best-effort persist: an unwritable configuration directory must not make
/// the in-session switch fail, so the result is intentionally dropped.
fn store(region: CollabRelayRegion) {
    let value = RegionPrefFile {
        region: match region {
            CollabRelayRegion::China => "cn".to_owned(),
            CollabRelayRegion::Global => "global".to_owned(),
        },
    };
    let _ = op_config_store::write_json(REGION_PREF_FILE, &value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_both_regions_through_the_wire_shape() {
        for (region, wire) in [
            (CollabRelayRegion::China, "cn"),
            (CollabRelayRegion::Global, "global"),
        ] {
            let file = RegionPrefFile {
                region: wire.to_owned(),
            };
            let json = serde_json::to_string(&file).unwrap();
            let back: RegionPrefFile = serde_json::from_str(&json).unwrap();
            assert_eq!(back.region, wire);
            let parsed = match back.region.as_str() {
                "cn" => CollabRelayRegion::China,
                "global" => CollabRelayRegion::Global,
                _ => unreachable!(),
            };
            assert_eq!(parsed, region);
        }
    }

    #[test]
    fn unknown_wire_value_falls_back_to_the_default_region() {
        let file = RegionPrefFile {
            region: "moon".to_owned(),
        };
        let parsed = match file.region.as_str() {
            "cn" => CollabRelayRegion::China,
            "global" => CollabRelayRegion::Global,
            _ => CollabRelayRegion::default(),
        };
        assert_eq!(parsed, CollabRelayRegion::default());
    }
}
