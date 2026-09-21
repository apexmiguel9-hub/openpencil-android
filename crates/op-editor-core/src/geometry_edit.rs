use std::collections::HashSet;

use jian_ops_schema::node::PenNode;
use crate::node_id::NodeId;
use crate::path_edit;
use crate::render_backend::Point2D;
use crate::state::EditorState;
use crate::viewport::Viewport;

/// The geometry/vertex edit interaction state for path objects.
///
/// When a user double-taps a Path node, the editor enters
/// geometry edit mode and creates a `GeometryEditSession`.
/// This session tracks which anchors, handles, and segments
/// are selected, hovered, or being dragged.
///
/// The session also owns the drag-bridge: when a drag begins
/// it pushes exactly one history snapshot, and subsequent
/// drag-move calls mutate `PenNode::Path::anchors` through
/// `path_edit::edit_anchor()` / `move_path_anchor_handle_ts()`.
#[derive(Debug, Clone)]
pub struct GeometryEditSession {
    /// The node IDs being edited (normally exactly one).
    pub edited_node_ids: HashSet<String>,
    /// Indices of selected anchors into `path_anchors`.
    pub selected_anchor_indices: HashSet<usize>,
    /// The segment index currently selected (between anchor[i] and anchor[i+1]).
    pub selected_segment: Option<usize>,
    /// The anchor index currently being dragged, if any.
    pub dragged_anchor: Option<usize>,
    /// The handle being dragged, if any.
    pub dragged_handle: Option<DraggedHandle>,
    /// The anchor index currently hovered, if any.
    pub hovered_anchor: Option<usize>,
    /// The handle side currently hovered, if any.
    pub hovered_handle: Option<DraggedHandle>,
    /// The segment index currently hovered, if any.
    pub hovered_segment: Option<usize>,
    /// Screen position where the current drag began (for delta computation).
    pub(crate) drag_start_screen: Option<Point2D>,
    /// Document-space position of the anchor at drag begin.
    pub(crate) drag_start_doc: Option<Point2D>,
}

/// Which handle side is being interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraggedHandle {
    /// The incoming control handle of anchor `idx`.
    In(usize),
    /// The outgoing control handle of anchor `idx`.
    Out(usize),
}

impl DraggedHandle {
    /// Return the anchor index this handle belongs to.
    pub fn anchor_idx(&self) -> usize {
        match self {
            DraggedHandle::In(i) => *i,
            DraggedHandle::Out(i) => *i,
        }
    }

    /// Return `true` if this is the incoming handle.
    pub fn is_in(&self) -> bool {
        matches!(self, DraggedHandle::In(_))
    }

    /// Return `true` if this is the outgoing handle.
    pub fn is_out(&self) -> bool {
        matches!(self, DraggedHandle::Out(_))
    }
}

impl Default for GeometryEditSession {
    fn default() -> Self {
        Self {
            edited_node_ids: HashSet::new(),
            selected_anchor_indices: HashSet::new(),
            selected_segment: None,
            dragged_anchor: None,
            dragged_handle: None,
            hovered_anchor: None,
            hovered_handle: None,
            hovered_segment: None,
            drag_start_screen: None,
            drag_start_doc: None,
        }
    }
}

impl GeometryEditSession {
    /// Create a new session editing the given node ID.
    pub fn for_node(node_id: impl Into<String>) -> Self {
        let mut session = Self::default();
        session.edited_node_ids.insert(node_id.into());
        session
    }

    /// Return `true` if we are currently editing the given node.
    pub fn is_editing(&self, node_id: &str) -> bool {
        self.edited_node_ids.contains(node_id)
    }

    /// Enter geometry edit mode for the given node.
    pub fn enter(&mut self, node_id: impl Into<String>) {
        self.edited_node_ids.clear();
        self.edited_node_ids.insert(node_id.into());
        self.deselect_all();
    }

    /// Exit geometry edit mode. Clear all selection.
    pub fn exit(&mut self) {
        self.deselect_all();
        self.edited_node_ids.clear();
    }

    /// Return `true` if geometry edit mode is active.
    pub fn is_active(&self) -> bool {
        !self.edited_node_ids.is_empty()
    }

    /// Deselect all anchors and segments.
    pub fn deselect_all(&mut self) {
        self.selected_anchor_indices.clear();
        self.selected_segment = None;
        self.dragged_anchor = None;
        self.dragged_handle = None;
        self.drag_start_screen = None;
        self.drag_start_doc = None;
    }

