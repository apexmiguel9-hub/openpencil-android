//! Figma `CANVAS` metadata mapped onto the canonical page model.

use crate::color::figma_color_to_hex;
use crate::figma_types::FigColor;
use crate::tree::TreeNode;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::page::PenPage;

pub(crate) fn pen_page(
    figma_page: &TreeNode,
    id: String,
    name: String,
    children: Vec<PenNode>,
) -> PenPage {
    PenPage {
        id,
        name,
        children,
        background_color: figma_page_background(figma_page),
        state: None,
        lifecycle: None,
    }
}

fn figma_page_background(page: &TreeNode) -> Option<String> {
    if page.figma.get_bool("backgroundEnabled") == Some(false) {
        return None;
    }
    let mut color = page
        .figma
        .get("backgroundColor")
        .and_then(FigColor::from_value)?;
    let opacity = page
        .figma
        .get_f64("backgroundOpacity")
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    color.a = Some(color.a.unwrap_or(1.0).clamp(0.0, 1.0) * opacity);
    Some(figma_color_to_hex(&color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiwi::FigValue;

    fn obj(pairs: Vec<(&str, FigValue)>) -> FigValue {
        FigValue::Object(pairs.into_iter().map(|(k, v)| (k.into(), v)).collect())
    }

    fn canvas(background_enabled: bool, opacity: f32) -> TreeNode {
        TreeNode {
            figma: obj(vec![
                ("type", FigValue::Str("CANVAS".into())),
                ("backgroundEnabled", FigValue::Bool(background_enabled)),
                ("backgroundOpacity", FigValue::Float(opacity)),
                (
                    "backgroundColor",
                    obj(vec![
                        ("r", FigValue::Float(215.0 / 255.0)),
                        ("g", FigValue::Float(228.0 / 255.0)),
                        ("b", FigValue::Float(243.0 / 255.0)),
                        ("a", FigValue::Float(1.0)),
                    ]),
                ),
            ]),
            children: Vec::new(),
        }
    }

    #[test]
    fn imports_canvas_color_and_multiplies_background_opacity() {
        let page = pen_page(&canvas(true, 0.5), "p1".into(), "Page 1".into(), Vec::new());
        assert_eq!(page.background_color.as_deref(), Some("#d7e4f380"));
    }

    #[test]
    fn disabled_canvas_background_is_not_authored() {
        let page = pen_page(
            &canvas(false, 1.0),
            "p1".into(),
            "Page 1".into(),
            Vec::new(),
        );
        assert_eq!(page.background_color, None);
    }
}
