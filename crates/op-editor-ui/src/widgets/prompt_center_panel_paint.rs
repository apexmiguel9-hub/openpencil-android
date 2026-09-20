//! Prompt Center painting, split from geometry to keep both files compact.

use op_editor_core::prompt_center_catalog::PromptCategory;
use op_editor_core::PromptCenterFocus;

use super::{
    delete_hover_token, estimated_text_width, filter_hover_token, save_category_hover_token,
    PromptCenterCard, PromptCenterPanel, CHIP_H, CHIP_LABEL_SIZE, CLOSE_BTN, HEADER_H,
    PROMPT_CENTER_CANCEL_HOVER, PROMPT_CENTER_CLOSE_HOVER, PROMPT_CENTER_OPEN_SAVE_HOVER,
    PROMPT_CENTER_SAVE_HOVER, SEARCH_PAD_X, SEARCH_TEXT_SIZE, TITLE_SIZE,
};
use crate::widgets::button::paint_button_feedback_wash;
use crate::widgets::canvas_viewport_image::{
    has_cached_image_bytes, note_pending_decode, required_raster_edge, store_remote_image_bytes,
};
use crate::widgets::prompt_center_previews::prompt_center_preview;
use crate::widgets::property_panel_text_input::paint_text_input_view;
use crate::widgets::{draw_icon, Icon, PaintCx};
use crate::{Color, ImageDrawMode, Point2D, Rect, TextLayout};

/// Corner radius of the gallery frame itself, matching the Asset Center's.
/// Larger than a dropdown's because the shape is read at canvas scale, not
/// at menu scale.
const PANEL_RADIUS: f32 = 16.0;
const CARD_RADIUS: f32 = 12.0;
const CARD_TITLE_SIZE: f32 = 14.0;
const META_SIZE: f32 = 11.0;

