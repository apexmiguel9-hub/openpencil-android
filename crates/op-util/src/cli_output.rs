//! Bounded, redacted tails of child-process output.
//!
//! Two crates need the same thing and cannot share it any other way:
//! `op-host-services` bridges coding-agent CLIs over stdio, and `op-acp`
//! spawns local ACP agents. Both drain the child's stderr so a full pipe
//! can never block the child, and both used to throw every byte away —
//! so a failing child produced an error message with no evidence in it
//! (`CLI exited with status 1`).
//!
//! Keeping the evidence has two hard constraints:
//!
//! 1. **Bounded.** Draining must stay O(1) in memory no matter how much
//!    the child prints, and the surfaced text has to fit in a chat
//!    bubble and one log field. [`BoundedTail`] caps both bytes and
//!    lines; [`diagnostic_tail`] caps the surfaced string again.
//! 2. **Redacted.** Child output routinely carries OAuth URLs
//!    (`client_id` / `code_challenge` / `state`), API keys, and bearer
//!    tokens. No credential may reach a log file, a saved document, or
//!    the UI. [`redact_secrets`] runs BEFORE truncation so a cut can
//!    never leave half a secret behind.

use std::collections::VecDeque;

/// Longest surfaced tail, in characters. Sized to stay readable in a
/// chat bubble and in a single `tracing` field while still carrying a
/// multi-line CLI failure (a full unauthenticated-Antigravity block
/// redacts down to ~300 characters).
pub const TAIL_MAX_CHARS: usize = 600;

/// Most CONTENT lines a surfaced tail may carry. Overflow is dropped
/// from the middle: [`HEAD_MAX_LINES`] are kept from the front and the
/// remainder from the back, joined by an elision marker that does not
/// count against this budget.
pub const TAIL_MAX_LINES: usize = 10;

/// Lines kept from the FRONT of an over-long output.
///
/// Keeping only the end is right for a panic backtrace, and wrong for
/// the far more common shape where a CLI states its reason on line 1
/// and then prints a long list. `agy` rejecting a `--model` value is
/// the measured case: line 1 is `Error: invalid model selection (…):
/// model … is not recognized`, and the ~11 lines after it are just the
/// catalog. A pure tail dropped the only line that said why, and the
/// user saw an unexplained wall of model names.
pub const HEAD_MAX_LINES: usize = 3;

/// Written where lines were dropped from the middle, as its own joined
/// element so the count is visible rather than implied.
fn line_elision(omitted: usize) -> String {
    format!("[{omitted} lines omitted]")
}

/// Written where CHARACTERS were dropped from the middle.
const CHAR_ELISION: &str = " … ";

/// Numerator / denominator of the character budget reserved for the
/// head when the joined text still overflows. Two fifths leaves the
/// larger share to the end (where a fatal last line lives) while still
/// fitting a long first line's reason clause.
const HEAD_CHAR_NUM: usize = 2;
const HEAD_CHAR_DEN: usize = 5;

/// Placeholder written in place of every redacted value.
const REDACTED: &str = "<redacted>";

/// A fixed-capacity view of the most recent lines a child printed.
///
/// Both caps are enforced on every push, so a child that prints
/// forever occupies a constant amount of memory. Overflow drops whole
/// lines from the front; a single line longer than the byte budget is
/// truncated to its head (which is where a message's meaning lives).
#[derive(Debug, Clone)]
pub struct BoundedTail {
    lines: VecDeque<String>,
    bytes: usize,
    max_bytes: usize,
    max_lines: usize,
}

impl BoundedTail {
    /// A tail holding at most `max_lines` lines and `max_bytes` bytes.
    /// Zero for either cap yields a sink that retains nothing.
    pub fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            lines: VecDeque::new(),
            bytes: 0,
            max_bytes,
            max_lines,
        }
    }

    /// Append one line, then evict from the front until both caps hold.
    pub fn push_line(&mut self, line: &str) {
        if self.max_bytes == 0 || self.max_lines == 0 {
            return;
        }
        let line = truncate_chars_to_bytes(line, self.max_bytes);
        self.bytes += line.len();
        self.lines.push_back(line.to_owned());
        while self.lines.len() > self.max_lines || self.bytes > self.max_bytes {
            match self.lines.pop_front() {
                Some(dropped) => self.bytes -= dropped.len(),
                None => break,
            }
        }
    }

    /// The retained lines joined by newlines.
    pub fn text(&self) -> String {
        let mut joined = String::with_capacity(self.bytes + self.lines.len());
        for (index, line) in self.lines.iter().enumerate() {
            if index > 0 {
                joined.push('\n');
            }
            joined.push_str(line);
        }
        joined
    }

    /// Whether anything has been retained.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Retained bytes, excluding the joining newlines.
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }

    /// Retained line count.
    pub fn retained_lines(&self) -> usize {
        self.lines.len()
    }
}

