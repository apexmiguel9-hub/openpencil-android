//! Repair for dotted property keys in an object literal — `justify.content:`
//! where the schema means `justifyContent:`.
//!
//! Measured 2026-08-05 (gemini-3.6-flash, script-gen subagent): the model
//! wrote `justify.content: "space_between"` in three places of an otherwise
//! correct slide script. A bare dotted key is a `SyntaxError` at the first
//! `.`, so QuickJS rejected the file before recording a single `I(...)` call
//! and the whole page was lost to a retry — a typo in three lines cost every
//! node on the board.
//!
//! Two properties keep this narrow enough to be safe:
//!
//! 1. **It only runs after the script has already failed to eval.** A script
//!    that runs is never rewritten, so this pass cannot regress a working
//!    generation.
//! 2. **The pattern it matches is never valid JavaScript.** A key position is
//!    "the token right after `{` or `,`", and JS has no bare dotted key there
//!    — `{ a.b: 1 }` does not parse under any interpretation. So a rewrite
//!    cannot change the meaning of a program that had one.
//!
//! Everything else is left alone by construction: the scan tracks string and
//! comment state, so `"a.b: c"` inside a literal, a URL in a `src`, and a
//! template literal's body are all invisible to it. Member access on the
//! right-hand side (`item.color + "40"`, `obj.prop = x`) never sits in a key
//! position, and `cond ? a.b : c` / `case a.b:` fail the preceding-token test.

