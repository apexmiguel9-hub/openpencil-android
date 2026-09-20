//! Card-overflow clipping and rail-width-collapse fix collectors.

use super::*;

/// A ROUNDED, PAINTED card whose child's resolved rect pokes past the card's
/// own bounds (a heart-rate sparkline path hanging out of the card's right
/// rounded edge — measured). Rounded cards crop by convention (the CSS
/// border-radius + overflow expectation); set `clipContent` so the overshoot
/// crops at the radius instead of painting outside the card. Geometry-proven
/// and one-way (never un-clips); plain unrounded wrappers are left alone —
/// their overflows belong to the resize fixers.
pub(super) const CARD_CLIP_RADIUS_MIN: f64 = 8.0;
pub(super) const CARD_CLIP_OVERSHOOT_EPS: f64 = 2.0;

pub(super) fn collect_card_overflow_clips(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    let painted = v
        .get("fill")
        .map(|f| match f {
            Value::Array(a) => !a.is_empty(),
            Value::Null => false,
            _ => true,
        })
        .unwrap_or(false);
    let rounded = num(v, "cornerRadius") >= CARD_CLIP_RADIUS_MIN;
    let clips = v.get("clipContent").and_then(Value::as_bool) == Some(true);
    if painted && rounded && !clips {
        if let Some(pr) = v
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| rects.get(id))
        {
            // ANY descendant counts — the overshooting sparkline sits two
            // wrappers below the card, not as a direct child.
            fn any_descendant_overshoots(
                v: &Value,
                rects: &HashMap<String, Rect>,
                pr: &Rect,
            ) -> bool {
                children(v).iter().any(|c| {
                    let out = c
                        .get("id")
                        .and_then(Value::as_str)
                        .and_then(|id| rects.get(id))
                        .is_some_and(|cr| {
                            cr.x + cr.w > pr.x + pr.w + CARD_CLIP_OVERSHOOT_EPS
                                || cr.y + cr.h > pr.y + pr.h + CARD_CLIP_OVERSHOOT_EPS
                                || cr.x < pr.x - CARD_CLIP_OVERSHOOT_EPS
                                || cr.y < pr.y - CARD_CLIP_OVERSHOOT_EPS
                        });
                    out || any_descendant_overshoots(c, rects, pr)
                })
            }
            let overshoots = any_descendant_overshoots(v, rects, pr);
            if overshoots {
                if let Some(id) = v.get("id").and_then(Value::as_str) {
                    cmds.push(EditorCommand::SetNodeLayoutProp {
                        node_id: NodeId::new(id.to_string()),
                        property: "clipContent".to_string(),
                        value: LayoutPropValue::Bool(true),
                    });
                }
            }
        }
    }
    for c in children(v) {
        collect_card_overflow_clips(c, rects, cmds);
    }
}

/// A horizontal "rail" of card siblings where one card declares a FIXED
/// pixel width sized for a much wider row while its siblings use
/// `fill_container` to "share the rest" — on a narrow row the fixed card
/// alone eats most of the width, squeezing the `fill_container` siblings
/// to a sliver. Measured on a real user design (finance dashboard "Savings
/// Goals" rail, 375px mobile page): a 200px fixed first card left only
/// ~103px for its two `fill_container` siblings on a 327px inner rail,
/// squeezing them to ~51px each — their own content (icon tile, title,
/// amount) then overshoots that sliver, which `collect_card_overflow_clips`
/// reacts to by clipping (cropping the title text, "New Car" → "Nev Car")
/// instead of fixing the real cause. `overflow.md`'s HORIZONTAL SCROLL ROWS
/// contract already bans this exact mix ("`fill_container` on cards in a
/// horizontal row — they squish down to invisibility"), but the
/// `minimal_skills` last-ditch retry rung strips every skill down to
/// `schema`, so a subtask that falls that far has no way to know the rule.
/// This detector is the geometry-driven safety net: it doesn't care which
/// generation path produced the mismatch, only that the RESOLVED widths
/// prove it broke. Repair: normalize every collapsed `fill_container`
/// sibling onto the reference card's declared fixed width — "make the rest
/// match card 1", the same fix a human designer would reach for.
pub(super) const RAIL_COLLAPSE_RATIO: f64 = 2.5;
pub(super) const RAIL_COLLAPSE_FLOOR: f64 = 80.0;

pub(super) fn is_rail_card_sibling(v: &Value) -> bool {
    matches!(
        v.get("type").and_then(Value::as_str),
        Some("frame" | "group")
    )
}

pub(super) fn rail_reference_width(cards: &[Value]) -> Option<f64> {
    cards
        .iter()
        .filter(|card| is_rail_card_sibling(card) && !is_compact_status_badge_structure(card))
        .filter_map(fixed_width)
        // A rail reference must itself be card-sized. Tiny dots, icons, and
        // avatar slots are ordinary row adornments, never the width contract
        // for their fill sibling.
        .filter(|width| *width >= RAIL_COLLAPSE_FLOOR)
        .fold(None, |acc: Option<f64>, width| {
            Some(acc.map_or(width, |current| current.max(width)))
        })
}

pub(super) fn collect_rail_width_collapse_fixes(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
) {
    collect_rail_width_collapse_fixes_with_context(v, rects, cmds, false);
}

pub(super) fn collect_rail_width_collapse_fixes_with_context(
    v: &Value,
    rects: &HashMap<String, Rect>,
    cmds: &mut Vec<EditorCommand>,
    in_table: bool,
) {
    if !in_table && layout_str(v) == Some("horizontal") {
        let cards = children(v);
        // The reference: the WIDEST sibling with a declared FIXED width — the
        // pattern the other siblings should have matched. Widest, not first,
        // so a small fixed icon leading the row (e.g. a 36px IconTile before
        // a fill_container title) never gets mistaken for the reference card.
        if let Some(ref_w) = rail_reference_width(cards) {
            for c in cards {
                // Only `fill_container` siblings are candidates — a sibling
                // with its own (smaller-by-design) fixed width is left alone.
                if !is_rail_card_sibling(c)
                    || is_compact_status_badge_structure(c)
                    || c.get("width").and_then(Value::as_str) != Some("fill_container")
                {
                    continue;
                }
                let Some(cid) = c.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let Some(cr) = rects.get(cid) else {
                    continue;
                };
                if cr.w <= 0.0 {
                    continue;
                }
                let collapsed = cr.w < RAIL_COLLAPSE_FLOOR && ref_w / cr.w > RAIL_COLLAPSE_RATIO;
                if collapsed {
                    cmds.push(EditorCommand::UpdateNode {
                        node_id: NodeId::new(cid.to_string()),
                        x: None,
                        y: None,
                        width: Some(ref_w.round() as i32),
                        height: None,
                        name: None,
                        fill_hex: None,
                        page_id: None,
                    });
                }
            }
        }
    }
    let child_in_table = in_table || is_table_shape(v);
    for c in children(v) {
        collect_rail_width_collapse_fixes_with_context(c, rects, cmds, child_in_table);
    }
}
