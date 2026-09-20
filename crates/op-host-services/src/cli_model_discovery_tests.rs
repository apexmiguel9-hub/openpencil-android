use super::*;

/// Serializes the three tests that spawn a real child process under a
/// deadline (`*_models_from_exe` against a `write_fake_cli` script).
///
/// Every other test in this file is pure parsing and runs freely in
/// parallel; these three are the only ones whose PASS/FAIL depends on wall
/// clock. The libtest harness runs them concurrently by default, so on a
/// loaded machine they were racing EACH OTHER for the same cores while each
/// one measured whether a fresh `exec` beat its own timeout — the exact
/// contention the escalating budget was invented to absorb. Serializing them
/// removes the one contention source this file controls (external `cargo
/// build` load is still absorbed by the escalation), so the budget only ever
/// has to cover genuine machine load, never sibling tests in the same binary.
///
/// Same shape as the established in-file guards elsewhere in the workspace
/// (`op-editor-core/src/agent_indicators_tests.rs`,
/// `op-host-native/src/widget_host/preview_edge_swipe_tests.rs`): a
/// `LazyLock<Mutex<()>>` whose guard is held for the body of the test, with
/// poisoning ignored so one failing test reports its own failure instead of
/// cascading into the siblings.
#[cfg(unix)]
static SUBPROCESS_PROBE_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(()));

#[cfg(unix)]
fn subprocess_probe_lock() -> std::sync::MutexGuard<'static, ()> {
    SUBPROCESS_PROBE_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

#[test]
fn parses_normal_grok_ids_and_custom_aliases_from_catalog_rows() {
    let text = "Available models:\n\
                * grok-code-fast-1 (default)\n\
                * my-model (custom)\n\
                | grok-4.1-fast | ready |\n\
                | company/sonnet:prod | configured |";
    let models = parse_grok_models(text);
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        [
            "company/sonnet:prod",
            "grok-4.1-fast",
            "grok-code-fast-1",
            "my-model",
        ]
    );

    let models =
        parse_grok_models(r#"{"models":[{"id":"grok-code-fast-1"},{"alias":"my-model"}]}"#);
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        ["grok-code-fast-1", "my-model"]
    );
}

#[test]
fn parses_custom_aliases_from_headered_tables() {
    let text = "Model | Status\n------|-------\nmy-model | default\ngrok-4.5 | ready";
    let models = parse_grok_models(text);
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        ["grok-4.5", "my-model"]
    );
}

#[test]
fn parses_antigravity_display_names_without_losing_effort_suffixes() {
    let text = "Available models:\n* Gemini 3.5 Flash (Medium)\n\
                * Claude Opus 4.6 (Thinking)\n* GPT-OSS 120B (Medium)";
    let models = parse_antigravity_models(text);
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        [
            "Claude Opus 4.6 (Thinking)",
            "GPT-OSS 120B (Medium)",
            "Gemini 3.5 Flash (Medium)",
        ]
    );
    assert!(models.iter().all(|model| model.value == model.display_name));
}

// ---------------------------------------------------------------------
// `agy models` output-format archive.
//
// Upstream has changed this format three times, and each change silently
// broke the parser until a user hit it. One version-named test per format,
// each against a REAL captured fixture:
//
//   pre-1.1.5   display names        `parses_antigravity_display_names_…`
//   1.1.5       one column of slugs  `parses_antigravity_v1_1_5_slug_catalog`
//   1.1.11      `id<TAB>display`     `parses_antigravity_v1_1_11_two_column_catalog`
//
// All three are still live inputs — users run whatever `agy` auto-updated
// them to. When it changes again: capture the real output, add the next
// version-named test, and leave the older ones alone.
// ---------------------------------------------------------------------

