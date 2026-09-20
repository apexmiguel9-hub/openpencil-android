//! The overflow `…` menu for [`GitPanel`] — the entry table, its
//! geometry, paint, and hit dispatch. Carved off `git_panel_menus.rs`
//! to keep every file under the 800-line cap.

use super::*;

impl GitPanel<'_> {
    /// The overflow menu's entries, top to bottom — a port of the TS
    /// header popover (`git-panel-header.tsx`): switch-tracked-file /
    /// clear-author / —— / remote-settings › / ssh-keys › / —— / close-repo.
    fn overflow_items(&self) -> Vec<OverflowItem> {
        vec![
            OverflowItem {
                icon: Icon::FileSearch,
                label_key: "git.header.overflowSwitchTracked",
                hit: GitPanelHit::OverflowSwitchTracked,
                submenu: false,
                divider_after: false,
            },
            OverflowItem {
                icon: Icon::UserX,
                label_key: "git.header.overflowClearAuthor",
                hit: GitPanelHit::OverflowClearAuthor,
                submenu: false,
                divider_after: true,
            },
            OverflowItem {
                icon: Icon::Settings2,
                label_key: "git.header.overflowRemoteSettings",
                hit: GitPanelHit::OverflowRemoteSettings,
                submenu: true,
                divider_after: false,
            },
            OverflowItem {
                icon: Icon::Key,
                label_key: "git.header.overflowSshKeys",
                hit: GitPanelHit::OverflowSshKeys,
                submenu: true,
                divider_after: true,
            },
            OverflowItem {
                icon: Icon::LogOut,
                label_key: "git.header.overflowCloseRepo",
                hit: GitPanelHit::OverflowCloseRepo,
                submenu: false,
                divider_after: false,
            },
        ]
    }

    /// Total extra height contributed by divider bands in the menu.
    fn overflow_dividers_height(&self) -> f32 {
        self.overflow_items()
            .iter()
            .filter(|it| it.divider_after)
            .count() as f32
            * OVERFLOW_DIVIDER_H
    }

    // ── Overflow menu ────────────────────────────────────────────────

    /// The overflow `…` menu rect, anchored below the overflow button
    /// and right-aligned to the panel edge.
    pub(in crate::widgets) fn overflow_panel(&self, panel_rect: Rect) -> Rect {
        let (_, _, overflow_btn) = self.ready_header_buttons(panel_rect);
        let items = self.overflow_items().len();
        let w = OVERFLOW_W.min(panel_rect.size.x - PAD * 2.0);
        let h = MENU_PAD * 2.0 + items as f32 * MENU_ROW_H + self.overflow_dividers_height();
        let right = overflow_btn.origin.x + overflow_btn.size.x;
        Rect {
            origin: Point2D::new(right - w, overflow_btn.origin.y + overflow_btn.size.y + 4.0),
            size: Point2D::new(w, h),
        }
    }

    /// One clickable rect per overflow-menu entry. The y-walk inserts a
    /// [`OVERFLOW_DIVIDER_H`] gap after any `divider_after` row so paint +
    /// hit-test agree on where each row lands.
    pub(in crate::widgets) fn overflow_row_rects(&self, panel_rect: Rect) -> Vec<Rect> {
        let panel = self.overflow_panel(panel_rect);
        let mut y = panel.origin.y + MENU_PAD;
        let mut rects = Vec::new();
        for item in self.overflow_items() {
            rects.push(Rect {
                origin: Point2D::new(panel.origin.x + MENU_PAD, y),
                size: Point2D::new(panel.size.x - MENU_PAD * 2.0, MENU_ROW_H),
            });
            y += MENU_ROW_H;
            if item.divider_after {
                y += OVERFLOW_DIVIDER_H;
            }
        }
        rects
    }

    /// Paint the overflow `…` menu (TS `git-panel-header.tsx` popover).
    pub(in crate::widgets) fn paint_overflow_menu(&self, cx: &mut PaintCx<'_>, panel_rect: Rect) {
        let t = self.theme;
        let panel = self.overflow_panel(panel_rect);
        cx.backend.fill_round_rect(panel, 8.0, t.popover);
        cx.backend.stroke_round_rect(panel, 8.0, t.border, 1.0);
        let rows = self.overflow_row_rects(panel_rect);
        for (i, (item, row)) in self.overflow_items().iter().zip(rows.iter()).enumerate() {
            let hovered = self.state.overflow_menu.hover == Some(i);
            let pressed = self.is_pressed(item.hit);
            if hovered || pressed {
                paint_button_feedback_wash(cx.backend, &self.theme, *row, 6.0, hovered, pressed);
            }
            // Leaf icon (TS size=13 strokeWidth=1.75, muted).
            draw_icon(
                cx.backend,
                item.icon,
                Point2D::new(row.origin.x + 8.0, row.origin.y + (row.size.y - 13.0) / 2.0),
                13.0,
                t.muted_foreground,
                1.75,
            );
            self.text(
                cx,
                self.t(item.label_key),
                row.origin.x + 30.0,
                row.origin.y + row.size.y / 2.0 + 4.0,
                12.0,
                t.foreground,
            );
            if item.submenu {
                draw_icon(
                    cx.backend,
                    Icon::ChevronRight,
                    Point2D::new(
                        row.origin.x + row.size.x - 18.0,
                        row.origin.y + (row.size.y - 12.0) / 2.0,
                    ),
                    12.0,
                    alpha(t.muted_foreground, 0.70),
                    1.5,
                );
            }
            // Divider band below the row (TS `<Separator className="my-1">`).
            if item.divider_after {
                let dy = row.origin.y + row.size.y + OVERFLOW_DIVIDER_H / 2.0;
                cx.backend.fill_rect(
                    Rect {
                        origin: Point2D::new(panel.origin.x + MENU_PAD, dy),
                        size: Point2D::new(panel.size.x - MENU_PAD * 2.0, 1.0),
                    },
                    alpha(t.border, 0.50),
                );
            }
        }
    }

    /// Hit-test the overflow menu. `None` when the point is outside the
    /// popover (the caller then closes it + falls through).
    pub(in crate::widgets) fn overflow_hit(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> Option<GitPanelHit> {
        match self.overflow_menu_hit(panel_rect, point) {
            MenuHit::Row(idx) => self.overflow_items().get(idx).map(|item| item.hit),
            MenuHit::Inside => Some(GitPanelHit::Inside),
            MenuHit::Outside => None,
        }
    }

    pub fn overflow_menu_hit(&self, panel_rect: Rect, point: Point2D) -> MenuHit {
        let panel = self.overflow_panel(panel_rect);
        if !panel.contains(point) {
            return MenuHit::Outside;
        }
        for (i, row) in self.overflow_row_rects(panel_rect).iter().enumerate() {
            if row.contains(point) {
                return MenuHit::Row(i);
            }
        }
        MenuHit::Inside
    }

    /// Paint whichever overflow view is active — the menu or a subview.
    pub(in crate::widgets) fn paint_overflow(&self, cx: &mut PaintCx<'_>, panel_rect: Rect) {
        match self.state.overflow_view {
            GitOverflowView::Menu => self.paint_overflow_menu(cx, panel_rect),
            GitOverflowView::RemoteSettings => self.paint_remote_settings(cx, panel_rect),
            GitOverflowView::TrackedPicker => self.paint_tracked_picker(cx, panel_rect),
            GitOverflowView::SshKeys => self.paint_ssh_keys(cx, panel_rect),
        }
    }

    /// Hit-test whichever overflow view is active.
    pub(in crate::widgets) fn overflow_hit_dispatch(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> Option<GitPanelHit> {
        match self.state.overflow_view {
            GitOverflowView::Menu => self.overflow_hit(panel_rect, point),
            GitOverflowView::RemoteSettings => self.remote_settings_hit(panel_rect, point),
            GitOverflowView::TrackedPicker => self.tracked_picker_hit(panel_rect, point),
            GitOverflowView::SshKeys => self.ssh_keys_hit(panel_rect, point),
        }
    }
}
