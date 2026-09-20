//! Explicit CSS grid placement (`grid-column` / `grid-row`).
//!
//! The row post-pass in `mapper_grid` chunks items N-per-row; an item with
//! `grid-column: span 2` has to occupy two track widths plus the gap between
//! them, and an explicit start line has to break the row when it points
//! backwards. Placement is discovered while the item itself is mapped (its
//! own computed style is only available there) and travels to the parent's
//! post-pass in `PenNodeBase::theme` under a private key, which the post-pass
//! always strips.

use crate::import_warning::ImportWarning;
use std::collections::BTreeMap;

use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::PenNode;

use crate::css::cascade::ComputedStyle;

use super::node_access::node_base_mut;
use super::MapCtx;

/// Private carrier key. Never reaches a serialized document: the grid post-pass
/// removes it from every child before the parent's children list is returned.
const PLACEMENT_KEY: &str = "__op_html_grid_placement";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Placement {
    /// 1-based explicit column start line, when the author pinned one.
    pub column_start: Option<usize>,
    pub column_span: usize,
    /// End line counted back from the last one (`/ -1` stores 1, `/ -2`
    /// stores 2). Only resolvable at pack time, once the track count is known.
    pub column_end_from_end: Option<usize>,
}

impl Default for Placement {
    /// Auto placement occupies exactly one track.
    fn default() -> Self {
        Self {
            column_start: None,
            column_span: 1,
            column_end_from_end: None,
        }
    }
}

impl Placement {
    fn is_default(&self) -> bool {
        self.column_start.is_none() && self.column_span <= 1 && self.column_end_from_end.is_none()
    }
}

/// Record the element's grid placement on its own node so the grid parent's
/// post-pass can read it. Only called when the parent is a grid container.
pub(super) fn record_placement(
    base: &mut PenNodeBase,
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
) {
    if style
        .get("grid-area")
        .is_some_and(|value| !value.trim().is_empty() && value.trim() != "auto")
    {
        context.warn_once(ImportWarning::GridTemplateAreasIgnored);
    }
    if let Some(row) = axis_shorthand(style, "grid-row") {
        // Rows are chunked evenly by the post-pass, so nothing an author says
        // about the block axis survives.
        if !parse_axis(&row).is_default() {
            context.warn_once(ImportWarning::GridRowPlacementIgnored);
        }
    }
    let Some(column) = axis_shorthand(style, "grid-column") else {
        return;
    };
    let placement = parse_axis(&column);
    if placement.is_default() && !authored_as_auto(&column) {
        // The form parsed to plain auto placement without saying `auto`:
        // a named line, a negative start line or something else unsupported.
        context.warn_once(ImportWarning::GridColumnUnsupported {
            value: column.trim().to_string(),
        });
    }
    if placement.is_default() {
        return;
    }
    base.theme
        .get_or_insert_with(BTreeMap::new)
        .insert(PLACEMENT_KEY.to_string(), encode(&placement));
}

/// The axis shorthand, composed from the `-start` / `-end` longhands when the
/// author only wrote those (`grid-column-start: 2; grid-column-end: -1`).
fn axis_shorthand(style: &ComputedStyle, axis: &str) -> Option<String> {
    if let Some(value) = style.get(axis) {
        return Some(value.to_string());
    }
    let start = style.get(&format!("{axis}-start"));
    let end = style.get(&format!("{axis}-end"));
    match (start, end) {
        (None, None) => None,
        (start, end) => Some(format!(
            "{} / {}",
            start.unwrap_or("auto").trim(),
            end.unwrap_or("auto").trim()
        )),
    }
}

/// True when every side of the shorthand really is `auto`, so auto placement
/// is what the author asked for rather than a parse giving up.
fn authored_as_auto(value: &str) -> bool {
    let value = value.trim();
    value.is_empty()
        || value
            .split('/')
            .all(|token| token.trim().is_empty() || token.trim().eq_ignore_ascii_case("auto"))
}

/// Strip every placement carrier from `children`, returning them positionally.
pub(super) fn take_placements(children: &mut [PenNode]) -> Vec<Placement> {
    children
        .iter_mut()
        .map(|node| {
            let base = node_base_mut(node);
            let Some(theme) = base.theme.as_mut() else {
                return Placement::default();
            };
            let placement = theme
                .remove(PLACEMENT_KEY)
                .and_then(|encoded| decode(&encoded))
                .unwrap_or_default();
            if theme.is_empty() {
                base.theme = None;
            }
            placement
        })
        .collect()
}

