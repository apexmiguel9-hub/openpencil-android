//! Native message dialogs with a working Linux fallback chain.
//!
//! `rfd`'s default Linux backend (xdg-desktop-portal) only routes
//! *file* dialogs through the portal API; *message* dialogs shell out
//! to `zenity`. On desktops without `zenity` (KDE ships `kdialog`
//! instead) rfd logs to a logger nobody installed and returns
//! `Cancel` — so the unsaved-changes prompt silently swallowed the
//! window close and the app could only be exited via kill (#197).
//!
//! Every message dialog in this crate goes through here. On Linux the
//! helper tool is probed once per process (`zenity`, then `kdialog`)
//! and then driven directly — not through rfd — so a runtime failure
//! is *detectable*: the `ask_*` functions return `None` instead of a
//! fabricated `Cancel`, and each call site applies an explicit safe
//! policy. `alert` degrades to stderr so a message is never lost
//! silently. macOS / Windows keep the stock rfd native dialogs.

use rfd::{MessageButtons, MessageDialogResult, MessageLevel};

/// Answer to a `Yes / No (/ Cancel)` prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Choice {
    Yes,
    No,
    Cancel,
}

/// Three-way prompt. `None` means the question could not be shown at
/// all — Linux with no working `zenity` / `kdialog`.
pub(crate) fn ask_yes_no_cancel(title: &str, body: &str, level: MessageLevel) -> Option<Choice> {
    ask(title, body, level, true)
}

/// Two-way prompt; never resolves to `Some(Choice::Cancel)`. `None`
/// means the question could not be shown at all.
pub(crate) fn ask_yes_no(title: &str, body: &str, level: MessageLevel) -> Option<Choice> {
    ask(title, body, level, false)
}

/// One-button informational dialog. Degrades to stderr when the
/// dialog could not be displayed so the message is never lost.
pub(crate) fn alert(title: &str, body: &str, level: MessageLevel) {
    let displayed = match backend() {
        Backend::Rfd => {
            rfd::MessageDialog::new()
                .set_title(title)
                .set_description(body)
                .set_level(level)
                .set_buttons(MessageButtons::Ok)
                .show();
            true
        }
        Backend::Zenity => run_tool(
            "zenity",
            &[zenity_alert_flag(level), "--title", title, "--text", body],
        )
        .is_some_and(|(code, _)| alert_was_displayed(code)),
        Backend::Kdialog => run_tool(
            "kdialog",
            &["--title", title, kdialog_alert_flag(level), body],
        )
        .is_some_and(|(code, _)| alert_was_displayed(code)),
        Backend::Unavailable => false,
    };
    if !displayed {
        eprintln!("openpencil-desktop: {title}: {body}");
    }
}

fn ask(title: &str, body: &str, level: MessageLevel, three_way: bool) -> Option<Choice> {
    match backend() {
        Backend::Rfd => {
            let buttons = if three_way {
                MessageButtons::YesNoCancel
            } else {
                MessageButtons::YesNo
            };
            let result = rfd::MessageDialog::new()
                .set_title(title)
                .set_description(body)
                .set_level(level)
                .set_buttons(buttons)
                .show();
            Some(match result {
                MessageDialogResult::Yes | MessageDialogResult::Ok => Choice::Yes,
                MessageDialogResult::No => Choice::No,
                _ => Choice::Cancel,
            })
        }
        Backend::Zenity => {
            // Same grammar rfd's zenity backend uses: the stock
            // buttons are Yes / Cancel(-labelled No); the three-way
            // form adds an `--extra-button No`, which exits 1 and
            // prints its label to stdout.
            let mut args = vec!["--question", "--title", title, "--text", body];
            if three_way {
                args.extend_from_slice(&["--extra-button", "No", "--cancel-label", "Cancel"]);
            }
            let (code, stdout) = run_tool("zenity", &args)?;
            choice_from_zenity_question(code, &stdout, three_way)
        }
        Backend::Kdialog => {
            let (code, _) = run_tool(
                "kdialog",
                &[
                    "--title",
                    title,
                    kdialog_question_flag(level, three_way),
                    body,
                ],
            )?;
            choice_from_kdialog_code(code, three_way)
        }
        Backend::Unavailable => None,
    }
}

/// Which dialog machinery this system drives. Only one platform's
/// probe constructs each variant, but every platform compiles the
/// full match arms — hence the blanket dead-code allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum Backend {
    /// Stock rfd path — native dialogs on macOS / Windows.
    Rfd,
    /// Drive `zenity` directly (Linux, GNOME-family desktops).
    Zenity,
    /// Drive `kdialog` directly (Linux without zenity, e.g. KDE).
    Kdialog,
    /// No way to show a message dialog on this system.
    Unavailable,
}

#[cfg(not(target_os = "linux"))]
fn backend() -> Backend {
    Backend::Rfd
}

#[cfg(target_os = "linux")]
fn backend() -> Backend {
    use std::sync::OnceLock;
    static BACKEND: OnceLock<Backend> = OnceLock::new();
    *BACKEND.get_or_init(|| {
        if tool_answers_version("zenity") {
            Backend::Zenity
        } else if tool_answers_version("kdialog") {
            Backend::Kdialog
        } else {
            Backend::Unavailable
        }
    })
}

