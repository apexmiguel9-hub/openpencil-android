//! Ready view: commit rows, the expanded commit card and pressed-button feedback.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn commit_rows_open_a_commit_diff() {
    let s = state_with(GitPanelState {
        recent_commits: vec![
            GitCommitSummary {
                short_hash: "aaa1111".into(),
                summary: "first".into(),
                author: "Ada".into(),
                time_label: "now".into(),
                is_initial: false,
            },
            GitCommitSummary {
                short_hash: "bbb2222".into(),
                summary: "second".into(),
                author: "Bo".into(),
                time_label: "now".into(),
                is_initial: false,
            },
        ],
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    // A clean tree shows the TS ready view; its history rows map to
    // a commit's diff.
    let rows = panel.ready_commit_row_rects(rect);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        panel.hit_test(rect, centre(rows[0])),
        Some(GitPanelHit::ShowCommitDiff(0))
    );
    assert_eq!(
        panel.hit_test(rect, centre(rows[1])),
        Some(GitPanelHit::ShowCommitDiff(1))
    );
}

#[test]
fn expanded_commit_card_maps_restore_and_copy_and_shifts_later_rows() {
    // Row 0 expanded → its inline detail card (里程碑详情) sits between
    // rows 0 and 1, exposing 恢复 / 复制哈希 buttons and pushing row 1
    // down by the card height.
    let commits = vec![
        GitCommitSummary {
            short_hash: "aaa1111".into(),
            summary: "first".into(),
            author: "Ada".into(),
            time_label: "now".into(),
            is_initial: false,
        },
        GitCommitSummary {
            short_hash: "bbb2222".into(),
            summary: "second".into(),
            author: "Bo".into(),
            time_label: "now".into(),
            is_initial: false,
        },
    ];
    let collapsed = state_with(GitPanelState {
        branch: Some("main".to_string()),
        recent_commits: commits.clone(),
        ..open_repo()
    });
    let cp = GitPanel::for_editor(&collapsed).unwrap();
    let crect = panel_rect(&cp);
    let row1_collapsed = cp.ready_commit_row_rects(crect)[1].origin.y;
    // Same state, but row 0 expanded.
    let expanded = state_with(GitPanelState {
        branch: Some("main".to_string()),
        recent_commits: commits,
        expanded_commit: Some(0),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&expanded).unwrap();
    let rect = panel_rect(&panel);
    // Card buttons exist and map to the expanded row's index.
    let (restore, copy) = panel.ready_commit_card_buttons(rect).unwrap();
    assert_eq!(
        panel.hit_test(rect, centre(restore)),
        Some(GitPanelHit::RestoreCommit(0))
    );
    assert_eq!(
        panel.hit_test(rect, centre(copy)),
        Some(GitPanelHit::CopyCommitHash(0))
    );
    // Row 1 shifted down by exactly the card height; the panel grew too.
    // (No diff loaded in this state → base card height, no patch rows.)
    let row1_expanded = panel.ready_commit_row_rects(rect)[1].origin.y;
    assert!((row1_expanded - row1_collapsed - 104.0).abs() < 0.5);
    assert!(panel.height() > cp.height());
    // The expanded card sits below row 0's click target.
    let row0 = panel.ready_commit_row_rects(rect)[0];
    assert!(restore.origin.y > row0.origin.y);
}

#[test]
fn ready_view_maps_each_header_and_commit_region() {
    // A clean bound repo → the TS ready layout. Its header exposes
    // the branch picker + pull/push + overflow; the commit box is a
    // focus target and its button commits a non-empty message.
    let mut state = open_repo();
    state.commit_input.set_text("ship it");
    let s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        // A remote + commits-ahead so pull/push are enabled (they now
        // disable when there's no remote / nothing to push, TS parity).
        remotes: vec!["origin → https://example.com/r.git".to_string()],
        ahead: 1,
        ..state
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (pull, push, overflow) = panel.ready_header_buttons(rect);
    assert_eq!(
        panel.hit_test(rect, centre(panel.ready_branch_rect(rect))),
        Some(GitPanelHit::BranchPicker)
    );
    assert_eq!(panel.hit_test(rect, centre(pull)), Some(GitPanelHit::Pull));
    assert_eq!(panel.hit_test(rect, centre(push)), Some(GitPanelHit::Push));
    assert_eq!(
        panel.hit_test(rect, centre(overflow)),
        Some(GitPanelHit::Overflow)
    );
    // With a non-empty message the Save-milestone button fires (it
    // saves the live design + commits, so no pre-staged file needed).
    assert_eq!(
        panel.hit_test(rect, centre(panel.ready_commit_btn(rect))),
        Some(GitPanelHit::CommitMilestone)
    );
    // The box body away from the button focuses the input.
    let box_r = panel.ready_commit_box(rect);
    let top_left = Point2D::new(box_r.origin.x + 6.0, box_r.origin.y + 6.0);
    assert_eq!(
        panel.hit_test(rect, top_left),
        Some(GitPanelHit::CommitInput)
    );
}

#[test]
fn ready_header_pressed_overflow_uses_shared_button_feedback() {
    let mut s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        ..open_repo()
    });
    s.editor_ui.pressed_button = Some(ButtonPressTarget::Git(op_editor_core::GitButton::Overflow));
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (_, _, overflow) = panel.ready_header_buttons(rect);
    let theme = crate::widgets::editor_state_ext::theme_for(&s.editor_ui);
    let expected = theme.button_hover.with_alpha(theme.button_hover.a * 1.8);
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend.fills.iter().any(|(fill, radius, color)| {
            *fill == overflow && (*radius - 6.0).abs() < 0.01 && color_close(*color, expected)
        }),
        "pressed overflow button should paint the shared pressed feedback token"
    );
}

