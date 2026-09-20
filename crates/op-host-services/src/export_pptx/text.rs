//! Text emission — the reason a structured `.pptx` beats a deck of
//! screenshots.
//!
//! A text node becomes a real PowerPoint text box holding real
//! characters, so the presenter can fix a typo on stage, the reviewer
//! can leave a comment on a sentence, and the whole deck stays
//! searchable. Everything else in this module tree exists so that this
//! one keeps working.
//!
//! **Fonts are named, not embedded.** The authored family is written
//! into each run and PowerPoint substitutes when the machine does not
//! have it. A substituted face changes glyph advances, but every box is
//! pinned by absolute position and given explicit line spacing, so the
//! drift stays inside the box — a line wrapping a word early, never a
//! caption sliding across the slide.
//!
//! **Line spacing is stated in points, not percent.** PowerPoint's
//! percentage spacing multiplies the FONT's natural line height (about
//! 1.2 em), so a design authored at `lineHeight: 1.2` would come out at
//! roughly 1.44 em — visibly looser, and looser by a different amount
//! per font. `spcPts` states the exact measure the canvas laid out with.

use op_editor_ui::layout_scene::{SceneNode, SceneTextAlign, SceneTextRun};
use op_editor_ui::{Color, Rect};
use op_util::xml_escape::escape_xml;
use std::fmt::Write as _;

use super::units::{font_hundredths_pt, hundredths_pt, solid_fill};
use super::xml::{nv_sp_pr, sp_pr, xfrm};

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

/// Emit one text node as a text box.
///
/// Text is pinned to the TOP of its box because the canvas painter is:
/// `canvas_viewport_text.rs` draws from the node's top-left and
/// deliberately ignores `textAlignVertical` (Figma exports bake vertical
/// placement into the authored y). Honouring the field here would move
/// every imported label relative to what the editor shows.
pub fn emit(out: &mut String, node: &SceneNode, rect: Rect, alpha: f32, id: u32) {
    let Some(text) = node.text.as_deref() else {
        return;
    };
    if text.is_empty() {
        return;
    }
    let font_size = if node.font_size > 0.0 {
        node.font_size
    } else {
        DEFAULT_FONT_SIZE
    };

    let _ = write!(
        out,
        "<p:sp>{}{}<p:txBody>{}<a:lstStyle/>{}</p:txBody></p:sp>",
        nv_sp_pr(id, &node.id, true),
        sp_pr(
            &xfrm(rect, node),
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>",
            "<a:noFill/>",
            None,
            ""
        ),
        body_pr(node),
        paragraphs(node, text, font_size, alpha)
    );
}

/// `<a:bodyPr>`: zero insets (the scene rect IS the text box, with no
/// padding of its own), no autofit (PowerPoint must not resize the type
/// the canvas already measured), and wrapping only where the document
/// asked for it.
fn body_pr(node: &SceneNode) -> String {
    let wrap = if node.text_wrap { "square" } else { "none" };
    format!(
        "<a:bodyPr wrap=\"{wrap}\" lIns=\"0\" tIns=\"0\" rIns=\"0\" bIns=\"0\" rtlCol=\"0\" \
anchor=\"t\"><a:noAutofit/></a:bodyPr>"
    )
}

/// One `<a:p>` per authored line.
///
/// Splitting on `\n` rather than emitting `<a:br/>` keeps each line a
/// paragraph, which is what carries the line-spacing and alignment
/// properties — a break inside one paragraph would inherit them, but an
/// empty line would then collapse to nothing.
fn paragraphs(node: &SceneNode, text: &str, font_size: f32, alpha: f32) -> String {
    let props = paragraph_props(node, font_size);
    let mut out = String::new();
    let mut offset = 0usize;
    for line in text.split('\n') {
        let start = offset;
        let end = start + line.len();
        // `+ 1` steps over the '\n' that `split` consumed.
        offset = end + 1;
        out.push_str("<a:p>");
        out.push_str(&props);
        if line.is_empty() {
            // An empty paragraph with no run has no height at all;
            // `endParaRPr` gives the blank line the node's type size so
            // the following line lands where the canvas puts it.
            let _ = write!(
                out,
                "<a:endParaRPr lang=\"en-US\" sz=\"{}\"/>",
                font_hundredths_pt(font_size)
            );
        } else {
            out.push_str(&runs(node, text, start, end, font_size, alpha));
        }
        out.push_str("</a:p>");
    }
    out
}

