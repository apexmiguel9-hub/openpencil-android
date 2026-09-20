//! Keeps test runs off the user's real `~/.openpencil`.
//!
//! `agent_connect_store` and `ui_prefs` persist through `op_config_store`'s
//! process-level helpers, which resolve the real user directory. Exercising
//! either from a test therefore rewrites live config — measured: a
//! `cargo test -p op-host-desktop` run rewrote `~/.openpencil/agents.json`,
//! the file that decides which agent providers auto-reconnect at launch,
//! and since the harness runs cases in parallel whichever one finished last
//! decided its contents.
//!
//! The guard sits at the persistence entry points rather than in a test
//! helper on purpose: a redirect installed by the tests themselves only
//! protects whatever runs after it, and nothing orders the harness's
//! threads. Guarding where the user root is resolved makes the isolation
//! independent of which test happens to run first.

/// Point the config store at a scratch directory the first time a desktop
/// persistence path runs inside a TEST binary.
///
/// Idempotent and shared process-wide (`redirect_user_root_for_tests` is
/// first-caller-wins), so every entry point may call it unconditionally.
/// One shared directory is fine because no test asserts on a stored file's
/// contents — they assert on in-memory state, and the only goal here is to
/// keep the writes off the user's real config.
#[cfg(test)]
pub(crate) fn guard_user_config() {
    let root = std::env::temp_dir().join(format!(
        "op-host-desktop-test-config-{}",
        std::process::id()
    ));
    op_config_store::redirect_user_root_for_tests(root);
}

/// Real builds must resolve the real `~/.openpencil` — compiles away.
#[cfg(not(test))]
#[inline]
pub(crate) fn guard_user_config() {}
