//! Text-node emission — the reason this exporter exists.
//!
//! A text node becomes a real text element holding real characters, so
//! the presented deck can be selected, copied, searched by the browser's
//! find bar and read aloud by a screen reader. Everything else in the
//! module tree is in service of not destroying that.
//!
//! **Fonts are not embedded** (see the module-level note in
//! `export_html_structured.rs`): the family the document authored is
//! named first and a system stack follows it. A viewer without the
//! authored face sees a substituted one, which changes glyph advances.
//! It cannot move the text BOX — that is pinned by absolute position —
//! so the substitution shows up as a line breaking a word earlier or a
//! run ending short inside its own rectangle, never as a caption
//! sliding across the slide.

use op_editor_ui::layout_scene::{SceneNode, SceneTextAlign, SceneTextRun};
use op_editor_ui::{Color, Rect};
use op_util::xml_escape::escape_html;
use std::fmt::Write as _;

use super::css;

/// Painter default when a text node authored no size (mirrors
/// `canvas_viewport_text.rs`).
const DEFAULT_FONT_SIZE: f32 = 13.0;

/// Painter default text colour for a fill-less text node.
const DEFAULT_FILL: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.08,
    a: 1.0,
};

/// Generic families appended after the authored one. No `@font-face`,
/// no network request — the presented file stays self-contained.
const FALLBACK_STACK: &str = "ui-sans-serif,system-ui,-apple-system,'Segoe UI',Roboto,\
'Helvetica Neue',Arial,'PingFang SC','Hiragino Sans GB','Microsoft YaHei',sans-serif";

/// Emit one text node as an absolutely positioned text box.
///
/// Text is pinned to the TOP of its box because the canvas painter is:
/// `canvas_viewport_text.rs` draws from the node's top-left and
/// deliberately ignores `textAlignVertical` (Figma exports bake vertical
/// placement into the authored y). Honouring the field here would move
/// every imported label relative to what the editor shows.
pub fn emit(out: &mut String, n: &SceneNode, rect: Rect) {
    let Some(text) = n.text.as_deref() else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let font_size = if n.font_size > 0.0 {
        n.font_size
    } else {
        DEFAULT_FONT_SIZE
    };

    let mut style = String::new();
    css::place(&mut style, rect);
    css::decl(
        &mut style,
        "font-size",
        &format!("{}px", css::num(font_size)),
    );
    css::decl(&mut style, "font-family", &family_stack(n));
    css::decl(
        &mut style,
        "color",
        &css::color(n.fill.unwrap_or(DEFAULT_FILL)),
    );
    if n.font_weight > 0 {
        css::decl(&mut style, "font-weight", &n.font_weight.to_string());
    }
    if n.italic {
        css::decl(&mut style, "font-style", "italic");
    }
    if let Some(decoration) = decoration(n.underline, n.strikethrough) {
        css::decl(&mut style, "text-decoration", decoration);
    }
    if n.line_height > 0.0 {
        // A unitless line-height is a multiplier of the font size —
        // the same thing the scene stores, and unbounded above 1.
        css::decl(&mut style, "line-height", &css::num(n.line_height));
    }
    if n.letter_spacing != 0.0 && n.letter_spacing.is_finite() {
        css::decl(
            &mut style,
            "letter-spacing",
            &format!("{}px", css::num(n.letter_spacing)),
        );
    }
    css::decl(&mut style, "text-align", align(n.text_align));
    // `pre-wrap` wraps at the box width AND keeps authored newlines;
    // `pre` keeps the newlines while letting a non-wrapping node
    // overflow its box exactly as the canvas lets it overflow.
    css::decl(
        &mut style,
        "white-space",
        if n.text_wrap { "pre-wrap" } else { "pre" },
    );

    let _ = write!(
        out,
        r#"<div class="n t" style="{style}">{}</div>"#,
        body(n, text)
    );
}

/// The authored family (already a CSS stack per the scene contract)
/// ahead of the system fallbacks.
fn family_stack(n: &SceneNode) -> String {
    let authored = n.font_family.trim();
    if authored.is_empty() {
        FALLBACK_STACK.to_string()
    } else {
        format!("{authored},{FALLBACK_STACK}")
    }
}

/// Inner markup: the escaped text, split into `<span>`s wherever the
/// node carries styled runs so per-segment weight / size / colour
/// survives without breaking the text into unselectable fragments
/// (adjacent inline spans still select and copy as one string).
fn body(n: &SceneNode, text: &str) -> String {
    if n.text_runs.is_empty() {
        return escape_html(text);
    }
    let mut out = String::new();
    let mut cursor = 0usize;
    for run in &n.text_runs {
        let start = run.start.min(text.len());
        let end = run.end.min(text.len());
        if end <= start || start < cursor {
            // Overlapping or reversed ranges would duplicate or drop
            // characters; skipping the run keeps the flat text intact,
            // which matters more than the run's styling.
            continue;
        }
        if !is_char_boundary_pair(text, start, end) {
            continue;
        }
        if start > cursor {
            out.push_str(&escape_html(&text[cursor..start]));
        }
        let style = run_style(run);
        if style.is_empty() {
            out.push_str(&escape_html(&text[start..end]));
        } else {
            let _ = write!(
                out,
                r#"<span style="{style}">{}</span>"#,
                escape_html(&text[start..end])
            );
        }
        cursor = end;
    }
    if cursor < text.len() {
        out.push_str(&escape_html(&text[cursor..]));
    }
    out
}

/// A run whose byte range lands mid-codepoint cannot be sliced. That
/// only happens on a malformed scene, and dropping the run's styling
/// beats panicking mid-export.
fn is_char_boundary_pair(text: &str, start: usize, end: usize) -> bool {
    text.is_char_boundary(start) && text.is_char_boundary(end)
}

/// Only the attributes the run overrides; the sentinels (`0.0` size,
/// `0` weight, `None` fill) mean "inherit from the node", which an
/// omitted declaration already does.
fn run_style(run: &SceneTextRun) -> String {
    let mut style = String::new();
    if run.font_size > 0.0 {
        css::decl(
            &mut style,
            "font-size",
            &format!("{}px", css::num(run.font_size)),
        );
    }
    if run.font_weight > 0 {
        css::decl(&mut style, "font-weight", &run.font_weight.to_string());
    }
    if let Some(fill) = run.fill {
        css::decl(&mut style, "color", &css::color(fill));
    }
    if run.italic {
        css::decl(&mut style, "font-style", "italic");
    }
    if let Some(decoration) = decoration(run.underline, run.strikethrough) {
        css::decl(&mut style, "text-decoration", decoration);
    }
    style
}

fn decoration(underline: bool, strikethrough: bool) -> Option<&'static str> {
    match (underline, strikethrough) {
        (true, true) => Some("underline line-through"),
        (true, false) => Some("underline"),
        (false, true) => Some("line-through"),
        (false, false) => None,
    }
}

fn align(align: SceneTextAlign) -> &'static str {
    match align {
        SceneTextAlign::Left => "left",
        SceneTextAlign::Center => "center",
        SceneTextAlign::Right => "right",
        SceneTextAlign::Justify => "justify",
    }
}
