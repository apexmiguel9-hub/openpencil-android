//! The geometry loop's `DesignForm` wiring (deck-system spec §4.5).
//!
//! No collector branches on the form yet, so what these guard is the wiring
//! itself: the loop reads the form from the single classifier, a projector
//! board is recognised as one, and routing through the form-aware entry point
//! leaves behaviour byte-identical.

use super::*;
use crate::test_support::VecDocSink;
use jian_ops_schema::node::PenNode;
use op_editor_core::PenNodeExt;
use serde_json::json;

/// A 1920×1080 board carrying a rigid child far wider than its row — the
/// shape the geometry loop provably repairs (it retargets the child to
/// `fill_container`), so the parity assertions below have a real fix to be
/// identical about.
fn board_with_an_overflowing_row() -> serde_json::Value {
    json!({
        "type":"frame","id":"board","name":"Slide 1","width":1920,"height":1080,
        "layout":"vertical","children":[
            {"type":"frame","id":"row","name":"Metric Row","layout":"horizontal","gap":16,
             "width":"fill_container","height":"fit_content","children":[
                {"type":"frame","id":"label","name":"Label","width":180,"height":120,"children":[]},
                {"type":"frame","id":"bar","name":"Chart Bar","width":2600,"height":120,"children":[]}
            ]}
        ]
    })
}

fn sink_with(root: serde_json::Value) -> (VecDocSink, String) {
    let root: PenNode = serde_json::from_value(root).expect("valid root");
    let mut sink = VecDocSink::new();
    sink.apply(EditorCommand::InsertSubtree {
        nodes: vec![root],
        parent_id: NodeId::NONE,
        page_id: None,
    });
    let root_id = sink.state().active_children()[0].id_str().to_string();
    (sink, root_id)
}

#[test]
fn the_loop_reads_a_projector_board_as_one() {
    let (sink, root_id) = sink_with(board_with_an_overflowing_row());
    assert_eq!(root_design_form(sink.state(), &root_id), DesignForm::Deck);
}

#[test]
fn a_root_that_is_gone_has_no_form_rather_than_a_default() {
    let (sink, _) = sink_with(board_with_an_overflowing_row());
    assert_eq!(
        root_design_form(sink.state(), "no-such-node"),
        DesignForm::Unknown
    );
}

#[test]
fn routing_through_the_form_aware_entry_point_changes_nothing() {
    let (mut classified, root_id) = sink_with(board_with_an_overflowing_row());
    let classified_rounds = geometry_validate_and_fix(&mut classified, &root_id);

    let (mut explicit, _) = sink_with(board_with_an_overflowing_row());
    let explicit_rounds =
        geometry_validate_and_fix_for_form(&mut explicit, &root_id, DesignForm::Deck);

    assert!(classified_rounds > 0, "the fixture must exercise a repair");
    assert_eq!(classified_rounds, explicit_rounds);
    assert_eq!(
        serde_json::to_value(classified.state().active_children()).unwrap(),
        serde_json::to_value(explicit.state().active_children()).unwrap(),
    );
}

#[test]
fn no_collector_branches_on_the_form_yet() {
    // The deck thresholds (spec §4.1) have not landed. Until they do, a board
    // and a page must come out of the loop identical — this test is expected
    // to CHANGE when the first deck collector lands, and it is the record of
    // when the behaviour split.
    let (mut as_deck, root_id) = sink_with(board_with_an_overflowing_row());
    let (mut as_page, _) = sink_with(board_with_an_overflowing_row());
    geometry_validate_and_fix_for_form(&mut as_deck, &root_id, DesignForm::Deck);
    geometry_validate_and_fix_for_form(&mut as_page, &root_id, DesignForm::Page);
    assert_eq!(
        serde_json::to_value(as_deck.state().active_children()).unwrap(),
        serde_json::to_value(as_page.state().active_children()).unwrap(),
    );
}