/// The real `agy models` stdout, captured 2026-08-07 from `agy` 1.1.11 and
/// verified byte-for-byte against the live command: one row per model,
/// `id<TAB>display name`, no heading. The tabs ARE the fixture — the
/// pre-existing samples in this file were hand-written without them, which
/// is why every test here passed while the shipped parser handed `agy` a
/// `--model` value with a tab and a display name embedded in it.
const AGY_MODELS_TSV: &str = concat!(
    "gemini-3.6-flash-high\tGemini 3.6 Flash (High)\n",
    "gemini-3.6-flash-medium\tGemini 3.6 Flash (Medium)\n",
    "gemini-3.6-flash-low\tGemini 3.6 Flash (Low)\n",
    "gemini-3.5-flash-high\tGemini 3.5 Flash (High)\n",
    "gemini-3.5-flash-medium\tGemini 3.5 Flash (Medium)\n",
    "gemini-3.5-flash-low\tGemini 3.5 Flash (Low)\n",
    "gemini-3.1-pro-high\tGemini 3.1 Pro (High)\n",
    "gemini-3.1-pro-low\tGemini 3.1 Pro (Low)\n",
    "claude-sonnet-4-6\tClaude Sonnet 4.6 (Thinking)\n",
    "claude-opus-4-6-thinking\tClaude Opus 4.6 (Thinking)\n",
    "gpt-oss-120b-medium\tGPT-OSS 120B (Medium)\n",
);

/// The block `agy` prints on stderr when it rejects a `--model` value,
/// captured the same day by running `agy --model definitely-not-a-model`.
/// It lists DISPLAY NAMES and no id column at all.
const AGY_REJECTED_MODEL_BLOCK: &str = concat!(
    "Error: invalid model selection (--model \"definitely-not-a-model\" --effort \"\"): ",
    "model definitely-not-a-model is not recognized as a known model or custom model in settings\n",
    "Available models:\n",
    "  Gemini 3.6 Flash (High)\n",
    "  Gemini 3.5 Flash (Medium)\n",
    "  Claude Opus 4.6 (Thinking)\n",
    "  GPT-OSS 120B (Medium)\n",
);

/// `agy` 1.1.11 (auto-updated 2026-08-07, ~5 hours before the first user
/// report): two columns. The 1.1.5 adaptation kept whole rows, so `--model`
/// received `"gemini-3.6-flash-high\tGemini 3.6 Flash (High)"`.
#[test]
fn parses_antigravity_v1_1_11_two_column_catalog() {
    let models = parse_antigravity_models(AGY_MODELS_TSV);
    assert_eq!(models.len(), 11, "{models:#?}");

    // The exact defect: a `--model` value that carries its own label.
    for model in &models {
        assert!(
            !model.value.contains('\t') && !model.value.contains(' '),
            "id absorbed the display column: {:?}",
            model.value
        );
    }

    let pairs: Vec<(&str, &str)> = models
        .iter()
        .map(|model| (model.value.as_str(), model.display_name.as_str()))
        .collect();
    assert!(
        pairs.contains(&("gemini-3.6-flash-high", "Gemini 3.6 Flash (High)")),
        "{pairs:#?}"
    );
    assert!(
        pairs.contains(&("claude-opus-4-6-thinking", "Claude Opus 4.6 (Thinking)")),
        "{pairs:#?}"
    );
    assert!(
        pairs.contains(&("gpt-oss-120b-medium", "GPT-OSS 120B (Medium)")),
        "{pairs:#?}"
    );
}

/// A rejected `--model` prints its own `Available models:` block. It
/// describes a run that FAILED, so it is a diagnostic and never a catalog —
/// even though its rows would parse, and even though those display names
/// happen to be usable `--model` values (`agy --model "Gemini 3.6 Flash
/// (High)"` answers normally; measured).
///
/// This is why "no id column ⇒ not a model id" would be the wrong rule:
/// pre-1.1.5 `agy models` legitimately printed display names and nothing
/// else. The distinction that holds is the SOURCE, not the shape.
#[test]
fn model_rejection_block_is_a_diagnostic_not_a_catalog() {
    assert!(
        parse_antigravity_models(AGY_REJECTED_MODEL_BLOCK).is_empty(),
        "an error block must not populate the model picker"
    );
    // …and a display-name catalog with no error in it still parses, so the
    // pre-1.1.5 format is not collateral damage. (Covered in full by
    // `parses_antigravity_display_names_without_losing_effort_suffixes`.)
    assert_eq!(
        parse_antigravity_models("Available models:\n  Gemini 3.6 Flash (High)").len(),
        1
    );
}

