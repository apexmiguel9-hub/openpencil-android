//! TEMPORARY evidence-collection probe — NOT a permanent test.
//!
//! Investigates: "objects dragged into an auto-layout container from
//! outside end up displaced >100px from the drop point, while objects
//! created inside behave." Records drop point (screen/doc), the same
//! point in container-local coords, the chosen insert index, and the
//! node's final rendered position for three cases:
//!   A. created inside the container area (production create path)
//!   B. dragged from outside, dropped 10 / 50 / 100 px inside the edge
//!   C. moved within the same auto-layout container
//! plus a control: drop into a FREE (layout: none) container.
//!
//! It intentionally panics with the full log so the evidence surfaces
//! in `cargo test` output / CI logs. Remove this file once the cause
//! is reported.

use super::{NodeDragState, WidgetHostNative};
use op_editor_core::{NodeId, PenNodeExt};
use op_editor_ui::Point2D;

/// Vertical auto-layout stack: children 150×150, gap 60, no padding.
/// Stack box (100,100) 300×900. Plus a free draggable rect outside.
const FIXTURE: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"stack","name":"stack","x":100,"y":100,"width":300,"height":900,
   "layout":"vertical","gap":60,
   "children":[
     {"type":"rectangle","id":"a","name":"a","width":150,"height":150},
     {"type":"rectangle","id":"b","name":"b","width":150,"height":150},
     {"type":"rectangle","id":"c","name":"c","width":150,"height":150},
     {"type":"rectangle","id":"d","name":"d","width":150,"height":150}
   ]},
  {"type":"rectangle","id":"dragme","name":"dragme","x":500,"y":100,"width":100,"height":40}
]}"#;

/// Free container control doc: same box, but NO auto layout.
const FIXTURE_FREE: &str = r#"{"version":"1.0.0","children":[
  {"type":"frame","id":"freebox","name":"freebox","x":100,"y":100,"width":300,"height":900,
   "children":[
     {"type":"rectangle","id":"a","name":"a","x":110,"y":120,"width":150,"height":150},
     {"type":"rectangle","id":"b","name":"b","x":110,"y":300,"width":150,"height":150}
   ]},
  {"type":"rectangle","id":"dragme","name":"dragme","x":500,"y":100,"width":100,"height":40}
]}"#;

fn seed(host: &mut WidgetHostNative, json: &str) {
    let doc = jian_ops_schema::load_str(json)
        .expect("fixture JSON parses")
        .value;
    *host.editor_state_mut() = op_editor_core::EditorState::from_document(doc);
    host.mark_paint_dirty_for_test();
}

fn scene_xy(host: &mut WidgetHostNative, id: &str) -> (f32, f32) {
    let n = scene_find(host, id);
    let b = n.bounds;
    (b.origin.x, b.origin.y)
}

fn scene_center(host: &mut WidgetHostNative, id: &str) -> (f32, f32) {
    let n = scene_find(host, id);
    let b = n.bounds;
    (b.origin.x + b.size.x / 2.0, b.origin.y + b.size.y / 2.0)
}

/// Resolve the active page via the getter (which lazily refreshes the
/// scene) and find `id`. On a miss, panic with the ids present on the
/// page so CI output names the discrepancy instead of a bare expect.
fn scene_find<'a>(host: &'a mut WidgetHostNative, id: &str) -> &'a op_editor_ui::layout_scene::SceneNode {
    let page = &*host.layout_scene().active_page().expect("scene has an active page");
    match page.find(id) {
        Some(n) => n,
        None => {
            let mut ids: Vec<String> = Vec::new();
            collect_ids(&page.children, &mut ids);
            panic!("scene node '{id}' missing; page ids = {ids:?}");
        }
    }
}

fn collect_ids(nodes: &[op_editor_ui::layout_scene::SceneNode], out: &mut Vec<String>) {
    for n in nodes {
        out.push(n.id.clone());
        collect_ids(&n.children, out);
    }
}

