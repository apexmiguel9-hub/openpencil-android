//! Synthetic wrapper frames that emulate the two CSS behaviours the Jian
//! schema cannot express on a node itself.
//!
//! * **In-flow offsets** — `position: relative` insets and a `transform`
//!   translation on a static element. Writing them onto `base.x` / `base.y`
//!   would make the layout engine treat the node as absolutely positioned and
//!   drop it out of its parent's auto layout, so the node is re-parented into
//!   a fixed-size wrapper that keeps the original box in flow and carries the
//!   shift internally.
//! * **Per-child auto margins** — `margin-left/right: auto`. The schema has no
//!   `align-self`, so the child is re-parented into a full-width row whose
//!   `justify_content` reproduces the push.

use crate::import_warning::ImportWarning;
use jian_ops_schema::node::base::PenNodeBase;
use jian_ops_schema::node::container::{ContainerProps, JustifyContent, LayoutMode};
use jian_ops_schema::node::{FrameNode, PenNode};
use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

use crate::css::cascade::ComputedStyle;

use super::node_access::{node_base, node_base_mut, node_height, node_width};
use super::MapCtx;

/// Horizontal placement implied by a child's `auto` margins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AutoMarginAlign {
    Center,
    End,
    Start,
}

impl AutoMarginAlign {
    fn justify(self) -> JustifyContent {
        match self {
            Self::Center => JustifyContent::Center,
            Self::End => JustifyContent::End,
            Self::Start => JustifyContent::Start,
        }
    }
}

/// Read `margin-left` / `margin-right: auto` off a child's computed style.
/// Only the inline axis is emulated; block-axis auto margins keep a warning.
pub(super) fn auto_margin_align(
    style: &ComputedStyle,
    context: &mut MapCtx<'_>,
) -> Option<AutoMarginAlign> {
    let is_auto = |name: &str| style.get(name).is_some_and(|value| value.trim() == "auto");
    if is_auto("margin-top") || is_auto("margin-bottom") {
        context.warn_once(ImportWarning::BlockAutoMarginsIgnored);
    }
    match (is_auto("margin-left"), is_auto("margin-right")) {
        (true, true) => Some(AutoMarginAlign::Center),
        (true, false) => Some(AutoMarginAlign::End),
        (false, true) => Some(AutoMarginAlign::Start),
        (false, false) => None,
    }
}

/// True when the parent's own layout can host a full-width alignment row.
/// Grid parents assign explicit track widths to their direct children and
/// wrapping flex parents measure them, so both keep the plain child.
///
/// A wrapping flex parent whose emulation later bails keeps the plain child
/// too — by then the child is already mapped, so the decision cannot be
/// revisited. `mapper_wrap`'s bail warnings name that loss.
pub(super) fn parent_accepts_alignment_row(parent: Option<&ComputedStyle>) -> bool {
    let Some(parent) = parent else {
        return false;
    };
    if matches!(parent.get("display"), Some("grid" | "inline-grid")) {
        return false;
    }
    !matches!(parent.get("flex-wrap"), Some("wrap" | "wrap-reverse"))
}

/// Re-parent `node` into a full-width row that reproduces the auto-margin
/// push. A `FillContainer` child already spans the row, so it is left alone.
pub(super) fn wrap_auto_margin(
    context: &mut MapCtx<'_>,
    node: PenNode,
    align: AutoMarginAlign,
) -> PenNode {
    if matches!(
        node_width(&node),
        Some(SizingBehavior::Keyword(SizingKeyword::FillContainer)) | None
    ) {
        return node;
    }
    if !reserve_node(context, ImportWarning::AutoMarginNodeLimit) {
        return node;
    }
    let container = ContainerProps {
        width: Some(SizingBehavior::Keyword(SizingKeyword::FillContainer)),
        height: Some(SizingBehavior::Keyword(SizingKeyword::FitContent)),
        layout: Some(LayoutMode::Horizontal),
        justify_content: Some(align.justify()),
        ..Default::default()
    };
    wrap(context, node, "Auto margin", container)
}

/// Re-parent `node` into a fixed-size wrapper and shift it by `offset`.
/// The wrapper keeps the node's box in flow; the child moves inside it.
///
/// `reserved` is the element's border-box size BEFORE a `transform: scale()`
/// was baked onto the node. CSS scaling is a paint-time operation that never
/// changes what the element occupies in flow, so the wrapper must reserve the
/// unscaled box or the scaled child pushes its siblings around.
pub(super) fn wrap_offset(
    context: &mut MapCtx<'_>,
    node: PenNode,
    offset: (f64, f64),
    reserved: Option<(f64, f64)>,
) -> PenNode {
    if offset.0 == 0.0 && offset.1 == 0.0 {
        return node;
    }
    let base = node_base(&node);
    if base.x.is_some() || base.y.is_some() {
        // Already out of flow: the caller folded the offset into x/y.
        return node;
    }
    let reserved = match reserved.or_else(|| numeric_box(&node)) {
        Some(reserved) => reserved,
        None => {
            context.warn_once(ImportWarning::FlowOffsetNoDefiniteSize);
            return node;
        }
    };
    if !reserve_node(context, ImportWarning::FlowOffsetNodeLimit) {
        return node;
    }
    context.warn_once(ImportWarning::FlowOffsetApproximated);
    let container = ContainerProps {
        width: Some(SizingBehavior::Number(reserved.0)),
        height: Some(SizingBehavior::Number(reserved.1)),
        layout: Some(LayoutMode::Vertical),
        ..Default::default()
    };
    let mut node = node;
    {
        let base = node_base_mut(&mut node);
        base.x = Some(offset.0);
        base.y = Some(offset.1);
    }
    wrap(context, node, "Offset", container)
}

/// The node's own definite border-box size, used when the caller has no
/// pre-scale box to hand (replaced elements, nodes with no baked scale).
fn numeric_box(node: &PenNode) -> Option<(f64, f64)> {
    match (node_width(node), node_height(node)) {
        (Some(SizingBehavior::Number(width)), Some(SizingBehavior::Number(height))) => {
            Some((*width, *height))
        }
        _ => None,
    }
}

/// Common tail: build the wrapper frame. The wrapper becomes the node the
/// parent's post-passes see, so the private carriers those passes consume —
/// the z-index stacking hint and the grid-placement hint — move up with it.
///
/// `base.explain` currently carries nothing but those private hints, so moving
/// it wholesale is safe. If author-facing text ever lands in `explain`, this
/// has to split the hint prefix out and leave the prose on the inner node.
fn wrap(
    context: &mut MapCtx<'_>,
    mut node: PenNode,
    name: &str,
    container: ContainerProps,
) -> PenNode {
    let (explain, theme) = {
        let base = node_base_mut(&mut node);
        (base.explain.take(), base.theme.take())
    };
    PenNode::Frame(FrameNode {
        base: PenNodeBase {
            id: context.generate_id(),
            name: Some(name.to_string()),
            explain,
            theme,
            ..Default::default()
        },
        container,
        children: Some(vec![node]),
        image_search_query: None,
        reusable: None,
        slot: None,
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        screen: None,
        breakpoint: None,
    })
}

fn reserve_node(context: &mut MapCtx<'_>, exhausted: ImportWarning) -> bool {
    if context.node_count.saturating_add(1) > crate::MAX_OUTPUT_NODES {
        context.warn_once(exhausted);
        return false;
    }
    context.node_count += 1;
    true
}
