//! Canvas-drag mutators: handle-resize + drag-end auto-layout reorder /
//! cross-container reparenting.
//!
//! Resize follows Pencil's frame semantics: mutate the selected node's
//! authored bounds only, then let the layout engine reflow Fill/Hug/flex
//! descendants. It is deliberately not a subtree Scale operation.
//! Split out of `mutators.rs` to keep that file under the 800-line
//! ceiling.

use crate::geometry::DocRect;
use crate::node_id::NodeId;
use crate::pen_node_ext::PenNodeExt;
use crate::state::EditorState;
use crate::walkers::{self, find_node_mut};
use jian_ops_schema::node::container::LayoutMode;
use jian_ops_schema::node::text::TextGrowth;
use jian_ops_schema::node::PenNode;

/// Main axis of an auto-layout (flex) container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Vertical,
    Horizontal,
}

/// Authored dimensions affected by a selection-handle drag.
///
/// Edge handles freeze one axis to a number; corner handles freeze both.
/// The untouched axis keeps its existing Number / Fill / Hug mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeAxes {
    Width,
    Height,
    Both,
}

impl ResizeAxes {
    fn width(self) -> bool {
        matches!(self, Self::Width | Self::Both)
    }

    fn height(self) -> bool {
        matches!(self, Self::Height | Self::Both)
    }
}

/// Canonical drop destination for canvas node dragging. The host
/// resolves hit-testing and absolute container bounds; core owns the
/// document-tree mutation and coordinate conversion.
#[derive(Debug, Clone, PartialEq)]
pub enum DragDropTarget {
    /// Insert as a top-level node in the active page.
    PageRoot { index: usize },
    /// Insert into a container. `parent_abs_*` are the target
    /// container's absolute document-space origin read from layout.
    Container {
        parent_id: NodeId,
        parent_abs_x: f64,
        parent_abs_y: f64,
        index: usize,
    },
}

/// The container's explicit auto-layout direction, or `None` for
/// free-layout containers and leaf nodes.
pub fn auto_layout_direction(node: &PenNode) -> Option<FlexDirection> {
    let layout = match node {
        PenNode::Frame(n) => n.container.layout.as_ref(),
        PenNode::Group(n) => n.container.layout.as_ref(),
        PenNode::Rectangle(n) => n.container.layout.as_ref(),
        _ => None,
    }?;
    match layout {
        LayoutMode::Vertical => Some(FlexDirection::Vertical),
        LayoutMode::Horizontal => Some(FlexDirection::Horizontal),
        LayoutMode::None => None,
    }
}

/// Current canvas drag semantics allow any editable dragged subtree to
/// leave its parent. The editability and cycle checks are enforced by
/// `move_node_to_drop_target`; this helper is kept for older callers
/// that still ask the policy question by node type.
pub fn should_auto_reparent_outside_parent(_node: &PenNode) -> bool {
    true
}

/// Immediate parent id of `target` anywhere in the forest, or `None`
/// when `target` is top-level (or missing).
pub fn parent_of(children: &[PenNode], target: &NodeId) -> Option<NodeId> {
    for child in children {
        if let Some(grand) = child.children() {
            if grand.iter().any(|c| c.id_str() == target.as_str()) {
                return NodeId::new_opt(child.id_str());
            }
            if let Some(found) = parent_of(grand, target) {
                return Some(found);
            }
        }
    }
    None
}

/// A child participates in its parent's auto-layout flow only when the parent
/// is flex and the child has no explicit position. Jian treats either `x` or
/// `y` as an absolute-position contract, so overlays inside a flex container
/// must still accept parent-relative left/top resize writes.
fn selected_is_flow_child(children: &[PenNode], target: &NodeId) -> bool {
    let Some((Some(parent_id), _)) = walkers::find_parent_and_index(children, target) else {
        return false;
    };
    let Some(parent) = walkers::find_node(children, &parent_id) else {
        return false;
    };
    let Some(node) = walkers::find_node(children, target) else {
        return false;
    };
    parent.is_auto_layout_container() && node.base().x.is_none() && node.base().y.is_none()
}

