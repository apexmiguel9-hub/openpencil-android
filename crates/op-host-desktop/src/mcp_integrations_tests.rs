//! Tests for `mcp_integrations.rs` — CLI config install / uninstall,
//! Antigravity's two-file transaction + rollback, and detection.
//!
//! Split out of `mcp_integrations.rs` (pure code motion) to keep that file
//! under the repo's 800-line-per-file cap.

use super::*;

fn temp_home(name: &str) -> PathBuf {
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("test");
    let safe_thread_name: String = thread_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect();
    let path = std::env::temp_dir().join(format!(
        "openpencil-mcp-{name}-{}-{}",
        std::process::id(),
        safe_thread_name
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp home");
    path
}

fn seed_antigravity_permission(home: &Path) {
    let path = antigravity_permissions_path(home);
    fs::create_dir_all(path.parent().expect("parent")).expect("create permission dir");
    fs::write(
        path,
        format!(r#"{{"permissions":{{"allow":["{ANTIGRAVITY_MCP_PERMISSION}"]}}}}"#),
    )
    .expect("seed Antigravity permission");
}

#[test]
fn explicit_home_grok_config_path_ignores_process_environment() {
    let home = Path::new("/test/home");
    assert_eq!(
        config_path(McpCli::GrokBuild, home, false),
        home.join(".grok").join("config.toml")
    );
}

#[test]
fn mcp_json_config_install_and_uninstall_preserves_other_servers() {
    let home = temp_home("json");
    let path = home.join(".claude.json");
    fs::write(
        &path,
        r#"{"theme":"dark","mcpServers":{"other":{"type":"http","url":"http://x"}}}"#,
    )
    .expect("seed config");

    set_cli_enabled_at_home(McpCli::ClaudeCode, true, 3101, &home).expect("install");
    let text = fs::read_to_string(&path).expect("read installed");
    assert!(text.contains(r#""openpencil""#), "{text}");
    assert!(
        text.contains(r#""url": "http://127.0.0.1:3101/mcp""#),
        "{text}"
    );
    assert!(text.contains(r#""other""#), "{text}");

    set_cli_enabled_at_home(McpCli::ClaudeCode, false, 3101, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains(r#""openpencil""#), "{text}");
    assert!(text.contains(r#""other""#), "{text}");

    let _ = fs::remove_dir_all(home);
}

#[test]
fn mcp_codex_config_replaces_existing_openpencil_block() {
    let home = temp_home("codex");
    let path = home.join(".codex").join("config.toml");
    fs::create_dir_all(path.parent().expect("parent")).expect("create codex dir");
    fs::write(
        &path,
        "model = \"gpt-5\"\n\n[mcp_servers.openpencil]\nurl = \"http://old\"\n\n[profiles.dev]\nmodel = \"gpt-5-codex\"\n",
    )
    .expect("seed config");

    set_cli_enabled_at_home(McpCli::Codex, true, 3200, &home).expect("install");
    let text = fs::read_to_string(&path).expect("read installed");
    assert_eq!(
        text.matches("[mcp_servers.openpencil]").count(),
        1,
        "{text}"
    );
    assert!(
        text.contains("url = \"http://127.0.0.1:3200/mcp\""),
        "{text}"
    );
    assert!(text.contains("[profiles.dev]"), "{text}");

    set_cli_enabled_at_home(McpCli::Codex, false, 3200, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains("[mcp_servers.openpencil]"), "{text}");
    assert!(text.contains("model = \"gpt-5\""), "{text}");
    assert!(text.contains("[profiles.dev]"), "{text}");

    let _ = fs::remove_dir_all(home);
}

#[test]
fn mcp_antigravity_config_uses_current_schema_and_preserves_other_servers() {
    let home = temp_home("antigravity");
    let path = home.join(".gemini").join("config").join("mcp_config.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create config dir");
    fs::write(
        &path,
        r#"{"permissionMode":"request-review","mcpServers":{"other":{"serverUrl":"https://example.com/mcp"}}}"#,
    )
    .expect("seed config");

    let written = set_cli_enabled_at_home(McpCli::Antigravity, true, 3300, &home).expect("install");
    assert_eq!(written, path);
    let root = read_json_object(&path).expect("read installed config");
    let servers = root
        .get("mcpServers")
        .and_then(Value::as_object)
        .expect("mcpServers");
    let openpencil = servers
        .get(SERVER_NAME)
        .and_then(Value::as_object)
        .expect("openpencil server");
    assert_eq!(
        openpencil.get("serverUrl").and_then(Value::as_str),
        Some("http://127.0.0.1:3300/mcp")
    );
    assert!(!openpencil.contains_key("serverURL"));
    assert!(!openpencil.contains_key("url"));
    assert!(servers.contains_key("other"));
    assert_eq!(
        root.get("permissionMode").and_then(Value::as_str),
        Some("request-review")
    );
    set_cli_enabled_at_home(McpCli::Antigravity, false, 3300, &home).expect("uninstall");
    let root = read_json_object(&path).expect("read uninstalled config");
    let servers = root
        .get("mcpServers")
        .and_then(Value::as_object)
        .expect("other server remains");
    assert!(!servers.contains_key(SERVER_NAME));
    assert!(servers.contains_key("other"));
    assert_eq!(
        root.get("permissionMode").and_then(Value::as_str),
        Some("request-review")
    );
    let _ = fs::remove_dir_all(home);
}

#[test]
fn mcp_antigravity_permission_is_narrow_idempotent_and_removable() {
    let home = temp_home("antigravity-permission");
    let settings = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("settings.json");
    fs::create_dir_all(settings.parent().expect("parent")).expect("create settings dir");
    fs::write(
        &settings,
        r#"{
            "colorScheme":"dark",
            "permissions":{
                "allow":["shell(git status)","mcp(other/*)"],
                "deny":["shell(rm -rf *)","mcp(openpencil/*)"]
            }
        }"#,
    )
    .expect("seed settings");
    set_cli_enabled_at_home(McpCli::Antigravity, true, 3302, &home).expect("install");
    set_cli_enabled_at_home(McpCli::Antigravity, true, 3302, &home).expect("idempotent install");
    let root = read_json_object(&settings).expect("read enabled settings");
    let permissions = root["permissions"].as_object().expect("permissions");
    let allow = permissions["allow"].as_array().expect("allow array");
    assert_eq!(
        allow
            .iter()
            .filter(|rule| rule.as_str() == Some(ANTIGRAVITY_MCP_PERMISSION))
            .count(),
        1
    );
    assert!(allow
        .iter()
        .any(|rule| rule.as_str() == Some("shell(git status)")));
    assert!(allow
        .iter()
        .any(|rule| rule.as_str() == Some("mcp(other/*)")));
    assert_eq!(
        permissions["deny"]
            .as_array()
            .and_then(|rules| rules[0].as_str()),
        Some("shell(rm -rf *)")
    );
    assert!(!permissions["deny"]
        .as_array()
        .expect("deny array")
        .iter()
        .any(|rule| rule.as_str() == Some(ANTIGRAVITY_MCP_PERMISSION)));
    assert_eq!(root["colorScheme"].as_str(), Some("dark"));
    set_cli_enabled_at_home(McpCli::Antigravity, false, 3302, &home).expect("uninstall");
    let root = read_json_object(&settings).expect("read disabled settings");
    let permissions = root["permissions"].as_object().expect("permissions");
    let allow = permissions["allow"].as_array().expect("allow array");
    assert!(!allow
        .iter()
        .any(|rule| rule.as_str() == Some(ANTIGRAVITY_MCP_PERMISSION)));
    assert!(allow
        .iter()
        .any(|rule| rule.as_str() == Some("shell(git status)")));
    assert!(allow
        .iter()
        .any(|rule| rule.as_str() == Some("mcp(other/*)")));
    assert_eq!(
        permissions["deny"]
            .as_array()
            .and_then(|rules| rules[0].as_str()),
        Some("shell(rm -rf *)")
    );
    let _ = fs::remove_dir_all(home);
}

#[test]
fn mcp_antigravity_rolls_back_config_when_permission_update_fails() {
    let home = temp_home("antigravity-rollback");
    let config = config_path(McpCli::Antigravity, &home, false);
    let settings = antigravity_permissions_path(&home);
    fs::create_dir_all(config.parent().expect("config parent")).expect("create config dir");
    fs::create_dir_all(settings.parent().expect("settings parent")).expect("create settings dir");
    let original_config = br#"{"mcpServers":{"other":{"serverUrl":"https://example.com/mcp"}}}"#;
    let invalid_settings = b"{ this is not valid JSON";
    fs::write(&config, original_config).expect("seed config");
    fs::write(&settings, invalid_settings).expect("seed invalid settings");
    let error = set_cli_enabled_at_home(McpCli::Antigravity, true, 3303, &home)
        .expect_err("permission parse must fail")
        .to_string();
    assert!(error.contains("parse"), "{error}");
    assert_eq!(
        fs::read(&config).expect("read rolled back config"),
        original_config
    );
    assert_eq!(
        fs::read(&settings).expect("read original settings"),
        invalid_settings
    );
    let _ = fs::remove_dir_all(home);
}

#[test]
fn mcp_grok_config_replaces_only_openpencil_tables() {
    let home = temp_home("grok");
    let path = home.join(".grok").join("config.toml");
    fs::create_dir_all(path.parent().expect("parent")).expect("create grok dir");
    fs::write(
        &path,
        "[cli]\ninstaller = \"internal\"\n\n[mcp_servers.other]\nurl = \"https://example.com/mcp\"\n\n[mcp_servers.openpencil]\nurl = \"http://old/mcp\"\n\n[mcp_servers.openpencil.headers]\nAuthorization = \"Bearer stale\"\n\n[model.custom]\nmodel = \"custom-id\"\n",
    )
    .expect("seed config");
    set_cli_enabled_at_home(McpCli::GrokBuild, true, 3400, &home).expect("install");
    let text = fs::read_to_string(&path).expect("read installed");
    assert_eq!(
        text.matches("[mcp_servers.openpencil]").count(),
        1,
        "{text}"
    );
    assert!(
        text.contains("url = \"http://127.0.0.1:3400/mcp\""),
        "{text}"
    );
    assert!(!text.contains("Bearer stale"), "{text}");
    assert!(text.contains("[mcp_servers.other]"), "{text}");
    assert!(text.contains("[model.custom]"), "{text}");

    set_cli_enabled_at_home(McpCli::GrokBuild, false, 3400, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains("mcp_servers.openpencil"), "{text}");
    assert!(text.contains("[mcp_servers.other]"), "{text}");
    assert!(text.contains("[model.custom]"), "{text}");

    let _ = fs::remove_dir_all(home);
}

#[test]
fn detects_antigravity_and_grok_openpencil_servers() {
    let home = temp_home("new-cli-detect");
    let antigravity = home.join(".gemini").join("config").join("mcp_config.json");
    fs::create_dir_all(antigravity.parent().expect("parent")).expect("create agy dir");
    fs::write(
        antigravity,
        r#"{"mcpServers":{"openpencil":{"serverUrl":"http://127.0.0.1:3000/mcp"}}}"#,
    )
    .expect("seed Antigravity config");
    seed_antigravity_permission(&home);
    let grok = home.join(".grok").join("config.toml");
    fs::create_dir_all(grok.parent().expect("parent")).expect("create Grok dir");
    fs::write(
        grok,
        "[mcp_servers.openpencil]\nurl = \"http://127.0.0.1:3000/mcp\"\n",
    )
    .expect("seed Grok config");

    let flags = detect_enabled_clis_at_home(&home);
    let antigravity_idx = McpCli::ALL
        .iter()
        .position(|cli| *cli == McpCli::Antigravity)
        .expect("Antigravity index");
    let grok_idx = McpCli::ALL
        .iter()
        .position(|cli| *cli == McpCli::GrokBuild)
        .expect("Grok index");
    assert!(flags[antigravity_idx], "{flags:?}");
    assert!(flags[grok_idx], "{flags:?}");
    assert_eq!(flags.iter().filter(|enabled| **enabled).count(), 2);

    let _ = fs::remove_dir_all(home);
}

#[test]
fn antigravity_detection_requires_a_valid_server_url() {
    let home = temp_home("antigravity-invalid-detect");
    let path = home.join(".gemini").join("config").join("mcp_config.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create config dir");
    let antigravity_idx = McpCli::ALL
        .iter()
        .position(|cli| *cli == McpCli::Antigravity)
        .expect("Antigravity index");
    seed_antigravity_permission(&home);

    for config in [
        r#"{"mcpServers":{"openpencil":{"serverURL":"http://127.0.0.1:3000/mcp"}}}"#,
        r#"{"mcpServers":{"openpencil":{"url":"http://127.0.0.1:3000/mcp"}}}"#,
        r#"{"mcpServers":{"openpencil":{"serverUrl":"not-a-url"}}}"#,
        r#"{"mcpServers":{"openpencil":{"serverUrl":"http://127.0.0.1:3000/mcp","disabled":true}}}"#,
    ] {
        fs::write(&path, config).expect("write invalid config");
        assert!(!detect_enabled_clis_at_home(&home)[antigravity_idx]);
    }

    fs::write(
        &path,
        r#"{"mcpServers":{"openpencil":{"serverUrl":"http://127.0.0.1:3000/mcp"}}}"#,
    )
    .expect("write valid config");
    assert!(detect_enabled_clis_at_home(&home)[antigravity_idx]);

    fs::write(
        antigravity_permissions_path(&home),
        format!(r#"{{"permissions":{{"allow":["{ANTIGRAVITY_MCP_PERMISSION}"],"deny":["{ANTIGRAVITY_MCP_PERMISSION}"]}}}}"#),
    )
    .expect("write denied permission");
    assert!(!detect_enabled_clis_at_home(&home)[antigravity_idx]);

    fs::remove_file(antigravity_permissions_path(&home)).expect("remove permission");
    assert!(!detect_enabled_clis_at_home(&home)[antigravity_idx]);

    let _ = fs::remove_dir_all(home);
}

#[test]
fn detects_legacy_codex_openpencil_server_config() {
    let home = temp_home("codex-detect");
    let path = home.join(".codex").join("config.toml");
    fs::create_dir_all(path.parent().expect("parent")).expect("create codex dir");
    fs::write(
        &path,
        "model = \"gpt-5\"\n\n[mcp_servers.openpencil]\ncommand = \"/usr/local/bin/node\"\nargs = [\"/Applications/OpenPencil.app/Contents/Resources/mcp-server.cjs\"]\n",
    )
    .expect("seed legacy config");

    let flags = detect_enabled_clis_at_home(&home);

    let codex_idx = McpCli::ALL
        .iter()
        .position(|cli| *cli == McpCli::Codex)
        .expect("Codex CLI index");
    assert!(flags[codex_idx]);
    assert!(
        flags.iter().filter(|enabled| **enabled).count() == 1,
        "{flags:?}"
    );

    let _ = fs::remove_dir_all(home);
}

/// Index of `cli` in `McpCli::ALL`, which is what `mcp_cli_enabled` and
/// `detect_enabled_clis_*` are keyed by.
fn cli_index(cli: McpCli) -> usize {
    McpCli::ALL
        .iter()
        .position(|candidate| *candidate == cli)
        .expect("CLI is registered in McpCli::ALL")
}

/// The server map for `cli`, read from wherever that CLI keeps it.
fn server_map(cli: McpCli, path: &Path) -> Map<String, Value> {
    let root = read_json_object(path).expect("config parses");
    let servers = if cli == McpCli::ZCode {
        root.get("mcp")
            .and_then(Value::as_object)
            .and_then(|mcp| mcp.get("servers"))
    } else {
        root.get("mcpServers")
    };
    servers
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn server_entry(path: &Path) -> Map<String, Value> {
    server_entry_for(McpCli::ClaudeCode, path)
}

fn server_entry_for(cli: McpCli, path: &Path) -> Map<String, Value> {
    server_map(cli, path)
        .get(SERVER_NAME)
        .and_then(Value::as_object)
        .cloned()
        .expect("openpencil server entry")
}

#[test]
fn gemini_cli_and_antigravity_write_separate_files_under_dot_gemini() {
    let home = Path::new("/test/home");
    assert_eq!(
        config_path(McpCli::GeminiCli, home, false),
        home.join(".gemini").join("settings.json")
    );
    assert_eq!(
        config_path(McpCli::Antigravity, home, false),
        home.join(".gemini").join("config").join("mcp_config.json")
    );
    assert_eq!(
        config_path(McpCli::QwenCode, home, false),
        home.join(".qwen").join("settings.json")
    );
    assert_eq!(
        config_path(McpCli::Cursor, home, false),
        home.join(".cursor").join("mcp.json")
    );
    assert_eq!(
        config_path(McpCli::Kimi, home, false),
        home.join(".kimi-code").join("mcp.json"),
        "explicit-home lookups must ignore KIMI_CODE_HOME"
    );
    assert_eq!(
        config_path(McpCli::ZCode, home, false),
        home.join(".zcode").join("cli").join("config.json")
    );
}

/// Shape verified against what `gemini mcp add --transport http` (Gemini CLI
/// 0.30.0) writes to `~/.gemini/settings.json`.
#[test]
fn mcp_gemini_cli_writes_type_http_and_leaves_antigravity_alone() {
    let home = temp_home("gemini-cli");
    let path = home.join(".gemini").join("settings.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create gemini dir");
    fs::write(&path, r#"{"selectedAuthType":"oauth-personal"}"#).expect("seed settings");

    set_cli_enabled_at_home(McpCli::GeminiCli, true, 3500, &home).expect("install");
    let entry = server_entry(&path);
    assert_eq!(entry.get("type").and_then(Value::as_str), Some("http"));
    assert_eq!(
        entry.get("url").and_then(Value::as_str),
        Some("http://127.0.0.1:3500/mcp")
    );
    let root = read_json_object(&path).expect("parse");
    assert_eq!(
        root.get("selectedAuthType").and_then(Value::as_str),
        Some("oauth-personal"),
        "unrelated Gemini settings must survive"
    );
    assert!(
        !config_path(McpCli::Antigravity, &home, false).exists(),
        "Antigravity's own config file must not be created"
    );

    set_cli_enabled_at_home(McpCli::GeminiCli, false, 3500, &home).expect("uninstall");
    let root = read_json_object(&path).expect("parse");
    assert!(!root.contains_key("mcpServers"), "{root:?}");
    assert_eq!(
        root.get("selectedAuthType").and_then(Value::as_str),
        Some("oauth-personal")
    );

    let _ = fs::remove_dir_all(home);
}

/// Qwen Code reads a bare `url` as SSE (`qwen mcp list` prints `(sse)`); only
/// `httpUrl` selects streamable HTTP, which is what `qwen mcp add --transport
/// http` writes itself.
#[test]
fn mcp_qwen_config_uses_http_url_key_and_preserves_other_servers() {
    let home = temp_home("qwen");
    let path = home.join(".qwen").join("settings.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create qwen dir");
    fs::write(
        &path,
        r#"{"ui":{"theme":"Default"},"mcpServers":{"other":{"command":"echo","args":["hi"]}}}"#,
    )
    .expect("seed settings");

    set_cli_enabled_at_home(McpCli::QwenCode, true, 3600, &home).expect("install");
    let entry = server_entry(&path);
    assert_eq!(
        entry.get("httpUrl").and_then(Value::as_str),
        Some("http://127.0.0.1:3600/mcp")
    );
    assert!(!entry.contains_key("url"), "{entry:?}");
    let text = fs::read_to_string(&path).expect("read installed");
    assert!(text.contains(r#""other""#), "{text}");
    assert!(text.contains(r#""theme""#), "{text}");

    set_cli_enabled_at_home(McpCli::QwenCode, false, 3600, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains(r#""openpencil""#), "{text}");
    assert!(text.contains(r#""other""#), "{text}");

    let _ = fs::remove_dir_all(home);
}

/// Cursor's config reader keys off `url` (it builds a `streamableHttp` server
/// from it) and ignores the extra `type`, so the shared JSON shape works. The
/// file is shared with the Cursor editor by design.
#[test]
fn mcp_cursor_config_install_and_uninstall_preserves_other_servers() {
    let home = temp_home("cursor");
    let path = home.join(".cursor").join("mcp.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create cursor dir");
    fs::write(
        &path,
        r#"{"mcpServers":{"other":{"url":"https://example.com/mcp"}}}"#,
    )
    .expect("seed config");

    set_cli_enabled_at_home(McpCli::Cursor, true, 3700, &home).expect("install");
    let entry = server_entry(&path);
    assert_eq!(
        entry.get("url").and_then(Value::as_str),
        Some("http://127.0.0.1:3700/mcp")
    );
    let text = fs::read_to_string(&path).expect("read installed");
    assert!(text.contains(r#""other""#), "{text}");

    set_cli_enabled_at_home(McpCli::Cursor, false, 3700, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains(r#""openpencil""#), "{text}");
    assert!(text.contains(r#""other""#), "{text}");

    let _ = fs::remove_dir_all(home);
}

/// An empty `~/.cursor/mcp.json` is what a fresh Cursor install leaves behind;
/// the writer must treat it as an empty object rather than a parse failure.
#[test]
fn mcp_cursor_config_installs_into_an_empty_file() {
    let home = temp_home("cursor-empty");
    let path = home.join(".cursor").join("mcp.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create cursor dir");
    fs::write(&path, "").expect("seed empty config");

    set_cli_enabled_at_home(McpCli::Cursor, true, 3701, &home).expect("install");

    assert_eq!(
        server_entry(&path).get("url").and_then(Value::as_str),
        Some("http://127.0.0.1:3701/mcp")
    );

    let _ = fs::remove_dir_all(home);
}

/// Shape verified against kimi-code 0.31.0's own config schema: a union
/// discriminated on `transport`, with `url` carrying the endpoint.
#[test]
fn mcp_kimi_config_uses_transport_key_and_preserves_other_servers() {
    let home = temp_home("kimi");
    let path = home.join(".kimi-code").join("mcp.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create kimi dir");
    fs::write(
        &path,
        r#"{"mcpServers":{"other":{"command":"echo","args":["hi"]}}}"#,
    )
    .expect("seed config");

    set_cli_enabled_at_home(McpCli::Kimi, true, 3800, &home).expect("install");
    let entry = server_entry(&path);
    assert_eq!(
        entry.get("url").and_then(Value::as_str),
        Some("http://127.0.0.1:3800/mcp")
    );
    assert_eq!(entry.get("transport").and_then(Value::as_str), Some("http"));
    let text = fs::read_to_string(&path).expect("read installed");
    assert!(text.contains(r#""other""#), "{text}");

    set_cli_enabled_at_home(McpCli::Kimi, false, 3800, &home).expect("uninstall");
    let text = fs::read_to_string(&path).expect("read uninstalled");
    assert!(!text.contains(r#""openpencil""#), "{text}");
    assert!(text.contains(r#""other""#), "{text}");

    let _ = fs::remove_dir_all(home);
}

#[test]
fn new_clis_install_idempotently_and_round_trip_through_detection() {
    let clis = [
        McpCli::GeminiCli,
        McpCli::QwenCode,
        McpCli::Cursor,
        McpCli::Kimi,
        McpCli::ZCode,
    ];
    for cli in clis {
        let home = temp_home(&format!("roundtrip-{}", cli_index(cli)));
        let idx = cli_index(cli);

        assert!(!detect_enabled_clis_at_home(&home)[idx], "{cli:?}");

        let path = set_cli_enabled_at_home(cli, true, 3900, &home).expect("install");
        set_cli_enabled_at_home(cli, true, 3900, &home).expect("idempotent install");
        let flags = detect_enabled_clis_at_home(&home);
        assert!(flags[idx], "{cli:?} {flags:?}");
        assert_eq!(
            flags.iter().filter(|enabled| **enabled).count(),
            1,
            "{cli:?} must not switch any other CLI on: {flags:?}"
        );
        assert_eq!(server_map(cli, &path).len(), 1, "{cli:?}");

        set_cli_enabled_at_home(cli, false, 3900, &home).expect("uninstall");
        assert!(!detect_enabled_clis_at_home(&home)[idx], "{cli:?}");

        let _ = fs::remove_dir_all(home);
    }
}

/// ZCode keys its server map at the nested `mcp.servers` path, not a
/// top-level `mcpServers`. Shape and nesting both come from the shipped
/// app bundle: its config descriptor declares
/// `userConfigDirSegments:[".zcode","cli"], fileName:"config.json",
/// configKeyName:"mcp.servers"`, its reader special-cases that key name into
/// `root.mcp.servers`, and its settings form documents entries as
/// `{"type":"http","url":…}`.
#[test]
fn mcp_zcode_config_writes_the_nested_mcp_servers_path() {
    let home = temp_home("zcode");
    let path = home.join(".zcode").join("cli").join("config.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create zcode dir");
    fs::write(&path, r#"{"theme":"dark"}"#).expect("seed config");

    set_cli_enabled_at_home(McpCli::ZCode, true, 3900, &home).expect("install");

    let root = read_json_object(&path).expect("parse");
    assert!(
        !root.contains_key("mcpServers"),
        "ZCode must not get a top-level mcpServers key: {root:?}"
    );
    let entry = server_entry_for(McpCli::ZCode, &path);
    assert_eq!(entry.get("type").and_then(Value::as_str), Some("http"));
    assert_eq!(
        entry.get("url").and_then(Value::as_str),
        Some("http://127.0.0.1:3900/mcp")
    );
    assert_eq!(root.get("theme").and_then(Value::as_str), Some("dark"));

    set_cli_enabled_at_home(McpCli::ZCode, false, 3900, &home).expect("uninstall");
    let root = read_json_object(&path).expect("parse");
    assert!(!root.contains_key("mcp"), "{root:?}");
    assert_eq!(root.get("theme").and_then(Value::as_str), Some("dark"));

    let _ = fs::remove_dir_all(home);
}

/// `mcp` carries ZCode's own settings alongside `servers`, and `servers`
/// carries the user's own MCP entries. Neither may be disturbed — on install
/// or on uninstall.
#[test]
fn mcp_zcode_preserves_sibling_keys_under_mcp_and_foreign_servers() {
    let home = temp_home("zcode-siblings");
    let path = home.join(".zcode").join("cli").join("config.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create zcode dir");
    fs::write(
        &path,
        r#"{"mcp":{"timeoutMs":5000,"servers":{"other":{"type":"http","url":"https://example.com/mcp"}}}}"#,
    )
    .expect("seed config");

    set_cli_enabled_at_home(McpCli::ZCode, true, 3901, &home).expect("install");

    let servers = server_map(McpCli::ZCode, &path);
    assert_eq!(servers.len(), 2, "{servers:?}");
    assert!(servers.contains_key("other"), "{servers:?}");
    let mcp = read_json_object(&path)
        .expect("parse")
        .get("mcp")
        .and_then(Value::as_object)
        .cloned()
        .expect("mcp object");
    assert_eq!(
        mcp.get("timeoutMs").and_then(Value::as_u64),
        Some(5000),
        "sibling keys under `mcp` must survive install"
    );

    set_cli_enabled_at_home(McpCli::ZCode, false, 3901, &home).expect("uninstall");

    let servers = server_map(McpCli::ZCode, &path);
    assert_eq!(servers.len(), 1, "{servers:?}");
    assert!(servers.contains_key("other"), "{servers:?}");
    let mcp = read_json_object(&path)
        .expect("parse")
        .get("mcp")
        .and_then(Value::as_object)
        .cloned()
        .expect("mcp object must survive uninstall");
    assert_eq!(mcp.get("timeoutMs").and_then(Value::as_u64), Some(5000));

    let _ = fs::remove_dir_all(home);
}

/// Detection must not confuse ZCode's nesting with the flat layout: a
/// top-level `mcpServers` in ZCode's file is not a ZCode integration.
#[test]
fn zcode_detection_ignores_a_top_level_mcp_servers_key() {
    let home = temp_home("zcode-detect");
    let path = home.join(".zcode").join("cli").join("config.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("create zcode dir");
    fs::write(
        &path,
        r#"{"mcpServers":{"openpencil":{"type":"http","url":"http://127.0.0.1:3100/mcp"}}}"#,
    )
    .expect("seed config");

    let zcode_idx = cli_index(McpCli::ZCode);
    assert!(!detect_enabled_clis_at_home(&home)[zcode_idx]);

    let _ = fs::remove_dir_all(home);
}
