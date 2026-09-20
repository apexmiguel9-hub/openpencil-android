//! Live model-list queries for Claude, plus the Codex
//! `latest-model.md` fallback parser.
//!
//! These are the "no hardcoded lists" upgrades shared by startup
//! discovery (`model_discovery.rs`) and the connect-time probe
//! (`provider_probe.rs`). Each mirrors the TS implementation in
//! `apps/web/server/api/ai/connect-agent.ts`:
//!
//! - Claude: the Agent SDK's `query().supportedModels()` /
//!   `accountInfo()` resolve from the CLI's `initialize` control
//!   response over stream-json stdio (connect-agent.ts:147-262 via
//!   `@anthropic-ai/claude-agent-sdk`). We speak the same wire
//!   protocol directly: spawn `claude --output-format stream-json
//!   --verbose --input-format stream-json …`, write one
//!   `control_request {subtype:"initialize"}` line, read the
//!   `control_response` carrying `models` + `account`.
//! - Codex: `~/.codex/skills/.system/openai-docs/references/
//!   latest-model.md` markdown-table rows when `models_cache.json`
//!   is missing (connect-agent.ts:378-413).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use op_ai::agent_settings_state::AgentProvider;
use op_ai::chat_models::ModelEntry;

use crate::model_discovery::{extract_json_object, resolve_cli};

/// Budget for the `claude` CLI to answer the `initialize` control
/// request. CLI startup (Node boot + auth check) takes a few
/// seconds; both callers run off the UI thread.
const CLAUDE_INIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Account fields the `initialize` control response carries —
/// the TS Agent SDK's `accountInfo()` shape used by
/// `buildClaudeConnectionInfo` (connect-agent.ts:183-197, 270-295).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClaudeAccount {
    pub email: Option<String>,
    pub subscription_type: Option<String>,
}

/// Result of the live Claude initialize query.
pub enum ClaudeInitResult {
    /// The CLI answered — model list (may be empty) + account info.
    Answered(Vec<ModelEntry>, Option<ClaudeAccount>),
    /// The CLI exited with a failure code before answering.
    ExitedWithError(i32),
    /// Spawn failure, timeout, or clean exit without a response —
    /// the TS "query closed before response" bucket that falls back
    /// to the default model list (connect-agent.ts:216-258).
    NoAnswer,
}

/// Query the Claude CLI for its supported models + account info via
/// the stream-json `initialize` control request — the same wire
/// call the TS Agent SDK's `supportedModels()` awaits.
pub fn claude_initialize_query() -> ClaudeInitResult {
    let Some(exe) = resolve_cli("claude") else {
        return ClaudeInitResult::NoAnswer;
    };
    // Arg set mirrors the TS connect options `{maxTurns: 1, tools:
    // [], permissionMode: 'plan', persistSession: false}` after the
    // SDK's arg mapping (sdk.mjs: `--tools ""` for an empty array,
    // `--no-session-persistence` for persistSession=false).
    let mut cmd = Command::new(exe);
    cmd.args([
        "--output-format",
        "stream-json",
        "--verbose",
        "--input-format",
        "stream-json",
        "--max-turns",
        "1",
        "--tools",
        "",
        "--permission-mode",
        "plan",
        "--no-session-persistence",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    // The SDK sets the entrypoint marker and strips NODE_OPTIONS.
    if std::env::var_os("CLAUDE_CODE_ENTRYPOINT").is_none() {
        cmd.env("CLAUDE_CODE_ENTRYPOINT", "sdk-ts");
    }
    cmd.env_remove("NODE_OPTIONS");
    crate::chat_spawn::hide_console_window(&mut cmd);
    let Ok(mut child) = cmd.spawn() else {
        return ClaudeInitResult::NoAnswer;
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return ClaudeInitResult::NoAnswer;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return ClaudeInitResult::NoAnswer;
    };

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let init = r#"{"request_id":"op-connect-1","type":"control_request","request":{"subtype":"initialize"}}"#;
    if writeln!(stdin, "{init}").is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return ClaudeInitResult::NoAnswer;
    }
    let _ = stdin.flush();

    let deadline = Instant::now() + CLAUDE_INIT_TIMEOUT;
    let mut answered = None;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                if let Some(parsed) = parse_claude_init_line(&line) {
                    answered = Some(parsed);
                    break;
                }
            }
            // Timeout, or the reader closed because the CLI exited
            // without answering — stop waiting either way.
            Err(_) => break,
        }
    }
    drop(stdin);
    let status = if answered.is_some() {
        let _ = child.kill();
        child.wait().ok()
    } else {
        // Give the exited process a moment to report its code so a
        // login failure maps to the friendly-error path.
        child.try_wait().ok().flatten().or_else(|| {
            let _ = child.kill();
            child.wait().ok()
        })
    };
    match answered {
        Some((models, account)) => ClaudeInitResult::Answered(models, account),
        None => match status {
            Some(st) if !st.success() => ClaudeInitResult::ExitedWithError(st.code().unwrap_or(1)),
            _ => ClaudeInitResult::NoAnswer,
        },
    }
}