fn encode(placement: &Placement) -> String {
    format!(
        "{}:{}:{}",
        placement.column_start.unwrap_or(0),
        placement.column_span.max(1),
        placement.column_end_from_end.unwrap_or(0)
    )
}

fn decode(encoded: &str) -> Option<Placement> {
    let mut parts = encoded.split(':');
    let start = parts.next()?;
    let span = parts.next()?;
    let end_from_end = parts.next()?;
    Some(Placement {
        column_start: start.parse::<usize>().ok().filter(|value| *value > 0),
        column_span: span.parse::<usize>().ok().filter(|value| *value > 0)?,
        column_end_from_end: end_from_end
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0),
    })
}

/// One side of a `grid-column` / `grid-row` shorthand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Line {
    Auto,
    /// A positive 1-based line number.
    Number(usize),
    /// A negative line number, stored as its magnitude: `-1` is the line after
    /// the last track, `-2` the one before it.
    FromEnd(usize),
    Span(usize),
}

/// Parse one `grid-column` / `grid-row` shorthand. Supported: `span N`,
/// a positive start line, `start / span N`, `start / end` and the full-bleed
/// `start / -N` form. Named lines fall back to auto placement.
pub(super) fn parse_axis(value: &str) -> Placement {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("auto") {
        return Placement::default();
    }
    let mut parts = value.split('/');
    let start_token = parts.next().unwrap_or("").trim();
    let end_token = parts.next().map(str::trim);
    if parts.next().is_some() {
        return Placement::default();
    }
    let start = parse_line(start_token);
    let end = parse_line(end_token.unwrap_or(""));
    // A negative START line would need the track count to become a column
    // index, which the row packer never re-derives; leave it auto-placed.
    let column_start = match start {
        Line::Number(line) => Some(line),
        _ => None,
    };
    let span = match (start, end) {
        (Line::Span(span), _) | (_, Line::Span(span)) => span,
        (Line::Number(start), Line::Number(end)) if end > start => end - start,
        _ => 1,
    };
    Placement {
        column_start,
        column_span: span.max(1),
        column_end_from_end: match end {
            Line::FromEnd(magnitude) => Some(magnitude),
            _ => None,
        },
    }
}

fn parse_line(token: &str) -> Line {
    let token = token.trim();
    if token.is_empty() || token.eq_ignore_ascii_case("auto") {
        return Line::Auto;
    }
    if let Some(rest) = token
        .strip_prefix("span")
        .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        let rest = rest.trim();
        if rest.is_empty() {
            return Line::Span(1);
        }
        return rest
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .map_or(Line::Auto, Line::Span);
    }
    if let Some(magnitude) = token.strip_prefix('-') {
        return magnitude
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .map_or(Line::Auto, Line::FromEnd);
    }
    token
        .parse::<usize>()
        .ok()
        .filter(|value| *value > 0)
        .map_or(Line::Auto, Line::Number)
}

/// A packed grid row: the item index plus its 0-based start column and span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Cell {
    pub start: usize,
    pub span: usize,
}

/// What the packer had to approximate, so the caller can report it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct PackNotes {
    /// An explicit start line left cells empty; no spacers were inserted.
    pub skipped_cells: bool,
    /// An explicit start line could not host its span and was re-flowed.
    pub reflowed_start: bool,
}

/// Pack items into rows honouring spans and explicit start lines. Returns one
/// `Vec<Cell>` per row, positionally matching the item order.
pub(super) fn pack_rows(placements: &[Placement], columns: usize) -> (Vec<Vec<Cell>>, PackNotes) {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut row: Vec<Cell> = Vec::new();
    let mut cursor = 0usize;
    let mut notes = PackNotes::default();
    for placement in placements {
        let span = resolved_span(placement, columns, cursor).clamp(1, columns.max(1));
        let explicit = placement
            .column_start
            .and_then(|line| (line <= columns).then_some(line - 1))
            .filter(|start| start + span <= columns);
        if placement.column_start.is_some() && explicit.is_none() {
            notes.reflowed_start = true;
        }
        let mut start = explicit.unwrap_or(cursor);
        if start < cursor || start + span > columns {
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
            }
            cursor = 0;
            start = explicit.unwrap_or(0);
            if start + span > columns {
                start = 0;
            }
        }
        if start > cursor {
            notes.skipped_cells = true;
        }
        row.push(Cell { start, span });
        cursor = start + span;
        if cursor >= columns {
            rows.push(std::mem::take(&mut row));
            cursor = 0;
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }
    (rows, notes)
}

