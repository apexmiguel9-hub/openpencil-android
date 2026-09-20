//! `coalesce_subtask_section` / blank-forest classification tests.

use super::*;

#[test]
fn coalesce_folds_trailing_badge_leaf_into_lone_section() {
    // glm shape: a populated Top Bar section frame + a stray cart-count
    // badge "3" emitted as a SIBLING. No empty slot exists, the orphan is a
    // leaf → it appends into the section, not survive as a floating "3" band.
    let json = r#"[
        {"type":"frame","id":"s1","name":"Top Bar","width":"fill_container","height":"fit_content","layout":"horizontal","children":[
            {"type":"text","id":"loc","content":"Home"}
        ]},
        {"type":"text","id":"badge","content":"3"}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(nodes.len(), 1, "badge must fold into the lone section");
    let kids = nodes[0].children().expect("section children");
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0].id_str(), "loc");
    assert_eq!(
        kids[1].id_str(),
        "badge",
        "trailing badge nests as last child"
    );
}

#[test]
fn coalesce_prepends_leading_leaf_as_section_heading() {
    // A heading text emitted BEFORE a populated content frame folds in as the
    // FIRST child (preserving the intended heading-above-content order).
    let json = r#"[
        {"type":"text","id":"head","content":"Featured"},
        {"type":"frame","id":"sec","name":"List","children":[{"type":"text","id":"item","content":"x"}]}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(nodes.len(), 1);
    let kids = nodes[0].children().expect("section children");
    assert_eq!(
        kids[0].id_str(),
        "head",
        "leading leaf prepended as heading"
    );
    assert_eq!(kids[1].id_str(), "item");
}

#[test]
fn coalesce_fills_empty_wrapper_with_split_pieces() {
    // tt5 Promo shape: an EMPTY `Promo Banner` wrapper emitted first, with
    // its `Promo Content` (text) + `Promo Food Image` hung as sibling roots.
    // They must reparent INTO the empty banner (forest order kept) so the
    // banner stops rendering as a blank gap with floating invisible text.
    let json = r#"[
        {"type":"frame","id":"banner","name":"Promo Banner","layout":"vertical","children":[]},
        {"type":"frame","id":"content","name":"Promo Content","children":[{"type":"text","id":"title","content":"Get 30% off"}]},
        {"type":"image","id":"img","name":"Promo Food Image","src":""}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(nodes.len(), 1, "split pieces fold into the empty wrapper");
    assert_eq!(nodes[0].id_str(), "banner");
    let kids = nodes[0].children().expect("banner children");
    assert_eq!(kids.len(), 2);
    assert_eq!(kids[0].id_str(), "content");
    assert_eq!(kids[1].id_str(), "img");
}

#[test]
fn coalesce_fills_empty_direct_child_row_with_orphan_cards() {
    // tt5 Popular Dishes shape: section with a Header + an EMPTY `Dish Row`
    // direct child, and the dish cards hung as sibling roots. The cards must
    // land inside the empty row, not flatten into separate page bands.
    let json = r#"[
        {"type":"frame","id":"sec","name":"Popular Dishes","children":[
            {"type":"frame","id":"header","name":"Header","children":[{"type":"text","id":"t","content":"Popular Dishes"}]},
            {"type":"frame","id":"row","name":"Dish Row","children":[]}
        ]},
        {"type":"image","id":"pizza","name":"Margherita Pizza","src":""},
        {"type":"frame","id":"pinfo","name":"Info","children":[{"type":"text","id":"pn","content":"Margherita"}]}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(
        nodes.len(),
        1,
        "cards fold into the section, not the page root"
    );
    let sec_kids = nodes[0].children().expect("section children");
    assert_eq!(sec_kids.len(), 2, "Header + Dish Row preserved");
    let row = &sec_kids[1];
    assert_eq!(row.id_str(), "row");
    let row_kids = row.children().expect("dish row children");
    assert_eq!(
        row_kids.len(),
        2,
        "both orphan cards landed in the empty row"
    );
    assert_eq!(row_kids[0].id_str(), "pizza");
    assert_eq!(row_kids[1].id_str(), "pinfo");
}

#[test]
fn coalesce_leaves_populated_multi_section_forest_untouched() {
    // Two POPULATED section containers with no empty slot → a legitimate
    // multi-section forest; never collapse real sections into one another.
    let json = r#"[
        {"type":"frame","id":"a","name":"A","children":[{"type":"text","id":"ta","content":"A"}]},
        {"type":"frame","id":"b","name":"B","children":[{"type":"text","id":"tb","content":"B"}]}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(nodes.len(), 2, "two populated sections must be left as-is");
}

#[test]
fn ref_only_forest_is_not_rejected_as_blank() {
    // A subtask that reuses a component is a lone childless `ref`. It has no
    // children pre-resolution but expands to the master's subtree — so it
    // must NOT count as a blank-scaffolding forest (which would `fail()`).
    let json = r#"[{"type":"ref","id":"inst","ref":"comp-card","x":0,"y":0}]"#;
    let nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    assert!(
        has_content_node(&nodes[0]),
        "a ref is content (it expands to the master subtree)"
    );
    assert!(
        !is_blank_container_forest(&nodes),
        "a ref-only forest must survive the blank-container guard"
    );
}