/// Parse one stream-json line from the Claude CLI; yields models +
/// account only for the successful `initialize` control response.
pub fn parse_claude_init_line(line: &str) -> Option<(Vec<ModelEntry>, Option<ClaudeAccount>)> {
    let json: serde_json::Value = serde_json::from_str(extract_json_object(line)?).ok()?;
    if json.get("type")?.as_str()? != "control_response" {
        return None;
    }
    let response = json.get("response")?;
    if response.get("subtype").and_then(|v| v.as_str()) != Some("success") {
        return None;
    }
    let inner = response.get("response")?;
    let models = inner
        .get("models")?
        .as_array()?
        .iter()
        .filter_map(|m| {
            let value = m.get("value")?.as_str()?;
            let name = m
                .get("displayName")
                .and_then(|v| v.as_str())
                .unwrap_or(value);
            Some(ModelEntry::new(AgentProvider::ClaudeCode, value, name))
        })
        .collect();
    let account = inner.get("account").map(|a| ClaudeAccount {
        email: a.get("email").and_then(|v| v.as_str()).map(str::to_string),
        subscription_type: a
            .get("subscriptionType")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    });
    Some((models, account))
}

/// TS `FALLBACK_CLAUDE_MODELS` (connect-agent.ts:102-145) — used
/// when `supportedModels()` doesn't answer (e.g. third-party API
/// proxies that don't support the listing endpoint).
pub fn fallback_claude_models() -> Vec<ModelEntry> {
    [
        ("claude-sonnet-4-6", "Claude Sonnet 4.6"),
        ("claude-opus-4-6", "Claude Opus 4.6"),
        ("claude-sonnet-4-5-20250514", "Claude Sonnet 4.5"),
        ("claude-haiku-4-5-20251001", "Claude Haiku 4.5"),
        ("claude-3-7-sonnet-20250219", "Claude 3.7 Sonnet"),
        ("claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet"),
        ("claude-3-5-haiku-20241022", "Claude 3.5 Haiku"),
    ]
    .iter()
    .map(|(value, name)| ModelEntry::new(AgentProvider::ClaudeCode, *value, *name))
    .collect()
}

/// `CODEX_HOME` env override or `~/.codex` — the same resolution
/// the TS route applies (connect-agent.ts:526).
pub fn codex_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    Some(dirs::home_dir()?.join(".codex"))
}

/// Parse model ids out of Codex's bundled `latest-model.md`
/// reference — the TS fallback when `models_cache.json` is missing
/// (connect-agent.ts:373-413). Text/reasoning models only.
pub fn codex_models_from_latest_md(codex_home: &Path) -> Vec<ModelEntry> {
    let md_path = codex_home
        .join("skills")
        .join(".system")
        .join("openai-docs")
        .join("references")
        .join("latest-model.md");
    let Ok(content) = std::fs::read_to_string(md_path) else {
        return Vec::new();
    };
    parse_codex_latest_model_md(&content)
}

