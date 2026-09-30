# PROGRESS: editor de nodos / geometría editable

Estado real, con fases y el guard que通过的 cada una.

## El guard (el método)

Una fase NO está hecha hasta que se cumple **todo**:

1. Compila en CI (`main` verde)
2. Los tests pasan
3. El APK ARM64 se construye y valida
4. **Se instala en el móvil, abre, y no crashea**

El paso 4 es el que importa y el que más veces ha atrapado bugs reales. El
CI verde NO dice que la app abra. Nunca dar una fase por buena sin el 4.

Desinstalar/reinstalar borra los datos de la app en el móvil (firma del
keystore debug regenerada por CI). Avisar antes.

## Las fases

### Fase 0 — diagnóstico (CERRADA)

Localizado el off-by-one de reordenación en contenedores con auto-layout
(`["n100","a","b","c"]` en vez de `["a","n100","b","c"]`).

- Descartado: Taffy 0.14. Los tests de `jian-core/leaf_sizing_repro_tests.rs`
  confirman que dimensiona bien las hojas no-texto (80x40) y respeta el gap.
- Descartado: drift de rects de `layout_repair`. Medido con interruptor real
  (`reorder_offbyone_tests.rs`): los rects son idénticos con y sin la capa, y
  el índice sale 1 en ambos.
- Descartado: falta de `mark_document_changed` en el drop. El fix es correcto
  (el doc de `state.rs:291` lo exige) pero NO arregla esto.
- **Medido y real**: `layout_repair` ES necesaria. Sin ella el índice sale 0.
- **Conclusión**: el bug vive en el pipeline de `WidgetHostNative` (scene
  cache, gesture state, `apply_cursor_move`), no en `compute_layout` aislado.
  Sigue abierto, acotado.

Descubrimiento de lado: **`op-pen-loader` no estaba en ningún `cargo test` del
workflow**, así que sus tests no se ejecutaban nunca. Ya está añadido.

### Fase 1 — el shim `SkPath` → `PathCommand` (CERRADA ✅)

`vendor/jian/crates/jian-skia/src/path.rs` → `to_path_commands`.

Por qué era el bloque que faltaba: las primitivas se saben construir como
`SkPath` (`build_rect_path`/`build_oval_path` en `boolean_ops.rs:110/:128`),
pero un `SkPath` no se edita como geometría. La geometría editable vive en
`PathNode.anchors`, y el loader lo dice en `adapter/shapes.rs:192-194`. Por eso
los boolean ops dan hoy un path MUERTO: salen con `anchors: None`
(`host_support_allocator.rs:358`) y borran las fuentes (`:376`).

Usa `Path::iter()` + `PathIterRec.{verb,points}`. **No** `Geometry::decompose`,
que no existe en skia-safe 0.97. `PathVerb` se importa del prelude, no de
`skia_safe::path` (E0603).

Guard: compila, 5 tests verdes, APK valida, abre en el móvil (pid vivo, 35
hilos, sin crash).

### Fase 2 — primitivas → geometría editable (CERRADA ✅)

`vendor/jian/crates/jian-skia/src/shape_to_path.rs`, traducido de
`penpot/penpot` `render-wasm/src/shapes/shape_to_path.rs` (282 líneas, MPL-2.0).

`rect_commands` (radio POR ESQUINA + clamp del W3C en `fix_radius`),
`ellipse_commands` (4 cúbicas con `BEZIER_CIRCLE_C`), `polygon_commands`,
`line_commands`.

Qué aporta frente a lo que ya había: `build_rect_path` llama `add_rect` con
radio **uniforme**, así que un rect con esquinas distintas se renderiza bien
pero al reconstruirlo como path se pierde el radio. Y sin el clamp del W3C, un
rect 60x60 con radios 40 genera geometría rota.

Guard: compila, 9 tests verdes, APK 187,3 MB, abre en el móvil (36 hilos,
estable, sin crash nativo).

### Fase 3 — doble tap entra al editor de nodos (SIGUIENTE)

**Elegido: opción A** (conversión explícita al entrar). Guardar B y C abajo.

El editor de nodos YA EXISTE, pero solo para `PathNode`:

