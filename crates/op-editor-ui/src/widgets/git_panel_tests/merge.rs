//! Merge mode: conflict rows and the resolution view.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn conflict_rows_open_a_file_diff_in_merge_mode() {
    let s = state_with(GitPanelState {
        merging: true,
        conflicted_files: vec!["a.op".into(), "b.op".into()],
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.list_row_rects(rect);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        panel.hit_test(rect, centre(rows[1])),
        Some(GitPanelHit::ShowFileDiff(1))
    );
}

#[test]
fn merge_resolution_view_maps_choices_and_actions() {
    let conflict = |id: &str| MergeConflictRow {
        id: id.into(),
        label: format!("Node {id}"),
        kind: "both modified".into(),
        theirs_allowed: true,
        take_theirs: false,
    };
    let s = state_with(GitPanelState {
        merge_resolve: Some(MergeResolveState {
            branch: "feature".into(),
            files: vec![MergeResolveFile {
                path: "doc.op".into(),
                base: "{}".into(),
                ours: "{}".into(),
                theirs: "{}".into(),
                conflicts: vec![conflict("n1"), conflict("n2")],
            }],
        }),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let layout = panel.resolve_layout(rect);
    assert_eq!(layout.rows.len(), 2);
    let (ours0, theirs0) = layout.rows[0];
    assert_eq!(
        panel.hit_test(rect, centre(ours0)),
        Some(GitPanelHit::MergeChoiceOurs(0))
    );
    assert_eq!(
        panel.hit_test(rect, centre(theirs0)),
        Some(GitPanelHit::MergeChoiceTheirs(0))
    );
    let (_, theirs1) = layout.rows[1];
    assert_eq!(
        panel.hit_test(rect, centre(theirs1)),
        Some(GitPanelHit::MergeChoiceTheirs(1))
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.apply)),
        Some(GitPanelHit::ApplyMergeResolution)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.cancel)),
        Some(GitPanelHit::CancelMergeResolution)
    );
}

#[test]
fn merge_resolve_set_choice_clamps_structural_to_ours() {
    let mut state = MergeResolveState {
        branch: "feature".into(),
        files: vec![MergeResolveFile {
            path: "doc.op".into(),
            base: "{}".into(),
            ours: "{}".into(),
            theirs: "{}".into(),
            conflicts: vec![
                MergeConflictRow {
                    id: "n1".into(),
                    label: "Node n1".into(),
                    kind: "both modified".into(),
                    theirs_allowed: true,
                    take_theirs: false,
                },
                MergeConflictRow {
                    id: "n2".into(),
                    label: "Node n2".into(),
                    kind: "added on remote".into(),
                    theirs_allowed: false,
                    take_theirs: false,
                },
            ],
        }],
    };
    // A prop conflict honours "theirs".
    state.set_choice(0, true);
    assert!(state.rows()[0].take_theirs);
    // A structural conflict clamps a "theirs" choice back to "ours".
    state.set_choice(1, true);
    assert!(!state.rows()[1].take_theirs);
}