impl PromptCenterPanel<'_> {
    /// Paint the complete non-modal panel.
    pub fn paint(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        cx.backend
            .fill_round_rect(panel, PANEL_RADIUS, self.theme.popover);
        cx.backend
            .stroke_round_rect(panel, PANEL_RADIUS, self.theme.border, 1.0);
        self.paint_header(cx, panel);
        self.paint_search(cx, panel);
        self.paint_filter_chips(cx, panel);
        if self.state.editor_ui.prompt_center.save_open {
            self.paint_save_form(cx, panel);
        }
        self.paint_cards(cx, panel);
    }

    fn paint_header(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        let content = Self::content_rect(panel);
        self.paint_text(
            cx,
            self.t("promptCenter.title"),
            Point2D::new(
                content.origin.x,
                jian_widgets::centered_text_baseline_y(
                    Rect::xywh(content.origin.x, panel.origin.y, content.size.x, HEADER_H),
                    TITLE_SIZE,
                ),
            ),
            TITLE_SIZE,
            self.theme.foreground,
        );

        if let Some(rect) = self.save_current_rect(panel) {
            cx.backend.fill_round_rect(rect, 7.0, self.theme.muted);
            paint_button_feedback_wash(
                cx.backend,
                &self.theme,
                rect,
                7.0,
                self.state.editor_ui.prompt_center.hover == Some(PROMPT_CENTER_OPEN_SAVE_HOVER),
                self.is_pressed(PROMPT_CENTER_OPEN_SAVE_HOVER),
            );
            let glyph = 15.0;
            draw_icon(
                cx.backend,
                Icon::Save,
                Point2D::new(
                    rect.origin.x + 9.0,
                    rect.origin.y + (rect.size.y - glyph) / 2.0,
                ),
                glyph,
                self.theme.muted_foreground,
                1.4,
            );
            let label = truncate_to_width(
                self.t("promptCenter.saveCurrent"),
                rect.size.x - 38.0,
                CHIP_LABEL_SIZE,
            );
            self.paint_text(
                cx,
                &label,
                Point2D::new(
                    rect.origin.x + 30.0,
                    jian_widgets::centered_text_baseline_y(rect, CHIP_LABEL_SIZE),
                ),
                CHIP_LABEL_SIZE,
                self.theme.foreground,
            );
        }

        let close = Self::close_rect(panel);
        jian_widgets::components::icon_button::IconButton {
            icon_paths: Icon::Close.paths(),
            hovered: self.state.editor_ui.prompt_center.hover == Some(PROMPT_CENTER_CLOSE_HOVER),
            pressed: self.is_pressed(PROMPT_CENTER_CLOSE_HOVER),
            active: false,
            enabled: true,
            icon_size: CLOSE_BTN - 14.0,
            stroke_width: 1.5,
        }
        .paint(
            cx.backend,
            close,
            &crate::widgets::button::tokens_from_theme(&self.theme),
        );

        cx.backend.fill_rect(
            Rect::xywh(panel.origin.x, panel.origin.y + HEADER_H, panel.size.x, 1.0),
            self.theme.border,
        );
    }

    fn paint_search(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        let rect = Self::search_rect(panel);
        cx.backend.fill_round_rect(rect, 9.0, self.theme.muted);
        cx.backend
            .stroke_round_rect(rect, 9.0, self.theme.border, 1.0);
        draw_icon(
            cx.backend,
            Icon::Search,
            Point2D::new(rect.origin.x + 10.0, rect.origin.y + 11.0),
            16.0,
            self.theme.muted_foreground,
            1.4,
        );
        paint_text_input_view(
            cx,
            &self.theme,
            &self.state.editor_ui.prompt_center.search,
            rect,
            SEARCH_TEXT_SIZE,
            SEARCH_PAD_X,
            jian_widgets::centered_text_baseline_y(rect, SEARCH_TEXT_SIZE),
            self.now_ms,
            self.t("promptCenter.searchPlaceholder"),
            self.state.editor_ui.prompt_center.focus == PromptCenterFocus::Search,
        );
    }

    fn paint_filter_chips(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        for (index, (rect, filter)) in self.filter_chip_rects(panel).into_iter().enumerate() {
            let active = self.state.editor_ui.prompt_center.filter == filter;
            let (fill, foreground) = if active {
                (self.theme.primary, self.theme.primary_foreground)
            } else {
                (self.theme.muted, self.theme.muted_foreground)
            };
            cx.backend.fill_round_rect(rect, CHIP_H / 2.0, fill);
            paint_button_feedback_wash(
                cx.backend,
                &self.theme,
                rect,
                CHIP_H / 2.0,
                self.state.editor_ui.prompt_center.hover == Some(filter_hover_token(index)),
                self.is_pressed(filter_hover_token(index)),
            );
            let label = truncate_to_width(
                self.filter_label(filter),
                rect.size.x - 14.0,
                CHIP_LABEL_SIZE,
            );
            let label_w = estimated_text_width(&label, CHIP_LABEL_SIZE);
            self.paint_text(
                cx,
                &label,
                Point2D::new(
                    rect.origin.x + ((rect.size.x - label_w) / 2.0).max(5.0),
                    jian_widgets::centered_text_baseline_y(rect, CHIP_LABEL_SIZE),
                ),
                CHIP_LABEL_SIZE,
                foreground,
            );
        }
    }

    fn paint_save_form(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        let form_top = Self::save_title_rect(panel).origin.y - 8.0;
        let form = Rect::xywh(panel.origin.x, form_top, panel.size.x, super::SAVE_FORM_H);
        cx.backend
            .fill_rect(form, self.theme.muted.with_alpha(0.35));
        cx.backend.fill_rect(
            Rect::xywh(form.origin.x, form.origin.y, form.size.x, 1.0),
            self.theme.border,
        );
        cx.backend.fill_rect(
            Rect::xywh(
                form.origin.x,
                form.origin.y + form.size.y - 1.0,
                form.size.x,
                1.0,
            ),
            self.theme.border,
        );

        let title = Self::save_title_rect(panel);
        cx.backend.fill_round_rect(title, 6.0, self.theme.popover);
        cx.backend
            .stroke_round_rect(title, 6.0, self.theme.border, 1.0);
        paint_text_input_view(
            cx,
            &self.theme,
            &self.state.editor_ui.prompt_center.save_title,
            title,
            SEARCH_TEXT_SIZE,
            10.0,
            jian_widgets::centered_text_baseline_y(title, SEARCH_TEXT_SIZE),
            self.now_ms,
            self.t("promptCenter.saveTitlePlaceholder"),
            self.state.editor_ui.prompt_center.focus == PromptCenterFocus::SaveTitle,
        );

        self.paint_form_button(
            cx,
            Self::cancel_button_rect(panel),
            self.t("promptCenter.cancel"),
            self.state.editor_ui.prompt_center.hover == Some(PROMPT_CENTER_CANCEL_HOVER),
            self.is_pressed(PROMPT_CENTER_CANCEL_HOVER),
            true,
        );
        self.paint_form_button(
            cx,
            Self::save_button_rect(panel),
            self.t("promptCenter.save"),
            self.state.editor_ui.prompt_center.hover == Some(PROMPT_CENTER_SAVE_HOVER),
            self.is_pressed(PROMPT_CENTER_SAVE_HOVER),
            self.can_commit_save(),
        );

        for (index, (rect, category)) in self.save_category_rects(panel).into_iter().enumerate() {
            let active = self.state.editor_ui.prompt_center.save_category == category;
            let fill = if active {
                self.theme.row_selected_primary
            } else {
                self.theme.popover
            };
            cx.backend.fill_round_rect(rect, CHIP_H / 2.0, fill);
            cx.backend
                .stroke_round_rect(rect, CHIP_H / 2.0, self.theme.border, 1.0);
            paint_button_feedback_wash(
                cx.backend,
                &self.theme,
                rect,
                CHIP_H / 2.0,
                self.state.editor_ui.prompt_center.hover == Some(save_category_hover_token(index)),
                self.is_pressed(save_category_hover_token(index)),
            );
            let label =
                truncate_to_width(self.category_label(category), rect.size.x - 14.0, META_SIZE);
            let label_w = estimated_text_width(&label, META_SIZE);
            self.paint_text(
                cx,
                &label,
                Point2D::new(
                    rect.origin.x + ((rect.size.x - label_w) / 2.0).max(5.0),
                    jian_widgets::centered_text_baseline_y(rect, META_SIZE),
                ),
                META_SIZE,
                if active {
                    self.theme.foreground
                } else {
                    self.theme.muted_foreground
                },
            );
        }
    }

    fn paint_form_button(
        &self,
        cx: &mut PaintCx<'_>,
        rect: Rect,
        label: &str,
        hovered: bool,
        pressed: bool,
        enabled: bool,
    ) {
        let fill = if enabled {
            self.theme.primary
        } else {
            self.theme.muted
        };
        cx.backend.fill_round_rect(rect, 6.0, fill);
        if enabled {
            paint_button_feedback_wash(cx.backend, &self.theme, rect, 6.0, hovered, pressed);
        }
        let text = truncate_to_width(label, rect.size.x - 12.0, CHIP_LABEL_SIZE);
        let width = estimated_text_width(&text, CHIP_LABEL_SIZE);
        self.paint_text(
            cx,
            &text,
            Point2D::new(
                rect.origin.x + ((rect.size.x - width) / 2.0).max(5.0),
                jian_widgets::centered_text_baseline_y(rect, CHIP_LABEL_SIZE),
            ),
            CHIP_LABEL_SIZE,
            if enabled {
                self.theme.primary_foreground
            } else {
                self.theme.muted_foreground
            },
        );
    }

    fn paint_cards(&self, cx: &mut PaintCx<'_>, panel: Rect) {
        let cards = self.filtered();
        let viewport = self.cards_viewport(panel);
        if cards.is_empty() {
            self.paint_text(
                cx,
                self.t("promptCenter.empty"),
                Point2D::new(viewport.origin.x, viewport.origin.y + 28.0),
                12.0,
                self.theme.muted_foreground,
            );
            return;
        }

        let bottom = viewport.origin.y + viewport.size.y;
        cx.backend.save();
        cx.backend.clip_rect(viewport);
        for (index, rect) in self.card_rects_for_count(panel, cards.len()) {
            if rect.origin.y + rect.size.y <= viewport.origin.y || rect.origin.y >= bottom {
                continue;
            }
            self.paint_card(cx, index, rect, &cards[index]);
        }
        cx.backend.restore();
    }

    fn paint_card(
        &self,
        cx: &mut PaintCx<'_>,
        index: usize,
        rect: Rect,
        card: &PromptCenterCard<'_>,
    ) {
        cx.backend
            .fill_round_rect(rect, CARD_RADIUS, self.theme.card);
        cx.backend
            .stroke_round_rect(rect, CARD_RADIUS, self.theme.border, 1.0);

        let preview = Self::card_preview_rect(rect);
        self.paint_card_preview(cx, preview, card);
        paint_button_feedback_wash(
            cx.backend,
            &self.theme,
            rect,
            CARD_RADIUS,
            self.state.editor_ui.prompt_center.hover == Some(index),
            self.is_pressed(index),
        );

        let title_right_pad = 14.0;
        let title = truncate_to_width(
            &card.title,
            rect.size.x - 14.0 - title_right_pad,
            CARD_TITLE_SIZE,
        );
        self.paint_text(
            cx,
            &title,
            Point2D::new(
                rect.origin.x + 14.0,
                preview.origin.y + preview.size.y + 24.0,
            ),
            CARD_TITLE_SIZE,
            self.theme.foreground,
        );

        let metadata = {
            let built_in = self.metadata(card);
            if built_in.is_empty() && card.custom {
                self.category_label(card.category).to_owned()
            } else {
                built_in
            }
        };
        if !metadata.is_empty() {
            self.paint_text(
                cx,
                &truncate_to_width(&metadata, rect.size.x - 28.0, META_SIZE),
                Point2D::new(rect.origin.x + 14.0, rect.origin.y + rect.size.y - 14.0),
                META_SIZE,
                self.theme.muted_foreground.with_alpha(0.85),
            );
        }

        if card.custom && self.state.editor_ui.prompt_center.custom_store_writable {
            let delete = Self::delete_rect(rect);
            cx.backend
                .fill_round_rect(delete, 6.0, self.theme.popover.with_alpha(0.88));
            paint_button_feedback_wash(
                cx.backend,
                &self.theme,
                delete,
                6.0,
                self.state.editor_ui.prompt_center.hover == Some(delete_hover_token(index)),
                self.is_pressed(delete_hover_token(index)),
            );
            draw_icon(
                cx.backend,
                Icon::Trash,
                Point2D::new(delete.origin.x + 5.0, delete.origin.y + 5.0),
                14.0,
                self.theme.destructive,
                1.4,
            );
        }
    }

    fn paint_card_preview(&self, cx: &mut PaintCx<'_>, preview: Rect, card: &PromptCenterCard<'_>) {
        let Some(asset) = prompt_center_preview(card.id.as_ref()) else {
            self.paint_preview_fallback(cx, preview, card);
            return;
        };
        let image_id = asset.image_id;
        let Some(encoded) = asset.bytes else {
            // Web only: the JPEG is not in the bundle and has not been fetched
            // yet. Ask the host for it and paint the same fallback an
            // unknown-id card gets — the card is readable either way, and a
            // failed fetch simply leaves it that way rather than blocking.
            op_editor_core::web_assets::request(asset.route);
            self.paint_preview_fallback(cx, preview, card);
            return;
        };
        if !has_cached_image_bytes(image_id) {
            store_remote_image_bytes(image_id, encoded.to_vec());
        }
        let max_edge_px = required_raster_edge(preview, cx.backend.dpi_scale());
        let sharp = cx.backend.image_decoded(image_id, encoded, max_edge_px);
        if !sharp {
            note_pending_decode(image_id, max_edge_px);
        }
        if !sharp && !cx.backend.image_resident(image_id) {
            self.paint_preview_fallback(cx, preview, card);
            return;
        }

        cx.backend.save();
        cx.backend.clip_round_rect(preview, 7.0);
        cx.backend
            .draw_image_with_mode(preview, image_id, encoded, ImageDrawMode::Fill);
        cx.backend.restore();
    }

    fn paint_preview_fallback(
        &self,
        cx: &mut PaintCx<'_>,
        preview: Rect,
        card: &PromptCenterCard<'_>,
    ) {
        let accent = match card.category {
            PromptCategory::Starter => self.theme.primary,
            PromptCategory::MobileApp => Color::rgb_u8(124, 92, 255),
            PromptCategory::WebPage => Color::rgb_u8(14, 165, 233),
            PromptCategory::Dashboard => self.theme.status_success,
            PromptCategory::Component => self.theme.status_warning,
            PromptCategory::Modify => Color::rgb_u8(236, 72, 153),
        };
        cx.backend.fill_round_rect_linear_gradient(
            preview,
            7.0,
            &[
                (0.0, accent.with_alpha(0.42)),
                (1.0, self.theme.muted.with_alpha(0.92)),
            ],
            135.0,
            1.0,
        );
        let icon = if card.custom {
            Icon::Pencil
        } else {
            match card.category {
                PromptCategory::Starter => Icon::Library,
                PromptCategory::MobileApp => Icon::Smartphone,
                PromptCategory::WebPage => Icon::Chrome,
                PromptCategory::Dashboard => Icon::LayoutGrid,
                PromptCategory::Component => Icon::Component,
                PromptCategory::Modify => Icon::Wand2,
            }
        };
        let icon_size = 34.0;
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                preview.origin.x + (preview.size.x - icon_size) / 2.0,
                preview.origin.y + (preview.size.y - icon_size) / 2.0,
            ),
            icon_size,
            self.theme.foreground.with_alpha(0.72),
            1.35,
        );
    }

    fn paint_text(
        &self,
        cx: &mut PaintCx<'_>,
        text: &str,
        position: Point2D,
        size: f32,
        color: Color,
    ) {
        let layout = TextLayout::single_run(
            text,
            "system-ui",
            size,
            color.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend.draw_text(&layout, position);
    }
}

fn truncate_to_width(text: &str, max_width: f32, size: f32) -> String {
    if estimated_text_width(text, size) <= max_width {
        return text.to_owned();
    }
    let ellipsis_width = estimated_text_width("…", size);
    let mut width = 0.0;
    let mut output = String::new();
    for ch in text.chars() {
        let advance = estimated_text_width(&ch.to_string(), size);
        if width + advance + ellipsis_width > max_width {
            break;
        }
        output.push(ch);
        width += advance;
    }
    output.push('…');
    output
}
