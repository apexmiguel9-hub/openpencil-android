//! Diff mode: panel geometry, header buttons and scroll metrics.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn diff_mode_widens_the_panel_and_fixes_its_height() {
    let normal = state_with(open_repo());
    let normal_w = GitPanel::for_editor(&normal).unwrap().panel_width();
    assert_eq!(normal_w, GIT_PANEL_WIDTH);

    let diffing = state_with(GitPanelState {
        diff: Some(GitDiffView {
            title: "Working tree".into(),
            lines: vec!["+a".into(), "-b".into()],
            scroll: 0,
            h_scroll: 0,
            stage_path: None,
        }),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&diffing).unwrap();
    assert_eq!(panel.panel_width(), GIT_DIFF_PANEL_WIDTH);
    // Diff mode is a tall fixed-height view.
    assert!(panel.height() > 400.0);
}

#[test]
fn diff_header_buttons_map_to_scroll_and_close() {
    let diffing = state_with(GitPanelState {
        diff: Some(GitDiffView {
            title: "Working tree".into(),
            lines: (0..200).map(|i| format!("+line {i}")).collect(),
            scroll: 0,
            h_scroll: 0,
            stage_path: None,
        }),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&diffing).unwrap();
    let rect = panel_rect(&panel);
    let [left, right, up, down, close] = GitPanel::diff_header_buttons(rect);
    assert_eq!(
        panel.hit_test(rect, centre(left)),
        Some(GitPanelHit::DiffScrollLeft)
    );
    assert_eq!(
        panel.hit_test(rect, centre(right)),
        Some(GitPanelHit::DiffScrollRight)
    );
    assert_eq!(
        panel.hit_test(rect, centre(up)),
        Some(GitPanelHit::DiffScrollUp)
    );
    assert_eq!(
        panel.hit_test(rect, centre(down)),
        Some(GitPanelHit::DiffScrollDown)
    );
    assert_eq!(
        panel.hit_test(rect, centre(close)),
        Some(GitPanelHit::CloseDiff)
    );
    // The diff body itself swallows clicks.
    assert_eq!(
        panel.hit_test(rect, Point2D::new(40.0, 200.0)),
        Some(GitPanelHit::Inside)
    );
}

#[test]
fn diff_scroll_metrics_clamp_to_the_line_count() {
    // A short diff fits in one page → nothing to scroll.
    let short = state_with(GitPanelState {
        diff: Some(GitDiffView {
            title: "t".into(),
            lines: vec!["+a".into(), "+b".into()],
            scroll: 0,
            h_scroll: 0,
            stage_path: None,
        }),
        ..open_repo()
    });
    assert_eq!(GitPanel::for_editor(&short).unwrap().diff_max_scroll(), 0);

    // A long diff → a positive max scroll, and a non-zero page step.
    let long = state_with(GitPanelState {
        diff: Some(GitDiffView {
            title: "t".into(),
            lines: (0..500).map(|i| format!("+{i}")).collect(),
            scroll: 0,
            h_scroll: 0,
            stage_path: None,
        }),
        ..open_repo()
    });
    assert!(GitPanel::for_editor(&long).unwrap().diff_max_scroll() > 0);
    assert!(GitPanel::diff_page_step() >= 1);
}
