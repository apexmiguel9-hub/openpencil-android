//! Integration guard for the GUI end of the `CollabHost` seam.
//!
//! `op-collab-host` runs its own suite against `HeadlessCollabHost`, which has
//! no paint caches at all. These tests keep the real GUI host covered: when the
//! runtime drives `WidgetHostNative` through the trait, dispatch must land on
//! the host's cache-invalidating overrides rather than the plain `EditorState`
//! blanket impl — otherwise a remote install would leave the paint snapshot
//! stale and the canvas would silently show the pre-install document.

use jian_ops_schema::PenDocument;
use op_collab_host::CollabHost;
use op_editor_core::{EditOrigin, PeerNamespace};
use op_host_native::WidgetHostNative;

fn document_with_rect(id: &str) -> PenDocument {
    serde_json::from_value(serde_json::json!({
        "version": "1.0",
        "children": [{
            "type": "rectangle",
            "id": id,
            "name": "Rect",
            "x": 0,
            "y": 0,
            "width": 10,
            "height": 10
        }]
    }))
    .expect("valid test document")
}

/// Generic over `CollabHost` exactly like every runtime method, so the calls
/// below resolve through the same dispatch the runtime uses.
fn install_through_seam(host: &mut impl CollabHost, document: PenDocument, origin: EditOrigin) {
    host.install_collaboration_document(document, origin)
        .expect("install succeeds");
    host.mark_editor_state_dirty();
}

fn toggle_ids_through_seam(host: &mut impl CollabHost, namespace: PeerNamespace) {
    host.enable_collaboration_ids(namespace)
        .expect("namespace allocator builds");
}

#[test]
fn snapshot_install_through_the_host_seam_rotates_the_document_epoch() {
    let mut host = WidgetHostNative::new();
    let epoch = host.document_epoch();

    install_through_seam(
        &mut host,
        document_with_rect("peer-1:1"),
        EditOrigin::Snapshot,
    );

    // The blanket `EditorState` impl cannot rotate the epoch — only the GUI
    // host's override does, so this equality proves the seam reached it.
    assert_eq!(host.document_epoch(), epoch.wrapping_add(1));
    assert_eq!(host.editor_state().doc.children.len(), 1);
}

#[test]
fn remote_commit_through_the_host_seam_keeps_the_document_lifetime() {
    let mut host = WidgetHostNative::new();
    let epoch = host.document_epoch();

    install_through_seam(
        &mut host,
        document_with_rect("peer-1:1"),
        EditOrigin::RemoteCommit,
    );

    assert_eq!(host.document_epoch(), epoch);
    assert_eq!(host.editor_state().doc.children.len(), 1);
}

#[test]
fn collaboration_ids_toggle_through_the_host_seam() {
    let mut host = WidgetHostNative::new();
    assert_eq!(host.collaboration_id_next_counter(), None);

    let namespace = PeerNamespace::parse("peer-1").expect("valid namespace");
    toggle_ids_through_seam(&mut host, namespace);
    assert!(host.collaboration_id_next_counter().is_some());

    CollabHost::disable_collaboration_ids(&mut host);
    assert_eq!(host.collaboration_id_next_counter(), None);
}
