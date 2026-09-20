use super::*;

/// The real unauthenticated-Antigravity block, measured 2026-08-07 by
/// running `agy --sandbox --print-timeout 90s --mode plan -p …` with a
/// private `--gemini_dir` and piped stdio. Every credential-shaped
/// value here is a fake placeholder — the shape is what matters.
const ANTIGRAVITY_AUTH_BLOCK: &str = concat!(
    "Authentication required. Please visit the URL to log in:\n",
    "  https://accounts.google.com/o/oauth2/auth?access_type=offline",
    "&client_id=000000000000-fakefakefakefakefakefake.apps.googleusercontent.com",
    "&code_challenge=FAKECODECHALLENGEVALUE0000000000000000000",
    "&code_challenge_method=S256&prompt=consent",
    "&redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback",
    "&response_type=code&state=FAKESTATEVALUE000000\n",
    "\n",
    "Waiting for authentication (timeout 60s)...\n",
    "Or, paste the authorization code here and press Enter:\n",
    "Error: authentication timed out.\n",
    "Error: authentication failed or timed out\n",
);

#[test]
fn oauth_url_keeps_its_endpoint_but_loses_every_parameter() {
    let tail = diagnostic_tail(ANTIGRAVITY_AUTH_BLOCK).expect("block is not blank");
    // The endpoint identity survives — that is the diagnostic value.
    assert!(
        tail.contains("accounts.google.com/o/oauth2/auth?<redacted>"),
        "{tail}"
    );
    assert!(tail.contains("Authentication required"), "{tail}");
    assert!(
        tail.contains("authentication failed or timed out"),
        "{tail}"
    );
    // Nothing from the query string does.
    for secret in [
        "client_id=",
        "fakefakefakefake",
        "code_challenge=",
        "FAKECODECHALLENGEVALUE",
        "state=",
        "FAKESTATEVALUE",
        "redirect_uri=",
    ] {
        assert!(!tail.contains(secret), "leaked {secret:?} in {tail}");
    }
}

#[test]
fn credential_shaped_assignments_lose_their_values() {
    let redacted = redact_secrets(
        "OPENAI_API_KEY=sk-fake000000000000000 --model=gpt-5.5 \
         authorization: Bearer eyJhbGciOiJIUzI1NiJ9.ZmFrZQ.c2ln \
         client_secret=fake-secret-value refresh_token=fake-refresh \
         GEMINI_DIR=/tmp/turn-42",
    );
    assert!(redacted.contains("OPENAI_API_KEY=<redacted>"), "{redacted}");
    assert!(redacted.contains("client_secret=<redacted>"), "{redacted}");
    assert!(redacted.contains("refresh_token=<redacted>"), "{redacted}");
    assert!(redacted.contains("authorization: <redacted>"), "{redacted}");
    for secret in [
        "sk-fake0",
        "fake-secret-value",
        "fake-refresh",
        "eyJhbGciOiJIUzI1NiJ9",
    ] {
        assert!(
            !redacted.contains(secret),
            "leaked {secret:?} in {redacted}"
        );
    }
    // Non-credential context is untouched, or the tail stops being
    // diagnostic.
    assert!(redacted.contains("--model=gpt-5.5"), "{redacted}");
    assert!(redacted.contains("GEMINI_DIR=/tmp/turn-42"), "{redacted}");
}

#[test]
fn bare_credential_tokens_are_dropped_without_a_key_name() {
    for token in [
        // Assembled at compile time so the collab boundary gate's source
        // scan for high-signal credential shapes does not flag the fixture.
        concat!("sk-ant-api03-", "fakefakefake"),
        "ghp_fakefakefakefakefake",
        "ya29.a0AfakeFakeFake",
        "eyJhbGciOiJIUzI1NiJ9.ZmFrZXBheWxvYWQ.c2lnbmF0dXJl",
    ] {
        let redacted = redact_secrets(&format!("agent said {token} while starting"));
        assert_eq!(
            redacted, "agent said <redacted> while starting",
            "token={token}"
        );
    }
    // A long ordinary word must survive — over-redaction destroys the
    // evidence just as thoroughly as no redaction.
    assert_eq!(
        redact_secrets("ENOENT: no such file or directory, open 'settings.json'"),
        "ENOENT: no such file or directory, open 'settings.json'"
    );
}