/// Probe `<tool> --version` with a hard cap so a wedged binary can't
/// hang the UI operation that triggered the first dialog.
#[cfg(target_os = "linux")]
fn tool_answers_version(tool: &str) -> bool {
    let child = std::process::Command::new(tool)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    for _ in 0..40 {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    false
}

/// Blocking dialog-tool invocation; returns `(exit_code, stdout)`, or
/// `None` when the tool could not be run at all.
fn run_tool(tool: &str, args: &[&str]) -> Option<(i32, String)> {
    let output = std::process::Command::new(tool)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let code = output.status.code()?;
    Some((code, String::from_utf8_lossy(&output.stdout).into_owned()))
}

/// `kdialog` question flag for the prompt shape + level. Questions
/// only have warning variants, so Info/Error use the plain forms.
fn kdialog_question_flag(level: MessageLevel, three_way: bool) -> &'static str {
    match (level, three_way) {
        (MessageLevel::Warning, true) => "--warningyesnocancel",
        (MessageLevel::Warning, false) => "--warningyesno",
        (_, true) => "--yesnocancel",
        (_, false) => "--yesno",
    }
}

/// `kdialog` one-button flag per severity.
fn kdialog_alert_flag(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Error => "--error",
        MessageLevel::Warning => "--sorry",
        MessageLevel::Info => "--msgbox",
    }
}

/// `zenity` one-button flag per severity.
fn zenity_alert_flag(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Error => "--error",
        MessageLevel::Warning => "--warning",
        MessageLevel::Info => "--info",
    }
}

/// kdialog exit codes: 0 = Yes, 1 = No, 2 = Cancel (Esc / window
/// close land on the dialog's reject code within this range). Any
/// other code means kdialog itself failed — that is a backend
/// failure (`None`), NOT a user answer: mapping it to Cancel would
/// let a broken kdialog swallow the window close again (#197).
fn choice_from_kdialog_code(code: i32, three_way: bool) -> Option<Choice> {
    match (code, three_way) {
        (0, _) => Some(Choice::Yes),
        (1, _) => Some(Choice::No),
        (2, true) => Some(Choice::Cancel),
        _ => None,
    }
}

/// zenity `--question` outcome: exit 0 = Yes; exit 1 = the extra
/// "No" button when it printed its label to stdout, else Cancel
/// (three-way) / No (two-way). Any other exit code is a backend
/// failure (`None`), not a user answer.
fn choice_from_zenity_question(code: i32, stdout: &str, three_way: bool) -> Option<Choice> {
    match code {
        0 => Some(Choice::Yes),
        1 if three_way && !stdout.trim().is_empty() => Some(Choice::No),
        1 if three_way => Some(Choice::Cancel),
        1 => Some(Choice::No),
        _ => None,
    }
}

/// One-button dialogs report 0 on OK; some tools report 1 when the
/// window is dismissed — both mean the user saw the message.
fn alert_was_displayed(code: i32) -> bool {
    code == 0 || code == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_flags_match_kdialog_grammar() {
        assert_eq!(
            kdialog_question_flag(MessageLevel::Warning, true),
            "--warningyesnocancel"
        );
        assert_eq!(
            kdialog_question_flag(MessageLevel::Warning, false),
            "--warningyesno"
        );
        assert_eq!(
            kdialog_question_flag(MessageLevel::Info, true),
            "--yesnocancel"
        );
        assert_eq!(kdialog_question_flag(MessageLevel::Error, false), "--yesno");
    }

    #[test]
    fn alert_flags_match_tool_grammar() {
        assert_eq!(kdialog_alert_flag(MessageLevel::Error), "--error");
        assert_eq!(kdialog_alert_flag(MessageLevel::Warning), "--sorry");
        assert_eq!(kdialog_alert_flag(MessageLevel::Info), "--msgbox");
        assert_eq!(zenity_alert_flag(MessageLevel::Error), "--error");
        assert_eq!(zenity_alert_flag(MessageLevel::Warning), "--warning");
        assert_eq!(zenity_alert_flag(MessageLevel::Info), "--info");
    }

    #[test]
    fn kdialog_exit_codes_map_to_choices() {
        assert_eq!(choice_from_kdialog_code(0, true), Some(Choice::Yes));
        assert_eq!(choice_from_kdialog_code(1, true), Some(Choice::No));
        assert_eq!(choice_from_kdialog_code(2, true), Some(Choice::Cancel));
        assert_eq!(choice_from_kdialog_code(0, false), Some(Choice::Yes));
        assert_eq!(choice_from_kdialog_code(1, false), Some(Choice::No));
    }

    #[test]
    fn a_failed_kdialog_is_a_backend_failure_not_an_answer() {
        // A bogus Cancel here is exactly the #197 swallow — the call
        // site must see None and take its explicit fallback instead.
        assert_eq!(choice_from_kdialog_code(254, true), None);
        assert_eq!(choice_from_kdialog_code(-1, true), None);
        assert_eq!(choice_from_kdialog_code(2, false), None);
        assert_eq!(choice_from_kdialog_code(127, false), None);
    }

    #[test]
    fn zenity_question_outcomes_map_to_choices() {
        assert_eq!(choice_from_zenity_question(0, "", true), Some(Choice::Yes));
        assert_eq!(
            choice_from_zenity_question(1, "No\n", true),
            Some(Choice::No)
        );
        assert_eq!(
            choice_from_zenity_question(1, "", true),
            Some(Choice::Cancel)
        );
        assert_eq!(choice_from_zenity_question(0, "", false), Some(Choice::Yes));
        assert_eq!(choice_from_zenity_question(1, "", false), Some(Choice::No));
    }

    #[test]
    fn a_failed_zenity_is_a_backend_failure_not_an_answer() {
        assert_eq!(choice_from_zenity_question(5, "", true), None);
        assert_eq!(choice_from_zenity_question(127, "", true), None);
        assert_eq!(choice_from_zenity_question(-1, "", false), None);
    }

    #[test]
    fn alert_display_accepts_ok_and_dismiss_codes_only() {
        assert!(alert_was_displayed(0));
        assert!(alert_was_displayed(1));
        assert!(!alert_was_displayed(2));
        assert!(!alert_was_displayed(127));
        assert!(!alert_was_displayed(-1));
    }
}
