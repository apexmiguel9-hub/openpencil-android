//! Geometry for the Styles tab's grid.
//!
//! The Templates tab is a plain uniform grid, and so was this one until
//! imports arrived. Now the list has two halves — what the user brought and
//! what ships with the app — and a heading between them, which a flat
//! row/column walk cannot express: a heading is a full-width band that
//! consumes vertical space without being a card, so every card below it
//! shifts by an amount the flat walker knows nothing about.
//!
//! Hence one walker here that both paint and hit-test call. Splitting it would
//! reintroduce exactly the class of bug the shared `card_rects` contract
//! exists to prevent: a heading counted in paint but not in hit-test puts
//! every card's click target one band away from where it is drawn.
//!
//! With no imports the walker degenerates to the flat grid it replaced —
//! same rects, no headings — so the shipped-only case is untouched.

use crate::Rect;

/// Height of a section heading band, heading text included.
pub(super) const STYLE_SECTION_HEADER_H: f32 = 32.0;

/// Extra breathing room between the imported section and the shipped one.
pub(super) const STYLE_SECTION_GAP: f32 = 12.0;

/// A section heading in the grid.
pub(super) struct StyleSectionHeader {
    pub rect: Rect,
    /// `true` for the imported section, `false` for the shipped corpus.
    pub is_user: bool,
}

/// Where every card and heading sits, and how tall the whole grid is.
pub(super) struct StyleGridLayout {
    pub headers: Vec<StyleSectionHeader>,
    /// `(index into the card list, rect)`, in list order.
    pub cards: Vec<(usize, Rect)>,
    /// Total scrollable height, headings included.
    pub content_height: f32,
}

/// Metrics the walker needs that it cannot derive from the viewport alone.
pub(super) struct StyleGridMetrics {
    pub columns: usize,
    pub card_w: f32,
    pub card_h: f32,
    pub card_gap: f32,
    /// How many leading entries are imports. The list is ordered
    /// imports-first, so this is also where the shipped section begins.
    pub user_count: usize,
    pub total: usize,
}

/// Lay the Styles grid out inside `viewport`, scrolled by `scroll`.
pub(super) fn style_grid_layout(
    viewport: Rect,
    metrics: &StyleGridMetrics,
    scroll: f32,
) -> StyleGridLayout {
    let columns = metrics.columns.max(1);
    let user_count = metrics.user_count.min(metrics.total);
    let builtin_count = metrics.total - user_count;
    // Headings are for telling the two halves apart. With only one half
    // present there is nothing to tell apart, and a lone "Built-in styles"
    // band over the shipped catalogue is a label nobody asked for.
    let sectioned = user_count > 0 && builtin_count > 0;

    let mut headers = Vec::new();
    let mut cards = Vec::with_capacity(metrics.total);
    let mut y = viewport.origin.y - scroll;

    let place_rows = |cards: &mut Vec<(usize, Rect)>, y: &mut f32, first: usize, count: usize| {
        for row_start in (0..count).step_by(columns) {
            for column in 0..columns.min(count - row_start) {
                cards.push((
                    first + row_start + column,
                    Rect::xywh(
                        viewport.origin.x + column as f32 * (metrics.card_w + metrics.card_gap),
                        *y,
                        metrics.card_w,
                        metrics.card_h,
                    ),
                ));
            }
            *y += metrics.card_h + metrics.card_gap;
        }
        if count > 0 {
            // The trailing gap belongs between rows, not after the last one.
            *y -= metrics.card_gap;
        }
    };

    if sectioned {
        headers.push(StyleSectionHeader {
            rect: Rect::xywh(
                viewport.origin.x,
                y,
                viewport.size.x,
                STYLE_SECTION_HEADER_H,
            ),
            is_user: true,
        });
        y += STYLE_SECTION_HEADER_H;
        place_rows(&mut cards, &mut y, 0, user_count);
        y += STYLE_SECTION_GAP + metrics.card_gap;
        headers.push(StyleSectionHeader {
            rect: Rect::xywh(
                viewport.origin.x,
                y,
                viewport.size.x,
                STYLE_SECTION_HEADER_H,
            ),
            is_user: false,
        });
        y += STYLE_SECTION_HEADER_H;
        place_rows(&mut cards, &mut y, user_count, builtin_count);
    } else {
        place_rows(&mut cards, &mut y, 0, metrics.total);
    }

    StyleGridLayout {
        headers,
        cards,
        content_height: (y - (viewport.origin.y - scroll)).max(0.0),
    }
}

#[cfg(test)]
#[path = "asset_center_style_layout_tests.rs"]
mod asset_center_style_layout_tests;
