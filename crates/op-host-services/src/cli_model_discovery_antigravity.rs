//! `agy models` catalog parsing.
//!
//! Split out of `cli_model_discovery.rs` to keep that file under the
//! repository's 800-line cap. The Antigravity half is the larger one
//! because upstream has changed the output format three times and each
//! generation is still a live input; the spine keeps the query/discover
//! surface and the helpers Grok shares (`trim_catalog_bullet`,
//! `strip_ansi`).

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use op_ai::agent_settings_state::AgentProvider;
use op_ai::chat_models::ModelEntry;

use super::{strip_ansi, trim_catalog_bullet};

/// Parse `agy models`.
///
/// **Upstream has changed this format three times, so all three are live
/// inputs** (see the version-named fixtures in the test sibling):
///
/// | `agy`      | shape                          |
/// | ---------- | ------------------------------ |
/// | pre-1.1.5  | display names                  |
/// | 1.1.5      | one column of kebab-case slugs |
/// | 1.1.11     | `id<TAB>display name`          |
///
/// JSON catalogs are accepted too. The returned `ModelEntry.value` is what
/// gets handed to `agy --model`, so a row's id column must never carry its
/// display column along: `agy` rejects
/// `"gemini-3.6-flash-high\tGemini 3.6 Flash (High)"` with
/// `invalid model selection`, and the failure reaches the user as a bare
/// `CLI exited with status 1`.
pub fn parse_antigravity_models(raw: &str) -> Vec<ModelEntry> {
    // A rejected `--model` prints its own `Available models:` block. That
    // block is a DIAGNOSTIC, not a catalog query — it lists display names
    // with no id column and describes a run that failed. Catalogs come
    // from `agy models` stdout and nowhere else, so refuse to mine one out
    // of an error. (Both callers already pass only `agy models` stdout and
    // short-circuit on a non-zero exit; this keeps that true by content as
    // well as by call site.)
    if looks_like_model_rejection(raw) {
        return Vec::new();
    }
    remember_catalog_format(catalog_format_code(raw));
    // id -> display label. Keyed by id because that is what identifies a
    // model to `--model`; two rows sharing an id are the same model.
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) {
        // A catalog is either a top-level array or lives under one of the
        // documented catalog wrapper fields. Do not mine arbitrary top-level
        // `name` / `id` diagnostics for model-looking strings.
        collect_antigravity_names(&value, &mut names, value.is_array());
    }

    let mut in_catalog = false;
    let mut catalog_ended = false;
    let mut saw_catalog_entry = false;
    for line in raw.lines() {
        let clean = strip_ansi(line);
        let clean = clean.trim();
        if clean.is_empty() {
            if in_catalog && saw_catalog_entry {
                in_catalog = false;
                catalog_ended = true;
            }
            continue;
        }
        let lower = clean.to_ascii_lowercase();
        if lower.contains("available models") || lower == "models:" {
            in_catalog = true;
            catalog_ended = false;
            saw_catalog_entry = false;
            continue;
        }
        let (candidate, was_bullet) = trim_catalog_bullet(clean);
        if candidate.is_empty() || is_catalog_diagnostic(candidate) {
            if in_catalog && saw_catalog_entry {
                in_catalog = false;
                catalog_ended = true;
            }
            continue;
        }
        let (id, label) = split_antigravity_row(candidate);
        let unheaded_bullet = was_bullet && !catalog_ended;
        let unheaded_model = !catalog_ended && looks_like_antigravity_model(id);
        if (in_catalog || unheaded_bullet || unheaded_model) && looks_like_antigravity_model(id) {
            names.insert(id.to_string(), label.to_string());
            if in_catalog || unheaded_bullet {
                saw_catalog_entry = true;
            }
        } else if in_catalog && saw_catalog_entry {
            in_catalog = false;
            catalog_ended = true;
        }
    }

    let total = names.len();
    let models: Vec<ModelEntry> = names
        .into_iter()
        .filter(|(id, _)| is_usable_model_id(id))
        .map(|(id, label)| ModelEntry::new(AgentProvider::Antigravity, id, label))
        .collect();
    warn_on_dropped_ids("Antigravity", "agy models", total, models.len());
    models
}

/// Which of the known `agy models` layouts this output is, as a stable code
/// for the log. Coarse on purpose — it answers "did the shape change?", not
/// "is it valid?", which is what [`is_usable_model_id`] is for.
pub(super) fn catalog_format_code(raw: &str) -> &'static str {
    if raw.trim().is_empty() {
        return "empty";
    }
    if serde_json::from_str::<serde_json::Value>(raw).is_ok() {
        return "json";
    }
    let rows: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|row| !row.is_empty())
        .collect();
    if rows.iter().any(|row| row.contains('\t')) {
        // 1.1.11
        "two-column-tsv"
    } else if rows.iter().all(|row| is_model_slug(row)) {
        // 1.1.5
        "slug-column"
    } else {
        // pre-1.1.5
        "display-names"
    }
}

