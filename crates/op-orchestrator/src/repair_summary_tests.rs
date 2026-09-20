use super::{CheckCategory, CountingSink, RepairCounter, RepairSummary};
use crate::test_support::VecDocSink;
use crate::types::DocSink;
use jian_ops_schema::node::PenNode;
use op_editor_core::{EditorCommand, NodeId, PenNodeExt};

fn frame(id: &str, name: &str) -> PenNode {
    serde_json::from_value(serde_json::json!({
        "type": "frame",
        "id": id,
        "name": name,
        "width": 100.0,
        "height": 100.0,
        "children": [],
    }))
    .expect("frame fixture")
}

#[test]
fn record_marks_category_checked_even_with_zero_repairs() {
    let mut summary = RepairSummary::default();
    summary.record(CheckCategory::Overflow, 0);

    assert!(
        !summary.is_empty(),
        "a zero-repair check still counts as run"
    );
    assert_eq!(summary.checked(), vec![CheckCategory::Overflow]);
    assert_eq!(summary.total_repairs(), 0);
    assert!(
        summary.repaired().is_empty(),
        "a clean category must not appear in the repaired list"
    );
}

#[test]
fn record_accumulates_per_category_and_orders_by_display_order() {
    let mut summary = RepairSummary::default();
    summary.record(CheckCategory::Structure, 2);
    summary.record(CheckCategory::Layout, 1);
    summary.record(CheckCategory::Structure, 3);
    summary.record(CheckCategory::Palette, 0);

    assert_eq!(summary.repairs_for(CheckCategory::Structure), 5);
    assert_eq!(summary.repairs_for(CheckCategory::Layout), 1);
    assert_eq!(summary.repairs_for(CheckCategory::Palette), 0);
    assert_eq!(summary.total_repairs(), 6);
    assert_eq!(
        summary.checked(),
        vec![
            CheckCategory::Layout,
            CheckCategory::Structure,
            CheckCategory::Palette,
        ],
        "checked categories come back in CheckCategory::ALL display order"
    );
    assert_eq!(
        summary.repaired(),
        vec![(CheckCategory::Layout, 1), (CheckCategory::Structure, 5)],
    );
}

#[test]
fn merge_folds_counts_and_checked_flags() {
    let mut a = RepairSummary::default();
    a.record(CheckCategory::Layout, 2);
    let mut b = RepairSummary::default();
    b.record(CheckCategory::Layout, 1);
    b.record(CheckCategory::Overflow, 0);

    a.merge(&b);

    assert_eq!(a.repairs_for(CheckCategory::Layout), 3);
    assert_eq!(a.repairs_for(CheckCategory::Overflow), 0);
    assert_eq!(
        a.checked(),
        vec![CheckCategory::Layout, CheckCategory::Overflow]
    );
}

#[test]
fn empty_summary_reports_nothing_checked() {
    let summary = RepairSummary::default();
    assert!(summary.is_empty());
    assert!(summary.checked().is_empty());
    assert_eq!(summary.total_repairs(), 0);
}

#[test]
fn category_keys_round_trip() {
    for category in CheckCategory::ALL {
        assert_eq!(CheckCategory::from_key(category.key()), Some(category));
    }
    assert_eq!(CheckCategory::from_key("not-a-category"), None);
}

#[test]
fn counting_sink_counts_only_accepted_applies() {
    let mut base = VecDocSink::new();
    let counter = RepairCounter::new();
    let mut summary = RepairSummary::default();
    let mut counter = counter;
    {
        let mut sink: CountingSink<'_> = counter.wrap(&mut base);
        assert!(sink.apply(EditorCommand::InsertSubtree {
            nodes: vec![frame("root", "Root")],
            parent_id: NodeId::NONE,
            page_id: None,
        }));
        // Rejected: no such node, so the sink refuses and nothing is counted.
        assert!(!sink.apply(EditorCommand::DeleteNode {
            node_id: NodeId::new("missing".to_string()),
            page_id: None,
        }));
    }
    counter.checkpoint(&mut summary, CheckCategory::Structure, "test-structure");

    assert_eq!(
        summary.repairs_for(CheckCategory::Structure),
        1,
        "a refused command is not a repair"
    );
}

#[test]
fn checkpoints_attribute_only_the_edits_since_the_previous_one() {
    let mut base = VecDocSink::new();
    let mut counter = RepairCounter::new();
    let mut summary = RepairSummary::default();
    {
        let mut sink = counter.wrap(&mut base);
        sink.apply(EditorCommand::InsertSubtree {
            nodes: vec![frame("root", "Root")],
            parent_id: NodeId::NONE,
            page_id: None,
        });
    }
    counter.checkpoint(&mut summary, CheckCategory::Structure, "test-structure");
    // `InsertSubtree` remaps ids, so target the root by the id it actually
    // landed under rather than the fixture's authored one.
    let root_id = base.state.active_children()[0].id_str().to_string();
    {
        let mut sink = counter.wrap(&mut base);
        sink.apply(EditorCommand::InsertSubtree {
            nodes: vec![frame("child", "Child")],
            parent_id: NodeId::new(root_id),
            page_id: None,
        });
    }
    counter.checkpoint(&mut summary, CheckCategory::Layout, "test-layout");

    assert_eq!(summary.repairs_for(CheckCategory::Structure), 1);
    assert_eq!(
        summary.repairs_for(CheckCategory::Layout),
        1,
        "the second checkpoint must not re-count the first checkpoint's edits"
    );
}

#[test]
fn counting_sink_forwards_real_remapped_root_ids() {
    let mut base = VecDocSink::new();
    let counter = RepairCounter::new();
    let ids = {
        let mut sink = counter.wrap(&mut base);
        sink.insert_subtree_returning_root_ids(vec![frame("root", "Root")], &NodeId::NONE)
    };
    // The default trait impl returns `Some(vec![])`; an immediate-apply sink
    // returns the REAL post-remap ids. A non-empty list is the proof that the
    // wrapper delegated instead of falling through to the default.
    let ids = ids.expect("insert accepted");
    assert_eq!(ids.len(), 1, "one root went in, one id must come back");
    assert_eq!(
        ids,
        base.state
            .active_children()
            .iter()
            .map(|n| n.id_str().to_string())
            .collect::<Vec<_>>(),
        "the ids handed back must be the ones the document actually holds"
    );
}