/// Markdown-table row parser for `latest-model.md` — matches the TS
/// row regex ``^\|\s*`([^`]+)`\s*\|\s*(.+?)\s*\|`` and skip regex
/// `/image|audio|tts|transcribe|realtime|sora|video|embedding|moderation/i`.
pub fn parse_codex_latest_model_md(content: &str) -> Vec<ModelEntry> {
    let mut models = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for line in content.lines() {
        let Some(rest) = line.trim_start().strip_prefix('|') else {
            continue;
        };
        let mut cells = rest.split('|');
        let Some(first) = cells.next() else { continue };
        let first = first.trim();
        let Some(slug) = first
            .strip_prefix('`')
            .and_then(|s| s.strip_suffix('`'))
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let desc = cells.next().unwrap_or("").trim();
        if codex_md_is_skipped(slug) || codex_md_is_skipped(desc) || seen.contains(slug) {
            continue;
        }
        seen.insert(slug.to_string());
        // TS maps `displayName: slug` (the description is carried in
        // a field ModelEntry doesn't have; display stays the slug).
        models.push(ModelEntry::new(AgentProvider::CodexCli, slug, slug));
    }
    models
}

fn codex_md_is_skipped(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "image",
        "audio",
        "tts",
        "transcribe",
        "realtime",
        "sora",
        "video",
        "embedding",
        "moderation",
    ]
    .iter()
    .any(|w| lower.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_init_response_parser_extracts_models_and_account() {
        // Mirrors the CLI's control_response envelope: the SDK reads
        // `$.response.response.models` / `.account`.
        let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"op-connect-1","response":{"models":[{"value":"claude-sonnet-4-6","displayName":"Claude Sonnet 4.6","description":"Latest"},{"value":"claude-haiku-4-5"}],"account":{"email":"a@b.c","subscriptionType":"pro","apiKeySource":"none"}}}}"#;
        let (models, account) = parse_claude_init_line(line).expect("parses");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].display_name, "Claude Sonnet 4.6");
        // Missing displayName falls back to the value.
        assert_eq!(models[1].display_name, "claude-haiku-4-5");
        let account = account.expect("account present");
        assert_eq!(account.email.as_deref(), Some("a@b.c"));
        assert_eq!(account.subscription_type.as_deref(), Some("pro"));
    }

    #[test]
    fn claude_init_parser_skips_non_init_lines() {
        // System/init chatter and error responses must not match.
        assert!(parse_claude_init_line(r#"{"type":"system","subtype":"init"}"#).is_none());
        assert!(parse_claude_init_line(
            r#"{"type":"control_response","response":{"subtype":"error","request_id":"x","error":"nope"}}"#
        )
        .is_none());
        assert!(parse_claude_init_line("not json").is_none());
    }

    #[test]
    fn claude_fallback_models_mirror_ts_constants() {
        let claude = fallback_claude_models();
        assert_eq!(claude.len(), 7);
        assert_eq!(claude[0].value, "claude-sonnet-4-6");
        assert_eq!(claude[6].display_name, "Claude 3.5 Haiku");
    }

    #[test]
    fn codex_latest_md_parser_keeps_text_models_only() {
        let md = "\
| Model | Description |\n\
| --- | --- |\n\
| `gpt-5.5` | Latest flagship reasoning model |\n\
| `gpt-5.5` | duplicate row |\n\
| `gpt-image-2` | Image generation model |\n\
| `sora-3` | Video generation |\n\
| `gpt-audio-mini` | Audio model |\n\
| no-backticks | not a model row |\n\
| `gpt-5.4-codex` | Coding model |\n";
        let models = parse_codex_latest_model_md(md);
        let values: Vec<&str> = models.iter().map(|m| m.value.as_str()).collect();
        assert_eq!(values, ["gpt-5.5", "gpt-5.4-codex"]);
        // displayName mirrors the slug like the TS mapper.
        assert_eq!(models[0].display_name, "gpt-5.5");
    }
}
