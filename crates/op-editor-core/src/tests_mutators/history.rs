//! History / node-flag / fill-type / colour-variable mutator tests.

use super::support::three_rects;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{rect, sample, state_with};
use crate::walkers::find_node;
use jian_ops_schema::variable::{VariableKind, VariableScalar};

#[test]
fn undo_redo_round_trips_a_translate() {
    let mut s = state_with(vec![rect("n1", "A", 10.0, 10.0, 50.0, 50.0)]);
    s.set_single_selection(NodeId::new("n1"));
    s.commit_history();
    s.translate_selected(20.0, 5.0);
    let moved = find_node(s.active_children(), &NodeId::new("n1"))
        .unwrap()
        .base()
        .x
        .unwrap();
    assert_eq!(moved, 30.0);
    assert!(s.undo());
    assert_eq!(
        find_node(s.active_children(), &NodeId::new("n1"))
            .unwrap()
            .base()
            .x
            .unwrap(),
        10.0
    );
    assert!(s.redo());
    assert_eq!(
        find_node(s.active_children(), &NodeId::new("n1"))
            .unwrap()
            .base()
            .x
            .unwrap(),
        30.0
    );
}

#[test]
fn undo_on_empty_history_is_false() {
    let mut s = sample();
    assert!(!s.undo());
    assert!(!s.redo());
}

#[test]
fn history_caps_at_100_entries() {
    let mut s = sample();
    for _ in 0..150 {
        s.commit_history();
    }
    assert_eq!(s.history.past.len(), 100);
}

#[test]
fn commit_history_clears_redo_stack() {
    let mut s = sample();
    s.commit_history();
    assert!(s.undo());
    assert!(s.history.can_redo());
    s.commit_history();
    assert!(!s.history.can_redo());
}

// --- Flag toggles ----------------------------------------------------

#[test]
fn toggle_node_hidden_flips_visible() {
    let mut s = three_rects();
    assert!(s.toggle_node_hidden(&NodeId::new("n1")));
    let n = find_node(s.active_children(), &NodeId::new("n1")).unwrap();
    assert_eq!(n.base().visible, Some(false));
    assert!(s.toggle_node_hidden(&NodeId::new("n1")));
    let n = find_node(s.active_children(), &NodeId::new("n1")).unwrap();
    assert_eq!(n.base().visible, Some(true));
}

#[test]
fn toggle_node_locked_flips_locked() {
    let mut s = three_rects();
    assert!(s.toggle_node_locked(&NodeId::new("n2")));
    let n = find_node(s.active_children(), &NodeId::new("n2")).unwrap();
    assert_eq!(n.base().locked, Some(true));
}

// --- Fill type (Gap 1) ----------------------------------------------

#[test]
fn set_selected_fill_type_writes_first_fill_variant() {
    let mut s = three_rects();
    s.set_single_selection(NodeId::new("n1"));
    // Default rect has no fills → reports Solid.
    assert_eq!(
        crate::first_fill_type(find_node(s.active_children(), &NodeId::new("n1")).unwrap()),
        crate::FillType::Solid
    );
    assert!(s.set_selected_fill_type(crate::FillType::LinearGradient));
    assert_eq!(
        crate::first_fill_type(find_node(s.active_children(), &NodeId::new("n1")).unwrap()),
        crate::FillType::LinearGradient
    );
    // Flipping again to Image lands too.
    assert!(s.set_selected_fill_type(crate::FillType::Image));
    assert_eq!(
        crate::first_fill_type(find_node(s.active_children(), &NodeId::new("n1")).unwrap()),
        crate::FillType::Image
    );
}

#[test]
fn set_selected_fill_type_no_selection_is_noop() {
    let mut s = three_rects();
    s.clear_selection();
    assert!(!s.set_selected_fill_type(crate::FillType::RadialGradient));
}

#[test]
fn set_selected_fill_type_rejects_locked_node() {
    let mut s = three_rects();
    s.toggle_node_locked(&NodeId::new("n1"));
    s.set_single_selection(NodeId::new("n1"));
    assert!(!s.set_selected_fill_type(crate::FillType::LinearGradient));
}

#[test]
fn bind_selected_color_variable_writes_fill_and_stroke_refs() {
    let mut s = three_rects();
    assert!(s.create_variable(
        "color-1",
        VariableKind::Color,
        VariableScalar::Str("#000000".into()),
    ));
    s.set_single_selection(NodeId::new("n1"));

    assert!(s.bind_selected_color_variable(crate::ColorTarget::Fill, "color-1"));
    assert_eq!(
        crate::fills::first_solid_fill_hex(
            find_node(s.active_children(), &NodeId::new("n1")).unwrap()
        ),
        Some("$color-1")
    );
    assert_eq!(
        s.ui.variables
            .fill_refs
            .get(&NodeId::new("n1"))
            .map(String::as_str),
        Some("color-1")
    );

    assert!(s.bind_selected_color_variable(crate::ColorTarget::Stroke, "color-1"));
    assert_eq!(
        crate::fills::first_solid_stroke_hex(
            find_node(s.active_children(), &NodeId::new("n1")).unwrap()
        ),
        Some("$color-1")
    );
    assert_eq!(
        s.ui.variables
            .stroke_refs
            .get(&NodeId::new("n1"))
            .map(String::as_str),
        Some("color-1")
    );
}

// --- Chat model (Gap 2) ---------------------------------------------