fn authored_xy(host: &WidgetHostNative, id: &str) -> (Option<f64>, Option<f64>) {
    let node =
        op_editor_core::walkers::find_node(host.editor_state().active_children(), &NodeId::new(id))
            .expect("node present");
    (node.base().x, node.base().y)
}

fn parent_of(host: &WidgetHostNative, id: &str) -> Option<String> {
    op_editor_core::drag_mutators::parent_of(host.editor_state().active_children(), &NodeId::new(id))
        .map(|p| p.as_str().to_string())
}

fn index_in_parent(host: &WidgetHostNative, parent: &str, child: &str) -> Option<usize> {
    let p = op_editor_core::walkers::find_node(
        host.editor_state().active_children(),
        &NodeId::new(parent),
    )?;
    p.children()?
        .iter()
        .position(|n| n.id_str() == child)
}

/// Drive the REAL host drag path: set the node-drag state, translate the
/// cursor (live translation + delta accumulation), then release-commit.
fn drag_node_to(host: &mut WidgetHostNative, id: &str, target_center: (f32, f32)) -> bool {
    let start = scene_center(host, id);
    host.editor_state_mut()
        .set_single_selection(NodeId::new(id));
    host.editor_state_mut().viewport.zoom = 1.0;
    host.refresh_layout_scene();
    host.node_drag = Some(NodeDragState {
        last_screen_x: start.0,
        last_screen_y: start.1,
        press_screen_x: start.0,
        press_screen_y: start.1,
        moved: true,
        total_dx: 0.0,
        total_dy: 0.0,
        overlay_bounds: None,
    });
    host.apply_cursor_move(target_center.0 as f32, target_center.1 as f32);
    let drag = host.node_drag.expect("drag set");
    host.commit_node_drag(&drag)
}