/// Rewrite dotted object-literal keys to camelCase (`justify.content:` →
/// `justifyContent:`). Returns `None` when the source has none.
pub(crate) fn repair_dotted_object_keys(script: &str) -> Option<String> {
    let chars: Vec<char> = script.chars().collect();
    let mut out = String::with_capacity(script.len());
    let mut i = 0usize;
    // The last non-whitespace character seen in code (not string / comment)
    // context. `{` or `,` means the next token is an object-literal key.
    let mut prev_code: Option<char> = None;
    let mut repaired = false;

    while i < chars.len() {
        let c = chars[i];

        // ── string + comment skipping (verbatim copy, no rewriting inside) ──
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            out.push(chars[i]);
            out.push(chars[i + 1]);
            i += 2;
            while i < chars.len()
                && !(chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/')
            {
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c == '"' || c == '\'' || c == '`' {
            out.push(c);
            i += 1;
            while i < chars.len() {
                let s = chars[i];
                out.push(s);
                i += 1;
                if s == '\\' {
                    if i < chars.len() {
                        out.push(chars[i]);
                        i += 1;
                    }
                    continue;
                }
                if s == c {
                    break;
                }
            }
            prev_code = Some(c);
            continue;
        }

        // ── key position: the token right after `{` or `,` ──
        if matches!(prev_code, Some('{') | Some(',')) && is_ident_start(c) {
            if let Some((end, camel)) = dotted_key_at(&chars, i) {
                out.push_str(&camel);
                i = end;
                prev_code = camel.chars().next_back();
                repaired = true;
                continue;
            }
        }

        out.push(c);
        if !c.is_whitespace() {
            prev_code = Some(c);
        }
        i += 1;
    }

    repaired.then_some(out)
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

/// Match `<ident>(.<ident>)+` followed by optional whitespace and `:` starting
/// at `start`. Returns the index just past the identifier chain (the colon and
/// any whitespace before it are left for the main loop) plus the camelCase
/// join. `None` when the chain has no dot or is not followed by a colon —
/// which is what keeps ordinary member access untouched.
fn dotted_key_at(chars: &[char], start: usize) -> Option<(usize, String)> {
    let mut segments: Vec<String> = Vec::new();
    let mut i = start;
    loop {
        let seg_start = i;
        while i < chars.len() && is_ident_char(chars[i]) {
            i += 1;
        }
        if i == seg_start {
            return None;
        }
        segments.push(chars[seg_start..i].iter().collect());
        if i < chars.len() && chars[i] == '.' {
            i += 1;
            continue;
        }
        break;
    }
    if segments.len() < 2 {
        return None;
    }

    let mut probe = i;
    while probe < chars.len() && chars[probe].is_whitespace() {
        probe += 1;
    }
    // A colon is required, and `::` is not a key (nothing in JS produces it,
    // but refusing it costs nothing and keeps the match unambiguous).
    if chars.get(probe) != Some(&':') || chars.get(probe + 1) == Some(&':') {
        return None;
    }

    let mut camel = segments[0].clone();
    for segment in &segments[1..] {
        let mut cs = segment.chars();
        if let Some(first) = cs.next() {
            camel.push(first.to_ascii_uppercase());
            camel.extend(cs);
        }
    }
    Some((i, camel))
}

#[cfg(test)]
mod tests {
    use super::repair_dotted_object_keys;

    #[test]
    fn rewrites_the_measured_gemini_shape() {
        let script =
            r#"I(null, {type:"frame", justify.content: "space_between", align.items:"center"});"#;
        let out = repair_dotted_object_keys(script).expect("dotted keys found");
        assert!(out.contains(r#"justifyContent: "space_between""#), "{out}");
        assert!(out.contains(r#"alignItems:"center""#), "{out}");
        assert!(!out.contains('.'), "no dotted key left: {out}");
    }

    #[test]
    fn rewrites_multi_line_and_multi_segment_keys() {
        let script = "const a = I(null, {\n  corner.radius: 24,\n  font.weight.value: 700\n});";
        let out = repair_dotted_object_keys(script).expect("dotted keys found");
        assert!(out.contains("cornerRadius: 24"), "{out}");
        assert!(out.contains("fontWeightValue: 700"), "{out}");
    }

    #[test]
    fn leaves_a_clean_script_alone() {
        let script = r#"const s = I(null, {type:"frame", justifyContent:"center"});"#;
        assert_eq!(repair_dotted_object_keys(script), None);
    }

    // ── negative cases: everything below must come back untouched ──

    #[test]
    fn ignores_dotted_text_inside_string_literals() {
        for script in [
            r#"I(null, {type:"text", text:"justify.content: space_between"});"#,
            r#"I(null, {type:"image", src:"https://cdn.test/a.b:8080/x.png"});"#,
            "I(null, {type:\"text\", text:`line\n  justify.content: center\n`});",
            r#"I(null, {type:"text", text:'{ align.items: "x" }'});"#,
        ] {
            assert_eq!(
                repair_dotted_object_keys(script),
                None,
                "rewrote inside a string: {script}"
            );
        }
    }

    #[test]
    fn ignores_member_access_on_the_right_hand_side() {
        for script in [
            r#"I(null, {fill:[{type:"solid", color: item.color + "40"}]});"#,
            "obj.prop = 1;\nI(null, {type:\"frame\"});",
            r#"const w = theme.sizes.width; I(null, {width: w});"#,
            "I(null, {height: cfg.rows.length});",
        ] {
            assert_eq!(
                repair_dotted_object_keys(script),
                None,
                "rewrote a member access: {script}"
            );
        }
    }

    #[test]
    fn ignores_colons_that_are_not_object_keys() {
        for script in [
            "const x = flag ? a.b : c; I(null, {width: x});",
            "switch (k) { case a.b: break; }",
            "I(null, {name: cond ? item.a : item.b});",
        ] {
            assert_eq!(
                repair_dotted_object_keys(script),
                None,
                "rewrote a non-key colon: {script}"
            );
        }
    }

    #[test]
    fn ignores_dotted_text_inside_comments() {
        let script = "// justify.content: space_between is wrong\n\
                      /* align.items: center */\n\
                      I(null, {type:\"frame\"});";
        assert_eq!(repair_dotted_object_keys(script), None);
    }
}