/// Reaching this block means `agy models` succeeded but printed something
/// we could not split, so the honest outcome is the existing
/// unrecognized-catalog fallback rather than ids that will fail later.
#[test]
fn a_rejection_block_on_stdout_reports_an_unrecognized_catalog() {
    let error = require_antigravity_models(AGY_REJECTED_MODEL_BLOCK, "")
        .unwrap_err()
        .to_string();
    assert!(error.contains("unrecognized model catalog"), "{error}");
}

/// The tripwire for the NEXT format change: an id that could not possibly
/// be a single wire value is dropped instead of being handed to `--model`.
/// Reachable through the JSON path, which takes its id verbatim.
#[test]
fn ids_carrying_a_column_separator_are_dropped_rather_than_handed_to_the_cli() {
    let models = parse_antigravity_models(
        r#"{"models":[{"id":"gemini-3.6-flash-high\tGemini 3.6 Flash (High)"},
                     {"id":"gemini-3.5-flash-low"}]}"#,
    );
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        ["gemini-3.5-flash-low"],
        "a spliced id must never survive to the wire"
    );

    // Dropping every id degrades to the catalog error the callers already
    // handle by falling back to the provider default.
    assert!(parse_antigravity_models(
        "{\"models\":[{\"id\":\"gemini-3.6-flash-high\\tGemini 3.6 Flash (High)\"}]}"
    )
    .is_empty());
}

#[test]
fn space_aligned_columns_split_only_when_the_left_cell_is_a_slug() {
    let models = parse_antigravity_models("gemini-3.1-pro-high   Gemini 3.1 Pro (High)");
    assert_eq!(models.len(), 1, "{models:#?}");
    assert_eq!(models[0].value, "gemini-3.1-pro-high");
    assert_eq!(models[0].display_name, "Gemini 3.1 Pro (High)");

    // A padded display name is one value, not an id plus its effort suffix.
    let models = parse_antigravity_models("Available models:\n* Gemini 3.5 Flash  (Medium)");
    assert_eq!(models.len(), 1, "{models:#?}");
    assert_eq!(models[0].value, "Gemini 3.5 Flash  (Medium)");
}

#[test]
fn json_object_carrying_both_columns_is_one_model_not_two() {
    let models = parse_antigravity_models(
        r#"{"models":[{"id":"gemini-3.6-flash-high","displayName":"Gemini 3.6 Flash (High)"}]}"#,
    );
    assert_eq!(models.len(), 1, "{models:#?}");
    assert_eq!(models[0].value, "gemini-3.6-flash-high");
    assert_eq!(models[0].display_name, "Gemini 3.6 Flash (High)");
}

