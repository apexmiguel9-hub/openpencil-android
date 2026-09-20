//! Empty-state cards, the clone form and the dirty bound-repo fallback.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

#[test]
fn empty_state_cards_map_to_actions() {
    // No repo + saved doc → Init enabled, all three cards act.
    let saved = state_with(GitPanelState {
        open: true,
        in_repo: false,
        has_saved_file: true,
        ..GitPanelState::default()
    });
    let panel = GitPanel::for_editor(&saved).unwrap();
    let rect = panel_rect(&panel);
    let cards = panel.empty_state_rects(rect);
    assert_eq!(
        panel.hit_test(rect, centre(cards[0])),
        Some(GitPanelHit::EmptyInit)
    );
    assert_eq!(
        panel.hit_test(rect, centre(cards[1])),
        Some(GitPanelHit::EmptyOpen)
    );
    assert_eq!(
        panel.hit_test(rect, centre(cards[2])),
        Some(GitPanelHit::EmptyClone)
    );

    // Unsaved doc → Init card is inert (swallowed), Open still acts.
    let unsaved = state_with(GitPanelState {
        open: true,
        in_repo: false,
        has_saved_file: false,
        ..GitPanelState::default()
    });
    let panel = GitPanel::for_editor(&unsaved).unwrap();
    let rect = panel_rect(&panel);
    let cards = panel.empty_state_rects(rect);
    assert_eq!(
        panel.hit_test(rect, centre(cards[0])),
        Some(GitPanelHit::Inside)
    );
    assert_eq!(
        panel.hit_test(rect, centre(cards[1])),
        Some(GitPanelHit::EmptyOpen)
    );
}

#[test]
fn clone_form_takes_over_and_maps_each_target() {
    // With `clone_form` set the panel switches to the clone view; each
    // field / button hit-tests to its own action regardless of repo
    // state (the wizard opens from the no-repo empty state).
    let st = state_with(GitPanelState {
        open: true,
        in_repo: false,
        clone_form: Some(CloneFormState {
            url_input: jian_core::text_input::TextInputState::with_text(
                "https://github.com/owner/repo.git",
            ),
            dest_input: jian_core::text_input::TextInputState::with_text("/tmp/repo"),
            focus: Some(CloneField::Url),
            ..Default::default()
        }),
        ..GitPanelState::default()
    });
    let panel = GitPanel::for_editor(&st).unwrap();
    let rect = panel_rect(&panel);
    // The view sizes to the clone layout (positive, finite height).
    assert!(panel.height() > 0.0);
    let layout = panel.clone_layout(rect);
    assert_eq!(
        panel.hit_test(rect, centre(layout.url_input)),
        Some(GitPanelHit::CloneUrlInput)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.dest_input)),
        Some(GitPanelHit::CloneDestInput)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.dest_pick)),
        Some(GitPanelHit::CloneDestPick)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.submit)),
        Some(GitPanelHit::CloneSubmit)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.cancel)),
        Some(GitPanelHit::CloneCancel)
    );
    // The dest input + pick button must not overlap.
    assert!(
        layout.dest_input.origin.x + layout.dest_input.size.x <= layout.dest_pick.origin.x + 0.01,
        "dest input + pick button overlap"
    );
}

#[test]
fn clone_view_locks_to_cancel_only_while_cloning() {
    // Mid-clone the form is locked: only Cancel acts (it abandons the
    // job); the URL / destination / pick / submit controls are greyed
    // and must swallow clicks instead of mutating a running clone.
    let st = state_with(GitPanelState {
        open: true,
        in_repo: false,
        clone_form: Some(CloneFormState {
            url_input: jian_core::text_input::TextInputState::with_text(
                "https://github.com/owner/repo.git",
            ),
            dest_input: jian_core::text_input::TextInputState::with_text("/tmp/repo"),
            cloning: true,
            ..Default::default()
        }),
        ..GitPanelState::default()
    });
    let panel = GitPanel::for_editor(&st).unwrap();
    let rect = panel_rect(&panel);
    let layout = panel.clone_layout(rect);
    assert_eq!(
        panel.hit_test(rect, centre(layout.cancel)),
        Some(GitPanelHit::CloneCancel)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.submit)),
        Some(GitPanelHit::Inside)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.url_input)),
        Some(GitPanelHit::Inside)
    );
    assert_eq!(
        panel.hit_test(rect, centre(layout.dest_pick)),
        Some(GitPanelHit::Inside)
    );
}

#[test]
fn dirty_bound_repo_still_shows_the_ready_view() {
    // TS parity: a bound, non-merging repo shows the ready view whether
    // the working tree is clean OR dirty (there is no per-file staging
    // view in TS; the commit-milestone flow handles dirty changes).
    let mut state = open_repo();
    state.changed_files = vec![GitFileEntry {
        path: "x.op".into(),
        staged: false,
        status: 'M',
    }];
    state.dirty_count = 1;
    let editor = state_with(state);
    let panel = GitPanel::for_editor(&editor).expect("open repo => panel");
    assert!(
        panel.is_ready_state(),
        "a dirty bound repo must still show the ready view",
    );
}
