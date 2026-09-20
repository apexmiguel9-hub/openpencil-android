//! Headless OpenPencil web / MCP server binary.
//!
//! Links only `op-host-services` (the extracted headless daemon) — no
//! winit / glutin / muda / accesskit adapters / skia-GL — so the
//! container / server image stays GUI-free. The `--serve-web` / `--mcp` /
//! `--mcp-http` dispatch is shared with the desktop binary via
//! [`op_host_services::cli_modes::run_cli_mode`], but here that IS the
//! whole program: there is no GUI fallback, so an unknown / missing mode
//! is a usage error rather than "open the editor window".

use std::process::exit;

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(first) = args.next() else {
        eprintln!(
            "op-host-web-server: missing mode. Usage:\n  \
             --serve-web <port> [doc] [--host <addr>]   headless web-canvas daemon\n  \
             --mcp <path>                               JSON-RPC stdio MCP server\n  \
             --mcp-http <port> <path>                   Streamable-HTTP MCP server"
        );
        exit(2);
    };
    match op_host_services::cli_modes::run_cli_mode("op-host-web-server", &first, args) {
        Some(0) => {}
        Some(code) => exit(code),
        None => {
            eprintln!(
                "op-host-web-server: unknown mode {first:?} \
                 (expected --serve-web / --mcp / --mcp-http)"
            );
            exit(2);
        }
    }
}
