//! Sidebar footer sinking (both the inline and already-structured paths)
//! and the column restructure into a sidebar/content row.

use super::*;

/// Drop the sidebar's footer (user/profile card) to the bottom of the now
/// full-height column. Pencil splits the sidebar into a Top + Bottom group with
/// `justifyContent: space_between`; weak models emit a flat column with a
/// FIXED-height spacer (e.g. 120px) that no longer reaches the bottom once the
/// sidebar stretches to the content height — so the footer floats mid-column.
/// Stretch an existing spacer to `fill_container`; if there is none, inject a
/// flexible spacer just before a footer-like last child.
pub(super) fn sink_sidebar_footer(sidebar: &mut Value) {
    let sidebar_id = sidebar
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("sidebar")
        .to_string();
    let Some(kids) = sidebar.get_mut("children").and_then(Value::as_array_mut) else {
        return;
    };
    if kids.len() < 2 {
        return;
    }
    let mut stretched = false;
    for c in kids.iter_mut() {
        if ident_text(c).contains("spacer") {
            if let Some(o) = c.as_object_mut() {
                o.insert("height".into(), json!("fill_container"));
                stretched = true;
            }
        }
    }
    if stretched {
        return;
    }
    let last_is_footer = kids
        .last()
        .map(ident_text)
        .map(|t| {
            ["user", "profile", "account", "avatar", "footer", "member"]
                .iter()
                .any(|k| t.contains(k))
        })
        .unwrap_or(false);
    if last_is_footer {
        let pos = kids.len() - 1;
        kids.insert(
            pos,
            json!({
                "type": "frame",
                "id": format!("{sidebar_id}-spacer"),
                "name": "Sidebar Spacer",
                "width": "fill_container",
                "height": "fill_container",
                "children": [],
            }),
        );
    }
}

// ── Standalone footer-sink for ALREADY-STRUCTURED sidebars ──
//
// `reshape_sidebar_to_app_shell` (+ its `sink_sidebar_footer`) only fires when
// the root is a flat-vertical band that must be turned INTO an app-shell. When a
// weak model already emits the correct `[sidebar | content]` shell but stacks the
// sidebar nav as a FLAT fit_content column — brand, nav groups, then a user/Pro
// footer as the last item, with NO `space_between` and NO spacer — the footer
// rides directly under the nav and the bottom of the rail is dead space. This
// whole-root pass sinks that footer: promote the column to `fill_container` and
// inject a flexible spacer before its footer-like last child. Runs in
// `run_cleanup_passes` (after the app-shell reshape, so it only sees sidebars the
// reshape left alone). Self-contained per node; never drops anything.

/// True when a node reads as a sidebar FOOTER: an explicit footer-ish name, or
/// (for the common UNNAMED footer card) its subtree text carries an account /
/// owner / upgrade signal. Name-only detection misses glm's unnamed
/// `{avatar, "James Miller", "Shop Owner"}` card — the content check catches it.
pub(super) fn is_footer_like(v: &Value) -> bool {
    let t = ident_text(v);
    if ["user", "profile", "account", "avatar", "footer", "member"]
        .iter()
        .any(|k| t.contains(k))
    {
        return true;
    }
    let mut text = String::new();
    collect_subtree_text(v, &mut text);
    [
        "owner", "admin", "account", "sign out", "log out", "upgrade", "pro plan", "go pro",
        "settings",
    ]
    .iter()
    .any(|k| text.contains(k))
}

/// Lowercased concatenation of every `text`/`content` string in the subtree.
pub(super) fn collect_subtree_text(v: &Value, out: &mut String) {
    if let Some(content) = v.get("content").and_then(Value::as_str) {
        out.push_str(&content.to_lowercase());
        out.push(' ');
    }
    if let Some(kids) = v.get("children").and_then(Value::as_array) {
        for c in kids {
            collect_subtree_text(c, out);
        }
    }
}

/// Whole-root driver: recurse, sinking the footer of any flat sidebar-nav column.
pub(crate) fn sink_structured_sidebar_footers(root: &mut PenNode) -> bool {
    let Ok(mut v) = serde_json::to_value(&*root) else {
        return false;
    };
    if !sink_mut(&mut v) {
        return false;
    }
    match serde_json::from_value::<PenNode>(v) {
        Ok(new_node) => {
            *root = new_node;
            true
        }
        Err(_) => false,
    }
}

