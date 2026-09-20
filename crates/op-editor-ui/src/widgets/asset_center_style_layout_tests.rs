//! Styles-grid geometry tests.

use super::*;
use crate::Point2D;

const VIEWPORT: Rect = Rect {
    origin: Point2D { x: 100.0, y: 200.0 },
    size: Point2D { x: 620.0, y: 400.0 },
};

fn metrics(user_count: usize, total: usize) -> StyleGridMetrics {
    StyleGridMetrics {
        columns: 2,
        card_w: 300.0,
        card_h: 100.0,
        card_gap: 20.0,
        user_count,
        total,
    }
}

/// With nothing imported the walker must produce exactly the flat grid it
/// replaced — same rects, no headings — or every shipped-only install would
/// see the layout shift for a feature it is not using.
#[test]
fn without_imports_the_grid_is_flat_and_unheaded() {
    let layout = style_grid_layout(VIEWPORT, &metrics(0, 5), 0.0);
    assert!(layout.headers.is_empty());
    assert_eq!(layout.cards.len(), 5);
    assert_eq!(layout.cards[0].1.origin, Point2D::new(100.0, 200.0));
    assert_eq!(layout.cards[1].1.origin, Point2D::new(420.0, 200.0));
    assert_eq!(layout.cards[2].1.origin, Point2D::new(100.0, 320.0));
    // Three rows of 100 with two 20px gaps between them.
    assert_eq!(layout.content_height, 340.0);
}

/// A catalogue that is *only* imports needs no heading either: there is
/// nothing to tell it apart from.
#[test]
fn an_all_imported_catalogue_needs_no_headings() {
    let layout = style_grid_layout(VIEWPORT, &metrics(3, 3), 0.0);
    assert!(layout.headers.is_empty());
    assert_eq!(layout.cards.len(), 3);
}

#[test]
fn a_mixed_catalogue_heads_both_sections_and_keeps_indices_in_list_order() {
    let layout = style_grid_layout(VIEWPORT, &metrics(1, 4), 0.0);
    assert_eq!(layout.headers.len(), 2);
    assert!(layout.headers[0].is_user);
    assert!(!layout.headers[1].is_user);
    // Headings run the full width of the grid; cards do not.
    assert_eq!(layout.headers[0].rect.size.x, VIEWPORT.size.x);

    let indices: Vec<usize> = layout.cards.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, vec![0, 1, 2, 3]);

    // The import sits under the first heading, alone on its row…
    assert_eq!(layout.cards[0].1.origin.y, 200.0 + STYLE_SECTION_HEADER_H);
    // …and the shipped section starts below the second heading, so the
    // first corpus card cannot share a row with it.
    assert!(layout.cards[1].1.origin.y > layout.cards[0].1.origin.y + 100.0);
    assert_eq!(layout.cards[1].1.origin.x, VIEWPORT.origin.x);
}

/// Paint clips to the viewport and hit-test rejects outside it, so both rely
/// on the walker applying the scroll — a layout that ignored it would draw
/// row one forever.
#[test]
fn scrolling_moves_every_row_and_every_heading_by_the_same_amount() {
    let unscrolled = style_grid_layout(VIEWPORT, &metrics(1, 4), 0.0);
    let scrolled = style_grid_layout(VIEWPORT, &metrics(1, 4), 60.0);

    assert_eq!(scrolled.content_height, unscrolled.content_height);
    assert_eq!(
        scrolled.headers[0].rect.origin.y,
        unscrolled.headers[0].rect.origin.y - 60.0
    );
    for (before, after) in unscrolled.cards.iter().zip(scrolled.cards.iter()) {
        assert_eq!(after.1.origin.y, before.1.origin.y - 60.0);
        assert_eq!(after.1.origin.x, before.1.origin.x);
    }
}

/// The heading bands are why the two tabs cannot share a height formula: a
/// sectioned grid is strictly taller than the same cards laid out flat, and a
/// scroll limit computed from the flat height would clip the last row off.
#[test]
fn headings_add_to_the_scrollable_height() {
    let flat = style_grid_layout(VIEWPORT, &metrics(0, 4), 0.0);
    let sectioned = style_grid_layout(VIEWPORT, &metrics(1, 4), 0.0);
    assert!(sectioned.content_height > flat.content_height);
}

#[test]
fn an_empty_catalogue_lays_out_to_nothing() {
    let layout = style_grid_layout(VIEWPORT, &metrics(0, 0), 0.0);
    assert!(layout.cards.is_empty());
    assert!(layout.headers.is_empty());
    assert_eq!(layout.content_height, 0.0);
}

/// A column count of zero would divide by zero in the row walk; the walker
/// clamps rather than trusting its caller, because the caller derives it from
/// a panel width that a collapsed layout can drive to nothing.
#[test]
fn a_degenerate_column_count_does_not_divide_by_zero() {
    let mut metrics = metrics(0, 3);
    metrics.columns = 0;
    let layout = style_grid_layout(VIEWPORT, &metrics, 0.0);
    assert_eq!(layout.cards.len(), 3);
}
