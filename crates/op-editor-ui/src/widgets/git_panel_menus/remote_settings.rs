//! The remote-settings subview reached from [`GitPanel`]'s overflow menu
//! — origin URL + HTTPS credential + SSH rows, and the geometry shared
//! by its paint and hit-test. Carved off `git_panel_menus.rs` to keep
//! every file under the 800-line cap.

use super::*;
use crate::widgets::text_metrics;

impl GitPanel<'_> {
    // ── Remote-settings subview ──────────────────────────────────────

    /// The remote-settings subview popover rect.
    pub(in crate::widgets) fn remote_settings_panel(&self, panel_rect: Rect) -> Rect {
        let (_, _, overflow_btn) = self.ready_header_buttons(panel_rect);
        let w = RS_W.min(panel_rect.size.x - PAD * 2.0);
        let top = overflow_btn.origin.y + overflow_btn.size.y + 4.0;
        // back + 远端 heading + optional empty hint + Origin-地址 label + URL
        // input row + the ahead/behind & credentials section.
        let h = (self.remote_url_top(top) - top) + RS_ROW_H + 8.0 + RS_SECTION;
        let right = overflow_btn.origin.x + overflow_btn.size.x;
        Rect {
            origin: Point2D::new(right - w, top),
            size: Point2D::new(w, h),
        }
    }

    /// Absolute y of the Origin-URL input — below the back row, the 远端
    /// heading, the optional "尚未配置远端仓库" hint, and the Origin-地址 label.
    fn remote_url_top(&self, panel_top: f32) -> f32 {
        let mut y = panel_top + RS_PAD + RS_BACK_H + 6.0; // 远端 heading row
        y += 18.0;
        if self.state.remotes.is_empty() {
            y += 18.0; // empty hint
        }
        y += 18.0; // Origin 地址 label
        y
    }

    /// Y of the first divider below the URL input.
    fn remote_settings_section_top(&self, p: Rect) -> f32 {
        self.remote_url_top(p.origin.y) + RS_ROW_H + 8.0
    }

    /// The "获取" (Fetch) button rect on the ahead/behind row.
    pub(in crate::widgets) fn remote_settings_fetch_rect(&self, panel_rect: Rect) -> Rect {
        let p = self.remote_settings_panel(panel_rect);
        let top = self.remote_settings_section_top(p);
        Rect {
            origin: Point2D::new(p.origin.x + p.size.x - RS_PAD - 44.0, top + 6.0),
            size: Point2D::new(44.0, 22.0),
        }
    }

    /// The subview's interactive sub-rects: `(back, url_input, set)` — the
    /// TS layout has no HTTPS-credential input here. Shared by paint +
    /// hit-test (+ tests).
    pub(in crate::widgets) fn remote_settings_rects(&self, panel_rect: Rect) -> (Rect, Rect, Rect) {
        let p = self.remote_settings_panel(panel_rect);
        let left = p.origin.x + RS_PAD;
        let inner_w = p.size.x - RS_PAD * 2.0;
        let back = Rect {
            origin: Point2D::new(left, p.origin.y + RS_PAD),
            size: Point2D::new(inner_w, RS_BACK_H),
        };
        let url_top = self.remote_url_top(p.origin.y);
        let field_w = inner_w - RS_BTN_W - RS_GAP;
        let url_input = Rect {
            origin: Point2D::new(left, url_top),
            size: Point2D::new(field_w, RS_ROW_H),
        };
        let set = Rect {
            origin: Point2D::new(left + field_w + RS_GAP, url_top),
            size: Point2D::new(RS_BTN_W, RS_ROW_H),
        };
        (back, url_input, set)
    }

    /// Paint the remote-settings subview.
    pub(in crate::widgets) fn paint_remote_settings(&self, cx: &mut PaintCx<'_>, panel_rect: Rect) {
        let t = self.theme;
        let p = self.remote_settings_panel(panel_rect);
        cx.backend.fill_round_rect(p, 8.0, t.popover);
        cx.backend.stroke_round_rect(p, 8.0, t.border, 1.0);
        let (back, url_input, set) = self.remote_settings_rects(panel_rect);
        let left = back.origin.x;
        // ← Back row.
        draw_icon(
            cx.backend,
            Icon::ChevronLeft,
            Point2D::new(left, back.origin.y + (back.size.y - 14.0) / 2.0),
            14.0,
            t.muted_foreground,
            1.5,
        );
        self.text(
            cx,
            self.t("git.remote.back"),
            left + 20.0,
            back.origin.y + back.size.y / 2.0 + 4.0,
            12.0,
            t.foreground,
        );
        // 远端 section heading (uppercase muted).
        let heading_y = back.origin.y + RS_BACK_H + 6.0;
        self.text(
            cx,
            self.t("git.remote.settingsHeading"),
            left,
            heading_y + 12.0,
            10.0,
            t.muted_foreground,
        );
        // Empty hint + Origin-地址 label above the URL input.
        let mut label_y = heading_y + 18.0;
        if self.state.remotes.is_empty() {
            self.text(
                cx,
                self.t("git.remote.emptyNoOrigin"),
                left,
                label_y + 12.0,
                11.0,
                t.muted_foreground,
            );
            label_y += 18.0;
        }
        self.text(
            cx,
            self.t("git.remote.urlLabel"),
            left,
            label_y + 12.0,
            11.0,
            t.muted_foreground,
        );
        // Origin-URL field + "Save".
        self.paint_menu_input(
            cx,
            url_input,
            &self.state.remote_input,
            self.t("git.remote.urlPlaceholder"),
            self.state.remote_focused,
        );
        self.paint_button_with_hit(
            cx,
            set,
            self.t("git.remote.saveButton"),
            !self.state.remote_input.text().trim().is_empty(),
            true,
            Some(GitPanelHit::SetRemote),
        );

        // ── Ahead/behind + Fetch + stored-credentials section (TS rows) ──
        let p = self.remote_settings_panel(panel_rect);
        let inner_w = p.size.x - RS_PAD * 2.0;
        let top = self.remote_settings_section_top(p);
        let divider = |cx: &mut PaintCx<'_>, y: f32| {
            cx.backend.fill_rect(
                Rect {
                    origin: Point2D::new(left, y),
                    size: Point2D::new(inner_w, 1.0),
                },
                alpha(t.border, 0.50),
            );
        };
        divider(cx, top);
        // 领先 N · 落后 N
        let ab = self
            .t("git.remote.aheadBehind")
            .replace("{{ahead}}", &self.state.ahead.to_string())
            .replace("{{behind}}", &self.state.behind.to_string());
        self.text(cx, &ab, left, top + 21.0, 11.0, t.muted_foreground);
        // 获取 button (enabled when a remote exists).
        self.paint_button_with_hit(
            cx,
            self.remote_settings_fetch_rect(panel_rect),
            self.t("git.remote.fetchButton"),
            !self.state.remotes.is_empty(),
            false,
            Some(GitPanelHit::FetchRemote),
        );
        // Credentials row.
        let d2 = top + 8.0 + 30.0;
        divider(cx, d2);
        self.text(
            cx,
            self.t("git.remote.storedAuthLabel"),
            left,
            d2 + 21.0,
            11.0,
            t.muted_foreground,
        );
        let status = if self.state.remote_host.is_none() {
            self.t("git.remote.storedAuth.noHost")
        } else {
            match self.state.stored_auth.as_str() {
                "ssh" => self.t("git.remote.storedAuth.ssh"),
                "token" => self.t("git.remote.storedAuth.token"),
                _ => self.t("git.remote.storedAuth.none"),
            }
        };
        let sw = text_metrics::measure_chrome(cx.backend, status, 11.0);
        self.text(
            cx,
            status,
            p.origin.x + p.size.x - RS_PAD - sw,
            d2 + 21.0,
            11.0,
            t.foreground,
        );
    }

    /// Hit-test the remote-settings subview.
    pub(in crate::widgets) fn remote_settings_hit(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> Option<GitPanelHit> {
        let p = self.remote_settings_panel(panel_rect);
        if !p.contains(point) {
            return None;
        }
        let (back, url_input, set) = self.remote_settings_rects(panel_rect);
        if back.contains(point) {
            return Some(GitPanelHit::OverflowBack);
        }
        if url_input.contains(point) {
            return Some(GitPanelHit::RemoteInput);
        }
        if set.contains(point) {
            return Some(GitPanelHit::SetRemote);
        }
        if !self.state.remotes.is_empty()
            && self.remote_settings_fetch_rect(panel_rect).contains(point)
        {
            return Some(GitPanelHit::FetchRemote);
        }
        Some(GitPanelHit::Inside)
    }
}
