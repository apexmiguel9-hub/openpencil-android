//! The branch-picker dropdown for [`GitPanel`] — the list / create /
//! merge modes behind the ready header's `⎇ <branch> ▾` button, plus
//! their shared geometry so paint and hit-test agree. Carved off
//! `git_panel_menus.rs` to keep every file under the 800-line cap.

use super::*;

impl GitPanel<'_> {
    // ── Branch picker ────────────────────────────────────────────────

    /// The branch-picker dropdown rect, anchored below the branch
    /// button. At least one row tall (a placeholder when no branches).
    pub(in crate::widgets) fn branch_picker_panel(&self, panel_rect: Rect) -> Rect {
        let btn = self.ready_branch_rect(panel_rect);
        let w = PICKER_W.min(panel_rect.size.x - PAD * 2.0);
        let body = match self.state.branch_picker_mode {
            GitBranchPickerMode::Create => PICKER_CREATE_H + 6.0,
            GitBranchPickerMode::Merge => {
                let n = self.merge_candidate_indices().len().max(1) as f32;
                PICKER_HEADER_H + n * PICKER_ROW_H + PICKER_FOOTER_H
            }
            GitBranchPickerMode::List => {
                let rows = self.state.branches.len().clamp(1, MENU_MAX_BRANCHES) as f32;
                PICKER_HEADER_H + rows * PICKER_ROW_H + PICKER_DIVIDER_H + PICKER_FOOTER_H
            }
        };
        Rect {
            origin: Point2D::new(btn.origin.x, btn.origin.y + btn.size.y + 4.0),
            size: Point2D::new(w, MENU_PAD * 2.0 + body),
        }
    }

    /// Indices (into `branches`) of the non-current branches — the
    /// merge-mode candidates.
    fn merge_candidate_indices(&self) -> Vec<usize> {
        self.state
            .branches
            .iter()
            .enumerate()
            .filter(|(_, b)| Some(*b) != self.state.branch.as_ref())
            .map(|(i, _)| i)
            .take(MENU_MAX_BRANCHES)
            .collect()
    }

    /// One clickable rect per listed branch — list mode lists every
    /// branch, merge mode only the non-current candidates, create mode
    /// has none. Offset below the header band, one two-line row each.
    pub(in crate::widgets) fn branch_picker_row_rects(&self, panel_rect: Rect) -> Vec<Rect> {
        let panel = self.branch_picker_panel(panel_rect);
        let top = panel.origin.y + MENU_PAD + PICKER_HEADER_H;
        let count = match self.state.branch_picker_mode {
            GitBranchPickerMode::Create => 0,
            GitBranchPickerMode::Merge => self.merge_candidate_indices().len(),
            GitBranchPickerMode::List => self.state.branches.len().min(MENU_MAX_BRANCHES),
        };
        (0..count)
            .map(|i| Rect {
                origin: Point2D::new(panel.origin.x + MENU_PAD, top + i as f32 * PICKER_ROW_H),
                size: Point2D::new(panel.size.x - MENU_PAD * 2.0, PICKER_ROW_H),
            })
            .collect()
    }

    /// (新建分支, 合并分支) footer button rects (List mode). A CJK-aware
    /// width heuristic keeps paint + hit aligned without a backend.
    fn branch_footer_rects(&self, panel: Rect) -> (Rect, Rect) {
        let y = panel.origin.y + panel.size.y - MENU_PAD - PICKER_FOOTER_H
            + (PICKER_FOOTER_H - 22.0) / 2.0;
        let create_w = label_px(self.t("git.branch.createAction"));
        let merge_w = label_px(self.t("git.branch.mergeAction"));
        let merge_x = panel.origin.x + panel.size.x - 12.0 - merge_w;
        let create_x = merge_x - 16.0 - create_w;
        (
            Rect {
                origin: Point2D::new(create_x - 6.0, y),
                size: Point2D::new(create_w + 12.0, 22.0),
            },
            Rect {
                origin: Point2D::new(merge_x - 6.0, y),
                size: Point2D::new(merge_w + 12.0, 22.0),
            },
        )
    }

    /// (input, submit, cancel) rects for the inline 新建分支 form. TS renders
    /// no header here, so the input starts near the popover top.
    fn branch_create_rects(&self, panel: Rect) -> (Rect, Rect, Rect) {
        let left = panel.origin.x + MENU_PAD + 6.0;
        let inner_w = panel.size.x - (MENU_PAD + 6.0) * 2.0;
        let input_top = panel.origin.y + MENU_PAD + 6.0;
        let input = Rect {
            origin: Point2D::new(left, input_top),
            size: Point2D::new(inner_w, 30.0),
        };
        let btn_y = input_top + 30.0 + 8.0;
        let submit_w = 64.0;
        let submit = Rect {
            origin: Point2D::new(left + inner_w - submit_w, btn_y),
            size: Point2D::new(submit_w, 24.0),
        };
        let cancel_w = 56.0;
        let cancel = Rect {
            origin: Point2D::new(submit.origin.x - 8.0 - cancel_w, btn_y),
            size: Point2D::new(cancel_w, 24.0),
        };
        (input, submit, cancel)
    }

    /// Paint the branch-picker dropdown.
    pub(in crate::widgets) fn paint_branch_picker(&self, cx: &mut PaintCx<'_>, panel_rect: Rect) {
        let t = self.theme;
        let panel = self.branch_picker_panel(panel_rect);
        cx.backend.fill_round_rect(panel, 8.0, t.popover);
        cx.backend.stroke_round_rect(panel, 8.0, t.border, 1.0);

        let mode = self.state.branch_picker_mode;
        // Header label per mode (TS list / create / merge headings). Merge
        // uses "合并到 {name}" (git.branch.mergeHeading) with the current
        // branch substituted, not the generic "合并分支…" action label.
        let merge_heading;
        let heading: &str = match mode {
            GitBranchPickerMode::Create => self.t("git.branch.createAction"),
            GitBranchPickerMode::Merge => {
                merge_heading = self
                    .t("git.branch.mergeHeading")
                    .replace("{{name}}", self.state.branch.as_deref().unwrap_or(""));
                &merge_heading
            }
            GitBranchPickerMode::List => self.t("git.branch.listHeading"),
        };
        // Create mode has no header band in TS — paint the heading only for
        // list / merge.
        if mode != GitBranchPickerMode::Create {
            self.text(
                cx,
                heading,
                panel.origin.x + 10.0,
                panel.origin.y + MENU_PAD + 14.0,
                11.0,
                t.muted_foreground,
            );
        }

        // Create mode — inline name input + Cancel / Create buttons.
        if mode == GitBranchPickerMode::Create {
            let (input, submit, cancel) = self.branch_create_rects(panel);
            self.paint_menu_input(
                cx,
                input,
                &self.state.branch_create_input,
                self.t("git.branch.createPlaceholder"),
                self.state.branch_create_focused,
            );
            // Ghost Cancel + primary Create, right-aligned (TS picker:271-285).
            self.paint_button_with_hit(
                cx,
                cancel,
                self.t("git.branch.cancel"),
                true,
                false,
                Some(GitPanelHit::BranchPickerCancel),
            );
            self.paint_button_with_hit(
                cx,
                submit,
                self.t("git.branch.createSubmit"),
                !self.state.branch_create_input.text().trim().is_empty(),
                true,
                Some(GitPanelHit::BranchCreateSubmit),
            );
            return;
        }

        // List / Merge — branch rows.
        let merging = mode == GitBranchPickerMode::Merge;
        let candidates = self.merge_candidate_indices();
        let rows = self.branch_picker_row_rects(panel_rect);
        for (i, row) in rows.iter().enumerate() {
            let bi = if merging { candidates[i] } else { i };
            let is_current = self.state.branches.get(bi) == self.state.branch.as_ref();
            let hovered = self.state.branch_picker_menu.hover == Some(i);
            let row_hit = if merging {
                GitPanelHit::MergeBranch(bi)
            } else {
                GitPanelHit::SwitchBranch(bi)
            };
            let pressed = self.is_pressed(row_hit);
            if (hovered || pressed) && !is_current {
                paint_button_feedback_wash(cx.backend, &self.theme, *row, 6.0, hovered, pressed);
            }
            let name = truncate(
                self.state
                    .branches
                    .get(bi)
                    .map(String::as_str)
                    .unwrap_or(""),
                BRANCH_NAME_MAX,
            );
            // Line 1 — branch name (always foreground; current branch is
            // signalled by the check, not by colour).
            self.text(
                cx,
                &name,
                row.origin.x + 10.0,
                row.origin.y + 16.0,
                12.0,
                t.foreground,
            );
            // Line 2 — last-commit subtitle (only the current branch's HEAD
            // commit is known here; others fall back to the no-commits label).
            let subtitle = if is_current {
                self.state
                    .recent_commits
                    .first()
                    .map(|c| c.summary.as_str())
                    .unwrap_or_else(|| self.t("git.branch.noCommits"))
            } else {
                self.t("git.branch.noCommits")
            };
            self.text(
                cx,
                &truncate(subtitle, BRANCH_NAME_MAX + 6),
                row.origin.x + 10.0,
                row.origin.y + 31.0,
                11.0,
                t.muted_foreground,
            );
            if merging {
                // TS merge-candidate row trailing glyph is a ChevronRight.
                draw_icon(
                    cx.backend,
                    Icon::ChevronRight,
                    Point2D::new(
                        row.origin.x + row.size.x - 22.0,
                        row.origin.y + (row.size.y - 12.0) / 2.0,
                    ),
                    12.0,
                    t.muted_foreground,
                    1.5,
                );
            } else if is_current {
                draw_icon(
                    cx.backend,
                    Icon::Check,
                    Point2D::new(
                        row.origin.x + row.size.x - 22.0,
                        row.origin.y + (row.size.y - 12.0) / 2.0,
                    ),
                    12.0,
                    t.foreground,
                    1.5,
                );
            } else {
                let merge = GitPanel::branch_merge_button(*row);
                draw_icon(
                    cx.backend,
                    Icon::GitBranch,
                    Point2D::new(merge.origin.x, merge.origin.y),
                    14.0,
                    t.muted_foreground,
                    1.5,
                );
            }
        }

        if merging {
            // Merge-mode footer hint (Escape returns to the list).
            let cy = panel.origin.y + panel.size.y - MENU_PAD - PICKER_FOOTER_H + 18.0;
            self.text(
                cx,
                self.t("git.branch.cancel"),
                panel.origin.x + 12.0,
                cy,
                12.0,
                t.muted_foreground,
            );
        } else {
            // List-mode footer: divider + 新建分支 / 合并分支 actions. The
            // labels paint at the shared `branch_footer_rects` so paint and
            // hit-test stay aligned. Labels resolve through op-i18n.
            let (create_r, merge_r) = self.branch_footer_rects(panel);
            cx.backend.fill_rect(
                Rect {
                    origin: Point2D::new(panel.origin.x + MENU_PAD, create_r.origin.y - 5.0),
                    size: Point2D::new(panel.size.x - MENU_PAD * 2.0, 1.0),
                },
                alpha(t.border, 0.60),
            );
            self.text(
                cx,
                self.t("git.branch.createAction"),
                create_r.origin.x + 6.0,
                create_r.origin.y + 16.0,
                12.0,
                t.foreground,
            );
            self.text(
                cx,
                self.t("git.branch.mergeAction"),
                merge_r.origin.x + 6.0,
                merge_r.origin.y + 16.0,
                12.0,
                t.foreground,
            );
        }
    }

    /// Hit-test the branch-picker dropdown. `None` when the point is
    /// outside the popover (the caller then closes it + falls through).
    pub(in crate::widgets) fn branch_picker_hit(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> Option<GitPanelHit> {
        let panel = self.branch_picker_panel(panel_rect);
        if !panel.contains(point) {
            return None;
        }
        match self.state.branch_picker_mode {
            GitBranchPickerMode::Create => {
                let (input, submit, cancel) = self.branch_create_rects(panel);
                if input.contains(point) {
                    return Some(GitPanelHit::BranchCreateInput);
                }
                if submit.contains(point) {
                    return Some(GitPanelHit::BranchCreateSubmit);
                }
                if cancel.contains(point) {
                    return Some(GitPanelHit::BranchPickerCancel);
                }
                Some(GitPanelHit::Inside)
            }
            GitBranchPickerMode::Merge => {
                let candidates = self.merge_candidate_indices();
                for (i, row) in self.branch_picker_row_rects(panel_rect).iter().enumerate() {
                    if row.contains(point) {
                        return Some(GitPanelHit::MergeBranch(candidates[i]));
                    }
                }
                // A click anywhere else in the popover (including the 取消
                // hint) cancels merge mode back to the branch list.
                Some(GitPanelHit::BranchPickerCancel)
            }
            GitBranchPickerMode::List => {
                for (i, row) in self.branch_picker_row_rects(panel_rect).iter().enumerate() {
                    if !row.contains(point) {
                        continue;
                    }
                    let is_current = self.state.branches.get(i) == self.state.branch.as_ref();
                    if is_current {
                        return Some(GitPanelHit::Inside);
                    }
                    if GitPanel::branch_merge_button(*row).contains(point) {
                        return Some(GitPanelHit::MergeBranch(i));
                    }
                    return Some(GitPanelHit::SwitchBranch(i));
                }
                let (create_r, merge_r) = self.branch_footer_rects(panel);
                if create_r.contains(point) {
                    return Some(GitPanelHit::BranchCreateMode);
                }
                if merge_r.contains(point) {
                    return Some(GitPanelHit::BranchMergeMode);
                }
                Some(GitPanelHit::Inside)
            }
        }
    }

    pub fn branch_picker_menu_hit(&self, panel_rect: Rect, point: Point2D) -> MenuHit {
        let panel = self.branch_picker_panel(panel_rect);
        if !panel.contains(point) {
            return MenuHit::Outside;
        }
        if self.state.branch_picker_mode == GitBranchPickerMode::Create {
            return MenuHit::Inside;
        }
        for (i, row) in self.branch_picker_row_rects(panel_rect).iter().enumerate() {
            if row.contains(point) {
                return MenuHit::Row(i);
            }
        }
        MenuHit::Inside
    }
}
