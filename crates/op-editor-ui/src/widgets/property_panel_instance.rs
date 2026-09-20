//! Component-instance controls in the property panel.
//!
//! A canonical `Ref` already stores its component target in `ref`.
//! This module exposes that field as an inline Swap list and keeps
//! paint, hit-test, and section-height geometry on one row model.

use crate::theme::Theme;
use crate::widgets::icons::{draw_icon, Icon};
use crate::widgets::property_panel_action::PropertyPanelAction;
use crate::widgets::property_panel_inputs::{
    create_component_block_height, COMPONENT_ACCENT, CREATE_COMPONENT_BTN_H, CREATE_COMPONENT_ICON,
    CREATE_COMPONENT_PAD_TOP, CREATE_COMPONENT_ROW_GAP, INSTANCE_ACCENT, PAD_X,
};
use crate::widgets::property_panel_sections::PropertyLabels;
use crate::widgets::property_panel_visibility::ComponentButtonState;
use crate::widgets::text_metrics;
use crate::widgets::PaintCx;
use crate::{Color, Point2D, Rect, TextLayout};

pub use op_editor_core::ComponentOption as InstanceComponentOption;

const OPTION_ROW_STEP: f32 = CREATE_COMPONENT_BTN_H + CREATE_COMPONENT_ROW_GAP;

#[derive(Debug)]
pub(crate) struct InstanceBlockRects {
    pub swap_trigger: Option<Rect>,
    first_option: Option<Rect>,
    option_count: usize,
    pub go_to_component: Option<Rect>,
    pub detach_instance: Option<Rect>,
}

impl InstanceBlockRects {
    pub(crate) fn option_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.option_count {
            return None;
        }
        let mut rect = self.first_option?;
        rect.origin.y += index as f32 * OPTION_ROW_STEP;
        Some(rect)
    }

    fn visible_option_range(&self, top: f32, bottom: f32) -> std::ops::Range<usize> {
        let Some(first) = self.first_option else {
            return 0..0;
        };
        if bottom <= first.origin.y
            || top >= first.origin.y + self.option_count as f32 * OPTION_ROW_STEP
        {
            return 0..0;
        }
        // Include at most one row straddling either clip edge. The backend
        // clip remains authoritative, while the loop stays viewport-sized.
        let start = (((top - first.origin.y) / OPTION_ROW_STEP).floor() as isize).max(0) as usize;
        let end =
            ((((bottom - first.origin.y) / OPTION_ROW_STEP).ceil() as isize) + 1).max(0) as usize;
        start.min(self.option_count)..end.min(self.option_count)
    }
}

fn row_rect(x: f32, y: f32, width: f32) -> Rect {
    Rect {
        origin: Point2D::new(x + PAD_X, y),
        size: Point2D::new(width - PAD_X * 2.0, CREATE_COMPONENT_BTN_H),
    }
}

/// Geometry shared by component-block paint and hit testing.
pub(crate) fn block_rects(
    x: f32,
    y: f32,
    width: f32,
    state: ComponentButtonState,
) -> InstanceBlockRects {
    let mut row_y = y + CREATE_COMPONENT_PAD_TOP;
    let step = CREATE_COMPONENT_BTN_H + CREATE_COMPONENT_ROW_GAP;
    match state {
        ComponentButtonState::Create | ComponentButtonState::DetachComponent => {
            InstanceBlockRects {
                swap_trigger: None,
                first_option: None,
                option_count: 0,
                go_to_component: None,
                detach_instance: None,
            }
        }
        ComponentButtonState::Instance {
            component_count,
            picker_open,
        } => {
            let swap_trigger = (component_count > 0).then(|| {
                let rect = row_rect(x, row_y, width);
                row_y += step;
                rect
            });
            let first_option =
                (picker_open && component_count > 0).then(|| row_rect(x, row_y, width));
            let option_count = if picker_open { component_count } else { 0 };
            row_y += option_count as f32 * step;
            let go_to_component = Some(row_rect(x, row_y, width));
            row_y += step;
            let detach_instance = Some(row_rect(x, row_y, width));
            InstanceBlockRects {
                swap_trigger,
                first_option,
                option_count,
                go_to_component,
                detach_instance,
            }
        }
    }
}

pub(crate) fn action_rects(
    x: f32,
    y: f32,
    width: f32,
    state: ComponentButtonState,
) -> Vec<(PropertyPanelAction, Rect)> {
    let primary = row_rect(x, y + CREATE_COMPONENT_PAD_TOP, width);
    match state {
        ComponentButtonState::Create => vec![(PropertyPanelAction::CreateComponent, primary)],
        ComponentButtonState::DetachComponent => {
            vec![(PropertyPanelAction::DetachComponent, primary)]
        }
        ComponentButtonState::Instance { .. } => {
            let rows = block_rects(x, y, width, state);
            let mut actions = Vec::with_capacity(3);
            if let Some(rect) = rows.swap_trigger {
                actions.push((PropertyPanelAction::ToggleInstanceComponentPicker, rect));
            }
            if let Some(rect) = rows.go_to_component {
                actions.push((PropertyPanelAction::GoToComponent, rect));
            }
            if let Some(rect) = rows.detach_instance {
                actions.push((PropertyPanelAction::DetachInstance, rect));
            }
            actions
        }
    }
}

pub(crate) fn option_index_at(
    x: f32,
    y: f32,
    width: f32,
    state: ComponentButtonState,
    point: Point2D,
) -> Option<usize> {
    let rows = block_rects(x, y, width, state);
    let first = rows.first_option?;
    let relative_y = point.y - first.origin.y;
    if relative_y < 0.0 {
        return None;
    }
    let index = (relative_y / OPTION_ROW_STEP).floor() as usize;
    rows.option_rect(index)
        .filter(|rect| rect.contains(point))
        .map(|_| index)
}

