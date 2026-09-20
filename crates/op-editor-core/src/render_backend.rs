//! Compatibility re-exports for the widget painter facade.
//!
//! The implementation moved to `jian-widgets` as part of the
//! cross-platform component-library extraction. Existing OP code keeps
//! importing this module during migration; new shared widget code should
//! import from `jian_widgets` directly.
//!
//! # Calling convention: never measure chrome text with `measure_text`
//!
//! `RenderBackend::measure_text` is family-BLIND — it resolves the backend's
//! default typeface (bundled Roboto on native), while chrome strings are
//! drawn as named `system-ui` runs that native resolves through the system
//! `FontMgr`. The blind call therefore under-reports the painted width, and
//! nothing errors: a fitter emits no ellipsis and the clip shears a glyph in
//! half, a centred label sits off-centre, a bubble is born too narrow for its
//! own text. It does not reproduce in CI either, because the test backends
//! measure both ways identically.
//!
//! Chrome measurement goes through `op_editor_ui::widgets::text_metrics`
//! (`measure_chrome` / `fit_chrome` / `centered_text_x` / `measure_in_family`),
//! which names the family the run is painted in. `tools/check-text-measure.sh`
//! enforces this for `op-editor-ui/src/widgets/`.

pub use jian_widgets::geometry::{Color, Point2D, Rect};
pub use jian_widgets::painter::{
    ImageAdjustments, ImageBlendMode, ImageDrawMode, Painter as RenderBackend, TextBaselineRequest,
    TextLayout,
};
