use jian_ops_schema::node::container::{AlignItems, LayoutMode};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::SizingBehavior;

use crate::{import_html, HtmlImportOptions};

fn table_of(html: &str) -> (FrameNode, Vec<String>) {
    let result = import_html(html, &HtmlImportOptions::default());
    let PenNode::Frame(root) = &result.nodes[0] else {
        panic!("expected a root frame")
    };
    let PenNode::Frame(table) = &root.children.as_ref().unwrap()[0] else {
        panic!("expected the table frame")
    };
    (table.clone(), result.warnings)
}

fn rows(table: &FrameNode) -> Vec<&FrameNode> {
    table
        .children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|node| match node {
            PenNode::Frame(frame) if frame.base.name.as_deref() == Some("tr") => Some(frame),
            _ => None,
        })
        .collect()
}

fn widths(row: &FrameNode) -> Vec<f64> {
    row.children
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|cell| match cell {
            PenNode::Frame(frame) => match frame.container.width {
                Some(SizingBehavior::Number(width)) => width,
                ref other => panic!("expected a numeric cell width, found {other:?}"),
            },
            other => panic!("expected a cell frame, found {other:?}"),
        })
        .collect()
}

#[test]
fn rows_become_horizontal_frames_with_evenly_split_columns() {
    let (table, warnings) = table_of(
        "<table style='width:900px;border-collapse:collapse'>\
           <thead><tr><th>a</th><th>b</th><th>c</th></tr></thead>\
           <tbody><tr><td>1</td><td>2</td><td>3</td></tr></tbody>\
         </table>",
    );
    let rows = rows(&table);
    assert_eq!(rows.len(), 2, "thead/tbody flatten into the table");
    assert_eq!(rows[0].container.layout, Some(LayoutMode::Horizontal));
    assert_eq!(rows[0].container.align_items, Some(AlignItems::Stretch));
    assert_eq!(widths(rows[0]), [300.0, 300.0, 300.0]);
    assert_eq!(widths(rows[1]), [300.0, 300.0, 300.0]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn colspan_occupies_the_columns_it_spans_including_the_interior_gap() {
    let (table, _) = table_of(
        "<table style='width:600px;border-collapse:collapse'>\
           <tr><th colspan='2'>wide</th><th>tail</th></tr>\
           <tr><td>1</td><td>2</td><td>3</td></tr>\
         </table>",
    );
    let rows = rows(&table);
    assert_eq!(widths(rows[0]), [400.0, 200.0]);
    assert_eq!(widths(rows[1]), [200.0, 200.0, 200.0]);
}

#[test]
fn colgroup_widths_pin_tracks_and_the_rest_split_the_remainder() {
    let (table, _) = table_of(
        "<table style='width:1000px;border-collapse:collapse'>\
           <colgroup><col width='400'><col><col></colgroup>\
           <tr><td>a</td><td>b</td><td>c</td></tr>\
         </table>",
    );
    assert_eq!(widths(rows(&table)[0]), [400.0, 300.0, 300.0]);

    let (percent, _) = table_of(
        "<table style='width:1000px;border-collapse:collapse'>\
           <colgroup><col style='width:50%'><col span='2'></colgroup>\
           <tr><td>a</td><td>b</td><td>c</td></tr>\
         </table>",
    );
    assert_eq!(widths(rows(&percent)[0]), [500.0, 250.0, 250.0]);
}

#[test]
fn border_spacing_becomes_the_row_and_table_gaps_and_narrows_the_columns() {
    let (table, _) = table_of(
        "<table style='width:620px;border-spacing:10px 4px'>\
           <tr><td>a</td><td>b</td><td>c</td></tr>\
         </table>",
    );
    use jian_ops_schema::node::base::NumberOrExpression;
    assert_eq!(table.container.gap, Some(NumberOrExpression::Number(4.0)));
    let row = rows(&table)[0];
    assert_eq!(row.container.gap, Some(NumberOrExpression::Number(10.0)));
    // 620 - 2 gaps of 10 = 600, split three ways.
    assert_eq!(widths(row), [200.0, 200.0, 200.0]);
}

#[test]
fn rowspan_is_reported_and_treated_as_a_single_row() {
    let (_, warnings) = table_of(
        "<table style='width:400px;border-collapse:collapse'>\
           <tr><td rowspan='2'>tall</td><td>b</td></tr>\
           <tr><td>c</td></tr>\
         </table>",
    );
    assert!(
        warnings.iter().any(|warning| warning.contains("rowspan")),
        "{warnings:?}"
    );
}

#[test]
fn th_defaults_to_bold_centered_text_and_caption_leads_the_table() {
    let (table, _) = table_of(
        "<table style='width:400px'><caption>Plans</caption>\
           <tr><th>Header</th></tr></table>",
    );
    let children = table.children.as_deref().unwrap();
    let PenNode::Frame(caption) = &children[0] else {
        panic!("caption should be the table's first child")
    };
    assert_eq!(caption.base.name.as_deref(), Some("caption"));

    let PenNode::Frame(cell) = &rows(&table)[0].children.as_deref().unwrap()[0] else {
        panic!("expected a th frame")
    };
    let PenNode::Text(text) = &cell.children.as_deref().unwrap()[0] else {
        panic!("expected the header text")
    };
    use jian_ops_schema::node::text::{FontWeight, TextAlign};
    assert!(matches!(text.font_weight, Some(FontWeight::Number(700))));
    assert_eq!(text.text_align, Some(TextAlign::Center));
}

/// A table with no authored width is shrink-to-fit in CSS. Nothing here solves
/// that, so the containing block stands in and the approximation is reported.
#[test]
fn an_indefinite_table_width_is_measured_against_the_parent_and_reported() {
    let (_, warnings) =
        table_of("<table style='border-collapse:collapse'><tr><td>a</td><td>b</td></tr></table>");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("column widths are approximate")),
        "{warnings:?}"
    );

    let (_, pinned) = table_of(
        "<table style='width:400px;border-collapse:collapse'>\
           <tr><td>a</td><td>b</td></tr></table>",
    );
    assert!(
        !pinned
            .iter()
            .any(|warning| warning.contains("column widths are approximate")),
        "a definite width needs no approximation: {pinned:?}"
    );
}