    /// Toggle anchor selection. Returns `true` if now selected.
    pub fn toggle_anchor(&mut self, idx: usize) -> bool {
        if self.selected_anchor_indices.contains(&idx) {
            self.selected_anchor_indices.remove(&idx);
            false
        } else {
            self.selected_anchor_indices.insert(idx);
            true
        }
    }

    /// Select a single anchor, deselecting all others.
    pub fn select_anchor(&mut self, idx: usize) {
        self.selected_anchor_indices.clear();
        self.selected_anchor_indices.insert(idx);
        self.selected_segment = None;
    }

    /// Select a segment, deselecting all anchors.
    pub fn select_segment(&mut self, idx: usize) {
        self.selected_anchor_indices.clear();
        self.selected_segment = Some(idx);
    }

    /// Add an anchor to the current selection (multi-select).
    pub fn add_to_selection(&mut self, idx: usize) {
        self.selected_anchor_indices.insert(idx);
    }

    /// Remove an anchor from the current selection.
    pub fn remove_from_selection(&mut self, idx: usize) {
        self.selected_anchor_indices.remove(&idx);
    }

    /// Return `true` if the given anchor is selected.
    pub fn is_anchor_selected(&self, idx: usize) -> bool {
        self.selected_anchor_indices.contains(&idx)
    }

    /// Return the number of selected anchors.
    pub fn selection_count(&self) -> usize {
        self.selected_anchor_indices.len()
    }

    // ── Drag bridge ────────────────────────────────────────────────

    /// Begin dragging an anchor. Pushes exactly one history snapshot.
    ///
    /// Call this ONCE at drag-begin. Subsequent `move_anchor_drag`
    /// calls mutate the document without pushing additional history.
    ///
    /// Returns `true` if the history snapshot was pushed and the
    /// drag state was initialized.
    pub fn begin_anchor_drag(
        &mut self,
        state: &mut EditorState,
        node_id: &str,
        idx: usize,
        screen_point: Point2D,
    ) -> bool {
        let node_id = NodeId::new_opt(node_id);
        let Some(node_id) = node_id else { return false };
        if !state.is_editable(&node_id) {
            return false;
        }
        let Some(node) = crate::walkers::find_node(state.active_children(), &node_id) else {
            return false;
        };
        let PenNode::Path(path) = &node else { return false };
        let anchors = match path.anchors.as_ref() {
            Some(a) => a,
            None => return false,
        };
        if idx >= anchors.len() {
            return false;
        }
        let anchor = &anchors[idx];
        let doc_pos = Point2D::new(anchor.x as f32, anchor.y as f32);

        // Push exactly one history snapshot for the entire drag.
        let snapshot = state.snapshot_for_history();
        state.history_push_past(snapshot);

        self.dragged_anchor = Some(idx);
        self.drag_start_screen = Some(screen_point);
        self.drag_start_doc = Some(doc_pos);
        true
    }

    /// Move the dragged anchor by a screen-space delta.
    ///
    /// Converts `screen_delta` to document space using the viewport
    /// zoom, then applies it to `PenNode::Path::anchors[idx]` via
    /// `path_edit::edit_anchor()`. Does NOT push history — the
    /// snapshot was already pushed by `begin_anchor_drag`.
    ///
    /// `screen_delta` is the pixel movement since the last call
    /// (or since drag-begin). Use the total displacement from
    /// drag-begin for correct behavior.
    pub fn move_anchor_drag(
        &mut self,
        state: &mut EditorState,
        node_id: &str,
        idx: usize,
        screen_delta: Point2D,
        viewport: Viewport,
    ) -> bool {
        if self.dragged_anchor != Some(idx) {
            return false;
        }
        // Convert screen delta to document delta.
        // Since `to_document` is affine, delta_doc = delta_screen / zoom.
        let zoom = viewport.zoom.max(0.0001);
        let doc_delta = Point2D::new(screen_delta.x / zoom, screen_delta.y / zoom);

        let node_id = match NodeId::new_opt(node_id) {
            Some(id) => id,
            None => return false,
        };
        path_edit::edit_anchor(state, &node_id, idx, |anchors, i, _| {
            anchors[i].x = (anchors[i].x as f32 + doc_delta.x) as f64;
            anchors[i].y = (anchors[i].y as f32 + doc_delta.y) as f64;
        })
    }

    /// End the anchor drag. Clears drag state.
    pub fn end_anchor_drag(&mut self) {
        self.dragged_anchor = None;
        self.drag_start_screen = None;
        self.drag_start_doc = None;
    }