/// The layout the last parse saw, so the version probe can name it without
/// re-reading the catalog. Written on every parse; read only when a connect
/// probe reports a version.
static LAST_PARSED_FORMAT: LazyLock<Mutex<Option<&'static str>>> =
    LazyLock::new(|| Mutex::new(None));

/// The `(version, format)` pair already written to the log, so a steady
/// state stays silent and only a CHANGE prints.
static LAST_LOGGED_SHAPE: LazyLock<Mutex<Option<(String, &'static str)>>> =
    LazyLock::new(|| Mutex::new(None));

fn remember_catalog_format(format: &'static str) {
    *lock(&LAST_PARSED_FORMAT) = Some(format);
}

/// Record which `agy` version produced the catalog we just parsed, and log
/// the pair when it differs from the last pair logged.
///
/// Motivation, from the 1.1.11 incident: reconstructing "the binary updated
/// at 11:03, the first user report came at 16:19" took reading the
/// executable's mtime and guessing from git history. That line belongs in
/// the log.
///
/// `version` is the string the connect probe ALREADY fetched — this spends
/// no subprocess of its own — and the whole call is informational: it
/// returns nothing and blocks nothing.
pub fn note_antigravity_catalog_version(version: &str) {
    let Some(format) = *lock(&LAST_PARSED_FORMAT) else {
        return;
    };
    note_catalog_shape(version, format);
}

/// Emit the breadcrumb if there is one to emit.
fn note_catalog_shape(version: &str, format: &'static str) {
    if let Some(line) = catalog_shape_change(version, format) {
        eprintln!("{line}");
    }
}

/// The change detector: returns the line to log when `(version, format)`
/// differs from the pair already logged, and `None` when it does not.
///
/// Returning the line instead of printing it is what makes "stays quiet on
/// an unchanged pair" observable — asserting on the stored pair cannot see
/// the difference, because a redundant write stores the same value.
///
/// The layout is passed in rather than read from the parse global so a test
/// can drive this without racing every sibling that calls
/// [`parse_antigravity_models`].
pub(super) fn catalog_shape_change(version: &str, format: &'static str) -> Option<String> {
    let seen = (version.trim().to_string(), format);
    let mut logged = lock(&LAST_LOGGED_SHAPE);
    if logged.as_ref() == Some(&seen) {
        return None;
    }
    let line = match logged.as_ref() {
        Some((was_version, was_format)) => format!(
            "[agents] Antigravity: `agy {}` prints a {} model catalog \
             (was `agy {was_version}` / {was_format})",
            seen.0, seen.1
        ),
        None => format!(
            "[agents] Antigravity: `agy {}` prints a {} model catalog",
            seen.0, seen.1
        ),
    };
    *logged = Some(seen);
    Some(line)
}

/// Lock helper that ignores poisoning: a panicking sibling must not turn
/// this bookkeeping into a second failure.
fn lock<T>(cell: &LazyLock<Mutex<T>>) -> std::sync::MutexGuard<'_, T> {
    cell.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Whether the raw text is `agy` complaining about a `--model` value rather
/// than answering a catalog query. Both markers come from the real message:
/// `Error: invalid model selection (--model "…" --effort ""): model … is not
/// recognized as a known model or custom model in settings`.
fn looks_like_model_rejection(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    lower.contains("invalid model selection")
        || lower.contains("is not recognized as a known model")
}

/// Shape self-check for a value about to be handed to `--model`.
///
/// This is the tripwire for the NEXT format change, and it is deliberately
/// not "contains no whitespace": pre-1.1.5 `agy` listed display names, and
/// `agy --model "Gemini 3.6 Flash (High)"` still answers normally, so a
/// space is legitimate. What is never legitimate is a column separator or a
/// control character inside a single wire value — that only happens when a
/// row was consumed whole instead of being split, which is exactly how the
/// 1.1.11 change broke us.
fn is_usable_model_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.chars().any(|c| c == '\t' || c.is_control())
        && id.trim() == id
}

/// Report ids the shape check refused, so an unrecognised format leaves a
/// breadcrumb at probe time instead of waiting for a user to hit
/// `CLI exited with status 1` on a real generation.
///
/// Dropping every id collapses to the existing empty-catalog path
/// (`UnrecognizedCatalog` → the provider's default model), so the fallback
/// is the one already in place rather than a new one.
fn warn_on_dropped_ids(provider: &str, command: &str, total: usize, kept: usize) {
    if kept < total {
        eprintln!(
            "[agents] {provider}: dropped {} of {total} model id(s) from `{command}` — \
             an id carried a column separator or control character, which means the \
             CLI's output format changed and the parser needs updating",
            total - kept
        );
    }
}

