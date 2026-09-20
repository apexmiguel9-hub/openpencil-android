//! Available-component manifest: category bucketing, ordering and the
//! rendered manifest block.

use super::*;

/// Max component entries listed in the AVAILABLE COMPONENTS manifest. A large
/// harvested library (shadcn, an imported design kit) can hold hundreds of
/// masters; listing them all would blow the prompt budget, so the manifest
/// caps at this many (grouped by category, alphabetical within a category) and
/// notes the remainder.
pub(super) const MAX_COMPONENT_MANIFEST_ENTRIES: usize = 60;

/// Best-effort category bucket for a component, derived from its name. Pencil /
/// shadcn kits name components like "Primary Button", "Card", "Nav Item",
/// "Input"; bucketing by a recognised keyword groups the manifest so the model
/// scans a short, readable list instead of a flat dump.
pub(super) fn component_category(name: &str) -> &'static str {
    let n = name.to_ascii_lowercase();
    let has = |kw: &str| n.contains(kw);
    if has("button") || has("btn") || has("cta") {
        "Buttons"
    } else if has("input") || has("field") || has("textarea") || has("select") || has("search") {
        "Inputs"
    } else if has("card") || has("tile") || has("panel") {
        "Cards"
    } else if has("nav") || has("tab") || has("menu") || has("sidebar") || has("breadcrumb") {
        "Navigation"
    } else if has("badge") || has("chip") || has("tag") || has("pill") || has("label") {
        "Badges"
    } else if has("avatar") || has("icon") || has("image") || has("logo") {
        "Media"
    } else if has("modal") || has("dialog") || has("popover") || has("tooltip") || has("toast") {
        "Overlays"
    } else if has("table") || has("row") || has("list") || has("cell") {
        "Tables & Lists"
    } else if has("header") || has("footer") || has("hero") || has("section") {
        "Layout"
    } else {
        "Other"
    }
}

/// Stable category order so the manifest reads consistently across runs.
pub(super) const COMPONENT_CATEGORY_ORDER: &[&str] = &[
    "Buttons",
    "Inputs",
    "Cards",
    "Navigation",
    "Badges",
    "Media",
    "Overlays",
    "Tables & Lists",
    "Layout",
    "Other",
];

/// Build the AVAILABLE COMPONENTS manifest block for the generation prompt.
///
/// Returns `None` when the library is empty — so a no-component document's
/// prompt is byte-for-byte unchanged (the block + the `component-composition`
/// skill flag only fire when masters exist). When present, the block lists
/// `id (Name)` entries grouped by category, capped at
/// [`MAX_COMPONENT_MANIFEST_ENTRIES`], plus a one-line instruction pointing the
/// model at the `ref` + `descendants` syntax taught by the
/// `component-composition` skill.
///
/// `script_on` picks which of the two ref dialects the trailing instruction
/// teaches. The subagent path always passes `true`; `false` is retained only
/// for direct core callers/tests that still need the legacy NODE dialect.
/// - `true` (script-gen) — a single
///   `I(<containerBinding>, {"type":"ref", ...})` call.
/// - `false` (legacy flat `_parent` JSONL) — a single
///   `{"_parent":...,"id":...,"type":"ref", ...}` line.
pub(super) fn available_components_manifest(
    components: &ComponentLibrary,
    script_on: bool,
) -> Option<String> {
    if components.is_empty() {
        return None;
    }
    // Bucket components by category, preserving registry order within a bucket.
    let mut by_category: HashMap<&'static str, Vec<(&str, &str)>> = HashMap::new();
    for c in &components.components {
        by_category
            .entry(component_category(&c.name))
            .or_default()
            .push((c.id.as_str(), c.name.as_str()));
    }

    let total = components.len();
    let mut lines = vec![format!(
        "AVAILABLE COMPONENTS ({total} reusable components in this document — \
         PREFER instantiating these with a `ref` node over building from scratch):"
    )];
    let mut listed = 0usize;
    'outer: for cat in COMPONENT_CATEGORY_ORDER {
        let Some(entries) = by_category.get(*cat) else {
            continue;
        };
        if entries.is_empty() {
            continue;
        }
        lines.push(format!("{cat}:"));
        for (id, name) in entries {
            if listed >= MAX_COMPONENT_MANIFEST_ENTRIES {
                lines.push(format!(
                    "  …and {} more not listed (ask only for the ids above).",
                    total - listed
                ));
                break 'outer;
            }
            lines.push(format!("  - {id} ({name})"));
            listed += 1;
        }
    }
    // The example is COMPLETE (not just the envelope) so this block is
    // self-sufficient: even if the component-composition skill were ever trimmed
    // out, the model still has a usable, copy-pasteable instruction. Which
    // dialect it shows MUST track `script_on` — see the doc comment above.
    let instruction = if script_on {
        "To use one, call I with a single ref node — no children needed; override its text/fill \
         via `descendants`. Example:\n  \
         const cta = I(<containerBinding>, {\"type\":\"ref\",\"ref\":\"<id from above>\",\"descendants\":{\"<descendant-id>\":{\"content\":\"Get started\"}}});\n\
         Only build an element by hand when no component above fits."
    } else {
        "To use one, emit a single node — `type:\"ref\"`, the component id, its `_parent`, and \
         override its text/fill via `descendants` (it needs no `children`). Example:\n  \
         {\"_parent\":\"<container-id>\",\"id\":\"<your-id>\",\"type\":\"ref\",\"ref\":\"<id from above>\",\"descendants\":{\"<descendant-id>\":{\"content\":\"Get started\"}}}\n\
         Only build an element by hand when no component above fits."
    };
    lines.push(instruction.to_string());
    Some(lines.join("\n"))
}
