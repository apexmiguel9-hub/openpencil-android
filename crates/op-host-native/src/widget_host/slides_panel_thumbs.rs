//! Rendered board thumbnails for the left rail's slides tab.
//!
//! Each row shows a REAL render of its board, not an icon, so the cache
//! here is the feature's load-bearing part. Three rules shape it:
//!
//! 1. **Keyed by `(board id, document revision)`.** The revision is the
//!    editor's own `mark_document_changed` counter — the single source
//!    for "did the document change" that the scene cache already
//!    measures against. Nothing here sniffs names, sizes or child
//!    counts to guess at staleness.
//! 2. **Independent of every other cache.** It never reads or writes the
//!    canvas pan cache or the scene cache; both have torn before, and a
//!    navigator must not be able to tear the canvas.
//! 3. **Bounded work per frame.** Rendering six 1920×1080 boards
//!    synchronously inside paint would drop frames, so paint renders at
//!    most [`RENDERS_PER_FRAME`] rows — visible ones first — and only
//!    once the document has been still for [`DEBOUNCE_MS`]. A stale
//!    thumbnail keeps painting meanwhile, which is why typing into a
//!    slide does not make the rail flicker.
//!
//! The raster is produced through the SAME scene painter the canvas
//! uses (`canvas_viewport_paint::paint_node`) over an offscreen surface
//! allocated from the frame's own canvas, so a thumbnail is the board,
//! at 1/10th the size, with the frame's warm image + typeface caches.

use std::collections::HashMap;

use op_editor_ui::layout_scene::SceneNode;
use op_editor_ui::widgets::{canvas_viewport_paint, PaintCx};
use op_editor_ui::{Point2D, Rect};

use crate::backend::NativeFrameBackend;

/// How many thumbnails one frame may render. Two keeps a six-slide deck
/// complete within three frames of opening while leaving the frame
/// budget to the canvas — the surface alloc + scene paint for a
/// 194×109 raster is the expensive half, and it is paid on the UI
/// thread.
pub(in crate::widget_host) const RENDERS_PER_FRAME: usize = 2;

/// How long the document must hold still before stale thumbnails are
/// re-rendered.
///
/// The revision counter ticks on every keystroke and every frame of a
/// node drag; re-rendering on each would be a raster storm that makes
/// the drag itself stutter. 180 ms is long enough to sit out a
/// continuous gesture (a drag emits revisions far faster than that) and
/// short enough that a finished edit shows up as one settling beat
/// rather than a visible wait.
pub(in crate::widget_host) const DEBOUNCE_MS: u64 = 180;

/// One board's cached raster.
struct SlideThumb {
    /// Document revision the raster was made from.
    revision: u64,
    /// Backend raster generation at capture time — a board whose image
    /// finished decoding after the capture would otherwise be served
    /// its placeholder art forever.
    raster_generation: u64,
    /// Logical size the raster was made for; a rail resize re-renders.
    size: Point2D,
    image: skia_safe::Image,
}

/// Thumbnail cache for the slides tab. Owned by the host, never shared.
#[derive(Default)]
pub(in crate::widget_host) struct SlideThumbCache {
    entries: HashMap<String, SlideThumb>,
    /// The revision the cache last saw, and when it changed. Together
    /// they are the debounce.
    last_revision: u64,
    revision_changed_ms: u64,
    /// When the host should wake to finish outstanding thumbnail work.
    ///
    /// The paint pass cannot ask for a repaint directly: the host's
    /// `mark_dirty` also drops the canvas pan cache, and spending the
    /// canvas's cache on a navigator's bookkeeping would be a real
    /// regression. So outstanding work is published as an animation
    /// deadline instead, which is how every other progressive repaint
    /// in the host schedules itself.
    wake_ms: Option<u64>,
    /// Renders performed, for tests to assert the cache actually skips
    /// work rather than merely looking like it does.
    pub(in crate::widget_host) renders: u64,
}

impl SlideThumbCache {
    /// Note the current document revision and return whether the
    /// document has been still long enough to re-render stale rows.
    pub(in crate::widget_host) fn tick(&mut self, revision: u64, now_ms: u64) -> bool {
        if revision != self.last_revision {
            self.last_revision = revision;
            self.revision_changed_ms = now_ms;
            // A first-ever revision (a freshly opened document) still
            // waits out the debounce; one settling beat before the rail
            // fills in is cheaper than racing the document load.
            self.wake_ms = Some(now_ms.saturating_add(DEBOUNCE_MS));
            return false;
        }
        if now_ms.saturating_sub(self.revision_changed_ms) >= DEBOUNCE_MS {
            return true;
        }
        self.wake_ms = Some(self.revision_changed_ms.saturating_add(DEBOUNCE_MS));
        false
    }

    /// Ask the host to come back in `delay_ms` for the next batch.
    pub(in crate::widget_host) fn wake_in(&mut self, now_ms: u64, delay_ms: u64) {
        self.wake_ms = Some(now_ms.saturating_add(delay_ms));
    }

