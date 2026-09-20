//! Page-only Design inspector shown when no canvas node is selected.

use crate::theme::Theme;
use crate::widgets::property_panel::{PropertyPanel, PropertyPanelAction};
use crate::widgets::property_panel_inputs::{
    paint_section_divider, paint_section_label, HEADER_HEIGHT, INPUT_HEIGHT, INPUT_RADIUS, PAD_X,
    SECTION_GAP, TAB_HEIGHT,
};
use crate::widgets::property_panel_sections::EditContext;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};
use op_editor_core::PropertyFocus;

const CLEAR_GAP: f32 = 8.0;
const CLEAR_W: f32 = 58.0;

pub fn background_input_rect(panel_rect: Rect, has_background: bool) -> Rect {
    let y = panel_rect.origin.y
        + TAB_HEIGHT
        + HEADER_HEIGHT
        + crate::widgets::property_panel_inputs::SECTION_HEADER_HEIGHT;
    let clear_space = if has_background {
        CLEAR_W + CLEAR_GAP
    } else {
        0.0
    };
    Rect::xywh(
        panel_rect.origin.x + PAD_X,
        y,
        panel_rect.size.x - PAD_X * 2.0 - clear_space,
        INPUT_HEIGHT,
    )
}

pub fn clear_rect(panel_rect: Rect, has_background: bool) -> Option<Rect> {
    has_background.then(|| {
        let input = background_input_rect(panel_rect, true);
        Rect::xywh(
            input.origin.x + input.size.x + CLEAR_GAP,
            input.origin.y,
            CLEAR_W,
            INPUT_HEIGHT,
        )
    })
}

pub fn content_height() -> f32 {
    TAB_HEIGHT
        + HEADER_HEIGHT
        + crate::widgets::property_panel_inputs::SECTION_HEADER_HEIGHT
        + INPUT_HEIGHT
        + 12.0
        + SECTION_GAP
}

#[allow(clippy::too_many_arguments)]
pub fn paint_page_inspector(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    locale: op_editor_core::Locale,
    page_name: &str,
    background: Option<&str>,
    panel_rect: Rect,
) {
    let x = panel_rect.origin.x;
    let width = panel_rect.size.x;
    let mut y = panel_rect.origin.y + TAB_HEIGHT;
    paint_page_header(cx, theme, page_name, x, y, width);
    y += HEADER_HEIGHT;
    y = paint_section_label(
        cx,
        theme,
        op_i18n::translate(locale, "page.background"),
        x,
        y,
        width,
    );
    paint_background_input(
        cx,
        theme,
        edit,
        background.unwrap_or_default(),
        background_input_rect(panel_rect, background.is_some()),
    );
    if let Some(clear) = clear_rect(panel_rect, background.is_some()) {
        jian_widgets::components::button::Button {
            label: op_i18n::translate(locale, "page.background.clear"),
            icon_paths: None,
            variant: jian_widgets::components::button::ButtonVariant::Secondary,
            enabled: true,
            hovered: false,
            pressed: false,
            font_size: 11.0,
        }
        .paint(
            cx.backend,
            clear,
            &crate::widgets::button::tokens_from_theme(theme),
        );
    }
    y += INPUT_HEIGHT + 12.0;
    paint_section_divider(cx, theme, x, y, width);
}