impl EditorState {
    /// Overwrite the anchor node's axis-aligned bounds (doc-space
    /// `x`/`y` + `width`/`height`). No-op when the node is locked /
    /// hidden / missing.
    ///
    /// Convenience for operations that author a complete new rectangle
    /// (shape creation and direct geometry tests). Handle drags should call
    /// [`Self::resize_selected_bounds`] so an untouched Fill/Hug axis stays
    /// authored as Fill/Hug.
    pub fn set_selected_bounds(&mut self, bounds: DocRect) {
        self.resize_selected_bounds(bounds, ResizeAxes::Both, Some(bounds.x), Some(bounds.y));
    }

    /// Resize only the selected node. Descendant authored geometry is never
    /// multiplied; normal layout re-resolution is solely responsible for
    /// adapting Fill/Hug children, text wrapping, alignment, and flex gaps.
    ///
    /// `new_x` / `new_y` are parent-relative authored coordinates supplied
    /// only when the dragged left/top edge moves. Flow children ignore them
    /// and keep `x/y: None`; explicit overlays inside auto-layout are not flow
    /// children and therefore retain correct relative positioning.
    pub fn resize_selected_bounds(
        &mut self,
        bounds: DocRect,
        axes: ResizeAxes,
        new_x: Option<f64>,
        new_y: Option<f64>,
    ) {
        let sel = self.selection.anchor.clone();
        if !sel.is_real() || !self.is_editable(&sel) {
            return;
        }
        let is_flow_child = selected_is_flow_child(self.active_children(), &sel);
        let mut wrote = false;
        if let Some(node) = find_node_mut(self.active_children_mut(), &sel) {
            if !is_flow_child {
                if let Some(x) = new_x.filter(|value| value.is_finite()) {
                    node.base_mut().x = Some(x);
                    wrote = true;
                }
                if let Some(y) = new_y.filter(|value| value.is_finite()) {
                    node.base_mut().y = Some(y);
                    wrote = true;
                }
            }
            if axes.width() && bounds.w.is_finite() && bounds.w > 0.0 {
                node.set_width_px(bounds.w);
                wrote = true;
            }
            if axes.height() && bounds.h.is_finite() && bounds.h > 0.0 {
                node.set_height_px(bounds.h);
                wrote = true;
            }
            if axes.width() {
                if let PenNode::Text(text) = &mut *node {
                    if text.text_growth.is_none() {
                        text.text_growth = Some(TextGrowth::FixedWidth);
                        wrote = true;
                    }
                }
            }
        }
        if wrote {
            self.mark_document_changed();
        }
    }

    /// Move `child` to position `index` within `parent`'s children
    /// (index counted in the list WITHOUT the child — TS
    /// `moveNode(id, parentId, newIndex)` after the midpoint walk).
    /// True when the order actually changed.
    pub fn reorder_child_to_index(
        &mut self,
        parent: &NodeId,
        child: &NodeId,
        index: usize,
    ) -> bool {
        if !parent.is_real() || !child.is_real() || parent == child {
            return false;
        }
        if !self.is_editable(child) {
            return false;
        }
        let children = self.active_children_mut();
        let Some(parent_node) = find_node_mut(children, parent) else {
            return false;
        };
        let Some(siblings) = parent_node.children_mut() else {
            return false;
        };
        let Some(cur) = siblings.iter().position(|n| n.id_str() == child.as_str()) else {
            return false;
        };
        let node = siblings.remove(cur);
        let idx = index.min(siblings.len());
        siblings.insert(idx, node);
        cur != idx
    }