    /// Begin dragging a handle. Pushes exactly one history snapshot.
    ///
    /// Call this ONCE at drag-begin. Subsequent `move_handle_drag`
    /// calls mutate the document without pushing additional history.
    pub fn begin_handle_drag(
        &mut self,
        state: &mut EditorState,
        node_id: &str,
        anchor_idx: usize,
        side: DraggedHandle,
        screen_point: Point2D,
    ) -> bool {
        let node_id = NodeId::new_opt(node_id);
        let Some(node_id) = node_id else { return false };
        if !state.is_editable(&node_id) {
            return false;
        }
        let Some(node) = crate::walkers::find_node(state.active_children(), &node_id) else {
            return false;
        };
        let PenNode::Path(path) = &node else { return false };
        let anchors = match path.anchors.as_ref() {
            Some(a) => a,
            None => return false,
        };
        if anchor_idx >= anchors.len() {
            return false;
        }
        let anchor = &anchors[anchor_idx];

        // Determine which handle position to track for delta.
        let handle_pos = if side.is_in() {
            anchor.handle_in.as_ref().map(|h| Point2D::new(h.x as f32, h.y as f32))
        } else {
            anchor.handle_out.as_ref().map(|h| Point2D::new(h.x as f32, h.y as f32))
        };

        // Push exactly one history snapshot for the entire drag.
        let snapshot = state.snapshot_for_history();
        state.history_push_past(snapshot);

        self.dragged_handle = Some(side);
        self.drag_start_screen = Some(screen_point);
        self.drag_start_doc = handle_pos;
        true
    }

    /// Move the dragged handle by a screen-space delta.
    ///
    /// Converts `screen_delta` to document delta, computes the new
    /// handle offset relative to the anchor, then calls
    /// `move_path_anchor_handle_ts()`. Does NOT push history —
    /// the snapshot was already pushed by `begin_handle_drag`.
    ///
    /// Respects the existing `Mirrored` / `Independent` / `Corner`
    /// logic inside `move_path_anchor_handle_ts()`.
    pub fn move_handle_drag(
        &mut self,
        state: &mut EditorState,
        node_id: &str,
        anchor_idx: usize,
        side: DraggedHandle,
        screen_delta: Point2D,
        viewport: Viewport,
    ) -> bool {
        if self.dragged_handle != Some(side) {
            return false;
        }
        let zoom = viewport.zoom.max(0.0001);
        let doc_delta = Point2D::new(screen_delta.x / zoom, screen_delta.y / zoom);

        // Compute the new handle position in document space.
        // `move_path_anchor_handle_ts` expects the offset from the
        // anchor, not the absolute position.
        let node_id = match NodeId::new_opt(node_id) {
            Some(id) => id,
            None => return false,
        };

        // Get the anchor's position to compute the new offset.
        let Some(node) = crate::walkers::find_node(state.active_children(), &node_id) else {
            return false;
        };
        let PenNode::Path(path) = &node else { return false };
        let anchors = match path.anchors.as_ref() {
            Some(a) => a,
            None => return false,
        };
        if anchor_idx >= anchors.len() {
            return false;
        }
        let anchor = &anchors[anchor_idx];
        let anchor_pos = Point2D::new(anchor.x as f32, anchor.y as f32);

        // Read the current handle offset (or default to anchor position).
        let current_offset = if side.is_in() {
            anchor.handle_in.as_ref().map(|h| Point2D::new(h.x as f32, h.y as f32))
                .unwrap_or(anchor_pos)
        } else {
            anchor.handle_out.as_ref().map(|h| Point2D::new(h.x as f32, h.y as f32))
                .unwrap_or(anchor_pos)
        };

        // New absolute position in doc space.
        let new_absolute = current_offset + doc_delta;
        // Convert to f64 offset from anchor.
        let offset = Point2D::new(
            new_absolute.x - anchor_pos.x,
            new_absolute.y - anchor_pos.y,
        );

        state.move_path_anchor_handle_ts(
            &node_id,
            anchor_idx,
            if side.is_in() {
                crate::pen::PathHandleSide::In
            } else {
                crate::pen::PathHandleSide::Out
            },
            (offset.x as f64, offset.y as f64),
        )
    }

    /// End the handle drag. Clears drag state.
    pub fn end_handle_drag(&mut self) {
        self.dragged_handle = None;
        self.drag_start_screen = None;
        self.drag_start_doc = None;
    }

    /// Return `true` if currently dragging something.
    pub fn is_dragging(&self) -> bool {
        self.dragged_anchor.is_some() || self.dragged_handle.is_some()
    }