/// `width: 100%` inside a definite parent resolves to a real width, so it is
/// as definite as a pinned one.
#[test]
fn a_full_width_table_counts_as_definite() {
    let (table, warnings) = table_of(
        "<table style='width:100%;border-collapse:collapse'>\
           <tr><td>a</td><td>b</td></tr></table>",
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning.contains("column widths are approximate")),
        "{warnings:?}"
    );
    let total: f64 = widths(rows(&table)[0]).iter().sum();
    assert_eq!(total, HtmlImportOptions::default().viewport_width);
}

/// A cell the author took out of the table box tree keeps its own layout and
/// must not consume the column grid's tracks either.
#[test]
fn a_cell_restyled_to_block_does_not_consume_its_colspan_tracks() {
    let (table, _) = table_of(
        "<table style='width:600px;border-collapse:collapse'>\
           <tr><td style='display:block' colspan='2'>a</td><td>b</td><td>c</td></tr>\
         </table>",
    );
    // Three tracks of 200: the block cell occupies one, not two.
    assert_eq!(widths(rows(&table)[0]), [200.0, 200.0, 200.0]);
}

/// Un-flattening the row groups re-parents every `<tr>`, which abandons the
/// shared column grid rather than silently mis-sizing it.
#[test]
fn un_flattened_row_groups_report_the_abandoned_column_grid() {
    let (_, warnings) = table_of(
        "<table style='width:600px'><tbody style='display:block'>\
           <tr><td>a</td><td>b</td></tr></tbody></table>",
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("un-flattened")),
        "{warnings:?}"
    );

    // An empty table has no grid to abandon and stays silent.
    let (_, empty) = table_of("<table style='width:600px'></table>");
    assert!(
        !empty.iter().any(|warning| warning.contains("un-flattened")),
        "{empty:?}"
    );
}

#[test]
fn caption_side_bottom_places_the_caption_after_the_rows() {
    let (table, _) = table_of(
        "<table style='width:400px;caption-side:bottom'><caption>Plans</caption>\
           <tr><th>Header</th></tr></table>",
    );
    let children = table.children.as_deref().unwrap();
    let PenNode::Frame(last) = children.last().expect("a child") else {
        panic!("expected a frame")
    };
    assert_eq!(last.base.name.as_deref(), Some("caption"));

    // The default keeps it first.
    let (top, _) = table_of(
        "<table style='width:400px'><caption>Plans</caption>\
           <tr><th>Header</th></tr></table>",
    );
    let PenNode::Frame(first) = &top.children.as_deref().unwrap()[0] else {
        panic!("expected a frame")
    };
    assert_eq!(first.base.name.as_deref(), Some("caption"));
}

#[test]
fn the_private_span_carrier_never_reaches_the_document() {
    let result = import_html(
        "<table style='width:400px'><tr><td colspan='2'>a</td><td>b</td></tr></table>",
        &HtmlImportOptions::default(),
    );
    let serialized = serde_json::to_string(&result.nodes).expect("serializable");
    assert!(!serialized.contains("__op_html_table_span"), "{serialized}");
}
