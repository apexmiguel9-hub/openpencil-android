//! Shared design.md ("Design System brief") generation support.
//!
//! Three hosts generate design.md from the open document — the desktop
//! app (`op-host-desktop::design_md_host`), the serve-web daemon
//! (`op-host-services::design_md_llm`), and the browser shell
//! (`op-host-web::web_design_md`). They share the system prompt and the
//! LLM-output cleanup chain below; transport and user-prompt framing
//! stay host-side.

/// System prompt for design.md generation, used verbatim by the desktop
/// and web hosts. The serve-web orchestrator flow splices two extra
/// workflow rules in via [`design_md_system_prompt_with_extra_rules`].
pub const DESIGN_MD_SYSTEM_PROMPT: &str = r##"You are a Design Systems Lead. Analyze the provided PenNode design tree and generate a comprehensive design.md in the Google Stitch format.

OUTPUT FORMAT — a complete markdown document with these sections:

# Design System: [Project Name]

## 1. Visual Theme & Atmosphere
Describe the mood, density, and aesthetic philosophy using evocative adjectives.

## 2. Color Palette & Roles
For each color found in the design:
- **Descriptive Name** (#HEX) — Functional role (e.g. "Primary CTA", "Background", "Body text")

## 3. Typography Rules
- Font families used, weight hierarchy, size scale, line-height conventions.

## 4. Component Stylings
- **Buttons**: shape, colors, padding, states
- **Cards**: corners, shadows, internal padding
- **Inputs**: borders, backgrounds
- **Navigation**: layout, spacing

## 5. Layout Principles
- Grid system, whitespace strategy, spacing units, responsive breakpoints.

## 6. Design System Notes
- Key language/terms to use when generating new designs in this style.

RULES:
- Use descriptive natural language, NOT technical jargon (e.g. "subtly rounded corners" not "rounded-lg").
- Pair ALL colors with exact hex codes.
- Explain functional roles for every design element.
- Output ONLY the markdown document, starting with "# Design System:".
- NO preamble, NO commentary, NO tool calls, NO code fences around the output.
- Do NOT use <tool_call> tags or any tool invocations. Just output the markdown text directly."##;

/// First output-format rule — host-specific extra rules splice in
/// directly before this line so they read as content rules, not
/// output-format rules.
const OUTPUT_RULES_MARKER: &str = "- Output ONLY the markdown document";

/// Character cap for the serialized design-tree JSON in the user prompt.
pub const DESIGN_MD_MAX_TREE_CHARS: usize = 24_000;
/// Character cap for the serialized design-variables JSON in the user prompt.
pub const DESIGN_MD_MAX_VAR_CHARS: usize = 6_000;

/// [`DESIGN_MD_SYSTEM_PROMPT`] with host-specific rule lines inserted
/// before the output-format rules. Each entry must be a complete
/// `- …` rule line without a trailing newline.
pub fn design_md_system_prompt_with_extra_rules(extra_rules: &[&str]) -> String {
    if extra_rules.is_empty() {
        return DESIGN_MD_SYSTEM_PROMPT.to_string();
    }
    let joined = extra_rules.join("\n");
    match DESIGN_MD_SYSTEM_PROMPT.find(OUTPUT_RULES_MARKER) {
        Some(idx) => format!(
            "{}{}\n{}",
            &DESIGN_MD_SYSTEM_PROMPT[..idx],
            joined,
            &DESIGN_MD_SYSTEM_PROMPT[idx..]
        ),
        None => format!("{DESIGN_MD_SYSTEM_PROMPT}\n{joined}"),
    }
}

/// Truncate to at most `max_chars` characters, appending a truncation
/// marker when anything was cut.
pub fn truncate_chars(input: &str, max_chars: usize) -> String {
    let mut chars = input.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push_str("\n... [truncated]");
    }
    out
}

/// Clean a raw LLM design.md answer into the bare markdown document:
/// strips `<tool_call>` blocks, an outer code fence, any preamble before
/// the first `# ` heading, and stray tool-JSON lines.
pub fn clean_ai_design_md_result(raw: &str) -> String {
    let mut text = strip_tool_call_blocks(raw.trim());
    text = strip_code_fence(text);
    if let Some(start) = text.find("# ") {
        text = text[start..].to_string();
    }
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("{\"name\"")
                && !trimmed.starts_with("{\"tool_use_id\"")
                && !trimmed.starts_with("{\"file_path\"")
                && trimmed != "<tool_call>"
                && trimmed != "</tool_call>"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace("\n\n\n\n", "\n\n\n")
        .trim()
        .to_string()
}

fn strip_code_fence(mut text: String) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return text;
    }
    text = trimmed.to_string();
    if let Some(idx) = text.find('\n') {
        text = text[idx + 1..].to_string();
    }
    if let Some(idx) = text.rfind("```") {
        text.truncate(idx);
    }
    text.trim().to_string()
}

fn strip_tool_call_blocks(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find("<tool_call>") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let after_start = &rest[start + "<tool_call>".len()..];
        let Some(end) = after_start.find("</tool_call>") else {
            break;
        };
        rest = &after_start[end + "</tool_call>".len()..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extra_rules_are_spliced_in_before_the_output_rules() {
        let prompt = design_md_system_prompt_with_extra_rules(&["- Rule A.", "- Rule B."]);
        let roles = prompt
            .find("- Explain functional roles for every design element.")
            .unwrap();
        let a = prompt.find("- Rule A.").unwrap();
        let b = prompt.find("- Rule B.").unwrap();
        let output_only = prompt.find(OUTPUT_RULES_MARKER).unwrap();
        assert!(roles < a && a < b && b < output_only);
        // Splicing keeps every rule on its own line.
        assert!(prompt.contains("- Rule A.\n- Rule B.\n- Output ONLY"));
    }

    #[test]
    fn no_extra_rules_returns_the_shared_prompt_verbatim() {
        assert_eq!(
            design_md_system_prompt_with_extra_rules(&[]),
            DESIGN_MD_SYSTEM_PROMPT
        );
    }

    #[test]
    fn truncate_chars_caps_and_marks() {
        assert_eq!(truncate_chars("short", 10), "short");
        assert_eq!(truncate_chars("abcdef", 3), "abc\n... [truncated]");
    }

    #[test]
    fn clean_strips_fence_and_tool_call_noise() {
        let raw = "```markdown\n<tool_call>{\"name\":\"x\"}</tool_call># Design System: App\n\nBody\n{\"tool_use_id\": 1}\n```";
        let cleaned = clean_ai_design_md_result(raw);
        assert_eq!(cleaned, "# Design System: App\n\nBody");
    }

    #[test]
    fn clean_drops_preamble_before_the_first_heading() {
        let raw = "Here you go:\n\n# Design System: App\n\nBody";
        assert_eq!(
            clean_ai_design_md_result(raw),
            "# Design System: App\n\nBody"
        );
    }

    #[test]
    fn clean_keeps_plain_markdown_untouched() {
        let raw = "# Design System: App\n\n## 1. Visual Theme & Atmosphere\nCalm.";
        assert_eq!(clean_ai_design_md_result(raw), raw);
    }
}
