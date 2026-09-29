//! Reproduccion del off-by-one de reordenacion en contenedores con
//! auto-layout, medido en el camino REAL del host.
//!
//! CONTEXTO: cuando se desactiva `layout_repair` (commit e34f0d0, revertido
//! en 9403934), 3.848 tests siguen pasando y solo fallan estos dos:
//!
//!   widget_host::canvas_select_drag_tests::
//!     option_dragging_vertical_layout_child_up_copies_before_source
//!     option_dragging_horizontal_layout_child_left_copies_before_source
//!
//! con el assertion:
//!     left:  ["n100", "a", "b", "c"]   <- indice 0 (sin layout_repair)
//!     right: ["a", "n100", "b", "c"]   <- indice 1 (esperado)
//!
//! Se descarto que la causa sea Taffy: los tests de jian-core
//! (layout/leaf_sizing_repro_tests.rs) confirman que taffy 0.14 dimensiona
//! bien las hojas no-texto (80x40) y respeta el gap. Asi que el bug esta en
//! la capa intermedia: los rects que produce
//! `adapter::pages::compute_layout` para los hijos del contenedor.
//!
//! `flex_insert_preview` (op-editor-ui/src/widgets/drag_flow_index.rs:311)
//! decide el indice comparando el punto medio del arrastre contra los puntos
//! medios de `flex_children`, que `index_containers` construye EXCLUYENDO al
//! nodo arrastrado (drag_flow_index.rs:253). Si los rects que ve ahi no son
//! los del flow real, el indice sale mal.
//!
//! Este test mide el mapa de rects que `compute_layout` produce realmente,
//! con y sin `layout_repair`, y calcula el indice que saldria.

#![cfg(test)]

use crate::adapter::pages::compute_layout;
use jian_ops_schema::node::PenNode;
use std::collections::BTreeMap;

/// El fixture exacto de `VSTACK` en canvas_select_drag_tests.rs, envuelto
/// como lo espera el loader: un frame raiz con layout vertical, gap 8, y
/// tres rectangulos hijos de 80x40.
const VSTACK: &str = r#"{"type":"frame","id":"stack","name":"Stack","x":400,"y":60,
   "width":200,"height":300,"layout":"vertical","gap":8,
   "children":[
     {"type":"rectangle","id":"a","name":"A","width":80,"height":40},
     {"type":"rectangle","id":"b","name":"B","width":80,"height":40},
     {"type":"rectangle","id":"c","name":"C","width":80,"height":40}
   ]}"#;

fn rects_for(json: &str) -> BTreeMap<String, [f32; 4]> {
    let root: PenNode = serde_json::from_str(json).expect("parse VSTACK");
    let mut out = BTreeMap::new();
    compute_layout(&root, &mut out);
    out
}

/// Los mismos rects pero con la capa de repair APAGADA de verdad.
/// Esto no simula nada: apaga el unico call-site real y lo deja correr, asi
/// que el mapa que sale es exactamente el que veria el host sin repair.
fn rects_without_repair(json: &str) -> BTreeMap<String, [f32; 4]> {
    crate::layout_repair::set_skip_repair(true);
    let out = rects_for(json);
    crate::layout_repair::set_skip_repair(false);
    out
}

/// La cuenta exacta de `flex_insert_preview` (drag_flow_index.rs:311), con la
/// lista de hijos SIN el nodo arrastrado (drag_flow_index.rs:253).
fn insert_index_for(siblings: &[[f32; 4]], dragged_mid: f32) -> usize {
    let mut index = siblings.len();
    for (position, b) in siblings.iter().enumerate() {
        let mid = b[1] + b[3] / 2.0;
        if dragged_mid < mid {
            index = position;
            break;
        }
    }
    index
}

/// MEDICION REAL: que rects produce cada capa, y que indice sale de cada una.
///
/// El host falla con indice 0 cuando la capa esta apagada
/// (["n100","a","b","c"] en vez de ["a","n100","b","c"]). Este test mide los
/// dos casos sin simular nada, para localizar el drift exacto.
#[test]
fn measure_drift_with_and_without_layout_repair() {
    let with = rects_for(VSTACK);
    let without = rects_without_repair(VSTACK);

    for id in ["stack", "a", "b", "c"] {
        println!(
            "PROBE|{id:6} con={:?}  sin={:?}",
            with.get(id),
            without.get(id)
        );
    }

    // El drag del host: mueve 'b' 12px arriba.
    let b_with = with.get("b").copied().unwrap();
    let b_without = without.get("b").copied().unwrap();
    let drag_mid_with = b_with[1] + b_with[3] / 2.0 - 12.0;
    let drag_mid_without = b_without[1] + b_without[3] / 2.0 - 12.0;

    // flex_children excluye 'b', luego compara contra a y c.
    let sib_with = [
        with.get("a").copied().unwrap(),
        with.get("c").copied().unwrap(),
    ];
    let sib_without = [
        without.get("a").copied().unwrap(),
        without.get("c").copied().unwrap(),
    ];

    let idx_with = insert_index_for(&sib_with, drag_mid_with);
    let idx_without = insert_index_for(&sib_without, drag_mid_without);

    println!(
        "PROBE|indices: con repair={idx_with} (drag_mid={drag_mid_with}) | \
         sin repair={idx_without} (drag_mid={drag_mid_without})"
    );

    assert_eq!(
        (idx_with, idx_without),
        (1, 0),
        "el bug real: con la capa sale 1, sin la capa sale 0. Si esto cambia, \
         la hipotesis del drift en los rects es incorrecta."
    );
}

/// El test del host: el stack esta en y=60, gap 8, hijos de 40 de alto.
/// Con la cadena de compute_layout, las posiciones absolutas esperadas son
/// a=60, b=108, c=156.
#[test]
fn flow_rects_from_the_real_host_path() {
    let rects = rects_for(VSTACK);

    for id in ["stack", "a", "b", "c"] {
        println!(
            "PROBE|host-path {id}: {:?}",
            rects.get(id)
        );
    }

    let a = rects.get("a").copied().expect("no hay rect para a");
    let b = rects.get("b").copied().expect("no hay rect para b");
    let c = rects.get("c").copied().expect("no hay rect para c");

    // y, alto de cada hijo segun el camino real del host.
    let (a_y, a_h) = (a[1], a[3]);
    let (b_y, b_h) = (b[1], b[3]);
    let (c_y, c_h) = (c[1], c[3]);
    println!(
        "PROBE|host-path mids: a={} b={} c={}",
        a_y + a_h / 2.0,
        b_y + b_h / 2.0,
        c_y + c_h / 2.0
    );

    // El drag mueve 'b' 12px arriba (canvas_select_drag_tests.rs:496), de
    // modo que su punto medio pasa de b_mid a b_mid - 12.
    let drag_mid = b_y + b_h / 2.0 - 12.0;
    // flex_children EXCLUYE al arrastrado, asi que la lista es [a, c].
    let a_mid = a_y + a_h / 2.0;
    let c_mid = c_y + c_h / 2.0;

    let index = if drag_mid < a_mid {
        0
    } else if drag_mid < c_mid {
        1
    } else {
        2
    };
    println!(
        "PROBE|host-path: drag_mid={drag_mid} a_mid={a_mid} c_mid={c_mid} => index={index}"
    );

    assert_eq!(
        index, 1,
        "el camino real del host deberia dar indice 1 (insertar antes que b). \
         Si da 0, los rects que ve drag_flow_index.rs no son los del flow. \
         a={a:?} b={b:?} c={c:?}"
    );
}