/// Pure predicate: does THIS node qualify as a flat sidebar nav whose footer
/// should be sunk? (sidebar-named vertical column, hug height, ≥3 children, last
/// child footer-like, no existing distribution intent / spacer.)
pub(super) fn try_sink_flat_sidebar_nav(v: &Value) -> bool {
    if !is_sidebar_named(&ident_text(v)) {
        return false;
    }
    if layout_str(v) != Some("vertical") {
        return false;
    }
    // Already distributing or already has a spacer → leave alone (idempotent +
    // respects an author who DID express the pattern).
    if matches!(
        v.get("justifyContent").and_then(Value::as_str),
        Some("space_between") | Some("space_around") | Some("space_evenly")
    ) {
        return false;
    }
    let Some(kids) = v.get("children").and_then(Value::as_array) else {
        return false;
    };
    if kids.len() < 3 {
        return false;
    }
    if kids.iter().any(|c| ident_text(c).contains("spacer")) {
        return false;
    }
    kids.last().map(is_footer_like).unwrap_or(false)
}

/// A sidebar wrapper that already HAS the two-group anatomy — EXACTLY
/// [top group, footer-like bottom group] — but forgot the distribution
/// contract (`height:fill_container` + `justifyContent:space_between`), so
/// the footer floats right under the nav instead of sinking (measured: a
/// "Sidebar Navigation" with a correct Top Group / Bottom Group pair, hug
/// height, no justifyContent). The 3-children spacer arm can't reach it;
/// this arm just supplies the missing two properties.
pub(super) fn try_sink_two_group_sidebar(v: &Value) -> bool {
    if !is_sidebar_named(&ident_text(v)) {
        return false;
    }
    if layout_str(v) != Some("vertical") {
        return false;
    }
    if matches!(
        v.get("justifyContent").and_then(Value::as_str),
        Some("space_between") | Some("space_around") | Some("space_evenly")
    ) {
        return false;
    }
    let Some(kids) = v.get("children").and_then(Value::as_array) else {
        return false;
    };
    kids.len() == 2 && two_group_footerish(&kids[1]) && !two_group_footerish(&kids[0])
}

/// Footer-ness for the two-group arm: the bottom group is usually NAMED
/// "Bottom Group" (not user/profile), with the profile card one level down —
/// so accept a "bottom"-named group or any DESCENDANT whose name reads as a
/// user/profile/account card, on top of the plain [`is_footer_like`] check.
pub(super) fn two_group_footerish(v: &Value) -> bool {
    if is_footer_like(v) || ident_text(v).contains("bottom") {
        return true;
    }
    fn descendant_profile_named(v: &Value) -> bool {
        let t = ident_text(v);
        if ["user", "profile", "account", "avatar", "member"]
            .iter()
            .any(|k| t.contains(k))
        {
            return true;
        }
        v.get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(descendant_profile_named)
    }
    descendant_profile_named(v)
}

/// Mutating walk: apply the sink to qualifying nodes, recursing into children.
pub(super) fn sink_mut(v: &mut Value) -> bool {
    let mut changed = false;
    if try_sink_two_group_sidebar(v) {
        if let Some(obj) = v.as_object_mut() {
            obj.insert("height".into(), json!("fill_container"));
            obj.insert("justifyContent".into(), json!("space_between"));
            changed = true;
        }
    }
    if try_sink_flat_sidebar_nav(v) {
        let id = v
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("sidebar")
            .to_string();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("height".into(), json!("fill_container"));
        }
        if let Some(kids) = v.get_mut("children").and_then(Value::as_array_mut) {
            let pos = kids.len() - 1;
            kids.insert(
                pos,
                json!({
                    "type": "frame",
                    "id": format!("{id}-footer-spacer"),
                    "name": "Sidebar Spacer",
                    "width": "fill_container",
                    "height": "fill_container",
                    "children": [],
                }),
            );
            changed = true;
        }
    }
    if let Some(kids) = v.get_mut("children").and_then(Value::as_array_mut) {
        for c in kids.iter_mut() {
            changed |= sink_mut(c);
        }
    }
    changed
}