/// `grid-column: 1 / -1` (full bleed) only becomes a span once the track count
/// is known. Grid lines are 1-based and run `1..=columns + 1`, so the negative
/// line `-k` is line `columns + 2 - k`.
fn resolved_span(placement: &Placement, columns: usize, cursor: usize) -> usize {
    let Some(magnitude) = placement.column_end_from_end else {
        return placement.column_span;
    };
    let end_line = (columns + 2).saturating_sub(magnitude);
    let start_line = placement.column_start.unwrap_or(cursor + 1);
    end_line.saturating_sub(start_line).max(1)
}

/// Width of a cell spanning `span` tracks starting at `start`, gaps included.
pub(super) fn cell_width(track_widths: &[f64], cell: Cell, gap: f64) -> Option<f64> {
    let end = cell.start.checked_add(cell.span)?;
    if end > track_widths.len() {
        return None;
    }
    let tracks: f64 = track_widths[cell.start..end].iter().sum();
    Some(tracks + gap * (cell.span.saturating_sub(1)) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_span_and_explicit_line_forms() {
        assert_eq!(parse_axis("span 2").column_span, 2);
        assert_eq!(parse_axis("span 2").column_start, None);
        assert_eq!(parse_axis("2").column_start, Some(2));
        assert_eq!(parse_axis("2").column_span, 1);
        assert_eq!(parse_axis("2 / span 3").column_start, Some(2));
        assert_eq!(parse_axis("2 / span 3").column_span, 3);
        assert_eq!(parse_axis("1 / 4").column_span, 3);
        assert_eq!(parse_axis("auto"), Placement::default());
        // Named lines stay auto-placed; a negative END line is kept for the
        // packer to resolve once it knows the track count.
        assert_eq!(parse_axis("main-start / main-end"), Placement::default());
        assert_eq!(parse_axis("-1"), Placement::default());
        assert_eq!(parse_axis("1 / -1").column_start, Some(1));
        assert_eq!(parse_axis("1 / -1").column_end_from_end, Some(1));
        assert_eq!(parse_axis("2 / -2").column_end_from_end, Some(2));
    }

    #[test]
    fn packs_spans_and_breaks_rows_on_backwards_start_lines() {
        let span2 = Placement {
            column_span: 2,
            ..Placement::default()
        };
        let auto = Placement::default();
        let (rows, notes) = pack_rows(&[span2, auto, auto, auto], 3);
        assert_eq!(
            rows,
            vec![
                vec![Cell { start: 0, span: 2 }, Cell { start: 2, span: 1 }],
                vec![Cell { start: 0, span: 1 }, Cell { start: 1, span: 1 }],
            ]
        );
        assert_eq!(notes, PackNotes::default());

        let pinned = Placement {
            column_start: Some(1),
            ..Placement::default()
        };
        let (rows, _) = pack_rows(&[auto, pinned], 3);
        assert_eq!(rows.len(), 2, "a backwards start line opens a new row");

        // A span wider than the track count clamps instead of looping.
        let wide = Placement {
            column_span: 9,
            ..Placement::default()
        };
        let (rows, notes) = pack_rows(&[wide, auto], 2);
        assert_eq!(
            rows,
            vec![
                vec![Cell { start: 0, span: 2 }],
                vec![Cell { start: 0, span: 1 }]
            ]
        );
        assert!(!notes.reflowed_start, "an auto-placed span is not pinned");
    }

    #[test]
    fn full_bleed_end_lines_resolve_against_the_track_count() {
        // `grid-column: 1 / -1` spans every track of the row.
        let full_bleed = parse_axis("1 / -1");
        let auto = Placement::default();
        let (rows, notes) = pack_rows(&[full_bleed, auto, auto], 3);
        assert_eq!(
            rows,
            vec![
                vec![Cell { start: 0, span: 3 }],
                vec![Cell { start: 0, span: 1 }, Cell { start: 1, span: 1 }],
            ]
        );
        assert_eq!(notes, PackNotes::default());

        // `2 / -1` starts on the second track and runs to the end.
        let (rows, _) = pack_rows(&[parse_axis("2 / -1")], 4);
        assert_eq!(rows, vec![vec![Cell { start: 1, span: 3 }]]);

        // Without a start line the end line is measured from the cursor.
        let (rows, _) = pack_rows(&[auto, parse_axis("auto / -1")], 3);
        assert_eq!(
            rows,
            vec![vec![Cell { start: 0, span: 1 }, Cell { start: 1, span: 2 }]]
        );
    }

    #[test]
    fn a_pinned_start_that_cannot_host_its_span_is_reported() {
        let pinned = Placement {
            column_start: Some(3),
            column_span: 2,
            column_end_from_end: None,
        };
        let (_, notes) = pack_rows(&[pinned], 3);
        assert!(
            notes.reflowed_start,
            "start line 3 + span 2 overflows a 3-track grid"
        );
    }

    #[test]
    fn cell_width_sums_tracks_and_interior_gaps() {
        let tracks = [100.0, 200.0, 300.0];
        assert_eq!(
            cell_width(&tracks, Cell { start: 0, span: 2 }, 10.0),
            Some(310.0)
        );
        assert_eq!(
            cell_width(&tracks, Cell { start: 2, span: 1 }, 10.0),
            Some(300.0)
        );
        assert_eq!(cell_width(&tracks, Cell { start: 2, span: 2 }, 10.0), None);
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::{import_html, HtmlImportOptions};
    use jian_ops_schema::node::{FrameNode, PenNode};
    use jian_ops_schema::sizing::SizingBehavior;

    fn grid_of(html: &str) -> (FrameNode, Vec<String>) {
        let result = import_html(html, &HtmlImportOptions::default());
        let PenNode::Frame(root) = &result.nodes[0] else {
            panic!("expected a root frame")
        };
        let PenNode::Frame(grid) = &root.children.as_ref().unwrap()[0] else {
            panic!("expected the grid container")
        };
        (grid.clone(), result.warnings)
    }

    fn first_cell_width(grid: &FrameNode) -> f64 {
        let PenNode::Frame(row) = &grid.children.as_ref().unwrap()[0] else {
            panic!("expected a grid row")
        };
        let PenNode::Frame(cell) = &row.children.as_ref().unwrap()[0] else {
            panic!("expected a cell")
        };
        match cell.container.width {
            Some(SizingBehavior::Number(width)) => width,
            ref other => panic!("expected a numeric width, found {other:?}"),
        }
    }

    /// C7. `grid-column: 1 / -1` is the full-bleed idiom; the negative end
    /// line only becomes a span once the track count is known.
    #[test]
    fn a_negative_end_line_spans_to_the_end_of_the_row() {
        let (grid, warnings) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
               <div style='grid-column:1 / -1'>full</div>\
               <div>b</div><div>c</div><div>d</div></div>",
        );
        assert_eq!(grid.children.as_ref().unwrap().len(), 2);
        assert_eq!(
            first_cell_width(&grid),
            900.0,
            "the item spans all 3 tracks"
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    /// C8. The `-start` / `-end` longhands compose into the same shorthand.
    #[test]
    fn grid_column_longhands_are_read_when_the_shorthand_is_absent() {
        let (grid, _) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
               <div style='grid-column-start:1;grid-column-end:-1'>full</div>\
               <div>b</div><div>c</div><div>d</div></div>",
        );
        assert_eq!(first_cell_width(&grid), 900.0);

        let (span_grid, _) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
               <div style='grid-column-end:span 2'>wide</div>\
               <div>b</div><div>c</div></div>",
        );
        assert_eq!(first_cell_width(&span_grid), 600.0);
    }

    /// C7. Forms that parse to something other than what the author wrote no
    /// longer collapse to span 1 in silence.
    #[test]
    fn unsupported_placement_forms_are_reported() {
        let (_, warnings) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
               <div style='grid-column:main-start / main-end'>x</div>\
               <div style='grid-row:3'>y</div></div>",
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("grid-column `main-start / main-end`")),
            "{warnings:?}"
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("grid-row placement is not imported")),
            "{warnings:?}"
        );

        // A genuine `auto` is not a degradation.
        let (_, auto_warnings) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(2,1fr);width:900px'>\
               <div style='grid-column:auto'>x</div><div>y</div></div>",
        );
        assert!(auto_warnings.is_empty(), "{auto_warnings:?}");
    }

    /// N4. An explicit start line that cannot host its span is re-flowed; the
    /// `skipped_cells` warning never covered that case.
    #[test]
    fn a_pinned_start_line_that_cannot_fit_is_reported() {
        let (_, warnings) = grid_of(
            "<div style='display:grid;grid-template-columns:repeat(3,1fr);width:900px'>\
               <div style='grid-column:3 / span 2'>x</div><div>y</div></div>",
        );
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("could not host its span")),
            "{warnings:?}"
        );
    }
}
