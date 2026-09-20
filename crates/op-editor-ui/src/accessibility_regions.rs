//! Editor-region enumeration for the accessibility tree — the half of
//! the a11y bridge both hosts used to spell out identically.
//!
//! [`accessibility`](crate::accessibility) is the generic assembler
//! (widget + rect → `accesskit::TreeUpdate`). This module is the
//! OpenPencil-specific layer above it: it pairs each always-present
//! editor region with the rect the paint pass places it at, in reading
//! order, and resolves the default focus target. The native
//! (`widget_host/a11y.rs`) and web (`widget_host/a11y_bridge.rs`) hosts
//! now only supply the geometry their own paint pass derived — the
//! canvas band, the floating chat rect, the status pill — and the widget
//! set / ordering / focus rules live here so the two trees cannot drift.
//!
//! Transient overlays (pickers, modals, context menus) are intentionally
//! omitted for v1: they come and go every frame and their
//! `access_node()`s are not yet richly labelled, so adding them would
//! churn the tree without improving navigability.

use op_editor_core::EditorState;

use crate::accessibility::{assemble_tree_update, PlacedWidget};
use crate::layout_scene::LayoutScene;
use crate::widgets::{
    AIChatPlaceholder, CanvasViewport, LayerPanel, LayoutCx, PropertyPanel, StatusBar, Toolbar,
    TopBar, Widget, WidgetId, ROOT_WIDGET_ID, TOOLBAR_WIDTH, TOP_BAR_HEIGHT,
};
use crate::{Point2D, Rect};

/// Stable widget ids of the always-present regions, mirrored from each
/// widget's constructor (`WidgetId::new(..)`). Used for focus targeting
/// and action routing without constructing the widget twice.
pub const AI_CHAT_WIDGET_ID: u64 = 7000;
/// Property-panel region id.
pub const PROPERTY_PANEL_WIDGET_ID: u64 = 2000;
/// Canvas region id.
pub const CANVAS_WIDGET_ID: u64 = 4000;
/// Toolbar region id.
pub const TOOLBAR_WIDGET_ID: u64 = 3000;

/// Toolbar layout is dpi-independent (fixed button metrics), so a 1.0
/// scale is exact for the a11y pass; the real value is only threaded
/// through paint for parity.
const TOOLBAR_LAYOUT_DPI: f32 = 1.0;

/// Host-resolved placement of the floating / band regions.
///
/// The fixed rails (top bar, layer panel, property panel) are derived
/// here from `EditorState` — both hosts computed them from the same two
/// fields — while the canvas band and the two floating panels come from
/// the host helpers paint uses, so the a11y tree and the painted frame
/// never drift.
#[derive(Debug, Clone, Copy)]
pub struct RegionPlacement {
    /// Logical viewport width.
    pub viewport_width: f32,
    /// Logical viewport height.
    pub viewport_height: f32,
    /// Canvas band left edge (`canvas_region().0`).
    pub canvas_left: f32,
    /// Canvas band width (`canvas_region().2`).
    pub canvas_width: f32,
    /// Canvas band height (`canvas_region().3`).
    pub canvas_height: f32,
    /// Floating chat panel rect, `None` when it does not fit.
    pub ai_chat_rect: Option<Rect>,
    /// Floating status pill rect, `None` when the canvas is too narrow.
    pub status_bar_rect: Option<Rect>,
    /// Host toolbar inset from the canvas band's top-left corner (the
    /// hosts own these constants, so they stay the single source of
    /// truth rather than being re-spelled here).
    pub toolbar_inset_x: f32,
    /// Vertical half of [`Self::toolbar_inset_x`].
    pub toolbar_inset_y: f32,
}