/// The user-facing / log-facing form of a child's output: redacted,
/// collapsed onto one line, and length-capped. `None` when the child
/// printed nothing worth quoting — the caller says "no output captured"
/// rather than dangling an empty quote.
///
/// Lines are joined with ` | ` so the result stays a single log field
/// and a single readable sentence in a chat bubble.
pub fn diagnostic_tail(text: &str) -> Option<String> {
    diagnostic_tail_capped(text, TAIL_MAX_CHARS)
}

/// [`diagnostic_tail`] with a caller-chosen character budget, for call
/// sites that append the tail to a message that already says something
/// (a friendly classification) and want the quote to stay subordinate
/// to it.
pub fn diagnostic_tail_capped(text: &str, max_chars: usize) -> Option<String> {
    // Redact first: truncating first could cut mid-secret and leave a
    // usable prefix behind.
    let redacted = redact_secrets(text);
    let lines: Vec<&str> = redacted
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    if lines.is_empty() {
        return None;
    }
    let joined = join_head_and_tail(&lines);
    Some(keep_head_and_tail(&joined, max_chars))
}

/// Join the retained lines with ` | `, dropping from the MIDDLE when
/// there are more than [`TAIL_MAX_LINES`] of them.
fn join_head_and_tail(lines: &[&str]) -> String {
    if lines.len() <= TAIL_MAX_LINES {
        return lines.join(" | ");
    }
    let head = HEAD_MAX_LINES.min(TAIL_MAX_LINES);
    let tail = TAIL_MAX_LINES - head;
    let mut parts: Vec<String> = lines[..head].iter().map(|line| line.to_string()).collect();
    parts.push(line_elision(lines.len() - head - tail));
    parts.extend(
        lines[lines.len() - tail..]
            .iter()
            .map(|line| line.to_string()),
    );
    parts.join(" | ")
}

/// Replace credential-shaped values with [`REDACTED`], preserving the
/// surrounding text so the message still reads.
///
/// Four rules, applied per whitespace-delimited token:
///
/// - an `http(s)://` URL keeps scheme + host + path and loses its whole
///   query and fragment (this is what kills OAuth `client_id` /
///   `code_challenge` / `state` in one stroke);
/// - `key=value` loses the value when the key is credential-shaped;
/// - the token after `Bearer`, or after a credential-shaped `key:`,
///   is a value and is dropped;
/// - a token carrying a well-known credential prefix (`sk-`, `ghp_`,
///   `ya29.`, a JWT, …) is dropped whole.
pub fn redact_secrets(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        redact_line(line, &mut out);
    }
    out
}