#[allow(clippy::too_many_arguments)]
pub fn paint_component_block(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    labels: &PropertyLabels,
    state: ComponentButtonState,
    options: &[InstanceComponentOption],
    current_target: Option<&str>,
    visible_top: f32,
    visible_bottom: f32,
    x: f32,
    y: f32,
    width: f32,
) -> f32 {
    let rows = block_rects(x, y, width, state);
    match state {
        ComponentButtonState::Create => paint_button(
            cx,
            theme,
            row_rect(x, y + CREATE_COMPONENT_PAD_TOP, width),
            labels.create_component,
            Icon::Component,
            theme.foreground,
            true,
            None,
        ),
        ComponentButtonState::DetachComponent => paint_button(
            cx,
            theme,
            row_rect(x, y + CREATE_COMPONENT_PAD_TOP, width),
            labels.detach_component,
            Icon::Diamond,
            COMPONENT_ACCENT,
            true,
            None,
        ),
        ComponentButtonState::Instance { picker_open, .. } => {
            if let Some(rect) = rows.swap_trigger {
                if rect.origin.y < visible_bottom && rect.origin.y + rect.size.y > visible_top {
                    let current = current_target
                        .and_then(|target| options.iter().find(|option| option.id == target))
                        .map(|option| option.name.as_str())
                        .or(current_target)
                        .unwrap_or("Missing component");
                    let label = format!("{}: {current}", labels.swap_component);
                    paint_button(
                        cx,
                        theme,
                        rect,
                        &label,
                        Icon::Component,
                        INSTANCE_ACCENT,
                        false,
                        Some(if picker_open {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        }),
                    );
                }
            }
            for index in rows.visible_option_range(visible_top, visible_bottom) {
                let Some(rect) = rows.option_rect(index) else {
                    continue;
                };
                if rect.origin.y >= visible_bottom || rect.origin.y + rect.size.y <= visible_top {
                    continue;
                }
                let Some(option) = options.get(index) else {
                    break;
                };
                let active = current_target == Some(option.id.as_str());
                paint_button(
                    cx,
                    theme,
                    rect,
                    &option.name,
                    if active { Icon::Check } else { Icon::Component },
                    if active {
                        INSTANCE_ACCENT
                    } else {
                        theme.foreground
                    },
                    false,
                    None,
                );
            }
            if let Some(rect) = rows.go_to_component {
                if rect.origin.y < visible_bottom && rect.origin.y + rect.size.y > visible_top {
                    paint_button(
                        cx,
                        theme,
                        rect,
                        labels.go_to_component,
                        Icon::Component,
                        INSTANCE_ACCENT,
                        true,
                        None,
                    );
                }
            }
            if let Some(rect) = rows.detach_instance {
                if rect.origin.y < visible_bottom && rect.origin.y + rect.size.y > visible_top {
                    paint_button(
                        cx,
                        theme,
                        rect,
                        labels.detach_instance,
                        Icon::Diamond,
                        INSTANCE_ACCENT,
                        true,
                        None,
                    );
                }
            }
        }
    }
    y + create_component_block_height(state)
}

#[allow(clippy::too_many_arguments)]
fn paint_button(
    cx: &mut PaintCx<'_>,
    theme: &Theme,
    rect: Rect,
    label_text: &str,
    icon_glyph: Icon,
    accent: Color,
    centered: bool,
    trailing_icon: Option<Icon>,
) {
    cx.backend.fill_round_rect(rect, 8.0, theme.muted);
    cx.backend.stroke_round_rect(rect, 8.0, theme.border, 1.0);
    let icon_y = rect.origin.y + (rect.size.y - CREATE_COMPONENT_ICON) / 2.0;
    draw_icon(
        cx.backend,
        icon_glyph,
        Point2D::new(rect.origin.x + 12.0, icon_y),
        CREATE_COMPONENT_ICON,
        accent,
        1.3,
    );
    if let Some(icon) = trailing_icon {
        draw_icon(
            cx.backend,
            icon,
            Point2D::new(
                rect.origin.x + rect.size.x - 12.0 - CREATE_COMPONENT_ICON,
                icon_y,
            ),
            CREATE_COMPONENT_ICON,
            accent,
            1.3,
        );
    }
    // The clip below is the button's text column; fit the label to it so a
    // long localized label ellipsizes inside the button instead of being
    // sheared by that clip.
    let text_clip = Rect {
        origin: Point2D::new(rect.origin.x + 34.0, rect.origin.y),
        size: Point2D::new((rect.size.x - 68.0).max(0.0), rect.size.y),
    };
    // The centred placement below carries a +12 leading-icon offset, so the
    // label's budget is the clip less that offset at BOTH ends — otherwise a
    // label fitted to the full clip is shifted right out of it.
    const CENTRED_LABEL_OFFSET: f32 = 12.0;
    let label_text = text_metrics::fit_chrome(
        cx.backend,
        label_text,
        (text_clip.size.x - CENTRED_LABEL_OFFSET * 2.0).max(0.0),
        13.0,
    );
    let label = TextLayout::single_run(
        &label_text,
        "system-ui",
        13.0,
        accent.to_jian(),
        Point2D::new(0.0, 0.0),
    );
    let text_x = if centered {
        rect.origin.x
            + (rect.size.x - text_metrics::measure_chrome(cx.backend, &label_text, 13.0)) / 2.0
            + 12.0
    } else {
        rect.origin.x + 36.0
    };
    cx.backend.save();
    cx.backend.clip_rect(text_clip);
    cx.backend.draw_text(
        &label,
        Point2D::new(text_x, rect.origin.y + rect.size.y / 2.0 + 4.5),
    );
    cx.backend.restore();
}
