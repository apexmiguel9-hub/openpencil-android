//! Branch picker, overflow menu, tracked picker and remote-settings subview.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn ready_commit_button_is_inert_without_a_message() {
    // An empty commit message → the button is not a commit target;
    // the click falls through to the box's focus instead.
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    assert_eq!(
        panel.hit_test(rect, centre(panel.ready_commit_btn(rect))),
        Some(GitPanelHit::CommitInput)
    );
}

#[test]
fn branch_picker_dropdown_switches_and_dismisses() {
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["feature".to_string(), "main".to_string()],
        branch_picker_open: true,
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.branch_picker_row_rects(rect);
    assert_eq!(rows.len(), 2);
    // Row 0 = feature (not current) → switch; row 1 = main (current) → no-op.
    assert_eq!(
        panel.hit_test(rect, centre(rows[0])),
        Some(GitPanelHit::SwitchBranch(0))
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[1])),
        Some(GitPanelHit::Inside)
    );
    // A click outside the dropdown (but inside the panel) dismisses it.
    let outside = Point2D::new(rect.origin.x + rect.size.x / 2.0, rect.origin.y + 8.0);
    assert_eq!(
        panel.hit_test(rect, outside),
        Some(GitPanelHit::DismissPopover)
    );
    // An open popover is modal: a click FAR OUTSIDE the panel (e.g. on
    // the canvas) also dismisses it rather than returning None (which
    // would leave the popover stuck open).
    let far = Point2D::new(rect.origin.x - 200.0, rect.origin.y + 400.0);
    assert_eq!(panel.hit_test(rect, far), Some(GitPanelHit::DismissPopover));
}

#[test]
fn branch_picker_submodes_map_create_input_and_cancel() {
    let mut create_state = open_repo();
    create_state.branch_create_input.set_text("feature/new");
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["feature".to_string(), "main".to_string()],
        branch_picker_open: true,
        branch_picker_mode: GitBranchPickerMode::Create,
        ..create_state
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let picker = panel.branch_picker_panel(rect);
    let input_point = Point2D::new(picker.origin.x + 16.0, picker.origin.y + 34.0);
    assert_eq!(
        panel.hit_test(rect, input_point),
        Some(GitPanelHit::BranchCreateInput)
    );
    let submit = Rect {
        origin: Point2D::new(
            picker.origin.x + picker.size.x - 18.0 - 64.0,
            picker.origin.y + 54.0,
        ),
        size: Point2D::new(64.0, 24.0),
    };
    assert_eq!(
        panel.hit_test(rect, centre(submit)),
        Some(GitPanelHit::BranchCreateSubmit)
    );

    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["feature".to_string(), "main".to_string()],
        branch_picker_open: true,
        branch_picker_mode: GitBranchPickerMode::Merge,
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let picker = panel.branch_picker_panel(rect);
    let cancel_point = Point2D::new(
        picker.origin.x + picker.size.x / 2.0,
        picker.origin.y + picker.size.y - 12.0,
    );
    assert_eq!(
        panel.hit_test(rect, cancel_point),
        Some(GitPanelHit::BranchPickerCancel)
    );
}

#[test]
fn ready_long_branch_never_eats_the_overflow_button() {
    // A long branch name must not push the pull/push cluster over the
    // right-anchored `…` overflow button (the branch rect is clamped).
    let s = state_with(GitPanelState {
        branch: Some("feature/a-very-long-branch-name-indeed".to_string()),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (_, _, overflow) = panel.ready_header_buttons(rect);
    assert_eq!(
        panel.hit_test(rect, centre(overflow)),
        Some(GitPanelHit::Overflow)
    );
    // The branch button must not overlap the pull button either.
    let branch = panel.ready_branch_rect(rect);
    let (pull, _, _) = panel.ready_header_buttons(rect);
    assert!(
        branch.origin.x + branch.size.x <= pull.origin.x,
        "branch button overruns the pull icon"
    );
}

#[test]
fn overflow_menu_maps_its_entries() {
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        overflow_open: true,
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.overflow_row_rects(rect);
    // TS 5-item menu: switch-tracked / clear-author / remote-settings /
    // ssh-keys / close-repo (with two dividers between groups).
    assert_eq!(rows.len(), 5);
    assert_eq!(
        panel.hit_test(rect, centre(rows[0])),
        Some(GitPanelHit::OverflowSwitchTracked)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[1])),
        Some(GitPanelHit::OverflowClearAuthor)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[2])),
        Some(GitPanelHit::OverflowRemoteSettings)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[3])),
        Some(GitPanelHit::OverflowSshKeys)
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[4])),
        Some(GitPanelHit::OverflowCloseRepo)
    );
}

