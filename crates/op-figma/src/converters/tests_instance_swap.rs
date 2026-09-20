use super::tests::{fresh_ctx, obj, solid_paint};
use super::*;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::sizing::SizingKeyword;

fn guid(session_id: u32, local_id: u32) -> FigValue {
    obj(vec![
        ("sessionID", FigValue::Uint(session_id)),
        ("localID", FigValue::Uint(local_id)),
    ])
}

fn sized_rectangle(name: &str, session_id: u32, local_id: u32) -> TreeNode {
    TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("RECTANGLE".into())),
            ("guid", guid(session_id, local_id)),
            ("name", FigValue::Str(name.into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: Vec::new(),
    }
}

#[test]
fn component_swap_expands_override_target_instead_of_stale_base_children() {
    let target = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str("Resolved component".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: vec![sized_rectangle("Resolved artwork", 2, 21)],
    };
    let instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(3, 30)),
            ("name", FigValue::Str("Icon instance".into())),
            ("overriddenSymbolID", guid(2, 20)),
            (
                "symbolData",
                obj(vec![
                    ("symbolID", guid(1, 10)),
                    ("symbolOverrides", FigValue::Array(Vec::new())),
                ]),
            ),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        // Figma retains these children from the base component. They must not
        // win over the explicit overriddenSymbolID target.
        children: vec![sized_rectangle("Swap", 1, 11)],
    };
    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &target);

    let PenNode::Frame(frame) = convert_instance(&instance, None, &mut ctx) else {
        panic!("swapped instance should inline as a frame");
    };
    let names = frame
        .children
        .expect("resolved component children")
        .into_iter()
        .map(|node| match node {
            PenNode::Rectangle(rect) => rect.base.name.unwrap_or_default(),
            other => panic!("expected rectangle, got {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["Resolved artwork"]);
}

#[test]
fn root_symbol_override_controls_instance_layout_without_replacing_its_size() {
    let symbol = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(96_899, 1_296)),
            ("name", FigValue::Str("Menu".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(88.0)),
                    ("y", FigValue::Float(40.0)),
                ]),
            ),
            ("stackMode", FigValue::Str("VERTICAL".into())),
            ("stackCounterSizing", FigValue::Str("RESIZE_TO_FIT".into())),
        ]),
        children: vec![sized_rectangle("Menu item", 96_899, 1_297)],
    };
    let root_override = obj(vec![
        (
            "guidPath",
            obj(vec![("guids", FigValue::Array(vec![guid(96_899, 1_296)]))]),
        ),
        ("stackCounterSizing", FigValue::Str("FIXED".into())),
        // Root overrides may carry geometry, but the synthetic frame must
        // retain the INSTANCE's authored dimensions.
        (
            "size",
            obj(vec![
                ("x", FigValue::Float(999.0)),
                ("y", FigValue::Float(999.0)),
            ]),
        ),
    ]);
    let instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("name", FigValue::Str("Menu".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(160.0)),
                    ("y", FigValue::Float(40.0)),
                ]),
            ),
            (
                "symbolData",
                obj(vec![
                    ("symbolID", guid(96_899, 1_296)),
                    ("symbolOverrides", FigValue::Array(vec![root_override])),
                ]),
            ),
        ]),
        children: Vec::new(),
    };

    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("96899:1296".into(), &symbol);
    let PenNode::Frame(frame) = convert_instance(&instance, None, &mut ctx) else {
        panic!("instance should materialise as a frame");
    };
    assert_eq!(frame.container.width, Some(SizingBehavior::Number(160.0)));
    assert_eq!(frame.container.height, Some(SizingBehavior::Number(40.0)));

    // A field authored directly on the INSTANCE remains the highest-priority
    // source, even when the root override carries a different value.
    let mut direct_instance = instance.clone();
    direct_instance
        .figma
        .set("stackCounterSizing", FigValue::Str("RESIZE_TO_FIT".into()));
    let mut direct_ctx = fresh_ctx();
    direct_ctx.symbol_tree.insert("96899:1296".into(), &symbol);
    let PenNode::Frame(direct_frame) = convert_instance(&direct_instance, None, &mut direct_ctx)
    else {
        panic!("instance should materialise as a frame");
    };
    assert_eq!(
        direct_frame.container.width,
        Some(SizingBehavior::Keyword(SizingKeyword::FitContent))
    );
}

