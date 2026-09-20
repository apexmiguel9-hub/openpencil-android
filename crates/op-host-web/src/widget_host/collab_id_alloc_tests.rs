//! Every web creation path mints from the session allocator while a
//! collaboration session owns the id space, and reports an exhausted
//! namespace instead of half-applying the gesture.

use super::WidgetHost;
use op_editor_core::{
    CollabNoticeKind, CollabRejectUiCode, NodeId, PeerNamespace, PenNodeExt, Tool,
};
use op_editor_ui::widgets::layer_context_menu::LayerContextAction;

const VIEWPORT_W: f32 = 1200.0;
const VIEWPORT_H: f32 = 800.0;

fn collaborating_host(namespace: &str) -> WidgetHost {
    let mut host = WidgetHost::new();
    host.editor_state.active_children_mut().clear();
    host.editor_state.clear_selection();
    host.editor_state_dirty = true;
    host.enable_collaboration_ids(PeerNamespace::try_from(namespace).unwrap())
        .expect("fresh document accepts the namespace");
    host
}

/// Every id in the active page, in tree order.
fn page_ids(host: &WidgetHost) -> Vec<String> {
    host.editor_state
        .active_children()
        .iter()
        .map(|node| node.id_str().to_string())
        .collect()
}

fn drag_create_rect(host: &mut WidgetHost) -> NodeId {
    host.editor_state.tool = Tool::Rect;
    let (cx0, cy0, _, _) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    assert!(host.apply_press(cx0 + 100.0, cy0 + 100.0, VIEWPORT_W, VIEWPORT_H));
    assert!(host.apply_cursor_move(cx0 + 200.0, cy0 + 180.0));
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    host.editor_state.selection.anchor.clone()
}

#[test]
fn drag_create_group_duplicate_and_paste_share_the_session_namespace() {
    let mut host = collaborating_host("peer-a");

    let rect = drag_create_rect(&mut host);
    assert_eq!(rect.as_str(), "c_peer-a_0");

    assert!(host.apply_group());
    assert_eq!(host.editor_state.selection.anchor.as_str(), "c_peer-a_1");

    assert!(host.apply_duplicate());
    assert_eq!(host.editor_state.selection.anchor.as_str(), "c_peer-a_2");

    assert!(host.editor_state.copy_selected());
    assert!(host.apply_paste());
    assert!(page_ids(&host).iter().all(|id| id.starts_with("c_peer-a_")));
}

#[test]
fn alt_drag_clone_mints_from_the_session_namespace() {
    let mut host = collaborating_host("peer-b");
    let rect = drag_create_rect(&mut host);
    host.editor_state.set_single_selection(rect);

    host.alt_held = true;
    host.start_node_drag(400.0, 300.0);
    assert_eq!(
        host.apply_node_drag_cursor_move(460.0, 300.0),
        Some(true),
        "the option-drag clone lands on the first move past the threshold"
    );

    let ids = page_ids(&host);
    assert_eq!(ids.len(), 2, "the clone joined the original: {ids:?}");
    assert!(ids.iter().all(|id| id.starts_with("c_peer-b_")));
}

#[test]
fn layer_context_duplicate_mints_from_the_session_namespace() {
    let mut host = collaborating_host("peer-c");
    let rect = drag_create_rect(&mut host);

    host.dispatch_layer_context_action(
        LayerContextAction::Duplicate,
        op_editor_core::ui_draft::LayerContextTarget::Layer(rect),
    );

    let ids = page_ids(&host);
    assert_eq!(ids.len(), 2, "the context-menu clone landed: {ids:?}");
    assert!(ids.iter().all(|id| id.starts_with("c_peer-c_")));
}

#[test]
fn boolean_op_result_path_mints_from_the_session_namespace() {
    let mut host = collaborating_host("peer-d");
    let first = drag_create_rect(&mut host);
    let (cx0, cy0, _, _) = host.canvas_region(VIEWPORT_W, VIEWPORT_H);
    host.editor_state.tool = Tool::Rect;
    assert!(host.apply_press(cx0 + 150.0, cy0 + 140.0, VIEWPORT_W, VIEWPORT_H));
    assert!(host.apply_cursor_move(cx0 + 260.0, cy0 + 240.0));
    assert!(host.apply_release_with_viewport(VIEWPORT_W, VIEWPORT_H));
    host.editor_state.toggle_selection(first);
    assert_eq!(host.editor_state.selection_count(), 2);

    assert!(host.apply_boolean_op(op_editor_core::BooleanOp::Union));
    let ids = page_ids(&host);
    assert_eq!(ids.len(), 1, "the sources collapsed into one path: {ids:?}");
    assert!(ids[0].starts_with("c_peer-d_"));
}

#[cfg(feature = "canvaskit")]
#[test]
fn figma_paste_deep_clone_mints_from_the_session_namespace() {
    let mut host = collaborating_host("peer-e");
    let node: jian_ops_schema::node::PenNode = serde_json::from_value(serde_json::json!({
        "type": "frame",
        "id": "figma-root",
        "name": "Card",
        "x": 0,
        "y": 0,
        "width": 100,
        "height": 60,
        "children": [{
            "type": "rectangle",
            "id": "figma-child",
            "name": "Body",
            "x": 0,
            "y": 0,
            "width": 40,
            "height": 20
        }]
    }))
    .expect("valid pasted node");

    assert!(host.paste_figma_nodes(vec![node], VIEWPORT_W, VIEWPORT_H));
    let root = host.editor_state.active_children().first().unwrap();
    assert!(root.id_str().starts_with("c_peer-e_"));
    let child = root.children().unwrap().first().unwrap();
    assert!(
        child.id_str().starts_with("c_peer-e_"),
        "the whole subtree is remapped: {}",
        child.id_str()
    );
}

#[test]
fn exhausted_namespace_reports_a_notice_without_mutating_the_document() {
    let mut host = WidgetHost::new();
    let doc: jian_ops_schema::PenDocument = serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{
            "type": "rectangle",
            "id": "c_peer_18446744073709551614",
            "name": "Rect",
            "x": 0,
            "y": 0,
            "width": 10,
            "height": 10
        }]
    }))
    .expect("valid test document");
    host.replace_editor_state(op_editor_core::EditorState::from_document(doc));
    // The document's highest peer id leaves exactly one counter value, so
    // the session resumes at `u64::MAX` and the next allocation overflows.
    host.enable_collaboration_ids(PeerNamespace::try_from("peer").unwrap())
        .expect("namespace resumes above the document ids");
    host.editor_state
        .set_single_selection(NodeId::new("c_peer_18446744073709551614"));

    let before = host.editor_state.doc.clone();
    assert!(
        host.apply_duplicate(),
        "the gesture is consumed so it cannot fall through to another handler"
    );
    assert_eq!(&host.editor_state.doc, &before);
    assert!(matches!(
        host.editor_state
            .editor_ui
            .collab
            .notice
            .map(|notice| notice.kind),
        Some(CollabNoticeKind::Reject(CollabRejectUiCode::ResourceLimit))
    ));
}