    /// Move the single selected child one slot along its parent
    /// auto-layout axis. Non-flow selections return false so callers
    /// can fall back to normal pixel nudging.
    pub fn move_selected_in_layout_direction(&mut self, dx: f64, dy: f64) -> bool {
        if self.selection_count() != 1 {
            return false;
        }
        let selected = self.selection.anchor.clone();
        if !selected.is_real() || !self.is_editable(&selected) {
            return false;
        }

        let children = self.active_children();
        let Some((Some(parent_id), current_index)) =
            walkers::find_parent_and_index(children, &selected)
        else {
            return false;
        };
        let Some(parent) = walkers::find_node(children, &parent_id) else {
            return false;
        };
        let Some(direction) = auto_layout_direction(parent) else {
            return false;
        };

        let target_index = match direction {
            FlexDirection::Vertical if dy < 0.0 => current_index.checked_sub(1),
            FlexDirection::Vertical if dy > 0.0 => Some(current_index + 1),
            FlexDirection::Horizontal if dx < 0.0 => current_index.checked_sub(1),
            FlexDirection::Horizontal if dx > 0.0 => Some(current_index + 1),
            _ => None,
        };
        let Some(target_index) = target_index else {
            return false;
        };
        self.reorder_child_to_index(&parent_id, &selected, target_index)
    }

    /// Detach `id` from its parent and re-insert it as the FIRST
    /// top-level child of the active page, preserving its visual
    /// position via the absolute `(abs_x, abs_y)` the caller read off
    /// the layout scene. TS parity: `handleDragEnd`'s
    /// `updateNode({x, y}) + moveNode(id, null, 0)`.
    pub fn reparent_to_page_root(&mut self, id: &NodeId, abs_x: f64, abs_y: f64) -> bool {
        if !id.is_real() || !self.is_subtree_unlocked(id) {
            return false;
        }
        let children = self.active_children_mut();
        // Already top-level — nothing to reparent.
        if children.iter().any(|n| n.id_str() == id.as_str()) {
            return false;
        }
        let Some(mut node) = walkers::extract_node(children, id) else {
            return false;
        };
        {
            let base = node.base_mut();
            base.x = Some(abs_x);
            base.y = Some(abs_y);
        }
        children.insert(0, node);
        true
    }

