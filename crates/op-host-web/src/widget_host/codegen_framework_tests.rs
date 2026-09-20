use super::{CodeSelectionDragState, WidgetHost};
use op_editor_core::codegen::{CodeSelection, CodegenPhase, Framework};
use op_editor_ui::widgets::property_panel_action::CodegenAction;
use op_editor_ui::widgets::PropertyPanelAction;

#[test]
fn framework_switch_restores_previous_web_output_on_return() {
    let mut host = WidgetHost::new();
    {
        let cg = &mut host.editor_state_mut().codegen;
        cg.framework = Framework::Html;
        cg.phase = CodegenPhase::Error;
        cg.code = "<!doctype html><main>Hello</main>".into();
        cg.code_scroll.offset = 48.0;
        cg.code_selection = Some(CodeSelection {
            anchor: 0,
            focus: 6,
        });
        cg.degraded = true;
        cg.selection_snapshot = vec!["hero".into()];
        cg.error = Some("assembly failed".into());
    }
    host.code_selection_drag = Some(CodeSelectionDragState { anchor: 0 });

    host.apply_property_action(PropertyPanelAction::Codegen(
        CodegenAction::SelectFramework(Framework::Vue),
    ));

    let cg = &host.editor_state().codegen;
    assert_eq!(cg.framework, Framework::Vue);
    assert_eq!(cg.phase, CodegenPhase::Idle);
    assert!(cg.code.is_empty());
    assert_eq!(cg.code_scroll.offset, 0.0);
    assert!(cg.code_selection.is_none());
    assert!(!cg.degraded);
    assert!(cg.selection_snapshot.is_empty());
    assert!(cg.error.is_none());
    assert!(host.code_selection_drag.is_none());

    host.apply_property_action(PropertyPanelAction::Codegen(
        CodegenAction::SelectFramework(Framework::Html),
    ));

    let cg = &host.editor_state().codegen;
    assert_eq!(cg.framework, Framework::Html);
    assert_eq!(cg.phase, CodegenPhase::Complete);
    assert_eq!(cg.code, "<!doctype html><main>Hello</main>");
    assert_eq!(cg.code_scroll.offset, 48.0);
    assert_eq!(
        cg.code_selection,
        Some(CodeSelection {
            anchor: 0,
            focus: 6
        })
    );
    assert!(cg.degraded);
    assert_eq!(cg.selection_snapshot, ["hero"]);
}