#[test]
fn childless_frame_with_stroke_is_content_not_blank() {
    // The otp_input await-input slots: a childless Frame carrying an
    // explicit stroke renders exactly like a bare rectangle — same
    // pixels, different spelling. It must NOT count as blank scaffolding.
    let json = r##"[
        {"type":"frame","id":"root","name":"Root","children":[
            {"type":"frame","id":"box","name":"Await Input","children":[],
             "stroke":{"thickness":1,"fill":[{"type":"solid","color":"#E2E8F0"}]}}
        ]}
    ]"##;
    let nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    assert!(
        has_content_node(&nodes[0]),
        "a childless frame with a stroke paints real pixels"
    );
    assert!(
        !is_blank_container_forest(&nodes),
        "a stroked childless-frame forest must survive the blank-container guard"
    );
}

#[test]
fn childless_frame_with_fill_is_content_not_blank() {
    // Same shape as above but the paint comes from `fill` instead of
    // `stroke` — both count as explicit paint on an otherwise-empty
    // container.
    let json = r##"[
        {"type":"frame","id":"root","name":"Root","children":[
            {"type":"frame","id":"box","name":"Color Block","children":[],
             "fill":[{"type":"solid","color":"#111111"}]}
        ]}
    ]"##;
    let nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    assert!(
        has_content_node(&nodes[0]),
        "a childless frame with a fill paints real pixels"
    );
    assert!(
        !is_blank_container_forest(&nodes),
        "a filled childless-frame forest must survive the blank-container guard"
    );
}

#[test]
fn childless_frame_without_paint_is_blank() {
    // Same shape, no stroke/fill: genuinely empty scaffolding. This is
    // the case the blank-container guard exists to catch, so it must
    // still be rejected.
    let json = r#"[
        {"type":"frame","id":"root","name":"Root","children":[
            {"type":"frame","id":"box","name":"Empty","children":[]}
        ]}
    ]"#;
    let nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    assert!(
        !has_content_node(&nodes[0]),
        "a childless frame with no paint is not content"
    );
    assert!(
        is_blank_container_forest(&nodes),
        "an unpainted childless-frame forest must be rejected as blank"
    );
}

#[test]
fn coalesce_keeps_ref_orphan_instead_of_dropping_it() {
    // A populated section plus a sibling component instance (`ref`). The ref
    // is childless but is real content — it must fold into the section, not
    // be silently dropped as an "empty container" orphan.
    let json = r#"[
        {"type":"frame","id":"sec","name":"Section","children":[{"type":"text","id":"t","content":"Hi"}]},
        {"type":"ref","id":"inst","ref":"comp-card"}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    // The ref folds into the section (no empty slot, orphan is foldable) —
    // the key assertion is that it is NOT discarded.
    let surviving: Vec<&str> = collect_ids(&nodes);
    assert!(
        surviving.contains(&"inst"),
        "the ref instance must survive coalesce, got ids {surviving:?}"
    );
}

/// Depth-first id collection for the ref-survival assertion.
fn collect_ids(nodes: &[PenNode]) -> Vec<&str> {
    let mut out = Vec::new();
    fn walk<'a>(nodes: &'a [PenNode], out: &mut Vec<&'a str>) {
        for n in nodes {
            out.push(n.id_str());
            if let Some(kids) = n.children() {
                walk(kids, out);
            }
        }
    }
    walk(nodes, &mut out);
    out
}

#[test]
fn coalesce_folds_stray_icon_and_drops_empty_badge() {
    // tt5 header: the Bell icon (leaf) + an EMPTY Notification Badge frame
    // were emitted as top-level SIBLINGS of the header (bell floated below
    // the search; the empty badge would normalize into a blank full-width
    // band). The icon folds into the header; the empty badge is dropped —
    // neither survives as a floating page section.
    let json = r#"[
        {"type":"frame","id":"hdr","name":"Header & Search","children":[
            {"type":"frame","id":"loc","name":"Location & Actions","children":[{"type":"text","id":"l","content":"NYC"}]},
            {"type":"frame","id":"sb","name":"Search Bar","children":[{"type":"text_input","id":"si","placeholder":"Search"}]}
        ]},
        {"type":"icon_font","id":"bell","iconFontName":"bell"},
        {"type":"frame","id":"badge","name":"Notification Badge","children":[]}
    ]"#;
    let mut nodes: Vec<PenNode> = serde_json::from_str(json).expect("parse forest");
    coalesce_subtask_section(&mut nodes);
    assert_eq!(
        nodes.len(),
        1,
        "only the header section survives at top level"
    );
    let kids = nodes[0].children().expect("header children");
    assert!(
        kids.iter().any(|k| matches!(k, PenNode::IconFont(_))),
        "stray bell icon folded into the header"
    );
    let names: Vec<&str> = kids
        .iter()
        .filter_map(|k| k.base().name.as_deref())
        .collect();
    assert!(
        !names.contains(&"Notification Badge"),
        "empty badge dropped, not folded as a blank band"
    );
}
