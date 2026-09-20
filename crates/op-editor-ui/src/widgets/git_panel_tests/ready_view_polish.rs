//! Ready-view polish: card button geometry, height clamping and hover wash.
//!
//! Split out of `git_panel_tests.rs` to keep that file under the
//! 800-line cap.

use super::*;

// --- Ready-view polish (milestone detail card, buttons, timeline, height) --

#[test]
fn expanded_card_buttons_clear_the_timeline_rail_column() {
    // The inline detail card insets past the timeline rail/dot column
    // (20px from the panel edge) so the vertical connector line stays
    // visible underneath an expanded card instead of a full-width card
    // background painting over (and visually severing) it.
    let s = state_with(GitPanelState {
        recent_commits: vec![one_commit()],
        expanded_commit: Some(0),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (restore, copy) = panel.ready_commit_card_buttons(rect).unwrap();
    assert!(
        restore.origin.x > rect.origin.x + 28.0,
        "the card's Restore button must sit clear of the rail + dot column"
    );
    assert!(
        copy.origin.x > restore.origin.x,
        "Copy hash sits after Restore"
    );
}

#[test]
fn expanded_card_restore_and_copy_share_the_same_button_style() {
    // Restore used to paint a bordered outline while Copy hash painted
    // bare text with no chrome at all — two different-looking
    // affordances for two sibling actions. Both must now paint the
    // SAME secondary-button fill (unified via `paint_button_with_hit`).
    let s = state_with(GitPanelState {
        recent_commits: vec![one_commit()],
        expanded_commit: Some(0),
        ..open_repo()
    });
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let (restore, copy) = panel.ready_commit_card_buttons(rect).unwrap();
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    let paints_button_chrome = |target: Rect| {
        backend
            .fills
            .iter()
            .any(|(r, radius, _)| *r == target && (*radius - 6.0).abs() < 0.01)
    };
    assert!(
        paints_button_chrome(restore),
        "Restore should paint the shared button fill"
    );
    assert!(
        paints_button_chrome(copy),
        "Copy hash should paint the SAME shared button fill it used to skip entirely"
    );
}

#[test]
fn ready_height_fits_content_instead_of_padding_with_fixed_filler() {
    // A short history used to always add 200px of flat filler space
    // below it regardless of content ("大片空白" under a one-line
    // history). The panel must now size close to its actual content.
    let s = state_with(open_repo());
    let h = GitPanel::for_editor(&s).unwrap().height();
    assert!(
        h < 300.0,
        "a short history must not pad the panel with fixed filler space (got {h})"
    );
}

#[test]
fn ready_height_clamps_at_the_max_instead_of_growing_unbounded() {
    // A full 8-row history plus a card expanded with a full patch list
    // is the worst case for content height — it must clamp rather than
    // grow the floating popover without bound.
    let commits: Vec<_> = (0..8)
        .map(|i| GitCommitSummary {
            short_hash: format!("hash{i}"),
            summary: format!("commit {i}"),
            author: "Ada".into(),
            time_label: "now".into(),
            is_initial: false,
        })
        .collect();
    let s = state_with(GitPanelState {
        recent_commits: commits,
        expanded_commit: Some(0),
        expanded_commit_diff: Some(CommitDiffView::Ready(CommitDiffSummary {
            frames_changed: 1,
            nodes_added: 2,
            nodes_removed: 0,
            nodes_modified: 1,
            patches: (0..6)
                .map(|i| CommitDiffPatch {
                    op: "update".into(),
                    node_id: format!("n{i}"),
                })
                .collect(),
        })),
        ..open_repo()
    });
    let h = GitPanel::for_editor(&s).unwrap().height();
    assert!(
        h <= 520.0 + 0.01,
        "the ready view must clamp at READY_MAX_HEIGHT (got {h})"
    );
}

#[test]
fn commit_rows_paint_a_hover_wash() {
    // Hovering a commit row used to show no feedback at all even
    // though clicking it toggles the detail card — the row needs the
    // same shared hover wash every other clickable row in the panel
    // paints.
    let mut s = state_with(GitPanelState {
        recent_commits: vec![one_commit()],
        ..open_repo()
    });
    s.editor_ui.git_panel.button_hover = Some(op_editor_core::GitButton::ShowCommitDiff(0));
    let panel = GitPanel::for_editor(&s).unwrap();
    let rect = panel_rect(&panel);
    let row = panel.ready_commit_row_rects(rect)[0];
    let mut backend = RoundFillBackend::default();
    let mut cx = crate::widgets::PaintCx {
        backend: &mut backend,
    };
    panel.paint(&mut cx, rect);
    assert!(
        backend.fills.iter().any(|(r, _, _)| *r == row),
        "a hovered commit row should paint the shared hover wash"
    );
}
