//! Convertir una primitiva en un path editable. FASE 3, punto de entrada.
//!
//! QUÉ HACE. Recibe un nodo primitivo (`Rectangle`, `Ellipse`, `Polygon`,
//! `Line`) y devuelve un `PenNode::Path` con los `anchors` puestos, para que el
//! editor de nodos que YA existe pueda editarlo. No inventa geometría: encadena
//! el motor de `jian-skia` (fases 1-3), que ya está probado.
//!
//!   primitiva      --(jian_core::shape_to_path)-->  PathCommand   [FASE 2]
//!   PathCommand    --(jian_core::commands_to_anchors)-->  anchors  [FASE 3]
//!   + original     --(este fichero)-->  PenNode::Path
//!
//! POR QUÉ ESTA FORMA. El overlay `canvas_path_overlay.rs:97` solo pinta
//! handles cuando el nodo es `NodeKind::Path` Y está seleccionado, y
//! `paint_path_editor` solo necesita `node.path_anchors` — no hace falta ningún
//! estado extra de "editando nodos". Convertir a Path y dejar el nodo
//! seleccionado basta para que aparezca el editor. No hay que tocar el
//! renderer, ni crear un modo, ni tocar la herramienta `Pen`.
//!
//! ESTE ES EL PUNTO ÚNICO DEL QUE DEPENDE EL DISEÑO (ver PROGRESS.md). La
//! conversión no destructiva (opción B, rect que sigue siendo rect con un flag
//! "geometría editable") se implementaría AQUÍ, cambiando el cuerpo de
//! `convert_primitive_to_path`, sin tocar el overlay, el menú de contexto ni el
//! host. Por eso está aislada en un solo fichero y no hay lógica de conversión
//! esparcida por el drag/`commit`.
//!
//! LO QUE NO HACEMOS. Los boolean ops (`host_support_allocator.rs:358`) siguen
//! produciendo `anchors: None` y por tanto un path muerto. Es un bug aparte y
//! se arregla aparte; aquí solo se arregla el camino de la conversión
//! explícita.
//!
//! POR QUÉ VIVE AQUÍ Y NO EN op-pen-loader. El convertidor original se puso
//! en el loader porque es quien tiene `jian-skia`. Pero `jian-skia` es
//! opcional ahi (feature `skia-measure`, apagado en el build web), y sobre
//! todo op-pen-loader YA DEPENDE de op-editor-core: meter la mutacion del
//! documento aqui seria una dependencia circular.
//!
//! La solucion: la geometria de `shape_to_path` y `commands_to_anchors` NO
//! USA SKIA. Son geometria pura sobre `PathCommand` y `PenPathAnchor`. Asi que
//! se movieron a `jian-core` (que ya depende de `jian-ops-schema`), y este
//! modulo —que solo necesita el modelo— puede vivir en el core sin arrastrar
//! skia a ninguna parte.

use jian_core::render::PathCommand;
use jian_ops_schema::node::container::CornerRadius;
use jian_ops_schema::node::{EllipseNode, LineNode, PathNode, PenNode, PolygonNode, RectangleNode};
// PenFill / PenStroke / PenEffect viven en style.rs, como los importa
// container.rs:2 -- no en node/.
use jian_ops_schema::style::{PenEffect, PenFill, PenStroke};
// base() viene del trait PenNodeExt (op-editor-core, pen_node_ext.rs:31).
// fill/stroke/effects NO tienen accessor: se leen con un match por tipo, abajo.
// Es solo un trait, asi que op-pen-loader lo usa sin arrastrar skia al core.
use crate::PenNodeExt;
use jian_ops_schema::sizing::SizingBehavior;

/// Un número de `SizingBehavior`, o `None` si no es un número fijo.
/// `fit_content` / `fill_container` no son geometría, no se pueden convertir.
fn sizing_num(v: Option<&SizingBehavior>) -> Option<f32> {
    match v {
        Some(SizingBehavior::Number(n)) => Some(*n as f32),
        _ => None,
    }
}

