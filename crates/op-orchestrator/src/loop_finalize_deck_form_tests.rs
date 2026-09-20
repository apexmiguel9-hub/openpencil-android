//! Which surface the loop-finalize path believes it is repairing, and what it
//! does with a deck violation it refuses to repair (deck-system spec §4.5/§4.6).

use super::*;
use serde_json::json;

fn nodes(values: Vec<serde_json::Value>) -> Vec<PenNode> {
    values
        .into_iter()
        .map(|value| serde_json::from_value(value).expect("fixture must deserialize as PenNode"))
        .collect()
}

fn board(id: &str, children: serde_json::Value) -> serde_json::Value {
    json!({
        "type":"frame","id":id,"name":id,"width":1920,"height":1080,
        "layout":"vertical","children":children
    })
}

#[test]
fn a_page_root_wrapper_around_a_board_is_a_board() {
    let forest = nodes(vec![board(
        "slide",
        json!([{"type":"text","id":"t","content":"Q3"}]),
    )]);
    assert_eq!(locate_root_form(&forest), DesignForm::Deck);
}

#[test]
fn a_multi_slide_deck_has_no_wrapper_and_is_still_a_deck() {
    // The shape a real deck actually has: every board at the top level. Without
    // the all-boards branch this classified as Unknown and the deck contracts
    // never reached the one document type they exist for.
    let forest = nodes(vec![
        board(
            "slide-1",
            json!([{"type":"text","id":"t1","content":"Cover"}]),
        ),
        board(
            "slide-2",
            json!([{"type":"text","id":"t2","content":"Agenda"}]),
        ),
        board(
            "slide-3",
            json!([{"type":"text","id":"t3","content":"Close"}]),
        ),
    ]);
    assert_eq!(locate_root_form(&forest), DesignForm::Deck);
}

#[test]
fn one_board_shaped_section_does_not_make_a_page_a_deck() {
    let forest = nodes(vec![
        board("hero", json!([{"type":"text","id":"t1","content":"Hero"}])),
        json!({
            "type":"frame","id":"features","name":"Features","width":1920,"height":640,
            "children":[{"type":"text","id":"t2","content":"Features"}]
        }),
    ]);
    assert_eq!(locate_root_form(&forest), DesignForm::Unknown);
}

#[test]
fn an_empty_document_has_no_form() {
    assert_eq!(locate_root_form(&[]), DesignForm::Unknown);
}

#[test]
fn a_boards_overflowing_row_is_reported_in_the_summary_not_clipped() {
    // End to end over the loop path: the board's row cannot fit at any width,
    // so the clip floor would normally take it. On a board the pass reports
    // instead — as a NOTE, because nothing was applied.
    let mut state = EditorState::default();
    let forest = nodes(vec![board(
        "slide-1",
        json!([{
            "type":"frame","id":"row","name":"KPI Row","layout":"horizontal","width":1776,"gap":48,
            "children":[
                {"type":"frame","id":"k1","name":"k1","width":700,"height":240,"children":[]},
                {"type":"frame","id":"k2","name":"k2","width":700,"height":240,"children":[]},
                {"type":"frame","id":"k3","name":"k3","width":700,"height":240,"children":[]}
            ]
        }]),
    )]);
    *state.active_children_mut() = forest;

    let summary = apply_loop_finalize_counted(&mut state);

    let notes = summary.notes().join("\n");
    assert!(
        notes.contains("KPI Row") && notes.contains("split the slide"),
        "the board overflow must reach the user-visible summary: {notes:?}"
    );
    let row = serde_json::to_value(state.active_children()).unwrap();
    assert_eq!(
        row[0]["children"][0].get("clipContent"),
        None,
        "and the row must not have been clipped: {row}"
    );
}
