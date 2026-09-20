//! Canonical `.op` / `.pen` loader.
//!
//! Bridges the `jian-ops-schema` canonical `PenDocument` into the
//! desktop's private `DocPayload`. Two responsibilities:
//!
//! 1. Convert each `PenNode` variant into a `NodePayload` carrying
//!    geometry + style. All 12 schema variants are routed.
//! 2. Defer flex layout to `jian-core::LayoutEngine` — the same
//!    taffy-backed solver that drives the (read-only) jian runtime
//!    against the same schema. Reusing it keeps OpenPencil's
//!    rendering bit-identical with what the TS editor and the
//!    canonical jian renderer produce.
//!
//! OpenPencil's canvas is infinite + unrouted, so each page-root
//! gets its own `LayoutEngine::compute` pass with `available =
//! (root_w, root_h)` (or a generous default when the root is
//! `fit_content`). Computed absolute scene-coord rects are baked
//! into each `NodePayload.bounds`.
//!
//! ### Module layout
//!
//! This file is the spine: the shared imports, the fallback canvas
//! constants and the measure-backend selection. The conversion code
//! lives in sibling submodules (per the 800-line-per-file ceiling) and
//! is glob re-exported here, so every existing `adapter::*` import
//! path still resolves:
//!
//! - [`entry`] — public `PenDocument` → `DocPayload` entry points
//! - `pages` — page assembly + the taffy layout pass
//! - [`geometry`] — root origin / available-size probes
//! - `node_payload` — the `PenNode` → `NodePayload` dispatcher
//! - `shapes` — the per-variant payload builders

pub mod entry;
pub mod geometry;
pub(crate) mod node_payload;
pub(crate) mod pages;
mod shapes;

pub use entry::*;
pub use geometry::*;
pub(crate) use node_payload::*;
pub(crate) use pages::*;
use shapes::*;

use std::{collections::BTreeMap, rc::Rc};

use jian_core::document::NodeTree;
use jian_core::layout::{measure::MeasureBackend, LayoutEngine};
use jian_ops_schema::{
    node::base::PenNodeBase,
    node::container::{AlignItems, ContainerProps, CornerRadius, LayoutMode, Padding},
    node::{
        EllipseNode, FontWeight, FrameNode, GroupNode, IconFontNode, ImageNode, LineNode, PathNode,
        PenNode, PolygonNode, RectangleNode, TextNode,
    },
    sizing::SizingBehavior,
    PenDocument,
};

use crate::payload::{DocPayload, NodePayload, PagePayload, StrokePayload};
use crate::style_payload::{
    apply_container_style, assign_first_fill, base_payload, image_node_adjustments,
    image_node_fit_to_payload, short_src, stroke_to_payload,
};

/// Default canvas allotment for a page-root sized with flex tokens
/// (`fill_container` / `fit_content`) and no authored bounds — large
/// enough to let real designs fill out without truncating layout,
/// small enough to avoid pathological taffy work.
const ROOT_FALLBACK_W: f32 = 1440.0;
const ROOT_FALLBACK_H: f32 = 900.0;

#[derive(Clone, Copy)]
struct TextChildCenterContext {
    x: f32,
    w: f32,
}

thread_local! {
    static LAYOUT_MEASURE_BACKEND: Rc<dyn MeasureBackend> = make_measure_backend();
}

/// Real skia paragraph shaper — native + web-skia builds (`skia-measure`, default).
/// Wrapped in a memoizing cache: paragraph shaping is the dominant layout cost,
/// and repeat reconversions (drag / resize / colour edits) re-measure identical
/// text, so the cache turns those into hash lookups. (The estimate backend below
/// is already cheap, so it is left unwrapped.)
#[cfg(feature = "skia-measure")]
fn make_measure_backend() -> Rc<dyn MeasureBackend> {
    // Windows CI sets this for tests that load op-pen-loader as a dependency.
    if cfg!(target_os = "windows") && std::env::var_os("OP_TEST_ESTIMATE_TEXT_MEASURE").is_some() {
        return jian_core::layout::measure::default_backend();
    }

    Rc::new(crate::measure_cache::CachingMeasureBackend::new(Rc::new(
        jian_skia::SkiaMeasure::new(),
    )))
}

/// Skia-free estimate backend — the CanvasKit web build links no jian-skia /
/// skia-safe. It is a character-count heuristic (~10% width error); the
/// CanvasKit backend re-measures glyphs exactly at paint time, so layout drift
/// is bounded to flex sizing of unconstrained text.
#[cfg(not(feature = "skia-measure"))]
fn make_measure_backend() -> Rc<dyn MeasureBackend> {
    jian_core::layout::measure::default_backend()
}

use crate::path_bounds::path_bounds_from_anchors;

#[cfg(test)]
#[path = "adapter_tests.rs"]
mod tests;
