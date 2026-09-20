//! Image-fill section tests for `widgets::property_panel`: the fill
//! body opening the editor popover, painting the selected image
//! preview / thumbnail, i18n labels, and the popover's upload + fit-
//! mode hit rects.

use super::property_panel::{PropertyPanel, PropertyPanelAction};
use super::property_panel_sections as sections;
use super::property_panel_test_support::{state_from, visible_for, CountingBackend};
use crate::widgets::{PaintCx, Widget};
use crate::{ImageDrawMode, Point2D, Rect};
use op_editor_core::NodeId;

fn image_fill_state_with_url(url: &str) -> op_editor_core::EditorState {
    let mut state = state_from(&format!(
        r##"{{ "version": "1.0.0", "children": [
              {{"type":"rectangle","id":"n60","name":"Photo fill",
               "x":40,"y":40,"width":180,"height":120,
               "fill":[{{"type":"image","url":"{}","mode":"fill",
                 "exposure":0,"contrast":0,"saturation":0,
                 "temperature":0,"tint":0,"highlights":0,"shadows":0}}]}}
        ]}}"##,
        url
    ));
    state.set_single_selection(NodeId::new("n60"));
    state
}

fn image_fill_state() -> op_editor_core::EditorState {
    image_fill_state_with_url("")
}

/// The drag-and-drop path lands an image fill on a **frame**, not a
/// rectangle, and frames report `caps.image == false`. Every other test
/// in this file uses a rectangle, so a frame-only regression in the fill
/// section would surface as "the dropped image cannot be adjusted" with
/// no failing test. Assert the whole reach: fill row -> popover -> mode
/// chips -> crop becomes drag-editable.
#[test]
fn dropped_image_fill_on_a_frame_reaches_the_mode_chips_and_crop_editing() {
    const PNG_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    let mut state = state_from(
        r##"{"version":"1.0.0","children":[{
            "type":"frame","id":"drop-frame","name":"Hero",
            "x":0,"y":0,"width":320,"height":180,"children":[]
        }]}"##,
    );
    let target = NodeId::new("drop-frame");
    assert!(
        state.apply_image_drop(&target, PNG_DATA_URL, Some([1600.0, 400.0])),
        "a frame must accept a dropped image fill",
    );

    let panel = PropertyPanel::for_selection(&state).expect("frame image fill panel");
    let rect = Rect {
        origin: Point2D::new(320.0, 24.0),
        size: Point2D::new(280.0, 900.0),
    };
    let body = sections::action_button_rects_with_fill_picker(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    )
    .into_iter()
    .find_map(|(a, r)| matches!(a, PropertyPanelAction::ToggleImageFillPopover).then_some(r))
    .expect("a frame carrying an image fill exposes the image fill row");
    assert!(matches!(
        panel.hit_test_action(
            rect,
            Point2D::new(
                body.origin.x + body.size.x / 2.0,
                body.origin.y + body.size.y / 2.0
            )
        ),
        Some(PropertyPanelAction::ToggleImageFillPopover)
    ));

    // Every schema mode must be offered, not just cover.
    state.editor_ui.image_fill_popover_open = true;
    let panel = PropertyPanel::for_selection(&state).expect("frame image fill panel");
    let popover =
        sections::image_fill_popover_action_rects(rect, visible_for(&panel), &panel.snapshot);
    for mode in op_editor_core::ImageFillMode::ALL {
        let chip = popover
            .iter()
            .find_map(|(a, r)| {
                matches!(a, PropertyPanelAction::SetImageFillMode(m) if *m == mode).then_some(*r)
            })
            .unwrap_or_else(|| panic!("popover must offer the {mode:?} chip"));
        assert!(
            matches!(
                panel.hit_test_action(
                    rect,
                    Point2D::new(
                        chip.origin.x + chip.size.x / 2.0,
                        chip.origin.y + chip.size.y / 2.0
                    )
                ),
                Some(PropertyPanelAction::SetImageFillMode(hit)) if hit == mode
            ),
            "the {mode:?} chip must dispatch its own mode",
        );
    }

    // A dropped fill starts as centered cover and is therefore not
    // repositionable; switching to Crop is what unlocks framing.
    assert!(!state.can_edit_selected_image_crop());
    assert!(state.set_selected_image_fill_mode(op_editor_core::ImageFillMode::Crop));
    assert!(
        state.can_edit_selected_image_crop(),
        "Crop mode on a dropped frame fill must become drag-editable",
    );
}