    /// Move a dragged node to a resolved canvas drop target,
    /// preserving the visual origin supplied by the host.
    ///
    /// Coordinate semantics:
    /// - Page root: node `x`/`y` become the dropped absolute origin.
    /// - Free container: node `x`/`y` become relative to the target
    ///   container origin.
    /// - Flex container: node enters flow at `index`, so authored
    ///   `x`/`y` are cleared.
    ///
    /// When the node changes parent, the resolved drag bounds are
    /// frozen as literal `width` / `height` so keyword sizing such as
    /// `fill_container` keeps its visual size after leaving the old
    /// container.
    pub fn move_node_to_drop_target(
        &mut self,
        id: &NodeId,
        target: DragDropTarget,
        abs_x: f64,
        abs_y: f64,
        abs_w: f64,
        abs_h: f64,
    ) -> bool {
        if !id.is_real() || !self.is_subtree_unlocked(id) {
            return false;
        }

        let source_parent = parent_of(self.active_children(), id);
        let (target_parent, target_index, target_flex, target_abs) = {
            let children = self.active_children();
            let Some(source) = walkers::find_node(children, id) else {
                return false;
            };
            match &target {
                DragDropTarget::PageRoot { index } => (None, *index, false, (0.0, 0.0)),
                DragDropTarget::Container {
                    parent_id,
                    parent_abs_x,
                    parent_abs_y,
                    index,
                } => {
                    if parent_id == id || walkers::descendant_contains(source, parent_id) {
                        return false;
                    }
                    let Some(parent) = walkers::find_node(children, parent_id) else {
                        return false;
                    };
                    if parent.children().is_none() {
                        return false;
                    }
                    (
                        Some(parent_id.clone()),
                        *index,
                        auto_layout_direction(parent).is_some(),
                        (*parent_abs_x, *parent_abs_y),
                    )
                }
            }
        };

        let Some(mut node) = walkers::extract_node(self.active_children_mut(), id) else {
            return false;
        };
        if source_parent != target_parent {
            if abs_w.is_finite() && abs_w > 0.0 {
                node.set_width_px(abs_w);
            }
            if abs_h.is_finite() && abs_h > 0.0 {
                node.set_height_px(abs_h);
            }
        }
        {
            let base = node.base_mut();
            if target_flex {
                base.x = None;
                base.y = None;
            } else if target_parent.is_some() {
                base.x = Some(abs_x - target_abs.0);
                base.y = Some(abs_y - target_abs.1);
            } else {
                base.x = Some(abs_x);
                base.y = Some(abs_y);
            }
        }

        let inserted = walkers::insert_into_parent(
            self.active_children_mut(),
            target_parent.as_ref(),
            Some(target_index),
            node,
        );
        if inserted {
            // Reordenar/insertar es una mutacion de contenido, asi que tiene
            // que bumpear la revision (ver EditorState::mark_document_changed):
            // la revision es la identidad del cache de escena
            // (op_pen_loader::SceneBuildCache), y sin bumpearla el host se
            // queda pintando la escena de ANTES del drop. Eso hacia que el
            // indice de insercion se calculara sobre la posicion vieja del
            // nodo y salia 0 en vez de 1.
            self.mark_document_changed();
        }
        inserted
    }

/// Sustituye una primitiva por su equivalente path, en el sitio.
///
/// Esto es la entrada del "doble tap entra al editor de nodos": la primitiva
/// se convierte a `PenNode::Path` con anchors (via
/// `convert_to_path::convert_primitive_to_path`, FASE 3), de
/// modo que el overlay que YA existe (`canvas_path_overlay.rs:97`) lo pinta
/// sin necesidad de tocar el renderer.
///
/// Devuelve `true` si el documento cambió.
///
/// Por qué conserva el índice: `find_parent_and_index` da (padre, índice) y el
/// nodo se reinserta exactamente ahí, así que un rect dentro de un frame sigue
/// en su posición y no salta de sitio al convertirse.
///
/// Por qué bumpea la revisión: `mark_document_changed` es obligatorio en todo
/// mutator de contenido (ver su doc), porque la revisión es la identidad del
/// caché de escena. Sin esto el host se queda pintando la escena anterior.
pub fn convert_node_to_path_in_place(&mut self, id: &NodeId) -> bool {
    let Some((parent, index)) = walkers::find_parent_and_index(self.active_children(), id) else {
        return false;
    };
    let Some(original) = walkers::find_node(self.active_children(), id).cloned() else {
        return false;
    };
    let Some(converted) =
        crate::convert_to_path::convert_primitive_to_path(&original) else {
        return false;
    };
    if converted == original {
        return false;
    }
    // extract_node se lleva el nodo (y sus hijos, por eso el guard de hijos
    // vive en el convertidor). Se reinserta en el mismo padre e indice.
    if walkers::extract_node(self.active_children_mut(), id).is_none() {
        return false;
    }
    if !walkers::insert_into_parent(
        self.active_children_mut(),
        parent.as_ref(),
        Some(index),
        converted,
    ) {
        return false;
    }
    // La seleccion sigue apuntando al mismo id, asi que no hace falta
    // re-seleccionar: el id se conserva en la conversion.
    self.mark_document_changed();
    true
}
}


#[cfg(test)]
mod convert_in_place_tests {
    use super::*;
    use jian_ops_schema::node::container::ContainerProps;
    use jian_ops_schema::node::{PenNodeBase, RectangleNode};
    use jian_ops_schema::sizing::{SizingBehavior, SizingKeyword};