#[test]
fn git_menus_use_shared_menu_state_protocol() {
    use jian_widgets::components::menu::{MenuHit, MenuState};

    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        overflow_open: true,
        overflow_menu: MenuState { hover: Some(2) },
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.overflow_row_rects(rect);
    assert_eq!(panel.state.overflow_menu.hover, Some(2));
    assert_eq!(
        panel.overflow_menu_hit(rect, centre(rows[2])),
        MenuHit::Row(2)
    );
    assert_eq!(
        panel.overflow_menu_hit(rect, Point2D::new(rows[2].origin.x, rows[2].origin.y - 4.0)),
        MenuHit::Inside
    );

    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["main".to_string(), "feature".to_string()],
        branch_picker_open: true,
        branch_picker_menu: MenuState { hover: Some(1) },
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.branch_picker_row_rects(rect);
    assert_eq!(panel.state.branch_picker_menu.hover, Some(1));
    assert_eq!(
        panel.branch_picker_menu_hit(rect, centre(rows[1])),
        MenuHit::Row(1)
    );
    assert_eq!(
        panel.branch_picker_menu_hit(rect, Point2D::new(rows[0].origin.x, rows[0].origin.y - 4.0)),
        MenuHit::Inside
    );
}

#[test]
fn tracked_picker_maps_rows_and_actions() {
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        overflow_open: true,
        overflow_view: GitOverflowView::TrackedPicker,
        candidate_files: vec![
            GitCandidateFile {
                path: "/r/a.op".into(),
                relative_path: "a.op".into(),
                milestone_count: 2,
                last_commit_time: "1h".into(),
                last_commit_message: Some("hi".into()),
            },
            GitCandidateFile {
                path: "/r/b.op".into(),
                relative_path: "b.op".into(),
                milestone_count: 0,
                last_commit_time: String::new(),
                last_commit_message: None,
            },
        ],
        tracked_picker_selected: Some(0),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.tracked_picker_row_rects(rect);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        panel.hit_test(rect, centre(rows[1])),
        Some(GitPanelHit::TrackedPickerRow(1))
    );
    // With a selection, both bind buttons are live; Back always is.
    let (back, bind, open) = panel.tracked_picker_footer_rects(rect);
    assert_eq!(
        panel.hit_test(rect, centre(back)),
        Some(GitPanelHit::TrackedPickerBack)
    );
    assert_eq!(
        panel.hit_test(rect, centre(bind)),
        Some(GitPanelHit::TrackedPickerBind)
    );
    assert_eq!(
        panel.hit_test(rect, centre(open)),
        Some(GitPanelHit::TrackedPickerBindOpen)
    );
}

#[test]
fn overflow_remote_settings_subview_maps_inputs_and_back() {
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        overflow_open: true,
        overflow_view: GitOverflowView::RemoteSettings,
        remotes: vec!["origin → https://example.com/r.git".to_string()],
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (back, url, set) = panel.remote_settings_rects(rect);
    assert_eq!(
        panel.hit_test(rect, centre(back)),
        Some(GitPanelHit::OverflowBack)
    );
    assert_eq!(
        panel.hit_test(rect, centre(url)),
        Some(GitPanelHit::RemoteInput)
    );
    assert_eq!(
        panel.hit_test(rect, centre(set)),
        Some(GitPanelHit::SetRemote)
    );
    // The TS remote-settings has no HTTPS-credential input — fetch is the
    // next interactive element (a remote is configured in this state).
    let fetch = panel.remote_settings_fetch_rect(rect);
    assert_eq!(
        panel.hit_test(rect, centre(fetch)),
        Some(GitPanelHit::FetchRemote)
    );
}