#[test]
fn image_fill_body_click_opens_the_image_popover() {
    let state = image_fill_state();
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");
    let rect = Rect {
        origin: Point2D::new(320.0, 24.0),
        size: Point2D::new(280.0, 900.0),
    };
    let rects = sections::action_button_rects_with_fill_picker(
        rect,
        visible_for(&panel),
        &panel.snapshot.effects,
        &panel.snapshot.fills,
        &panel.snapshot.interactions,
        false,
        0,
        false,
        false,
        false,
        false,
        false,
    );
    let body = rects
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::ToggleImageFillPopover))
        .map(|(_, r)| *r)
        .expect("image fill body emits popover toggle action");
    let center = Point2D::new(
        body.origin.x + body.size.x / 2.0,
        body.origin.y + body.size.y / 2.0,
    );
    assert!(
        matches!(
            panel.hit_test_action(rect, center),
            Some(PropertyPanelAction::ToggleImageFillPopover)
        ),
        "image fill body click should open the image editor popover",
    );
}

#[test]
fn open_image_fill_popover_paints_selected_image_preview() {
    const PNG_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    let mut state = image_fill_state_with_url(PNG_DATA_URL);
    state.editor_ui.image_fill_popover_open = true;
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");
    assert_eq!(
        panel
            .snapshot
            .image_fill
            .as_ref()
            .unwrap()
            .image_url
            .as_deref(),
        Some(PNG_DATA_URL),
    );

    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        let rect = Rect {
            origin: Point2D::new(320.0, 24.0),
            size: Point2D::new(280.0, 900.0),
        };
        panel.paint(&mut cx, rect);
        panel.paint_overlays(&mut cx, rect);
    }
    assert!(
        backend.images.iter().any(|(_, _, bytes)| *bytes > 0),
        "selected image data URL should be decoded and painted in the upload well",
    );
}

#[test]
fn image_fill_body_paints_selected_image_thumbnail_with_mode() {
    const PNG_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    let mut state = image_fill_state_with_url(PNG_DATA_URL);
    assert!(state.set_selected_image_fill_mode(op_editor_core::ImageFillMode::Tile));
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");

    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(
            &mut cx,
            Rect {
                origin: Point2D::new(320.0, 24.0),
                size: Point2D::new(280.0, 900.0),
            },
        );
    }

    assert!(
        backend.image_modes.contains(&ImageDrawMode::Tile),
        "fill body thumbnail should paint the selected image using the current image mode",
    );
}

#[test]
fn legacy_crop_thumbnail_forwards_affine_window_and_original_size() {
    const PNG_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    let mut state = state_from(&format!(
        r##"{{"version":"1.0.0","children":[{{
            "type":"rectangle","id":"legacy-crop","name":"Legacy crop",
            "x":0,"y":0,"width":191,"height":236,
            "fill":[{{"type":"image","url":"{PNG_DATA_URL}","mode":"stretch",
                "originalSize":{{"width":1179,"height":2556}},
                "transform":{{"m00":0.5089059,"m01":0.0,"m02":0.490246,
                    "m10":0.0,"m11":0.28951487,"m12":0.37636933}}}}]
        }}]}}"##
    ));
    state.set_single_selection(NodeId::new("legacy-crop"));
    state.editor_ui.locale = op_editor_core::Locale::ZhCn;
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");
    assert_eq!(
        panel.snapshot.image_fill.as_ref().unwrap().mode,
        op_editor_core::ImageFillMode::Crop,
    );

    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(
            &mut cx,
            Rect {
                origin: Point2D::new(320.0, 24.0),
                size: Point2D::new(280.0, 900.0),
            },
        );
    }

    assert_eq!(backend.image_modes, vec![ImageDrawMode::Crop]);
    assert_eq!(
        backend.image_transforms,
        vec![Some([
            0.5089059, 0.0, 0.490246, 0.0, 0.28951487, 0.37636933
        ])],
    );
    assert_eq!(backend.image_original_sizes, vec![Some([1179.0, 2556.0])]);
    assert!(
        backend.texts.iter().any(|text| text == "裁剪"),
        "the legacy transformed stretch row should expose Crop semantics",
    );
}

