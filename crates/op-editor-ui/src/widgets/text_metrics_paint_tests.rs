//! Cross-panel guard against family-blind text measurement.
//!
//! Every covered panel is painted **twice**: once into a backend where a
//! named family is 40% wider than the default face (the real macOS gap
//! between the bundled Roboto that `RenderBackend::measure_text` resolves and
//! the `.AppleSystemUIFont` a `system-ui` run actually paints), and once into
//! the control where the two agree — which is what every other test backend
//! models, and precisely why this bug class ships green.
//!
//! The assertion is the **difference**: widening the painted family must not
//! push a single new run outside its panel. A widget that fits, centres, or
//! sizes a container against the family-blind number fails immediately —
//! its fitter trims to the wrong budget, so the wider paint spills. A widget
//! that measures through [`crate::widgets::text_metrics`] trims to the real
//! budget and stays inside in both worlds.
//!
//! Diffing rather than asserting absolute containment is deliberate. Some
//! localized strings (long Russian / Hindi empty-state copy) are painted
//! without any fitter at all and overflow in *both* worlds; that is a real
//! but separate layout bug, and folding it in here would bury the signal this
//! guard exists to carry.
//!
//! This test is the asset, not the per-call-site assertions: any future
//! `measure_text` that creeps back into a covered panel fails here.

use op_editor_core::{EditorState, NodeId};

use crate::widgets::test_family_gap_backend::FamilyGapBackend;
use crate::widgets::{PaintCx, Widget};
use crate::{Point2D, Rect};

