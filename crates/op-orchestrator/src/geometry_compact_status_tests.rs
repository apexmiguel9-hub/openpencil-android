use super::*;
use serde_json::json;
use std::collections::HashMap;

fn rects(entries: &[(&str, f64, f64, f64, f64)]) -> HashMap<String, Rect> {
    entries
        .iter()
        .map(|(id, x, y, w, h)| {
            (
                (*id).to_string(),
                Rect {
                    x: *x,
                    y: *y,
                    w: *w,
                    h: *h,
                },
            )
        })
        .collect()
}

#[test]
fn healthy_narrow_header_with_flexible_title_stays_horizontal() {
    let row = json!({
        "type":"frame","id":"header","width":180,"layout":"horizontal",
        "gap":8,"justifyContent":"space_between","alignItems":"center",
        "children":[
            {
                "type":"frame","id":"title","width":"fill_container",
                "layout":"horizontal","gap":6,"alignItems":"center",
                "children":[
                    {
                        "type":"icon_font","id":"icon","iconFontName":"wind",
                        "width":14,"height":14
                    },
                    {
                        "type":"text","id":"title-text","content":"AIR QUALITY",
                        "width":"fit_content"
                    }
                ]
            },
            {
                "type":"frame","id":"badge","width":"fit_content",
                "layout":"horizontal","gap":4,"padding":[3,8],
                "alignItems":"center",
                "fill":[{"type":"solid","color":"#C4F82A20"}],
                "children":[
                    {"type":"ellipse","id":"dot","width":6,"height":6},
                    {
                        "type":"text","id":"label","content":"Good",
                        "width":"fit_content"
                    }
                ]
            }
        ]
    });
    let rects = rects(&[
        ("header", 0.0, 0.0, 180.0, 22.0),
        ("title", 0.0, 4.0, 121.0, 14.0),
        ("icon", 0.0, 4.0, 14.0, 14.0),
        ("title-text", 20.0, 4.0, 76.0, 14.0),
        ("badge", 129.0, 0.0, 51.0, 22.0),
        ("dot", 137.0, 8.0, 6.0, 6.0),
        ("label", 147.0, 4.0, 25.0, 14.0),
    ]);
    let mut cmds = Vec::new();

    collect_row_overfull_fixes(&row, &rects, &mut cmds, false);

    assert!(
        cmds.is_empty(),
        "a fitting flexible title is healthy, not an old damaged state: {cmds:?}"
    );
}