#[test]
fn probe_flex_drop_displacement_evidence() {
    let mut log = String::new();
    log.push_str("PROBE|flex-drop investigation start\n");

    // ---------------------------------------------------------------- A
    // Created inside the container area (production create path).
    for d in [10.0_f32, 50.0, 100.0] {
        let mut host = WidgetHostNative::new();
        seed(&mut host, FIXTURE);
        host.editor_state_mut().tool = op_editor_core::Tool::Rect;
        host.refresh_layout_scene();
        let drop_top = Point2D::new(250.0, 100.0 + d);
        let id = host
            .create_node_for_active_tool(drop_top)
            .expect("create succeeds");
        host.refresh_layout_scene();
        let final_xy = scene_xy(&mut host, id.as_str());
        let authored = authored_xy(&host, id.as_str());
        log.push_str(&format!(
            "PROBE|A created-inside|d={d:>3}|drop_top_doc=({:.0},{:.0})|drop_top_local=({:.0},{:.0})|parent={}|authored_x_y=({:?},{:?})|final_top=({:.0},{:.0})|disp_top={:.1}\n",
            drop_top.x, drop_top.y,
            drop_top.x - 100.0, drop_top.y - 100.0,
            parent_of(&host, id.as_str()).unwrap_or_else(|| "PAGE_ROOT".into()),
            authored.0, authored.1,
            final_xy.0, final_xy.1,
            final_xy.1 - drop_top.y,
        ));
    }

    // ---------------------------------------------------------------- B
    // Dragged from OUTSIDE (page root free node) into the flex stack,
    // dropped so the node's TOP edge is at container-local y = 10/50/100.
    for d in [10.0_f32, 50.0, 100.0] {
        let mut host = WidgetHostNative::new();
        seed(&mut host, FIXTURE);
        host.refresh_layout_scene();
        // dragme starts at (500,100) 100×40 → center (550,120).
        // Drop: node top edge at container-local y = d → center (250, 120+d).
        let target_center = (250.0_f32, 120.0 + d);
        let mutated = drag_node_to(&mut host, "dragme", target_center);
        host.refresh_layout_scene();
        let drop_top = (200.0, 100.0 + d);
        let dropped_center = target_center;
        let final_xy = scene_xy(&mut host, "dragme");
        let auth = authored_xy(&host, "dragme");
        let parent = parent_of(&host, "dragme").unwrap_or_else(|| "PAGE_ROOT".into());
        let index = index_in_parent(&host, &parent, "dragme");
        log.push_str(&format!(
            "PROBE|B drag-from-outside|d={d:>3}|mutated={mutated}|drop_top_doc=({:.0},{:.0})|drop_center_doc=({:.0},{:.0})|drop_top_local=({:.0},{:.0})|drop_center_local=({:.0},{:.0})|insert_index={:?}|parent={parent}|authored_x_y=({:?},{:?})|final_top=({:.0},{:.0})|disp_top={:.1}\n",
            drop_top.0, drop_top.1,
            dropped_center.0, dropped_center.1,
            drop_top.0 - 100.0, drop_top.1 - 100.0,
            dropped_center.0 - 100.0, dropped_center.1 - 100.0,
            index, auth.0, auth.1, final_xy.0, final_xy.1,
            final_xy.1 - drop_top.1,
        ));
    }

    // ---------------------------------------------------------------- C
    // Moved WITHIN the same flex container: flow child "a" reordered to
    // the slot between b and c.
    {
        let mut host = WidgetHostNative::new();
        seed(&mut host, FIXTURE);
        host.refresh_layout_scene();
        let a_start = scene_center(&mut host, "a");
        let b = host
            .layout_scene
            .active_page()
            .and_then(|p| p.find("b"))
            .expect("b in scene");
        let c = host
            .layout_scene
            .active_page()
            .and_then(|p| p.find("c"))
            .expect("c in scene");
        let gap_mid_y = (b.bounds.origin.y + b.bounds.size.y + c.bounds.origin.y) / 2.0;
        let index_before = index_in_parent(&host, "stack", "a");
        let mutated = drag_node_to(&mut host, "a", (a_start.0, gap_mid_y));
        host.refresh_layout_scene();
        let index_after = index_in_parent(&host, "stack", "a");
        let final_xy = scene_xy(&mut host, "a");
        let auth = authored_xy(&host, "a");
        log.push_str(&format!(
            "PROBE|C moved-within|mutated={mutated}|start_center=({:.0},{:.0})|drop_center_local=({:.0},{:.0})|index_before={index_before:?}|index_after={index_after:?}|authored_x_y=({:?},{:?})|final_top=({:.0},{:.0})|disp_center={:.1}\n",
            a_start.0, a_start.1,
            a_start.0 - 100.0, gap_mid_y - 100.0,
            auth.0, auth.1, final_xy.0, final_xy.1,
            final_xy.1 + 75.0 - gap_mid_y,
        ));
    }

    // ------------------------------------------------------------ CONTROL
    // Dragged from outside into a FREE container (layout none): the
    // position must land exactly on the drop point (conversion check).
    {
        let mut host = WidgetHostNative::new();
        seed(&mut host, FIXTURE_FREE);
        host.refresh_layout_scene();
        let target_center = (250.0_f32, 170.0); // freebox-local (150, 70)
        let mutated = drag_node_to(&mut host, "dragme", target_center);
        host.refresh_layout_scene();
        let final_xy = scene_xy(&mut host, "dragme");
        let auth = authored_xy(&host, "dragme");
        log.push_str(&format!(
            "PROBE|CTRL drop-into-free|mutated={mutated}|drop_top_doc=(200.0,150.0)|drop_top_local=(100.0,50.0)|insert_index=0|parent={}|authored_x_y=({:?},{:?})|final_top=({:.0},{:.0})|disp_top={:.1}\n",
            parent_of(&host, "dragme").unwrap_or_else(|| "PAGE_ROOT".into()),
            auth.0, auth.1, final_xy.0, final_xy.1, final_xy.1 - 150.0,
        ));
    }

    log.push_str("PROBE|flex-drop investigation end");
    eprintln!("\n{log}\n");
    panic!("\n{log}\n");
}