/// Los cuatro radios como los espera `shape_to_path::rect_commands`.
fn radii_of(radius: Option<&CornerRadius>) -> Option<[(f32, f32); 4]> {
    let r = radius?;
    let v = match r {
        CornerRadius::Uniform(n) => [*n as f32; 4],
        CornerRadius::PerCorner([a, b, c, d]) => {
            [*a as f32, *b as f32, *c as f32, *d as f32]
        }
    };
    if v.iter().all(|r| *r <= 0.0) {
        return None;
    }
    Some([(v[0], v[0]), (v[1], v[1]), (v[2], v[2]), (v[3], v[3])])
}

/// La geometría de una primitiva como `PathCommand`, o `None` si no es
/// convertible.
fn primitive_commands(node: &PenNode) -> Option<Vec<PathCommand>> {
    let x = node.base().x? as f32;
    let y = node.base().y? as f32;
    match node {
        PenNode::Rectangle(RectangleNode { container, .. }) => {
            let w = sizing_num(container.width.as_ref())?;
            let h = sizing_num(container.height.as_ref())?;
            Some(jian_core::shape_to_path::rect_commands(
                x,
                y,
                w,
                h,
                radii_of(container.corner_radius.as_ref()),
            ))
        }
        PenNode::Ellipse(EllipseNode {
            width, height, ..
        }) => {
            let w = sizing_num(width.as_ref())?;
            let h = sizing_num(height.as_ref())?;
            // Solo la elipse completa. Un arco (start_angle / sweep_angle /
            // inner_radius) necesita su propio camino y es un caso aparte; si
            // aparece, se degrada a elipse completa en vez de a nada.
            Some(jian_core::shape_to_path::ellipse_commands(x, y, w, h))
        }
        PenNode::Polygon(PolygonNode {
            width,
            height,
            polygon_count,
            ..
        }) => {
            let w = sizing_num(width.as_ref())?;
            let h = sizing_num(height.as_ref())?;
            let sides = (*polygon_count).max(3);
            Some(jian_core::shape_to_path::polygon_commands(
                x, y, w, h, sides as u32,
            ))
        }
        PenNode::Line(LineNode { x2, y2, .. }) => {
            let x2 = x2.unwrap_or(x as f64).abs() as f32;
            let y2 = y2.unwrap_or(y as f64).abs() as f32;
            Some(jian_core::shape_to_path::line_commands(x, y, x2, y2))
        }
        _ => None,
    }
}

/// La caja de una primitiva, para sembrar width/height del path resultante.
fn primitive_size(node: &PenNode) -> Option<(f32, f32)> {
    match node {
        PenNode::Rectangle(r) => Some((
            sizing_num(r.container.width.as_ref())?,
            sizing_num(r.container.height.as_ref())?,
        )),
        PenNode::Ellipse(e) => Some((sizing_num(e.width.as_ref())?, sizing_num(e.height.as_ref())?)),
        PenNode::Polygon(p) => Some((sizing_num(p.width.as_ref())?, sizing_num(p.height.as_ref())?)),
        // Una linea no tiene caja propia: se deriva de sus dos puntos.
        PenNode::Line(LineNode { x2, y2, .. }) => {
            let x = node.base().x? as f32;
            let y = node.base().y? as f32;
            let x2 = x2.unwrap_or(x as f64).abs() as f32;
            let y2 = y2.unwrap_or(y as f64).abs() as f32;
            Some(((x2 - x).abs(), (y2 - y).abs()))
        }
        _ => None,
    }
}


/// El `fill` del nodo, sea del tipo que sea.
///
/// No hay un accessor `fill()` en el schema: `fill` vive en
/// `ContainerProps` para los contenedores y en cada leaf aparte. Este match
/// es el unico sitio que hay que tocar si anades un tipo de nodo nuevo.
fn fill_of(node: &PenNode) -> Option<Vec<PenFill>> {
    match node {
        PenNode::Frame(f) => f.container.fill.clone(),
        PenNode::Group(g) => g.container.fill.clone(),
        PenNode::Rectangle(r) => r.container.fill.clone(),
        PenNode::Path(p) => p.fill.clone(),
        _ => None,
    }
}

fn stroke_of(node: &PenNode) -> Option<PenStroke> {
    match node {
        PenNode::Frame(f) => f.container.stroke.clone(),
        PenNode::Group(g) => g.container.stroke.clone(),
        PenNode::Rectangle(r) => r.container.stroke.clone(),
        PenNode::Path(p) => p.stroke.clone(),
        _ => None,
    }
}