fn paragraph_props(node: &SceneNode, font_size: f32) -> String {
    let mut inner = String::new();
    if node.line_height > 0.0 {
        let _ = write!(
            inner,
            "<a:lnSpc><a:spcPts val=\"{}\"/></a:lnSpc>",
            hundredths_pt(node.line_height * font_size)
        );
    }
    // Without `buNone` a bullet inherited from the layout's list styles
    // can appear in front of a line that was never a list item.
    inner.push_str("<a:buNone/>");
    format!(
        "<a:pPr marL=\"0\" indent=\"0\" algn=\"{}\">{inner}</a:pPr>",
        align(node.text_align)
    )
}

/// The runs covering `text[start..end]`.
///
/// Styled runs are byte ranges over the WHOLE string, so each line takes
/// the slice of them that overlaps it. A run that is reversed, that
/// overlaps its predecessor, or that lands mid-codepoint is skipped: the
/// characters still reach the slide with the node's own style, which is
/// a far smaller loss than dropping the text or panicking on a bad
/// slice.
fn runs(
    node: &SceneNode,
    text: &str,
    start: usize,
    end: usize,
    font_size: f32,
    alpha: f32,
) -> String {
    let node_style = RunStyle::from_node(node, font_size);
    if node.text_runs.is_empty() {
        return run(&text[start..end], &node_style, alpha);
    }
    let mut out = String::new();
    let mut cursor = start;
    for styled in &node.text_runs {
        let (run_start, run_end) = (styled.start.max(start), styled.end.min(end));
        if run_end <= run_start || run_start < cursor {
            continue;
        }
        if !text.is_char_boundary(run_start) || !text.is_char_boundary(run_end) {
            continue;
        }
        if run_start > cursor {
            out.push_str(&run(&text[cursor..run_start], &node_style, alpha));
        }
        out.push_str(&run(
            &text[run_start..run_end],
            &node_style.overlaid(styled),
            alpha,
        ));
        cursor = run_end;
    }
    if cursor < end {
        out.push_str(&run(&text[cursor..end], &node_style, alpha));
    }
    out
}

/// Everything a `<a:rPr>` needs, resolved — node level first, then the
/// per-run overrides laid over it.
struct RunStyle {
    size: f32,
    weight: u16,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    letter_spacing: f32,
    color: Color,
    family: String,
}

impl RunStyle {
    fn from_node(node: &SceneNode, font_size: f32) -> Self {
        Self {
            size: font_size,
            weight: node.font_weight,
            italic: node.italic,
            underline: node.underline,
            strikethrough: node.strikethrough,
            letter_spacing: node.letter_spacing,
            color: node.fill.unwrap_or(DEFAULT_FILL),
            family: primary_family(&node.font_family),
        }
    }

    /// The sentinels (`0.0` size, `0` weight, `None` fill) mean "inherit
    /// from the node", so only a set field overrides.
    fn overlaid(&self, run: &SceneTextRun) -> Self {
        Self {
            size: if run.font_size > 0.0 {
                run.font_size
            } else {
                self.size
            },
            weight: if run.font_weight > 0 {
                run.font_weight
            } else {
                self.weight
            },
            italic: self.italic || run.italic,
            underline: self.underline || run.underline,
            strikethrough: self.strikethrough || run.strikethrough,
            letter_spacing: self.letter_spacing,
            color: run.fill.unwrap_or(self.color),
            family: self.family.clone(),
        }
    }
}

fn run(text: &str, style: &RunStyle, alpha: f32) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut attrs = format!(" sz=\"{}\"", font_hundredths_pt(style.size));
    // PowerPoint has one bold bit, not a 100-900 axis. 600 is the
    // threshold the canvas backends already use for synthetic bold, so
    // a semibold heading reads heavy in both.
    if style.weight >= 600 {
        attrs.push_str(" b=\"1\"");
    }
    if style.italic {
        attrs.push_str(" i=\"1\"");
    }
    if style.underline {
        attrs.push_str(" u=\"sng\"");
    }
    if style.strikethrough {
        attrs.push_str(" strike=\"sngStrike\"");
    }
    if style.letter_spacing != 0.0 && style.letter_spacing.is_finite() {
        attrs.push_str(&format!(" spc=\"{}\"", hundredths_pt(style.letter_spacing)));
    }
    let family = escape_xml(&style.family);
    let typefaces = if style.family.is_empty() {
        String::new()
    } else {
        // `latin` covers Latin script, `ea` East Asian, `cs` complex
        // script. A CJK deck whose family is named only in `latin` gets
        // PowerPoint's own East Asian default instead of the authored
        // face, so all three are set to the same family.
        format!(
            "<a:latin typeface=\"{family}\"/><a:ea typeface=\"{family}\"/>\
<a:cs typeface=\"{family}\"/>"
        )
    };
    format!(
        "<a:r><a:rPr lang=\"en-US\"{attrs} dirty=\"0\">{}{typefaces}</a:rPr><a:t>{}</a:t></a:r>",
        solid_fill(style.color, alpha),
        escape_xml(&sanitize(text))
    )
}

