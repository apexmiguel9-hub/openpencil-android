#![cfg(test)]

use crate::command::EditorCommand;
use crate::components::ComponentLibrary;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::test_support::{frame, group, rect, sample, state_with};
use crate::walkers::find_node;
use jian_ops_schema::node::PenNode;
use jian_ops_schema::page::PenPage;

fn id(s: &str) -> NodeId {
    NodeId::new(s)
}

#[test]
fn create_component_marks_frame_reusable_and_registers_it() {
    let mut s = sample();
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "Hero".into(),
    }));
    let c = s.components.find_by_id(&id("n10")).expect("component");
    assert_eq!(c.name, "Hero");
    assert_eq!(c.root.id_str(), "n10");
    match find_node(s.active_children(), &id("n10")).unwrap() {
        PenNode::Frame(f) => assert_eq!(f.reusable, Some(true)),
        _ => panic!("expected frame"),
    }
    assert!(s.history.can_undo());
}

#[test]
fn create_and_detach_group_component_updates_registry() {
    let mut s = state_with(vec![group(
        "g1",
        "Text Group",
        vec![rect("r1", "Background", 0.0, 0.0, 100.0, 48.0)],
    )]);

    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("g1"),
        name: "Text Group".into(),
    }));
    assert!(s.components.find_by_id(&id("g1")).is_some());

    let survived = s.detach_component(&id("g1")).expect("group detaches");
    assert_eq!(survived, id("g1"));
    assert!(
        s.components.find_by_id(&id("g1")).is_none(),
        "detaching a registered group component removes it from the component registry",
    );
}

#[test]
fn create_component_rejects_non_container_and_blank_name() {
    let mut s = sample();
    assert!(!s.apply(EditorCommand::CreateComponent {
        node_id: id("n11"),
        name: "Text".into(),
    }));
    assert!(!s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "  ".into(),
    }));
    assert!(s.components.is_empty());
}

#[test]
fn instantiate_component_clones_registered_root_with_fresh_ids() {
    let mut s = sample();
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "Hero".into(),
    }));
    assert!(s.apply(EditorCommand::InstantiateComponent {
        component_id: id("n10"),
    }));
    assert_eq!(s.active_children().len(), 2);
    assert!(s.find_duplicate_id().is_none());
    let clone = s.active_children().last().expect("inserted clone");
    assert_ne!(clone.id_str(), "n10");
    assert_eq!(clone.base().name.as_deref(), Some("Hero"));
    assert_eq!(clone.base().x, Some(60.0));
    assert_eq!(clone.base().y, Some(60.0));
    assert_eq!(s.selection.anchor.as_str(), clone.id_str());
    match clone {
        PenNode::Frame(f) => assert_ne!(f.reusable, Some(true)),
        _ => panic!("expected frame clone"),
    }
}

#[test]
fn instantiate_document_component_reads_the_live_master_subtree() {
    let mut s = sample();
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "Hero".into(),
    }));
    s.active_children_mut()[0].set_width_px(444.0);

    let instance_id = s
        .instantiate_component(&id("n10"))
        .expect("live document master instantiates");
    let instance = find_node(s.active_children(), &instance_id).expect("inserted instance");
    assert_eq!(instance.width_px(), Some(444.0));
    assert_eq!(
        instance.children().map(Vec::len),
        Some(2),
        "the metadata-only compatibility root must not truncate instances"
    );
}

#[test]
fn rename_and_delete_component_update_registry_and_source_flag() {
    let mut s = state_with(vec![frame("n1", "Card", 0.0, 0.0, 100.0, 80.0, Vec::new())]);
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n1"),
        name: "Card".into(),
    }));
    assert!(s.apply(EditorCommand::RenameComponent {
        component_id: id("n1"),
        name: "X".into(),
    }));
    assert_eq!(s.components.find_by_id(&id("n1")).unwrap().name, "X");
    assert!(s.apply(EditorCommand::DeleteComponent {
        component_id: id("n1"),
    }));
    assert!(s.components.find_by_id(&id("n1")).is_none());
    match find_node(s.active_children(), &id("n1")).unwrap() {
        PenNode::Frame(f) => assert_eq!(f.reusable, None),
        _ => panic!("expected frame"),
    }
}

#[test]
fn undo_redo_create_component_restores_registry_and_source_flag() {
    let mut s = state_with(vec![frame("n1", "Card", 0.0, 0.0, 100.0, 80.0, Vec::new())]);
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n1"),
        name: "Card".into(),
    }));
    assert!(s.components.find_by_id(&id("n1")).is_some());
    assert!(s.apply(EditorCommand::Undo));
    assert!(s.components.find_by_id(&id("n1")).is_none());
    match find_node(s.active_children(), &id("n1")).unwrap() {
        PenNode::Frame(f) => assert_eq!(f.reusable, None),
        _ => panic!("expected frame"),
    }
    assert!(s.apply(EditorCommand::Redo));
    assert!(s.components.find_by_id(&id("n1")).is_some());
    match find_node(s.active_children(), &id("n1")).unwrap() {
        PenNode::Frame(f) => assert_eq!(f.reusable, Some(true)),
        _ => panic!("expected frame"),
    }
}