/// Recursively retarget any descendant whose width was sized to (about) the OLD
/// full root width down to `fill_container`, so full-width sections/dividers
/// fill their new narrower column instead of overflowing it (taffy never
/// shrinks the cross axis). Targeted: only ~full-width Number widths change.
pub(super) fn fill_full_width_descendants(v: &mut Value, old_root_w: f64) {
    if let Some(w) = num(v, "width") {
        if w >= 0.9 * old_root_w {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("width".into(), json!("fill_container"));
            }
        }
    }
    if let Some(kids) = v.get_mut("children").and_then(Value::as_array_mut) {
        for c in kids.iter_mut() {
            fill_full_width_descendants(c, old_root_w);
        }
    }
}

/// Detection already passed: split the wrapper's children into `[sidebar |
/// content-column]` and flip the wrapper to a horizontal app-shell. Returns
/// `false` only if the tree shape changed under us (defensive).
pub(super) fn restructure(v: &mut Value) -> bool {
    let root_w = match num(v, "width") {
        Some(w) => w,
        None => return false,
    };
    let content_gap = num(v, "gap").filter(|g| *g > 0.0).unwrap_or(24.0);
    let content_id = format!(
        "{}-content",
        v.get("id").and_then(Value::as_str).unwrap_or("root")
    );

    let Some(obj) = v.as_object_mut() else {
        return false;
    };
    let Some(kids) = obj.get_mut("children").and_then(Value::as_array_mut) else {
        return false;
    };
    if kids.len() < 3 {
        return false;
    }
    let mut sidebar = kids.remove(0);
    let mut content_sections: Vec<Value> = std::mem::take(kids);

    // Sidebar → fixed narrow vertical column that stretches to the row height.
    // `height: fill_container` is the LOAD-BEARING cross-axis stretch (jian maps
    // `alignItems: stretch` to FlexStart, so the wrapper's alignItems alone
    // would NOT stretch it). Clip + width-retarget keep its old full-width
    // descendants from bleeding over the content column.
    if let Some(s) = sidebar.as_object_mut() {
        s.insert("width".into(), json!(SIDEBAR_WIDTH));
        s.insert("height".into(), json!("fill_container"));
        if s.get("layout").and_then(Value::as_str) != Some("vertical") {
            s.insert("layout".into(), json!("vertical"));
        }
        s.insert("clipContent".into(), json!(true));
    }
    fill_full_width_descendants(&mut sidebar, root_w);
    sink_sidebar_footer(&mut sidebar);

    // Content sections → fill the new column instead of the old full root width.
    for section in content_sections.iter_mut() {
        fill_full_width_descendants(section, root_w);
    }

    let content = json!({
        "type": "frame",
        "id": content_id,
        "name": "Main Content",
        "width": "fill_container",
        "height": "fit_content",
        "layout": "vertical",
        "gap": content_gap,
        // Outer page gutter (Pencil's app-shell content carries padding:[32,40]
        // so sections don't run edge-to-edge into the viewport). Without it the
        // new fill_container column lets the stat cards touch the right edge.
        "padding": CONTENT_PADDING,
        "children": content_sections,
    });

    obj.insert("children".into(), json!([sidebar, content]));
    obj.insert("layout".into(), json!("horizontal"));
    obj.insert("gap".into(), json!(0));
    obj.insert("alignItems".into(), json!("stretch"));
    // The old height was the SUM of the vertical stack (incl. the sidebar); the
    // horizontal shell only needs the taller column. Track content instead of
    // leaving ~600px of dead canvas below.
    obj.insert("height".into(), json!("fit_content"));
    true
}

// ── Evict mis-parented content sections from the sidebar column ──
//
// A dashboard's LEFT RAIL holds navigation only. But the two-column routing
// (`run.rs` → `dashboard_columns::is_sidebar_subtask`) matches the bare
// `nav`/`menu` substrings, so a weak model that describes a "Client Directory"
// content section with a "filter menu" — or emits it as a second forest root of
// the sidebar subtask — strands a full data TABLE inside the 260px `clipContent`
// rail, where it overflows and paints over the nav. This whole-root pass
// relocates such sections into the sibling `Main Content` column.
//
// Detection is STRUCTURAL (a real multi-column data table) or a content-only
// NAME token ("table"/"directory"/"data grid"). It deliberately does NOT reuse
// `section_has_dashboard_signal`, whose broad "analytics"/"metric" tokens also
// match sidebar MENU-ITEM labels (this very bug's rail carries an "Analytics"
// nav item — flagging the whole rail as content would evict the navigation
// itself). Runs in `run_cleanup_passes`. Self-contained; only MOVES nodes.
