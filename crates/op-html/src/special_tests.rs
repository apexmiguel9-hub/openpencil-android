use jian_ops_schema::node::base::NumberOrExpression;
use jian_ops_schema::node::container::CornerRadius;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingBehavior;
use jian_ops_schema::style::{BlendMode, PenFill};

fn element_with(tag: &str, attrs: Vec<(&str, &str)>) -> crate::dom::DomElement {
    crate::dom::DomElement {
        tag: tag.into(),
        attrs: attrs
            .into_iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect(),
        children: Vec::new(),
    }
}

fn map_it(element: crate::dom::DomElement) -> Option<PenNode> {
    let (rules, _) = crate::css::cascade::parse_stylesheet("", 0);
    let options = crate::HtmlImportOptions::default();
    let mut context = crate::mapper::MapCtx {
        opts: &options,
        rules: &rules,
        warnings: Vec::new(),
        warned: Default::default(),
        next_id: 0,
        node_count: 0,
        containing_width: options.viewport_width,
        containing_height: options.viewport_height(),
        containing_width_is_definite: true,
        positioned_width: options.viewport_width,
        positioned_height: options.viewport_height(),
        auto_margin_handled_by_parent: false,
        pending_base_outcome: Default::default(),
    };
    crate::mapper::map_element(&mut context, &[&element], None)
}

#[test]
fn img_maps_to_image_node() {
    let node = map_it(element_with(
        "img",
        vec![
            ("src", "https://x.dev/a.png"),
            ("width", "120"),
            ("height", "80"),
        ],
    ));
    let Some(PenNode::Image(image)) = node else {
        panic!("expected image")
    };
    assert_eq!(image.src.as_str(), "https://x.dev/a.png");
    assert!(matches!(
        image.width,
        Some(SizingBehavior::Number(value)) if value == 120.0
    ));
}

#[test]
fn special_nodes_receive_computed_base_and_visual_styles() {
    let node = map_it(element_with(
        "img",
        vec![
            ("src", "hero.png"),
            (
                "style",
                "position:absolute;left:12px;top:8px;opacity:.4;\
                 transform:rotate(15deg);width:50px;height:40px;\
                 border-radius:6px;filter:blur(2px)",
            ),
        ],
    ));
    let Some(PenNode::Image(image)) = node else {
        panic!("expected image")
    };
    assert_eq!(image.base.x, Some(12.0));
    assert_eq!(image.base.y, Some(8.0));
    assert_eq!(image.base.rotation, Some(15.0));
    assert!(matches!(
        image.base.opacity,
        Some(NumberOrExpression::Number(value)) if value == 0.4
    ));
    assert!(matches!(
        image.width,
        Some(SizingBehavior::Number(value)) if value == 50.0
    ));
    assert!(matches!(
        image.height,
        Some(SizingBehavior::Number(value)) if value == 40.0
    ));
    assert!(matches!(
        image.corner_radius,
        Some(CornerRadius::Uniform(value)) if value == 6.0
    ));
    assert!(image
        .effects
        .as_ref()
        .is_some_and(|effects| !effects.is_empty()));
}

#[test]
fn horizontal_rule_and_radio_keep_css_visuals() {
    let Some(PenNode::Rectangle(rule)) = map_it(element_with(
        "hr",
        vec![(
            "style",
            "width:80px;height:3px;background:#123456;border-radius:2px;opacity:.5",
        )],
    )) else {
        panic!("expected rectangle")
    };
    assert!(matches!(
        rule.container.width,
        Some(SizingBehavior::Number(value)) if value == 80.0
    ));
    assert!(matches!(
        rule.container.height,
        Some(SizingBehavior::Number(value)) if value == 3.0
    ));
    assert!(matches!(
        rule.container.fill.as_deref(),
        Some([PenFill::Solid(fill)]) if fill.color == "#123456"
    ));
    assert!(matches!(
        rule.base.opacity,
        Some(NumberOrExpression::Number(value)) if value == 0.5
    ));

    let Some(PenNode::RadioGroup(radio)) = map_it(element_with(
        "input",
        vec![
            ("type", "radio"),
            (
                "style",
                "width:24px;height:24px;background:#abcdef;border-radius:12px",
            ),
        ],
    )) else {
        panic!("expected radio group")
    };
    assert!(matches!(
        radio.fill.as_deref(),
        Some([PenFill::Solid(fill)]) if fill.color == "#abcdef"
    ));
    assert!(matches!(
        radio.corner_radius,
        Some(CornerRadius::Uniform(value)) if value == 12.0
    ));
}

#[test]
fn inline_svg_becomes_data_url_image() {
    let mut svg = element_with("svg", vec![("viewBox", "0 0 10 10")]);
    svg.children.push(crate::dom::DomNode::Element(element_with(
        "rect",
        vec![("width", "10")],
    )));
    let Some(PenNode::Image(image)) = map_it(svg) else {
        panic!()
    };
    assert!(image.src.as_str().starts_with("data:image/svg+xml;base64,"));
}

#[test]
fn form_controls_map_to_widget_nodes() {
    assert!(matches!(
        map_it(element_with(
            "input",
            vec![("type", "text"), ("placeholder", "Name")]
        )),
        Some(PenNode::TextInput(input)) if input.placeholder.as_deref() == Some("Name")
    ));
    assert!(matches!(
        map_it(element_with(
            "input",
            vec![("type", "checkbox"), ("checked", "")]
        )),
        Some(PenNode::Checkbox(_))
    ));
    assert!(matches!(
        map_it(element_with(
            "input",
            vec![("type", "range"), ("min", "0"), ("max", "10")]
        )),
        Some(PenNode::Slider(slider)) if slider.max == Some(10.0)
    ));
    assert!(matches!(
        map_it(element_with(
            "progress",
            vec![("value", "3"), ("max", "10")]
        )),
        Some(PenNode::Progress(_))
    ));
}

#[test]
fn select_collects_options() {
    let mut select = element_with("select", vec![]);
    let mut option = element_with("option", vec![("value", "a"), ("selected", "")]);
    option
        .children
        .push(crate::dom::DomNode::Text("Alpha".into()));
    select.children.push(crate::dom::DomNode::Element(option));
    let Some(PenNode::Select(select)) = map_it(select) else {
        panic!()
    };
    assert_eq!(select.value.as_deref(), Some("a"));
    let options = select.options.as_ref().unwrap();
    assert_eq!(options[0].label, "Alpha");
}

#[test]
fn button_is_frame_with_role() {
    let mut button = element_with("button", vec![]);
    button.children.push(crate::dom::DomNode::Text("Go".into()));
    let Some(PenNode::Frame(frame)) = map_it(button) else {
        panic!()
    };
    assert_eq!(frame.base.role.as_deref(), Some("button"));
}

#[test]
fn image_blending_is_preserved_while_unrepresentable_position_is_reported() {
    let result = crate::import_html(
        "<img src='hero.png' style='object-fit:cover;object-position:65% bottom;\
                                    mix-blend-mode:multiply'>",
        &crate::HtmlImportOptions::default(),
    );
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("object-position")));
    assert!(result
        .warnings
        .iter()
        .all(|warning| !warning.contains("mix-blend-mode")));
    let node = map_it(element_with(
        "img",
        vec![("src", "hero.png"), ("style", "mix-blend-mode:multiply")],
    ));
    let Some(PenNode::Image(image)) = node else {
        panic!("expected image node")
    };
    assert_eq!(image.base.blend_mode, Some(BlendMode::Multiply));
}