#[test]
fn tile_thumbnail_forwards_authored_scale() {
    const PNG_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/p9sAAAAASUVORK5CYII=";
    let mut state = state_from(&format!(
        r##"{{"version":"1.0.0","children":[{{
            "type":"rectangle","id":"tile","name":"Tile",
            "x":0,"y":0,"width":220,"height":220,
            "fill":[{{"type":"image","url":"{PNG_DATA_URL}","mode":"tile",
                "originalSize":{{"width":4096,"height":2048}},
                "tileScale":0.38618907}}]
        }}]}}"##
    ));
    state.set_single_selection(NodeId::new("tile"));
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");

    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint(
            &mut cx,
            Rect {
                origin: Point2D::new(320.0, 24.0),
                size: Point2D::new(280.0, 900.0),
            },
        );
    }

    assert_eq!(backend.image_modes, vec![ImageDrawMode::Tile]);
    assert_eq!(backend.image_original_sizes, vec![Some([4096.0, 2048.0])]);
    assert_eq!(backend.image_tile_scales, vec![0.38618907]);
}

#[test]
fn image_fill_adjustment_reset_label_uses_i18n() {
    let mut state = image_fill_state();
    state.editor_ui.image_fill_popover_open = true;
    state.editor_ui.locale = op_editor_core::Locale::ZhCn;
    assert!(
        state.set_selected_image_adjustment(op_editor_core::ImageAdjustmentField::Exposure, 36.0)
    );
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");

    let mut backend = CountingBackend::default();
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        panel.paint_overlays(
            &mut cx,
            Rect {
                origin: Point2D::new(320.0, 24.0),
                size: Point2D::new(280.0, 900.0),
            },
        );
    }
    assert!(backend.texts.iter().any(|s| s == "重置"));
    assert!(!backend.texts.iter().any(|s| s == "Reset"));
}

#[test]
fn open_image_fill_popover_routes_upload_and_mode_actions() {
    let mut state = image_fill_state();
    state.editor_ui.image_fill_popover_open = true;
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");
    let rect = Rect {
        origin: Point2D::new(320.0, 24.0),
        size: Point2D::new(280.0, 900.0),
    };
    let popup_rects =
        sections::image_fill_popover_action_rects(rect, visible_for(&panel), &panel.snapshot);
    let upload = popup_rects
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::PickFillImage))
        .map(|(_, r)| *r)
        .expect("open image popover exposes an upload hit rect");
    let upload_center = Point2D::new(
        upload.origin.x + upload.size.x / 2.0,
        upload.origin.y + upload.size.y / 2.0,
    );
    assert!(
        matches!(
            panel.hit_test_action(rect, upload_center),
            Some(PropertyPanelAction::PickFillImage)
        ),
        "upload well should trigger the image file picker",
    );
    let crop = popup_rects
        .iter()
        .find(|(a, _)| {
            matches!(
                a,
                PropertyPanelAction::SetImageFillMode(op_editor_core::ImageFillMode::Crop)
            )
        })
        .map(|(_, r)| *r)
        .expect("open image popover exposes fit-mode hit rects");
    let crop_center = Point2D::new(
        crop.origin.x + crop.size.x / 2.0,
        crop.origin.y + crop.size.y / 2.0,
    );
    assert!(
        matches!(
            panel.hit_test_action(rect, crop_center),
            Some(PropertyPanelAction::SetImageFillMode(
                op_editor_core::ImageFillMode::Crop
            ))
        ),
        "fit-mode chips should dispatch mode updates",
    );
}

