//! Stroke resolution for the scene builder.
//!
//! Pure code motion out of the `layout_scene.rs` spine (800-line ceiling):
//! the payload → [`SceneStroke`] conversion plus the two predicates that
//! decide when an authored stroke must NOT reach the scene at all.

use jian_scene::layout_scene::{SceneStroke, SceneStrokeAlign};
use op_editor_core::scene_vars::VariableTable;

use crate::payload::{NodePayload, StrokePayload};

/// Resolve a payload stroke into a scene stroke. The `$ref` stroke
/// resolution parallels the fill path.
pub(super) fn scene_stroke(
    s: &StrokePayload,
    node_id: &op_editor_core::NodeId,
    var_table: &VariableTable,
) -> SceneStroke {
    SceneStroke {
        // A stroke with no resolvable paint keeps painting the historical
        // opaque black for ordinary shapes; only widget nodes drop it (see
        // `is_unpainted_widget_stroke`), because for them the stroke carries
        // the *inactive track / border* role rather than a literal outline.
        color: var_table
            .stroke_color_for(node_id)
            .unwrap_or_else(|| super::array_to_color(s.color.unwrap_or([0.0, 0.0, 0.0, 1.0]))),
        width: s.width,
        sides: s.sides,
        align: match s.align {
            a if a < 0 => SceneStrokeAlign::Inside,
            a if a > 0 => SceneStrokeAlign::Outside,
            _ => SceneStrokeAlign::Center,
        },
    }
}

/// An iPhone status-bar layout shell ("Time" / "Levels") authored with a
/// stroke but no fill — Pencil paints nothing, so the no-fill stroke (which
/// resolves to opaque black) must not draw a phantom box. Mirrors
/// `legacy_payload_repair::is_legacy_status_bar_shell` on the resolved
/// `NodePayload` the scene path carries (no canonical `PenNode` here). Width
/// is intentionally unconstrained — the shells are `fill_container`, so a
/// wider status bar computes a larger width than a fixed-width sample.
pub(super) fn is_status_bar_shell_stroke(node: &NodePayload) -> bool {
    // A thin status-bar row: the authored 22 px computes to ~22-23 here
    // (stroke / rounding), so match a small range rather than an exact 22.
    // The no-fill + stroke + non-empty-children gates already exclude the
    // inner "Time" text glyph node (which is filled, strokeless, leaf).
    node.stroke.is_some()
        && node.fill.is_none()
        && matches!(node.name.as_str(), "Time" | "Levels")
        && !node.children.is_empty()
        && (18.0..=28.0).contains(&node.h)
}

/// A first-class widget node whose authored stroke resolves to no paint at
/// all — neither a solid/gradient fill on the stroke itself nor a `$ref`
/// variable. The canonical widget schema exposes one `fill` (accent) and one
/// `stroke` (inactive track / border) and lets the renderer derive the rest,
/// so "stroke declared, paint missing" must degrade to the resolver's role
/// defaults rather than to a literal black outline. Weak model output writes
/// `"stroke": {"thickness": 1}` with no `fill` often enough that this is the
/// common case, not an exotic one.
pub(super) fn is_unpainted_widget_stroke(
    node: &NodePayload,
    node_id: &op_editor_core::NodeId,
    var_table: &VariableTable,
) -> bool {
    node.widget.is_some()
        && node
            .stroke
            .as_ref()
            .is_some_and(|stroke| stroke.color.is_none())
        && var_table.stroke_color_for(node_id).is_none()
}