    /// Nothing outstanding — stop waking for thumbnails.
    pub(in crate::widget_host) fn settle(&mut self) {
        self.wake_ms = None;
    }

    /// The host's next thumbnail wake, if any.
    pub(in crate::widget_host) fn wake_deadline_ms(&self) -> Option<u64> {
        self.wake_ms
    }

    /// The cached raster for `board_id`, fresh or stale. Stale rasters
    /// are still drawn — a thumbnail one edit behind reads far better
    /// than a hole.
    pub(in crate::widget_host) fn image(&self, board_id: &str) -> Option<&skia_safe::Image> {
        self.entries.get(board_id).map(|entry| &entry.image)
    }

    /// Whether `board_id` needs a render for this revision / size.
    fn needs_render(
        &self,
        board_id: &str,
        revision: u64,
        raster_generation: u64,
        size: Point2D,
    ) -> bool {
        match self.entries.get(board_id) {
            None => true,
            Some(entry) => {
                entry.revision != revision
                    || entry.raster_generation != raster_generation
                    || (entry.size.x - size.x).abs() > 0.5
                    || (entry.size.y - size.y).abs() > 0.5
            }
        }
    }

    /// Drop rasters for boards the deck no longer has, so a long editing
    /// session cannot grow the cache without bound.
    pub(in crate::widget_host) fn retain_boards(&mut self, live: &[String]) {
        if self.entries.len() <= live.len() {
            return;
        }
        self.entries.retain(|id, _| live.iter().any(|l| l == id));
    }

    /// Render up to [`RENDERS_PER_FRAME`] of the requested boards.
    ///
    /// `wanted` is in priority order — hosts pass visible rows first —
    /// and every entry names a board id, the scene node to paint, and
    /// the raster size that board's own row asked for. The size is
    /// per-entry rather than per-call because rows are a fixed height
    /// and each board is fitted into that box on its own aspect, so a
    /// phone screen and a dashboard on one page want different rasters.
    /// Returns whether anything was rendered, which the host uses to ask
    /// for one more frame so the remaining rows fill in.
    pub(in crate::widget_host) fn render_pending(
        &mut self,
        frame: &mut NativeFrameBackend<'_>,
        wanted: &[(String, &SceneNode, Point2D)],
        revision: u64,
    ) -> bool {
        let raster_generation = frame.raster_generation();
        let mut rendered = 0usize;
        for (board_id, node, size) in wanted {
            if rendered >= RENDERS_PER_FRAME {
                break;
            }
            if !self.needs_render(board_id, revision, raster_generation, *size) {
                continue;
            }
            let Some(image) = render_board(frame, node, *size) else {
                continue;
            };
            self.entries.insert(
                board_id.clone(),
                SlideThumb {
                    revision,
                    raster_generation,
                    size: *size,
                    image,
                },
            );
            self.renders += 1;
            rendered += 1;
        }
        rendered > 0
    }

    /// Whether any requested board still lacks a current raster — the
    /// host's "come back next frame" signal.
    pub(in crate::widget_host) fn has_pending(
        &self,
        frame: &mut NativeFrameBackend<'_>,
        wanted: &[(String, &SceneNode, Point2D)],
        revision: u64,
    ) -> bool {
        let raster_generation = frame.raster_generation();
        wanted
            .iter()
            .any(|(id, _, size)| self.needs_render(id, revision, raster_generation, *size))
    }
}

/// Render one board into an offscreen raster `size` logical px across.
///
/// Paints through the canvas's own scene painter at the zoom that fits
/// the board into `size`, so a thumbnail carries the same fills, image
/// crops, clip scopes and styled text the canvas shows. `size` is the
/// row's own fitted rect, so the raster already has the board's aspect
/// and the fit below is a rounding correction rather than a letterbox.
fn render_board(
    frame: &mut NativeFrameBackend<'_>,
    node: &SceneNode,
    size: Point2D,
) -> Option<skia_safe::Image> {
    let bounds = node.aggregate_bounds();
    if bounds.size.x <= 0.0 || bounds.size.y <= 0.0 || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    // Fit, not fill: a board whose aspect differs from the row's (a
    // mixed deck) is letterboxed rather than cropped, so no slide is
    // shown missing an edge it actually has.
    let zoom = (size.x / bounds.size.x).min(size.y / bounds.size.y);
    let origin = Point2D::new(
        -bounds.origin.x * zoom + (size.x - bounds.size.x * zoom) / 2.0,
        -bounds.origin.y * zoom + (size.y - bounds.size.y * zoom) / 2.0,
    );
    let surface = frame.render_offscreen_layer(
        Rect {
            origin: Point2D::ZERO,
            size,
        },
        0.0,
        |off| {
            let mut cx = PaintCx { backend: off };
            canvas_viewport_paint::paint_node(
                &mut cx,
                node,
                origin,
                zoom,
                Rect {
                    origin: Point2D::ZERO,
                    size,
                },
            );
        },
    );
    Some(surface?.image_snapshot())
}

#[cfg(test)]
#[path = "slides_panel_thumbs_tests.rs"]
mod slides_panel_thumbs_tests;