#[test]
fn empty_boolean_instance_paints_artwork_without_filling_its_wrapper() {
    let mut artwork = sized_rectangle("Icon artwork", 2, 21);
    artwork.figma.set(
        "fillPaints",
        FigValue::Array(vec![solid_paint(1.0, 0.0, 0.0)]),
    );
    let symbol = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str("Setting".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
        ]),
        children: vec![artwork],
    };
    let instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("name", FigValue::Str("Setting".into())),
            ("symbolData", obj(vec![("symbolID", guid(2, 20))])),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
        ]),
        children: Vec::new(),
    };
    let mut result_paint = solid_paint(0.0, 0.0, 0.0);
    result_paint.set("opacity", FigValue::Float(0.45));
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Union".into())),
            ("booleanOperation", FigValue::Str("UNION".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(12.5)),
                    ("y", FigValue::Float(12.5)),
                ]),
            ),
            ("fillPaints", FigValue::Array(vec![result_paint])),
        ]),
        children: vec![instance],
    };
    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &symbol);

    let PenNode::Group(group) =
        convert_empty_boolean_group(&boolean, None, "boolean".into(), &mut ctx)
            .expect("empty boolean should retain its instance artwork")
    else {
        panic!("empty boolean should convert to a group");
    };
    let PenNode::Frame(wrapper) = &group.children.as_deref().unwrap()[0] else {
        panic!("component operand should expand to a frame");
    };
    assert!(
        wrapper.container.fill.is_none(),
        "transparent instance wrapper must not become a solid icon bounding-box"
    );
    let PenNode::Rectangle(artwork) = &wrapper.children.as_deref().unwrap()[0] else {
        panic!("component artwork should remain a rectangle");
    };
    let jian_ops_schema::style::PenFill::Solid(fill) =
        &artwork.container.fill.as_deref().unwrap()[0]
    else {
        panic!("boolean result should remain a solid fill");
    };
    assert_eq!(fill.color, "#000000");
    assert_eq!(fill.opacity, Some(0.45));
}

#[test]
fn swapped_icon_cached_fill_does_not_paint_transparent_symbol_wrapper() {
    let mut artwork = sized_rectangle("shape", 2, 21);
    artwork.figma.set(
        "fillPaints",
        FigValue::Array(vec![solid_paint(0.0, 0.0, 0.0)]),
    );
    let mut invisible_symbol_fill = solid_paint(1.0, 1.0, 1.0);
    invisible_symbol_fill.set("visible", FigValue::Bool(false));
    let target = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str(".3E4AV".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
            ("fillPaints", FigValue::Array(vec![invisible_symbol_fill])),
        ]),
        children: vec![artwork],
    };
    let swapped_instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(3, 30)),
            ("name", FigValue::Str("Setting".into())),
            ("overriddenSymbolID", guid(2, 20)),
            ("symbolData", obj(vec![("symbolID", guid(1, 10))])),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
            // Kiwi keeps this resolved cache on the swapped INSTANCE even
            // though the target SYMBOL root is explicitly unpainted.
            (
                "fillPaints",
                FigValue::Array(vec![solid_paint(1.0, 1.0, 1.0)]),
            ),
        ]),
        children: Vec::new(),
    };
    let mut result_paint = solid_paint(0.0, 0.0, 0.0);
    result_paint.set("opacity", FigValue::Float(0.45));
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Union".into())),
            ("booleanOperation", FigValue::Str("UNION".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(12.5)),
                    ("y", FigValue::Float(12.5)),
                ]),
            ),
            ("fillPaints", FigValue::Array(vec![result_paint])),
        ]),
        children: vec![swapped_instance],
    };
    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &target);

    let PenNode::Group(group) =
        convert_swapped_boolean_group(&boolean, None, "boolean".into(), &mut ctx)
            .expect("swapped boolean should rebuild from its target artwork")
    else {
        panic!("swapped boolean should convert to a group");
    };
    let PenNode::Frame(wrapper) = &group.children.as_deref().unwrap()[0] else {
        panic!("swapped component should expand to a frame");
    };
    assert!(
        wrapper.container.fill.is_none(),
        "cached INSTANCE fill must not paint an explicitly transparent SYMBOL wrapper"
    );
    let PenNode::Rectangle(artwork) = &wrapper.children.as_deref().unwrap()[0] else {
        panic!("target artwork should remain a rectangle");
    };
    let jian_ops_schema::style::PenFill::Solid(fill) =
        &artwork.container.fill.as_deref().unwrap()[0]
    else {
        panic!("target artwork should receive the boolean result paint");
    };
    assert_eq!(fill.color, "#000000");
    assert_eq!(fill.opacity, Some(0.45));
}