    /// Clear hover state.
    pub fn clear_hover(&mut self) {
        self.hovered_anchor = None;
        self.hovered_handle = None;
        self.hovered_segment = None;
    }
}

/// Hit-test result for a geometry edit interaction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GeometryHitTarget {
    /// The cursor is over an anchor point.
    Anchor(usize),
    /// The cursor is over a handle (in or out) of an anchor.
    Handle(DraggedHandle),
    /// The cursor is over a segment between two anchors.
    Segment(usize),
    /// The cursor is over empty space.
    Empty,
}

/// Device-px radii for geometry edit hit-tests.
pub const ANCHOR_RADIUS_PX: f32 = 15.0;
pub const HANDLE_RADIUS_PX: f32 = 10.0;
pub const SEGMENT_BAND_PX: f32 = 8.0;

/// Double-tap detection parameters.
pub const DOUBLE_TAP_TIMEOUT_MS: u64 = 300;
pub const DOUBLE_TAP_RADIUS_PX: f32 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_for_node_creates_active_session() {
        let session = GeometryEditSession::for_node("n1");
        assert!(session.is_active());
        assert!(session.is_editing("n1"));
        assert!(!session.is_editing("n2"));
        assert_eq!(session.edited_node_ids.len(), 1);
    }

    #[test]
    fn exit_clears_all_state() {
        let mut session = GeometryEditSession::for_node("n1");
        session.select_anchor(0);
        session.select_segment(1);
        session.exit();
        assert!(!session.is_active());
        assert!(session.selected_anchor_indices.is_empty());
        assert!(session.selected_segment.is_none());
    }

    #[test]
    fn anchor_selection_works() {
        let mut session = GeometryEditSession::for_node("n1");
        assert!(session.toggle_anchor(0));
        assert!(session.is_anchor_selected(0));
        assert!(!session.toggle_anchor(0));
        assert!(!session.is_anchor_selected(0));
    }

    #[test]
    fn multi_select_works() {
        let mut session = GeometryEditSession::for_node("n1");
        session.select_anchor(0);
        assert_eq!(session.selection_count(), 1);
        session.add_to_selection(1);
        assert_eq!(session.selection_count(), 2);
        session.remove_from_selection(0);
        assert_eq!(session.selection_count(), 1);
    }

    #[test]
    fn drag_state_tracking() {
        let mut session = GeometryEditSession::for_node("n1");
        assert!(!session.is_dragging());
        session.dragged_anchor = Some(0);
        assert!(session.is_dragging());
        session.dragged_anchor = None;
        assert!(!session.is_dragging());
    }

    #[test]
    fn handle_side_accessors() {
        let h_in = DraggedHandle::In(0);
        let h_out = DraggedHandle::Out(0);
        assert!(h_in.is_in());
        assert!(!h_in.is_out());
        assert!(!h_out.is_in());
        assert!(h_out.is_out());
        assert_eq!(h_in.anchor_idx(), 0);
        assert_eq!(h_out.anchor_idx(), 0);
    }

    #[test]
    fn screen_to_doc_delta_conversion() {
        let viewport = Viewport {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 2.0,
        };
        // Screen delta of (10, 10) at zoom 2.0 = doc delta of (5, 5)
        let screen_delta = Point2D::new(10.0, 10.0);
        let zoom = viewport.zoom.max(0.0001);
        let doc_delta = Point2D::new(screen_delta.x / zoom, screen_delta.y / zoom);
        assert!((doc_delta.x - 5.0).abs() < 0.001);
        assert!((doc_delta.y - 5.0).abs() < 0.001);
    }

    #[test]
    fn screen_to_doc_delta_at_zoom_1() {
        let viewport = Viewport::IDENTITY;
        let screen_delta = Point2D::new(20.0, 30.0);
        let zoom = viewport.zoom.max(0.0001);
        let doc_delta = Point2D::new(screen_delta.x / zoom, screen_delta.y / zoom);
        assert!((doc_delta.x - 20.0).abs() < 0.001);
        assert!((doc_delta.y - 30.0).abs() < 0.001);
    }

    #[test]
    fn drag_begin_end_clears_state() {
        let mut session = GeometryEditSession::default();
        session.dragged_anchor = Some(0);
        session.drag_start_screen = Some(Point2D::new(100.0, 200.0));
        session.drag_start_doc = Some(Point2D::new(50.0, 75.0));
        assert!(session.is_dragging());
        session.end_anchor_drag();
        assert!(!session.is_dragging());
        assert!(session.drag_start_screen.is_none());
        assert!(session.drag_start_doc.is_none());
    }
}