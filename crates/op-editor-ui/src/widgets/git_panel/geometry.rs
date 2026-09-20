//! Layout maths for [`GitPanel`] — list-mode selection, panel height,
//! and the shared paint/hit-test rect walkers (action area, list rows,
//! status line, Branches section). Carved off `git_panel.rs` to keep
//! every file under the 800-line cap; nothing here paints.

use super::*;

impl GitPanel<'_> {
    /// What the panel's list slot shows — conflicts during a merge,
    /// the working-tree changes when the tree is dirty, otherwise the
    /// recent-commit history.
    pub(in crate::widgets) fn list_mode(&self) -> ListMode {
        if self.state.merging {
            ListMode::Merge
        } else if !self.state.changed_files.is_empty() {
            ListMode::Changes
        } else {
            ListMode::Commits
        }
    }

    /// Row count of the list slot. At least one (the placeholder
    /// row), capped at `MAX_COMMITS`.
    fn list_rows(&self) -> usize {
        let len = match self.list_mode() {
            ListMode::Merge => self.state.conflicted_files.len(),
            ListMode::Changes => self.state.changed_files.len(),
            ListMode::Commits => self.state.recent_commits.len(),
        };
        len.clamp(1, MAX_COMMITS)
    }

    /// The panel's total height for the current content.
    pub fn height(&self) -> f32 {
        // The inline clone wizard takes over the whole panel.
        if self.state.clone_form.is_some() {
            return self.clone_view_height();
        }
        // The merge-resolution view sizes to its conflict count.
        if self.state.merge_resolve.is_some() {
            return self.resolve_view_height();
        }
        // Diff mode is a fixed-height scrollable view.
        if self.state.diff.is_some() {
            return DIFF_VIEW_HEIGHT;
        }
        if self.is_empty_state() {
            // Centred onboarding: clock + heading + cards + note.
            return EMPTY_STATE_HEIGHT;
        }
        if self.state.loading || !self.state.in_repo {
            // Header + one status line ("Loading…" / "not a repo")
            // + footer — no branch / action area / commit list.
            return HEADER_BASELINE + 24.0 + FOOTER_H + PAD;
        }
        // Clean bound-repo → the TS ready layout sizes to its history.
        if self.is_ready_state() {
            return self.ready_height();
        }
        // List + Branches + Remotes sections, then the footer.
        self.remotes_section_top() + self.remotes_block_height() + FOOTER_H + PAD
    }

    /// Panel-relative `y` where the Remotes section begins — below
    /// the list slot and the (optional) Branches section.
    pub(in crate::widgets) fn remotes_section_top(&self) -> f32 {
        let commit_rows = self.list_rows();
        let branch_count = self.state.branches.len().min(MAX_BRANCHES);
        // The Branches section is omitted entirely when empty.
        let branches_h = if branch_count == 0 {
            0.0
        } else {
            SECTION_GAP + BRANCH_LABEL_GAP + branch_count as f32 * BRANCH_ROW_H
        };
        COMMITS_FIRST_BASELINE + commit_rows as f32 * COMMIT_ROW_H + branches_h
    }

    /// The Branches section layout — the "Branches" label baseline
    /// and one clickable rect per listed branch. The list is empty
    /// when the repository has no branches yet. Shared by
    /// [`GitPanel::paint`] + [`GitPanel::hit_test`].
    pub(in crate::widgets) fn branch_layout(&self, panel: Rect) -> (f32, Vec<Rect>) {
        let commit_rows = self.list_rows();
        let label_baseline = panel.origin.y
            + COMMITS_FIRST_BASELINE
            + commit_rows as f32 * COMMIT_ROW_H
            + SECTION_GAP;
        let left = panel.origin.x + PAD;
        let inner_w = panel.size.x - PAD * 2.0;
        let first_row_top = label_baseline + BRANCH_LABEL_GAP;
        let rects = (0..self.state.branches.len().min(MAX_BRANCHES))
            .map(|i| Rect {
                origin: Point2D::new(left, first_row_top + i as f32 * BRANCH_ROW_H),
                size: Point2D::new(inner_w, BRANCH_ROW_H),
            })
            .collect();
        (label_baseline, rects)
    }

    /// The interactive action-area sub-rects, derived from the panel
    /// rect. The button row holds 4 equal buttons normally and 3 in
    /// merge mode. Shared by [`GitPanel::paint`] + [`GitPanel::hit_test`].
    pub(in crate::widgets) fn action_rects(panel: Rect, merging: bool) -> ActionRects {
        let left = panel.origin.x + PAD;
        let inner_w = panel.size.x - PAD * 2.0;
        let input = Rect {
            origin: Point2D::new(left, panel.origin.y + INPUT_TOP),
            size: Point2D::new(inner_w, INPUT_H),
        };
        let n = if merging { 3 } else { 4 };
        let nf = n as f32;
        let button_w = (inner_w - (nf - 1.0) * BUTTON_GAP) / nf;
        let button_top = panel.origin.y + BUTTON_TOP;
        let buttons = (0..n)
            .map(|i| Rect {
                origin: Point2D::new(left + i as f32 * (button_w + BUTTON_GAP), button_top),
                size: Point2D::new(button_w, BUTTON_H),
            })
            .collect();
        ActionRects { input, buttons }
    }

    /// One clickable rect per displayed list row — changed files,
    /// recent commits, or conflicts depending on [`GitPanel::list_mode`].
    /// Mirrors the [`GitPanel::paint`] list walk so paint + hit-test
    /// agree.
    pub(in crate::widgets) fn list_row_rects(&self, panel: Rect) -> Vec<Rect> {
        let count = match self.list_mode() {
            ListMode::Merge => self.state.conflicted_files.len(),
            ListMode::Changes => self.state.changed_files.len(),
            ListMode::Commits => self.state.recent_commits.len(),
        }
        .min(MAX_COMMITS);
        let left = panel.origin.x + PAD;
        let inner_w = panel.size.x - PAD * 2.0;
        let first = panel.origin.y + COMMITS_FIRST_BASELINE;
        (0..count)
            .map(|i| Rect {
                origin: Point2D::new(left, first + i as f32 * COMMIT_ROW_H - 15.0),
                size: Point2D::new(inner_w, COMMIT_ROW_H),
            })
            .collect()
    }

    /// The working-tree status-line rect — a diff trigger when the
    /// tree has changes.
    pub(in crate::widgets) fn status_rect(&self, panel: Rect) -> Rect {
        Rect {
            origin: Point2D::new(
                panel.origin.x + PAD,
                panel.origin.y + STATUS_BASELINE - 14.0,
            ),
            size: Point2D::new(panel.size.x - PAD * 2.0, 18.0),
        }
    }

    /// The "merge into current" button at the right edge of a
    /// (non-current) branch row.
    pub(in crate::widgets) fn branch_merge_button(row: Rect) -> Rect {
        let size = BRANCH_ROW_H - 4.0;
        Rect {
            origin: Point2D::new(row.origin.x + row.size.x - size - 4.0, row.origin.y + 2.0),
            size: Point2D::new(size, size),
        }
    }
}