#[test]
fn delete_component_clears_source_flag_outside_active_page() {
    let mut s = state_with(vec![]);
    let mut component_frame = frame("n50", "Card", 0.0, 0.0, 100.0, 80.0, Vec::new());
    if let PenNode::Frame(f) = &mut component_frame {
        f.reusable = Some(true);
    }
    s.doc.pages = Some(vec![
        PenPage {
            id: "p1".into(),
            name: "Page 1".into(),
            children: Vec::new(),
            background_color: None,
            state: None,
            lifecycle: None,
        },
        PenPage {
            id: "p2".into(),
            name: "Page 2".into(),
            children: vec![component_frame],
            background_color: None,
            state: None,
            lifecycle: None,
        },
    ]);
    s.ui.active_page_index = 0;
    s.components = ComponentLibrary::from_document(&s.doc);

    assert!(s.apply(EditorCommand::DeleteComponent {
        component_id: id("n50"),
    }));
    match &s.doc.pages.as_ref().unwrap()[1].children[0] {
        PenNode::Frame(f) => assert_eq!(f.reusable, None),
        _ => panic!("expected frame"),
    }
}

#[test]
fn detach_component_clears_reusable_flag_and_registry() {
    let mut s = sample();
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "Hero".into(),
    }));
    assert_eq!(s.components.len(), 1);

    let survived = s.detach_component(&id("n10")).expect("detach succeeds");
    assert_eq!(survived, id("n10"));
    assert!(s.components.is_empty(), "registry entry drops");
    match find_node(s.active_children(), &id("n10")).unwrap() {
        PenNode::Frame(f) => assert_eq!(f.reusable, None, "reusable flag clears"),
        _ => panic!("expected frame"),
    }
    assert!(s.history.can_undo());
}

#[test]
fn detach_instance_materializes_overridden_subtree_with_fresh_ids() {
    let mut s = sample();
    assert!(s.apply(EditorCommand::CreateComponent {
        node_id: id("n10"),
        name: "Hero".into(),
    }));
    // Mint an instance by duplicating the reusable frame.
    s.set_single_selection(id("n10"));
    let mut next = 1000u64;
    let inst_id = s
        .duplicate_selected(&mut next, 10.0)
        .expect("duplicate mints an instance");
    match find_node(s.active_children(), &inst_id).unwrap() {
        PenNode::Ref(r) => assert_eq!(r.target, "n10"),
        other => panic!("expected ref instance, got {other:?}"),
    }

    let detached = s.detach_component(&inst_id).expect("instance detaches");
    let node = find_node(s.active_children(), &detached).expect("detached tree in place");
    match node {
        PenNode::Frame(f) => {
            assert_eq!(f.reusable, None, "detached tree is independent");
        }
        other => panic!("expected materialized frame, got {other:?}"),
    }
    assert!(
        find_node(s.active_children(), &inst_id).is_none(),
        "the ref node itself is gone"
    );
}

#[test]
fn set_instance_component_updates_only_the_existing_ref_target_and_is_undoable() {
    let doc = jian_ops_schema::load_str(
        r##"{
          "version":"1.0.0",
          "children":[
            {"type":"frame","id":"card","name":"Card","reusable":true,
             "x":0,"y":0,"width":100,"height":80,"children":[]},
            {"type":"frame","id":"banner","name":"Banner","reusable":true,
             "x":0,"y":100,"width":200,"height":40,"children":[]},
            {"type":"ref","id":"inst","ref":"card","x":300,"y":50,
             "descendants":{"card":{"opacity":0.5}}}
          ]
        }"##,
    )
    .expect("component fixture parses")
    .value;
    let mut state = crate::EditorState::from_document(doc);

    assert!(state.set_instance_component(&id("inst"), &id("banner")));
    let PenNode::Ref(reference) = find_node(state.active_children(), &id("inst")).unwrap() else {
        panic!("instance remains a Ref");
    };
    assert_eq!(reference.target, "banner");
    assert_eq!(reference.base.x, Some(300.0));
    assert!(
        reference.descendants.is_some(),
        "authored overrides are preserved"
    );
    assert!(state.history.can_undo());

    assert!(!state.set_instance_component(&id("inst"), &id("banner")));
    assert!(!state.set_instance_component(&id("inst"), &id("missing")));
    assert!(state.undo());
    let PenNode::Ref(reference) = find_node(state.active_children(), &id("inst")).unwrap() else {
        panic!("undo restores the Ref");
    };
    assert_eq!(reference.target, "card");
}