fn effects_of(node: &PenNode) -> Option<Vec<PenEffect>> {
    match node {
        PenNode::Frame(f) => f.container.effects.clone(),
        PenNode::Group(g) => g.container.effects.clone(),
        PenNode::Rectangle(r) => r.container.effects.clone(),
        PenNode::Path(p) => p.effects.clone(),
        _ => None,
    }
}

/// Convierte una primitiva en un `PenNode::Path` con anchors editables.
///
/// `None` si el nodo no es una primitiva convertible: un `Path` que no lo es
/// (se devuelve clonado, para que llamarlo dos veces no pierda handles), un
/// `Text` (no tiene sentido como geometría), o una primitiva cuyo tamaño no es
/// un número fijo.
///
/// PRESERVA: id, nombre, rol, posición, rotación, constraints, fill, stroke,
/// effects y límites. El índice dentro del padre lo conserva quien sustituye,
/// porque este fichero no toca el árbol.
pub fn convert_primitive_to_path(node: &PenNode) -> Option<PenNode> {
    // Un Path ya es editable. Devolverlo evita un round-trip que perdería
    // handles si se llama dos veces.
    if matches!(node, PenNode::Path(_)) {
        return Some(node.clone());
    }
    // UNA PRIMITIVA CON HIJOS NO SE CONVIERTE. Un Rectangle es contenedor en
    // este schema (children: Option<Vec<PenNode>>), o sea que un "card" es un
    // rect con hijos. Convertirlo a Path destruiria los hijos (extract_node se
    // los lleva) y su auto-layout. Es perdida de datos silenciosa, asi que
    // aqui se niega en vez de arriesgar.
    //
    // OJO al probar: un Frame VACIO si se convierte, porque lo que se mira son
    // los hijos, no el tipo. El rectangulo inicial de un documento nuevo es un
    // Frame vacio, asi que deberia entrar. Si no entra, el problema es el
    // gesto (que no llega a la rama) y no el guard.
    //
    // Lo correcto para un container seria otra operacion ("aplanar"), que no
    // es lo mismo que deformar la forma.
    if node.children().map_or(false, |c| !c.is_empty()) {
        return None;
    }
    let commands = primitive_commands(node)?;
    let (w, h) = primitive_size(node)?;
    let geom = jian_core::commands_to_anchors::commands_to_anchors(&commands);

    // Una linea no cierra; el resto sí.
    let closed = !matches!(node, PenNode::Line(_)) && geom.closed;

    let path = PathNode {
        base: node.base().clone(),
        icon_id: None,
        // `d` se deja a None A PROPOSITO: el loader prefiere `anchors` sobre
        // `svg_path` (adapter/shapes.rs:189-195), y poner `d` haria que se
        // pintase por svg_path en vez de por geometria editable.
        d: None,
        anchors: Some(geom.anchors),
        closed: Some(closed),
        fill_rule: None,
        mask: None,
        width: Some(SizingBehavior::Number(w as f64)),
        height: Some(SizingBehavior::Number(h as f64)),
        fill: fill_of(node),
        stroke: stroke_of(node),
        effects: effects_of(node),
        state: None,
        bindings: None,
        events: None,
        lifecycle: None,
        semantics: None,
        gestures: None,
        route: None,
        limits: Default::default(),
    };
    Some(PenNode::Path(path))
}