fn paint_page_header(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    page_name: &str,
    x: f32,
    y: f32,
    width: f32,
) {
    let text = if page_name.is_empty() {
        "Page"
    } else {
        page_name
    };
    let layout = TextLayout::single_run(
        text,
        "system-ui",
        13.0,
        theme.foreground.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    cx.backend
        .draw_text(&layout, Point2D::new(x + PAD_X, y + 20.0));
    cx.backend.fill_rect(
        Rect::xywh(x + PAD_X, y + HEADER_HEIGHT - 1.0, width - PAD_X * 2.0, 1.0),
        theme.border,
    );
}

fn paint_background_input(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    edit: &EditContext<'_>,
    fallback: &str,
    rect: Rect,
) {
    let focus = PropertyFocus::PageBackgroundHex;
    let focused = edit.focus == Some(focus);
    let value = edit.value_for(focus, fallback);
    cx.backend.fill_round_rect(rect, INPUT_RADIUS, theme.muted);
    cx.backend.stroke_round_rect(
        rect,
        INPUT_RADIUS,
        if focused { theme.primary } else { theme.border },
        if focused { 1.5 } else { 1.0 },
    );
    let swatch = Rect::xywh(rect.origin.x + 7.0, rect.origin.y + 7.0, 16.0, 16.0);
    let swatch_color = crate::widgets::property_panel_snapshot::color_from_hex(value)
        .unwrap_or(Color::TRANSPARENT);
    cx.backend.fill_round_rect(swatch, 3.0, swatch_color);
    cx.backend.stroke_round_rect(swatch, 3.0, theme.border, 1.0);
    let value_x = rect.origin.x + 31.0;
    let input_view = Rect::xywh(
        value_x,
        rect.origin.y,
        (rect.origin.x + rect.size.x - 8.0 - value_x).max(0.0),
        rect.size.y,
    );
    if !edit.paint_input_view_at(
        cx,
        theme,
        focus,
        input_view,
        12.0,
        0.0,
        rect.origin.y + 19.0,
    ) {
        edit.paint_selection_at(
            cx,
            theme,
            focus,
            value,
            value_x,
            rect.origin.y + 19.0,
            12.0,
            rect.origin.x + rect.size.x - 8.0,
        );
        let text = TextLayout::single_run(
            value,
            "system-ui",
            12.0,
            theme.foreground.to_jian(),
            Point2D::new(0.0, 0.0),
        );
        cx.backend
            .draw_text(&text, Point2D::new(value_x, rect.origin.y + 19.0));
        if let Some(pos) = edit.caret_at(focus) {
            let prefix = &value[..pos.min(value.len())];
            let caret_x = value_x + text_metrics::measure_chrome(cx.backend, prefix, 12.0);
            cx.backend.fill_rect(
                Rect::xywh(caret_x, rect.origin.y + 6.0, 1.5, rect.size.y - 12.0),
                theme.foreground,
            );
        }
    }
}

impl PropertyPanel {
    pub(crate) fn page_input_at(&self, panel_rect: Rect, point: Point2D) -> Option<PropertyFocus> {
        (self.page_only
            && background_input_rect(panel_rect, self.page_background.is_some()).contains(point))
        .then_some(PropertyFocus::PageBackgroundHex)
    }

    pub(crate) fn page_action_at(
        &self,
        panel_rect: Rect,
        point: Point2D,
    ) -> Option<PropertyPanelAction> {
        clear_rect(panel_rect, self.page_background.is_some())
            .filter(|rect| rect.contains(point))
            .map(|_| PropertyPanelAction::ClearPageBackground)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_button_only_exists_for_an_authored_background() {
        let panel = Rect::xywh(0.0, 0.0, 280.0, 500.0);
        assert!(clear_rect(panel, false).is_none());
        assert!(clear_rect(panel, true).is_some());
        assert!(
            background_input_rect(panel, false).size.x > background_input_rect(panel, true).size.x
        );
    }

    #[test]
    fn no_selection_does_not_implicitly_open_page_inspector() {
        let doc = jian_ops_schema::load_str(
            r##"{
                "version":"1.0.0",
                "pages":[{
                    "id":"page-a",
                    "name":"Canvas A",
                    "children":[],
                    "backgroundColor":"#d7e4f380"
                }]
            }"##,
        )
        .expect("page fixture")
        .value;
        let state = op_editor_core::EditorState::from_document(doc);
        assert!(PropertyPanel::for_selection(&state).is_none());
    }
}