#[test]
fn swapped_icon_without_root_paint_does_not_paint_transparent_symbol_wrapper() {
    let mut artwork = sized_rectangle("Vector", 2, 21);
    artwork.figma.set(
        "fillPaints",
        FigValue::Array(vec![solid_paint(0.0, 0.0, 0.0)]),
    );
    let target = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str("Form".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
        ]),
        children: vec![artwork],
    };
    let swapped_instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(3, 30)),
            ("name", FigValue::Str("Form".into())),
            ("overriddenSymbolID", guid(2, 20)),
            ("symbolData", obj(vec![("symbolID", guid(1, 10))])),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
            // Figma can retain a resolved fill cache on an otherwise
            // transparent icon instance even when the SYMBOL root has no
            // paint fields at all.
            (
                "fillPaints",
                FigValue::Array(vec![solid_paint(0.0, 0.0, 0.0)]),
            ),
        ]),
        children: Vec::new(),
    };
    let mut result_paint = solid_paint(0.0, 0.0, 0.0);
    result_paint.set("opacity", FigValue::Float(0.85));
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Union".into())),
            ("booleanOperation", FigValue::Str("UNION".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(14.0)),
                    ("y", FigValue::Float(14.0)),
                ]),
            ),
            ("fillPaints", FigValue::Array(vec![result_paint])),
        ]),
        children: vec![swapped_instance],
    };
    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &target);

    let PenNode::Group(group) =
        convert_swapped_boolean_group(&boolean, None, "boolean".into(), &mut ctx)
            .expect("swapped boolean should rebuild from target artwork")
    else {
        panic!("swapped boolean should convert to a group");
    };
    let PenNode::Frame(wrapper) = &group.children.as_deref().unwrap()[0] else {
        panic!("swapped component should expand to a frame");
    };
    assert!(
        wrapper.container.fill.is_none(),
        "an unpainted SYMBOL root must remain a transparent sizing wrapper"
    );
    let PenNode::Rectangle(artwork) = &wrapper.children.as_deref().unwrap()[0] else {
        panic!("target artwork should remain a rectangle");
    };
    let jian_ops_schema::style::PenFill::Solid(fill) =
        &artwork.container.fill.as_deref().unwrap()[0]
    else {
        panic!("target artwork should receive the boolean result fill");
    };
    assert_eq!(fill.color, "#000000");
    assert_eq!(fill.opacity, Some(0.85));
}

#[test]
fn swapped_component_visible_symbol_background_remains_artwork() {
    let mut artwork = sized_rectangle("foreground", 2, 21);
    artwork.figma.set(
        "fillPaints",
        FigValue::Array(vec![solid_paint(1.0, 1.0, 1.0)]),
    );
    let target = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str("Painted card".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
            (
                "fillPaints",
                FigValue::Array(vec![solid_paint(1.0, 0.0, 0.0)]),
            ),
        ]),
        children: vec![artwork],
    };
    let swapped_instance = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(3, 30)),
            ("name", FigValue::Str("Painted card instance".into())),
            ("overriddenSymbolID", guid(2, 20)),
            ("symbolData", obj(vec![("symbolID", guid(1, 10))])),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: Vec::new(),
    };
    let mut result_paint = solid_paint(0.0, 0.0, 0.0);
    result_paint.set("opacity", FigValue::Float(0.45));
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Union".into())),
            ("booleanOperation", FigValue::Str("UNION".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
            ("fillPaints", FigValue::Array(vec![result_paint])),
        ]),
        children: vec![swapped_instance],
    };
    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &target);

    let PenNode::Group(group) =
        convert_swapped_boolean_group(&boolean, None, "boolean".into(), &mut ctx)
            .expect("swapped boolean should rebuild its painted component")
    else {
        panic!("swapped boolean should convert to a group");
    };
    let PenNode::Frame(component) = &group.children.as_deref().unwrap()[0] else {
        panic!("swapped component should expand to a frame");
    };
    let jian_ops_schema::style::PenFill::Solid(fill) =
        &component.container.fill.as_deref().unwrap()[0]
    else {
        panic!("visible component background should remain in the silhouette");
    };
    assert_eq!(fill.color, "#000000");
    assert_eq!(fill.opacity, Some(0.45));
}