/// Paint `f` into `backend` and hand it back with everything it drew.
fn paint_into(mut backend: FamilyGapBackend, f: impl FnOnce(&mut PaintCx<'_>)) -> FamilyGapBackend {
    {
        let mut cx = PaintCx {
            backend: &mut backend,
        };
        f(&mut cx);
    }
    backend
}

/// Paint `f` under both faces and assert the wider one spills no further.
#[track_caller]
fn assert_no_new_overflow(what: &str, container: Rect, f: impl Fn(&mut PaintCx<'_>)) {
    let gap = paint_into(FamilyGapBackend::default(), &f);
    let control = paint_into(FamilyGapBackend::uniform(), &f);

    assert!(
        !gap.runs.is_empty(),
        "{what} painted no text — the guard would pass vacuously"
    );

    let gap_over = gap.overflowing(container);
    let control_over = control.overflowing(container);
    if gap_over.len() <= control_over.len() {
        return;
    }
    let detail: Vec<String> = gap_over
        .iter()
        .map(|run| {
            format!(
                "{:?} @x={} spans {}px in {:?} (clip {:?})",
                run.text,
                run.origin.x,
                run.width_in_paint_family(),
                run.family,
                run.clip.map(|c| (c.origin.x, c.origin.x + c.size.x)),
            )
        })
        .collect();
    panic!(
        "{what}: painting in the real (wider) family pushed {} run(s) outside \
         [{}, {}] versus {} in the control — something measured with \
         RenderBackend::measure_text instead of crate::widgets::text_metrics.\n  {}",
        gap_over.len(),
        container.origin.x,
        container.origin.x + container.size.x,
        control_over.len(),
        detail.join("\n  "),
    );
}

fn sample_state() -> EditorState {
    let mut state = EditorState::sample();
    state.set_single_selection(NodeId::new("n10"));
    state
}

#[test]
fn settings_modal_holds_its_content_column_in_the_painted_family() {
    use crate::widgets::agent_settings_panel::{content_viewport, AgentSettingsPanel};

    for tab in [
        op_editor_core::AgentSettingsTab::Agents,
        op_editor_core::AgentSettingsTab::Mcp,
        op_editor_core::AgentSettingsTab::Images,
        op_editor_core::AgentSettingsTab::System,
    ] {
        for locale in op_i18n::Locale::ALL {
            let mut state = EditorState::default();
            state.editor_ui.locale = locale;
            state.editor_ui.agent_settings.tab = tab;
            let panel = AgentSettingsPanel::for_editor(&state);
            let rect = panel.rect(1200.0, 800.0);

            // The modal rect, not the content column: the sidebar nav paints
            // left of the column and is part of the same modal.
            assert_no_new_overflow(&format!("settings modal {tab:?}/{locale:?}"), rect, |cx| {
                panel.paint(cx, rect)
            });
            assert_no_new_overflow(
                &format!("settings modal content column {tab:?}/{locale:?}"),
                content_viewport(rect),
                |cx| panel.paint(cx, rect),
            );
        }
    }
}

#[test]
fn property_panel_holds_the_rail_in_the_painted_family() {
    use crate::widgets::PropertyPanel;

    for locale in op_i18n::Locale::ALL {
        let mut state = sample_state();
        state.editor_ui.locale = locale;
        let Some(panel) = PropertyPanel::for_selection(&state) else {
            continue;
        };
        let rect = Rect {
            origin: Point2D::new(920.0, 44.0),
            size: Point2D::new(280.0, 1600.0),
        };

        assert_no_new_overflow(&format!("property panel {locale:?}"), rect, |cx| {
            panel.paint(cx, rect)
        });
    }
}

#[test]
fn layer_panel_holds_the_rail_in_the_painted_family() {
    use crate::widgets::LayerPanel;

    for locale in op_i18n::Locale::ALL {
        let mut state = sample_state();
        state.editor_ui.locale = locale;
        let panel = LayerPanel::from_editor(&state);
        let rect = Rect {
            origin: Point2D::new(0.0, 44.0),
            size: Point2D::new(240.0, 900.0),
        };

        assert_no_new_overflow(&format!("layer panel {locale:?}"), rect, |cx| {
            panel.paint(cx, rect)
        });
    }
}

#[test]
fn chat_panel_holds_its_card_in_the_painted_family() {
    use crate::widgets::AIChatPlaceholder;

    for locale in op_i18n::Locale::ALL {
        let mut state = EditorState::sample();
        state.editor_ui.locale = locale;
        let panel = AIChatPlaceholder::from_editor(&state);
        let rect = Rect {
            origin: Point2D::new(600.0, 300.0),
            size: Point2D::new(360.0, 520.0),
        };

        assert_no_new_overflow(&format!("chat panel {locale:?}"), rect, |cx| {
            panel.paint(cx, rect)
        });
    }
}

#[test]
fn variables_panel_holds_its_panel_in_the_painted_family() {
    use crate::widgets::variables_panel::VariablesPanel;

    for locale in op_i18n::Locale::ALL {
        let mut state = EditorState::sample();
        state.editor_ui.locale = locale;
        let panel = VariablesPanel::for_editor(&state);
        let rect = Rect {
            origin: Point2D::new(320.0, 120.0),
            size: Point2D::new(560.0, 420.0),
        };

        assert_no_new_overflow(&format!("variables panel {locale:?}"), rect, |cx| {
            panel.paint(cx, rect)
        });
    }
}

#[test]
fn top_bar_holds_the_bar_in_the_painted_family() {
    use crate::widgets::{TopBar, TOP_BAR_HEIGHT};

    for locale in op_i18n::Locale::ALL {
        let mut ui = op_editor_core::editor_ui_state::EditorUiState {
            account_ui_available: true,
            ..Default::default()
        };
        ui.locale = locale;
        ui.collab.availability = op_editor_core::CollabAvailability::Ready;
        let bar = TopBar::for_editor_ui(&ui);
        let rect = Rect {
            origin: Point2D::ZERO,
            size: Point2D::new(1400.0, TOP_BAR_HEIGHT),
        };

        assert_no_new_overflow(&format!("top bar {locale:?}"), rect, |cx| {
            bar.paint(cx, rect)
        });
    }
}

/// The guard has to be able to fail. Painting a deliberately family-blind
/// fitter into the pair must be caught — otherwise a green run above proves
/// nothing.
#[test]
#[should_panic(expected = "instead of crate::widgets::text_metrics")]
fn the_guard_catches_a_family_blind_fitter() {
    use crate::{Color, TextLayout};

    let container = Rect::xywh(0.0, 0.0, 100.0, 30.0);
    assert_no_new_overflow("deliberately blind widget", container, |cx| {
        // Exactly the shipped bug: fit against `measure_text`, paint as a
        // named family. The control's two faces agree so it fits there; the
        // real face is wider, so it spills.
        let fitted = crate::util::ellipsize_to_width("a long chrome label here", 100.0, |s| {
            cx.backend.measure_text(s, 12.0)
        });
        let layout = TextLayout::single_run(
            &fitted,
            "system-ui",
            12.0,
            Color::WHITE.to_jian(),
            Point2D::ZERO,
        );
        cx.backend.draw_text(&layout, Point2D::ZERO);
    });
}