/// The three live layouts must classify apart, or the version/format
/// breadcrumb cannot report a change. Pure function, no globals touched.
#[test]
fn each_known_catalog_layout_gets_its_own_format_code() {
    assert_eq!(catalog_format_code(AGY_MODELS_TSV), "two-column-tsv");
    assert_eq!(
        catalog_format_code("gemini-3.6-flash-high\ngemini-3.1-pro-low"),
        "slug-column"
    );
    assert_eq!(
        catalog_format_code("Available models:\n  Gemini 3.6 Flash (High)"),
        "display-names"
    );
    assert_eq!(catalog_format_code(r#"{"models":[]}"#), "json");
    assert_eq!(catalog_format_code("   \n\n"), "empty");
}

/// The breadcrumb is a CHANGE log, not a per-startup line: the same
/// version + layout prints once and then stays quiet.
///
/// Driven through `catalog_shape_change`, which RETURNS the line it would
/// log: asserting on the stored pair instead would not catch a detector
/// that logs unconditionally, because a redundant write stores the same
/// value. (Measured — an always-log injection passed that weaker test.)
/// `LAST_LOGGED_SHAPE` is touched by nothing else in this binary.
#[test]
fn the_version_breadcrumb_only_reports_a_change() {
    let first = catalog_shape_change("1.1.11", "two-column-tsv").expect("first sighting must log");
    assert!(first.contains("1.1.11"), "{first}");
    assert!(first.contains("two-column-tsv"), "{first}");

    assert!(
        catalog_shape_change("1.1.11", "two-column-tsv").is_none(),
        "an unchanged version + layout must stay silent"
    );

    let bumped = catalog_shape_change("1.1.12", "two-column-tsv")
        .expect("a version bump must log even when the layout held");
    // The line carries both sides, so the log dates the change by itself.
    assert!(
        bumped.contains("1.1.12") && bumped.contains("1.1.11"),
        "{bumped}"
    );

    let reshaped = catalog_shape_change("1.1.12", "slug-column")
        .expect("a layout change must log even when the version held");
    assert!(
        reshaped.contains("slug-column") && reshaped.contains("two-column-tsv"),
        "{reshaped}"
    );
}

/// `grok models` prints bullet rows today, so this pins the defensive
/// branch: were it ever to switch to the tab layout `agy` uses, the rows
/// must yield ids rather than being dropped whole.
#[test]
fn grok_tab_separated_rows_keep_only_the_id_column() {
    let models = parse_grok_models(
        "Available models:\ngrok-4.5\tGrok 4.5\ngrok-code-fast-1\tGrok Code Fast 1",
    );
    assert_eq!(
        models
            .iter()
            .map(|model| model.value.as_str())
            .collect::<Vec<_>>(),
        ["grok-4.5", "grok-code-fast-1"]
    );
}

#[test]
fn parses_antigravity_v1_1_5_slug_catalog() {
    let text = "gemini-3.6-flash-high\ngemini-3.6-flash-medium\ngemini-3.6-flash-low\ngemini-3.5-flash-high\ngemini-3.5-flash-medium\ngemini-3.5-flash-low\ngemini-3.1-pro-high\ngemini-3.1-pro-low\nclaude-sonnet-4-6\nclaude-opus-4-6-thinking\ngpt-oss-120b-medium";
    let models = parse_antigravity_models(text);
    assert_eq!(models.len(), 11);
    assert!(models.iter().any(|m| m.value == "gemini-3.6-flash-high"));
    assert!(models.iter().any(|m| m.value == "claude-opus-4-6-thinking"));
}

#[test]
fn parses_antigravity_json_and_ignores_auth_prose() {
    let models = parse_antigravity_models(
        r#"{"models":[{"displayName":"Gemini 3.1 Pro (High)"},{"name":"Claude Sonnet 4.6 (Thinking)"}]}"#,
    );
    assert_eq!(models.len(), 2);
    assert!(parse_antigravity_models("Please sign in to view available models").is_empty());
    assert!(parse_antigravity_models(
        "Available models:\n* Gemini authentication required\n* Claude login failed"
    )
    .is_empty());
    assert!(parse_antigravity_models(r#"{"name":"Gemini authentication required"}"#).is_empty());
}

#[test]
fn human_catalog_does_not_resume_after_its_blank_terminator() {
    let antigravity = parse_antigravity_models(
        "Available models:\n* Gemini 3.5 Flash (High)\n\n* Claude CLI troubleshooting",
    );
    assert_eq!(antigravity.len(), 1);
    assert_eq!(antigravity[0].value, "Gemini 3.5 Flash (High)");

    let grok = parse_grok_models("Available models:\n* grok-code-fast-1\n\n* release-notes-model");
    assert_eq!(grok.len(), 1);
    assert_eq!(grok[0].value, "grok-code-fast-1");
}

#[test]
fn ignores_catalog_headings_and_unrelated_prose() {
    assert!(parse_grok_models("Available models:\nDefault model: automatic").is_empty());
    assert!(parse_grok_models(
        "Available models:\nStatus: ready\nconnected\nAuthentication required"
    )
    .is_empty());
    assert!(parse_grok_models("Please sign in to continue").is_empty());
    assert!(parse_grok_models(r#""connected""#).is_empty());
    assert!(parse_grok_models(r#"{"name":"grok-diagnostic"}"#).is_empty());
    assert!(parse_grok_models("Available models:\n* request failed\n* loading-models").is_empty());
}

#[test]
fn verified_catalogs_reject_empty_auth_and_unknown_output() {
    let empty = require_antigravity_models("", "").unwrap_err().to_string();
    assert!(empty.contains("no model catalog"));

    let auth = require_antigravity_models("", "Please sign in to continue")
        .unwrap_err()
        .to_string();
    assert!(auth.contains("requires authentication"));

    let unknown = require_grok_models("Available models:\nautomatic", "")
        .unwrap_err()
        .to_string();
    assert!(unknown.contains("unrecognized model catalog"));

    let auth = require_grok_models("", "Authentication required")
        .unwrap_err()
        .to_string();
    assert!(auth.contains("requires authentication"));
}

// Large-output draining without deadlock is `bounded_cli_output`'s
// contract, exercised in `cli_probe_support`'s own test module
// (`bounded_cli_output_drains_large_stdout_and_stderr_before_exit`).

/// Starting probe budget for the hung-CLI tests, and the ceiling the
/// escalation in [`probe_until_captured`] may reach.
///
/// These are TEST HARNESS numbers only — no production timeout is derived
/// from them. What the hung-CLI tests prove is that a probe (a) reaches its
/// timeout branch and (b) still carries the output the CLI printed before the
/// deadline. Both are races against process startup: spawning a
/// just-written temp executable is milliseconds idle, but under concurrent
/// cargo builds the exec can stall long enough that a tight deadline fires
/// with an EMPTY capture, and the assert flakes. A 200 ms window flaked, then
/// 2 s flaked.
///
/// Rather than pick a bigger fixed number and hope, the tests start here and
/// RETRY with a doubled budget whenever the capture came back empty, up to
/// the cap. Idle machines pay the floor; loaded ones escalate. The cap keeps
/// a genuinely broken probe failing instead of looping, and stays well under
/// the fake CLI's own sleep so the timeout branch is still guaranteed.
///
/// The escalation is no longer the only line of defence. Two other things
/// now absorb load before it has to:
///
/// 1. The three tests that use it hold [`subprocess_probe_lock`], so they no
///    longer contend with EACH OTHER — only with load from outside this test
///    binary.
/// 2. [`write_fake_cli`] warms the spawn path before handing back a script,
///    so the first measured probe does not also pay cold `/bin/sh` + dyld +
///    temp-dir page-cache cost.
///
/// Measured, in order:
///
/// - Trio alone, five repeats, against three concurrent
///   `cargo build -p op-editor-ui` jobs (each in its own `CARGO_TARGET_DIR`
///   so they compete for CPU instead of blocking on cargo's build lock):
///   5/5 passed. Four runs took ~8.3 s — the no-escalation cost of two 4 s
///   deadlines back to back — and only the first, coldest run escalated,
///   once, to 8 s. That single cold-run escalation is what motivated the
///   warm-up in (2).
/// - Whole `cargo test -p op-host-services` (731 sibling tests in parallel)
///   during a burst of concurrent agent builds — a dozen simultaneous
///   `cargo check` / `clippy` / `test` invocations across the workspace —
///   **escalated all the way to the old 16 s cap and still captured
///   nothing**. Even the success-path test, whose script exits immediately,
///   could not spawn inside 16 s.
/// - Same trio + three-build load repeated after raising the cap to 32 s:
///   5/5 passed again. Runs 2-5 took ~8.8 s with no escalation; run 1, with
///   cold caches AND all three load builds compiling the dependency tree
///   from scratch, escalated deep and still passed. Under the old 16 s cap
///   that run would have failed.
///
/// The second measurement is why the ceiling below moved 16 s → 32 s: the
/// old cap was demonstrably reachable under real load, and a test that fails
/// at the cap is exactly the flake this is meant to remove. The floor stays
/// at 4 s so an idle machine still pays ~8 s for the pair.
#[cfg(unix)]
const PROBE_BUDGET: Duration = Duration::from_secs(4);
#[cfg(unix)]
const PROBE_BUDGET_CAP: Duration = Duration::from_secs(32);

/// How long the fake CLIs hang after printing. Must outlast
/// `PROBE_BUDGET_CAP` by a wide margin — otherwise a slow escalation could
/// see the script EXIT and the probe would report success where the test
/// demands a timeout. Raised alongside the cap (30 s → 150 s) to keep that
/// margin at ~5x rather than letting it collapse to under 2x.
///
/// Spelled `exec sleep N` by the script builders below: a forked `sleep`
/// inherits the probe's stdout/stderr pipes, so killing the shell on the
/// deadline would leave them open and the probe's reader-thread `join` would
/// block for the whole hang. `exec` makes the sleeping process the very pid
/// the probe kills, so the probe really does return on its deadline (and
/// nothing outlives the test).
#[cfg(unix)]
const FAKE_CLI_HANG_SECS: u32 = 150;

/// Run `probe` under an escalating deadline until its message shows the
/// script's output actually made it into the capture (`captured_marker`), or
/// the budget hits [`PROBE_BUDGET_CAP`]. Returns the message and the budget
/// that produced it, because the timeout wording embeds that budget's
/// seconds.
///
/// Each attempt also asserts the probe returned on its OWN deadline rather
/// than by waiting out the child: the fake CLI hangs for
/// [`FAKE_CLI_HANG_SECS`], so a return inside a small multiple of the budget
/// is proof the deadline — not the process — ended the probe. That keeps
/// "probes are deadline-bounded" under test even though the budget moved.
#[cfg(unix)]
fn probe_until_captured(
    mut probe: impl FnMut(Duration) -> String,
    captured_marker: &str,
) -> (String, Duration) {
    let mut budget = PROBE_BUDGET;
    loop {
        let started = std::time::Instant::now();
        let message = probe(budget);
        let elapsed = started.elapsed();
        // Two bounds, because neither alone is sufficient evidence at both
        // ends of the escalation. `budget * 4` is the tight one at the floor;
        // at the cap it grows past the script's own hang, so it would also
        // accept a probe that was ended by the CHILD exiting rather than by
        // its deadline. The absolute bound rules that out at every budget:
        // returning before the script could possibly have finished is what
        // "deadline-bounded" means here.
        assert!(
            elapsed < budget * 4,
            "probe must return on its own deadline ({budget:?}), not outlast the \
             {FAKE_CLI_HANG_SECS}s script; took {elapsed:?}"
        );
        assert!(
            elapsed < Duration::from_secs(u64::from(FAKE_CLI_HANG_SECS)),
            "probe must be ended by its {budget:?} deadline, not by the \
             {FAKE_CLI_HANG_SECS}s script exiting; took {elapsed:?}"
        );
        if message.contains(captured_marker) || budget >= PROBE_BUDGET_CAP {
            return (message, budget);
        }
        budget = (budget * 2).min(PROBE_BUDGET_CAP);
    }
}

/// Writes an executable `/bin/sh` script standing in for a real CLI so
/// `*_models_from_exe` can be pointed at it directly — the discover
/// chain's fixed `&["models"]` args rule out the `/bin/sh -c <script>`
/// trick `cli_probe_support`'s own tests use.
#[cfg(unix)]
fn write_fake_cli(label: &str, script_body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "openpencil-cli-model-discovery-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, format!("#!/bin/sh\n{script_body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    warm_up_spawn_path();
    path
}

/// Spawn-and-reap one trivial script from the same temp dir, so the probe
/// that follows is not the first `/bin/sh` exec of the test run.
///
/// The measured evidence for this: with the trio serialized, four of five
/// repeats needed no escalation at all, but the FIRST run consistently
/// escalated once. Nothing about the first run differs except that it pays
/// the cold costs — resolving and mapping `/bin/sh`, the dyld shared-cache
/// warm-up, and the first write+stat round trip in the temp directory. Under
/// load those are exactly the costs that push a fresh exec past a tight
/// deadline and leave the capture empty.
///
/// Warming a THROWAWAY script rather than the caller's is deliberate: the
/// scripts this module hands out hang for [`FAKE_CLI_HANG_SECS`] on purpose,
/// so running one to completion is not an option. Everything cold here is
/// shared between the two anyway.
///
/// Failures are ignored — this is an optimisation, not a precondition. If
/// the warm-up cannot run, the escalation still covers the test.
#[cfg(unix)]
fn warm_up_spawn_path() {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::temp_dir().join(format!(
        "openpencil-cli-model-discovery-warmup-{}",
        std::process::id()
    ));
    if std::fs::write(&path, "#!/bin/sh\nexit 0\n").is_ok()
        && std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).is_ok()
    {
        let _ = std::process::Command::new(&path).status();
    }
    let _ = std::fs::remove_file(&path);
}

#[cfg(unix)]
#[test]
fn antigravity_query_surfaces_auth_prompt_when_it_hangs_mid_oauth() {
    let _serialized = subprocess_probe_lock();
    // Regression coverage for the discover chain's own timeout branch:
    // the retired `command_output` (`Option<Output>`) discarded
    // whatever a hung `agy models` had already printed, so a CLI stuck
    // on first-run OAuth surfaced only a generic "failed or timed out".
    // Routed through the shared `diagnose_timeout` it now names the fix.
    let exe = write_fake_cli(
        "agy-hang",
        &format!(
            "printf 'Authentication required. Please visit the URL to log in:\\n'; \
             exec sleep {FAKE_CLI_HANG_SECS}"
        ),
    );
    // The budget must outlast spawning a just-written temp executable, not
    // just the probe's own polling: a 200ms window let a cold exec reach the
    // deadline before `printf` ran, so the assert flaked on an empty capture.
    // `probe_until_captured` escalates instead of guessing — see its docs.
    let (message, _budget) = probe_until_captured(
        |budget| {
            antigravity_models_from_exe(&exe, budget)
                .unwrap_err()
                .to_string()
        },
        "not authenticated",
    );
    assert_eq!(
        message,
        "Antigravity is not authenticated. Run `agy` once in a terminal."
    );
}

#[cfg(unix)]
#[test]
fn grok_query_falls_back_to_truncated_tail_when_timeout_has_no_auth_marker() {
    let _serialized = subprocess_probe_lock();
    let exe = write_fake_cli(
        "grok-hang",
        &format!(
            "printf 'initializing sandbox...\\nstill working\\n'; exec sleep {FAKE_CLI_HANG_SECS}"
        ),
    );
    let (message, budget) = probe_until_captured(
        |budget| grok_models_from_exe(&exe, budget).unwrap_err().to_string(),
        "still working",
    );
    // The reported duration is the budget the probe actually ran under, so
    // read it back from the attempt that produced this message rather than
    // hardcoding it.
    assert!(
        message.contains(&format!(
            "Grok Build CLI timed out after {}s",
            budget.as_secs()
        )),
        "{message}"
    );
    assert!(message.contains("`grok`"));
    assert!(message.contains("still working"));
}

#[cfg(unix)]
#[test]
fn grok_query_success_path_parses_catalog_unchanged_when_process_exits_in_time() {
    let _serialized = subprocess_probe_lock();
    // Pins the completed-output branch (parse + non-empty-catalog check)
    // exactly as it behaved before the shared bounded-probe migration.
    let exe = write_fake_cli("grok-ok", "printf 'Available models:\\n* grok-4.5\\n'");
    // The script exits immediately, so the probe returns as soon as it does —
    // a generous budget costs nothing here and removes the last way machine
    // load can turn this success-path assertion into a timeout.
    let models = grok_models_from_exe(&exe, PROBE_BUDGET_CAP).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].value, "grok-4.5");
}
