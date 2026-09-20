//! Selection / preview / image-fill / icon-replacement mutator tests.

use crate::node_id::NodeId;
use crate::test_support::{frame, rect, sample, state_with};
use crate::walkers::find_node;
use jian_ops_schema::style::PenFill;

// --- Selection -------------------------------------------------------

#[test]
fn set_single_selection_replaces_set_and_anchor() {
    let mut s = sample();
    s.set_single_selection(NodeId::new("n10"));
    assert_eq!(s.selection.anchor, NodeId::new("n10"));
    assert_eq!(s.selection.set, vec![NodeId::new("n10")]);
}

// --- Preview (Play) mode — document invariance -----------------------

#[test]
fn enter_exit_preview_leaves_document_byte_identical() {
    // Phase D5: entering and exiting Preview must NOT mutate the saved
    // document. The runtime is built host-side from the serialized doc;
    // the editor state only flips the flag. Assert the canonical
    // serialization is identical across an enter → exit cycle.
    let mut s = state_with(vec![frame(
        "root",
        "Root",
        0.0,
        0.0,
        200.0,
        100.0,
        vec![rect("a", "A", 10.0, 10.0, 50.0, 50.0)],
    )]);
    let before = serde_json::to_string(&s.doc).expect("serialize before");

    s.editor_ui.enter_preview();
    assert!(s.editor_ui.preview.mode);
    s.editor_ui.exit_preview();
    assert!(!s.editor_ui.preview.mode);

    let after = serde_json::to_string(&s.doc).expect("serialize after");
    assert_eq!(before, after, "preview enter→exit must not touch doc");
}

#[test]
fn set_single_selection_none_clears() {
    let mut s = sample();
    s.set_single_selection(NodeId::NONE);
    assert!(s.selection.is_empty());
}

#[test]
fn toggle_selection_adds_then_removes() {
    let mut s = sample();
    s.clear_selection();
    s.toggle_selection(NodeId::new("n10"));
    s.toggle_selection(NodeId::new("n11"));
    assert_eq!(s.selection_count(), 2);
    assert_eq!(s.selection.anchor, NodeId::new("n11"));
    s.toggle_selection(NodeId::new("n11"));
    assert_eq!(s.selection_count(), 1);
    assert_eq!(s.selection.anchor, NodeId::new("n10"));
}

#[test]
fn select_all_top_level_picks_every_root() {
    let mut s = state_with(vec![
        rect("n1", "A", 0.0, 0.0, 10.0, 10.0),
        rect("n2", "B", 0.0, 0.0, 10.0, 10.0),
    ]);
    assert!(s.select_all_top_level());
    assert_eq!(s.selection_count(), 2);
    assert_eq!(s.selection.anchor, NodeId::new("n2"));
}

#[test]
fn select_all_top_level_empty_page_is_noop() {
    let mut s = state_with(vec![]);
    assert!(!s.select_all_top_level());
}

#[test]
fn set_selected_image_fill_mode_updates_primary_image_fill() {
    let mut node = rect("n60", "Photo fill", 0.0, 0.0, 100.0, 80.0);
    crate::fills::set_primary_fill_type(&mut node, crate::FillType::Image);
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("n60"));

    assert!(s.set_selected_image_fill_mode(crate::ImageFillMode::Crop));

    let node = find_node(s.active_children(), &NodeId::new("n60")).unwrap();
    match crate::fills::node_fills(node).unwrap().first().unwrap() {
        PenFill::Image(body) => {
            assert_eq!(body.mode, Some(jian_ops_schema::style::ImageFillMode::Crop));
        }
        other => panic!("expected image fill, got {other:?}"),
    }
}

#[test]
fn image_fill_summary_exposes_selected_image_url_for_preview() {
    let mut node = rect("n62", "Photo fill", 0.0, 0.0, 100.0, 80.0);
    crate::fills::set_primary_fill_type(&mut node, crate::FillType::Image);
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("n62"));

    let url = "data:image/png;base64,iVBORw0KGgo=";
    assert!(s.set_selected_fill_image_url(url));

    let node = find_node(s.active_children(), &NodeId::new("n62")).unwrap();
    let summary = crate::fills::first_image_fill_summary(node).unwrap();
    assert!(summary.has_image);
    assert_eq!(summary.image_url.as_deref(), Some(url));
}

#[test]
fn set_selected_image_adjustment_clamps_and_resets() {
    let mut node = rect("n61", "Photo fill", 0.0, 0.0, 100.0, 80.0);
    crate::fills::set_primary_fill_type(&mut node, crate::FillType::Image);
    let mut s = state_with(vec![node]);
    s.set_single_selection(NodeId::new("n61"));

    assert!(s.set_selected_image_adjustment(crate::ImageAdjustmentField::Exposure, 125.0));
    assert!(s.set_selected_image_adjustment(crate::ImageAdjustmentField::Contrast, -125.0));

    let node = find_node(s.active_children(), &NodeId::new("n61")).unwrap();
    match crate::fills::node_fills(node).unwrap().first().unwrap() {
        PenFill::Image(body) => {
            assert_eq!(body.exposure, Some(100.0));
            assert_eq!(body.contrast, Some(-100.0));
        }
        other => panic!("expected image fill, got {other:?}"),
    }

    assert!(s.reset_selected_image_adjustments());
    let node = find_node(s.active_children(), &NodeId::new("n61")).unwrap();
    match crate::fills::node_fills(node).unwrap().first().unwrap() {
        PenFill::Image(body) => {
            assert_eq!(body.exposure, Some(0.0));
            assert_eq!(body.contrast, Some(0.0));
            assert_eq!(body.saturation, Some(0.0));
            assert_eq!(body.temperature, Some(0.0));
            assert_eq!(body.tint, Some(0.0));
            assert_eq!(body.highlights, Some(0.0));
            assert_eq!(body.shadows, Some(0.0));
        }
        other => panic!("expected image fill, got {other:?}"),
    }
}

#[test]
fn replace_selected_icon_updates_icon_font_name_without_closing_over_color() {
    let mut s = sample();
    let id = s
        .insert_icon_font_node_at("search", "lucide", 50.0, 50.0)
        .expect("insert icon");

    assert!(s.replace_selected_icon("home", "lucide", None));

    let node = find_node(s.active_children(), &id).unwrap();
    let jian_ops_schema::node::PenNode::IconFont(icon) = node else {
        panic!("expected icon_font node");
    };
    assert_eq!(icon.icon_font_name, "home");
    assert_eq!(icon.icon_font_family.as_deref(), Some("lucide"));
    assert!(icon.fill.is_some(), "replacement preserves display color");
}

// --- Delete ----------------------------------------------------------
