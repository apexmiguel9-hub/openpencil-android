//! Create-screen service-region selector: geometry, hit-testing, hover, and
//! the queued runtime action.

use super::*;
use op_editor_core::{
    CollabAvailability, CollabPanelHover, CollabPanelView, CollabRelayRegion, CollabUiAction,
    EditorUiState,
};

fn viewport() -> Rect {
    Rect::xywh(0.0, 0.0, 1_000.0, 800.0)
}

fn center(rect: Rect) -> Point2D {
    Point2D::new(
        rect.origin.x + rect.size.x / 2.0,
        rect.origin.y + rect.size.y / 2.0,
    )
}

#[test]
fn create_screen_region_options_hit_hover_and_stay_off_other_screens() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.panel.open = true;
    ui.collab.panel.view = CollabPanelView::Create;
    let panel = CollabPanel::for_editor_ui(&ui).unwrap();
    let rect = panel.rect_at(Rect::xywh(600.0, 8.0, 100.0, 26.0), viewport());
    let options = panel.region_option_rects(rect, panel.body_top(rect));
    assert_eq!(
        options
            .iter()
            .map(|(_, region)| *region)
            .collect::<Vec<_>>(),
        vec![CollabRelayRegion::China, CollabRelayRegion::Global]
    );
    for (button, region) in options {
        assert_eq!(
            panel.hit_test(rect, center(button)),
            Some(CollabPanelHit::Action(CollabUiAction::SetRelayRegion {
                region
            }))
        );
        assert_eq!(
            panel.hover_at(rect, center(button)),
            Some(CollabPanelHover::Region(region))
        );
    }

    // The options are create-screen geometry only; the home screen must not
    // hit-test or hover a region option.
    ui.collab.panel.view = CollabPanelView::Home;
    let panel = CollabPanel::for_editor_ui(&ui).unwrap();
    let rect = panel.rect_at(Rect::xywh(600.0, 8.0, 100.0, 26.0), viewport());
    assert!(panel
        .region_option_rects(rect, panel.body_top(rect))
        .is_empty());
}

#[test]
fn set_relay_region_hit_queues_the_runtime_action() {
    let mut ui = EditorUiState::default();
    ui.collab.availability = CollabAvailability::Ready;
    ui.collab.panel.open = true;
    ui.collab.panel.view = CollabPanelView::Create;
    assert!(crate::widgets::collab_ui::apply_panel_hit(
        &mut ui,
        CollabPanelHit::Action(CollabUiAction::SetRelayRegion {
            region: CollabRelayRegion::Global
        })
    ));
    assert_eq!(
        ui.collab.pending_action,
        Some(CollabUiAction::SetRelayRegion {
            region: CollabRelayRegion::Global
        })
    );
}