    fn rect(id: &str, parent: Option<&str>, index_in_parent: usize) -> PenNode {
        PenNode::Rectangle(RectangleNode {
            base: PenNodeBase {
                id: id.into(),
                name: Some("R".into()),
                x: Some(0.0),
                y: Some(0.0),
                ..Default::default()
            },
            container: ContainerProps {
                width: Some(SizingBehavior::Number(40.0)),
                height: Some(SizingBehavior::Number(20.0)),
                ..Default::default()
            },
            children: None,
            state: None,
            bindings: None,
            events: None,
            lifecycle: None,
            semantics: None,
            gestures: None,
            route: None,
        })
    }

    /// Un rect suelto se convierte y conserva su id.
    #[test]
    fn top_level_rect_converts_in_place() {
        let mut s = EditorState::default();
        s.active_children_mut().push(rect("r1", None, 0));
        assert!(s.convert_node_to_path_in_place(&NodeId::new("r1")));
        let node = &s.active_children()[0];
        assert!(matches!(node, PenNode::Path(_)), "debe ser Path ahora");
        assert_eq!(node.id_str(), "r1", "el id se conserva");
    }

    /// Un rect dentro de un frame se convierte Y sigue en su indice. Si el
    /// indice se perdiera, el rect se moveria de sitio al convertirse, que es
    /// justo el tipo de salto que el usuario no espera.
    #[test]
    fn nested_rect_keeps_its_index_among_siblings() {
        let mut s = EditorState::default();
        let mut frame = rect("frame", None, 0);
        if let PenNode::Rectangle(r) = &mut frame {
            r.children = Some(vec![rect("a", None, 0), rect("b", None, 1), rect("c", None, 2)]);
        }
        s.active_children_mut().push(frame);

        assert!(s.convert_node_to_path_in_place(&NodeId::new("b")));
        let children = s.active_children()[0].children().unwrap();
        let ids: Vec<&str> = children.iter().map(|n| n.id_str()).collect();
        assert_eq!(ids, vec!["a", "b", "c"], "el orden no debe cambiar");
        assert!(matches!(children[1], PenNode::Path(_)), "b es Path");
    }

    /// Un rect CON HIJOS no se convierte: seria perder los hijos.
    #[test]
    fn rect_with_children_is_refused() {
        let mut s = EditorState::default();
        let mut parent = rect("parent", None, 0);
        if let PenNode::Rectangle(r) = &mut parent {
            r.children = Some(vec![rect("kid", None, 0)]);
        }
        s.active_children_mut().push(parent);
        assert!(
            !s.convert_node_to_path_in_place(&NodeId::new("parent")),
            "un contenedor no debe convertirse"
        );
        // Y el árbol sigue intacto.
        assert_eq!(s.active_children()[0].children().unwrap().len(), 1);
    }

    /// Un id que no existe no rompe nada.
    #[test]
    fn missing_id_is_a_noop() {
        let mut s = EditorState::default();
        s.active_children_mut().push(rect("r1", None, 0));
        assert!(!s.convert_node_to_path_in_place(&NodeId::new("nope")));
    }

    /// Un tamaño no fijo no es geometría: no se convierte.
    #[test]
    fn fit_content_rect_is_refused() {
        let mut s = EditorState::default();
        let mut r = rect("r1", None, 0);
        if let PenNode::Rectangle(x) = &mut r {
            x.container.height = Some(SizingBehavior::Keyword(SizingKeyword::FitContent));
        }
        s.active_children_mut().push(r);
        assert!(!s.convert_node_to_path_in_place(&NodeId::new("r1")));
    }

    /// Convertir sube la revisión, que es la identidad del caché de escena.
    /// Sin esto el host se queda pintando la escena anterior.
    #[test]
    fn conversion_bumps_the_revision() {
        let mut s = EditorState::default();
        s.active_children_mut().push(rect("r1", None, 0));
        let before = s.document_revision();
        assert!(s.convert_node_to_path_in_place(&NodeId::new("r1")));
        assert!(
            s.document_revision() != before,
            "la revisión debe cambiar: es lo que invalida el caché de escena"
        );
    }
}