| Pieza | Estado |
|---|---|
| `canvas_path_overlay.rs` (377 L) | ✅ pinta anchors |
| `path_anchor_context_menu.rs` (205 L) | ✅ Corner / Mirrored / Free / Reset |
| `pen.rs` (425 L) | ✅ pen tool |
| `path_edit.rs` (617 L) | ✅ edición de path |
| estado "editando nodos" | ❌ no existe |
| convertir primitiva → path | ❌ no existe (motor ya listo, fases 1-2) |
| añadir/eliminar vértice | ❌ no existe → **Fase 4** |
| tipo `Auto` | ❌ no existe → **Fase 5** |

**Por qué la Fase 3 es barata**: no hay que construir un editor de nodos.
Está hecho. Hay que convertir el nodo a `PathNode` con anchors y dejar que el
editor existente lo atienda.

El doble tap en el select tool YA está ocupado por 4 ramas
(`canvas_select_drag.rs:74`), pero **ninguna aplica a una primitiva**:
`enter_selected_image_crop_edit` (solo imágenes),
`jump_to_deepest_text_edit` (solo texto), `enter_child_scope` (solo si hay hijo
bajo el cursor), `start_text_edit` (solo texto). Un rect, elipse o polígono no
entra en ninguna, así que el gesto está libre justo donde lo queremos. Y no hay
conflicto con el pen tool, que usa doble clic para **terminar** el trazado
(`pen_press.rs:41-42`), otro contexto.

### Fase 4 — añadir / eliminar vértice (PENDIENTE)

Añadir un anchor entre dos, y borrar uno re-derivando los handles de sus
vecinos.

**Requisito previo que no existe**: el estado de "segmento seleccionado"
(elegir la línea entre vértice A y B). Hoy hay `PathAnchorHit` pero no
selección de segmento. Es la mitad del editor de nodos de Inkscape
(`SEGMENT_STRAIGHT` / `SEGMENT_CUBIC_BEZIER`).

### Fase 5 — tipo de nodo `Auto` (PENDIENTE)

Inkscape tiene 4: `CUSP`, `SMOOTH`, `AUTO`, `SYMMETRIC`. OpenPencil tiene 3
(`Corner` / `Mirrored` / `Independent`). Falta `Auto`: los handles se ajustan
solos según los vecinos al mover el nodo. Es el tipo que "hace la curva
bonita solo" y el que más se nota en la práctica.

## Decisiones guardadas por si acaso

### B — conversión no destructiva (GUARDADA)

El rect sigue siendo rect con sus parámetros vivos (`corner-radius` por
esquina, auto-layout), y encima lleva un flag "geometría editable" con los
anchors.

**Por qué no rompe nada volver a ella desde A**: A y B comparten el mismo punto
de entrada —producen un `PathNode` con `anchors`—. Si ese punto está detrás de
una función (p.ej. `convert_primitive_to_path(node) -> PenNode`), cambiar de A
a B es cambiar el cuerpo de esa función y no tocar el editor, el overlay ni el
contexto. Por eso ese punto debe ser UNO y no lógica esparcida.

Coste: mantener los dos modelos sincronizados (un rect con radio de 8 y 4
anchors necesita saber qué pasa al redimensionar).

### C — botón explícito en el panel de Diseño (GUARDADA)

"Convertir objeto a trazos" como `PropertyPanelAction`, que abre el mismo modo.
Útil si en el futuro hay más conversiones (stroke→path, imagen→path) o si
convertir no debe ser automático al hacer doble tap.

El enum `PropertyPanelAction` ya existe (~40 variantes) con precedente de
agrupar (`ToggleCornerExpand` agrupa las 4 esquinas). Añadir una variante es
barato.

**Nota**: con A + C harían falta las dos, porque A es el gesto y C es el
botón. Ahora mismo A basta y el botón sería redundante.

## Lo que no depende de estas fases

- **El animador.** No existe en OpenPencil ni en Graphite (Graphite lo tiene en
  roadmap "late 2026"; en OpenPencil los "keyframes" son recetas de entrada
  para widgets en preview, no un animador). Es el bloque grande y es
  independiente.
- **El off-by-one del auto-layout** (Fase 0). Acotado, abierto.
- **Rect → path de las primitivas**: lo que hacen las fases 1-2 es el motor.
  Falta el comando que lo conecte (Fase 3).
