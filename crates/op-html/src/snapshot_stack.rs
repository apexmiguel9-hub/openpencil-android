//! CSS painting order → canonical child order for browser snapshots.
//!
//! A snapshot child array arrives in DOM order, which is *back-to-front*:
//! CSS paints later siblings over earlier ones, and lifts positioned /
//! z-indexed boxes out of that order entirely. Canonical `PenNode` children
//! are the opposite — `children[0]` is the frontmost layer (the same
//! convention [`crate::mapper_stack`] encodes for the HTML path and
//! `jian-scene` hit-testing documents).
//!
//! Emitting DOM order unchanged therefore inverts every overlap. On a real
//! landing page that is catastrophic rather than cosmetic: a full-bleed hero
//! background is nearly always the first child of its section, so it lands on
//! top and buries the heading, the call-to-action, and every overlay the page
//! is about.
//!
//! Snapshot frames are absolutely positioned (`LayoutMode::None`, explicit
//! `x`/`y` per child), so reordering here changes paint only — never
//! geometry. That is what makes a full CSS-order reconstruction safe in this
//! path where the HTML mapper has to keep flow items in source order.

use serde_json::{Map, Value};

/// Where a child paints inside its parent's stacking context, ordered
/// back-to-front. Derived `Ord` is the painting order, so a plain sort puts
/// the list in CSS paint order.
///
/// This is CSS 2.1 appendix E steps 3 (negative stacking contexts), 4 (in-flow
/// block boxes), 5 (inline content) and 8 (positioned descendants) collapsed to
/// what a computed-style snapshot can actually distinguish. Floats (step 4's
/// sibling) are not represented: `float` is not captured, and a floated box's
/// rect already lands where the browser put it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PaintBucket {
    /// `z-index` below zero — painted behind the parent's in-flow content.
    NegativeZ(i32),
    /// In-flow, non-positioned boxes.
    Flow,
    /// Inline content (snapshot text runs), which CSS paints above the
    /// backgrounds of in-flow block siblings.
    Inline,
    /// Positioned (`relative` / `absolute` / `fixed` / `sticky`) with
    /// `z-index: auto` or `0`.
    Positioned,
    /// `z-index` above zero, ascending.
    PositiveZ(i32),
}

/// Is this computed `display` value a flex or grid container?
///
/// The children of one are flex / grid *items*, and CSS gives `z-index`
/// effect on an item even while it stays `position: static` — the one case
/// where a child's own styles are not enough to place it.
pub(super) fn is_flex_or_grid(display: Option<&String>) -> bool {
    display.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "flex" | "inline-flex" | "grid" | "inline-grid"
        )
    })
}

/// Classify one raw snapshot child.
///
/// `parent_is_flex_or_grid` comes from the *parent* element's captured
/// `display`, because that is what decides whether a static child's `z-index`
/// is honoured or inert.
pub(super) fn paint_bucket(child: &Value, parent_is_flex_or_grid: bool) -> PaintBucket {
    let Some(object) = child.as_object() else {
        return PaintBucket::Flow;
    };
    if object.get("kind").and_then(Value::as_str) == Some("text") {
        return PaintBucket::Inline;
    }
    let styles = object.get("styles").and_then(Value::as_object);
    let positioned = styles
        .and_then(|styles| style_str(styles, "position"))
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "relative" | "absolute" | "fixed" | "sticky"
            )
        });
    let z_index = styles
        .and_then(|styles| style_str(styles, "z-index"))
        .and_then(|value| value.trim().parse::<i32>().ok());
    // A `z-index` takes effect on a positioned box, and on a flex / grid item
    // whatever its `position` — that second case is the negative-margin
    // overlay idiom (`margin-left: -12px; z-index: 2` on an avatar stack),
    // where reading the item as ordinary flow inverts every overlap.
    let z_applies = positioned || parent_is_flex_or_grid;
    match z_index {
        Some(z) if z_applies && z < 0 => PaintBucket::NegativeZ(z),
        Some(z) if z_applies && z > 0 => PaintBucket::PositiveZ(z),
        // `z-index: 0` (or `auto`) on a static item paints in document order
        // exactly like an unstyled sibling, so it stays in `Flow`.
        _ if positioned => PaintBucket::Positioned,
        _ => PaintBucket::Flow,
    }
}

fn style_str<'a>(styles: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    styles.get(name).and_then(Value::as_str)
}