/// The first family of the authored CSS stack.
///
/// The scene carries a stack (`"Noto Sans SC", sans-serif`); DrawingML
/// names exactly one face and resolves the rest itself, so the fallback
/// tail is dropped rather than jammed into the attribute.
fn primary_family(stack: &str) -> String {
    stack
        .split(',')
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches(['"', '\''])
        .trim()
        .to_string()
}

/// Drop the control characters XML 1.0 forbids.
///
/// Text arriving from an import can carry a stray `\r` or `\u{0}`; a
/// single one of those makes the whole part unparseable, which costs the
/// deck rather than the character.
fn sanitize(text: &str) -> String {
    text.chars()
        .filter(|c| *c == '\t' || !c.is_control())
        .collect()
}

fn align(align: SceneTextAlign) -> &'static str {
    match align {
        SceneTextAlign::Left => "l",
        SceneTextAlign::Center => "ctr",
        SceneTextAlign::Right => "r",
        SceneTextAlign::Justify => "just",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_editor_ui::layout_scene::NodeKind;
    use op_editor_ui::Point2D;

    fn text_node(content: &str) -> SceneNode {
        let mut n = SceneNode::leaf("t1", NodeKind::Text);
        n.text = Some(content.to_string());
        n.font_size = 32.0;
        n.font_family = "\"Noto Sans SC\", sans-serif".to_string();
        n.bounds = Rect {
            origin: Point2D::new(0.0, 0.0),
            size: Point2D::new(300.0, 48.0),
        };
        n
    }

    fn emitted(node: &SceneNode) -> String {
        let mut out = String::new();
        emit(&mut out, node, node.bounds, 1.0, 2);
        out
    }

    #[test]
    fn the_characters_land_as_text_not_as_a_picture() {
        let xml = emitted(&text_node("Quarterly Review"));
        assert!(xml.contains("<a:t>Quarterly Review</a:t>"), "{xml}");
        assert!(xml.contains("sz=\"2400\""), "32 px is 24 pt: {xml}");
    }

    #[test]
    fn only_the_first_family_of_the_css_stack_is_named() {
        let xml = emitted(&text_node("hi"));
        assert!(
            xml.contains("<a:latin typeface=\"Noto Sans SC\"/>"),
            "{xml}"
        );
        assert!(!xml.contains("sans-serif"), "{xml}");
    }

    #[test]
    fn each_authored_line_becomes_its_own_paragraph() {
        let xml = emitted(&text_node("one\n\nthree"));
        assert_eq!(xml.matches("<a:p>").count(), 3, "{xml}");
        // The blank middle line keeps its height.
        assert!(xml.contains("<a:endParaRPr"), "{xml}");
    }

    #[test]
    fn line_height_is_stated_as_an_exact_measure() {
        let mut n = text_node("hi");
        n.line_height = 1.5;
        // 1.5 x 32 px = 48 px = 36 pt.
        assert!(
            emitted(&n).contains("<a:spcPts val=\"3600\"/>"),
            "{}",
            emitted(&n)
        );
    }

    #[test]
    fn markup_in_the_authored_text_is_escaped() {
        let xml = emitted(&text_node("a < b & \"c\""));
        assert!(xml.contains("a &lt; b &amp; &quot;c&quot;"), "{xml}");
        assert!(!xml.contains("a < b"), "{xml}");
    }

    #[test]
    fn a_styled_run_overrides_only_what_it_sets() {
        let mut n = text_node("plain bold");
        n.font_weight = 400;
        n.text_runs = vec![SceneTextRun {
            start: 6,
            end: 10,
            font_size: 0.0,
            font_weight: 700,
            fill: None,
            italic: false,
            underline: false,
            strikethrough: false,
        }];
        let xml = emitted(&n);
        assert!(xml.contains("<a:t>plain </a:t>"), "{xml}");
        assert!(xml.contains("b=\"1\""), "{xml}");
        assert!(xml.contains("<a:t>bold</a:t>"), "{xml}");
        // Both runs keep the node's size — the run overrode weight only.
        assert_eq!(xml.matches("sz=\"2400\"").count(), 2, "{xml}");
    }
}
