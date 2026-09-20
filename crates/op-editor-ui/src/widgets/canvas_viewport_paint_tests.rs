//! Sibling test file for `canvas_viewport_paint.rs` (800-line cap
//! convention) — arc tessellation, text-node paint, SVG-path paint,
//! path flattening and `clipContent` child clipping.
//!
//! The individual test modules were split out into the sibling
//! `canvas_viewport_paint_tests/` directory to keep every file under
//! the 800-line cap; this spine only declares them.

#[path = "canvas_viewport_paint_tests/arc_tests.rs"]
mod arc_tests;

#[path = "canvas_viewport_paint_tests/text_tests.rs"]
mod text_tests;

#[path = "canvas_viewport_paint_tests/path_tests.rs"]
mod path_tests;

#[path = "canvas_viewport_paint_tests/clip_tests.rs"]
mod clip_tests;

#[path = "canvas_viewport_paint_tests/stroke_align_tests.rs"]
mod stroke_align_tests;

#[path = "canvas_viewport_paint_tests/per_corner_radius_tests.rs"]
mod per_corner_radius_tests;

#[path = "canvas_viewport_paint_tests/background_blur_tests.rs"]
mod background_blur_tests;

#[path = "canvas_viewport_paint_tests/effect_lod_tests.rs"]
mod effect_lod_tests;