/// Reorder mapped children from DOM order into canonical front-to-back order.
///
/// `entries` must be in DOM order; each item carries the bucket of the raw
/// child it came from. Within a bucket the later sibling paints on top, so it
/// ends up earlier in the returned list.
pub(super) fn to_front_to_back<T>(entries: Vec<(PaintBucket, T)>) -> Vec<T> {
    let mut indexed: Vec<(PaintBucket, usize, T)> = entries
        .into_iter()
        .enumerate()
        .map(|(index, (bucket, node))| (bucket, index, node))
        .collect();
    // Back-to-front: bucket first, DOM order inside a bucket.
    indexed.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    indexed.reverse();
    indexed.into_iter().map(|(_, _, node)| node).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn child(kind: &str, position: &str, z_index: Option<&str>) -> Value {
        let mut styles = serde_json::Map::new();
        if !position.is_empty() {
            styles.insert("position".into(), json!(position));
        }
        if let Some(z_index) = z_index {
            styles.insert("z-index".into(), json!(z_index));
        }
        json!({ "kind": kind, "styles": Value::Object(styles) })
    }

    /// Bucket a child of an ordinary block parent.
    fn bucket(child: &Value) -> PaintBucket {
        paint_bucket(child, false)
    }

    #[test]
    fn dom_order_is_reversed_for_plain_siblings() {
        let entries = vec![
            (bucket(&child("element", "", None)), "first"),
            (bucket(&child("element", "", None)), "second"),
            (bucket(&child("element", "", None)), "third"),
        ];
        assert_eq!(to_front_to_back(entries), ["third", "second", "first"]);
    }

    #[test]
    fn full_bleed_background_first_in_dom_lands_at_the_back() {
        // The hero pattern: an absolutely positioned background element
        // declared before the overlay content it sits behind.
        let entries = vec![
            (bucket(&child("element", "absolute", Some("0"))), "bg"),
            (bucket(&child("element", "relative", Some("2"))), "heading"),
            (bucket(&child("element", "absolute", Some("2"))), "phone"),
        ];
        assert_eq!(to_front_to_back(entries), ["phone", "heading", "bg"]);
    }

    #[test]
    fn negative_z_stays_behind_flow_and_text_stays_above_it() {
        let entries = vec![
            (bucket(&child("text", "", None)), "label"),
            (bucket(&child("element", "", None)), "card"),
            (bucket(&child("element", "absolute", Some("-1"))), "behind"),
        ];
        assert_eq!(to_front_to_back(entries), ["label", "card", "behind"]);
    }

    #[test]
    fn positive_z_order_is_ascending_and_beats_positioned_auto() {
        let entries = vec![
            (bucket(&child("element", "absolute", Some("5"))), "five"),
            (bucket(&child("element", "absolute", None)), "auto"),
            (bucket(&child("element", "fixed", Some("10"))), "ten"),
        ];
        assert_eq!(to_front_to_back(entries), ["ten", "five", "auto"]);
    }

    #[test]
    fn z_index_on_a_static_box_does_not_reorder_it() {
        let entries = vec![
            (bucket(&child("element", "static", Some("99"))), "first"),
            (bucket(&child("element", "", None)), "second"),
        ];
        assert_eq!(to_front_to_back(entries), ["second", "first"]);
    }

    /// The avatar-stack idiom: static flex items overlapped by a negative
    /// margin, ordered by `z-index` alone. CSS honours `z-index` on a flex
    /// item, so the first item declared has to end up on top.
    #[test]
    fn z_index_on_a_static_flex_item_is_honoured() {
        let first = child("element", "", Some("3"));
        let second = child("element", "", Some("2"));
        let third = child("element", "", Some("1"));
        let entries = vec![
            (paint_bucket(&first, true), "first"),
            (paint_bucket(&second, true), "second"),
            (paint_bucket(&third, true), "third"),
        ];
        assert_eq!(to_front_to_back(entries), ["first", "second", "third"]);
        // The same children under a block parent keep plain document order,
        // because there `z-index` on a static box is inert.
        let block = vec![
            (paint_bucket(&first, false), "first"),
            (paint_bucket(&second, false), "second"),
            (paint_bucket(&third, false), "third"),
        ];
        assert_eq!(to_front_to_back(block), ["third", "second", "first"]);
    }

    #[test]
    fn zero_z_index_on_a_static_flex_item_stays_in_flow() {
        let entries = vec![
            (
                paint_bucket(&child("element", "", Some("0")), true),
                "first",
            ),
            (paint_bucket(&child("element", "", None), true), "second"),
        ];
        assert_eq!(to_front_to_back(entries), ["second", "first"]);
    }

    #[test]
    fn flex_and_grid_displays_are_recognized() {
        for value in ["flex", "inline-flex", "grid", "inline-grid", " GRID "] {
            assert!(is_flex_or_grid(Some(&value.to_string())), "{value}");
        }
        for value in ["block", "inline", "inline-block", "table"] {
            assert!(!is_flex_or_grid(Some(&value.to_string())), "{value}");
        }
        assert!(!is_flex_or_grid(None));
    }
}