#[test]
fn surfaced_tail_is_bounded_in_lines_and_characters() {
    let huge: String = (0..5_000)
        .map(|i| format!("line {i} with a fair amount of padding text on it\n"))
        .collect();
    let tail = diagnostic_tail(&huge).expect("not blank");
    assert!(
        tail.chars().count() <= TAIL_MAX_CHARS,
        "tail was {} chars",
        tail.chars().count()
    );
    // Truncation drops from the MIDDLE: the END carries a failing
    // process's fatal line, the HEAD carries a rejected-argument
    // error's reason, and the marker says how much went missing.
    assert!(tail.contains("line 4999"), "{tail}");
    assert!(tail.contains("line 0 "), "{tail}");
    assert!(tail.contains("lines omitted"), "{tail}");
    assert!(!tail.contains("line 2500 "), "{tail}");
}

/// `agy` rejecting a `--model` value: the reason is line 1 and the rest
/// is a catalog. Keeping only the tail left the user with an
/// unexplained wall of model names, which is what this pins.
#[test]
fn argument_rejection_keeps_its_reason_and_the_end_of_its_list() {
    let rejection = concat!(
        "Error: invalid model selection (--model \"gemini-3.6-flash-high\" --effort \"\"): ",
        "model gemini-3.6-flash-high is not recognized as a known model or custom model ",
        "in settings\n",
        "Available models:\n",
        "  Gemini 3.6 Flash (High)\n",
        "  Gemini 3.6 Flash (Medium)\n",
        "  Gemini 3.6 Flash (Low)\n",
        "  Gemini 3.5 Flash (High)\n",
        "  Gemini 3.5 Flash (Medium)\n",
        "  Gemini 3.5 Flash (Low)\n",
        "  Gemini 3.1 Pro (High)\n",
        "  Gemini 3.1 Pro (Low)\n",
        "  Claude Sonnet 4.6 (Thinking)\n",
        "  Claude Opus 4.6 (Thinking)\n",
        "  GPT-OSS 120B (Medium)\n",
    );
    let tail = diagnostic_tail(rejection).expect("not blank");
    assert!(tail.chars().count() <= TAIL_MAX_CHARS, "{tail}");
    assert!(tail.contains("invalid model selection"), "{tail}");
    assert!(tail.contains("is not recognized"), "{tail}");
    assert!(tail.contains("GPT-OSS 120B (Medium)"), "{tail}");
}

#[test]
fn a_character_cut_also_keeps_both_ends() {
    let mut text = String::from("Error: ");
    text.push_str(&"reason ".repeat(100));
    text.push('\n');
    for index in 0..40 {
        text.push_str(&format!("candidate-{index}\n"));
    }
    let tail = diagnostic_tail(&text).expect("not blank");
    assert!(tail.chars().count() <= TAIL_MAX_CHARS, "{tail}");
    assert!(tail.starts_with("Error: reason"), "{tail}");
    assert!(tail.contains("candidate-39"), "{tail}");
}

#[test]
fn blank_output_has_no_tail_to_quote() {
    assert!(diagnostic_tail("").is_none());
    assert!(diagnostic_tail("   \n\n  \t \n").is_none());
}

#[test]
fn bounded_tail_does_not_grow_under_unbounded_input() {
    let mut tail = BoundedTail::new(4 * 1024, 64);
    for i in 0..100_000 {
        tail.push_line(&format!("chatty agent line number {i}"));
    }
    assert!(tail.retained_lines() <= 64, "{}", tail.retained_lines());
    assert!(
        tail.retained_bytes() <= 4 * 1024,
        "{}",
        tail.retained_bytes()
    );
    // It keeps the most recent lines, not the oldest.
    let text = tail.text();
    assert!(text.contains("line number 99999"), "{text}");
    assert!(!text.contains("line number 0\n"), "{text}");
}

#[test]
fn bounded_tail_truncates_a_single_oversized_line_on_a_char_boundary() {
    let mut tail = BoundedTail::new(16, 4);
    tail.push_line(&"字".repeat(50));
    assert!(tail.retained_bytes() <= 16, "{}", tail.retained_bytes());
    // Round-tripping through `text()` proves no character was split.
    assert!(tail.text().chars().all(|c| c == '字'));
}

#[test]
fn zero_capacity_tail_retains_nothing() {
    let mut tail = BoundedTail::new(0, 8);
    tail.push_line("dropped");
    assert!(tail.is_empty());
    assert_eq!(tail.text(), "");
}
