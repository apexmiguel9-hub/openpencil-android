//! The deck gate on `fix_horizontal_overflow`'s clip floor
//! (deck-system spec §4.6).
//!
//! Clipping an over-wide row is the right last resort on a screen and the
//! wrong one on a projector board: the audience cannot scroll the tail back
//! into view, so the clipped content is simply gone and nobody knows it was
//! there. On a board the pass reports and leaves the row alone.

use serde_json::json;

use super::fix_horizontal_overflow;
use crate::deck_echo::DeckEcho;
use crate::design_type::DesignForm;

/// A 1920-wide board row whose children cannot fit it at any width — the same
/// shape as the phone chip row the clip floor was written for, scaled up.
fn overfull_board_row() -> serde_json::Value {
    json!({
        "type":"frame","id":"kpi","name":"KPI Row","layout":"horizontal",
        "width":1776,"gap":48,
        "children":[
            {"type":"frame","width":460,"height":240},
            {"type":"frame","width":460,"height":240},
            {"type":"frame","width":460,"height":240},
            {"type":"frame","width":460,"height":240}
        ]
    })
}

#[test]
fn a_deck_board_reports_the_overflow_instead_of_clipping_it() {
    let mut row = overfull_board_row();
    let before = row.clone();
    let mut echoes = Vec::new();
    fix_horizontal_overflow(&mut row, 1920.0, DesignForm::Deck, &mut echoes);

    assert_eq!(
        row.get("clipContent"),
        None,
        "a board row must never take the clip floor — clipped slide content is gone: {row}"
    );
    assert_eq!(
        row["width"], before["width"],
        "the row is left visibly too wide rather than silently spanned + clipped: {row}"
    );
    assert_eq!(
        echoes.len(),
        1,
        "the violation must still be reported: {echoes:?}"
    );
    let DeckEcho::HorizontalOverflow {
        node_id,
        node_name,
        content_width,
        available_width,
    } = &echoes[0];
    assert_eq!(node_id.as_deref(), Some("kpi"));
    assert_eq!(node_name.as_deref(), Some("KPI Row"));
    assert!(
        content_width > available_width,
        "the echo carries the measurements that proved the overflow: {echoes:?}"
    );
}

#[test]
fn every_other_form_still_takes_the_clip_floor() {
    // The gate is deck-only: Page / MobileScreen / Unknown must behave exactly
    // as they did before it existed.
    for form in [
        DesignForm::Page,
        DesignForm::MobileScreen,
        DesignForm::Unknown,
    ] {
        let mut row = overfull_board_row();
        let mut echoes = Vec::new();
        fix_horizontal_overflow(&mut row, 1920.0, form, &mut echoes);
        assert_eq!(
            row["width"],
            json!("fill_container"),
            "{form:?} still spans the viewport: {row}"
        );
        assert_eq!(
            row["clipContent"],
            json!(true),
            "{form:?} still clips at the edge: {row}"
        );
        assert!(
            echoes.is_empty(),
            "only a board echoes — every other form repaired it: {echoes:?}"
        );
    }
}

#[test]
fn a_board_row_that_only_needs_a_tighter_gap_is_still_repaired() {
    // The gate covers ONLY the clip branch. A row that fits once its gap
    // shrinks is a repair with a single correct answer, and a board takes it
    // like anything else — nothing is hidden and no page needs splitting.
    let mut row = json!({
        "type":"frame","id":"row","layout":"horizontal","width":1776,"gap":48,
        "children":[
            {"type":"frame","width":880,"height":240},
            {"type":"frame","width":880,"height":240}
        ]
    });
    let mut echoes = Vec::new();
    fix_horizontal_overflow(&mut row, 1920.0, DesignForm::Deck, &mut echoes);

    assert_eq!(
        row["gap"],
        json!(8.0),
        "the gap still tightens on a board: {row}"
    );
    assert_eq!(
        row["width"],
        json!(1776),
        "and the row keeps its width: {row}"
    );
    assert!(echoes.is_empty(), "nothing was left unrepaired: {echoes:?}");
}

#[test]
fn a_board_row_that_fits_is_untouched() {
    let mut row = json!({
        "type":"frame","id":"row","layout":"horizontal","width":1776,"gap":48,
        "children":[
            {"type":"frame","width":400,"height":240},
            {"type":"frame","width":400,"height":240}
        ]
    });
    let before = row.clone();
    let mut echoes = Vec::new();
    fix_horizontal_overflow(&mut row, 1920.0, DesignForm::Deck, &mut echoes);
    assert_eq!(row, before);
    assert!(echoes.is_empty());
}
