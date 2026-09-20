#!/usr/bin/env bash
# tools/check-text-measure.sh — chrome text must be measured in the family
# it is painted in.
#
# `RenderBackend::measure_text` (jian's `Painter::measure_text`) is
# family-BLIND: it resolves the backend's default typeface — the bundled
# Roboto on native, the CanvasKit default in the browser — while every chrome
# string is DRAWN as a named run (`system-ui`, which macOS resolves to
# `.AppleSystemUIFont`). SF Pro is wider than Roboto at the same point size,
# so the blind call under-reports the painted width.
#
# Nothing errors when that happens. An ellipsizer believes a string fits and
# emits no `…`, so the content clip shears the last glyph in half; a centred
# label lands left of centre; a tooltip bubble sized as `measured + padding`
# is born too narrow for its own text; a caret drifts off the glyph being
# edited. And none of it reproduces in CI, because every test backend
# measures blind and family-aware identically — the bug is visible only by
# eye, on a real machine. That is what makes this worth a build gate.
#
# The sanctioned path is `crate::widgets::text_metrics` (`measure_chrome` /
# `fit_chrome` / `centered_text_x` / `measure_in_family`), which routes to
# `measure_text_family` with the run's real family.
#
# Exit semantics:
#   0   PASS — no family-blind measurement in widget code.
#   1   FAIL — a widget module called `measure_text` directly.

set -euo pipefail

WIDGETS="crates/op-editor-ui/src/widgets"

# Files that legitimately name the blind call:
#   text_metrics.rs          — defines the sanctioned wrappers, and its own
#                              tests compare blind against family-aware.
#   text_input_backend.rs    — a decorating `RenderBackend` that must forward
#                              every trait method, blind one included.
#   test_family_gap_backend.rs / test_capture_backend.rs
#                            — test backends implementing the trait.
#   text_metrics_paint_tests.rs
#                            — the cross-panel guard's negative control
#                              paints a deliberately blind fitter to prove the
#                              guard can fail.
ALLOWED_RE='^crates/op-editor-ui/src/widgets/(text_metrics|text_metrics_paint_tests|text_input_backend|test_family_gap_backend|test_capture_backend)\.rs:'

# `.measure_text(` as a method call, excluding the family-aware and
# weight-aware siblings (`measure_text_family`, `measure_text_family_styled`,
# `measure_text_weighted`, `measure_text_styled`) and trait-impl signatures
# (`fn measure_text(`).
hits="$(grep -RInE '\.[[:space:]]*measure_text[[:space:]]*\(' "${WIDGETS}" 2>/dev/null \
  | grep -vE "${ALLOWED_RE}" \
  || true)"

if [ -n "${hits}" ]; then
  printf 'FAIL: family-blind text measurement in widget code\n' >&2
  printf '\n' >&2
  printf '%s\n' "${hits}" >&2
  printf '\n' >&2
  printf 'RenderBackend::measure_text resolves the backend default font, not the\n' >&2
  printf 'family these runs are painted with. Measure through\n' >&2
  printf 'crate::widgets::text_metrics instead:\n' >&2
  printf '\n' >&2
  printf '  cx.backend.measure_text(s, size)\n' >&2
  printf '    -> text_metrics::measure_chrome(cx.backend, s, size)\n' >&2
  printf '  ellipsize_to_width(s, w, |t| cx.backend.measure_text(t, size))\n' >&2
  printf '    -> text_metrics::fit_chrome(cx.backend, s, w, size)\n' >&2
  printf '  rect.origin.x + (rect.size.x - measured) / 2.0\n' >&2
  printf '    -> text_metrics::centered_text_x(cx.backend, s, size, rect)\n' >&2
  printf '\n' >&2
  printf 'For a run drawn in some other family (a jian component painting in\n' >&2
  printf '"Inter", a monospace readout), name that family:\n' >&2
  printf '  text_metrics::measure_in_family / fit_in_family.\n' >&2
  exit 1
fi

echo "PASS: text measurement check"
