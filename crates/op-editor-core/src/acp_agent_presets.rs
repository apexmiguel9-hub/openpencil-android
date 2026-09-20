//! Quick-add presets for the ACP dynamic channel.
//!
//! The ACP channel is deliberately open: any agent that speaks ACP over
//! stdio can be added by hand through the settings form, and no vendor
//! gets an entry in `AgentProvider` / `CliName` for it. This table is
//! therefore *not* a provider registry — it is a set of prefilled forms
//! that save the user from typing a command and a flag they would
//! otherwise have to look up. Adding one produces an ordinary
//! [`crate::agent_settings::AcpAgentConfig`] that goes through the exact
//! same handshake, edit, and disconnect paths as a hand-typed entry.
//!
//! Each preset's `command` + `args` are taken from the vendor's own ACP
//! documentation; the per-entry comments carry the source, because these
//! flags have already moved once (see the `--acp` note below) and the
//! next reader needs to know what to re-check.

/// One prefilled local-stdio ACP agent.
pub struct AcpAgentPreset {
    /// Stable slug. Doubles as the created `AcpAgentConfig.id`, which is
    /// what makes "already added" an exact id lookup rather than a fuzzy
    /// command comparison. Hand-added agents use the `acp-<n>` sequence,
    /// so the two id spaces cannot collide, and `settings_io`'s
    /// `next_acp_agent_id` scan ignores anything that is not `acp-<n>`.
    pub id: &'static str,
    pub display_name: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    /// Shown when the command is known to be missing from PATH. A copyable
    /// install line, not a sentence — the UI renders it verbatim.
    pub install_hint: &'static str,
}

pub const ACP_AGENT_PRESETS: [AcpAgentPreset; 3] = [
    // `kimi acp` is a SUBCOMMAND, not a flag — there is no `--acp` on
    // either Kimi binary. Both MoonshotAI CLIs install a binary named
    // `kimi` and both use the same entry point, so this one preset covers
    // either install: the older Python `kimi-cli` (`pip install kimi-cli`,
    // now being wound down) and its successor `kimi-code`. Sources:
    // MoonshotAI/kimi-cli README and the Kimi Code CLI `kimi acp`
    // reference, both of which publish `{"command": "kimi",
    // "args": ["acp"]}` as the Zed/JetBrains agent config.
    AcpAgentPreset {
        id: "kimi",
        display_name: "Kimi CLI",
        command: "kimi",
        args: &["acp"],
        install_hint: "curl -fsSL https://code.kimi.com/kimi-code/install.sh | bash",
    },
    // Gemini CLI and Qwen Code share an ancestor, and both renamed this
    // flag the same way: `--experimental-acp` graduated to `--acp`, with
    // the old spelling kept as a deprecated alias that still works. We
    // ship the current name. If a user is on a build old enough to reject
    // `--acp`, the probe fails with the CLI's own usage error and the args
    // field is editable right there on the card.
    AcpAgentPreset {
        id: "gemini-cli",
        display_name: "Gemini CLI",
        command: "gemini",
        args: &["--acp"],
        install_hint: "npm install -g @google/gemini-cli",
    },
    // `qwen serve` is NOT this — that is a separate HTTP+SSE daemon mode.
    // ACP over stdio is `qwen --acp`.
    AcpAgentPreset {
        id: "qwen-code",
        display_name: "Qwen Code",
        command: "qwen",
        args: &["--acp"],
        install_hint: "npm install -g @qwen-code/qwen-code",
    },
];

pub fn acp_agent_preset(id: &str) -> Option<&'static AcpAgentPreset> {
    ACP_AGENT_PRESETS.iter().find(|preset| preset.id == id)
}

/// Whether a local-stdio agent's transport is byte-for-byte the preset's.
/// Used to recognise a hand-typed duplicate of a preset, so the quick-add
/// row for it disappears instead of offering a second identical card.
pub fn matches_preset_transport(preset: &AcpAgentPreset, command: &str, args: &[String]) -> bool {
    command.trim() == preset.command
        && args.len() == preset.args.len()
        && args
            .iter()
            .zip(preset.args)
            .all(|(arg, expected)| arg.trim() == *expected)
}

/// Whether the local binary backing a preset is known to be present.
///
/// `Unknown` is the honest default and the only value a host without
/// filesystem access (the browser) can produce — the UI must not claim
/// "not installed" on a host that never looked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AcpPresetAvailability {
    #[default]
    Unknown,
    Installed,
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_is_completely_specified() {
        for preset in &ACP_AGENT_PRESETS {
            assert!(!preset.id.is_empty(), "preset id must not be empty");
            assert!(
                !preset.display_name.trim().is_empty(),
                "{} has no display name",
                preset.id
            );
            assert!(
                !preset.command.trim().is_empty(),
                "{} has no command — a preset with a blank command would save \
                 an unusable card",
                preset.id
            );
            assert!(
                !preset.args.is_empty(),
                "{} has no args — every ACP entry point we ship needs an \
                 explicit ACP flag or subcommand",
                preset.id
            );
            assert!(
                !preset.install_hint.trim().is_empty(),
                "{} has no install hint",
                preset.id
            );
        }
    }

    #[test]
    fn preset_ids_are_unique_and_lookupable() {
        for preset in &ACP_AGENT_PRESETS {
            assert_eq!(
                acp_agent_preset(preset.id).map(|found| found.id),
                Some(preset.id)
            );
            assert_eq!(
                ACP_AGENT_PRESETS
                    .iter()
                    .filter(|other| other.id == preset.id)
                    .count(),
                1,
                "duplicate preset id `{}` — the id is the created agent's id, \
                 so a duplicate would make the two indistinguishable",
                preset.id
            );
        }
    }

    /// The id space must stay disjoint from the `acp-<n>` sequence that
    /// hand-added agents draw from, or a preset could shadow a saved agent.
    #[test]
    fn preset_ids_never_collide_with_the_hand_added_sequence() {
        for preset in &ACP_AGENT_PRESETS {
            assert!(
                preset
                    .id
                    .strip_prefix("acp-")
                    .and_then(|rest| rest.parse::<u64>().ok())
                    .is_none(),
                "preset id `{}` looks like a hand-added agent id",
                preset.id
            );
        }
    }

    #[test]
    fn presets_carry_the_documented_acp_entry_points() {
        let kimi = acp_agent_preset("kimi").expect("kimi preset");
        assert_eq!(kimi.command, "kimi");
        // Subcommand, not a flag — `kimi --acp` does not exist.
        assert_eq!(kimi.args, &["acp"]);

        let gemini = acp_agent_preset("gemini-cli").expect("gemini preset");
        assert_eq!(gemini.command, "gemini");
        assert_eq!(gemini.args, &["--acp"]);

        let qwen = acp_agent_preset("qwen-code").expect("qwen preset");
        assert_eq!(qwen.command, "qwen");
        assert_eq!(qwen.args, &["--acp"]);
    }

    #[test]
    fn transport_match_is_exact() {
        let gemini = acp_agent_preset("gemini-cli").expect("gemini preset");
        assert!(matches_preset_transport(
            gemini,
            " gemini ",
            &["--acp".to_string()]
        ));
        // The deprecated alias is a different configuration, so a user who
        // typed it keeps their card AND still sees the quick-add row.
        assert!(!matches_preset_transport(
            gemini,
            "gemini",
            &["--experimental-acp".to_string()]
        ));
        assert!(!matches_preset_transport(gemini, "gemini", &[]));
        assert!(!matches_preset_transport(
            gemini,
            "qwen",
            &["--acp".to_string()]
        ));
    }
}