#[test]
fn baked_boolean_geometry_yields_to_swapped_component_operand() {
    fn collect_names(node: &PenNode, names: &mut Vec<String>) {
        match node {
            PenNode::Frame(node) => {
                names.push(node.base.name.clone().unwrap_or_default());
                for child in node.children.as_deref().unwrap_or_default() {
                    collect_names(child, names);
                }
            }
            PenNode::Group(node) => {
                names.push(node.base.name.clone().unwrap_or_default());
                for child in node.children.as_deref().unwrap_or_default() {
                    collect_names(child, names);
                }
            }
            PenNode::Rectangle(node) => {
                names.push(node.base.name.clone().unwrap_or_default());
            }
            _ => {}
        }
    }
    fn find_rectangle<'a>(
        node: &'a PenNode,
        name: &str,
    ) -> Option<&'a jian_ops_schema::node::RectangleNode> {
        match node {
            PenNode::Rectangle(rectangle) if rectangle.base.name.as_deref() == Some(name) => {
                Some(rectangle)
            }
            PenNode::Frame(frame) => frame
                .children
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find_map(|child| find_rectangle(child, name)),
            PenNode::Group(group) => group
                .children
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find_map(|child| find_rectangle(child, name)),
            PenNode::Rectangle(rectangle) => rectangle
                .children
                .as_deref()
                .unwrap_or_default()
                .iter()
                .find_map(|child| find_rectangle(child, name)),
            _ => None,
        }
    }

    let mut painted_artwork = sized_rectangle("Target artwork", 2, 21);
    painted_artwork.figma.set(
        "fillPaints",
        FigValue::Array(vec![solid_paint(1.0, 0.0, 0.0)]),
    );
    let mut stroked_artwork = sized_rectangle("Stroked target artwork", 2, 23);
    stroked_artwork.figma.set(
        "strokePaints",
        FigValue::Array(vec![solid_paint(1.0, 0.0, 0.0)]),
    );

    let target = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("SYMBOL".into())),
            ("guid", guid(2, 20)),
            ("name", FigValue::Str("Target component".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: vec![
            painted_artwork,
            stroked_artwork,
            sized_rectangle("Transparent hitbox", 2, 22),
        ],
    };
    let swapped_operand = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("INSTANCE".into())),
            ("guid", guid(3, 30)),
            ("name", FigValue::Str("Swapped operand".into())),
            ("overriddenSymbolID", guid(2, 20)),
            ("symbolData", obj(vec![("symbolID", guid(1, 10))])),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: vec![sized_rectangle("Stale default artwork", 1, 11)],
    };
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Union".into())),
            ("booleanOperation", FigValue::Str("UNION".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
            (
                "fillGeometry",
                FigValue::Array(vec![obj(vec![("commandsBlob", FigValue::Uint(0))])]),
            ),
            (
                "fillPaints",
                FigValue::Array(vec![solid_paint(0.0, 0.5, 1.0)]),
            ),
        ]),
        children: vec![swapped_operand],
    };
    let mut blob = vec![0x01];
    blob.extend_from_slice(&0.0f32.to_le_bytes());
    blob.extend_from_slice(&0.0f32.to_le_bytes());
    blob.push(0x02);
    blob.extend_from_slice(&10.0f32.to_le_bytes());
    blob.extend_from_slice(&0.0f32.to_le_bytes());

    let mut ctx = fresh_ctx();
    ctx.symbol_tree.insert("2:20".into(), &target);
    ctx.blobs = vec![crate::figma_types::BlobOrString::Bytes(blob)];
    let resolved = convert_vector(&boolean, None, &mut ctx);
    assert!(matches!(resolved, PenNode::Group(_)));
    let mut names = Vec::new();
    collect_names(&resolved, &mut names);
    assert!(names.iter().any(|name| name == "Target artwork"));
    assert!(!names.iter().any(|name| name == "Stale default artwork"));
    assert!(
        find_rectangle(&resolved, "Target artwork")
            .expect("painted target")
            .container
            .fill
            .is_some(),
        "authored artwork fill should inherit the boolean result paint"
    );
    assert!(
        find_rectangle(&resolved, "Transparent hitbox")
            .expect("transparent hitbox")
            .container
            .fill
            .is_none(),
        "transparent target layers must not become solid background boxes"
    );
    let stroke_fills = find_rectangle(&resolved, "Stroked target artwork")
        .expect("stroke-only target")
        .container
        .stroke
        .as_ref()
        .and_then(|stroke| stroke.fill.as_ref())
        .expect("fill-only boolean paint should recolour stroke geometry");
    let result_fills = find_rectangle(&resolved, "Target artwork")
        .expect("fill target")
        .container
        .fill
        .as_ref()
        .expect("boolean result fill");
    assert_eq!(stroke_fills, result_fills);
}

#[test]
fn multi_operand_non_union_keeps_cached_boolean_geometry() {
    let mut swapped = sized_rectangle("swapped", 3, 30);
    swapped.figma.set("type", FigValue::Str("INSTANCE".into()));
    swapped
        .figma
        .set("symbolData", obj(vec![("symbolID", guid(1, 10))]));
    swapped.figma.set("overriddenSymbolID", guid(2, 20));
    let boolean = TreeNode {
        figma: obj(vec![
            ("type", FigValue::Str("BOOLEAN_OPERATION".into())),
            ("name", FigValue::Str("Subtract".into())),
            ("booleanOperation", FigValue::Str("SUBTRACT".into())),
            (
                "size",
                obj(vec![
                    ("x", FigValue::Float(24.0)),
                    ("y", FigValue::Float(24.0)),
                ]),
            ),
        ]),
        children: vec![swapped, sized_rectangle("base", 4, 40)],
    };
    let mut ctx = fresh_ctx();
    assert!(convert_swapped_boolean_group(&boolean, None, "bool".into(), &mut ctx).is_none());
    assert!(ctx
        .warnings
        .iter()
        .any(|warning| warning.contains("kept cached geometry")));
}