/// Split one catalog row into `(id, display label)`.
///
/// `agy models` (verified against 1.1.x with `od -c`) prints
/// `gemini-3.6-flash-high\tGemini 3.6 Flash (High)`. Treating the whole row
/// as the id is what made every generation fail: `agy` answers
/// `model … is not recognized as a known model or custom model in settings`
/// and exits 1.
///
/// Rows with no id column — the bare-slug catalog older builds printed, and
/// the display-name list `agy` prints when it rejects a `--model` value —
/// degrade to `(row, row)`. That is correct rather than a second bug:
/// `agy --model "Gemini 3.6 Flash (High)"` runs normally (verified by
/// running it), so a display name is a usable `--model` value.
fn split_antigravity_row(row: &str) -> (&str, &str) {
    if let Some((id, label)) = row.split_once('\t') {
        let (id, label) = (id.trim(), label.trim());
        if !id.is_empty() && !label.is_empty() {
            return (id, label);
        }
    }
    // Space-aligned columns, gated on the left cell being slug-shaped: a
    // display name padded out to a column width must stay one value, or
    // `Gemini 3.5 Flash  (Medium)` would lose its effort suffix.
    if let Some((id, label)) = split_on_column_gap(row) {
        if is_model_slug(id) && !label.is_empty() {
            return (id, label);
        }
    }
    (row, row)
}

/// Split at the first run of two or more spaces — the conventional
/// column gap in a human-formatted table.
fn split_on_column_gap(row: &str) -> Option<(&str, &str)> {
    let bytes = row.as_bytes();
    let gap = (0..bytes.len().saturating_sub(1))
        .find(|index| bytes[*index] == b' ' && bytes[index + 1] == b' ')?;
    let (left, right) = row.split_at(gap);
    Some((left.trim(), right.trim()))
}

/// Whether a string is shaped like a wire model id rather than a human
/// label: no whitespace, and only the punctuation model ids use.
fn is_model_slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/' | ':'))
}

fn collect_antigravity_names(
    value: &serde_json::Value,
    out: &mut BTreeMap<String, String>,
    catalog_context: bool,
) {
    match value {
        serde_json::Value::String(name)
            if catalog_context && looks_like_antigravity_model(name) =>
        {
            let name = name.trim().to_string();
            out.insert(name.clone(), name);
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_antigravity_names(value, out, true);
            }
        }
        serde_json::Value::Object(map) => {
            // One object is one model, so its id and its display name are
            // read together. Reading each key independently used to emit
            // `{"id": …, "displayName": …}` as two separate picker rows.
            if catalog_context {
                if let Some((id, label)) = antigravity_object_entry(map) {
                    out.insert(id, label);
                }
            }
            for (key, value) in map {
                if matches!(key.as_str(), "models" | "data" | "result" | "catalog") {
                    collect_antigravity_names(value, out, true);
                }
            }
        }
        _ => {}
    }
}

/// Read one catalog object's `(id, display label)` pair. An object carrying
/// only a display name yields it as both, matching the text path.
fn antigravity_object_entry(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<(String, String)> {
    let field = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .filter_map(|key| map.get(*key))
            .filter_map(serde_json::Value::as_str)
            .map(str::trim)
            .find(|value| !value.is_empty())
            .map(str::to_string)
    };
    let label = field(&["displayName", "display_name", "name"])
        .filter(|label| looks_like_antigravity_model(label));
    let id = field(&["id", "model"]).filter(|id| looks_like_antigravity_model(id));
    // An id that is not model-shaped falls back to the label rather than
    // discarding the object, which is what the per-key walk this replaced
    // did for `{"id": "x1", "displayName": "Gemini …"}`.
    match (id, label) {
        (Some(id), Some(label)) => Some((id, label)),
        (Some(id), None) => Some((id.clone(), id)),
        (None, Some(label)) => Some((label.clone(), label)),
        (None, None) => None,
    }
}

fn looks_like_antigravity_model(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    !is_catalog_diagnostic(&lower)
        && [
            "gemini ",
            "gemini-",
            "claude ",
            "claude-",
            "gpt-",
            "gpt ",
            "gemma ",
            "deepseek ",
            "grok ",
            "qwen ",
        ]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn is_catalog_diagnostic(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    [
        "sign in",
        "signin",
        "log in",
        "login",
        "authenticate",
        "authentication",
        "unauthorized",
        "credential",
        "api key",
        "required",
        "unavailable",
        "failed",
        "failure",
        "error:",
        "no models",
        "loading",
        "checking",
        "timed out",
        "troubleshoot",
        "documentation",
        "release notes",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}