/// ¿Es este nodo una primitiva que se puede convertir?
pub fn is_convertible_primitive(node: &PenNode) -> bool {
    convert_primitive_to_path(node).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jian_ops_schema::node::PenPathPointType;

    fn rect(width: f64, height: f64) -> PenNode {
        PenNode::Rectangle(RectangleNode {
            base: jian_ops_schema::node::PenNodeBase {
                id: "r1".into(),
                name: Some("Rect".into()),
                x: Some(10.0),
                y: Some(20.0),
                ..Default::default()
            },
            container: jian_ops_schema::node::container::ContainerProps {
                width: Some(SizingBehavior::Number(width)),
                height: Some(SizingBehavior::Number(height)),
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

    /// El caso central: un rect plano se vuelve un Path con 4 anchors
    /// editables y cerrado.
    #[test]
    fn plain_rect_becomes_editable_path() {
        let out = convert_primitive_to_path(&rect(100.0, 50.0)).expect("debe convertir");
        let PenNode::Path(p) = &out else { panic!("esperaba Path") };
        let anchors = p.anchors.as_ref().expect("debe tener anchors");
        assert_eq!(anchors.len(), 4, "un rect son 4 vertices");
        assert_eq!(p.closed, Some(true));
        // Sin radios: sin handles, y todos cusp.
        for a in anchors {
            assert!(a.handle_in.is_none() && a.handle_out.is_none());
            assert_eq!(a.point_type, Some(PenPathPointType::Corner));
        }
        // Y la geometria es la del rect, en coordenadas absolutas.
        let xs: Vec<f64> = anchors.iter().map(|a| a.x).collect();
        let ys: Vec<f64> = anchors.iter().map(|a| a.y).collect();
        assert_eq!(xs, vec![10.0, 110.0, 110.0, 10.0]);
        assert_eq!(ys, vec![20.0, 20.0, 70.0, 70.0]);
    }

    /// Lo que hace que esto sirva: los anchors NO son None. Un path con
    /// anchors vacios es un path muerto (el bug de los boolean ops).
    #[test]
    fn converted_path_is_never_anchorless() {
        let out = convert_primitive_to_path(&rect(10.0, 10.0)).unwrap();
        let PenNode::Path(p) = &out else { panic!() };
        assert!(!p.anchors.as_ref().unwrap().is_empty());
        assert!(p.d.is_none(), "d debe quedar vacio para que se use anchors");
    }

    /// Un rect con radio produce curvas, o sea handles.
    #[test]
    fn rounded_rect_produces_handles() {
        let mut node = rect(100.0, 100.0);
        if let PenNode::Rectangle(r) = &mut node {
            r.container.corner_radius = Some(CornerRadius::Uniform(12.0));
        }
        let out = convert_primitive_to_path(&node).expect("debe convertir");
        let PenNode::Path(p) = &out else { panic!() };
        let anchors = p.anchors.as_ref().unwrap();
        let con_handles = anchors
            .iter()
            .filter(|a| a.handle_in.is_some() || a.handle_out.is_some())
            .count();
        assert!(
            con_handles >= 4,
            "un rect redondeado debe tener curvas, hallados {}",
            con_handles
        );
    }

    /// Preserva identidad y estilo: el path tiene que seguir SIENDO el mismo
    /// objeto para el usuario, con otro tipo de geometria.
    #[test]
    fn preserves_id_position_and_size() {
        let out = convert_primitive_to_path(&rect(100.0, 50.0)).unwrap();
        assert_eq!(out.base().id, "r1");
        assert_eq!(out.base().x, Some(10.0));
        assert_eq!(out.base().y, Some(20.0));
        let PenNode::Path(p) = &out else { panic!() };
        assert_eq!(p.width, Some(SizingBehavior::Number(100.0)));
        assert_eq!(p.height, Some(SizingBehavior::Number(50.0)));
    }

    /// Idempotente: convertir dos veces no pierde los handles.
    #[test]
    fn converting_twice_is_lossless() {
        let once = convert_primitive_to_path(&rect(100.0, 100.0)).unwrap();
        let twice = convert_primitive_to_path(&once).expect("un Path tambien convierte");
        let (PenNode::Path(a), PenNode::Path(b)) = (&once, &twice) else {
            panic!()
        };
        assert_eq!(a.anchors, b.anchors, "los anchors no deben cambiar");
    }

    /// Un Path que ya existe se devuelve tal cual, sin round-trip.
    #[test]
    fn existing_path_passes_through() {
        let p = convert_primitive_to_path(&rect(10.0, 10.0)).unwrap();
        let again = convert_primitive_to_path(&p).unwrap();
        assert!(matches!(again, PenNode::Path(_)));
        let (PenNode::Path(a), PenNode::Path(b)) = (&p, &again) else { panic!() };
        assert_eq!(a.anchors, b.anchors);
    }

    /// Un tamaño que no es numero fijo no es geometria: no se convierte.
    #[test]
    fn fit_content_rect_is_not_convertible() {
        let mut node = rect(100.0, 50.0);
        if let PenNode::Rectangle(r) = &mut node {
            r.container.height = Some(SizingBehavior::Keyword(
                jian_ops_schema::sizing::SizingKeyword::FitContent,
            ));
        }
        assert!(convert_primitive_to_path(&node).is_none());
    }
}
