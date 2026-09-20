//! Argv dispatcher for the headless CLI server modes.
//!
//! The `--mcp` / `--mcp-http` / `--serve-web` dispatch itself is shared
//! with the `op-host-web-server` binary via
//! [`op_host_services::cli_modes::run_cli_mode`]; this residual only
//! decides the desktop policy around it: an unknown / missing leading
//! arg falls through to GUI mode instead of being a usage error.
//! `main` calls it before opening the GUI window.

/// If argv requests a headless server mode, run it and return `true` —
/// the caller (`main`) should then exit. Returns `false` for normal
/// GUI mode. Exits the process on a malformed invocation.
///
/// - `--mcp <path>` — JSON-RPC stdio MCP server backed by `<path>`.
/// - `--mcp-http <port> <path>` — Streamable-HTTP MCP server.
/// - `--serve-web <port> [doc] [--host <addr>]` — headless web-canvas daemon.
///
/// External CLIs (Claude Code / Codex / OpenCode / Kiro / Copilot /
/// Antigravity / Grok Build) spawn the binary in these modes to drive
/// the Rust editor.
pub fn run_cli_if_requested() -> bool {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        return false;
    };
    match op_host_services::cli_modes::run_cli_mode("openpencil-desktop", &first, args) {
        Some(0) => true,
        Some(code) => std::process::exit(code),
        // Unknown leading arg → fall through to GUI mode for now.
        None => false,
    }
}
