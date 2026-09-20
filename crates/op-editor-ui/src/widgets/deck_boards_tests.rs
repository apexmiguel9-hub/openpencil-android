use super::*;
use op_editor_core::scene_template_catalog::TemplateScene;

const THREE_BOARDS: &str = r#"{"version":"1.0.0","children":[
    {"type":"frame","id":"slide-1","name":"Cover","x":0,"y":0,"width":1920,"height":1080},
    {"type":"frame","id":"slide-2","name":"Agenda","x":2100,"y":0,"width":1920,"height":1080},
    {"type":"frame","id":"slide-3","name":"封面之后的一页","x":4200,"y":0,"width":1920,"height":1080}
]}"#;

fn deck_state(source: &str) -> EditorState {
    let document = jian_ops_schema::load_str(source)
        .expect("fixture parses")
        .value;
    let mut state = EditorState::from_document(document);
    state.editor_ui.scenario = Some(TemplateScene::Slides);
    state
}

fn canvas() -> Rect {
    Rect {
        origin: Point2D::new(240.0, 48.0),
        size: Point2D::new(1000.0, 700.0),
    }
}

fn three_board_scene() -> crate::layout_scene::LayoutScene {
    use crate::layout_scene::{LayoutScene, NodeKind, SceneNode, ScenePage};
    let board = |id: &str, x: f32| {
        let mut node = SceneNode::leaf(id, NodeKind::Frame);
        node.bounds = Rect::xywh(x, 0.0, 1920.0, 1080.0);
        node
    };
    LayoutScene {
        pages: vec![ScenePage {
            id: "page-1".into(),
            name: "Page 1".into(),
            children: vec![
                board("slide-1", 0.0),
                board("slide-2", 2100.0),
                board("slide-3", 4200.0),
            ],
        }],
        active_page_index: 0,
    }
}

#[test]
fn boards_are_listed_in_page_order_with_their_names() {
    let deck = deck_state(THREE_BOARDS);
    let chips = board_chips(&deck);
    assert_eq!(
        chips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["slide-1", "slide-2", "slide-3"]
    );
    assert_eq!(chips[2].name, "封面之后的一页");
}

#[test]
fn a_document_with_no_boards_lists_nothing() {
    let empty = deck_state(r#"{"version":"1.0.0","children":[]}"#);
    assert!(board_chips(&empty).is_empty());
}

#[test]
fn the_current_slide_is_the_board_nearest_the_viewport_centre() {
    let mut deck = deck_state(THREE_BOARDS);
    let chips = board_chips(&deck);
    let scene = three_board_scene();

    // Camera parked on board 2 (its centre is doc (3060, 540)).
    deck.viewport.zoom = 0.25;
    deck.viewport.pan_x = canvas().size.x / 2.0 - 3060.0 * 0.25;
    deck.viewport.pan_y = canvas().size.y / 2.0 - 540.0 * 0.25;
    assert_eq!(
        active_chip_index(&chips, &scene, &deck, canvas()),
        Some(1),
        "the nearest board is the one on screen"
    );

    // Nudge the camera towards board 3 — the answer follows the camera,
    // not the selection or the document order.
    deck.viewport.pan_x = canvas().size.x / 2.0 - 5160.0 * 0.25;
    assert_eq!(active_chip_index(&chips, &scene, &deck, canvas()), Some(2));
}

#[test]
fn dropping_either_side_of_the_dragged_row_changes_nothing() {
    // Slot 2 is the gap the row already sits in front of, slot 3 the one
    // behind it: both leave slide 3 exactly where it was.
    assert_eq!(reorder_target_index(2, 2), None);
    assert_eq!(reorder_target_index(2, 3), None);
}

#[test]
fn a_drop_converts_to_the_index_the_board_ends_up_at() {
    // Dragging slide 4 (index 3) to the very front.
    assert_eq!(reorder_target_index(3, 0), Some(0));
    // Dragging slide 1 to the end of a five-slide deck: after removing it
    // the array is one shorter, so the last index is 4.
    assert_eq!(reorder_target_index(0, 5), Some(4));
    // A short hop backwards keeps the slot as-is.
    assert_eq!(reorder_target_index(4, 1), Some(1));
}

#[test]
fn a_reorder_moves_the_board_in_child_order_and_touches_no_geometry() {
    let mut deck = deck_state(THREE_BOARDS);
    let before: Vec<(String, f64, f64)> = deck
        .active_children()
        .iter()
        .map(|node| {
            let base = op_editor_core::PenNodeExt::base(node);
            (
                base.id.clone(),
                base.x.unwrap_or(0.0),
                base.y.unwrap_or(0.0),
            )
        })
        .collect();

    assert!(apply_reorder(&mut deck, "slide-1", 2));

    assert_eq!(
        op_editor_core::preview_slideshow::active_page_boards(&deck),
        ["slide-2", "slide-3", "slide-1"],
        "the page order is the child order"
    );
    for (id, x, y) in before {
        let node = deck
            .active_children()
            .iter()
            .find(|node| op_editor_core::PenNodeExt::id_str(*node) == id)
            .expect("every board survives a reorder");
        let base = op_editor_core::PenNodeExt::base(node);
        assert_eq!(
            (base.x.unwrap_or(0.0), base.y.unwrap_or(0.0)),
            (x, y),
            "{id} moved on the canvas — a reorder must only change the sequence"
        );
    }
}

#[test]
fn a_refused_reorder_leaves_no_entry_on_the_undo_stack() {
    let mut deck = deck_state(THREE_BOARDS);
    let depth = deck.history.past.len();
    assert!(!apply_reorder(&mut deck, "no-such-board", 1));
    assert_eq!(
        deck.history.past.len(),
        depth,
        "a rejected reorder must not leave an empty undo step"
    );
}