#[test]
fn image_fill_popover_internal_gap_is_consumed_without_action() {
    let mut state = image_fill_state();
    state.editor_ui.image_fill_popover_open = true;
    let panel = PropertyPanel::for_selection(&state).expect("image fill panel");
    let rect = Rect {
        origin: Point2D::new(320.0, 24.0),
        size: Point2D::new(280.0, 900.0),
    };
    let popup_rects =
        sections::image_fill_popover_action_rects(rect, visible_for(&panel), &panel.snapshot);
    let upload = popup_rects
        .iter()
        .find(|(a, _)| matches!(a, PropertyPanelAction::PickFillImage))
        .map(|(_, r)| *r)
        .expect("upload rect exists");
    let gap = Point2D::new(upload.origin.x + 20.0, upload.origin.y - 5.0);

    assert_eq!(panel.hit_test_action(rect, gap), None);
    assert!(
        panel.image_fill_popover_contains(rect, gap),
        "clicks in non-interactive popover gaps must be consumed so the popover stays open",
    );
}

#[test]
fn tile_image_fill_exposes_scale_input_but_other_image_surfaces_do_not() {
    let rect = Rect {
        origin: Point2D::new(320.0, 24.0),
        size: Point2D::new(280.0, 900.0),
    };

    let mut tiled = image_fill_state();
    tiled.editor_ui.image_fill_popover_open = true;
    assert!(tiled.set_selected_image_fill_mode(op_editor_core::ImageFillMode::Tile));
    let tiled_panel = PropertyPanel::for_selection(&tiled).expect("tiled fill panel");
    assert_eq!(
        tiled_panel
            .snapshot
            .image_fill
            .as_ref()
            .and_then(|summary| summary.tile_scale),
        Some(1.0)
    );
    let tile_input = super::property_panel_image_fill::image_fill_popover_input_rect(
        rect,
        visible_for(&tiled_panel),
        &tiled_panel.snapshot,
    )
    .expect("Tile PenFill input rect");
    let tile_point = Point2D::new(
        tile_input.origin.x + tile_input.size.x / 2.0,
        tile_input.origin.y + tile_input.size.y / 2.0,
    );
    assert_eq!(
        tiled_panel.image_fill_popover_input_at(rect, tile_point),
        Some(op_editor_core::PropertyFocus::ImageTileScale)
    );

    let mut non_tile = image_fill_state();
    non_tile.editor_ui.image_fill_popover_open = true;
    let non_tile_panel = PropertyPanel::for_selection(&non_tile).expect("fill panel");
    assert!(
        super::property_panel_image_fill::image_fill_popover_input_rect(
            rect,
            visible_for(&non_tile_panel),
            &non_tile_panel.snapshot,
        )
        .is_none()
    );

    let mut standalone = state_from(
        r##"{"version":"1.0.0","children":[{
            "type":"image","id":"standalone","name":"Image",
            "x":0,"y":0,"width":100,"height":100,"src":""
        }]}"##,
    );
    standalone.set_single_selection(NodeId::new("standalone"));
    standalone.editor_ui.image_fill_popover_open = true;
    let standalone_panel = PropertyPanel::for_selection(&standalone).expect("image node panel");
    assert!(standalone_panel
        .snapshot
        .image_fill
        .as_ref()
        .is_some_and(|summary| summary.tile_scale.is_none()));
    assert!(
        super::property_panel_image_fill::image_fill_popover_input_rect(
            rect,
            visible_for(&standalone_panel),
            &standalone_panel.snapshot,
        )
        .is_none()
    );
}