/// Assemble the accessibility tree for the current editor frame.
///
/// Hosts call this on the same cadence they paint (initial publish +
/// every dirty frame); the assembler suppresses no-op events on the
/// adapter side. `layer_panel` is passed in because each host owns its
/// own row-ownership seed for the panel widget.
pub fn editor_tree_update(
    state: &EditorState,
    scene: &LayoutScene,
    layer_panel: &LayerPanel,
    now_ms: u64,
    placement: RegionPlacement,
) -> accesskit::TreeUpdate {
    let RegionPlacement {
        viewport_width,
        viewport_height,
        canvas_left,
        canvas_width,
        canvas_height,
        ai_chat_rect,
        status_bar_rect,
        toolbar_inset_x,
        toolbar_inset_y,
    } = placement;

    let window_bounds = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(viewport_width, viewport_height),
    };
    let ui = &state.editor_ui;

    // 1. TopBar — full-width top strip. Built without the host's
    // traffic-control tweak on purpose: `TopBar::access_node()` is a
    // fixed `Header` / "Title bar" node, so the platform chrome flag
    // cannot change what the tree reports.
    let top_bar = TopBar::for_editor_ui(ui);
    let top_bar_rect = Rect {
        origin: Point2D::new(0.0, 0.0),
        size: Point2D::new(viewport_width, TOP_BAR_HEIGHT),
    };

    // 2. LayerPanel — left rail, only when the sidebar is open.
    let layer_panel_rect = Rect {
        origin: Point2D::new(0.0, TOP_BAR_HEIGHT),
        size: Point2D::new(
            ui.layer_panel_width,
            (viewport_height - TOP_BAR_HEIGHT).max(0.0),
        ),
    };

    // 3. CanvasViewport — middle band (sidebar / right-rail aware).
    let canvas = CanvasViewport::from_editor(state, scene);
    let canvas_rect = Rect {
        origin: Point2D::new(canvas_left, TOP_BAR_HEIGHT),
        size: Point2D::new(canvas_width, canvas_height),
    };

    // 4. PropertyPanel — right rail, only with a selection.
    let property_panel = PropertyPanel::for_selection_at(state, now_ms);
    let property_rect = Rect {
        origin: Point2D::new(viewport_width - ui.property_panel_width, TOP_BAR_HEIGHT),
        size: Point2D::new(
            ui.property_panel_width,
            (viewport_height - TOP_BAR_HEIGHT).max(0.0),
        ),
    };

    // 5. Toolbar — floating vertical column over the canvas.
    let toolbar = Toolbar::for_editor(state);
    let toolbar_h = toolbar
        .layout(&LayoutCx {
            available_width: TOOLBAR_WIDTH,
            dpi: TOOLBAR_LAYOUT_DPI,
        })
        .rect
        .size
        .y;
    let toolbar_rect = Rect {
        origin: Point2D::new(
            canvas_left + toolbar_inset_x,
            TOP_BAR_HEIGHT + toolbar_inset_y,
        ),
        size: Point2D::new(TOOLBAR_WIDTH, toolbar_h),
    };
    let toolbar_visible = canvas_width > TOOLBAR_WIDTH + toolbar_inset_x * 2.0;

    // 6. AIChatPlaceholder — floating chat panel.
    let chat = AIChatPlaceholder::from_editor_at(state, now_ms);

    // 7. StatusBar — floating bottom-right zoom pill.
    let status = StatusBar::for_editor(state);

    // Assemble the ordered, present set. Order = reading order.
    let mut placed: Vec<PlacedWidget<'_>> = Vec::with_capacity(8);
    placed.push(PlacedWidget::new(&top_bar, top_bar_rect));
    if ui.sidebar_open {
        placed.push(PlacedWidget::new(layer_panel, layer_panel_rect));
    }
    if canvas_width > 0.0 && canvas_height > 0.0 {
        placed.push(PlacedWidget::new(&canvas, canvas_rect));
    }
    if let Some(panel) = property_panel.as_ref() {
        placed.push(PlacedWidget::new(panel, property_rect));
    }
    if toolbar_visible {
        placed.push(PlacedWidget::new(&toolbar, toolbar_rect));
    }
    if let Some(rect) = ai_chat_rect {
        placed.push(PlacedWidget::new(&chat, rect));
    }
    if let Some(rect) = status_bar_rect {
        placed.push(PlacedWidget::new(&status, rect));
    }

    let focus = focus_target(state, canvas_width, canvas_height, property_panel.is_some());
    assemble_tree_update(window_bounds, &placed, focus)
}

/// Pick a sensible default focus target for the a11y tree.
///
/// Order: focused chat input → property panel (when an editable
/// selection is up) → canvas (the editor's primary work surface) → root.
/// The chosen id must be a region actually present this frame, which the
/// assembler re-checks before emitting.
pub fn focus_target(
    state: &EditorState,
    canvas_width: f32,
    canvas_height: f32,
    property_panel_present: bool,
) -> WidgetId {
    if state.chat.focused {
        return WidgetId::new(AI_CHAT_WIDGET_ID);
    }
    if property_panel_present && state.ui.property_focus.is_some() {
        return WidgetId::new(PROPERTY_PANEL_WIDGET_ID);
    }
    if canvas_width > 0.0 && canvas_height > 0.0 {
        return WidgetId::new(CANVAS_WIDGET_ID);
    }
    ROOT_WIDGET_ID
}

/// Route an accessibility action targeting a known editor region back
/// into editor state. Returns `true` when state changed (so the host
/// repaints + re-publishes the tree).
///
/// `target` is the raw `accesskit::NodeId.0` (== `WidgetId.0`), and
/// `is_focus` distinguishes a focus request from a click / default
/// activation. v1 handles focusing the chat input (and activating it)
/// plus blurring the chat when focus moves to the canvas / a panel.
pub fn apply_region_action(
    state: &mut EditorState,
    target: u64,
    is_focus: bool,
    now_ms: u64,
) -> bool {
    match target {
        // AIChat panel — focus or click/default both focus + ready the
        // chat input (mirrors `click.rs` `AIChatHit::FocusInput`).
        AI_CHAT_WIDGET_ID => {
            focus_chat_input(state, now_ms);
            true
        }
        // Canvas / Toolbar / Property panel — moving a11y focus off the
        // chat blurs the chat input so caret + send routing follow the
        // screen reader's focus. Only meaningful when the chat holds it.
        CANVAS_WIDGET_ID | TOOLBAR_WIDGET_ID | PROPERTY_PANEL_WIDGET_ID if is_focus => {
            if state.chat.focused {
                state.chat.focused = false;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Focus the chat input from the hidden a11y mirror — mirrors
/// `widget_host/click.rs` `AIChatHit::FocusInput` (focus + clear stale
/// selections). The caret-blink anchor rides along through
/// `focus_input_at_end`, so the painted caret restarts its phase like a
/// real click; callers push `now_ms` first.
pub fn focus_chat_input(state: &mut EditorState, now_ms: u64) {
    state.chat.focus_input_at_end(now_ms);
    state.chat.transcript_selection = None;
}

/// Activate a tool from the hidden a11y toolbar — mirrors the painted
/// toolbar's `ToolbarHit::Tool` arm (tool write + shape-picker close).
pub fn set_tool(state: &mut EditorState, tool: op_editor_core::Tool) {
    state.tool = tool;
    state.editor_ui.shape_picker.open = false;
    state.editor_ui.shape_picker.hover = None;
    state.editor_ui.shape_picker.pressed = None;
}