#[test]
fn pressed_branch_switch_row_uses_shared_button_feedback() {
    let mut s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["main".to_string(), "feature".to_string()],
        merging: true,
        ..open_repo()
    });
    s.editor_ui.pressed_button = Some(ButtonPressTarget::Git(
        op_editor_core::GitButton::SwitchBranch(1),
    ));
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (_, branch_rects) = panel.branch_layout(rect);
    let feature_row = branch_rects[1];
    let theme = crate::widgets::editor_state_ext::theme_for(&s.editor_ui);
    let expected = theme.button_hover.with_alpha(theme.button_hover.a * 1.8);
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend.fills.iter().any(|(fill, radius, color)| {
            *fill == feature_row && (*radius - 4.0).abs() < 0.01 && color_close(*color, expected)
        }),
        "pressed branch switch row should paint the shared pressed feedback token"
    );
}

#[test]
fn ready_branch_picker_pressed_row_uses_shared_button_feedback() {
    let mut s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        branches: vec!["main".to_string(), "feature".to_string()],
        branch_picker_open: true,
        ..open_repo()
    });
    s.editor_ui.pressed_button = Some(ButtonPressTarget::Git(
        op_editor_core::GitButton::SwitchBranch(1),
    ));
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.branch_picker_row_rects(rect);
    let theme = crate::widgets::editor_state_ext::theme_for(&s.editor_ui);
    let expected = theme.button_hover.with_alpha(theme.button_hover.a * 1.8);
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend.fills.iter().any(|(fill, radius, color)| {
            *fill == rows[1] && (*radius - 6.0).abs() < 0.01 && color_close(*color, expected)
        }),
        "pressed ready branch-picker row should paint the shared pressed feedback token"
    );
}

#[test]
fn overflow_menu_pressed_row_uses_shared_button_feedback() {
    let mut s = state_with(GitPanelState {
        branch: Some("main".to_string()),
        overflow_open: true,
        ..open_repo()
    });
    s.editor_ui.pressed_button = Some(ButtonPressTarget::Git(
        op_editor_core::GitButton::OverflowRemoteSettings,
    ));
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let rows = panel.overflow_row_rects(rect);
    let theme = crate::widgets::editor_state_ext::theme_for(&s.editor_ui);
    let expected = theme.button_hover.with_alpha(theme.button_hover.a * 1.8);
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };

    panel.paint(&mut cx, rect);

    assert!(
        backend.fills.iter().any(|(fill, radius, color)| {
            *fill == rows[2] && (*radius - 6.0).abs() < 0.01 && color_close(*color, expected)
        }),
        "pressed overflow-menu row should paint the shared pressed feedback token"
    );
}