fn redact_line(line: &str, out: &mut String) {
    let mut previous = String::new();
    let mut rest = line;
    while !rest.is_empty() {
        let gap = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        out.push_str(&rest[..gap]);
        rest = &rest[gap..];
        if rest.is_empty() {
            break;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let token = &rest[..end];
        rest = &rest[end..];
        out.push_str(&redact_token(token, &previous));
        previous = token.to_ascii_lowercase();
    }
}

fn redact_token(token: &str, previous_lower: &str) -> String {
    if let Some(url_start) = find_url_start(token) {
        return redact_url(token, url_start);
    }
    if introduces_a_secret(previous_lower) {
        return REDACTED.to_string();
    }
    if let Some((key, value)) = token.split_once('=') {
        if !value.is_empty() && is_secret_key(key) {
            return format!("{key}={REDACTED}");
        }
    }
    if let Some((key, value)) = token.split_once(':') {
        if !value.is_empty() && is_secret_key(key) {
            return format!("{key}:{REDACTED}");
        }
    }
    if has_credential_prefix(token) {
        return REDACTED.to_string();
    }
    token.to_string()
}

/// Byte offset of an embedded `http://` / `https://`, so a URL wrapped
/// in brackets or quotes is still recognised.
fn find_url_start(token: &str) -> Option<usize> {
    let lower = token.to_ascii_lowercase();
    let http = lower.find("http://");
    let https = lower.find("https://");
    match (http, https) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Keep everything up to the first `?` or `#`; the rest is parameters,
/// which is exactly where OAuth secrets live.
fn redact_url(token: &str, url_start: usize) -> String {
    let (prefix, url) = token.split_at(url_start);
    let cut = url.find(['?', '#']);
    match cut {
        Some(at) => {
            let (kept, dropped) = url.split_at(at);
            let marker = &dropped[..1];
            format!("{prefix}{kept}{marker}{REDACTED}")
        }
        None => token.to_string(),
    }
}

/// Whether the PREVIOUS token means "the next token is a credential" —
/// `Bearer <token>` and `Authorization: <token>` shapes.
fn introduces_a_secret(previous_lower: &str) -> bool {
    let trimmed = previous_lower.trim_end_matches([':', '=']);
    trimmed == "bearer" || (previous_lower.ends_with(':') && is_secret_key(trimmed))
}

/// Credential-shaped key names. Exact matches first, then the suffix
/// families (`*_token` / `*_secret` / `*_key` / `*_password`) so
/// provider-specific names we have never seen are still covered.
fn is_secret_key(key: &str) -> bool {
    const EXACT: &[&str] = &[
        "access_token",
        "api_key",
        "apikey",
        "auth",
        "authorization",
        "authtoken",
        "bearer",
        "client_id",
        "client_secret",
        "clientid",
        "code",
        "code_challenge",
        "code_verifier",
        "cookie",
        "credential",
        "credentials",
        "id_token",
        "key",
        "passphrase",
        "passwd",
        "password",
        "pwd",
        "refresh_token",
        "secret",
        "session",
        "session_id",
        "signature",
        "state",
        "token",
    ];
    const SUFFIXES: &[&str] = &["_token", "_secret", "_key", "_password", "_credential"];
    let normalized: String = key
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_ascii_lowercase()
        .replace('-', "_");
    EXACT.contains(&normalized.as_str())
        || SUFFIXES
            .iter()
            .any(|suffix| normalized.ends_with(suffix) && normalized.len() > suffix.len())
}

/// Well-known credential shapes that carry no key name at all.
fn has_credential_prefix(token: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "sk-",
        "sk_live_",
        "sk_test_",
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "github_pat_",
        "xai-",
        "xoxb-",
        "xoxp-",
        "ya29.",
        "AIza",
        "ASIA",
        "AKIA",
    ];
    let bare = token.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_');
    if bare.len() < 12 {
        return false;
    }
    if PREFIXES.iter().any(|prefix| bare.starts_with(prefix)) {
        return true;
    }
    // JWT: three base64url segments, and the header segment of a JSON
    // header always encodes to `eyJ`.
    bare.starts_with("eyJ") && bare.matches('.').count() == 2
}

/// Truncate to at most `max_bytes`, never splitting a character.
fn truncate_chars_to_bytes(line: &str, max_bytes: usize) -> &str {
    if line.len() <= max_bytes {
        return line;
    }
    let mut end = max_bytes;
    while end > 0 && !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

/// Keep BOTH ends within `max_chars` characters, marking the cut.
///
/// A failing process puts its fatal line last, but an argument-
/// validation failure puts its reason first and then pads the output
/// with candidates — so a cut that only preserves one end is wrong
/// half the time. The head takes [`HEAD_CHAR_NUM`]/[`HEAD_CHAR_DEN`]
/// of the budget and the tail the rest; the marker is charged to the
/// budget too, so the result is never longer than `max_chars`.
fn keep_head_and_tail(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let marker = CHAR_ELISION.chars().count();
    if max_chars <= marker {
        // No room to mark a middle cut, let alone keep two ends.
        return keep_last_chars(text, max_chars);
    }
    let body = max_chars - marker;
    let head = body * HEAD_CHAR_NUM / HEAD_CHAR_DEN;
    let tail = body - head;
    let mut kept: String = text.chars().take(head).collect();
    kept.push_str(CHAR_ELISION);
    kept.extend(text.chars().skip(total - tail));
    kept
}

/// Keep the LAST `max_chars` characters, marking the cut. Used when the
/// budget is too small to carry a middle-elision marker at all.
fn keep_last_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    // The ellipsis counts against the budget, so the returned string is
    // never longer than `max_chars` — callers assert on that bound.
    let keep = max_chars.saturating_sub(1);
    let mut kept = String::from('…');
    kept.extend(text.chars().skip(total - keep));
    kept
}

#[cfg(test)]
#[path = "cli_output_tests.rs"]
mod tests;
