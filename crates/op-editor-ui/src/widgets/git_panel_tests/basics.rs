//! Panel sizing, placeholder rows and coarse action-target mapping.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn closed_panel_yields_none() {
    let s = state_with(GitPanelState::default());
    assert!(GitPanel::for_editor(&s).is_none());
}

#[test]
fn open_panel_height_grows_with_commits() {
    let base = state_with(open_repo());
    let h0 = GitPanel::for_editor(&base).unwrap().height();
    let with_commits = state_with(GitPanelState {
        recent_commits: vec![
            GitCommitSummary {
                short_hash: "abc1234".into(),
                summary: "first".into(),
                author: "Ada".into(),
                time_label: "now".into(),
                is_initial: false,
            };
            3
        ],
        ..open_repo()
    });
    let h3 = GitPanel::for_editor(&with_commits).unwrap().height();
    assert!(h3 > h0, "more commits → taller panel");
}

#[test]
fn empty_history_reserves_a_placeholder_row() {
    let empty = state_with(open_repo());
    let one = state_with(GitPanelState {
        recent_commits: vec![GitCommitSummary {
            short_hash: "abc1234".into(),
            summary: "only".into(),
            author: "Ada".into(),
            time_label: "now".into(),
            is_initial: false,
        }],
        ..open_repo()
    });
    assert_eq!(
        GitPanel::for_editor(&empty).unwrap().height(),
        GitPanel::for_editor(&one).unwrap().height(),
    );
}

#[test]
fn merge_mode_remaps_the_action_buttons() {
    // Conflicts still present — Complete is disabled, so its slot
    // dispatches nothing (a swallowed `Inside`).
    let blocked = state_with(GitPanelState {
        merging: true,
        conflicted_files: vec!["doc.op".to_string()],
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&blocked).unwrap();
    let rect = panel_rect(&panel);
    let rects = GitPanel::action_rects(rect, true);
    // Merge mode: 3 buttons — Abort / Refresh / Complete.
    assert_eq!(rects.buttons.len(), 3);
    assert_eq!(
        panel.hit_test(rect, centre(rects.buttons[0])),
        Some(GitPanelHit::AbortMerge)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rects.buttons[1])),
        Some(GitPanelHit::Refresh)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rects.input)),
        Some(GitPanelHit::Inside)
    );
    // Complete slot — inert while conflicts remain.
    assert_eq!(
        panel.hit_test(rect, centre(rects.buttons[2])),
        Some(GitPanelHit::Inside)
    );

    // Conflicts resolved — Complete becomes actionable.
    let ready = state_with(GitPanelState {
        merging: true,
        conflicted_files: vec![],
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&ready).unwrap();
    let rect = panel_rect(&panel);
    let rects = GitPanel::action_rects(rect, true);
    assert_eq!(
        panel.hit_test(rect, centre(rects.buttons[2])),
        Some(GitPanelHit::CompleteMerge)
    );
}

#[test]
fn non_repo_panel_has_no_action_targets() {
    let s = state_with(GitPanelState {
        open: true,
        in_repo: false,
        ..GitPanelState::default()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    // Any in-bounds click is just swallowed.
    assert_eq!(
        panel.hit_test(rect, Point2D::new(40.0, 40.0)),
        Some(GitPanelHit::Inside)
    );
}

#[test]
fn truncate_caps_long_summaries() {
    assert_eq!(truncate("short", 38), "short");
    let long = "x".repeat(50);
    let t = truncate(&long, 38);
    assert_eq!(t.chars().count(), 38);
    assert!(t.ends_with('…'));
}

// --- Diff view ----------------------------------------------------
