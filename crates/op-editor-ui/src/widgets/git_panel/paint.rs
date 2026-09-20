//! Paint pass for [`GitPanel`]'s classic status body — header, branch /
//! status lines, the action area, the list slot (conflicts / changed
//! files / commits), the Branches section, and the shared button +
//! divider primitives. Carved off `git_panel.rs` to keep every file
//! under the 800-line cap; geometry comes from `git_panel/geometry.rs`.

use super::*;
use crate::widgets::text_metrics;

impl GitPanel<'_> {
    /// Paint the panel into `rect`.
    pub fn paint(&self, cx: &mut PaintCx<'_>, rect: Rect) {
        // TS popover radius is `rounded-md` = 6px (Rust was 10px, too round).
        cx.backend.fill_round_rect(rect, 6.0, self.theme.popover);
        cx.backend
            .stroke_round_rect(rect, 6.0, self.theme.border, 1.0);

        // The inline clone wizard replaces the whole body.
        if self.state.clone_form.is_some() {
            self.paint_clone(cx, rect);
            return;
        }
        // The merge-resolution view replaces the whole body.
        if self.state.merge_resolve.is_some() {
            self.paint_resolve(cx, rect);
            return;
        }
        // Diff mode replaces the whole body with the scrollable view.
        if let Some(view) = &self.state.diff {
            self.paint_diff(cx, rect, view);
            return;
        }
        // No-repo onboarding empty state — a centred clock + heading +
        // Init/Open/Clone cards + note (no "Git" title bar), mirroring
        // the TS `git-panel-empty-state`.
        if self.is_empty_state() {
            self.paint_empty_state(cx, rect);
            return;
        }

        let left = rect.origin.x + PAD;
        let top = rect.origin.y;

        // Clean bound-repo → the TS `GitPanelReady` layout: a compact
        // header (branch button + pull/push + overflow), a commit
        // textarea, and the recent-commit history. Painted BEFORE the
        // classic "Git" title so that title never bleeds through the
        // ready header. Dirty trees + merges keep the classic body.
        if self.is_ready_state() {
            self.paint_ready(cx, rect);
            // Header popovers paint on top of the ready body.
            if self.state.branch_picker_open {
                self.paint_branch_picker(cx, rect);
            } else if self.state.overflow_open {
                self.paint_overflow(cx, rect);
            }
            return;
        }

        self.text(
            cx,
            self.t("git.panel.title"),
            left,
            top + HEADER_BASELINE,
            15.0,
            self.theme.foreground,
        );

        // Loading: the prior data is for a since-switched repository,
        // so show a neutral "Loading…" rather than stale branch /
        // commits until the new snapshot lands.
        if self.state.loading {
            self.text(
                cx,
                self.t("git.panel.loading"),
                left,
                top + HEADER_BASELINE + 24.0,
                12.0,
                self.theme.muted_foreground,
            );
            self.footer(cx, left, top + self.height() - PAD);
            return;
        }

        if !self.state.in_repo {
            self.text(
                cx,
                self.t("git.panel.notARepo"),
                left,
                top + HEADER_BASELINE + 24.0,
                12.0,
                self.theme.muted_foreground,
            );
            self.footer(cx, left, top + self.height() - PAD);
            return;
        }

        // Branch + working-tree status.
        let branch = self
            .state
            .branch
            .clone()
            .unwrap_or_else(|| self.t("git.panel.detachedHead").to_string());
        self.text(
            cx,
            &self.t("git.panel.branch").replace("{{name}}", &branch),
            left,
            top + BRANCH_BASELINE,
            13.0,
            self.theme.foreground,
        );
        let (status_text, status_color) = self.status_line();
        self.text(
            cx,
            &status_text,
            left,
            top + STATUS_BASELINE,
            12.0,
            status_color,
        );

        self.divider(cx, left, top + DIVIDER_1_Y, rect.size.x);

        // Action area. Normal mode: commit input + Commit / Refresh
        // / Pull / Push. Merge mode: a warning banner + Abort /
        // Refresh / Complete (Complete only once conflicts resolve).
        let rects = Self::action_rects(rect, self.state.merging);
        if self.state.merging {
            self.paint_merge_banner(cx, rects.input);
            self.paint_button_with_hit(
                cx,
                rects.buttons[0],
                self.t("git.panel.abortMerge"),
                true,
                false,
                Some(GitPanelHit::AbortMerge),
            );
            self.paint_button_with_hit(
                cx,
                rects.buttons[1],
                self.t("git.panel.refresh"),
                true,
                false,
                Some(GitPanelHit::Refresh),
            );
            let can_complete = self.state.conflicted_files.is_empty();
            self.paint_button_with_hit(
                cx,
                rects.buttons[2],
                self.t("git.panel.complete"),
                can_complete,
                true,
                Some(GitPanelHit::CompleteMerge),
            );
        } else {
            self.paint_input(cx, rects.input);
            // Commit needs a message *and* a staged file — it commits
            // exactly the staged set, so nothing staged is a no-op.
            let commit_enabled = !self.state.commit_input.text().trim().is_empty()
                && self.state.changed_files.iter().any(|f| f.staged);
            self.paint_button_with_hit(
                cx,
                rects.buttons[0],
                self.t("git.panel.commit"),
                commit_enabled,
                true,
                Some(GitPanelHit::Commit),
            );
            self.paint_button_with_hit(
                cx,
                rects.buttons[1],
                self.t("git.panel.refresh"),
                true,
                false,
                Some(GitPanelHit::Refresh),
            );
            // Pull / Push are disabled while their op already runs.
            self.paint_button_with_hit(
                cx,
                rects.buttons[2],
                self.t("git.panel.pull"),
                !self.state.pulling,
                false,
                Some(GitPanelHit::Pull),
            );
            self.paint_button_with_hit(
                cx,
                rects.buttons[3],
                self.t("git.panel.push"),
                !self.state.pushing,
                false,
                Some(GitPanelHit::Push),
            );
        }

        self.divider(cx, left, top + DIVIDER_2_Y, rect.size.x);

        // List section — conflicts (merge), the per-file staging
        // list (dirty tree), or recent commits (clean tree).
        let conflict_red = Color {
            r: 0.94,
            g: 0.27,
            b: 0.27,
            a: 1.0,
        };
        let label_y = top + COMMITS_LABEL_BASELINE;
        let mut y = top + COMMITS_FIRST_BASELINE;
        match self.list_mode() {
            ListMode::Merge => {
                self.text(
                    cx,
                    self.t("git.panel.conflicts"),
                    left,
                    label_y,
                    12.0,
                    self.theme.muted_foreground,
                );
                if self.state.conflicted_files.is_empty() {
                    self.text(
                        cx,
                        self.t("git.panel.noConflicts"),
                        left,
                        y,
                        12.0,
                        self.theme.muted_foreground,
                    );
                }
                for path in self.state.conflicted_files.iter().take(MAX_COMMITS) {
                    self.text(
                        cx,
                        &format!("⚠ {}", truncate(path, SUMMARY_MAX)),
                        left,
                        y,
                        12.0,
                        conflict_red,
                    );
                    y += COMMIT_ROW_H;
                }
            }
            ListMode::Changes => {
                let staged = self.state.changed_files.iter().filter(|f| f.staged).count();
                self.text(
                    cx,
                    &self
                        .t("git.panel.changes")
                        .replace("{{staged}}", &staged.to_string())
                        .replace("{{total}}", &self.state.changed_files.len().to_string()),
                    left,
                    label_y,
                    12.0,
                    self.theme.muted_foreground,
                );
                for file in self.state.changed_files.iter().take(MAX_COMMITS) {
                    // `[✓] M  path` — a click on the row toggles
                    // whether the file is staged.
                    let mark = if file.staged { "☑" } else { "☐" };
                    let color = if file.staged {
                        self.theme.foreground
                    } else {
                        self.theme.muted_foreground
                    };
                    self.text(
                        cx,
                        &format!(
                            "{mark} {}  {}",
                            file.status,
                            truncate(&file.path, SUMMARY_MAX)
                        ),
                        left,
                        y,
                        12.0,
                        color,
                    );
                    y += COMMIT_ROW_H;
                }
            }
            ListMode::Commits => {
                self.text(
                    cx,
                    self.t("git.panel.recentCommits"),
                    left,
                    label_y,
                    12.0,
                    self.theme.muted_foreground,
                );
                if self.state.recent_commits.is_empty() {
                    self.text(
                        cx,
                        self.t("git.panel.noCommits"),
                        left,
                        y,
                        12.0,
                        self.theme.muted_foreground,
                    );
                }
                for commit in self.state.recent_commits.iter().take(MAX_COMMITS) {
                    let summary = truncate(&commit.summary, SUMMARY_MAX);
                    self.text(
                        cx,
                        &format!("{}  {}", commit.short_hash, summary),
                        left,
                        y,
                        12.0,
                        self.theme.foreground,
                    );
                    y += COMMIT_ROW_H;
                }
            }
        }

        // Branches section — one row per local branch, the current
        // one marked + faintly highlighted, the rest click-to-switch.
        if !self.state.branches.is_empty() {
            let (label_baseline, branch_rects) = self.branch_layout(rect);
            self.text(
                cx,
                self.t("git.panel.branches"),
                left,
                label_baseline,
                12.0,
                self.theme.muted_foreground,
            );
            for (i, row) in branch_rects.iter().enumerate() {
                let name = &self.state.branches[i];
                let is_current = Some(name) == self.state.branch.as_ref();
                if is_current {
                    cx.backend.fill_round_rect(*row, 4.0, self.theme.muted);
                } else {
                    self.wash_if_hovered(cx, *row, 4.0, GitPanelHit::SwitchBranch(i));
                }
                let (marker, color) = if is_current {
                    ("● ", self.theme.primary)
                } else {
                    ("  ", self.theme.foreground)
                };
                let baseline = row.origin.y + BRANCH_ROW_H - 7.0;
                self.text(
                    cx,
                    &format!("{marker}{name}"),
                    left + 4.0,
                    baseline,
                    12.0,
                    color,
                );
                // Non-current branches carry a "merge into current"
                // button at the row's right edge.
                if !is_current {
                    self.paint_glyph_button(
                        cx,
                        Self::branch_merge_button(*row),
                        "⤵",
                        self.state.button_hover == Some(GitButton::MergeBranch(i)),
                        self.pressed == Some(GitButton::MergeBranch(i)),
                    );
                }
            }
        }

        // Remotes section — remote summary + a URL input that adds /
        // re-points `origin` (see `git_panel_remotes.rs`).
        self.paint_remotes(cx, rect);

        // Footer — always pinned a fixed inset above the panel foot.
        self.footer(cx, left, top + self.height() - PAD);
    }

    /// Paint one action button. `enabled` dims a disabled button;
    /// `primary` paints the accent (Commit) style; `hit` (when `Some`)
    /// drives the per-button `theme.button_hover` wash.
    pub(in crate::widgets) fn paint_button_with_hit(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        label: &str,
        enabled: bool,
        primary: bool,
        hit: Option<GitPanelHit>,
    ) {
        let (fill, text_color) = match (enabled, primary) {
            (true, true) => (self.theme.primary, self.theme.primary_foreground),
            (true, false) => (self.theme.muted, self.theme.foreground),
            (false, _) => (self.theme.muted, self.theme.muted_foreground),
        };
        cx.backend.fill_round_rect(rect, 6.0, fill);
        // Hover/pressed wash — only when actionable + the pointer is on this button.
        if enabled {
            let hovered = hit.is_some_and(|h| self.is_hovered(h));
            let pressed = hit.is_some_and(|h| self.is_pressed(h));
            crate::widgets::button::paint_ghost_button_feedback(
                cx.backend,
                &self.theme,
                rect,
                hovered,
                pressed,
            );
        }
        if !primary {
            cx.backend
                .stroke_round_rect(rect, 6.0, self.theme.border, 1.0);
        }
        // Centre the label using the real measured width so CJK labels
        // (e.g. "保存为里程碑", ~2× a Latin glyph) centre correctly
        // instead of overflowing the right edge. A long localized label
        // can still exceed the fixed button — clip the draw to the rect
        // so it never bleeds into a neighbour.
        let label_w = text_metrics::measure_chrome(cx.backend, label, 12.0);
        let text_x = rect.origin.x + (rect.size.x - label_w).max(6.0) / 2.0;
        let baseline = rect.origin.y + rect.size.y / 2.0 + 4.0;
        cx.backend.save();
        cx.backend.clip_rect(rect);
        self.text(cx, label, text_x, baseline, 12.0, text_color);
        cx.backend.restore();
    }

    /// Paint a 1-px divider line.
    pub(in crate::widgets) fn divider(
        &self,
        cx: &mut PaintCx<'_>,
        left: f32,
        y: f32,
        panel_width: f32,
    ) {
        cx.backend.fill_rect(
            Rect {
                origin: Point2D::new(left, y),
                size: Point2D::new(panel_width - PAD * 2.0, 1.0),
            },
            self.theme.border,
        );
    }
}
