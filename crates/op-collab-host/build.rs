//! Fail the build when a release pipeline injects a malformed collaboration
//! hub URL.
//!
//! The hub endpoints reach the crate through `option_env!` (see
//! `runtime/relay_bootstrap_select.rs`) and are then held to a deliberately
//! strict endpoint policy at runtime: exact bootstrap path, https, no
//! surrounding whitespace. A secret carrying a stray space or trailing
//! newline therefore parses as "no usable hub" and every relay attempt
//! reports a transient-sounding failure — a shipped binary whose public
//! relay can never work, with nothing in the build log to say so.
//!
//! The runtime keeps refusing such values; this guard only makes the refusal
//! happen at build time, where it is actionable. An absent variable stays
//! valid: open-source and fork builds legitimately carry no production hub.
//!
//! The check restates the runtime policy instead of calling it: a build
//! script cannot depend on the crate it builds. `bootstrap_url::parse` stays
//! the authority, and it is what the crate's own tests cover — this file is
//! deliberately the looser of the two, so it can only reject what the runtime
//! would also reject.

use std::env;

const BOOTSTRAP_URL_ENVS: [&str; 2] = [
    "OPENPENCIL_BUILD_COLLAB_BOOTSTRAP_URL_CN",
    "OPENPENCIL_BUILD_COLLAB_BOOTSTRAP_URL_GLOBAL",
];

/// Mirrors the runtime endpoint policy in `relay_bootstrap.rs`.
const BOOTSTRAP_PATH: &str = "/api/v1/collaboration/bootstrap";
const MAX_URL_BYTES: usize = 2_048;

fn main() {
    for name in BOOTSTRAP_URL_ENVS {
        println!("cargo:rerun-if-env-changed={name}");
        // Absent is valid. Present-but-empty is not: a pipeline that wires
        // the variable to a missing secret sets it to the empty string, and
        // that must not read as "this build has no hub".
        let Some(value) = env::var_os(name) else {
            continue;
        };
        let value = value.to_str().unwrap_or_else(|| {
            panic!("{name} must be UTF-8");
        });
        if let Err(reason) = validate(value) {
            panic!(
                "{name} is not a usable collaboration hub URL ({reason}). \
                 Expected exactly `https://<host>{BOOTSTRAP_PATH}` with no \
                 surrounding whitespace; check the repository secret for a \
                 stray space or trailing newline."
            );
        }
    }
}

fn validate(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("the value is empty");
    }
    if value.len() > MAX_URL_BYTES {
        return Err("the value is too long");
    }
    if value.trim() != value {
        return Err("the value has leading or trailing whitespace");
    }
    if !value.is_ascii() {
        return Err("the value is not ASCII");
    }
    let Some(authority) = value.strip_prefix("https://") else {
        return Err("the scheme is not https");
    };
    let Some(host) = authority.strip_suffix(BOOTSTRAP_PATH) else {
        return Err("the path is not the bootstrap path");
    };
    if host.is_empty() {
        return Err("the host is empty");
    }
    // `@` would make the leading segment userinfo rather than the host, and
    // `/` would leave an extra path segment ahead of the bootstrap path.
    if host.contains(['@', '/', '?', '#']) {
        return Err("the authority carries userinfo, a query, or extra path");
    }
    if host != host.to_ascii_lowercase() {
        return Err("the host is not lowercase");
    }
    Ok(())
}
